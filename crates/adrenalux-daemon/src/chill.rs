use adrenalux_core::protocol::{ChillConfig, ChillState};
use evdev::Device;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

pub struct ChillEngine {
    config: Arc<RwLock<ChillConfig>>,
    start_instant: Instant,
    last_input_instant_ms: Arc<AtomicI64>,
}

impl ChillEngine {
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(ChillConfig::default())),
            start_instant: Instant::now(),
            last_input_instant_ms: Arc::new(AtomicI64::new(0)),
        }
    }

    pub fn start(&self) {
        let last_input = Arc::clone(&self.last_input_instant_ms);
        let start = self.start_instant;

        std::thread::spawn(move || {
            let mut devices = Vec::new();
            let input_dir = Path::new("/dev/input");

            if input_dir.exists() {
                if let Ok(entries) = fs::read_dir(input_dir) {
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name.starts_with("event") {
                            if let Ok(dev) = Device::open(entry.path()) {
                                devices.push(dev);
                            }
                        }
                    }
                }
            }

            if !devices.is_empty() {
                println!(
                    "[ChillEngine] Successfully opened {} input devices for Open Chill tracking.",
                    devices.len()
                );
            } else {
                println!(
                    "[ChillEngine] Notice: /dev/input access restricted (user not in 'input' group). Fallback active."
                );
            }

            loop {
                let mut activity = false;
                for dev in &mut devices {
                    if let Ok(events) = dev.fetch_events() {
                        for _ in events {
                            activity = true;
                        }
                    }
                }

                if activity {
                    let now_ms = start.elapsed().as_millis() as i64;
                    last_input.store(now_ms, Ordering::Relaxed);
                }

                std::thread::sleep(Duration::from_millis(50));
            }
        });
    }

    pub fn record_input(&self) {
        let now_ms = self.start_instant.elapsed().as_millis() as i64;
        self.last_input_instant_ms.store(now_ms, Ordering::Relaxed);
    }

    pub fn update_config(&self, new_config: ChillConfig) {
        let mut cfg = self.config.write().unwrap();
        *cfg = new_config;
    }

    pub fn get_state(&self) -> ChillState {
        let cfg = self.config.read().unwrap().clone();
        let now_ms = self.start_instant.elapsed().as_millis() as i64;
        let last_ms = self.last_input_instant_ms.load(Ordering::Relaxed);
        let delta_ms = if now_ms >= last_ms {
            (now_ms - last_ms) as u64
        } else {
            0
        };

        let is_throttled = cfg.enabled && delta_ms > cfg.idle_timeout_ms;
        let current_target_fps = if is_throttled {
            cfg.idle_fps
        } else {
            cfg.active_fps
        };

        ChillState {
            enabled: cfg.enabled,
            is_throttled,
            current_target_fps,
            last_input_delta_ms: delta_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chill_state_transitions() {
        let engine = ChillEngine::new();
        engine.update_config(ChillConfig {
            enabled: true,
            idle_fps: 30,
            active_fps: 144,
            idle_timeout_ms: 100, // 100ms timeout for test speed
            boost_enabled: false,
        });

        // Record input now -> should be active
        engine.record_input();
        let state1 = engine.get_state();
        assert!(state1.enabled);
        assert!(!state1.is_throttled);
        assert_eq!(state1.current_target_fps, 144);

        // Sleep 150ms -> should transition to throttled
        std::thread::sleep(Duration::from_millis(150));
        let state2 = engine.get_state();
        assert!(state2.is_throttled);
        assert_eq!(state2.current_target_fps, 30);
        assert!(state2.last_input_delta_ms >= 100);

        // Record input again -> should immediately wake up
        engine.record_input();
        let state3 = engine.get_state();
        assert!(!state3.is_throttled);
        assert_eq!(state3.current_target_fps, 144);
    }

    #[test]
    fn test_chill_disabled_state() {
        let engine = ChillEngine::new();
        engine.update_config(ChillConfig {
            enabled: false,
            idle_fps: 30,
            active_fps: 144,
            idle_timeout_ms: 50,
            boost_enabled: false,
        });

        std::thread::sleep(Duration::from_millis(100));
        let state = engine.get_state();
        assert!(!state.enabled);
        assert!(!state.is_throttled);
        assert_eq!(state.current_target_fps, 144);
    }
}
