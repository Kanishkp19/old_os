//! Native shell process entry point. Compositor surfaces are wired by the Linux session.
use hh_shell::{apps, input_mode, ipc::ShellIpcClient, launcher::Launcher,
    status_bar::StatusBarState, window_adapter::{calculate_work_area, WindowManager}};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    #[cfg(unix)]
    if std::env::args().any(|arg| arg == "--show-home") {
        let socket = shell_socket()?;
        let sender = tokio::net::UnixDatagram::unbound()?;
        sender.send_to(b"home", socket).await?;
        return Ok(());
    }

    let mode = input_mode::detect();
    let apps = apps::load_app_manifest()?;
    let launcher = Launcher::new(apps);
    let mut status = StatusBarState::default();
    let windows = WindowManager::new(calculate_work_area(1366, 768));
    if let Ok(snapshot) = ShellIpcClient::new().request_shell_snapshot().await {
        status.update_from_snapshot(&snapshot);
    }
    tracing::info!(?mode, app_count = launcher.all_apps.len(),
        work_area_width = windows.work_area.width, hub_name = %status.hub_name,
        "owner shell initialized");

    #[cfg(unix)]
    {
        let path = shell_socket()?;
        if path.exists() {
            let probe = tokio::net::UnixDatagram::unbound()?;
            if probe.connect(&path).is_ok() {
                return Err("owner shell is already running".into());
            }
            std::fs::remove_file(&path)?;
        }
        let listener = tokio::net::UnixDatagram::bind(&path)?;
        let mut message = [0u8; 32];
        loop {
            tokio::select! {
                result = listener.recv(&mut message) => {
                    let length = result?;
                    if &message[..length] == b"home" {
                        tracing::info!("Home launcher requested");
                    }
                }
                result = tokio::signal::ctrl_c() => {
                    result?;
                    break;
                }
            }
        }
        std::fs::remove_file(path)?;
    }
    #[cfg(not(unix))]
    tokio::signal::ctrl_c().await?;
    Ok(())
}

#[cfg(unix)]
fn shell_socket() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").ok_or("XDG_RUNTIME_DIR is required")?;
    Ok(std::path::PathBuf::from(runtime).join("homehub-shell.sock"))
}
