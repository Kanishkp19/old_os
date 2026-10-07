//! hh-stream: screen sharing, Phase 3 (TRD §10, API_SPEC §9).
//!
//! Architecture: signaling over the existing mTLS API (SDP offer/answer +
//! trickle ICE), media over WebRTC (`webrtc-rs`) on the LAN — no STUN/TURN.
//! Capture/encode run in the per-user helper (Session 0 cannot capture the
//! desktop): Windows Graphics Capture → Media Foundation HW encode → H.264.
//!
//! This crate owns the session lifecycle and drives a [`WebRtcPeer`] per
//! session: the answer SDP comes from the peer (munged to the negotiated
//! preset), ICE candidates are journaled for both sides, and every stop —
//! explicit, error, or Hub restart — runs the same cleanup path. The real
//! capture peer is provided by the helper process; a deterministic loopback
//! mock covers tests and CI.

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
            .join("\n")
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
}

/// Abstraction over a WebRTC peer (webrtc-rs in production, mock in tests).
pub trait WebRtcPeer: Send {
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

/// Loopback peer used for tests/CI and for "Limited" fallback without the
/// helper: mirrors the offer into an answer with the preset's bandwidth cap
/// and journals ICE candidates it accepts.
#[derive(Default)]
pub struct LoopbackPeer {
    pub offer: Option<String>,
    pub candidates: Vec<String>,
    pub closed: bool,
}

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

    fn new_peer(&self) -> Box<dyn WebRtcPeer> {
        match &self.peer_factory {
            Some(f) => f(),
            None => Box::new(LoopbackPeer::default()),
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
        let max = max_preset_for_rating(rating)
            .ok_or_else(|| Error::Conflict("screen sharing is limited on this laptop".into()))?;
        let preset = match requested {
            Some(p) if p <= max => p,
            _ => max,
        };
        let id = ulid::Ulid::new().to_string();
        let mut peer = self.new_peer();
        peer.set_remote_offer(offer_sdp)?;
        let answer = Preset::apply_to_sdp(&peer.create_answer()?, preset);

        let meta = ScreenSession {
            id: id.clone(),
            device_id: device_id.to_string(),
            kind: kind.to_string(),
            preset,
            started_at: now_ms(),
        };
        self.db_record_start(&meta)?;
        self.state
            .lock()
            .map_err(|_| Error::Internal("stream lock poisoned".into()))?
            .sessions
            .insert(id.clone(), ActiveSession { meta, peer, ice: vec![] });
        Ok((id, preset, answer))
    }

    /// Trickle ICE from the viewer (both sides journal for the peer impl).
    pub fn add_ice_candidate(&self, session_id: &str, candidate: &str) -> Result<()> {
        let mut st = self
            .state
            .lock()
            .map_err(|_| Error::Internal("stream lock poisoned".into()))?;
        let s = st
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| Error::NotFound(format!("screen session {session_id}")))?;
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
        self.db_record_end(id)?;
        let mut peer = s.peer;
        peer.close()
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
        let s = StreamService::with_db(db.clone());

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
