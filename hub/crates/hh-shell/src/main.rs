//! Native shell process entry point. Compositor surfaces are wired by the Linux session.
use hh_shell::{apps, input_mode, ipc::ShellIpcClient, launcher::Launcher,
    notifications::NotificationCenter, oobe::OobeState, quick_settings::QuickSettingsState,
    status_bar::StatusBarState, window_adapter::{calculate_work_area, WindowManager}};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    #[cfg(unix)]
    if let Some(message) = std::env::args().find_map(|arg| match arg.as_str() {
        "--show-home" => Some(&b"home"[..]),
        "--quick-settings" => Some(&b"quick-settings"[..]),
        "--notifications" => Some(&b"notifications"[..]),
        _ => None,
    }) {
        let socket = shell_socket()?;
        let sender = tokio::net::UnixDatagram::unbound()?;
        sender.send_to(message, socket).await?;
        return Ok(());
    }

    let mode = input_mode::detect();
    let apps = apps::load_app_manifest()?;
    let launcher = Launcher::new(apps);
    let mut status = StatusBarState::default();
    let mut notifications = NotificationCenter::default();
    let mut quick_settings = QuickSettingsState::default();
    let checkpoint = input_mode::owner_preferences_path().with_file_name("oobe.json");
    let oobe = OobeState::load_checkpoint(&checkpoint)?.unwrap_or_default();
    let windows = WindowManager::new(calculate_work_area(1366, 768));
    if let Ok(snapshot) = ShellIpcClient::new().request_shell_snapshot().await {
        status.update_from_snapshot(&snapshot);
    }
    tracing::info!(?mode, app_count = launcher.all_apps.len(),
        work_area_width = windows.work_area.width, hub_name = %status.hub_name,
        oobe_completed = oobe.completed,
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
        let mut banner_timer = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            tokio::select! {
                _ = banner_timer.tick() => notifications.expire_banners(),
                result = listener.recv(&mut message) => {
                    let length = result?;
                    if &message[..length] == b"home" {
                        tracing::info!("Home launcher requested");
                    } else if &message[..length] == b"quick-settings" {
                        quick_settings.is_open = true;
                        tracing::info!("Quick settings requested");
                    } else if &message[..length] == b"notifications" {
                        match ShellIpcClient::new().request_notifications_list().await {
                            Ok(records) => {
                                for record in records { notifications.post_notification(record); }
                                tracing::info!(count = notifications.history.len(), "Notifications requested");
                            }
                            Err(error) => tracing::warn!(%error, "Could not load notifications"),
                        }
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
