//! `--capture=<path>`: opens the `--screen` (the game when none is given), lets it settle, takes one
//! screenshot, writes it as PNG and closes the window. A failure prints `capture failed: <reason>`
//! and exits 1 (see `main`). `cargo bk manual capture` drives this.
//!
//! Pixels: the PNG has the size of the window in physical pixels. The window asks for 1280x800
//! logical pixels, which is 1280x800 at scale factor 1, the scale of the capture machines (xvfb and
//! WARP). On a scaled display the file is larger and `size_note` says so on stderr.

use bowlingbox_audio::{AudioOutput, SilentOutput};
use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// The window of a capture run, in logical pixels.
pub const WINDOW: [u16; 2] = [1280, 800];
/// Frames drawn before the screenshot is asked for.
pub const MIN_FRAMES: u32 = 10;
/// The screen is given up as unsettled after this many frames.
const SETTLE_LIMIT: u32 = 1200;
/// The screenshot is given up as lost this many frames after the request.
const IMAGE_LIMIT: u32 = 600;

/// What the capture does after a frame.
#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    Wait,
    /// Ask the window for a screenshot now.
    Request,
    /// The screenshot is there: write it and close.
    Write,
    Fail(&'static str),
}

/// The settle rule: at least [`MIN_FRAMES`] frames and the follow camera at home.
pub fn settled(frames: u32, camera_home: bool) -> bool {
    frames >= MIN_FRAMES && camera_home
}

/// The capture's progress, frame by frame. It knows nothing of the window.
#[derive(Debug, Default)]
pub struct Capture {
    frames: u32,
    requested_at: Option<u32>,
}

impl Capture {
    /// Call once per drawn frame. `image_ready` says the screenshot has arrived.
    pub fn frame(&mut self, camera_home: bool, image_ready: bool) -> Step {
        self.frames += 1;
        if let Some(at) = self.requested_at {
            return if image_ready {
                Step::Write
            } else if self.frames - at > IMAGE_LIMIT {
                Step::Fail("the window gave no screenshot")
            } else {
                Step::Wait
            };
        }
        if settled(self.frames, camera_home) {
            self.requested_at = Some(self.frames);
            Step::Request
        } else if self.frames > SETTLE_LIMIT {
            Step::Fail("the screen did not settle")
        } else {
            Step::Wait
        }
    }
}

/// Where the PNG is written first; renaming it onto `path` makes the file appear whole or not at all.
pub fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

/// The pixels as opaque 8-bit RGB rows.
pub fn rgb_bytes(image: &egui::ColorImage) -> Vec<u8> {
    image
        .pixels
        .iter()
        .flat_map(|p| [p.r(), p.g(), p.b()])
        .collect()
}

/// A line for stderr when the screenshot is not the 1280x800 the manual expects.
pub fn size_note(size: [usize; 2]) -> Option<String> {
    let expected = WINDOW.map(usize::from);
    (size != expected).then(|| {
        format!(
            "capture: the window is {}x{} pixels, not {}x{} (a scaled display?); written as is",
            size[0], size[1], expected[0], expected[1]
        )
    })
}

fn encode(image: &egui::ColorImage, file: std::fs::File) -> Result<(), String> {
    let [width, height] = image.size;
    let dimension = |n: usize| u32::try_from(n).map_err(|e| e.to_string());
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(file),
        dimension(width)?,
        dimension(height)?,
    );
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer
        .write_image_data(&rgb_bytes(image))
        .map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())
}

/// Writes `image` as PNG to a temporary file next to `path`, then renames it to `path`. A failure
/// leaves no temporary file behind and `path` as it was.
pub fn write_png(path: &Path, image: &egui::ColorImage) -> Result<(), String> {
    let temp = temp_path(path);
    let result = std::fs::File::create(&temp)
        .map_err(|e| format!("cannot write {}: {e}", temp.display()))
        .and_then(|file| encode(image, file))
        .and_then(|()| {
            std::fs::rename(&temp, path)
                .map_err(|e| format!("cannot write {}: {e}", path.display()))
        });
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

/// The output for a run: silence for a capture run, whose `open` is then never called, so no audio
/// device is touched.
pub fn audio_output(
    capturing: bool,
    open: impl FnOnce() -> (Box<dyn AudioOutput>, Option<String>),
) -> (Box<dyn AudioOutput>, Option<String>) {
    if capturing {
        (Box::new(SilentOutput::default()), None)
    } else {
        open()
    }
}

/// How a capture run ended: `Ok` once the PNG is written.
pub type Outcome = Arc<Mutex<Option<Result<(), String>>>>;

/// What `main` reports: a window error, or a window closed early, is a failure too.
pub fn finish(window: Result<(), String>, outcome: &Outcome) -> Result<(), String> {
    window?;
    outcome
        .lock()
        .map_err(|e| e.to_string())?
        .take()
        .unwrap_or_else(|| Err("the window closed before the screenshot was taken".to_owned()))
}

/// A capture run inside the window: the progress, the screenshot slot and the outcome.
pub struct Run {
    path: PathBuf,
    capture: Capture,
    image: Arc<Mutex<Option<Arc<egui::ColorImage>>>>,
    outcome: Outcome,
}

impl Run {
    pub fn new(path: PathBuf, outcome: Outcome) -> Self {
        Self {
            path,
            capture: Capture::default(),
            image: Arc::default(),
            outcome,
        }
    }

    /// Call at the end of every frame. Keeps the window drawing, asks for the screenshot, writes
    /// it and closes the window.
    pub fn frame(&mut self, ctx: &egui::Context, camera_home: bool) {
        let ready = self.image.lock().is_ok_and(|slot| slot.is_some());
        ctx.request_repaint();
        let result = match self.capture.frame(camera_home, ready) {
            Step::Wait => return,
            Step::Request => {
                let slot = Arc::clone(&self.image);
                let wake = ctx.clone();
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
                    egui::ScreenshotCallback::new(move |image| {
                        if let Ok(mut slot) = slot.lock() {
                            *slot = Some(image);
                        }
                        wake.request_repaint();
                    }),
                ));
                return;
            }
            Step::Write => self.write(),
            Step::Fail(reason) => Err(reason.to_owned()),
        };
        if let Ok(mut outcome) = self.outcome.lock() {
            *outcome = Some(result);
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn write(&self) -> Result<(), String> {
        let image = self
            .image
            .lock()
            .map_err(|e| e.to_string())?
            .take()
            .ok_or("the screenshot is gone")?;
        if let Some(note) = size_note(image.size) {
            eprintln!("{note}");
        }
        write_png(&self.path, &image)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(width: usize, height: usize) -> egui::ColorImage {
        let pixels = (0..width * height)
            .map(|i| egui::Color32::from_rgb(u8::try_from(i % 251).unwrap(), 7, 9))
            .collect();
        egui::ColorImage::new([width, height], pixels)
    }

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("bowlingbox-capture-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn settled_needs_ten_frames_and_the_camera_home() {
        assert!(!settled(9, true));
        assert!(settled(10, true));
        assert!(!settled(500, false));
    }

    #[test]
    fn the_screenshot_is_asked_for_once_after_ten_frames() {
        let mut capture = Capture::default();
        for _ in 0..9 {
            assert_eq!(capture.frame(true, false), Step::Wait);
        }
        assert_eq!(capture.frame(true, false), Step::Request);
        assert_eq!(capture.frame(true, false), Step::Wait);
        assert_eq!(capture.frame(true, true), Step::Write);
    }

    #[test]
    fn a_camera_away_from_home_delays_the_request() {
        let mut capture = Capture::default();
        for _ in 0..30 {
            assert_eq!(capture.frame(false, false), Step::Wait);
        }
        assert_eq!(capture.frame(true, false), Step::Request);
    }

    #[test]
    fn an_unsettled_screen_fails_instead_of_hanging() {
        let mut capture = Capture::default();
        let last = (0..=SETTLE_LIMIT)
            .map(|_| capture.frame(false, false))
            .last();
        assert_eq!(last, Some(Step::Fail("the screen did not settle")));
    }

    #[test]
    fn a_lost_screenshot_fails_instead_of_hanging() {
        let mut capture = Capture::default();
        for _ in 0..9 {
            capture.frame(true, false);
        }
        assert_eq!(capture.frame(true, false), Step::Request);
        for _ in 0..IMAGE_LIMIT {
            assert_eq!(capture.frame(true, false), Step::Wait);
        }
        assert_eq!(
            capture.frame(true, false),
            Step::Fail("the window gave no screenshot")
        );
    }

    #[test]
    fn the_hidpi_rule_writes_the_window_size_and_notes_a_scaled_one() {
        assert_eq!(size_note([1280, 800]), None);
        let note = size_note([2560, 1600]).unwrap();
        assert!(
            note.contains("2560x1600") && note.contains("1280x800"),
            "{note}"
        );
    }

    #[test]
    fn the_temp_file_sits_next_to_the_target() {
        assert_eq!(
            temp_path(Path::new("/a/b/game.png")),
            Path::new("/a/b/game.png.tmp")
        );
    }

    #[test]
    fn rgb_bytes_drops_alpha() {
        let image = egui::ColorImage::new(
            [2, 1],
            vec![
                egui::Color32::from_rgb(1, 2, 3),
                egui::Color32::from_rgb(4, 5, 6),
            ],
        );
        assert_eq!(rgb_bytes(&image), [1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn write_png_makes_a_png_of_the_image_and_leaves_no_temp_file() {
        let dir = scratch("ok");
        let path = dir.join("shot.png");
        write_png(&path, &image(5, 3)).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        let decoder = png::Decoder::new(std::io::Cursor::new(&bytes));
        let mut reader = decoder.read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut buf).unwrap();
        assert_eq!((info.width, info.height), (5, 3));
        assert_eq!(&buf[..3], &[0, 7, 9]);
        assert!(!temp_path(&path).exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_write_error_names_the_path_and_leaves_nothing() {
        let dir = scratch("missing");
        let path = dir.join("no-such-dir").join("shot.png");
        let error = write_png(&path, &image(2, 2)).unwrap_err();
        assert!(error.starts_with("cannot write "), "{error}");
        assert!(!path.exists() && !temp_path(&path).exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_failed_rename_removes_the_temp_file() {
        let dir = scratch("rename");
        let path = dir.join("shot.png");
        std::fs::create_dir(&path).unwrap();
        assert!(write_png(&path, &image(2, 2)).is_err());
        assert!(!temp_path(&path).exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_capture_run_opens_no_audio_device() {
        let (_, warning) = audio_output(true, || panic!("the device was opened"));
        assert_eq!(warning, None);
        let (_, warning) = audio_output(false, || {
            (Box::new(SilentOutput::default()), Some("w".into()))
        });
        assert_eq!(warning.as_deref(), Some("w"));
    }

    #[test]
    fn finish_reports_the_outcome_or_the_early_close() {
        let outcome: Outcome = Arc::default();
        assert_eq!(
            finish(Ok(()), &outcome),
            Err("the window closed before the screenshot was taken".to_owned())
        );
        *outcome.lock().unwrap() = Some(Err("boom".to_owned()));
        assert_eq!(finish(Ok(()), &outcome), Err("boom".to_owned()));
        *outcome.lock().unwrap() = Some(Ok(()));
        assert_eq!(finish(Ok(()), &outcome), Ok(()));
        assert_eq!(finish(Err("gpu".into()), &outcome), Err("gpu".to_owned()));
    }
}
