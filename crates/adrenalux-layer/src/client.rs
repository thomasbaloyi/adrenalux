use adrenalux_core::protocol::{ChillState, Request, Response};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

pub struct LayerIpcClient {
    stream: Option<UnixStream>,
    cached_chill: ChillState,
    last_query: Instant,
}

impl LayerIpcClient {
    pub fn new() -> Self {
        let mut client = Self {
            stream: None,
            cached_chill: ChillState::default(),
            // Set in the past so the first call queries immediately
            last_query: Instant::now()
                .checked_sub(Duration::from_secs(10))
                .unwrap_or_else(Instant::now),
        };
        client.try_connect();
        // Pre-fetch initial state
        let _ = client.query_daemon();
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
                if stream.write_all(&len).is_ok() && stream.write_all(&encoded).is_ok() && stream.flush().is_ok() {
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

    pub fn get_chill_state(&mut self) -> ChillState {
        let now = Instant::now();
        // Query daemon at most once every 16ms
        if now.duration_since(self.last_query).as_millis() >= 16 {
            self.last_query = now;
            let _ = self.query_daemon();
        }

        self.cached_chill.clone()
    }
}
