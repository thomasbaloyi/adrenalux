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

    #[cfg(test)]
    pub fn from_paths(card_path: PathBuf, hwmon_path: PathBuf) -> Self {
        Self {
            card_path: Some(card_path),
            hwmon_path: Some(hwmon_path),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_parsing_with_mock_sysfs() {
        let temp_dir = std::env::temp_dir().join(format!("adrenalux_test_hwmon_{}", std::process::id()));
        let card_dir = temp_dir.join("card");
        let hwmon_dir = card_dir.join("hwmon").join("hwmon0");

        fs::create_dir_all(&hwmon_dir).expect("Failed to create mock hwmon dir");

        // Write mock files
        fs::write(hwmon_dir.join("temp1_input"), "45000\n").unwrap();
        fs::write(hwmon_dir.join("temp2_input"), "52000\n").unwrap();
        fs::write(hwmon_dir.join("temp3_input"), "55000\n").unwrap();
        fs::write(hwmon_dir.join("freq1_input"), "2400000000\n").unwrap();
        fs::write(hwmon_dir.join("freq2_input"), "1250000000\n").unwrap();
        fs::write(hwmon_dir.join("power1_average"), "180000000\n").unwrap();
        fs::write(hwmon_dir.join("power1_cap"), "304000000\n").unwrap();
        fs::write(hwmon_dir.join("fan1_input"), "1450\n").unwrap();
        fs::write(hwmon_dir.join("pwm1"), "95\n").unwrap();

        fs::write(card_dir.join("mem_info_vram_used"), "2147483648\n").unwrap(); // 2048 MB
        fs::write(card_dir.join("mem_info_vram_total"), "17179869184\n").unwrap(); // 16384 MB

        let mon = HardwareMonitor::from_paths(card_dir, hwmon_dir);
        let tel = mon.read_telemetry();

        assert_eq!(tel.gpu_temp_edge_c, 45.0);
        assert_eq!(tel.gpu_temp_junction_c, 52.0);
        assert_eq!(tel.gpu_temp_mem_c, 55.0);
        assert_eq!(tel.core_clock_mhz, 2400);
        assert_eq!(tel.mem_clock_mhz, 1250);
        assert_eq!(tel.power_draw_w, 180.0);
        assert_eq!(tel.power_cap_w, 304.0);
        assert_eq!(tel.fan_rpm, 1450);
        assert_eq!(tel.fan_pwm, 95);
        assert_eq!(tel.vram_used_mb, 2048);
        assert_eq!(tel.vram_total_mb, 16384);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_missing_files_graceful_fallback() {
        let temp_dir = std::env::temp_dir().join(format!("adrenalux_empty_hwmon_{}", std::process::id()));
        let card_dir = temp_dir.join("card");
        let hwmon_dir = card_dir.join("hwmon");
        fs::create_dir_all(&hwmon_dir).unwrap();

        let mon = HardwareMonitor::from_paths(card_dir, hwmon_dir);
        let tel = mon.read_telemetry();

        assert_eq!(tel.gpu_temp_edge_c, 0.0);
        assert_eq!(tel.power_draw_w, 0.0);
        assert_eq!(tel.core_clock_mhz, 0);

        let _ = fs::remove_dir_all(temp_dir);
    }
}
