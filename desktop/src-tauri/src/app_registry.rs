use crate::store::Result;
use tauri::WebviewWindow;

pub const TRUSTED_WINDOWS: &[&str] = &[
    "main", "photos", "files", "browser", "youtube", "music", "notes", "calculator",
    "devices", "settings",
];

fn is_authorized(label: &str, url: &url::Url) -> bool {
    let is_local = (url.scheme() == "tauri" && url.host_str() == Some("localhost"))
        || (matches!(url.scheme(), "http" | "https")
            && url.host_str() == Some("tauri.localhost"))
        || (cfg!(debug_assertions)
            && url.scheme() == "http"
            && url.host_str() == Some("127.0.0.1")
            && url.port() == Some(1420));
    !label.starts_with("browser-") && is_local && TRUSTED_WINDOWS.contains(&label)
}

pub fn verify_caller(window: &WebviewWindow, _command: &str) -> Result<()> {
    let url = window.url().map_err(|_| "Cannot identify app window")?;
    if !is_authorized(window.label(), &url) {
        return Err("This command is only available in Home Hub authorized apps".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::is_authorized;

    #[test]
    fn accepts_only_trusted_labels_and_local_origins() {
        for origin in ["tauri://localhost/", "http://tauri.localhost/", "https://tauri.localhost/"] {
            assert!(is_authorized("main", &origin.parse().unwrap()));
            assert!(is_authorized("photos", &origin.parse().unwrap()));
        }
        let debug = "http://127.0.0.1:1420/".parse().unwrap();
        assert_eq!(is_authorized("files", &debug), cfg!(debug_assertions));
        for origin in ["https://example.com/", "http://127.0.0.1:1421/", "http://tauri.localhost.evil/"] {
            assert!(!is_authorized("main", &origin.parse().unwrap()));
        }
        let local = "tauri://localhost/".parse().unwrap();
        for label in ["browser-1", "unknown", "Main"] {
            assert!(!is_authorized(label, &local));
        }
    }
}
