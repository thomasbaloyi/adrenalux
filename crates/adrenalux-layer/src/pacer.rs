use std::time::Instant;

fn get_monotonic_ns() -> i64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe {
        libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts);
    }
    (ts.tv_sec) * 1_000_000_000 + (ts.tv_nsec)
}

pub struct FramePacer {
    next_frame_target_ns: i64,
    current_fps: f32,
    frames_in_period: u32,
    fps_period_start: Instant,
}

impl FramePacer {
    pub fn new() -> Self {
        Self {
            next_frame_target_ns: 0,
            current_fps: 0.0,
            frames_in_period: 0,
            fps_period_start: Instant::now(),
        }
    }

    pub fn pace_frame(&mut self, target_fps: u32) {
        let now_ns = get_monotonic_ns();

        if target_fps > 0 {
            let target_interval_ns = 1_000_000_000i64 / (target_fps as i64);

            if self.next_frame_target_ns == 0
                || now_ns > self.next_frame_target_ns + target_interval_ns * 2
            {
                self.next_frame_target_ns = now_ns + target_interval_ns;
            } else {
                self.next_frame_target_ns += target_interval_ns;
            }

            if self.next_frame_target_ns > now_ns {
                let req = libc::timespec {
                    tv_sec: (self.next_frame_target_ns / 1_000_000_000) as libc::time_t,
                    tv_nsec: (self.next_frame_target_ns % 1_000_000_000) as libc::c_long,
                };
                unsafe {
                    while libc::clock_nanosleep(
                        libc::CLOCK_MONOTONIC,
                        libc::TIMER_ABSTIME,
                        &req,
                        std::ptr::null_mut(),
                    ) == libc::EINTR
                    {}
                }
            }
        } else {
            self.next_frame_target_ns = 0;
        }

        self.frames_in_period += 1;
        let now = Instant::now();
        let period_elapsed = now.duration_since(self.fps_period_start);
        if period_elapsed.as_millis() >= 500 {
            self.current_fps =
                (self.frames_in_period as f32 * 1000.0) / (period_elapsed.as_millis() as f32);
            self.frames_in_period = 0;
            self.fps_period_start = now;
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
        let target_fps = 60;

        let start = Instant::now();
        for _ in 0..5 {
            pacer.pace_frame(target_fps);
        }
        let elapsed = start.elapsed();

        // 5 frames at 60 FPS (~16.6ms each) should take at least ~60-85ms
        assert!(elapsed >= Duration::from_millis(50));
    }
}
