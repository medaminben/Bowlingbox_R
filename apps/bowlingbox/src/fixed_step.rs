//! Turns irregular frame times into whole physics steps, so the game runs at the same speed
//! whatever the frame rate.

/// Seconds per step.
pub const DT: f32 = 1.0 / 60.0;
/// After a stall, run this many steps and drop the rest instead of fast-forwarding.
pub const MAX_STEPS: u32 = 5;

/// Game time that passed since the last frame.
///
/// The window sleeps while the game waits for input, so the first frame after the wait reports
/// the whole wait as `elapsed`. Nothing moved while idle, so that frame counts no time: a charge
/// started on it begins at power 0.
pub fn frame_time(was_idle: bool, elapsed: f32) -> f32 {
    if was_idle { 0.0 } else { elapsed }
}

#[derive(Debug, Default)]
pub struct FixedStep {
    pending: f32,
}

impl FixedStep {
    /// Adds the time since the last frame and returns how many [`DT`] steps to run now.
    pub fn advance(&mut self, elapsed: f32) -> u32 {
        self.pending += elapsed.max(0.0);
        // Truncation is the point (whole steps only); at most `MAX_STEPS` reach the subtraction.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let steps = (self.pending / DT) as u32;
        if steps > MAX_STEPS {
            self.pending = 0.0;
            return MAX_STEPS;
        }
        #[allow(clippy::cast_precision_loss)]
        let consumed = steps as f32 * DT;
        self.pending -= consumed;
        steps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frame_after_an_idle_wait_counts_no_time() {
        let mut clock = FixedStep::default();
        assert_eq!(clock.advance(frame_time(true, 30.0)), 0);
        assert_eq!(frame_time(false, 0.016), 0.016);
    }

    #[test]
    fn half_steps_accumulate() {
        let mut clock = FixedStep::default();
        assert_eq!(clock.advance(DT / 2.0), 0);
        assert_eq!(clock.advance(DT / 2.0), 1);
    }

    #[test]
    fn a_jittery_16ms_timer_still_runs_60_steps_per_second() {
        let mut clock = FixedStep::default();
        let steps: u32 = (0..1000)
            .map(|i| clock.advance(if i % 2 == 0 { 0.012 } else { 0.020 }))
            .sum();
        // 16 s of game time.
        assert!(steps.abs_diff(960) <= 1, "{steps}");
    }

    #[test]
    fn long_stall_runs_at_most_five_steps_and_drops_the_rest() {
        let mut clock = FixedStep::default();
        assert_eq!(clock.advance(2.0), MAX_STEPS);
        assert_eq!(clock.advance(0.0), 0);
        // A clock going backwards is ignored.
        assert_eq!(clock.advance(-1.0), 0);
    }
}
