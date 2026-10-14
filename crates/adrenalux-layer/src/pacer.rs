use std::time::Instant;

pub struct FramePacer {
    last_present: Instant,
    current_fps: f32,
    frames_in_period: u32,
    fps_period_start: Instant,
}

impl FramePacer {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            last_present: now,
            current_fps: 0.0,
            frames_in_period: 0,
            fps_period_start: now,
        }
    }

    pub fn pace_frame(&mut self, target_fps: u32) {
        let now = Instant::now();
        let elapsed_ns = now.duration_since(self.last_present).as_nanos() as i64;

        if target_fps > 0 {
            let target_interval_ns = 1_000_000_000i64 / (target_fps as i64);
            let sleep_ns = target_interval_ns - elapsed_ns;

            if sleep_ns > 50_000 {
                let req = libc::timespec {
                    tv_sec: (sleep_ns / 1_000_000_000) as libc::time_t,
                    tv_nsec: (sleep_ns % 1_000_000_000) as libc::c_long,
                };
                unsafe {
                    libc::clock_nanosleep(libc::CLOCK_MONOTONIC, 0, &req, std::ptr::null_mut());
                }
            }
        }

        self.last_present = Instant::now();
        self.frames_in_period += 1;

        let period_elapsed = self.last_present.duration_since(self.fps_period_start);
        if period_elapsed.as_millis() >= 500 {
            self.current_fps = (self.frames_in_period as f32 * 1000.0) / (period_elapsed.as_millis() as f32);
            self.frames_in_period = 0;
            self.fps_period_start = self.last_present;
        }
    }

    #[allow(dead_code)]
    pub fn get_fps(&self) -> f32 {
        self.current_fps
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_pacer_initialization() {
        let pacer = FramePacer::new();
        assert_eq!(pacer.get_fps(), 0.0);
    }

    #[test]
    fn test_pacer_pacing_rate() {
        let mut pacer = FramePacer::new();
        let target_fps = 60; // ~16.6ms per frame

        let start = Instant::now();
        // Pace 3 frames
        for _ in 0..3 {
            pacer.pace_frame(target_fps);
        }
        let elapsed = start.elapsed();

        // 3 frames at 60 FPS should take at least ~30-50ms
        assert!(elapsed >= Duration::from_millis(25));
    }
}
