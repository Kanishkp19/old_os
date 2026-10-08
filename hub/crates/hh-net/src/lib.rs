//! hh-net: network surface of the Hub.
//!
//! - Port 47800: paired-device API, TLS 1.3 + mTLS, default-deny scopes.
//! - Port 47801: loopback dashboard (never on LAN).
//! - Port 47802: pairing endpoint, token-gated, logically time-boxed.
//! - UDP 5353/47803: mDNS + broadcast fallback discovery.
//!
//! Source IPs are restricted to LAN ranges (SECURITY §7, threat T13).

pub mod dashboard;
pub mod admin;
pub mod mdns;
pub mod pair_server;
pub mod ratelimit;
pub mod relay;
pub mod routes;
pub mod sse;
pub mod tls;
pub mod update;

use std::net::IpAddr;
use std::sync::Arc;

use hh_auth::{HubIdentity, PairingManager, RevocationList};
use hh_core::Config;
use hh_db::Db;
use hh_hw::HwService;
use hh_photos::PhotoService;
use hh_remote::RemoteService;
use hh_storage::StorageService;
use hh_stream::StreamService;
use hh_transfer::TransferEngine;
use tokio::sync::broadcast;

use crate::sse::EventBus;

/// Shared state for all routes.
#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub cfg: Config,
    pub ca: Arc<HubIdentity>,
    pub pairing: Arc<PairingManager>,
    pub revocation: RevocationList,
    pub transfers: TransferEngine,
    pub storage: StorageService,
    pub photos: PhotoService,
    pub remote: RemoteService,
    pub hw: HwService,
    pub stream: StreamService,
    pub events: EventBus,
    pub rate_limiter: Arc<crate::ratelimit::RateLimiter>,
    pub screen_owners: Arc<std::sync::Mutex<std::collections::HashMap<String,String>>>,
    pub started_at_ms: i64,
    /// LAN addresses for QR payload `a=` hints (refreshed on network change).
    pub lan_addrs: Arc<std::sync::RwLock<Vec<String>>>,
    /// Live when a session helper transport was detected at startup (TRD §9):
    /// input/power actually work, and system/hotspot/BitLocker/screen ops can
    /// be forwarded. None ⇒ those endpoints answer with honest errors.
    pub helper: Option<Arc<hh_remote::helper_client::HelperClient>>,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        db: Db,
        cfg: Config,
        ca: Arc<HubIdentity>,
        pairing: Arc<PairingManager>,
        revocation: RevocationList,
        remote: RemoteService,
        helper: Option<Arc<hh_remote::helper_client::HelperClient>>,
    ) -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            transfers: TransferEngine::new(db.clone(), cfg.clone()),
            storage: StorageService::new(db.clone(), cfg.clone()),
            photos: PhotoService::new(db.clone(), cfg.clone()),
            hw: HwService::new(db.clone(), cfg.clone()),
            stream: match &helper {Some(h)=>StreamService::with_db(db.clone()).with_helper((**h).clone()),None=>StreamService::with_db(db.clone())},
            events: EventBus::new(tx),
            rate_limiter: Arc::new(crate::ratelimit::RateLimiter::default()),
            screen_owners: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            started_at_ms: hh_core::time::now_ms(),
            lan_addrs: Arc::new(std::sync::RwLock::new(vec![])),
            db,
            cfg,
            ca,
            pairing,
            revocation,
            remote,
            helper,
        }
    }
}

/// LAN-only source check (SECURITY §7): RFC1918, link-local, IPv6 ULA.
pub fn is_lan_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private() || v4.is_link_local() || v4.is_loopback()
        }
        IpAddr::V6(v6) => v6.is_loopback() || (v6.segments()[0] & 0xfe00) == 0xfc00,
    }
}

pub mod serve {
    //! Accept loops. The mTLS API uses a manual accept loop so the peer's
    //! device certificate can be resolved to a `PeerIdentity` once per
    //! connection and injected into every request's extensions.

    use hh_core::error::Result;
    use hh_core::{API_PORT, DASHBOARD_PORT, PAIRING_PORT};
    use hyper::body::Incoming;
    use hyper_util::rt::{TokioExecutor, TokioIo};
    use tokio::net::TcpListener;
    use tokio_rustls::TlsAcceptor;
    use tower::ServiceExt;

    /// axum's `Router::oneshot` accepts `Request<Body>`, not `Request<Incoming>`.
    async fn to_axum_body(req: hyper::Request<Incoming>) -> hyper::Request<axum::body::Body> {
        let (parts, body) = req.into_parts();
        hyper::Request::from_parts(parts, axum::body::Body::new(body))
    }

    use crate::routes::PeerIdentityExt;
    use crate::{is_lan_ip, AppState};

    /// Port 47800: mTLS API for paired devices.
    pub async fn serve_api(state: AppState, acceptor: TlsAcceptor) -> Result<()> {
        let listener = TcpListener::bind(("0.0.0.0", API_PORT)).await?;
        tracing::info!(port = API_PORT, "mTLS API listening");
        let connections=std::sync::Arc::new(tokio::sync::Semaphore::new(128));
        loop {
            let (tcp, peer) = match listener.accept().await {
                Ok(x) => x,
                Err(e) => {
                    tracing::warn!(error = %e, "accept failed");
                    continue;
                }
            };
            // SECURITY §7: refuse non-LAN sources (T13).
            if !is_lan_ip(peer.ip()) {
                tracing::warn!(ip = %peer.ip(), "refusing non-LAN connection");
                drop(tcp);
                continue;
            }
            let permit=match connections.clone().try_acquire_owned(){Ok(p)=>p,Err(_)=>{drop(tcp);continue;}};
            let acceptor = acceptor.clone();
            let state = state.clone();
            tokio::spawn(async move {
                let _permit=permit;
                let tls = match tokio::time::timeout(std::time::Duration::from_secs(10),acceptor.accept(tcp)).await {
                    Ok(result)=>match result {
                    Ok(t) => t,
                    Err(e) => {
                        tracing::warn!(error = ?e, "mTLS accept failed");
                        return;
                    }
                    },Err(_)=>return,
                };
                // Resolve device identity from the peer certificate.
                let identity = tls
                    .get_ref()
                    .1
                    .peer_certificates()
                    .and_then(|c| c.first())
                    .and_then(|der| crate::tls::parse_peer_cert(der.as_ref()).ok())
                    .and_then(|(device_id, serial)| {
                        state
                            .db
                            .device_by_id(&device_id)
                            .ok()
                            .flatten()
                            .filter(|d| d.status == "active" && state.db.device_accepts_serial(&device_id,&serial).unwrap_or(false))
                            .map(|d| crate::tls::PeerIdentity {
                                device_id,
                                cert_serial: serial,
                                scopes: d.scopes,
                            })
                    });
                let watch_identity=identity.clone();let watch_db=state.db.clone();
                let router = crate::routes::router(state.clone());
                let svc = hyper::service::service_fn(move |req: hyper::Request<Incoming>| {
                    let identity = identity.clone();
                    let router = router.clone();
                    async move {
                        let mut req = to_axum_body(req).await;
                        if let Some(id) = identity {
                            req.extensions_mut().insert(PeerIdentityExt(id));
                        }
                        router.oneshot(req).await
                    }
                });
                let io = TokioIo::new(tls);
                let builder=hyper_util::server::conn::auto::Builder::new(TokioExecutor::new());
                let connection=builder.serve_connection_with_upgrades(io,svc);
                tokio::select! {
                    _=connection=>{},
                    _=async move {loop {tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        let Some(peer)=&watch_identity else {break;};
                        if !watch_db.device_accepts_serial(&peer.device_id,&peer.cert_serial).unwrap_or(false) || watch_db.is_serial_revoked(&peer.cert_serial).unwrap_or(true) || watch_db.get_setting("sharing.paused").ok().flatten().as_deref()==Some("true"){break;}
                    }}=>{}
                }
            });
        }
    }

    /// Port 47802: pairing endpoint (TLS, no client cert).
    pub async fn serve_pairing(state: AppState, acceptor: TlsAcceptor) -> Result<()> {
        let listener = TcpListener::bind(("0.0.0.0", PAIRING_PORT)).await?;
        tracing::info!(port = PAIRING_PORT, "pairing endpoint listening");
        let connections=std::sync::Arc::new(tokio::sync::Semaphore::new(32));
        loop {
            let (tcp, peer) = match listener.accept().await {
                Ok(x) => x,
                Err(e) => {
                    tracing::warn!(error = %e, "pairing accept failed");
                    continue;
                }
            };
            if !is_lan_ip(peer.ip()) {
                drop(tcp);
                continue;
            }
            let permit=match connections.clone().try_acquire_owned(){Ok(p)=>p,Err(_)=>{drop(tcp);continue;}};
            let acceptor = acceptor.clone();
            let state = state.clone();
            tokio::spawn(async move {
                let _permit=permit;
                let tls = match tokio::time::timeout(std::time::Duration::from_secs(10),acceptor.accept(tcp)).await {
                    Ok(result)=>match result {
                    Ok(t) => t,
                    Err(_) => return,
                    },Err(_)=>return,
                };
                let router = crate::pair_server::router(state);
                let svc = hyper::service::service_fn(move |req: hyper::Request<Incoming>| {
                    let router = router.clone();
                    async move { router.oneshot(to_axum_body(req).await).await }
                });
                let io = TokioIo::new(tls);
                let _ = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
                    .serve_connection(io, svc)
                    .await;
            });
        }
    }

    /// Port 47801: loopback dashboard. NEVER bound on LAN interfaces.
    pub async fn serve_dashboard(state: AppState, token: String) -> Result<()> {
        let router = crate::dashboard::router(state, token);
        let listener = TcpListener::bind(("127.0.0.1", DASHBOARD_PORT)).await?;
        tracing::info!(port = DASHBOARD_PORT, "dashboard listening on loopback only");
        axum::serve(listener, router).await?;
        Ok(())
    }
}
