//! hh-service: Home Hub service entry point.
//!
//! Modes:
//! - `hh-service --console` — run as a normal process (development, M0 exit).
//! - `hh-service` (no args, started by SCM) — run as a Windows service with
//!   auto-start and recovery actions (FR-1.1); see `windows_service` module.
//!
//! Both modes share one runtime ([`run_hub`]): DB + CA + three listeners +
//! background workers. The mode only decides where the stop signal comes
//! from (Ctrl+C in console, SCM STOP/SHUTDOWN control as a service).
//!
//! Background workers (all throttled, idle-priority; see AGENTS.md §6):
//! - thumbnail pipeline, integrity scrub, SMART poll (6 h), transfer GC,
//!   trash retention GC, free-space alerts.

mod logging;
mod local_security;
use std::sync::Arc;

use clap::Parser;
use hh_auth::{HubIdentity, PairingManager, RevocationList};
use hh_core::platform::noop;
use hh_core::{Config, Result};
use hh_db::Db;
use hh_net::AppState;
use hh_remote::RemoteService;

#[derive(Parser)]
#[command(name = "hh-service", about = "Home Hub background service")]
struct Args {
    /// Run in the foreground as a console app (development mode).
    #[arg(long)]
    console: bool,
    #[arg(long)]
    backup_database:Option<std::path::PathBuf>,
    #[arg(long)]
    wake_only:bool,
    /// Override data dir (default %ProgramData%\HomeHub).
    #[arg(long)]
    data_dir: Option<String>,
    /// Override library root.
    #[arg(long)]
    library_root: Option<String>,
}

fn main() -> Result<()> {
    // Both rustls backends end up in the feature graph (ring here, aws-lc-rs
    // via reqwest in hh-tools-style deps), so the process default must be
    // pinned explicitly before any TLS config is built.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let args = Args::parse();
    if args.wake_only{return Ok(());}
    let mut defaults = Config::default();
    if let Some(d)=&args.data_dir { defaults.data_dir=d.into();defaults.log_dir=defaults.data_dir.join("logs"); }
    if let Some(destination)=&args.backup_database {return Db::snapshot(&defaults.db_path(),destination);}
    let config_path=defaults.data_dir.join("config.json");
    let mut cfg=Config::load_or_create(&config_path,defaults)?;
    if let Some(d)=args.data_dir {cfg.data_dir=d.into();}
    if let Some(l)=args.library_root {cfg.library_root=l.into();}
    cfg.ensure_dirs()?;
    init_logging(&cfg);

    #[cfg(windows)]
    {
        if !args.console {
            return windows_service::run_as_service(cfg);
        }
    }

    run_console(cfg)
}

fn init_logging(cfg: &Config) {
    let writer=logging::BoundedLog::new(&cfg.log_dir);
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,hh_=debug".into()),
        )
        .with_writer(move||writer.clone())
        .with_ansi(false)
        .init();
    // NOTE: never log tokens, keys, or file contents (AGENTS.md §2.7).

}

/// Console mode: Ctrl+C is the stop signal.
#[tokio::main]
async fn run_console(cfg: Config) -> Result<()> {
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = stop_tx.send(());
    });
    run_hub(cfg, stop_rx).await
}

/// Shared runtime: everything from DB open to the three listeners, with the
/// stop signal injected so console and SCM service modes behave identically.
async fn run_hub(cfg: Config, mut stop: tokio::sync::oneshot::Receiver<()>) -> Result<()> {
    tracing::info!(version = hh_core::HUB_VERSION, "home hub starting");

    let db = Db::open(&cfg.db_path())?;
    let cfg=hh_storage::StorageService::new(db.clone(),cfg).recover_library_moves()?;
    cfg.ensure_dirs()?;
    let (hub_id, hub_name) = db.hub_identity()?;
    if hub_name == "Home Hub" {
        db.set_hub_name(&cfg.hub_name)?;
    }

    let ca = HubIdentity::load_or_create(&cfg.data_dir, &cfg.hub_name)?;
    let revocation = RevocationList::load(db.clone())?;
    let pairing = Arc::new(PairingManager::new(db.clone()));

    // Platform implementations: Windows forwards input/power through the
    // per-user hh-session helper (Session 0 isolation, TRD §9); unix dev
    // builds auto-detect a `hh-session --mode socket` dev helper and keep
    // honest no-ops when neither is present.
    let helper = hh_remote::helper_client::HelperClient::detect().map(Arc::new);
    let (input, power): (Arc<dyn hh_core::platform::InputControl>, Arc<dyn hh_core::platform::PowerControl>) =
        match &helper {
            Some(h) => {
                match h.ping() {
                    Ok(v) => tracing::info!(
                        transport = h.transport_label(),
                        helper = %v.get("helper").and_then(|x| x.as_str()).unwrap_or("?"),
                        "session helper detected: input/power active"
                    ),
                    Err(e) => tracing::warn!(error = %e, "helper transport present but not answering; falling back to no-ops"),
                }
                (h.clone(), h.clone())
            }
            None => {
                tracing::info!("no session helper detected; input/power will answer as unsupported");
                (Arc::new(noop::Unsupported), Arc::new(noop::Unsupported))
            }
        };
    let remote = RemoteService::new(db.clone(), input, power, cfg.features.remote);

    let state = AppState::new(db.clone(), cfg.clone(), ca.clone(), pairing, revocation, remote, helper);

    // Startup reconciliation: orphan .part cleanup (AGENTS.md §6).
    state.transfers.reconcile_on_startup()?;
    state.storage.recover_storage_jobs()?;

    // LAN addresses for QR payload hints.
    if let Ok(net) = state.hw.network_info() {
        let addrs: Vec<String> = net
            .interfaces
            .iter()
            .flat_map(|i| i.ips.iter())
            .filter(|ip| ip.parse::<std::net::IpAddr>().map(hh_net::is_lan_ip).unwrap_or(false))
            .map(|ip| format!("{ip}:{}", hh_core::PAIRING_PORT))
            .collect();
        if let Ok(mut w) = state.lan_addrs.write() {
            *w = addrs;
        }
    }

    // Discovery: mDNS + broadcast fallback (NW-01/02).
    let fp = ca.fingerprint()?;
    let (_hub_id2, hub_name2) = db.hub_identity()?;
    let _mdns = hh_net::mdns::MdnsAdvertisement::start(&hub_id, &hub_name2, &fp, false).ok();
    tokio::spawn(hh_net::mdns::broadcast_responder(hub_id.clone(), hub_name2.clone(), fp));

    // TLS configs.
    let mtls_cfg = hh_net::tls::server_config_mtls(&ca, state.revocation.clone())?;
    let pair_cfg = hh_net::tls::server_config_pairing(&ca)?;

    // Background workers.
    spawn_workers(state.clone());

    // Servers.
    let api = hh_net::serve::serve_api(state.clone(), tokio_rustls::TlsAcceptor::from(mtls_cfg));
    let pairing_srv = hh_net::serve::serve_pairing(state.clone(), tokio_rustls::TlsAcceptor::from(pair_cfg));
    let dash_token = hh_net::dashboard::new_token();
    // Persist for same-user local clients (tray, scripts). Loopback-only
    // listener + user-profile ACL keep this a local secret.
    let token_path=cfg.data_dir.join("dashboard_token");
    {
        use std::io::Write;
        let mut options=std::fs::OpenOptions::new();options.write(true).create(true).truncate(true);
        #[cfg(unix)] {use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
        let mut file=options.open(&token_path)?;local_security::protect_admin_token(&cfg.data_dir,&token_path)?;file.write_all(dash_token.as_bytes())?;file.sync_all()?;
        #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(&token_path,std::fs::Permissions::from_mode(0o600))?;}
    }
    let dashboard = hh_net::serve::serve_dashboard(state.clone(), dash_token);

    tracing::info!("hub ready: api :47800, dashboard http://127.0.0.1:47801, pairing :47802");
    tokio::select! {
        r = api => { tracing::error!("api server exited: {r:?}"); }
        r = pairing_srv => { tracing::error!("pairing server exited: {r:?}"); }
        r = dashboard => { tracing::error!("dashboard exited: {r:?}"); }
        _ = &mut stop => {
            state.events.emit("hub.shutting_down", serde_json::json!({}));
            tracing::info!("shutting down");
        }
    }
    Ok(())
}

fn spawn_workers(state: AppState) {
    {let s=state.clone();tokio::spawn(async move {loop {tokio::time::sleep(std::time::Duration::from_secs(1)).await;let service=s.stream.clone();let db=s.db.clone();let expired=tokio::task::spawn_blocking(move||{
        for session in service.active_sessions(){let allowed=db.device_by_id(&session.device_id).ok().flatten().is_some_and(|d|d.status=="active"&&d.has_scope("remote"))&&db.get_setting("sharing.paused").ok().flatten().as_deref()!=Some("true");if !allowed{let _=service.stop_session(&session.id);}}
        let mut closed=service.reconcile_closed_sessions()?;closed.extend(service.expire_idle_sessions(45_000)?);Ok::<_,hh_core::Error>(closed)
    }).await;if let Ok(Ok(ids))=expired {if let Ok(mut owners)=s.screen_owners.lock(){for id in ids {owners.remove(&id);}}}}});}

    {
        let s=state.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(300)).await;
                let interval=s.db.get_setting("second_copy.interval_minutes").ok().flatten().and_then(|v|v.parse::<i64>().ok()).unwrap_or(1440).clamp(1,43200)*60*1000;
                let configured=s.db.get_setting("second_copy.root").ok().flatten().is_some_and(|v|!v.is_empty()) || s.cfg.second_copy_root.is_some();
                let due=s.storage.last_second_copy_age_ms().ok().flatten().map(|age|age>=interval).unwrap_or(true);
                if configured && due && s.db.list_transfers(None,Some("open")).map(|v|v.is_empty()).unwrap_or(false) {
                    let storage=s.storage.clone();
                    match tokio::task::spawn_blocking(move||storage.start_maintenance("second_copy")).await {
                        Ok(Ok(_))=>{},Ok(Err(error))=>tracing::warn!(%error,"scheduled second copy failed"),Err(error)=>tracing::warn!(%error,"second copy worker failed"),
                    }
                }
            }
        });
    }
    // Thumbnail worker: idle-priority, small batches (TRD §3).
    {
        let s = state.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                let photos = s.photos.clone();
                let _ = tokio::task::spawn_blocking(move || photos.process_thumb_queue(4)).await;
            }
        });
    }
    // Integrity scrub: 1–2% of library nightly (TRD §7.4); here a small
    // batch hourly — production schedules nightly at idle.
    {
        let s = state.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(300)).await;
                let hours=s.db.get_setting("scrub.interval_hours").ok().flatten().and_then(|v|v.parse::<i64>().ok()).unwrap_or(24).clamp(1,720);
                let now=hh_core::time::now_ms();let last=s.db.get_setting("scrub.last_scheduled").ok().flatten().and_then(|v|v.parse::<i64>().ok()).unwrap_or(0);
                if now-last>=hours*3600*1000&&s.db.list_transfers(None,Some("open")).map(|v|v.is_empty()).unwrap_or(false){
                    let storage=s.storage.clone();if matches!(tokio::task::spawn_blocking(move||storage.start_scheduled_scrub()).await,Ok(Ok(_))){let _=s.db.set_setting("scrub.last_scheduled",&now.to_string());}
                }
            }
        });
    }
    // SMART poll every 6 h + on boot (TRD §7.4).
    {
        let s = state.clone();
        tokio::spawn(async move {
            loop {
                let storage = s.storage.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    storage.collect_health(&hh_storage::health::NativeDiskHealth)
                })
                .await;
                let _ = s.storage.check_free_space_alerts();
                tokio::time::sleep(std::time::Duration::from_secs(6 * 3600)).await;
            }
        });
    }
    // Transfer GC + trash retention (BACKEND_SCHEMA §11).
    {
        let s = state.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
                let transfers=s.transfers.clone();
                if let Err(error)=tokio::task::spawn_blocking(move||transfers.reconcile_on_startup()).await {tracing::warn!(%error,"transfer reconciliation worker failed");}
                let storage=s.storage.clone();
                if let Err(error)=tokio::task::spawn_blocking(move||storage.purge_expired_trash()).await {tracing::warn!(%error,"trash retention worker failed");}
            }
        });
    }
}

#[cfg(windows)]
mod windows_service {
    //! Windows Service Control Manager integration (FR-1.1).
    //! Install: `hh-service.exe install` via the MSI; recovery: restart on
    //! failure (set by the installer with `sc failure`).

    use std::time::Duration;

    use hh_core::{Config, Result};
    use windows_service::service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    };
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};

    pub fn run_as_service(cfg: Config) -> Result<()> {
        // service_dispatcher::start blocks and invokes service_main on another
        // thread with no arguments, so the config travels via a static.
        let _ = SERVICE_CFG.set(cfg);
        windows_service::service_dispatcher::start("HomeHub", ffi_service_main)
            .map_err(|e| hh_core::Error::Internal(format!("service dispatcher: {e}")))?;
        Ok(())
    }

    static SERVICE_CFG: std::sync::OnceLock<Config> = std::sync::OnceLock::new();

    windows_service::define_windows_service!(ffi_service_main, service_main);

    fn service_main(_arguments: Vec<std::ffi::OsString>) {
        if let Err(e) = run_service() {
            eprintln!("HomeHub service failed: {e}");
        }
    }

    fn run_service() -> std::result::Result<(), String> {
        // The SCM stop/shutdown controls resolve the shared runtime's stop
        // signal, exactly like Ctrl+C in console mode.
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
        let stop_tx=std::sync::Mutex::new(Some(stop_tx));
        let handler = move |control: ServiceControl| -> ServiceControlHandlerResult {
            match control {
                ServiceControl::Stop | ServiceControl::Shutdown => {
                    if let Ok(mut tx)=stop_tx.lock(){if let Some(tx)=tx.take(){let _=tx.send(());}}
                    ServiceControlHandlerResult::NoError
                }
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        };
        let status_handle = service_control_handler::register("HomeHub", handler)
            .map_err(|e| e.to_string())?;

        let report = |state: ServiceState, controls: ServiceControlAccept, exit: u32, checkpoint: u32| {
            status_handle
                .set_service_status(ServiceStatus {
                    service_type: ServiceType::OWN_PROCESS,
                    current_state: state,
                    controls_accepted: controls,
                    exit_code: ServiceExitCode::Win32(exit),
                    checkpoint,
                    wait_hint: Duration::from_secs(30),
                    process_id: None,
                })
                .map_err(|e| e.to_string())
        };

        // START_PENDING → RUNNING → (run_hub) → STOPPED.
        report(ServiceState::StartPending, ServiceControlAccept::empty(), 0, 0)?;
        // Logging and data dirs were initialized in main() before dispatch.
        let cfg = SERVICE_CFG.get().cloned().unwrap_or_default();

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("tokio runtime: {e}"))?;
        report(ServiceState::Running, ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN, 0, 0)?;
        let result = rt.block_on(crate::run_hub(cfg, stop_rx));
        report(ServiceState::Stopped, ServiceControlAccept::empty(), 0, 0)?;
        result.map_err(|e| e.to_string())
    }
}
