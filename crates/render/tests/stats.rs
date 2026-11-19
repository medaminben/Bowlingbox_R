//! `BOWLINGBOX_RENDER_STATS`: the averaged per-pass line, the line for adapters without
//! timestamps and the once-a-second rule, all without a GPU.

use bowlingbox_render::{
    PassStats, TimedPass, render_stats_from_env, stats_due, stats_unsupported_line,
};
use std::time::Duration;

#[test]
fn only_one_turns_the_stats_on() {
    assert!(render_stats_from_env(Some("1")));
    for value in [
        None,
        Some("0"),
        Some(""),
        Some("yes"),
        Some("true"),
        Some(" 1"),
    ] {
        assert!(!render_stats_from_env(value), "{value:?}");
    }
}

#[test]
fn the_line_averages_each_pass_over_its_samples_as_cpp_prints_it() {
    let mut stats = PassStats::default();
    stats.add(TimedPass::Shadow, 0.5);
    stats.add(TimedPass::Shadow, 0.25);
    stats.add(TimedPass::Reflection, 1.0);
    stats.add(TimedPass::Main, 2.0);
    stats.add(TimedPass::Main, 4.0);
    stats.add(TimedPass::Main, 3.0);
    stats.add(TimedPass::Post, 0.126);
    for _ in 0..60 {
        stats.end_frame();
    }
    assert_eq!(
        stats.line(),
        "render: shadow 0.38 ms, reflection 1.00 ms, main 3.00 ms, post 0.13 ms (GPU, 60 frames)"
    );
}

#[test]
fn a_pass_without_samples_prints_zero() {
    let mut stats = PassStats::default();
    stats.add(TimedPass::Main, 1.5);
    stats.end_frame();
    assert_eq!(
        stats.line(),
        "render: shadow 0.00 ms, reflection 0.00 ms, main 1.50 ms, post 0.00 ms (GPU, 1 frames)"
    );
}

#[test]
fn reset_starts_the_next_second_from_nothing() {
    let mut stats = PassStats::default();
    stats.add(TimedPass::Post, 9.0);
    stats.end_frame();
    stats.reset();
    assert_eq!(
        stats.line(),
        "render: shadow 0.00 ms, reflection 0.00 ms, main 0.00 ms, post 0.00 ms (GPU, 0 frames)"
    );
}

#[test]
fn the_line_is_due_once_a_second_has_passed() {
    assert!(!stats_due(Duration::ZERO));
    assert!(!stats_due(Duration::from_millis(999)));
    assert!(stats_due(Duration::from_secs(1)));
    assert!(stats_due(Duration::from_millis(1700)));
}

#[test]
fn the_unsupported_line_is_the_planned_text() {
    assert_eq!(
        stats_unsupported_line(),
        "render stats: timestamp queries not supported by this adapter"
    );
}
