use adrenalux_core::protocol::{ChillState, Request, Response, TelemetryData};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

pub struct LayerIpcClient {
    stream: Option<UnixStream>,
    cached_chill: ChillState,
    cached_telemetry: TelemetryData,
    last_query: Instant,
    last_telemetry_query: Instant,
}

impl LayerIpcClient {
    pub fn new() -> Self {
        let past = Instant::now()
            .checked_sub(Duration::from_secs(10))
            .unwrap_or_else(Instant::now);

        let mut client = Self {
            stream: None,
            cached_chill: ChillState::default(),
            cached_telemetry: TelemetryData::default(),
            last_query: past,
            last_telemetry_query: past,
        };
        client.try_connect();
        let _ = client.query_daemon();
        let _ = client.query_telemetry();
        client
    }

    fn try_connect(&mut self) {
        if self.stream.is_none() {
            let path = adrenalux_core::get_socket_path();
            if let Ok(s) = UnixStream::connect(path) {
                let _ = s.set_read_timeout(Some(Duration::from_millis(50)));
                let _ = s.set_write_timeout(Some(Duration::from_millis(50)));
                self.stream = Some(s);
            }
        }
    }

    fn query_daemon(&mut self) -> Option<ChillState> {
        self.try_connect();

        if let Some(stream) = &mut self.stream {
            let req = Request::GetChillState;
            if let Ok(encoded) = bincode::serialize(&req) {
                let len = (encoded.len() as u32).to_le_bytes();
                if stream.write_all(&len).is_ok()
                    && stream.write_all(&encoded).is_ok()
                    && stream.flush().is_ok()
                {
                    let mut len_buf = [0u8; 4];
                    if stream.read_exact(&mut len_buf).is_ok() {
                        let resp_len = u32::from_le_bytes(len_buf) as usize;
                        let mut resp_buf = vec![0u8; resp_len];
                        if stream.read_exact(&mut resp_buf).is_ok() {
                            if let Ok(Response::Chill(st)) = bincode::deserialize(&resp_buf) {
                                self.cached_chill = st;
                                return Some(self.cached_chill.clone());
                            }
                        }
                    }
                }
            }
        }

        self.stream = None;
        None
    }

    fn query_telemetry(&mut self) -> Option<TelemetryData> {
        self.try_connect();

        if let Some(stream) = &mut self.stream {
            let req = Request::GetTelemetry;
            if let Ok(encoded) = bincode::serialize(&req) {
                let len = (encoded.len() as u32).to_le_bytes();
                if stream.write_all(&len).is_ok()
                    && stream.write_all(&encoded).is_ok()
                    && stream.flush().is_ok()
                {
                    let mut len_buf = [0u8; 4];
                    if stream.read_exact(&mut len_buf).is_ok() {
                        let resp_len = u32::from_le_bytes(len_buf) as usize;
                        let mut resp_buf = vec![0u8; resp_len];
                        if stream.read_exact(&mut resp_buf).is_ok() {
                            if let Ok(Response::Telemetry(tel)) = bincode::deserialize(&resp_buf) {
                                self.cached_telemetry = tel;
                                return Some(self.cached_telemetry.clone());
                            }
                        }
                    }
                }
            }
        }

        self.stream = None;
        None
    }

    pub fn get_chill_state(&mut self) -> ChillState {
        let now = Instant::now();
        // Query daemon at most once every 16ms
        if now.duration_since(self.last_query).as_millis() >= 16 {
            self.last_query = now;
            let _ = self.query_daemon();
        }

        self.cached_chill.clone()
    }

    pub fn get_telemetry(&mut self) -> TelemetryData {
        let now = Instant::now();
        // Query telemetry at most once every 250ms
        if now.duration_since(self.last_telemetry_query).as_millis() >= 250 {
            self.last_telemetry_query = now;
            let _ = self.query_telemetry();
        }

        self.cached_telemetry.clone()
    }
}
