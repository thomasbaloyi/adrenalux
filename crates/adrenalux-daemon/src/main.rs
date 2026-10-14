use adrenaluxd::chill::ChillEngine;
use adrenaluxd::dvr::DvrController;
use adrenaluxd::hwmon::HardwareMonitor;
use adrenaluxd::server::IpcServer;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("  AdrenaLux Daemon (adrenaluxd) v0.1.0");
    println!("  The Linux Gaming Performance Suite (Rust)");
    println!("========================================");

    let hw_mon = Arc::new(HardwareMonitor::new());
    let chill = Arc::new(ChillEngine::new());
    let dvr = Arc::new(DvrController::new());

    chill.start();

    let server = IpcServer::new(hw_mon, chill, dvr);

    tokio::select! {
        res = server.run() => {
            if let Err(e) = res {
                eprintln!("[adrenaluxd] Server failed: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("\n[adrenaluxd] Received Ctrl+C, shutting down gracefully...");
        }
    }

    let sock_path = adrenalux_core::get_socket_path();
    if sock_path.exists() {
        let _ = std::fs::remove_file(sock_path);
    }

    println!("[adrenaluxd] Stopped.");
    Ok(())
}
