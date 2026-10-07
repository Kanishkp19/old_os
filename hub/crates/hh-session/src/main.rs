//! hh-session: Home Hub per-user session helper (TRD §9).
//!
//! Windows services run in Session 0 and cannot inject input, capture the
//! desktop, or toggle the mobile hotspot. This helper runs in the interactive
//! user session and serves those capabilities to `hh-service` over the
//! newline-delimited JSON named-pipe protocol defined in
//! `hh_remote::helper_protocol` (`\\.\pipe\homehub-session`).
//!
//! Modes:
//! - `hh-session` (default, Windows) — named-pipe server + real input/power.
//! - `hh-session` on unix — builds for protocol tests; pipe mode needs
//!   Windows, use a unix build only via the test helpers.
//!
//! Verification status: protocol, dispatch and framing are tested
//! cross-platform (`cargo test -p hh-session`); SendInput/power/WMI/hotspot
//! paths compile on Windows CI and need a real Windows session to exercise.

use hh_remote::helper_protocol::{HelperRequest, HelperResponse};

pub mod dispatch;
pub mod enc;
pub mod server;

#[cfg(windows)]
pub mod platform_win;

#[cfg(feature = "screen")]
pub mod screen;
#[cfg(not(feature = "screen"))]
pub mod screen {
    //! Not compiled with the `screen` feature: same protocol surface, calls
    //! fail honestly instead of silently doing nothing.
    pub fn handle_offer(_sdp: &str, _preset: &str, _kind: &str) -> Result<serde_json::Value, String> {
        Err("screen host not compiled into this helper build".into())
    }
    pub fn add_ice(_candidate: &str) -> Result<(), String> {
        Err("screen host not compiled into this helper build".into())
    }
    pub fn stop() -> Result<(), String> {
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,hh_session=debug".into()),
        )
        .with_ansi(false)
        .init();

    let cli = Cli::parse();
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    rt.block_on(async move {
        match cli.mode.as_str() {
            "socket" => {
                #[cfg(unix)]
                {
                    server::serve_unix(&server::default_socket_path()).await
                }
                #[cfg(not(unix))]
                {
                    anyhow::bail!("socket mode requires a unix platform")
                }
            }
            "pipe" => {
                #[cfg(windows)]
                {
                    server::serve_windows().await
                }
                #[cfg(not(windows))]
                {
                    anyhow::bail!("named-pipe mode is Windows-only")
                }
            }
            other => anyhow::bail!("unknown mode {other:?} (expected pipe|socket)"),
        }
    })
}

#[derive(clap::Parser)]
#[command(name = "hh-session", about = "Home Hub user-session helper")]
struct Cli {
    /// pipe (Windows named pipe) or socket (unix dev/test socket).
    #[arg(long, default_value = "pipe")]
    mode: String,
}

use clap::Parser as _;

/// Execute one request against the platform implementations. Shared by both
/// transports so the wire behavior is identical.
pub fn execute(req: &HelperRequest) -> HelperResponse {
    dispatch::dispatch(req)
}

/// Convenience for responses with payload data (screen answer, system info…).
pub fn ok_data(data: serde_json::Value) -> HelperResponse {
    HelperResponse { ok: true, error: None, data: Some(data) }
}

pub fn err(msg: impl Into<String>) -> HelperResponse {
    HelperResponse { ok: false, error: Some(msg.into()), data: None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hh_remote::helper_protocol::HelperRequest;

    #[test]
    fn ping_dispatches() {
        let r = execute(&HelperRequest::Ping);
        assert!(r.ok);
        assert_eq!(r.data.as_ref().unwrap()["platform"], std::env::consts::OS);
    }

    #[test]
    fn unsupported_ops_fail_honestly_on_unix() {
        #[cfg(not(windows))]
        {
            let r = execute(&HelperRequest::MouseMove { dx: 10, dy: -3 });
            assert!(!r.ok, "mousemove on unix must fail honestly");
            assert!(r.error.unwrap().contains("not supported"));
        }
        #[cfg(windows)]
        {
            // On Windows the op executes for real; just check it apexes ok.
            let r = execute(&HelperRequest::MouseMove { dx: 0, dy: 0 });
            assert!(r.ok, "mousemove on Windows session should succeed: {:?}", r.error);
        }
    }

    #[test]
    fn screen_stub_reports_compile_state() {
        let r = execute(&HelperRequest::ScreenOffer {
            sdp: "v=0".into(),
            preset: "balanced".into(),
            kind: "view".into(),
        });
        #[cfg(feature = "screen")]
        assert!(r.ok, "screen feature build must accept offers");
        #[cfg(not(feature = "screen"))]
        assert!(!r.ok);
    }
}
