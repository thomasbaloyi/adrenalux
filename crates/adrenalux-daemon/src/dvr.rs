use adrenalux_core::protocol::ReplayResult;
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use std::fs;
use std::path::{Path, PathBuf};

pub struct DvrController {
    replay_dir: PathBuf,
}

impl DvrController {
    pub fn new() -> Self {
        let replay_dir = if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join("Videos").join("Replays")
        } else {
            PathBuf::from("/tmp/Replays")
        };

        let _ = fs::create_dir_all(&replay_dir);

        Self { replay_dir }
    }

    #[cfg(test)]
    pub fn with_dir(replay_dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(&replay_dir);
        Self { replay_dir }
    }

    fn find_gsr_pid(&self) -> Option<i32> {
        let proc = Path::new("/proc");
        if let Ok(entries) = fs::read_dir(proc) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if let Ok(pid) = name.parse::<i32>() {
                    let comm_file = entry.path().join("comm");
                    if let Ok(comm) = fs::read_to_string(comm_file) {
                        let comm_trimmed = comm.trim();
                        if comm_trimmed == "gpu-screen-reco"
                            || comm_trimmed == "gpu-screen-recorder"
                        {
                            return Some(pid);
                        }
                    }
                }
            }
        }
        None
    }

    pub fn trigger_replay(&self, _buffer_seconds: u32) -> ReplayResult {
        if let Some(pid) = self.find_gsr_pid() {
            if kill(Pid::from_raw(pid), Signal::SIGUSR1).is_ok() {
                ReplayResult {
                    success: true,
                    saved_path: self.replay_dir.to_string_lossy().to_string(),
                    message: format!("Sent SIGUSR1 to gpu-screen-recorder (PID {})", pid),
                }
            } else {
                ReplayResult {
                    success: false,
                    saved_path: self.replay_dir.to_string_lossy().to_string(),
                    message: "Failed to signal gpu-screen-recorder process".into(),
                }
            }
        } else {
            ReplayResult {
                success: false,
                saved_path: self.replay_dir.to_string_lossy().to_string(),
                message: "gpu-screen-recorder replay buffer is not currently active".into(),
            }
        }
    }
}

impl Default for DvrController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dvr_directory_creation() {
        let temp_dir =
            std::env::temp_dir().join(format!("adrenalux_test_dvr_{}", std::process::id()));
        let ctrl = DvrController::with_dir(temp_dir.clone());
        assert!(temp_dir.exists());

        // When GSR is not running, trigger_replay must return a graceful error rather than panicking
        let res = ctrl.trigger_replay(60);
        assert!(!res.success);
        assert!(res.message.contains("gpu-screen-recorder"));
        assert_eq!(res.saved_path, temp_dir.to_string_lossy().to_string());

        let _ = fs::remove_dir_all(temp_dir);
    }
}
