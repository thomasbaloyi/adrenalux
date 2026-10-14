# AdrenaLux: Master Engineering Plan & Technical Specification

> **Project Goal:** Build a unified, high-performance Linux gaming overlay and management suite that brings the best of AMD Adrenalin (Unified In-Game HUD, Dynamic Frame Pacing / Radeon Chill, Instant Replay DVR, Hardware Tuning, and Automated OptiScaler / DLSS-to-FSR profiles) to the Linux gaming ecosystem.

---

## 1. Executive Summary & Problem Space

### The Problem
On Windows, AMD users enjoy **AMD Software: Adrenalin Edition** — an integrated suite providing:
1. An in-game overlay triggered via `Alt+R` / `Alt+Z`.
2. **Radeon Chill & Boost**: Dynamic, input-aware frame limiting and motion-based dynamic resolution scaling.
3. **Radeon ReLive**: Zero-friction, hardware-accelerated DVR replay buffering and clipping.
4. **Hardware Tuning**: Power limits, custom fan curves, voltage offsets, and real-time telemetry.
5. **Driver-level upscaling / mod management**: FSR toggles and game-specific profiles.

On Linux, the open-source driver stack (**Mesa RADV + Linux amdgpu**) delivers exceptional raw compute performance and stability, but the user experience is **fragmented across disconnected tools**:
* Overclocking/Fan control requires **LACT** or **CoreCtrl**.
* On-screen metrics require **MangoHud**.
* DVR clipping requires standalone **`gpu-screen-recorder`**.
* Upscaling requires manual **Gamescope** CLI flags or copying DLLs for **OptiScaler**.
* **Dynamic input-aware frame limiting (Chill)** does not exist on Linux at all — limiters are strictly static.

### The Solution: AdrenaLux
**AdrenaLux** provides a modular, daemon-driven architecture featuring:
* A **Vulkan Implicit Layer** that renders a sleek in-game overlay (Dear ImGui) directly into the game's swapchain on `Alt+R`.
* An **Open Chill engine** monitoring `evdev` input events to dynamically throttle framerates during menus/AFK and instantly ramp up upon user action.
* An **OptiScaler & Profile Manager** that identifies running Steam/Proton games, injects DLSS-to-FSR translation layers, and allows in-game configuration of upscaling and frame generation.
* An **Instant Replay Controller** backed by `gpu-screen-recorder` for hardware-accelerated clipping via an in-game hotkey.

---

## 2. System Architecture

```
+-----------------------------------------------------------------------------------+
|                              GAME PROCESS (Proton / Native)                       |
|                                                                                   |
|   +-------------------+       +-----------------------------------------------+   |
|   |    Game Engine    | ----> |              Vulkan Swapchain                 |   |
|   |  (D3D11/12/Vulkan)|       +-----------------------------------------------+   |
|   +-------------------+                               |                           |
|             |                                         v                           |
|             |                        +---------------------------------+          |
|             |                        |   AdrenaLux Vulkan Layer        |          |
|             v                        |   (VK_LAYER_ADRENALUX_overlay)  |          |
|   +-------------------+              +---------------------------------+          |
|   | OptiScaler Bridge |                               |                           |
|   | (DLSS -> FSR 3.1) |                               | ImGui Render & Frame Pacer|
|   +-------------------+                               v                           |
|                                                  Display Present                  |
+-----------------------------------------------------------------------------------+
                                         ^
                                         | Local UNIX Socket IPC
                                         v
+-----------------------------------------------------------------------------------+
|                            ADRENALUX CORE DAEMON (adrenaluxd)                     |
|                                                                                   |
|  +--------------------+   +--------------------+   +---------------------------+  |
|  |   Input Monitor    |   |   Hardware Engine  |   |    DVR Replay Engine      |  |
|  |   (libevdev Chill) |   |   (amdgpu sysfs)   |   |   (gpu-screen-recorder)   |  |
|  +--------------------+   +--------------------+   +---------------------------+  |
|                                                                                   |
|  +--------------------+   +--------------------+   +---------------------------+  |
|  |   Profile Engine   |   |  OptiScaler Bridge |   |      D-Bus / CLI API      |  |
|  | (Steam/Proton scan)|   | (DLL injection/INI)|   |   (adrenalux desktop app) |  |
|  +--------------------+   +--------------------+   +---------------------------+  |
+-----------------------------------------------------------------------------------+
```

---

## 3. Core Modules & Technical Specifications

### Module A: In-Game Overlay (`adrenalux-overlay`)
* **Technology:** C++ / Rust with Vulkan API and **Dear ImGui**.
* **Mechanism:** Registered as an implicit Vulkan layer (`~/.local/share/vulkan/implicit_layer.d/VkLayer_ADRENALUX_overlay.json`).
* **Interceptions:**
  * `vkCreateDevice` / `vkDestroyDevice`: Initialize graphics pipeline and descriptor pools.
  * `vkCreateSwapchainKHR`: Hook render targets and format specifications.
  * `vkQueuePresentKHR`: Render ImGui overlay passes on top of game frame before handoff to compositor.
* **Hotkey Handling:** Global shortcut (`Alt+R` or configurable `Alt+Z`) toggles UI focus and input capture.
* **Features:**
  * Telemetry HUD (Frametime graph, 1% lows, GPU Clock, VRAM, Hotspot Temp, Power Draw).
  * Quick Actions: Save Replay (hotkey), Toggle Chill, Upscaling Mode selector.
  * Hardware Tuning drawer (Fan profiles, Power Limit slider).

### Module B: Open Chill & Boost (`adrenalux-chill`)
* **Objective:** Deliver the power-saving and thermal benefits of AMD Radeon Chill and Boost on Linux.
* **Input Tracking:**
  * Daemon listens to `/dev/input/event*` devices using `libevdev` (mouse, keyboard, gamepad).
  * Measures idle duration $\Delta t = t_{\text{current}} - t_{\text{last\_input}}$.
* **Dynamic Frame Pacing Algorithm:**
  ```text
  If (now - last_input_time) > idle_timeout_ms:
      target_fps = lerp(target_fps, fps_idle, transition_rate)
  Else:
      target_fps = fps_active (or uncapped)

  sleep_duration = calculate_target_frame_time(target_fps) - render_time
  if sleep_duration > 0:
      clock_nanosleep(CLOCK_MONOTONIC, sleep_duration)
  ```
* **Open Boost:** Detects rapid camera sweeps ($d\theta/dt > \text{threshold}$) and signals the upscaler or Gamescope to temporarily downscale dynamic render resolution to prioritize FPS consistency.

### Module C: OptiScaler & Game Profile Engine (`adrenalux-profiles`)
* **Objective:** Enable seamless DLSS-to-FSR upscaling and frame generation in Proton titles without manual DLL tinkering.
* **Game Detection:**
  * Monitors `/proc` or queries Steam API / `libraryfolders.vdf` for active game launching (`AppId`).
  * Resolves game directory and Wine prefix path (`compatdata/<AppId>/pfx`).
* **Automated Injection:**
  * Deploys OptiScaler artifacts (`dxgi.dll`, `nvngx.dll`, `libxess.dll`) into the game binary directory.
  * Configures Wine DLL overrides automatically (`WINEDLLOVERRIDES="dxgi=n,b;nvngx=n,b"`).
* **Live In-Game Config Sync:**
  * Parses and writes `OptiScaler.ini` in real-time.
  * Allows the user to toggle FSR 3.1 Frame Gen, adjust sharpness, and change upscaling presets (Quality, Balanced, Performance) directly from the in-game overlay without restarting the game.

### Module D: Replay DVR & AMD Hardware Control (`adrenalux-sys`)
* **Instant Replay (DVR):**
  * Spawns `gpu-screen-recorder` in background replay-buffer mode using AMD VCN hardware encoding (`-c mp4 -r 60 -w screen`).
  * On hotkey / overlay trigger: Sends `SIGUSR1` to immediately dump the ring buffer into `~/Videos/Replays/` with zero CPU overhead.
  * Displays in-overlay toast notification confirming clip capture.
* **Hardware Telemetry & Tuning:**
  * Directly interfaces with `/sys/class/drm/card*/device/hwmon/hwmon*/`:
    * Edge, junction/hotspot, and VRAM temperatures.
    * Real-time power draw (`power1_average`) and power limits (`power1_cap`).
    * Fan speed (`fan1_input`) and manual PWM curve modulation (`pwm1`).
  * Integrates with or leverages `amdgpu.ppfeaturemask` overdrive endpoints for safe clock/voltage offsets.

---

## 4. Multi-Month Implementation Roadmap

```mermaid
gantt
    title AdrenaLux Development Schedule
    dateFormat  YYYY-MM-DD
    section Phase 1: Core Foundation
    Repo & Architecture Setup          :a1, 2026-11-01, 14d
    Vulkan Layer Skeleton & ImGui Hook :a2, after a1, 21d
    Daemon IPC (UNIX Domain Socket)    :a3, after a2, 14d
    section Phase 2: Open Chill & Pacer
    libevdev Input Tracker Daemon      :b1, 2026-12-15, 14d
    vkQueuePresentKHR Pacing Engine    :b2, after b1, 21d
    Chill UI Controls & Tuning         :b3, after b2, 10d
    section Phase 3: Hardware & DVR
    gpu-screen-recorder Replay Hook    :c1, 2027-01-20, 14d
    amdgpu sysfs Telemetry & Fan Curves:c2, after c1, 21d
    In-Game Replay Clip Notification   :c3, after c2, 10d
    section Phase 4: OptiScaler Integration
    Steam Game & Proton Prefix Scanner :d1, 2027-02-25, 14d
    DLL Injection & Override Engine    :d2, after d1, 21d
    In-Game OptiScaler.ini Live Tweak  :d3, after d2, 14d
    section Phase 5: Polishing & Release
    Per-Game Profile Cloud/Local DB    :e1, 2027-04-01, 14d
    Packaging (AUR, Flatpak, Debian)   :e2, after e1, 14d
    Beta Testing & Public Release      :e3, after e2, 14d
```

### Month 1: Architecture Foundation & Core IPC
- [x] Initialize Cargo workspace layout (`adrenalux-core`, `adrenalux-daemon`, `adrenalux-ctl`, `adrenalux-layer`).
- [x] Design typed binary IPC protocol over UNIX domain sockets (`bincode` + `serde`).
- [x] Build minimal Vulkan Implicit Layer intercepting loader negotiation, `vkCreateDevice`, and `vkQueuePresentKHR`.
- [x] Implement monotonic precision frame pacer (`clock_nanosleep`) in `adrenalux-layer`.
- [x] Build `adrenaluxd` background daemon with auto-detection of AMD GPUs and sysfs metrics.
- [x] Build `adrenalux-ctl` CLI client (`status`, `telemetry`, `chill`, `replay`, `monitor`).
- [x] Implement automated test suite (19 unit, component, and integration tests passing).
- [ ] Integrate in-game HUD rendering pass inside `vkQueuePresentKHR`.
- [ ] Implement global hotkey toggle (`Alt+R` / `Alt+Z`) for overlay focus.

### Month 2: Open Chill (Input-Aware Frame Limiting) & Pacing Tests
- [x] Initial `libevdev` input tracking daemon and idle delta calculation.
- [x] Dynamic hysteresis state machine (active FPS vs throttled idle FPS).
- [x] In-game & benchmark validation with `vkcube` and `vkmark` on live Radeon RX 9070 XT.
- [x] Absolute monotonic frame pacing (`TIMER_ABSTIME`) verified at 30 FPS, 60 FPS, and uncapped.
- [ ] Advanced input velocity monitoring (mouse delta velocity for Open Boost).
- [ ] Automated pacing jitter test suite (benchmarking frametime variance at 30/60/144 FPS).
- [ ] Power consumption benchmarking on AMD RDNA 4 (RX 9070 XT) across menus vs active gameplay.

### Month 3: Instant Replay DVR & AMD Hardware Controls
- [x] Foundation: Process discovery and signal handling for `gpu-screen-recorder`.
- [ ] Build background managed replay session manager (auto-launching buffer if not running).
- [ ] In-game overlay trigger and hotkey (`Alt+F10` / custom) to fire `SIGUSR1` clip save.
- [ ] In-overlay toast notification confirming clip capture with file link.
- [ ] Advanced fan curve profile editor and power cap slider in overlay.
- [ ] Hardware mock test harness simulating fan curve and power limit validation without risking hardware.

### Month 4: OptiScaler & Proton Game Profiles
- [ ] Implement Steam library discovery (`~/.local/share/Steam` and external library folders).
- [ ] Build automated OptiScaler injection module for detected Proton prefixes (`WINEDLLOVERRIDES`).
- [ ] Create parser and writer for `OptiScaler.ini`.
- [ ] Add in-game overlay panel for adjusting upscaling quality, sharpness, and frame generation on the fly.
- [ ] Automated regression tests verifying `OptiScaler.ini` roundtrip parsing and prefix discovery.

### Month 5: Optimization, Packaging & Community Launch
- [ ] Implement per-game profile persistence (storing custom Chill limits, fan curves, and upscaler choices per Steam AppId).
- [ ] Test compatibility across major compositors (KDE KWin Wayland, GNOME Mutter, Hyprland, Gamescope).
- [ ] Package for Arch Linux (AUR), Fedora (COPR), and Debian/Ubuntu.
- [ ] Publish documentation, installation guide, and demonstration video.

---

## 5. Technology Stack Decisions

| Component | Language / Framework | Justification |
| :--- | :--- | :--- |
| **Vulkan Layer** | Rust (`cdylib`) | Low overhead, zero runtime pause/GC, pure C ABI compatibility, memory safety. |
| **Core Daemon (`adrenaluxd`)** | Rust (Tokio) | Concurrency safety without data races, fast async I/O, evdev monitoring, and sysfs. |
| **Control CLI (`adrenalux-ctl`)** | Rust (Clap) | Fast, expressive CLI framework with zero runtime dependencies. |
| **IPC Protocol** | Rust (`bincode` / `serde`) | Ultra-fast zero-copy binary serialization over UNIX domain sockets. |
| **DVR Backend** | `gpu-screen-recorder` | Extremely fast, zero frame-drop hardware encoder using AMD VCN / VA-API. |
| **Upscaling Integration** | OptiScaler (FSR 2/3.1 bridge) | De-facto standard for translating DLSS/XeSS calls to AMD FSR. |

---

## 6. Testing & Quality Assurance Architecture

Testing is built directly into every crate in the AdrenaLux workspace to ensure zero regressions across drivers, kernel updates, and game releases.

### 6.1 Automated Unit Tests (`cargo test`)
* **`adrenalux-core`:**
  * Exhaustive serialization and deserialization roundtrip tests for every `Request` and `Response` variant.
  * Validation of default configurations and socket path determination.
* **`adrenalux-daemon`:**
  * **Mock Sysfs Test Harness (`test_telemetry_parsing_with_mock_sysfs`):** Creates temporary directory trees mimicking `/sys/class/drm/card*/device/hwmon/hwmon*` to verify correct conversion of micro-watts, milli-Celsius, and Hz across different driver sysfs formats.
  * **Fault Tolerance (`test_missing_files_graceful_fallback`):** Verifies that missing or unreadable sensors return safe defaults rather than panicking.
  * **Chill State Machine (`test_chill_state_transitions`):** Simulates user activity and idle timeouts, verifying exact state transitions between throttled and active targets.
* **`adrenalux-layer`:**
  * **Frame Pacer Precision (`test_pacer_pacing_rate`):** Verifies that monotonic `clock_nanosleep` pacing meets target frame intervals with sub-millisecond accuracy.

### 6.2 Integration Testing (`tests/ipc_integration_test.rs`)
* Spawns an isolated Tokio UNIX domain socket server on a temporary path.
* Runs concurrent client requests across threads to verify:
  * Ping/Pong liveliness.
  * Live telemetry dispatch.
  * Dynamic configuration updates applied and echoed back.
  * Error recovery on unexpected client disconnection.

### 6.3 Hardware Validation Suite
* **Real-Device Testing:** Tested and benchmarked live on **AMD Radeon RX 9070 XT** (Navi 48 / RDNA 4, Linux 7.2.9 CachyOS).
* **Game/Vulkan Validation Matrix:**
  * Stage 1: Synthetic Vulkan test via `vkcube` with `ADRENALUX_OVERLAY=1`.
  * Stage 2: Native Linux Vulkan games (e.g., Counter-Strike 2).
  * Stage 3: Steam Proton titles with DXVK (DirectX 11) and VKD3D-Proton (DirectX 12).

---

## 7. Risk Analysis & Mitigations

1. **Wayland Input Capture during Overlay Mode:**
   * *Risk:* Wayland compositors isolate input between surfaces.
   * *Mitigation:* Because the Vulkan layer lives inside the game's own rendering process, input events passed to the game window can be intercepted before reaching the game logic, bypassing compositor-level isolation.
2. **Anti-Cheat Interference:**
   * *Risk:* Kernel or aggressive anticheat (Easy Anti-Cheat / BattlEye) might flag unfamiliar injected libraries.
   * *Mitigation:* AdrenaLux functions as a standard Vulkan layer (identical to MangoHud, Steam Overlay, and vkBasalt). Allow games with strict anti-cheat to be blacklisted in profile settings.
3. **Privilege Requirements for GPU Tuning:**
   * *Risk:* Writing to `power1_cap` or `pwm1` requires root/superuser permissions.
   * *Mitigation:* `adrenaluxd` will include a dedicated helper daemon or polkit policy rule granting secure sysfs write access without requiring the main daemon to run as root.

---

## 8. Current Status & Next Steps

* **Current Status:** Phase 1 Foundation complete. 100% Rust workspace compiling in release mode with 11 automated unit and integration tests passing.
* **Next Active Sprint:**
  1. Vulkan Swapchain HUD rendering pass inside `crates/adrenalux-layer`.
  2. In-game `Alt+R` hotkey activation.
  3. OptiScaler Proton prefix scanner.
