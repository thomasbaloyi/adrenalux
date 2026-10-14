use adrenalux_core::protocol::{ChillConfig, ChillState};
use evdev::{Device, EventType, Key};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Instant;

pub struct ChillEngine {
    config: Arc<RwLock<ChillConfig>>,
    start_instant: Instant,
    last_input_instant_ms: Arc<AtomicI64>,
    is_overlay_open: Arc<AtomicBool>,
}

impl ChillEngine {
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(ChillConfig::default())),
            start_instant: Instant::now(),
            last_input_instant_ms: Arc::new(AtomicI64::new(0)),
            is_overlay_open: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start(&self) {
        let last_input = Arc::clone(&self.last_input_instant_ms);
        let overlay_open = Arc::clone(&self.is_overlay_open);
        let start = self.start_instant;

        std::thread::spawn(move || {
            let mut devices = Vec::new();
            let input_dir = Path::new("/dev/input");

            if input_dir.exists() {
                if let Ok(entries) = fs::read_dir(input_dir) {
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name.starts_with("event") {
                            let path = entry.path();
                            if let Ok(dev) = Device::open(&path) {
                                // Only monitor devices that support keys or mouse relative movement
                                if dev.supported_events().contains(EventType::KEY)
                                    || dev.supported_events().contains(EventType::RELATIVE)
                                {
                                    // Set non-blocking on the underlying file descriptor
                                    unsafe {
                                        use std::os::unix::io::AsRawFd;
                                        let fd = dev.as_raw_fd();
                                        let flags = libc::fcntl(fd, libc::F_GETFL);
                                        libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
                                    }
                                    devices.push(dev);
                                }
                            }
                        }
                    }
                }
            }

            if !devices.is_empty() {
                println!(
                    "[ChillEngine] Successfully opened {} input devices for Open Chill & Alt+A hotkey tracking.",
                    devices.len()
                );
            } else {
                println!(
                    "[ChillEngine] Notice: /dev/input access restricted (user not in 'input' group). Fallback active."
                );
            }

            let mut alt_down = false;

            loop {
                // Poll all device file descriptors with 25ms timeout so we don't spin or block
                let mut poll_fds: Vec<libc::pollfd> = devices
                    .iter()
                    .map(|d| {
                        use std::os::unix::io::AsRawFd;
                        libc::pollfd {
                            fd: d.as_raw_fd(),
                            events: libc::POLLIN,
                            revents: 0,
                        }
                    })
                    .collect();

                let poll_ret = unsafe {
                    libc::poll(poll_fds.as_mut_ptr(), poll_fds.len() as libc::nfds_t, 25)
                };

                let mut activity = false;

                if poll_ret > 0 {
                    for (i, pfd) in poll_fds.iter().enumerate() {
                        if (pfd.revents & libc::POLLIN) != 0 {
                            let dev = &mut devices[i];
                            let event_list: Vec<_> = match dev.fetch_events() {
                                Ok(evs) => evs.collect(),
                                Err(_) => Vec::new(),
                            };

                            for ev in event_list {
                                activity = true;
                                if ev.event_type() == EventType::KEY {
                                    let key = Key::new(ev.code());
                                    let val = ev.value();

                                    if key == Key::KEY_LEFTALT || key == Key::KEY_RIGHTALT {
                                        alt_down = val == 1 || val == 2;
                                    } else if key == Key::KEY_A && val == 1 {
                                        let kernel_alt = dev.get_key_state().is_ok_and(|keys| {
                                            keys.contains(Key::KEY_LEFTALT)
                                                || keys.contains(Key::KEY_RIGHTALT)
                                        });

                                        if alt_down || kernel_alt {
                                            // Alt+A pressed! Toggle overlay!
                                            let new_val = !overlay_open.load(Ordering::Relaxed);
                                            overlay_open.store(new_val, Ordering::Relaxed);
                                            println!(
                                                "[ChillEngine] Global Hotkey Alt+A detected! Overlay is now: {}",
                                                if new_val { "OPEN" } else { "CLOSED" }
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if activity {
                    let now_ms = start.elapsed().as_millis() as i64;
                    last_input.store(now_ms, Ordering::Relaxed);
                }
            }
        });
    }

    pub fn record_input(&self) {
        let now_ms = self.start_instant.elapsed().as_millis() as i64;
        self.last_input_instant_ms.store(now_ms, Ordering::Relaxed);
    }

    pub fn toggle_overlay(&self) -> bool {
        let new_val = !self.is_overlay_open.load(Ordering::Relaxed);
        self.is_overlay_open.store(new_val, Ordering::Relaxed);
        new_val
    }

    pub fn set_overlay_open(&self, open: bool) {
        self.is_overlay_open.store(open, Ordering::Relaxed);
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
            is_overlay_open: self.is_overlay_open.load(Ordering::Relaxed),
        }
    }
}

impl Default for ChillEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

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

        engine.record_input();
        let state1 = engine.get_state();
        assert!(state1.enabled);
        assert!(!state1.is_throttled);
        assert_eq!(state1.current_target_fps, 144);
        assert!(!state1.is_overlay_open);

        std::thread::sleep(Duration::from_millis(150));
        let state2 = engine.get_state();
        assert!(state2.is_throttled);
        assert_eq!(state2.current_target_fps, 30);
        assert!(state2.last_input_delta_ms >= 100);

        engine.record_input();
        let state3 = engine.get_state();
        assert!(!state3.is_throttled);
        assert_eq!(state3.current_target_fps, 144);
    }

    #[test]
    fn test_overlay_toggle() {
        let engine = ChillEngine::new();
        assert!(!engine.get_state().is_overlay_open);

        assert!(engine.toggle_overlay());
        assert!(engine.get_state().is_overlay_open);

        assert!(!engine.toggle_overlay());
        assert!(!engine.get_state().is_overlay_open);
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
