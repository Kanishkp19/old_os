//! Lock service transport. Unlock is refused until compositor lock surfaces exist.

#[cfg(unix)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt;

    tracing_subscriber::fmt::init();
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").ok_or("XDG_RUNTIME_DIR is required")?;
    let path = std::path::PathBuf::from(runtime).join("homehub-lock.sock");
    if path.exists() {
        return Err("lock socket already exists; refusing to replace it".into());
    }
    let socket = tokio::net::UnixDatagram::bind(&path)?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    tracing::warn!("compositor session-lock backend is unavailable; unlock requests are refused");
    let mut buffer = [0u8; 128];
    loop {
        tokio::select! {
            result = socket.recv_from(&mut buffer) => {
                let (length, peer) = result?;
                if &buffer[..length] == b"status" {
                    if let Some(peer_path) = peer.as_pathname() {
                        socket.send_to(b"locked", peer_path).await?;
                    }
                } else {
                    tracing::warn!("lock request refused: secure compositor backend unavailable");
                }
            }
            result = tokio::signal::ctrl_c() => {
                result?;
                return Err("lock service stopped while locked".into());
            }
        }
    }
}

#[cfg(not(unix))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err("Unix lock socket requires a Unix platform".into())
}
