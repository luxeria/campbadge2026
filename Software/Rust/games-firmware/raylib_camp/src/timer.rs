//! Frame pacing and timing helpers driven by an external millisecond clock.

/// Tracks frame duration and instantaneous frames-per-second.
///
/// The hardware provides a monotonic millisecond clock; each render frame the
/// firmware calls [`FrameTimer::tick`] with the current time and reads back the
/// elapsed seconds and the smoothed frame rate.
pub struct FrameTimer {
    last_tick_ms: Option<u32>,
    delta_seconds: f32,
    smoothed_fps: f32,
}

impl FrameTimer {
    /// Creates a timer with no recorded history.
    pub fn new() -> Self {
        FrameTimer {
            last_tick_ms: None,
            delta_seconds: 0.0,
            smoothed_fps: 0.0,
        }
    }

    /// Records a frame boundary at `now_ms` and returns the elapsed seconds
    /// since the previous `tick`.
    pub fn tick(&mut self, now_ms: u32) -> f32 {
        if let Some(previous_ms) = self.last_tick_ms {
            let elapsed_ms = now_ms.saturating_sub(previous_ms);
            self.delta_seconds = elapsed_ms as f32 / 1000.0;
            if self.delta_seconds > 0.0 {
                let instant_fps = 1.0 / self.delta_seconds;
                self.smoothed_fps = if self.smoothed_fps == 0.0 {
                    instant_fps
                } else {
                    0.9 * self.smoothed_fps + 0.1 * instant_fps
                };
            }
        }
        self.last_tick_ms = Some(now_ms);
        self.delta_seconds
    }

    /// Returns the seconds elapsed in the most recent tick.
    pub fn delta_seconds(&self) -> f32 {
        self.delta_seconds
    }

    /// Returns the smoothed frame rate in frames per second.
    pub fn fps(&self) -> f32 {
        self.smoothed_fps
    }
}

impl Default for FrameTimer {
    fn default() -> Self {
        FrameTimer::new()
    }
}

#[cfg(test)]
mod tests {
    use super::FrameTimer;

    #[test]
    fn delta_reflects_millis_between_ticks() {
        let mut timer = FrameTimer::new();
        timer.tick(0);
        let delta = timer.tick(16);
        assert!((delta - 0.016).abs() < 0.001);
    }

    #[test]
    fn fps_tracks_a_thirty_frame_per_second_tempo() {
        let mut timer = FrameTimer::new();
        for frame in 0..30 {
            timer.tick(frame * 33);
        }
        assert!((timer.fps() - 30.0).abs() < 5.0);
    }
}
