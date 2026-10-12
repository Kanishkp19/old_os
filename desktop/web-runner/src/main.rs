//! Minimal controller endpoint for the future sandboxed web process.
pub mod control;
pub mod downloads;

use control::{sanitize_origin, RunnerCommand, RunnerEvent};
use std::{collections::HashMap, path::PathBuf};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixListener,
};

const MAX_MESSAGE: usize = 4096;
const MAX_TABS: usize = 12;

struct Config {
    app_id: String,
    profile_dir: PathBuf,
    socket: PathBuf,
}

fn arguments() -> Result<Config, String> {
    let mut app_id = None;
    let mut profile_dir = None;
    let mut socket = None;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| format!("Missing value for {flag}"))?;
        match flag.as_str() {
            "--app-id" if app_id.is_none() => app_id = Some(value),
            "--profile-dir" if profile_dir.is_none() => profile_dir = Some(PathBuf::from(value)),
            "--socket" if socket.is_none() => socket = Some(PathBuf::from(value)),
            _ => return Err(format!("Unknown or duplicate option: {flag}")),
        }
    }
    let app_id = app_id.ok_or("Missing --app-id")?;
    if app_id != "browser" && app_id != "youtube" {
        return Err("--app-id must be browser or youtube".into());
    }
    Ok(Config {
        app_id,
        profile_dir: profile_dir.ok_or("Missing --profile-dir")?,
        socket: socket.ok_or("Missing --socket")?,
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let config = arguments()?;
    // Each app ID owns an independent child profile under the supplied root.
    let profile = config.profile_dir.join(&config.app_id);
    std::fs::create_dir_all(&profile)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&profile, std::fs::Permissions::from_mode(0o700))?;
    }
    let listener = UnixListener::bind(&config.socket)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&config.socket, std::fs::Permissions::from_mode(0o600))?;
    }
    let mut tabs = HashMap::<String, String>::new();
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            accepted = listener.accept() => {
                let (mut stream, _) = accepted?;
                let mut bytes = Vec::new();
                let read = tokio::time::timeout(std::time::Duration::from_secs(5), (&mut stream).take((MAX_MESSAGE + 1) as u64).read_to_end(&mut bytes)).await;
                let event = match read {
                    Ok(Ok(_)) if bytes.len() <= MAX_MESSAGE => match serde_json::from_slice::<RunnerCommand>(&bytes) {
                        Ok(RunnerCommand::Navigate { tab_id, url }) if !tab_id.is_empty() && tab_id.len() <= 128 && (tabs.contains_key(&tab_id) || tabs.len() < MAX_TABS) => {
                            match sanitize_origin(&url) {
                                Ok(origin) => { tabs.insert(tab_id.clone(), origin.clone()); RunnerEvent::TabNavigated { tab_id, title: origin.clone(), origin } }
                                Err(message) => RunnerEvent::Error { tab_id: Some(tab_id), message },
                            }
                        }
                        Ok(RunnerCommand::CloseTab { tab_id }) if tab_id.len() <= 128 => { tabs.remove(&tab_id); RunnerEvent::Error { tab_id: Some(tab_id), message: "Tab closed".into() } }
                        Ok(RunnerCommand::Shutdown) => break,
                        _ => RunnerEvent::Error { tab_id: None, message: "Invalid or over-limit command".into() },
                    },
                    _ => RunnerEvent::Error { tab_id: None, message: "Message too large or timed out".into() },
                };
                let reply = serde_json::to_vec(&event)?;
                stream.write_all(&reply).await?;
            }
        }
    }
    drop(listener);
    std::fs::remove_file(&config.socket)?;
    Ok(())
}
