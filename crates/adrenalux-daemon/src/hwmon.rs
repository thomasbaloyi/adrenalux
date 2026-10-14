use adrenalux_core::protocol::TelemetryData;
use std::fs;
use std::path::{Path, PathBuf};

pub struct HardwareMonitor {
    card_path: Option<PathBuf>,
    hwmon_path: Option<PathBuf>,
}

impl HardwareMonitor {
    pub fn new() -> Self {
        let mut mon = Self {
            card_path: None,
            hwmon_path: None,
        };
        mon.find_amd_gpu();
        mon
    }

    fn find_amd_gpu(&mut self) {
        let drm_base = Path::new("/sys/class/drm");
        if !drm_base.exists() {
            return;
        }

        if let Ok(entries) = fs::read_dir(drm_base) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with("card") || name.contains('-') {
                    continue;
                }

                let device_dir = entry.path().join("device");
                let vendor_file = device_dir.join("vendor");

                if let Ok(vendor) = fs::read_to_string(&vendor_file) {
                    if vendor.trim() == "0x1002" {
                        self.card_path = Some(device_dir.clone());
                        let hwmon_base = device_dir.join("hwmon");
                        if let Ok(hw_entries) = fs::read_dir(hwmon_base) {
                            for hw in hw_entries.flatten() {
                                if hw.file_name().to_string_lossy().starts_with("hwmon") {
                                    self.hwmon_path = Some(hw.path());
                                    println!(
                                        "[HardwareMonitor] Discovered AMD GPU at {:?} (hwmon: {:?})",
                                        self.card_path.as_ref().unwrap(),
                                        self.hwmon_path.as_ref().unwrap()
                                    );
                                    return;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn read_sysfs_i64(&self, path: &Path) -> i64 {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| s.trim().parse::<i64>().ok())
            .unwrap_or(0)
    }

    pub fn read_telemetry(&self) -> TelemetryData {
        let mut data = TelemetryData::default();

        if let Some(hwmon) = &self.hwmon_path {
            // Temps in milli-Celsius -> convert to Celsius
            data.gpu_temp_edge_c = (self.read_sysfs_i64(&hwmon.join("temp1_input")) as f32) / 1000.0;
            data.gpu_temp_junction_c = (self.read_sysfs_i64(&hwmon.join("temp2_input")) as f32) / 1000.0;
            data.gpu_temp_mem_c = (self.read_sysfs_i64(&hwmon.join("temp3_input")) as f32) / 1000.0;

            // Clocks in Hz -> convert to MHz
            data.core_clock_mhz = (self.read_sysfs_i64(&hwmon.join("freq1_input")) / 1_000_000) as u32;
            data.mem_clock_mhz = (self.read_sysfs_i64(&hwmon.join("freq2_input")) / 1_000_000) as u32;

            // Power in micro-watts -> convert to Watts
            data.power_draw_w = (self.read_sysfs_i64(&hwmon.join("power1_average")) as f32) / 1_000_000.0;
            data.power_cap_w = (self.read_sysfs_i64(&hwmon.join("power1_cap")) as f32) / 1_000_000.0;

            // Fan
            data.fan_rpm = self.read_sysfs_i64(&hwmon.join("fan1_input")) as u32;
            data.fan_pwm = self.read_sysfs_i64(&hwmon.join("pwm1")) as u32;
        }

        if let Some(card) = &self.card_path {
            let used_bytes = self.read_sysfs_i64(&card.join("mem_info_vram_used"));
            let total_bytes = self.read_sysfs_i64(&card.join("mem_info_vram_total"));
            data.vram_used_mb = (used_bytes / (1024 * 1024)) as u32;
            data.vram_total_mb = (total_bytes / (1024 * 1024)) as u32;
        }

        data
    }
}
