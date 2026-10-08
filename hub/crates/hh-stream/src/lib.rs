//! hh-stream: screen sharing, Phase 3 (TRD §10, API_SPEC §9).
//!
//! Architecture: signaling over the existing mTLS API (SDP offer/answer +
//! trickle ICE), media over WebRTC (`webrtc-rs`) on the LAN — no STUN/TURN.
//! Capture/encode run in the per-user helper (Session 0 cannot capture the
//! desktop): native Windows capture → persistent H.264 encode → WebRTC.
//!
//! This crate owns the session lifecycle and drives a [`WebRtcPeer`] per
//! session: the answer SDP comes from the peer (munged to the negotiated
//! preset), ICE candidates are journaled for both sides, and every stop —
//! explicit, error, or Hub restart — runs the same cleanup path. The real
//! capture peer is provided by the helper process. The loopback mock is
//! compiled only for tests; an absent helper refuses sessions.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use hh_core::error::{Error, Result};
use hh_core::time::now_ms;
use hh_db::Db;
use rusqlite::params;
use serde::{Deserialize, Serialize};

/// Quality presets (TRD §10). Auto-selected from the hardware audit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Preset {
    Low,      // 720p30, 2 Mbps
    Balanced, // 1080p30, 6 Mbps
    High,     // 1080p60, 12 Mbps
}

impl Preset {
    pub fn bitrate_bps(self) -> u32 {
        match self {
            Preset::Low => 2_000_000,
            Preset::Balanced => 6_000_000,
            Preset::High => 12_000_000,
        }
    }

    /// Cap the answer SDP's declared bandwidth to the negotiated preset.
    fn apply_to_sdp(offer: &str, preset: Preset) -> String {
        offer
            .lines()
            .map(|line| {
                if line.starts_with("b=AS:") {
                    format!("b=AS:{}", preset.bitrate_bps() / 1000)
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\r\n")
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ScreenCapabilities {
    pub encoders: Vec<String>,
    pub max_preset: Preset,
    pub hw_encode: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScreenSession {
    pub id: String,
    pub device_id: String,
    pub kind: String, // view | cast
    pub preset: Preset,
    pub started_at: i64,
    pub last_activity_ms: i64,
    pub state: String,
}

/// Abstraction over a WebRTC peer (webrtc-rs in production, mock in tests).
pub trait WebRtcPeer: Send {
    fn configure(&mut self, _id: &str, _kind: &str, _preset: Preset) -> Result<()> { Ok(()) }
    fn set_remote_offer(&mut self, sdp: &str) -> Result<()>;
    fn create_answer(&mut self) -> Result<String>;
    fn add_ice_candidate(&mut self, candidate: &str) -> Result<()>;
    fn close(&mut self) -> Result<()>;
}

/// Screen capture source, implemented by the helper process (WGC/DXGI).
pub trait ScreenCapture: Send {
    fn start(&mut self, preset: Preset) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
}

/// Deterministic test peer. Never used as a production fallback.
#[cfg(test)]
#[derive(Default)]
pub struct LoopbackPeer {
    pub offer: Option<String>,
    pub candidates: Vec<String>,
    pub closed: bool,
}

#[cfg(test)]
impl WebRtcPeer for LoopbackPeer {
    fn set_remote_offer(&mut self, sdp: &str) -> Result<()> {
        self.offer = Some(sdp.to_string());
        Ok(())
    }

    fn create_answer(&mut self) -> Result<String> {
        let offer = self.offer.clone().ok_or_else(|| Error::Conflict("no offer before answer".into()))?;
        Ok(format!("v=0\r\no=homehub-loopback\r\n{offer}\r\n"))
    }

    fn add_ice_candidate(&mut self, candidate: &str) -> Result<()> {
        self.candidates.push(candidate.to_string());
        Ok(())
    }

    fn close(&mut self) -> Result<()> {
        self.closed = true;
        Ok(())
    }
}

/// Installed Windows helper peer. All commands carry the same screen id.
struct HelperPeer {
    helper: hh_remote::helper_client::HelperClient,
    id: String,
    kind: String,
    preset: Preset,
    offer: Option<String>,
    started: bool,
}
impl WebRtcPeer for HelperPeer {
    fn configure(&mut self, id: &str, kind: &str, preset: Preset) -> Result<()> {
        self.id = id.into(); self.kind = kind.into(); self.preset = preset; Ok(())
    }
    fn set_remote_offer(&mut self, sdp: &str) -> Result<()> { self.offer = Some(sdp.into()); Ok(()) }
    fn create_answer(&mut self) -> Result<String> {
        let offer = self.offer.as_deref().ok_or_else(|| Error::Conflict("screen offer required".into()))?;
        let preset = match self.preset { Preset::Low => "low", Preset::Balanced => "balanced", Preset::High => "high" };
        // A transport timeout can hide a successful start. Always attempt
        // exact-id cleanup even when no answer reached the caller.
        self.started = true;
        let answer = self.helper.screen_offer_for(&self.id, offer, preset, &self.kind)?;
        answer.get("answer_sdp").and_then(serde_json::Value::as_str).filter(|s| !s.is_empty()).map(str::to_owned)
            .ok_or_else(|| Error::Internal("session helper returned no screen answer".into()))
    }
    fn add_ice_candidate(&mut self, candidate: &str) -> Result<()> { self.helper.add_ice_for(&self.id, candidate) }
    fn close(&mut self) -> Result<()> {
        if self.started { self.started = false; self.helper.screen_stop_for(&self.id)?; }
        Ok(())
    }
}
impl Drop for HelperPeer { fn drop(&mut self) { let _ = self.close(); } }

#[derive(Default)]
struct State {
    sessions: HashMap<String, ActiveSession>,
}

struct ActiveSession {
    meta: ScreenSession,
    peer: Box<dyn WebRtcPeer>,
    ice: Vec<String>,
}

#[derive(Clone)]
pub struct StreamService {
    state: Arc<Mutex<State>>,
    db: Option<Db>,
    peer_factory: Option<Arc<dyn Fn() -> Box<dyn WebRtcPeer> + Send + Sync>>,
    peer_max_preset: Option<Preset>,
    helper: Option<hh_remote::helper_client::HelperClient>,
}

/// Max usable preset given the hardware audit ratings (TRD §11):
/// streaming "limited" → Low, "not_recommended" → refuse honestly.
pub fn max_preset_for_rating(rating: &str) -> Option<Preset> {
    match rating {
        "excellent" => Some(Preset::High),
        "good" => Some(Preset::Balanced),
        "limited" => Some(Preset::Low),
        _ => None, // UI shows "Limited on this laptop" (UI_UX §4.10)
    }
}

impl StreamService {
    pub fn new() -> Self {
        Self::default()
    }

    /// Production constructor: sessions persist to the hub DB.
    pub fn with_db(db: Db) -> Self {
        Self { db: Some(db), ..Self::default() }
    }

    /// Install the production peer factory (helper-process backed).
    pub fn with_peer_factory(mut self, factory: Arc<dyn Fn() -> Box<dyn WebRtcPeer> + Send + Sync>) -> Self {
        self.peer_factory = Some(factory);
        self
    }

    /// Install the real helper adapter. Software capture is capped at
    /// balanced regardless of CPU-only hardware-audit estimates.
    pub fn with_helper(mut self, helper: hh_remote::helper_client::HelperClient) -> Self {
        self.helper = Some(helper.clone());
        self.peer_factory = Some(Arc::new(move || Box::new(HelperPeer { helper: helper.clone(), id: String::new(), kind: String::new(), preset: Preset::Low, offer: None, started: false })));
        self.peer_max_preset = Some(Preset::Balanced);
        self
    }

    fn new_peer(&self) -> Result<Box<dyn WebRtcPeer>> {
        match &self.peer_factory {
            Some(f) => Ok(f()),
            None => Err(Error::Conflict("screen helper unavailable".into())),
        }
    }

    /// Offer → session + preset-negotiated answer SDP.
    pub fn start_session(
        &self,
        device_id: &str,
        kind: &str,
        requested: Option<Preset>,
        rating: &str,
        offer_sdp: &str,
    ) -> Result<(String, Preset, String)> {
        if !matches!(kind, "view" | "cast") || offer_sdp.is_empty() || offer_sdp.len() > 512 * 1024 { return Err(Error::BadRequest("invalid screen offer".into())); }
        let mut state = self.state.lock().map_err(|_| Error::Internal("stream lock poisoned".into()))?;
        if !state.sessions.is_empty() { return Err(Error::Conflict("a screen session is already active".into())); }
        let max = max_preset_for_rating(rating)
            .ok_or_else(|| Error::Conflict("screen sharing is limited on this laptop".into()))?;
        let max = self.peer_max_preset.map(|cap| max.min(cap)).unwrap_or(max);
        let preset = match requested {
            Some(p) if p <= max => p,
            _ => max,
        };
        let id = ulid::Ulid::new().to_string();
        let mut peer = self.new_peer()?;
        let negotiated = (|| {
            peer.configure(&id, kind, preset)?;
            peer.set_remote_offer(offer_sdp)?;
            peer.create_answer().map(|answer| Preset::apply_to_sdp(&answer, preset))
        })();
        let answer = match negotiated { Ok(answer) => answer, Err(e) => { let _ = peer.close(); return Err(e); } };

        let meta = ScreenSession {
            id: id.clone(),
            device_id: device_id.to_string(),
            kind: kind.to_string(),
            preset,
            started_at: now_ms(),
            last_activity_ms: now_ms(),
            state: "active".into(),
        };
        if let Err(e) = self.db_record_start(&meta) { let _ = peer.close(); return Err(e); }
        state.sessions.insert(id.clone(), ActiveSession { meta, peer, ice: vec![] });
        Ok((id, preset, answer))
    }

    /// Trickle ICE from the viewer (both sides journal for the peer impl).
    pub fn add_ice_candidate(&self, session_id: &str, candidate: &str) -> Result<()> {
        if candidate.len() > 8192 { return Err(Error::TooLarge("screen ICE candidate".into())); }
        let mut st = self
            .state
            .lock()
            .map_err(|_| Error::Internal("stream lock poisoned".into()))?;
        let s = st
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| Error::NotFound(format!("screen session {session_id}")))?;
        if s.ice.len() >= 128 { return Err(Error::RateLimited); }
        s.peer.add_ice_candidate(candidate)?;
        s.ice.push(candidate.to_string());
        Ok(())
    }

    pub fn ice_candidates(&self, session_id: &str) -> Result<Vec<String>> {
        let st = self
            .state
            .lock()
            .map_err(|_| Error::Internal("stream lock poisoned".into()))?;
        Ok(st
            .sessions
            .get(session_id)
            .map(|s| s.ice.clone())
            .unwrap_or_default())
    }

    /// The one cleanup path: closes the peer, drops the session, marks the
    /// DB row ended. Used by stop_session, restart recovery, and errors.
    pub fn stop_session(&self, id: &str) -> Result<()> {
        let session = {
            let mut st = self
                .state
                .lock()
                .map_err(|_| Error::Internal("stream lock poisoned".into()))?;
            st.sessions.remove(id)
        };
        let s = session.ok_or_else(|| Error::NotFound(format!("screen session {id}")))?;
        let mut peer = s.peer;
        let closed = peer.close();
        let recorded = self.db_record_end(id);
        closed.and(recorded)
    }

    /// Hub-restart recovery: any DB row still marked active is stale — the
    /// in-memory peer is gone. Mark ended.
    pub fn reconcile_on_startup(&self) -> Result<()> {
        if let Some(db) = &self.db {
            let c = db.lock()?;
            c.execute(
                "UPDATE remote_sessions SET ended_at=?1 WHERE ended_at IS NULL AND kind LIKE 'screen_%'",
                params![now_ms()],
            )
            .map_err(|e| Error::Db(e.to_string()))?;
        }
        Ok(())
    }

    /// Renew the media lease for its authenticated device.
    pub fn touch_session(&self, id: &str, device_id: &str) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| Error::Internal("stream lock poisoned".into()))?;
        let session = state.sessions.get_mut(id).ok_or_else(|| Error::NotFound("screen session".into()))?;
        if session.meta.device_id != device_id { return Err(Error::ForbiddenScope("remote".into())); }
        session.meta.last_activity_ms = now_ms();
        Ok(())
    }

    /// End helper peers whose media disconnected or whose cast window closed.
    /// Holding the lifecycle lock while querying prevents a stale helper
    /// snapshot from accidentally retiring a concurrently starting session.
    pub fn reconcile_closed_sessions(&self) -> Result<Vec<String>> {
        let Some(helper) = &self.helper else { return Ok(Vec::new()); };
        let removed = {
            let mut state = self.state.lock().map_err(|_| Error::Internal("stream lock poisoned".into()))?;
            if state.sessions.is_empty() { return Ok(Vec::new()); }
            let capabilities = helper.screen_capabilities()?;
            let active = capabilities.get("current_sessions").and_then(serde_json::Value::as_array).ok_or_else(|| Error::Internal("invalid helper session status".into()))?;
            if active.len() > 1 { return Err(Error::Internal("invalid helper session count".into())); }
            let closed: Vec<_> = state.sessions.keys().filter(|id| !active.iter().any(|v| v.as_str() == Some(id.as_str()))).cloned().collect();
            closed.into_iter().filter_map(|id| state.sessions.remove(&id).map(|session| (id, session))).collect::<Vec<_>>()
        };
        let mut closed = Vec::new(); let mut first_error = None;
        for (id, mut session) in removed {
            let ended = session.peer.close().and(self.db_record_end(&id));
            if let Err(error) = ended { if first_error.is_none() { first_error = Some(error); } }
            closed.push(id);
        }
        if let Some(error) = first_error { return Err(error); }
        Ok(closed)
    }

    /// Remove expired peers through the same exact-session cleanup path.
    pub fn expire_idle_sessions(&self, timeout_ms: i64) -> Result<Vec<String>> {
        if timeout_ms <= 0 { return Err(Error::BadRequest("invalid screen idle timeout".into())); }
        let now = now_ms();
        // Decide and remove while holding one lock, so a successful heartbeat
        // cannot race a stale expiration decision.
        let removed = {
            let mut state = self.state.lock().map_err(|_| Error::Internal("stream lock poisoned".into()))?;
            let ids: Vec<_> = state.sessions.values().filter(|s| now.saturating_sub(s.meta.last_activity_ms) >= timeout_ms).map(|s| s.meta.id.clone()).collect();
            ids.into_iter().filter_map(|id| state.sessions.remove(&id).map(|session| (id, session))).collect::<Vec<_>>()
        };
        let mut expired = Vec::new(); let mut first_error = None;
        for (id, mut session) in removed {
            let closed = session.peer.close(); let recorded = self.db_record_end(&id);
            if let Err(error) = closed.and(recorded) { if first_error.is_none() { first_error = Some(error); } }
            expired.push(id);
        }
        if let Some(error) = first_error { return Err(error); }
        Ok(expired)
    }

    pub fn active_sessions(&self) -> Vec<ScreenSession> {
        self.state
            .lock()
            .map(|s| s.sessions.values().map(|a| a.meta.clone()).collect())
            .unwrap_or_default()
    }

    fn db_record_start(&self, meta: &ScreenSession) -> Result<()> {
        if let Some(db) = &self.db {
            // Screen sessions live in remote_sessions (BACKEND_SCHEMA §8),
            // which uses the screen_view / screen_cast kind vocabulary.
            let kind = match meta.kind.as_str() {
                "cast" => "screen_cast",
                _ => "screen_view",
            };
            let c = db.lock()?;
            c.execute(
                "INSERT OR IGNORE INTO remote_sessions (id, device_id, kind, started_at)
                 VALUES (?1,?2,?3,?4)",
                params![meta.id, meta.device_id, kind, meta.started_at],
            )
            .map_err(|e| Error::Db(e.to_string()))?;
        }
        Ok(())
    }

    fn db_record_end(&self, id: &str) -> Result<()> {
        if let Some(db) = &self.db {
            let c = db.lock()?;
            c.execute(
                "UPDATE remote_sessions SET ended_at=?2 WHERE id=?1",
                params![id, now_ms()],
            )
            .map_err(|e| Error::Db(e.to_string()))?;
        }
        Ok(())
    }
}

impl Default for StreamService {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(State::default())),
            db: None,
            peer_factory: None,
            peer_max_preset: None,
            helper: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingPeer {
        closes: Arc<AtomicUsize>,
        has_offer: bool,
    }

    impl WebRtcPeer for CountingPeer {
        fn set_remote_offer(&mut self, _sdp: &str) -> Result<()> {
            self.has_offer = true;
            Ok(())
        }
        fn create_answer(&mut self) -> Result<String> {
            if !self.has_offer {
                return Err(Error::Conflict("no offer".into()));
            }
            Ok("v=0\r\nb=AS:9999\r\n".into())
        }
        fn add_ice_candidate(&mut self, _candidate: &str) -> Result<()> {
            Ok(())
        }
        fn close(&mut self) -> Result<()> {
            self.closes.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    fn counting_service() -> (StreamService, Arc<AtomicUsize>) {
        let closes = Arc::new(AtomicUsize::new(0));
        let closes_for_factory = closes.clone();
        let svc = StreamService::default().with_peer_factory(Arc::new(move || {
            Box::new(CountingPeer { closes: closes_for_factory.clone(), has_offer: false })
                as Box<dyn WebRtcPeer>
        }));
        (svc, closes)
    }

    #[test]
    fn production_without_helper_refuses_screen_support() {
        assert!(StreamService::new().start_session("d1", "view", None, "good", "v=0").is_err());
    }

    #[test]
    fn preset_gating() {
        assert!(matches!(max_preset_for_rating("excellent"), Some(Preset::High)));
        assert!(matches!(max_preset_for_rating("limited"), Some(Preset::Low)));
        assert!(max_preset_for_rating("not_recommended").is_none());
    }

    #[test]
    fn session_lifecycle_and_answer_sdp() {
        let (s, closes) = counting_service();
        // Requested High on a "good" laptop caps to Balanced; answer BW set.
        let (id, preset, answer) =
            s.start_session("d1", "view", Some(Preset::High), "good", "v=0\r\nb=AS:9999\r\n").unwrap();
        assert!(matches!(preset, Preset::Balanced));
        assert!(answer.contains("b=AS:6000"));
        assert_eq!(s.active_sessions().len(), 1);

        // ICE journaling round-trips.
        s.add_ice_candidate(&id, "candidate:1 udp ...").unwrap();
        assert_eq!(s.ice_candidates(&id).unwrap(), vec!["candidate:1 udp ..."]);

        s.stop_session(&id).unwrap();
        assert!(s.active_sessions().is_empty());
        assert_eq!(closes.load(Ordering::SeqCst), 1, "peer must be closed on stop");
        assert!(matches!(s.stop_session(&id), Err(Error::NotFound(_))));
    }

    #[test]
    fn stop_is_exactly_once_even_if_peer_close_errors() {
        struct BoomPeer;
        impl WebRtcPeer for BoomPeer {
            fn set_remote_offer(&mut self, _: &str) -> Result<()> { Ok(()) }
            fn create_answer(&mut self) -> Result<String> { Ok("v=0".into()) }
            fn add_ice_candidate(&mut self, _: &str) -> Result<()> { Ok(()) }
            fn close(&mut self) -> Result<()> { Err(Error::Internal("boom".into())) }
        }
        let svc = StreamService::default()
            .with_peer_factory(Arc::new(|| Box::new(BoomPeer) as Box<dyn WebRtcPeer>));
        let (id, _, _) = svc.start_session("d1", "view", None, "good", "v=0").unwrap();
        // Session is removed from the map even though peer.close() failed.
        assert!(svc.stop_session(&id).is_err());
        assert!(svc.active_sessions().is_empty());
    }

    #[test]
    fn ownership_and_heartbeat_guard_lifecycle() {
        let (svc, closes) = counting_service();
        let (id, _, _) = svc.start_session("d1", "cast", None, "good", "v=0").unwrap();
        assert!(svc.touch_session(&id, "d2").is_err());
        assert!(svc.start_session("d2", "view", None, "good", "v=0").is_err());
        assert_eq!(closes.load(Ordering::SeqCst), 0);
        svc.state.lock().unwrap().sessions.get_mut(&id).unwrap().meta.last_activity_ms = now_ms() - 60_000;
        svc.touch_session(&id, "d1").unwrap();
        assert!(svc.expire_idle_sessions(45_000).unwrap().is_empty());
        svc.state.lock().unwrap().sessions.get_mut(&id).unwrap().meta.last_activity_ms = now_ms() - 60_000;
        assert_eq!(svc.expire_idle_sessions(45_000).unwrap(), vec![id]);
        assert_eq!(closes.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn rejects_when_rating_says_no() {
        let s = StreamService::new();
        let err = s.start_session("d1", "view", None, "not_recommended", "v=0").unwrap_err();
        assert!(matches!(err, Error::Conflict(_)));
    }

    #[test]
    fn db_persistence_and_restart_recovery() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = hh_core::Config {
            hub_name: "Test".into(),
            data_dir: tmp.path().join("data"),
            library_root: tmp.path().join("lib"),
            log_dir: tmp.path().join("logs"),
            second_copy_root: None,
            features: Default::default(),
        };
        cfg.ensure_dirs().unwrap();
        let db = hh_db::Db::open(&cfg.db_path()).unwrap();
        // remote_sessions.device_id FKs to devices(id).
        db.insert_device(
            &hh_db::DeviceRow {
                id: "d1".into(),
                name: "Phone".into(),
                platform: "android".into(),
                model: None,
                app_version: None,
                cert_serial: "s".into(),
                cert_expires_at: 0,
                scopes: vec![],
                paired_at: 0,
                last_seen_at: None,
                status: "active".into(),
            },
            "PEM",
            "PUB",
        )
        .unwrap();
        let s = StreamService::with_db(db.clone()).with_peer_factory(Arc::new(|| Box::new(LoopbackPeer::default())));

        let (id, _, _) = s.start_session("d1", "view", None, "good", "v=0").unwrap();
        let open_count: i64 = db
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM remote_sessions WHERE ended_at IS NULL", [], |r| r.get::<_, i64>(0))
            .unwrap();
        assert_eq!(open_count, 1);

        s.stop_session(&id).unwrap();
        let open_after_stop: i64 = db
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM remote_sessions WHERE ended_at IS NULL", [], |r| r.get::<_, i64>(0))
            .unwrap();
        assert_eq!(open_after_stop, 0);

        // Restart recovery clears stale open screen-session rows.
        s.start_session("d1", "cast", None, "good", "v=0").unwrap();
        s.reconcile_on_startup().unwrap();
        let open_after_reconcile: i64 = db
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM remote_sessions WHERE ended_at IS NULL", [], |r| r.get::<_, i64>(0))
            .unwrap();
        assert_eq!(open_after_reconcile, 0);
    }
}
