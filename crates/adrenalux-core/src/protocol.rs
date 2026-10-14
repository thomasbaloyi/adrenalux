use serde::{Deserialize, Serialize};

pub const PROTOCOL_MAGIC: u32 = 0x41445258; // 'ADRX'
pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Request {
    Ping,
    GetTelemetry,
    GetChillState,
    SetChillConfig(ChillConfig),
    TriggerReplay { buffer_seconds: u32 },
    ToggleOverlay,
    SetOverlayOpen(bool),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Response {
    Pong,
    Telemetry(TelemetryData),
    Chill(ChillState),
    Replay(ReplayResult),
    Error(String),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TelemetryData {
    pub gpu_temp_edge_c: f32,
    pub gpu_temp_junction_c: f32,
    pub gpu_temp_mem_c: f32,
    pub core_clock_mhz: u32,
    pub mem_clock_mhz: u32,
    pub power_draw_w: f32,
    pub power_cap_w: f32,
    pub fan_rpm: u32,
    pub fan_pwm: u32,
    pub vram_used_mb: u32,
    pub vram_total_mb: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChillConfig {
    pub enabled: bool,
    pub idle_fps: u32,
    pub active_fps: u32, // 0 = uncapped
    pub idle_timeout_ms: u64,
    pub boost_enabled: bool,
}

impl Default for ChillConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            idle_fps: 30,
            active_fps: 0,
            idle_timeout_ms: 1500,
            boost_enabled: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ChillState {
    pub enabled: bool,
    pub is_throttled: bool,
    pub current_target_fps: u32,
    pub last_input_delta_ms: u64,
    pub is_overlay_open: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ReplayResult {
    pub success: bool,
    pub saved_path: String,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_serialization_roundtrip() {
        let requests = vec![
            Request::Ping,
            Request::GetTelemetry,
            Request::GetChillState,
            Request::SetChillConfig(ChillConfig {
                enabled: true,
                idle_fps: 45,
                active_fps: 144,
                idle_timeout_ms: 2000,
                boost_enabled: true,
            }),
            Request::TriggerReplay { buffer_seconds: 60 },
        ];

        for req in requests {
            let encoded = bincode::serialize(&req).expect("Failed to serialize request");
            let decoded: Request =
                bincode::deserialize(&encoded).expect("Failed to deserialize request");
            assert_eq!(req, decoded);
        }
    }

    #[test]
    fn test_response_serialization_roundtrip() {
        let responses = vec![
            Response::Pong,
            Response::Telemetry(TelemetryData {
                gpu_temp_edge_c: 42.5,
                gpu_temp_junction_c: 48.0,
                gpu_temp_mem_c: 50.0,
                core_clock_mhz: 2400,
                mem_clock_mhz: 1250,
                power_draw_w: 185.0,
                power_cap_w: 304.0,
                fan_rpm: 1200,
                fan_pwm: 80,
                vram_used_mb: 4096,
                vram_total_mb: 16304,
            }),
            Response::Chill(ChillState {
                enabled: true,
                is_throttled: true,
                current_target_fps: 30,
                last_input_delta_ms: 3500,
                is_overlay_open: true,
            }),
            Response::Replay(ReplayResult {
                success: true,
                saved_path: "/home/user/Videos/Replays/clip.mp4".into(),
                message: "Saved successfully".into(),
            }),
            Response::Error("Device error".into()),
        ];

        for resp in responses {
            let encoded = bincode::serialize(&resp).expect("Failed to serialize response");
            let decoded: Response =
                bincode::deserialize(&encoded).expect("Failed to deserialize response");
            assert_eq!(resp, decoded);
        }
    }

    #[test]
    fn test_chill_config_defaults() {
        let cfg = ChillConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.idle_fps, 30);
        assert_eq!(cfg.active_fps, 0);
        assert_eq!(cfg.idle_timeout_ms, 1500);
        assert!(!cfg.boost_enabled);
    }
}
