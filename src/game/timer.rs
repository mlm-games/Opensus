#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TimerMode {
    Once,
    Repeating,
}

/// Fixed-step replacement for bevy's `Timer` (no bevy_time in this stack).
/// `tick` must be driven by `SimTime.delta_secs` (the fixed step).
#[derive(Clone, Copy, Debug)]
pub struct SimTimer {
    duration: f32,
    elapsed: f32,
    mode: TimerMode,
    just_finished: bool,
    finished_once: bool,
}

impl Default for SimTimer {
    fn default() -> Self {
        Self {
            duration: 0.0,
            elapsed: 0.0,
            mode: TimerMode::Once,
            just_finished: false,
            finished_once: false,
        }
    }
}

impl SimTimer {
    pub fn from_seconds(duration: f32, mode: TimerMode) -> Self {
        Self {
            duration,
            elapsed: 0.0,
            mode,
            just_finished: false,
            finished_once: false,
        }
    }

    pub fn tick(&mut self, delta_secs: f32) {
        self.just_finished = false;
        let delta = if delta_secs.is_finite() {
            delta_secs.max(0.0)
        } else {
            0.0
        };
        match self.mode {
            TimerMode::Once => {
                if self.finished_once {
                    self.elapsed = self.duration;
                    return;
                }
                self.elapsed += delta;
                if self.elapsed >= self.duration {
                    self.elapsed = self.duration;
                    self.just_finished = true;
                    self.finished_once = true;
                }
            }
            TimerMode::Repeating => {
                if self.duration <= 0.0 {
                    return;
                }
                self.elapsed += delta;
                while self.elapsed >= self.duration {
                    self.elapsed -= self.duration;
                    self.just_finished = true;
                }
            }
        }
    }

    #[inline]
    pub fn just_finished(&self) -> bool {
        self.just_finished
    }

    #[inline]
    pub fn is_finished(&self) -> bool {
        self.elapsed >= self.duration
    }

    #[inline]
    pub fn remaining_secs(&self) -> f32 {
        (self.duration - self.elapsed).max(0.0)
    }

    #[inline]
    pub fn elapsed_secs(&self) -> f32 {
        self.elapsed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn once_timer_fires_exactly_once() {
        let mut timer = SimTimer::from_seconds(1.0, TimerMode::Once);
        timer.tick(0.6);
        assert!(!timer.just_finished());
        timer.tick(0.6);
        assert!(timer.just_finished());
        timer.tick(0.6);
        assert!(!timer.just_finished());
        assert!(timer.is_finished());
    }

    #[test]
    fn repeating_timer_wraps_and_refires() {
        let mut timer = SimTimer::from_seconds(0.4, TimerMode::Repeating);
        timer.tick(0.45);
        assert!(timer.just_finished());
        assert!(!timer.is_finished());
        timer.tick(0.4);
        assert!(timer.just_finished());
    }
}
