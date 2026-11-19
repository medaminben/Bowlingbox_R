//! `BOWLINGBOX_RENDER_STATS=1`: GPU time per pass from timestamp queries, averaged and printed
//! about once a second (C++ `GpuTimer`). The queries rotate through `RING` sets; a set is read
//! only once its buffer is mapped, so a frame never waits on the GPU, and a frame that finds no
//! free set goes untimed.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

/// The passes the stats time. `Post` covers the resolve, the bloom chain and the composite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimedPass {
    Shadow,
    Reflection,
    Main,
    Post,
}

const PASSES: usize = 4;

/// Per-pass sums since the last line.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PassStats {
    sum_ms: [f64; PASSES],
    samples: [u32; PASSES],
    frames: u32,
}

impl PassStats {
    /// One measurement of `pass`.
    pub fn add(&mut self, pass: TimedPass, ms: f64) {
        self.sum_ms[pass as usize] += ms;
        self.samples[pass as usize] += 1;
    }

    /// One frame drawn, timed or not.
    pub fn end_frame(&mut self) {
        self.frames += 1;
    }

    /// The C++ line: each pass's average over its measurements, 0 for a pass without any.
    #[must_use]
    pub fn line(&self) -> String {
        let avg = |p: TimedPass| {
            let n = self.samples[p as usize];
            if n > 0 {
                self.sum_ms[p as usize] / f64::from(n)
            } else {
                0.0
            }
        };
        format!(
            "render: shadow {:.2} ms, reflection {:.2} ms, main {:.2} ms, post {:.2} ms (GPU, {} frames)",
            avg(TimedPass::Shadow),
            avg(TimedPass::Reflection),
            avg(TimedPass::Main),
            avg(TimedPass::Post),
            self.frames
        )
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// `BOWLINGBOX_RENDER_STATS` turns the stats on with exactly `1`.
#[must_use]
pub fn render_stats_from_env(value: Option<&str>) -> bool {
    value == Some("1")
}

/// A line is due once a second has passed since the last one (C++ `endFrame`).
#[must_use]
pub fn stats_due(elapsed: Duration) -> bool {
    elapsed >= Duration::from_secs(1)
}

/// Printed once when the stats are on and the device has no `TIMESTAMP_QUERY`.
#[must_use]
pub fn stats_unsupported_line() -> &'static str {
    "render stats: timestamp queries not supported by this adapter"
}

/// Query sets in flight; C++ `GpuTimer::kFrames`.
const RING: usize = 3;
/// A begin and an end timestamp per pass.
const QUERIES: u32 = 2 * PASSES as u32;
const RESULT_BYTES: u64 = QUERIES as u64 * wgpu::QUERY_SIZE as u64;
/// Each pass's pair resolves on its own: a pass that did not run leaves its queries unwritten,
/// and those must not be resolved.
const PAIR_BYTES: u64 = 2 * wgpu::QUERY_SIZE as u64;

const FREE: u8 = 0;
/// Recorded; the buffer maps once the commands are submitted and done.
const PENDING: u8 = 1;
const MAPPED: u8 = 2;
const FAILED: u8 = 3;

struct Slot {
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    state: Arc<AtomicU8>,
    /// The passes that wrote timestamps.
    timed: [bool; PASSES],
}

/// The timestamp writes of one frame's set of queries.
#[derive(Clone, Copy)]
pub(crate) struct PassWrites<'a> {
    set: &'a wgpu::QuerySet,
    base: u32,
}

impl<'a> PassWrites<'a> {
    /// Writes `pass`'s start timestamp when `begin`, its end timestamp when `end`.
    pub(crate) fn span(
        self,
        pass: TimedPass,
        begin: bool,
        end: bool,
    ) -> wgpu::RenderPassTimestampWrites<'a> {
        let index = self.base + 2 * pass as u32;
        wgpu::RenderPassTimestampWrites {
            query_set: self.set,
            beginning_of_pass_write_index: begin.then_some(index),
            end_of_pass_write_index: end.then_some(index + 1),
        }
    }

    pub(crate) fn whole(self, pass: TimedPass) -> wgpu::RenderPassTimestampWrites<'a> {
        self.span(pass, true, true)
    }
}

/// The timestamp queries, their read-back ring and the sums for the next line.
pub(crate) struct GpuTimer {
    set: wgpu::QuerySet,
    slots: Vec<Slot>,
    /// Nanoseconds per timestamp tick.
    period: f64,
    /// The slot this frame writes into; `None` when every slot is still in flight.
    current: Option<usize>,
    stats: PassStats,
    last: Instant,
    line: Option<String>,
}

impl GpuTimer {
    /// `None` when `device` lacks `TIMESTAMP_QUERY`.
    pub(crate) fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }
        let set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("pass timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: QUERIES * RING as u32,
        });
        let buffer = |label, size, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        let slots = (0..RING)
            .map(|_| Slot {
                resolve: buffer(
                    "timestamp resolve",
                    PASSES as u64 * wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT,
                    wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                ),
                readback: buffer(
                    "timestamp readback",
                    RESULT_BYTES,
                    wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                ),
                state: Arc::new(AtomicU8::new(FREE)),
                timed: [false; PASSES],
            })
            .collect();
        Some(Self {
            set,
            slots,
            period: f64::from(queue.get_timestamp_period()),
            current: None,
            stats: PassStats::default(),
            last: Instant::now(),
            line: None,
        })
    }

    /// Adds the results that are ready and picks this frame's slot. Never waits.
    pub(crate) fn begin_frame(&mut self, device: &wgpu::Device) {
        // Runs the map callbacks of finished frames; returns at once.
        let _ = device.poll(wgpu::PollType::Poll);
        for slot in &mut self.slots {
            match slot.state.load(Ordering::Acquire) {
                MAPPED => {
                    let ticks: Vec<u64> = slot.readback.get_mapped_range(..).map_or_else(
                        |_| Vec::new(),
                        |bytes| {
                            let words = bytes.as_chunks::<8>().0;
                            words.iter().map(|b| u64::from_le_bytes(*b)).collect()
                        },
                    );
                    slot.readback.unmap();
                    for (p, pass) in [
                        TimedPass::Shadow,
                        TimedPass::Reflection,
                        TimedPass::Main,
                        TimedPass::Post,
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let (Some(&begin), Some(&end)) = (ticks.get(2 * p), ticks.get(2 * p + 1))
                        else {
                            continue;
                        };
                        if slot.timed[p] && end > begin {
                            // Tick counts of a frame fit an f64 exactly enough for ms.
                            #[allow(clippy::cast_precision_loss)]
                            let ns = (end - begin) as f64 * self.period;
                            self.stats.add(pass, ns / 1e6);
                        }
                    }
                    slot.state.store(FREE, Ordering::Release);
                }
                FAILED => slot.state.store(FREE, Ordering::Release),
                _ => {}
            }
        }
        self.current = self
            .slots
            .iter()
            .position(|s| s.state.load(Ordering::Acquire) == FREE);
    }

    /// This frame's timestamp writes; `None` when the frame goes untimed.
    pub(crate) fn writes(&self) -> Option<PassWrites<'_>> {
        self.current.map(|i| PassWrites {
            set: &self.set,
            base: i as u32 * QUERIES,
        })
    }

    /// Resolves this frame's queries for read-back after submission (`timed` are the passes that
    /// wrote them), counts the frame and keeps a line when one is due.
    pub(crate) fn end_frame(&mut self, encoder: &mut wgpu::CommandEncoder, timed: [bool; PASSES]) {
        if let Some(i) = self.current.take() {
            let slot = &mut self.slots[i];
            let base = i as u32 * QUERIES;
            for p in (0..PASSES).filter(|&p| timed[p]) {
                let first = base + 2 * p as u32;
                let at = p as u64 * wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT;
                encoder.resolve_query_set(&self.set, first..first + 2, &slot.resolve, at);
                let to = p as u64 * PAIR_BYTES;
                encoder.copy_buffer_to_buffer(&slot.resolve, at, &slot.readback, to, PAIR_BYTES);
            }
            slot.timed = timed;
            slot.state.store(PENDING, Ordering::Release);
            let state = Arc::clone(&slot.state);
            encoder.map_buffer_on_submit(&slot.readback, wgpu::MapMode::Read, .., move |result| {
                state.store(
                    if result.is_ok() { MAPPED } else { FAILED },
                    Ordering::Release,
                );
            });
        }
        self.stats.end_frame();
        let now = Instant::now();
        if stats_due(now - self.last) {
            self.line = Some(self.stats.line());
            self.stats.reset();
            self.last = now;
        }
    }

    /// The line due since the last call, if any.
    pub(crate) fn take_line(&mut self) -> Option<String> {
        self.line.take()
    }
}
