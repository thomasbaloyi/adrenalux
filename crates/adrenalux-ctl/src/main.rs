use adrenalux_core::protocol::{ChillConfig, ChillState, Request, Response, TelemetryData};
use clap::{Parser, Subcommand};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::thread::sleep;
use std::time::Duration;

#[derive(Parser, Debug, PartialEq)]
#[command(name = "adrenalux-ctl")]
#[command(about = "AdrenaLux Control & Telemetry Utility (Rust)", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug, PartialEq)]
pub enum Commands {
    /// Show system telemetry and Open Chill status
    Status,
    /// Print raw AMD GPU telemetry metrics
    Telemetry,
    /// Configure or query Open Chill dynamic frame pacing
    Chill {
        #[arg(long)]
        enable: bool,
        #[arg(long)]
        disable: bool,
        #[arg(long)]
        idle_fps: Option<u32>,
        #[arg(long)]
        active_fps: Option<u32>,
        #[arg(long)]
        timeout_ms: Option<u64>,
    },
    /// Trigger an instant replay clip save via gpu-screen-recorder
    Replay {
        #[arg(short, long, default_value_t = 0)]
        seconds: u32,
    },
    /// Launch real-time live terminal monitor
    Monitor,
    /// Manage or query in-game Control Center overlay (Alt+A)
    Overlay {
        #[arg(long)]
        toggle: bool,
        #[arg(long)]
        open: bool,
        #[arg(long)]
        close: bool,
    },
}

struct Client {
    stream: UnixStream,
}

impl Client {
    fn connect() -> Result<Self, Box<dyn std::error::Error>> {
        let sock = adrenalux_core::get_socket_path();
        let stream = UnixStream::connect(&sock)?;
        Ok(Self { stream })
    }

    fn call(&mut self, req: Request) -> Result<Response, Box<dyn std::error::Error>> {
        let encoded = bincode::serialize(&req)?;
        let len = (encoded.len() as u32).to_le_bytes();
        self.stream.write_all(&len)?;
        self.stream.write_all(&encoded)?;
        self.stream.flush()?;

        let mut len_buf = [0u8; 4];
        self.stream.read_exact(&mut len_buf)?;
        let resp_len = u32::from_le_bytes(len_buf) as usize;

        let mut resp_buf = vec![0u8; resp_len];
        self.stream.read_exact(&mut resp_buf)?;
        let resp: Response = bincode::deserialize(&resp_buf)?;
        Ok(resp)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let mut client = match Client::connect() {
        Ok(c) => c,
        Err(_) => {
            eprintln!(
                "Error: Could not connect to adrenaluxd at {:?}",
                adrenalux_core::get_socket_path()
            );
            eprintln!("Ensure the daemon is running: adrenaluxd");
            std::process::exit(1);
        }
    };

    match cli.command {
        Commands::Status => {
            println!("=== AdrenaLux Status (Rust) ===");
            println!(
                "Daemon IPC: Connected ({:?})",
                adrenalux_core::get_socket_path()
            );

            if let Ok(Response::Telemetry(t)) = client.call(Request::GetTelemetry) {
                println!("\n[AMD GPU Telemetry]");
                println!("  Edge Temp:      {:.1} °C", t.gpu_temp_edge_c);
                println!("  Hotspot Temp:   {:.1} °C", t.gpu_temp_junction_c);
                println!("  VRAM Temp:      {:.1} °C", t.gpu_temp_mem_c);
                println!("  Core Clock:     {} MHz", t.core_clock_mhz);
                println!("  Mem Clock:      {} MHz", t.mem_clock_mhz);
                println!(
                    "  Power Draw:     {:.1} W (Cap: {:.1} W)",
                    t.power_draw_w, t.power_cap_w
                );
                println!("  Fan Speed:      {} RPM (PWM: {})", t.fan_rpm, t.fan_pwm);
                println!(
                    "  VRAM Used:      {} MB / {} MB",
                    t.vram_used_mb, t.vram_total_mb
                );
            }

            if let Ok(Response::Chill(c)) = client.call(Request::GetChillState) {
                println!("\n[Open Chill State]");
                println!("  Enabled:        {}", if c.enabled { "Yes" } else { "No" });
                println!(
                    "  Status:         {}",
                    if c.is_throttled {
                        "THROTTLED (Power-saving)"
                    } else {
                        "ACTIVE (Full speed)"
                    }
                );
                println!(
                    "  Target FPS:     {}",
                    if c.current_target_fps == 0 {
                        "Uncapped".to_string()
                    } else {
                        c.current_target_fps.to_string()
                    }
                );
                println!("  Idle Duration:  {} ms", c.last_input_delta_ms);
            }
        }

        Commands::Telemetry => {
            if let Ok(Response::Telemetry(t)) = client.call(Request::GetTelemetry) {
                println!(
                    "GPU: {:.1}°C | Hotspot: {:.1}°C | {:.1}W | {}MHz | Fan: {}RPM | VRAM: {}/{}MB",
                    t.gpu_temp_edge_c,
                    t.gpu_temp_junction_c,
                    t.power_draw_w,
                    t.core_clock_mhz,
                    t.fan_rpm,
                    t.vram_used_mb,
                    t.vram_total_mb
                );
            }
        }

        Commands::Chill {
            enable,
            disable,
            idle_fps,
            active_fps,
            timeout_ms,
        } => {
            let mut cfg = ChillConfig::default();
            if enable {
                cfg.enabled = true;
            }
            if disable {
                cfg.enabled = false;
            }
            if let Some(fps) = idle_fps {
                cfg.idle_fps = fps;
            }
            if let Some(fps) = active_fps {
                cfg.active_fps = fps;
            }
            if let Some(ms) = timeout_ms {
                cfg.idle_timeout_ms = ms;
            }

            if let Ok(Response::Chill(c)) = client.call(Request::SetChillConfig(cfg)) {
                println!("Open Chill configuration updated successfully.");
                println!("  Enabled:    {}", if c.enabled { "Yes" } else { "No" });
                println!(
                    "  Target FPS: {}",
                    if c.current_target_fps == 0 {
                        "Uncapped".to_string()
                    } else {
                        c.current_target_fps.to_string()
                    }
                );
            }
        }

        Commands::Replay { seconds } => {
            println!("Triggering instant replay clip save...");
            if let Ok(Response::Replay(res)) = client.call(Request::TriggerReplay {
                buffer_seconds: seconds,
            }) {
                if res.success {
                    println!("✓ {} -> {}", res.message, res.saved_path);
                } else {
                    eprintln!("✗ Failed: {}", res.message);
                }
            }
        }

        Commands::Monitor => {
            print!("\x1B[2J\x1B[H"); // Clear screen
            loop {
                let tel: TelemetryData = match client.call(Request::GetTelemetry) {
                    Ok(Response::Telemetry(t)) => t,
                    _ => break,
                };
                let chill: ChillState = match client.call(Request::GetChillState) {
                    Ok(Response::Chill(c)) => c,
                    _ => break,
                };

                print!("\x1B[H"); // Cursor home
                println!("=================== AdrenaLux Live Monitor (Rust) ===================");
                println!(
                    " Temp (Edge / Hotspot / VRAM): {:.1}°C / {:.1}°C / {:.1}°C       ",
                    tel.gpu_temp_edge_c, tel.gpu_temp_junction_c, tel.gpu_temp_mem_c
                );
                println!(
                    " Clocks (Core / Memory):       {} MHz / {} MHz                   ",
                    tel.core_clock_mhz, tel.mem_clock_mhz
                );
                println!(
                    " Power Draw:                   {:.1} W (Cap: {:.1} W)            ",
                    tel.power_draw_w, tel.power_cap_w
                );
                println!(
                    " Fan Speed:                    {} RPM (PWM: {})                  ",
                    tel.fan_rpm, tel.fan_pwm
                );
                println!(
                    " VRAM Allocation:              {} MB / {} MB                     ",
                    tel.vram_used_mb, tel.vram_total_mb
                );
                println!("---------------------------------------------------------------------");
                println!(
                    " Open Chill:                   {}",
                    if chill.enabled { "ENABLED" } else { "DISABLED" }
                );
                println!(
                    " Pacing Mode:                  {}    ",
                    if chill.is_throttled {
                        ">> THROTTLED (Menus/Idle) <<"
                    } else {
                        ">> ACTIVE (Full Speed) <<"
                    }
                );
                println!(
                    " Target Framerate:             {}         ",
                    if chill.current_target_fps == 0 {
                        "Uncapped".into()
                    } else {
                        format!("{} FPS", chill.current_target_fps)
                    }
                );
                println!(
                    " Time Since Last Input:        {:.1} s                           ",
                    (chill.last_input_delta_ms as f32) / 1000.0
                );
                println!("=====================================================================");
                println!("Press Ctrl+C to exit.");

                sleep(Duration::from_millis(200));
            }
        }

        Commands::Overlay { toggle, open, close } => {
            let req = if toggle {
                Request::ToggleOverlay
            } else if open {
                Request::SetOverlayOpen(true)
            } else if close {
                Request::SetOverlayOpen(false)
            } else {
                Request::GetChillState
            };

            if let Ok(Response::Chill(c)) = client.call(req) {
                println!(
                    "In-Game Control Center Overlay: {}",
                    if c.is_overlay_open { "OPEN" } else { "CLOSED" }
                );
                println!("Trigger hotkey: Alt+A");
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing_status() {
        let args = vec!["adrenalux-ctl", "status"];
        let cli = Cli::try_parse_from(args).expect("Failed to parse status command");
        assert_eq!(cli.command, Commands::Status);
    }

    #[test]
    fn test_cli_parsing_chill_flags() {
        let args = vec![
            "adrenalux-ctl",
            "chill",
            "--enable",
            "--idle-fps",
            "45",
            "--active-fps",
            "165",
            "--timeout-ms",
            "2500",
        ];
        let cli = Cli::try_parse_from(args).expect("Failed to parse chill command");
        if let Commands::Chill {
            enable,
            disable,
            idle_fps,
            active_fps,
            timeout_ms,
        } = cli.command
        {
            assert!(enable);
            assert!(!disable);
            assert_eq!(idle_fps, Some(45));
            assert_eq!(active_fps, Some(165));
            assert_eq!(timeout_ms, Some(2500));
        } else {
            panic!("Expected Chill command");
        }
    }

    #[test]
    fn test_cli_parsing_replay() {
        let args = vec!["adrenalux-ctl", "replay", "--seconds", "30"];
        let cli = Cli::try_parse_from(args).expect("Failed to parse replay command");
        assert_eq!(cli.command, Commands::Replay { seconds: 30 });
    }

    #[test]
    fn test_cli_parsing_overlay() {
        let args = vec!["adrenalux-ctl", "overlay", "--toggle"];
        let cli = Cli::try_parse_from(args).expect("Failed to parse overlay command");
        assert_eq!(
            cli.command,
            Commands::Overlay {
                toggle: true,
                open: false,
                close: false
            }
        );
    }
}
