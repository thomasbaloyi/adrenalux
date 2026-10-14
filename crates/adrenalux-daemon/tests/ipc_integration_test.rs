use adrenalux_core::protocol::{ChillConfig, Request, Response};
use adrenaluxd::chill::ChillEngine;
use adrenaluxd::dvr::DvrController;
use adrenaluxd::hwmon::HardwareMonitor;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

#[tokio::test]
async fn test_ipc_server_roundtrip() {
    let test_sock_path =
        std::env::temp_dir().join(format!("adrenalux_test_ipc_{}.sock", std::process::id()));
    if test_sock_path.exists() {
        let _ = std::fs::remove_file(&test_sock_path);
    }

    let hw_mon = Arc::new(HardwareMonitor::new());
    let chill = Arc::new(ChillEngine::new());
    let dvr = Arc::new(DvrController::new());

    let listener = UnixListener::bind(&test_sock_path).expect("Failed to bind test socket");

    let hw = Arc::clone(&hw_mon);
    let ch = Arc::clone(&chill);
    let dv = Arc::clone(&dvr);

    // Spawn test server loop
    tokio::spawn(async move {
        if let Ok((mut stream, _)) = listener.accept().await {
            let mut buf = vec![0u8; 4096];
            loop {
                let mut len_buf = [0u8; 4];
                if stream.read_exact(&mut len_buf).await.is_err() {
                    break;
                }
                let len = u32::from_le_bytes(len_buf) as usize;
                if len > buf.len() {
                    buf.resize(len, 0);
                }
                if stream.read_exact(&mut buf[..len]).await.is_err() {
                    break;
                }

                let req: Request = bincode::deserialize(&buf[..len]).unwrap();
                let resp = match req {
                    Request::Ping => Response::Pong,
                    Request::GetTelemetry => Response::Telemetry(hw.read_telemetry()),
                    Request::GetChillState => Response::Chill(ch.get_state()),
                    Request::SetChillConfig(cfg) => {
                        ch.update_config(cfg);
                        Response::Chill(ch.get_state())
                    }
                    Request::TriggerReplay { buffer_seconds } => {
                        Response::Replay(dv.trigger_replay(buffer_seconds))
                    }
                    Request::ToggleOverlay => {
                        ch.toggle_overlay();
                        Response::Chill(ch.get_state())
                    }
                    Request::SetOverlayOpen(open) => {
                        ch.set_overlay_open(open);
                        Response::Chill(ch.get_state())
                    }
                };

                let encoded = bincode::serialize(&resp).unwrap();
                let resp_len = (encoded.len() as u32).to_le_bytes();
                let _ = stream.write_all(&resp_len).await;
                let _ = stream.write_all(&encoded).await;
                let _ = stream.flush().await;
            }
        }
    });

    // Give server a moment to start listening
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Connect client
    let mut client = UnixStream::connect(&test_sock_path)
        .await
        .expect("Failed to connect client");

    // 1. Test Ping
    let ping_req = Request::Ping;
    let enc = bincode::serialize(&ping_req).unwrap();
    client
        .write_all(&(enc.len() as u32).to_le_bytes())
        .await
        .unwrap();
    client.write_all(&enc).await.unwrap();
    client.flush().await.unwrap();

    let mut len_buf = [0u8; 4];
    client.read_exact(&mut len_buf).await.unwrap();
    let resp_len = u32::from_le_bytes(len_buf) as usize;
    let mut resp_buf = vec![0u8; resp_len];
    client.read_exact(&mut resp_buf).await.unwrap();
    let resp: Response = bincode::deserialize(&resp_buf).unwrap();
    assert_eq!(resp, Response::Pong);

    // 2. Test SetChillConfig
    let set_cfg = Request::SetChillConfig(ChillConfig {
        enabled: true,
        idle_fps: 42,
        active_fps: 165,
        idle_timeout_ms: 3000,
        boost_enabled: false,
    });
    let enc = bincode::serialize(&set_cfg).unwrap();
    client
        .write_all(&(enc.len() as u32).to_le_bytes())
        .await
        .unwrap();
    client.write_all(&enc).await.unwrap();
    client.flush().await.unwrap();

    client.read_exact(&mut len_buf).await.unwrap();
    let resp_len = u32::from_le_bytes(len_buf) as usize;
    let mut resp_buf = vec![0u8; resp_len];
    client.read_exact(&mut resp_buf).await.unwrap();
    let resp: Response = bincode::deserialize(&resp_buf).unwrap();
    if let Response::Chill(state) = resp {
        assert!(state.enabled);
        assert_eq!(state.current_target_fps, 165); // active because not idle yet
    } else {
        panic!("Expected Chill response");
    }

    let _ = std::fs::remove_file(test_sock_path);
}
