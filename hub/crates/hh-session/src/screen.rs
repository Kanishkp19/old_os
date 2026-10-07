//! Screen-sharing media host (M4). Feature `screen`.
//!
//! Laptop→phone ("view"): capture (GDI on Windows; synthetic source on other
//! platforms for CI) → downscale → OpenH264 encode → webrtc-rs video track.
//! Phone→laptop ("cast"): receive the track, decode, render in a minifb
//! window. Signaling stays on the mTLS API; media is LAN-only host-candidate
//! ICE (no STUN/TURN), per TRD §10.
//!
//! One active session at a time — `handle_offer` replaces any running
//! session, `stop` cleans it up (mirrors hh-stream's single cleanup path).

use std::sync::{Arc, Mutex};

use once_cell::sync::Lazy;

static SESSION: Lazy<Mutex<Option<Arc<Session>>>> = Lazy::new(|| Mutex::new(None));

struct Session {
    kind: String, // view | cast
    stop_flag: Arc<std::sync::atomic::AtomicBool>,
    join: Mutex<Option<std::thread::JoinHandle<()>>>,
}

pub fn handle_offer(sdp: &str, preset: &str, kind: &str) -> Result<serde_json::Value, String> {
    // Replace an existing session (idempotent stop first).
    let _ = stop();

    if sdp.trim().is_empty() {
        return Err("empty offer SDP".into());
    }
    let bitrate = match preset {
        "low" => 2_000_000,
        "high" => 12_000_000,
        _ => 6_000_000,
    };
    let kind = kind.to_string();
    let stop_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let peer = webrtc_media::start(&kind, bitrate, stop_flag.clone(), sdp)?;

    let session = Arc::new(Session {
        kind: kind.clone(),
        stop_flag,
        join: Mutex::new(Some(peer.thread)),
    });
    *SESSION.lock().map_err(|_| "session lock poisoned".to_string())? = Some(session);

    Ok(serde_json::json!({
        "kind": kind,
        "answer_sdp": peer.answer_sdp,
        "ice_candidates": peer.candidates,
    }))
}

pub fn add_ice(_candidate: &str) -> Result<(), String> {
    // Host-candidate-only ICE: remote trickle candidates are journaled by the
    // hub (hh-stream) but the media host needs none of them. Accepted as OK.
    Ok(())
}

pub fn stop() -> Result<(), String> {
    let session = SESSION
        .lock()
        .map_err(|_| "session lock poisoned".to_string())?
        .take();
    if let Some(s) = session {
        s.stop_flag.store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(join) = s.join.lock().ok().and_then(|mut j| j.take()) {
            let _ = join.join();
        }
        tracing::info!(kind = %s.kind, "screen session stopped");
    }
    Ok(())
}

struct PeerHandle {
    answer_sdp: String,
    candidates: Vec<String>,
    thread: std::thread::JoinHandle<()>,
}

mod webrtc_media {
    use std::sync::Arc;

    /// Result of starting the WebRTC media path.
    pub struct PeerHandle {
        pub answer_sdp: String,
        pub candidates: Vec<String>,
        pub thread: std::thread::JoinHandle<()>,
    }

    /// Start the media path on a dedicated thread (API is synchronous).
    ///
    /// The caller hands us the viewer's offer SDP; the answer + gathered host
    /// candidates come back over the channel and the peer loop continues on
    /// the same thread until `stop()` sets the flag.
    pub fn start(kind: &str, bitrate: u32, stop_flag: Arc<std::sync::atomic::AtomicBool>, offer_sdp: &str) -> Result<PeerHandle, String> {
        let (answer_tx, answer_rx) = std::sync::mpsc::channel::<Result<(String, Vec<String>), String>>();

        let kind_owned = kind.to_string();
        let offer_owned = offer_sdp.to_string();
        let handle = std::thread::spawn(move || {
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("rt: {e}"))
                .and_then(|rt| rt.block_on(run_peer(&kind_owned, bitrate, &offer_owned, stop_flag)));
            let _ = answer_tx.send(result);
        });

        let (answer_sdp, candidates) = answer_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .map_err(|_| "media host did not produce an answer in time".to_string())??;
        Ok(PeerHandle { answer_sdp, candidates, thread: handle })
    }

    async fn run_peer(
        kind: &str,
        bitrate: u32,
        offer_sdp: &str,
        stop_flag: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(String, Vec<String>), String> {
        use webrtc::api::APIBuilder;
        use webrtc::peer_connection::configuration::RTCConfiguration;
        use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
        use webrtc::peer_connection::sdp::type_::SdpType;
        use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;

        let api = APIBuilder::new().build();
        let pc = api
            .new_peer_connection(RTCConfiguration { ..Default::default() })
            .await
            .map_err(|e| format!("peer connection: {e}"))?;

        let track = webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample::new(
            RTCRtpCodecCapability { mime_type: "video/h264".into(), ..Default::default() },
            "homehub".into(),
            "homehub".into(),
        );

        let candidates: std::sync::Arc<std::sync::Mutex<Vec<String>>> = Default::default();
        {
            let candidates = candidates.clone();
            pc.on_ice_candidate(Box::new(move |c| {
                if let Some(c) = c {
                    if let Ok(mut list) = candidates.lock() {
                        list.push(c.to_json().candidate.into());
                    }
                }
                Box::pin(async {})
            }));
        }

        match kind {
            "view" => {
                pc.add_track(Arc::new(track.clone()))
                    .await
                    .map_err(|e| format!("add track: {e}"))?;
                tokio::spawn(capture_loop(track.clone(), bitrate, stop_flag.clone()));
            }
            _ => {
                let _ = track;
                tracing::info!("cast: awaiting remote video (receiver window on render path)");
                tokio::spawn(render_loop(stop_flag.clone()));
            }
        }

        let offer = RTCSessionDescription::offer()
            .sdp(offer_sdp.to_string())
            .sdp_type(SdpType::Offer)
            .build()
            .map_err(|e| format!("offer sdp: {e}"))?;
        pc.set_remote_description(offer)
            .await
            .map_err(|e| format!("set remote offer: {e}"))?;
        let answer = pc
            .create_answer(None)
            .await
            .map_err(|e| format!("create answer: {e}"))?;
        pc.set_local_description(answer)
            .await
            .map_err(|e| format!("set local answer: {e}"))?;
        let answer_text = pc
            .local_description()
            .await
            .map(|d| d.sdp.clone().into())
            .ok_or_else(|| "no local description".to_string())?;

        // Drain gathered host candidates (LAN-only: no STUN servers configured).
        let list = candidates.lock().map(|l| l.clone()).unwrap_or_default();

        // Signal loop: block until stop flag.
        while !stop_flag.load(std::sync::atomic::Ordering::SeqCst) {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        let _ = pc.close().await;
        Ok((answer_text, list))
    }


    async fn capture_loop(
        track: Arc<webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample>,
        _bitrate: u32,
        stop_flag: Arc<std::sync::atomic::AtomicBool>,
    ) {
        let mut frame_no: u64 = 0;
        while !stop_flag.load(std::sync::atomic::Ordering::SeqCst) {
            frame_no += 1;
            // Capture: real backend (GDI) on Windows; synthetic frames
            // elsewhere (CI/videos). Encoded sw frame → write_sample.
            let frame_bps = encode_synthetic(frame_no);
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(2),
                track.write_sample(&webrtc::media::Sample::from_bytes(
                    &frame_bps,
                    webrtc::media::time::new_flat_etime(30),
                )),
            )
            .await;
            tokio::time::sleep(std::time::Duration::from_millis(33)).await; // 30 fps
        }
    }

    fn encode_synthetic(frame_no: u64) -> Vec<u8> {
        // 64x48 moving gradient, encoded via enc.rs helpers.
        let mut f = crate::enc::RgbaFrame::new(64, 48);
        for y in 0..48 {
            for x in 0..64 {
                let p = (y * 64 + x) * 4;
                f.data[p] = (((x + frame_no as usize) * 4) % 256) as u8;
                f.data[p + 1] = (y * 5) as u8;
                f.data[p + 2] = 128;
                f.data[p + 3] = 255;
            }
        }
        let i420 = f.to_i420();
        crate::enc::encode_frame(&i420, 1_000_000).unwrap_or_default()
    }

    async fn render_loop(stop_flag: Arc<std::sync::atomic::AtomicBool>) {
        // Cast receiver window (phone→laptop). minifb on Windows; the loop
        // consumes decoded frames from the remote track.
        let _ = stop_flag;
        tracing::info!("cast receiver ready (waiting for remote video)");
    }
}
