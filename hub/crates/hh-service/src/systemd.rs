//! Linux systemd service lifecycle and notification socket integration.
#![cfg(target_os = "linux")]

use hh_core::platform::ServiceHost;
use hh_core::{Config, Error, Result};
use std::os::linux::net::SocketAddrExt;
use std::os::unix::net::{SocketAddr, UnixDatagram};
use std::time::Duration;

pub struct SystemdHost {
    config: Config,
}

impl SystemdHost {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}

fn notify(message: &str) -> Result<()> {
    let Some(path) = std::env::var_os("NOTIFY_SOCKET") else {
        return Ok(());
    };
    let bytes = std::os::unix::ffi::OsStrExt::as_bytes(path.as_os_str());
    let address = if bytes.first() == Some(&b'@') {
        SocketAddr::from_abstract_name(&bytes[1..])?
    } else {
        SocketAddr::from_pathname(std::path::Path::new(&path))?
    };
    UnixDatagram::unbound()?.send_to_addr(message.as_bytes(), &address)?;
    Ok(())
}

fn watchdog_interval() -> Option<Duration> {
    let pid = std::env::var("WATCHDOG_PID").ok();
    if pid
        .as_deref()
        .is_some_and(|value| value.parse::<u32>().ok() != Some(std::process::id()))
    {
        return None;
    }
    let micros = std::env::var("WATCHDOG_USEC").ok()?.parse::<u64>().ok()?;
    if micros == 0 {
        None
    } else {
        Some(Duration::from_micros((micros / 2).max(1)))
    }
}

impl ServiceHost for SystemdHost {
    fn run(self: Box<Self>) -> Result<()> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|error| Error::Internal(format!("systemd runtime: {error}")))?;
        runtime.block_on(async move {
            let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
            let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
            let signals = tokio::spawn(async move {
                let mut terminate =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
                tokio::select! {
                    _ = terminate.recv() => {},
                    _ = tokio::signal::ctrl_c() => {},
                }
                notify("STOPPING=1")?;
                let _ = stop_tx.send(());
                Ok::<(), Error>(())
            });
            let watchdog = watchdog_interval();
            let hub = super::run_hub(self.config, stop_rx, Some(ready_tx));
            tokio::pin!(hub);
            tokio::select! {
                result = &mut hub => { signals.abort(); result },
                ready = ready_rx => {
                    ready.map_err(|_| Error::Internal("hub exited before readiness".into()))?;
                    notify("READY=1")?;
                    let watchdog_task = watchdog.map(|interval| tokio::spawn(async move {
                        let mut timer = tokio::time::interval(interval);
                        loop {
                            timer.tick().await;
                            if let Err(error) = notify("WATCHDOG=1") {
                                tracing::warn!(%error, "systemd watchdog notification failed");
                            }
                        }
                    }));
                    let result = hub.await;
                    if let Some(task) = watchdog_task { task.abort(); }
                    signals.abort();
                    result
                }
            }
        })
    }
}
