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
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ReplayResult {
    pub success: bool,
    pub saved_path: String,
    pub message: String,
}
