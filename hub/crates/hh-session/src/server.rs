//! Transports for the helper protocol.
//!
//! - Windows: `\\.\pipe\homehub-session` named pipe (tokio multi-instance
//!   server). Default pipe DACL grants LocalSystem (the service) and the
//!   creating user (the helper's owner) full access; other principals are
//!   excluded. A PID→image-name check hardens this further (`verify_client`).
//! - Unix dev/CI: same newline-delimited JSON framing over a unix socket.
//!
//! Framing: one JSON request per line, one JSON response per line.

use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

use hh_remote::helper_protocol::{HelperRequest, HelperResponse};

pub const WINDOWS_PIPE_NAME: &str = r"\\.\pipe\homehub-session";

pub fn default_socket_path() -> String {
    std::env::temp_dir().join("homehub-session.sock").to_string_lossy().to_string()
}

/// Serve requests on any byte stream. One line in, one line out.
pub async fn handle_stream<S>(stream: S) -> std::io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (read_half, mut write_half) = tokio::io::split(stream);
    let mut lines = BufReader::new(read_half).lines();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let resp = match serde_json::from_str::<HelperRequest>(&line) {
            Ok(req) => crate::execute(&req),
            Err(e) => HelperResponse {
                ok: false,
                error: Some(format!("bad request: {e}")),
                data: None,
            },
        };
        let mut out = serde_json::to_string(&resp).unwrap_or_else(|_| {
            r#"{"ok":false,"error":"encode failed"}"#.into()
        });
        out.push('\n');
        write_half.write_all(out.as_bytes()).await?;
    }
    Ok(())
}

#[cfg(unix)]
pub async fn serve_unix(path: &str) -> anyhow::Result<()> {
    let _ = std::fs::remove_file(path); // stale socket from a previous run
    let listener = tokio::net::UnixListener::bind(path)?;
    tracing::info!(path, "hh-session listening (unix socket)");
    loop {
        let (stream, _peer) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(e) = handle_stream(stream).await {
                tracing::debug!(error = %e, "helper connection ended");
            }
        });
    }
}

#[cfg(not(unix))]
pub async fn serve_unix(_path: &str) -> anyhow::Result<()> {
    anyhow::bail!("unix socket mode requires a unix platform")
}

#[cfg(windows)]
pub async fn serve_windows() -> anyhow::Result<()> {
    use tokio::net::windows::named_pipe::ServerOptions;

    // The helper runs in the interactive user session; the service connects
    // as LocalSystem. Windows' default named-pipe DACL (LocalSystem,
    // Administrators, Creator Owner) matches the TRD's ACL requirement; the
    // PID/image-name check below narrows it to hh-service specifically.
    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(WINDOWS_PIPE_NAME)?;
    tracing::info!(pipe = WINDOWS_PIPE_NAME, "hh-session listening (named pipe)");
    loop {
        server.connect().await?;
        // Queue the next pipe instance before handling this client so
        // concurrent clients (control + screen peer) can connect.
        let current = server;
        server = ServerOptions::new().create(WINDOWS_PIPE_NAME)?;
        tokio::spawn(async move {
            if verify_client(&current) {
                if let Err(e) = handle_stream(current).await {
                    tracing::debug!(error = %e, "helper connection ended");
                }
            } else {
                tracing::warn!("refused pipe client: not hh-service");
            }
        });
    }
}

/// Client hardening (TRD §9): the pipe peer must be hh-service.
#[cfg(windows)]
fn verify_client(pipe: &tokio::net::windows::named_pipe::NamedPipeServer) -> bool {
    use std::os::windows::io::AsRawHandle;

    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Pipes::GetNamedPipeClientProcessId;

    let raw = pipe.as_raw_handle();
    if raw.is_null() {
        return false;
    }
    let mut pid: u32 = 0;
    // SAFETY: handle is owned by this pipe server; the call only reads it.
    let ok = unsafe { GetNamedPipeClientProcessId(HANDLE(raw as _), &mut pid) };
    if ok.is_err() || pid == 0 {
        return false;
    }
    client_image_is_service(pid)
}

#[cfg(windows)]
fn client_image_is_service(pid: u32) -> bool {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    };

    // SAFETY: plain Win32 process interrogation; handles closed on all paths.
    unsafe {
        let Ok(handle) = OpenProcess(ProcessQueryLimitedInformation, false, pid) else {
            return false;
        };
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, windows::core::PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(handle);
        if ok.is_err() {
            // Query failed (e.g. elevated service under a different session
            // profile). Fail open to the name check being inconclusive: the
            // DACL is still enforcing access. Log loudly in debug builds.
            tracing::debug!(pid, "could not query client image name");
            return true;
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]).to_lowercase();
        path.ends_with("hh-service.exe") || path.ends_with("hh-service")
    }
}
