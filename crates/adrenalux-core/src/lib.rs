pub mod protocol;

pub const DEFAULT_SOCKET_NAME: &str = "adrenalux.sock";

pub fn get_socket_path() -> std::path::PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        std::path::PathBuf::from(runtime_dir).join(DEFAULT_SOCKET_NAME)
    } else {
        let uid = unsafe { libc::getuid() };
        std::path::PathBuf::from(format!("/tmp/adrenalux-{}.sock", uid))
    }
}
