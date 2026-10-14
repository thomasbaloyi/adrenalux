#include "adrenalux/ipc_protocol.hpp"
#include "adrenalux/version.hpp"
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>
#include <iostream>
#include <iomanip>
#include <cstring>
#include <filesystem>
#include <thread>
#include <chrono>

using namespace adrenalux;

namespace adrenalux::cli {

class Client {
public:
    Client() {
        const char* runtime_dir = std::getenv("XDG_RUNTIME_DIR");
        if (runtime_dir && std::filesystem::exists(runtime_dir)) {
            socket_path_ = std::string(runtime_dir) + "/" + ipc::DEFAULT_SOCKET_NAME;
        } else {
            socket_path_ = "/tmp/adrenalux-" + std::to_string(getuid()) + ".sock";
        }
    }

    ~Client() {
        disconnect();
    }

    bool connect() {
        fd_ = socket(AF_UNIX, SOCK_STREAM, 0);
        if (fd_ < 0) return false;

        struct sockaddr_un addr{};
        addr.sun_family = AF_UNIX;
        std::strncpy(addr.sun_path, socket_path_.c_str(), sizeof(addr.sun_path) - 1);

        if (::connect(fd_, reinterpret_cast<struct sockaddr*>(&addr), sizeof(addr)) < 0) {
            close(fd_);
            fd_ = -1;
            return false;
        }
        return true;
    }

    void disconnect() {
        if (fd_ >= 0) {
            close(fd_);
            fd_ = -1;
        }
    }

    bool ping() {
        ipc::MessageHeader req{};
        req.type = ipc::MessageType::Ping;
        if (write(fd_, &req, sizeof(req)) != sizeof(req)) return false;

        ipc::MessageHeader resp{};
        if (read(fd_, &resp, sizeof(resp)) != sizeof(resp)) return false;
        return resp.type == ipc::MessageType::Pong;
    }

    bool get_telemetry(ipc::TelemetryPayload& out) {
        ipc::MessageHeader req{};
        req.type = ipc::MessageType::GetTelemetry;
        if (write(fd_, &req, sizeof(req)) != sizeof(req)) return false;

        ipc::MessageHeader resp{};
        if (read(fd_, &resp, sizeof(resp)) != sizeof(resp)) return false;
        if (resp.type != ipc::MessageType::TelemetryData) return false;

        return read(fd_, &out, sizeof(out)) == sizeof(out);
    }

    bool get_chill_state(ipc::ChillStatePayload& out) {
        ipc::MessageHeader req{};
        req.type = ipc::MessageType::GetChillState;
        if (write(fd_, &req, sizeof(req)) != sizeof(req)) return false;

        ipc::MessageHeader resp{};
        if (read(fd_, &resp, sizeof(resp)) != sizeof(resp)) return false;
        if (resp.type != ipc::MessageType::ChillState) return false;

        return read(fd_, &out, sizeof(out)) == sizeof(out);
    }

    bool set_chill_config(const ipc::ChillConfigPayload& cfg, ipc::ChillStatePayload& out) {
        ipc::MessageHeader req{};
        req.type = ipc::MessageType::SetChillConfig;
        req.payload_size = sizeof(cfg);
        if (write(fd_, &req, sizeof(req)) != sizeof(req)) return false;
        if (write(fd_, &cfg, sizeof(cfg)) != sizeof(cfg)) return false;

        ipc::MessageHeader resp{};
        if (read(fd_, &resp, sizeof(resp)) != sizeof(resp)) return false;
        if (resp.type != ipc::MessageType::ChillState) return false;

        return read(fd_, &out, sizeof(out)) == sizeof(out);
    }

    bool trigger_replay(uint32_t buffer_seconds, ipc::ReplayResultPayload& out) {
        ipc::MessageHeader req{};
        req.type = ipc::MessageType::TriggerReplay;
        req.payload_size = sizeof(ipc::ReplayTriggerPayload);

        ipc::ReplayTriggerPayload payload{buffer_seconds};
        if (write(fd_, &req, sizeof(req)) != sizeof(req)) return false;
        if (write(fd_, &payload, sizeof(payload)) != sizeof(payload)) return false;

        ipc::MessageHeader resp{};
        if (read(fd_, &resp, sizeof(resp)) != sizeof(resp)) return false;
        if (resp.type != ipc::MessageType::ReplayResult) return false;

        return read(fd_, &out, sizeof(out)) == sizeof(out);
    }

    std::string socket_path() const { return socket_path_; }

private:
    std::string socket_path_;
    int fd_{-1};
};

} // namespace adrenalux::cli

void print_help() {
    std::cout << "AdrenaLux Control (adrenalux-ctl) v" << adrenalux::VERSION_STRING << "\n\n"
              << "Usage: adrenalux-ctl <command> [options]\n\n"
              << "Commands:\n"
              << "  status              Check daemon status, GPU telemetry, and Chill state\n"
              << "  telemetry           Show real-time AMD GPU metrics\n"
              << "  chill [options]     Configure or inspect Open Chill dynamic frame pacing\n"
              << "      --enable          Enable Open Chill\n"
              << "      --disable         Disable Open Chill\n"
              << "      --idle-fps <N>    Set target FPS when idle (e.g. 30)\n"
              << "      --active-fps <N>  Set active FPS (0 = uncapped)\n"
              << "      --timeout-ms <N>  Idle timeout in ms before throttling (e.g. 1500)\n"
              << "  replay              Trigger an instant replay DVR clip save\n"
              << "  monitor             Launch a live updating console dashboard\n"
              << std::endl;
}

int main(int argc, char** argv) {
    if (argc < 2) {
        print_help();
        return 1;
    }

    std::string cmd = argv[1];
    if (cmd == "-h" || cmd == "--help" || cmd == "help") {
        print_help();
        return 0;
    }

    adrenalux::cli::Client client;
    if (!client.connect()) {
        std::cerr << "Error: Could not connect to adrenaluxd at " << client.socket_path() << "\n"
                  << "Make sure the daemon is running: adrenaluxd\n";
        return 1;
    }

    if (cmd == "status") {
        ipc::TelemetryPayload t{};
        ipc::ChillStatePayload c{};
        bool has_t = client.get_telemetry(t);
        bool has_c = client.get_chill_state(c);

        std::cout << "=== AdrenaLux Status ===" << std::endl;
        std::cout << "Daemon IPC: Connected (" << client.socket_path() << ")" << std::endl;
        if (has_t) {
            std::cout << "\n[AMD GPU Telemetry]" << std::endl;
            std::cout << "  Edge Temp:      " << std::fixed << std::setprecision(1) << (t.gpu_temp_edge_mc / 1000.0) << " °C" << std::endl;
            std::cout << "  Hotspot Temp:   " << std::fixed << std::setprecision(1) << (t.gpu_temp_junction_mc / 1000.0) << " °C" << std::endl;
            std::cout << "  VRAM Temp:      " << std::fixed << std::setprecision(1) << (t.gpu_temp_mem_mc / 1000.0) << " °C" << std::endl;
            std::cout << "  Core Clock:     " << t.core_clock_mhz << " MHz" << std::endl;
            std::cout << "  Mem Clock:      " << t.mem_clock_mhz << " MHz" << std::endl;
            std::cout << "  Power Draw:     " << (t.power_draw_mw / 1000.0) << " W (Cap: " << (t.power_cap_mw / 1000.0) << " W)" << std::endl;
            std::cout << "  Fan Speed:      " << t.fan_rpm << " RPM" << std::endl;
            std::cout << "  VRAM Used:      " << t.vram_used_mb << " MB / " << t.vram_total_mb << " MB" << std::endl;
        }
        if (has_c) {
            std::cout << "\n[Open Chill State]" << std::endl;
            std::cout << "  Enabled:        " << (c.enabled ? "Yes" : "No") << std::endl;
            std::cout << "  Status:         " << (c.is_throttled ? "THROTTLED (Power-saving)" : "ACTIVE (Full rate)") << std::endl;
            std::cout << "  Target FPS:     " << (c.current_target_fps == 0 ? "Uncapped" : std::to_string(c.current_target_fps)) << std::endl;
            std::cout << "  Idle Time:      " << c.last_input_delta_ms << " ms" << std::endl;
        }
        return 0;
    }

    if (cmd == "telemetry") {
        ipc::TelemetryPayload t{};
        if (client.get_telemetry(t)) {
            std::cout << "GPU: " << (t.gpu_temp_edge_mc / 1000.0) << "°C | Hotspot: " 
                      << (t.gpu_temp_junction_mc / 1000.0) << "°C | " 
                      << (t.power_draw_mw / 1000.0) << "W | "
                      << t.core_clock_mhz << "MHz | Fan: " << t.fan_rpm << "RPM | VRAM: " 
                      << t.vram_used_mb << "/" << t.vram_total_mb << "MB" << std::endl;
        }
        return 0;
    }

    if (cmd == "chill") {
        ipc::ChillConfigPayload cfg{};
        cfg.enabled = true;
        cfg.idle_fps = 30;
        cfg.active_fps = 0;
        cfg.idle_timeout_ms = 1500;

        for (int i = 2; i < argc; ++i) {
            std::string arg = argv[i];
            if (arg == "--enable") cfg.enabled = true;
            else if (arg == "--disable") cfg.enabled = false;
            else if (arg == "--idle-fps" && i + 1 < argc) cfg.idle_fps = std::stoi(argv[++i]);
            else if (arg == "--active-fps" && i + 1 < argc) cfg.active_fps = std::stoi(argv[++i]);
            else if (arg == "--timeout-ms" && i + 1 < argc) cfg.idle_timeout_ms = std::stoi(argv[++i]);
        }

        ipc::ChillStatePayload out{};
        if (client.set_chill_config(cfg, out)) {
            std::cout << "Open Chill configuration updated successfully." << std::endl;
            std::cout << "  Enabled:    " << (out.enabled ? "Yes" : "No") << std::endl;
            std::cout << "  Target FPS: " << (out.current_target_fps == 0 ? "Uncapped" : std::to_string(out.current_target_fps)) << std::endl;
        }
        return 0;
    }

    if (cmd == "replay") {
        std::cout << "Triggering instant replay clip capture..." << std::endl;
        ipc::ReplayResultPayload res{};
        if (client.trigger_replay(0, res)) {
            if (res.success) {
                std::cout << "✓ " << res.message << " -> " << res.saved_path << std::endl;
            } else {
                std::cerr << "✗ Failed: " << res.message << std::endl;
            }
        }
        return 0;
    }

    if (cmd == "monitor") {
        std::cout << "\033[2J\033[H"; // Clear screen
        while (true) {
            ipc::TelemetryPayload t{};
            ipc::ChillStatePayload c{};
            client.get_telemetry(t);
            client.get_chill_state(c);

            std::cout << "\033[H"; // Move cursor to top
            std::cout << "=================== AdrenaLux Live Monitor ===================" << std::endl;
            std::cout << " Temp (Edge/Hotspot/Mem):  " 
                      << (t.gpu_temp_edge_mc / 1000.0) << "°C / " 
                      << (t.gpu_temp_junction_mc / 1000.0) << "°C / "
                      << (t.gpu_temp_mem_mc / 1000.0) << "°C       " << std::endl;
            std::cout << " Clocks (Core / Memory):   " 
                      << t.core_clock_mhz << " MHz / " << t.mem_clock_mhz << " MHz           " << std::endl;
            std::cout << " Power Draw:               " 
                      << (t.power_draw_mw / 1000.0) << " W (Cap: " << (t.power_cap_mw / 1000.0) << " W)       " << std::endl;
            std::cout << " Fan:                      " << t.fan_rpm << " RPM (PWM: " << t.fan_pwm << ")            " << std::endl;
            std::cout << " VRAM:                     " << t.vram_used_mb << " MB / " << t.vram_total_mb << " MB       " << std::endl;
            std::cout << "--------------------------------------------------------------" << std::endl;
            std::cout << " Open Chill:               " << (c.enabled ? "ENABLED" : "DISABLED") << std::endl;
            std::cout << " Pacing Mode:              " 
                      << (c.is_throttled ? ">> THROTTLED (Menus/Idle) <<" : ">> ACTIVE (Full Speed) <<") << "    " << std::endl;
            std::cout << " Target Framerate:         " 
                      << (c.current_target_fps == 0 ? "Uncapped" : std::to_string(c.current_target_fps) + " FPS") << "         " << std::endl;
            std::cout << " Time Since Last Input:    " << (c.last_input_delta_ms / 1000.0) << " s                 " << std::endl;
            std::cout << "==============================================================" << std::endl;
            std::cout << "Press Ctrl+C to exit." << std::endl;

            std::this_thread::sleep_for(std::chrono::milliseconds(200));
        }
        return 0;
    }

    std::cerr << "Unknown command: " << cmd << std::endl;
    print_help();
    return 1;
}
