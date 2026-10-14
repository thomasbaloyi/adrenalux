use crate::chill::ChillEngine;
use crate::dvr::DvrController;
use crate::hwmon::HardwareMonitor;
use adrenalux_core::protocol::{Request, Response};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

pub struct IpcServer {
    hw_mon: Arc<HardwareMonitor>,
    chill: Arc<ChillEngine>,
    dvr: Arc<DvrController>,
}

impl IpcServer {
    pub fn new(
        hw_mon: Arc<HardwareMonitor>,
        chill: Arc<ChillEngine>,
        dvr: Arc<DvrController>,
    ) -> Self {
        Self { hw_mon, chill, dvr }
    }

    pub async fn run(&self) -> std::io::Result<()> {
        let sock_path = adrenalux_core::get_socket_path();
        if sock_path.exists() {
            let _ = std::fs::remove_file(&sock_path);
        }

        let listener = UnixListener::bind(&sock_path)?;
        println!("[IpcServer] Listening on UNIX domain socket: {:?}", sock_path);

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let hw = Arc::clone(&self.hw_mon);
                    let ch = Arc::clone(&self.chill);
                    let dv = Arc::clone(&self.dvr);

                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_client(stream, hw, ch, dv).await {
                            eprintln!("[IpcServer] Client handler error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("[IpcServer] Accept error: {}", e);
                }
            }
        }
    }

    async fn handle_client(
        mut stream: UnixStream,
        hw: Arc<HardwareMonitor>,
        ch: Arc<ChillEngine>,
        dv: Arc<DvrController>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut buf = vec![0u8; 4096];

        loop {
            // Read message size prefix (u32 little-endian)
            let mut len_buf = [0u8; 4];
            if stream.read_exact(&mut len_buf).await.is_err() {
                break; // Client disconnected
            }
            let len = u32::from_le_bytes(len_buf) as usize;
            if len > buf.len() {
                buf.resize(len, 0);
            }

            stream.read_exact(&mut buf[..len]).await?;
            let req: Request = bincode::deserialize(&buf[..len])?;

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
            };

            let encoded = bincode::serialize(&resp)?;
            let resp_len = (encoded.len() as u32).to_le_bytes();
            stream.write_all(&resp_len).await?;
            stream.write_all(&encoded).await?;
            stream.flush().await?;
        }

        Ok(())
    }
}
