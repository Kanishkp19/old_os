//! Bounded newline JSON IPC. Production is a session-scoped Windows pipe,
//! protected by an explicit LocalSystem-only DACL and two-way identity checks.
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use hh_remote::{helper_client::{MAX_FRAME_BYTES, IPC_TIMEOUT}, helper_protocol::{HelperRequest, HelperResponse}};

pub fn default_socket_path() -> String {
    std::env::temp_dir().join("homehub-session.sock").to_string_lossy().to_string()
}
/// Exactly one request per connection; reconnects do not own media lifetime.
pub async fn handle_stream<S>(mut stream: S) -> std::io::Result<()>
where S: AsyncRead + AsyncWrite + Unpin {
    let mut line = Vec::new();
    let mut reader = tokio::io::BufReader::new(&mut stream);
    tokio::time::timeout(IPC_TIMEOUT, async {
        loop {
            let byte = reader.read_u8().await?;
            if byte == b'\n' { break; }
            if line.len() >= MAX_FRAME_BYTES {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "helper frame too large"));
            }
            line.push(byte);
        }
        Ok(())
    }).await.map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "helper request timed out"))??;
    drop(reader);
    static DISPATCH_SLOTS: once_cell::sync::Lazy<std::sync::Arc<tokio::sync::Semaphore>> = once_cell::sync::Lazy::new(|| std::sync::Arc::new(tokio::sync::Semaphore::new(4)));
    let response = match serde_json::from_slice::<HelperRequest>(&line) {
        Ok(request) => {
            let permit = DISPATCH_SLOTS.clone().try_acquire_owned().map_err(|_| std::io::Error::other("helper busy"))?;
            let deadline = if matches!(request, HelperRequest::ScreenOffer { .. }) { std::time::Duration::from_secs(30) } else { IPC_TIMEOUT };
            match tokio::time::timeout(deadline, tokio::task::spawn_blocking(move || { let _permit = permit; crate::execute(&request) })).await {
                Ok(Ok(response)) => response,
                Ok(Err(_)) => HelperResponse { ok: false, error: Some("helper dispatcher failed".into()), data: None },
                Err(_) => HelperResponse { ok: false, error: Some("helper operation timed out".into()), data: None },
            }
        },
        Err(_) => HelperResponse { ok: false, error: Some("invalid helper request".into()), data: None },
    };
    let mut out = serde_json::to_vec(&response).map_err(std::io::Error::other)?;
    if out.len() >= MAX_FRAME_BYTES { return Err(std::io::Error::other("helper response too large")); }
    out.push(b'\n');
    tokio::time::timeout(IPC_TIMEOUT, stream.write_all(&out)).await
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "helper response timed out"))??;
    Ok(())
}
#[cfg(unix)]
pub async fn serve_unix(path: &str) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    if std::env::var_os("HOMEHUB_DEV_HELPER").is_none() { anyhow::bail!("development helper requires HOMEHUB_DEV_HELPER"); }
    // Do not unlink an arbitrary existing path or socket owned by another process.
    let listener = tokio::net::UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    loop {
        let (stream, _) = listener.accept().await?;
        // Serial dispatch bounds outstanding connections and screen starts.
        if let Err(e) = handle_stream(stream).await { tracing::debug!(error = %e, "helper connection ended"); }
    }
}
#[cfg(windows)]
pub async fn serve_windows() -> anyhow::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use tokio::net::windows::named_pipe::ServerOptions;
    use hh_remote::windows_security::{current_pipe_name, active_pipe_name, PipeSecurity, ServiceLifetime, verify_service_client};
    let pipe_name = current_pipe_name().ok_or_else(|| anyhow::anyhow!("interactive user session required"))?;
    let security = PipeSecurity::new()?;
    let mut attributes = security.attributes();
    let make_pipe = |first| unsafe {
        ServerOptions::new().first_pipe_instance(first).reject_remote_clients(true)
            .create_with_security_attributes_raw(&pipe_name, std::ptr::from_mut(&mut attributes).cast())
    };
    let mut make_pipe = make_pipe;
    let mut pipe = make_pipe(true)?;
    let mut service = None::<ServiceLifetime>;
    loop {
        tokio::select! {
            result = pipe.connect() => result?,
            _ = tokio::signal::ctrl_c() => { let _ = crate::screen::stop(); let _ = crate::dispatch::platform::release_input(); return Ok(()); },
            _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {
                if active_pipe_name().as_deref() != Some(pipe_name.as_str()) || service.as_ref().is_some_and(|peer| !peer.is_alive()) {
                    let _ = crate::screen::stop(); let _ = crate::dispatch::platform::release_input();
                    service = None;
                }
                continue;
            }
        }
        if verify_service_client(pipe.as_raw_handle()) {
            let lifetime = ServiceLifetime::from_pipe(pipe.as_raw_handle());
            if lifetime.is_some() {
                service = lifetime;
                if let Err(e) = handle_stream(&mut pipe).await { tracing::debug!(error = %e, "helper connection ended"); }
            }
        } else { tracing::warn!("session IPC identity rejected"); }
        // Keep the existing instance until its successor has been created,
        // preventing another process from claiming the pipe name in a gap.
        let next = make_pipe(false)?;
        drop(pipe);
        pipe = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn reconnect_protocol_answers_one_bounded_request() {
        let (mut client, server) = tokio::io::duplex(4096);
        let task = tokio::spawn(handle_stream(server));
        client.write_all(b"{\"op\":\"ping\"}\n").await.unwrap();
        let mut response = String::new();
        use tokio::io::AsyncBufReadExt;
        tokio::io::BufReader::new(client).read_line(&mut response).await.unwrap();
        assert!(serde_json::from_str::<HelperResponse>(&response).unwrap().ok);
        task.await.unwrap().unwrap();
    }
    #[tokio::test]
    async fn invalid_json_fails_without_dispatching_input() {
        let (mut client, server) = tokio::io::duplex(4096);
        let task = tokio::spawn(handle_stream(server));
        client.write_all(b"invalid\n").await.unwrap();
        let mut response = String::new();
        use tokio::io::AsyncBufReadExt;
        tokio::io::BufReader::new(client).read_line(&mut response).await.unwrap();
        assert!(!serde_json::from_str::<HelperResponse>(&response).unwrap().ok);
        task.await.unwrap().unwrap();
    }
}
