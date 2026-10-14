#pragma once

#include <cstdint>
#include <string>
#include <vector>

namespace adrenalux::ipc {

constexpr const char* DEFAULT_SOCKET_NAME = "adrenalux.sock";
constexpr uint32_t PROTOCOL_MAGIC = 0x41445258; // 'ADRX'
constexpr uint32_t PROTOCOL_VERSION = 1;

enum class MessageType : uint32_t {
    Ping = 1,
    Pong = 2,
    
    // Telemetry
    GetTelemetry = 10,
    TelemetryData = 11,

    // Open Chill & Boost
    GetChillState = 20,
    SetChillConfig = 21,
    ChillState = 22,

    // Instant Replay DVR
    TriggerReplay = 30,
    ReplayResult = 31,

    // Profiles & OptiScaler
    GetGameProfile = 40,
    SetGameProfile = 41,
    GameProfileData = 42,

    // Error
    ErrorResponse = 99
};

#pragma pack(push, 1)

struct MessageHeader {
    uint32_t magic{PROTOCOL_MAGIC};
    uint32_t version{PROTOCOL_VERSION};
    MessageType type;
    uint32_t payload_size{0};
};

struct TelemetryPayload {
    // Temperatures in milli-Celsius (e.g., 55000 = 55.0 C)
    int32_t gpu_temp_edge_mc{0};
    int32_t gpu_temp_junction_mc{0};
    int32_t gpu_temp_mem_mc{0};

    // Clocks in MHz
    uint32_t core_clock_mhz{0};
    uint32_t mem_clock_mhz{0};

    // Power in milliwatts
    uint32_t power_draw_mw{0};
    uint32_t power_cap_mw{0};

    // Fan in RPM
    uint32_t fan_rpm{0};
    uint32_t fan_pwm{0};

    // VRAM usage in Megabytes
    uint32_t vram_used_mb{0};
    uint32_t vram_total_mb{0};
};

struct ChillConfigPayload {
    bool enabled{true};
    uint32_t idle_fps{30};
    uint32_t active_fps{0}; // 0 = uncapped
    uint32_t idle_timeout_ms{1500};
    bool boost_enabled{false};
};

struct ChillStatePayload {
    bool enabled{true};
    bool is_throttled{false};
    uint32_t current_target_fps{0};
    uint64_t last_input_delta_ms{0};
};

struct ReplayTriggerPayload {
    // 0 = default buffer length (e.g. 60s)
    uint32_t buffer_seconds{0};
};

struct ReplayResultPayload {
    bool success{false};
    char saved_path[256]{0};
    char message[128]{0};
};

#pragma pack(pop)

} // namespace adrenalux::ipc
