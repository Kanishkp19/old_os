//! Video-only LAN media host. Real primary-desktop GDI capture and persistent
//! Media Foundation hardware H264 where usable, persistent OpenH264 fallback
//! and software decode. No synthetic backend is available in production.
use std::{sync::{Arc, Mutex, atomic::{AtomicBool, AtomicUsize, Ordering}}, time::{Duration, Instant}};
use once_cell::sync::Lazy;
use tokio::sync::mpsc;
use webrtc::{api::{APIBuilder, media_engine::MediaEngine}, peer_connection::{configuration::RTCConfiguration, peer_connection_state::RTCPeerConnectionState, sdp::session_description::RTCSessionDescription}, rtp_transceiver::{rtp_codec::{RTCRtpCodecCapability, RTCRtpCodecParameters, RTPCodecType}, rtp_transceiver_direction::RTCRtpTransceiverDirection, RTCRtpTransceiverInit}, track::track_local::track_local_static_sample::TrackLocalStaticSample};
static HARDWARE_STATUS: AtomicUsize = AtomicUsize::new(0); // 0 unprobed, 1 encoded, 2 unavailable/failed
static CURRENT_ENCODER: Lazy<Mutex<&'static str>> = Lazy::new(|| Mutex::new("not_started"));
static WORKERS: AtomicUsize = AtomicUsize::new(0);
struct WorkerGuard;
impl WorkerGuard { fn new() -> Self { WORKERS.fetch_add(1, Ordering::AcqRel); Self } }
impl Drop for WorkerGuard { fn drop(&mut self) { WORKERS.fetch_sub(1, Ordering::AcqRel); } }
static SESSION: Lazy<Mutex<Option<Session>>> = Lazy::new(|| Mutex::new(None));
// Serializes offer/stop to prevent an older start from replacing a newer peer.
static LIFECYCLE: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
struct Session { id: String, stop: Arc<AtomicBool>, commands: mpsc::Sender<Command>, done: std::sync::mpsc::Receiver<()>, thread: Option<std::thread::JoinHandle<()>> }
enum Command { Ice(webrtc::ice_transport::ice_candidate::RTCIceCandidateInit) }
#[derive(Clone, Copy)]
struct Quality { width: usize, height: usize, fps: u32, bitrate: u32 }
impl Quality {
    fn preset(name: &str) -> Result<Self, String> {
        match name {
            "low" => Ok(Self { width: 1280, height: 720, fps: 30, bitrate: 2_000_000 }),
            "balanced" => Ok(Self { width: 1920, height: 1080, fps: 30, bitrate: 6_000_000 }),
            // Software encoder capability is capped at balanced. High can be
            // requested by older clients, but negotiated output is truthful.
            "high" => Self::preset("balanced"),
            _ => Err("unknown screen quality".into()),
        }
    }
}
pub fn capabilities() -> serde_json::Value {
    let sessions = SESSION.lock().map(|s| s.as_ref().filter(|s| !s.stop.load(Ordering::Acquire)).map(|s| vec![s.id.clone()]).unwrap_or_default()).unwrap_or_default();
    let hardware = HARDWARE_STATUS.load(Ordering::Acquire) == 1;
    let current = if sessions.is_empty() { "not_started" } else { CURRENT_ENCODER.lock().map(|name| *name).unwrap_or("unavailable") };
    let encoders = if hardware { vec!["media_foundation_h264", "openh264"] } else { vec!["openh264"] };
    serde_json::json!({"available":crate::capture::desktop_available() && (!sessions.is_empty() || WORKERS.load(Ordering::Acquire) == 0),"encoders":encoders,"preferred_encoder":if hardware {"media_foundation_h264"} else {"openh264"},"encoder_policy":"hardware_then_software","current_encoder":current,"hardware_probe":match HARDWARE_STATUS.load(Ordering::Acquire) {0=>"not_attempted",1=>"encoded_real_frame",_=>"unavailable_or_failed"},"capture":"gdi-primary-desktop","max_preset":"balanced","hw_encode":hardware,"audio":false,"current_sessions":sessions})
}
fn lan_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ip) => ip.is_private() || ip.is_link_local(),
        std::net::IpAddr::V6(ip) => {
            if let Some(ip) = ip.to_ipv4_mapped() { return ip.is_private() || ip.is_link_local(); }
            let first = ip.segments()[0]; first & 0xfe00 == 0xfc00 || first & 0xffc0 == 0xfe80
        }
    }
}
fn validate_candidate(candidate: &str) -> Result<(), String> {
    if candidate.is_empty() { return Ok(()); } // end-of-candidates
    let fields: Vec<_> = candidate.trim_start_matches("a=").split_whitespace().collect();
    if fields.len() < 8 || !fields[0].starts_with("candidate:") || !fields[2].eq_ignore_ascii_case("udp") || fields[6] != "typ" || fields[7] != "host" {
        return Err("screen media requires LAN host candidates".into());
    }
    let ip = fields[4].parse().map_err(|_| "screen ICE requires a numeric LAN address")?;
    if !lan_ip(ip) || fields[5].parse::<u16>().ok().filter(|port| *port != 0).is_none() { return Err("screen ICE address outside LAN".into()); }
    Ok(())
}
fn validate_offer(sdp: &str, kind: &str) -> Result<(), String> {
    if !matches!(kind, "view" | "cast") || sdp.len() > 512 * 1024 || !sdp.starts_with("v=0") { return Err("invalid screen offer".into()); }
    let media: Vec<_> = sdp.lines().filter(|l| l.starts_with("m=")).collect();
    if media.len() != 1 || !media[0].starts_with("m=video ") || media[0].split_whitespace().nth(1) == Some("0") { return Err("screen sessions require one video track and no audio/data".into()); }
    let direction = if kind == "view" { "a=recvonly" } else { "a=sendonly" };
    if !sdp.lines().any(|l| l.trim() == direction) { return Err("screen offer has wrong video direction".into()); }
    Ok(())
}
fn lan_offer(sdp: &str) -> Result<String, String> {
    let mut seen = 0; let mut accepted = 0; let mut output = Vec::new();
    for line in sdp.lines() {
        if line.starts_with("a=candidate:") {
            seen += 1;
            if seen > 128 { return Err("too many screen ICE candidates".into()); }
            // Dual-stack LAN interfaces can also gather public IPv6 hosts.
            // Keep usable private hosts and never probe those public peers.
            if validate_candidate(line).is_err() { continue; }
            accepted += 1;
        }
        if line.starts_with("a=remote-candidates:") { continue; }
        output.push(line);
    }
    if seen > 0 && accepted == 0 { return Err("screen offer contains no numeric LAN host candidate".into()); }
    Ok(output.join("\r\n") + "\r\n")
}
pub fn handle_offer(id: &str, sdp: &str, preset: &str, kind: &str) -> Result<serde_json::Value, String> {
    validate_offer(sdp, kind)?;
    if id.is_empty() || id.len() > 128 || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') { return Err("screen session id required".into()); }
    let offer = lan_offer(sdp)?;
    let quality = Quality::preset(preset)?;
    if !crate::capture::desktop_available() { return Err("interactive desktop unavailable or locked".into()); }
    let _guard = LIFECYCLE.lock().map_err(|_| "screen lifecycle lock failed")?;
    if SESSION.lock().map_err(|_| "screen state lock failed")?.as_ref().is_some_and(|s| !s.stop.load(Ordering::Acquire)) { return Err("a screen session is already active".into()); }
    stop_inner(None)?;
    if WORKERS.load(Ordering::Acquire) != 0 { return Err("previous screen worker is still stopping".into()); }
    let stop = Arc::new(AtomicBool::new(false));
    let (answer_tx, answer_rx) = std::sync::mpsc::sync_channel(1);
    let (done_tx, done) = std::sync::mpsc::sync_channel(1);
    let (commands, command_rx) = mpsc::channel(16);
    let mode = kind.to_owned(); let worker_stop = stop.clone();
    let thread = std::thread::spawn(move || {
        let result = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| e.to_string())
            .and_then(|rt| rt.block_on(run_peer(&mode, quality, &offer, worker_stop.clone(), command_rx, &answer_tx)));
        if let Err(e) = result { let _ = answer_tx.try_send(Err(e)); }
        worker_stop.store(true, Ordering::Release); let _ = done_tx.send(());
    });
    let answer = match answer_rx.recv_timeout(Duration::from_secs(12)) {
        Ok(Ok(answer)) => answer,
        Ok(Err(error)) => { stop.store(true, Ordering::Release); let _ = done.recv_timeout(Duration::from_secs(2)); return Err(error); }
        Err(_) => { stop.store(true, Ordering::Release); let _ = done.recv_timeout(Duration::from_secs(2)); return Err("screen negotiation timed out".into()); }
    };
    *SESSION.lock().map_err(|_| "screen state lock failed")? = Some(Session { id: id.into(), stop, commands, done, thread: Some(thread) });
    Ok(serde_json::json!({"session_id":id,"kind":kind,"preset":if preset == "high" {"balanced"} else {preset},"answer_sdp":answer,"ice_candidates":[],"encoder":if kind == "view" {Some(CURRENT_ENCODER.lock().map(|name| *name).unwrap_or("unavailable"))} else {None},"decoder":if kind == "cast" {Some("openh264")} else {None},"audio":false}))
}
pub fn add_ice(id: &str, candidate: &str) -> Result<(), String> {
    if candidate.len() > 8192 { return Err("ICE candidate too large".into()); }
    // Structured candidate JSON preserves mid/m-line index. Legacy bare
    // candidate strings are valid for our sole video media section.
    let candidate: webrtc::ice_transport::ice_candidate::RTCIceCandidateInit = if candidate.starts_with('{') {
        serde_json::from_str(candidate).map_err(|_| "invalid ICE candidate")?
    } else { webrtc::ice_transport::ice_candidate::RTCIceCandidateInit { candidate: candidate.into(), sdp_mid: Some("0".into()), sdp_mline_index: Some(0), ..Default::default() } };
    validate_candidate(&candidate.candidate)?;
    let state = SESSION.lock().map_err(|_| "screen state lock failed")?;
    let session = state.as_ref().ok_or("screen session not active")?;
    if id != session.id || session.stop.load(Ordering::Acquire) { return Err("screen session not active".into()); }
    session.commands.try_send(Command::Ice(candidate)).map_err(|_| "screen ICE queue full or closed".into())
}
pub fn stop_session(id: &str) -> Result<(), String> {
    let _guard = LIFECYCLE.lock().map_err(|_| "screen lifecycle lock failed")?;
    stop_inner(Some(id))
}
pub fn stop() -> Result<(), String> {
    let _guard = LIFECYCLE.lock().map_err(|_| "screen lifecycle lock failed")?;
    stop_inner(None)
}
fn stop_inner(id: Option<&str>) -> Result<(), String> {
    let mut state = SESSION.lock().map_err(|_| "screen state lock failed")?;
    if id.is_some_and(|id| state.as_ref().is_some_and(|s| s.id != id)) { return Err("screen session not active".into()); }
    let session = state.take(); drop(state);
    if let Some(mut session) = session {
        session.stop.store(true, Ordering::Release);
        // Never block the IPC dispatcher forever on driver/window teardown.
        if session.done.recv_timeout(Duration::from_secs(3)).is_ok() {
            if let Some(thread) = session.thread.take() { let _ = thread.join(); }
        } else { return Err("screen worker shutdown timed out".into()); }
    }
    Ok(())
}
async fn run_peer(kind: &str, quality: Quality, sdp: &str, stop: Arc<AtomicBool>, mut commands: mpsc::Receiver<Command>, answer_tx: &std::sync::mpsc::SyncSender<Result<String, String>>) -> Result<(), String> {
    let mut engine = MediaEngine::default();
    for (payload, profile) in [(102, "42e01f"), (104, "42001f"), (106, "42e028")] {
        engine.register_codec(RTCRtpCodecParameters { capability: RTCRtpCodecCapability { mime_type: "video/H264".into(), clock_rate: 90000, rtcp_feedback: vec![webrtc::rtp_transceiver::RTCPFeedback { typ: "nack".into(), parameter: "".into() }, webrtc::rtp_transceiver::RTCPFeedback { typ: "nack".into(), parameter: "pli".into() }], sdp_fmtp_line: format!("level-asymmetry-allowed=1;packetization-mode=1;profile-level-id={profile}"), ..Default::default() }, payload_type: payload, ..Default::default() }, RTPCodecType::Video).map_err(|e| e.to_string())?;
    }
    let registry = webrtc::api::interceptor_registry::register_default_interceptors(webrtc::interceptor::registry::Registry::new(), &mut engine).map_err(|e| e.to_string())?;
    let mut settings = webrtc::api::setting_engine::SettingEngine::default();
    settings.set_ip_filter(Box::new(lan_ip));
    settings.set_network_types(vec![webrtc::ice::network_type::NetworkType::Udp4, webrtc::ice::network_type::NetworkType::Udp6]);
    let api = APIBuilder::new().with_media_engine(engine).with_interceptor_registry(registry).with_setting_engine(settings).build();
    let pc = Arc::new(api.new_peer_connection(RTCConfiguration::default()).await.map_err(|e| e.to_string())?);
    let connected = Arc::new(AtomicBool::new(false));
    let disconnected = Arc::new(Mutex::new(None::<Instant>));
    { let stop = stop.clone(); let connected = connected.clone(); let disconnected = disconnected.clone();
        pc.on_peer_connection_state_change(Box::new(move |state| {
            connected.store(state == RTCPeerConnectionState::Connected, Ordering::Release);
            if matches!(state, RTCPeerConnectionState::Failed | RTCPeerConnectionState::Closed) { stop.store(true, Ordering::Release); }
            if let Ok(mut since) = disconnected.lock() { *since = (state == RTCPeerConnectionState::Disconnected).then(Instant::now); }
            Box::pin(async {})
        }));
    }
    let mut media_task = None;
    let mut feedback_task = None;
    let mut media_worker = None;
    let setup: Result<String, String> = async {
        if kind == "view" {
            let track = Arc::new(TrackLocalStaticSample::new(RTCRtpCodecCapability { mime_type: "video/H264".into(), clock_rate: 90000, sdp_fmtp_line: "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f".into(), ..Default::default() }, "desktop".into(), "homehub".into()));
            let transceiver = pc.add_transceiver_from_track(track.clone(), Some(RTCRtpTransceiverInit { direction: RTCRtpTransceiverDirection::Sendonly, send_encodings: vec![] })).await.map_err(|e| e.to_string())?;
            let sender = transceiver.sender().await;
            let requested_keyframe = Arc::new(AtomicBool::new(false));
            let feedback_keyframe = requested_keyframe.clone(); let feedback_stop = stop.clone();
            feedback_task = Some(tokio::spawn(async move {
                while !feedback_stop.load(Ordering::Acquire) {
                    match sender.read_rtcp().await {
                        Ok((packets, _)) => if packets.iter().any(|packet| packet.as_any().is::<webrtc::rtcp::payload_feedbacks::picture_loss_indication::PictureLossIndication>() || packet.as_any().is::<webrtc::rtcp::payload_feedbacks::full_intra_request::FullIntraRequest>()) { feedback_keyframe.store(true, Ordering::Release); },
                        Err(_) => break,
                    }
                }
            }));
            let (frames, mut packets) = mpsc::channel::<(Vec<u8>, Duration)>(2);
            let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
            let capture_stop = stop.clone(); let capture_connected = connected.clone();
            let worker_guard = WorkerGuard::new();
            media_worker = Some(std::thread::spawn(move || { let _guard = worker_guard; capture_worker(quality, capture_stop, capture_connected, requested_keyframe, frames, ready_tx); }));
            // Wait for real capture/encoder initialization before claiming availability.
            ready_rx.recv_timeout(Duration::from_secs(3)).map_err(|_| "desktop capture startup timed out")??;
            let send_stop = stop.clone();
            media_task = Some(tokio::spawn(async move {
                while let Some((data, duration)) = packets.recv().await {
                    if send_stop.load(Ordering::Acquire) { break; }
                    let result = tokio::time::timeout(Duration::from_secs(2), track.write_sample(&webrtc::media::Sample { data: data.into(), duration, ..Default::default() })).await;
                    if !matches!(result, Ok(Ok(()))) { send_stop.store(true, Ordering::Release); break; }
                }
            }));
        } else {
            let (packets, receiver) = std::sync::mpsc::sync_channel::<Vec<u8>>(2);
            let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
            let render_stop = stop.clone();
            let worker_guard = WorkerGuard::new();
            media_worker = Some(std::thread::spawn(move || { let _guard = worker_guard; render_worker(receiver, render_stop, ready_tx); }));
            ready_rx.recv_timeout(Duration::from_secs(3)).map_err(|_| "cast window startup timed out")??;
            let received = Arc::new(AtomicBool::new(false)); let weak_pc = Arc::downgrade(&pc); let track_stop = stop.clone();
            pc.on_track(Box::new(move |track, _, _| {
                let packets = packets.clone(); let stop = track_stop.clone(); let pc = weak_pc.clone();
                let first = !received.swap(true, Ordering::AcqRel);
                Box::pin(async move {
                    if !first || track.codec().capability.mime_type.to_lowercase() != "video/h264" { stop.store(true, Ordering::Release); return; }
                    let mut assembler = crate::h264::Assembler::default();
                    let mut last_pli = Instant::now() - Duration::from_secs(2);
                    while !stop.load(Ordering::Acquire) {
                        let packet = match tokio::time::timeout(Duration::from_secs(10), track.read_rtp()).await {
                            Ok(Ok((packet, _))) => packet,
                            _ => { stop.store(true, Ordering::Release); break; }
                        };
                        let result = assembler.push(packet.header.sequence_number, packet.header.timestamp, packet.header.marker, &packet.payload);
                        let mut request_keyframe = false;
                        match result {
                            Ok(Some(frame)) => if packets.try_send(frame).is_err() { request_keyframe = true; },
                            Ok(None) => {}, Err(_) => request_keyframe = true,
                        }
                        if (request_keyframe || last_pli.elapsed() > Duration::from_secs(3)) && last_pli.elapsed() > Duration::from_secs(1) {
                            last_pli = Instant::now();
                            if let Some(pc) = pc.upgrade() {
                                let _ = pc.write_rtcp(&[Box::new(webrtc::rtcp::payload_feedbacks::picture_loss_indication::PictureLossIndication { sender_ssrc: 0, media_ssrc: track.ssrc() })]).await;
                            }
                        }
                    }
                })
            }));
            pc.add_transceiver_from_kind(RTPCodecType::Video, Some(RTCRtpTransceiverInit { direction: RTCRtpTransceiverDirection::Recvonly, send_encodings: vec![] })).await.map_err(|e| e.to_string())?;
        }
        pc.set_remote_description(RTCSessionDescription::offer(sdp.into()).map_err(|e| e.to_string())?).await.map_err(|e| e.to_string())?;
        let answer = pc.create_answer(None).await.map_err(|e| e.to_string())?;
        let mut gathered = pc.gathering_complete_promise().await;
        pc.set_local_description(answer).await.map_err(|e| e.to_string())?;
        tokio::time::timeout(Duration::from_secs(6), gathered.recv()).await.map_err(|_| "ICE gathering timed out")?;
        pc.local_description().await.map(|d| d.sdp).ok_or_else(|| "missing local answer".into())
    }.await;
    match setup {
        Ok(answer) => { answer_tx.send(Ok(answer)).map_err(|_| "screen signaling receiver closed")?; }
        Err(error) => { let _ = answer_tx.try_send(Err(error)); stop.store(true, Ordering::Release); }
    }
    let started = Instant::now(); let mut was_connected = false;
    while !stop.load(Ordering::Acquire) {
        if connected.load(Ordering::Acquire) { was_connected = true; }
        if !was_connected && started.elapsed() > Duration::from_secs(30) { break; }
        if disconnected.lock().map(|d| d.is_some_and(|since| since.elapsed() > Duration::from_secs(5))).unwrap_or(true) { break; }
        tokio::select! {
            command = commands.recv() => match command {
                Some(Command::Ice(candidate)) => if pc.add_ice_candidate(candidate).await.is_err() { break; },
                None => break,
            },
            _ = tokio::time::sleep(Duration::from_millis(50)) => {},
        }
    }
    stop.store(true, Ordering::Release);
    let _ = tokio::time::timeout(Duration::from_secs(1), pc.close()).await;
    if let Some(task) = media_task { task.abort(); }
    if let Some(task) = feedback_task { task.abort(); }
    // Workers poll stop and bounded queues. Avoid blocking driver teardown.
    if let Some(worker) = media_worker {
        let deadline = Instant::now() + Duration::from_secs(1);
        while !worker.is_finished() && Instant::now() < deadline { tokio::time::sleep(Duration::from_millis(10)).await; }
        if worker.is_finished() { let _ = worker.join(); }
    }
    Ok(())
}
enum EncoderPipeline {
    Hardware(crate::mf_encoder::HardwareEncoder),
    Software(openh264::encoder::Encoder),
}
impl EncoderPipeline {
    fn software(frame: &crate::enc::I420Frame, quality: Quality) -> Result<Self, String> {
        use openh264::{OpenH264API, encoder::{Encoder, EncoderConfig, UsageType}};
        let mut encoder = Encoder::with_api_config(OpenH264API::from_source(), EncoderConfig::new().set_bitrate_bps(quality.bitrate).max_frame_rate(quality.fps as f32).usage_type(UsageType::ScreenContentRealTime).set_multiple_thread_idc(2)).map_err(|e| e.to_string())?;
        let first = encoder.encode(frame).map_err(|e| e.to_string())?.to_vec();
        crate::h264::validate_initial_packet(&first)?;
        if let Ok(mut current) = CURRENT_ENCODER.lock() { *current = "openh264"; }
        Ok(Self::Software(encoder))
    }
    fn select(frame: &crate::enc::I420Frame, quality: Quality, hardware: bool) -> Result<Self, String> {
        if hardware {
            if let Ok((encoder, _real_first_packet)) = crate::mf_encoder::HardwareEncoder::start(frame, quality.fps, quality.bitrate) {
                HARDWARE_STATUS.store(1, Ordering::Release);
                if let Ok(mut current) = CURRENT_ENCODER.lock() { *current = "media_foundation_h264"; }
                return Ok(Self::Hardware(encoder));
            }
            HARDWARE_STATUS.store(2, Ordering::Release);
        }
        Self::software(frame, quality)
    }
    fn encode(&mut self, frame: &crate::enc::I420Frame, quality: Quality, keyframe: bool, allow_hardware: &mut bool) -> Result<Vec<u8>, String> {
        match self {
            Self::Hardware(encoder) => match encoder.encode(frame, keyframe) {
                Ok(packet) => Ok(packet),
                Err(_) => {
                    // Failed device/driver never breaks the entire viewer.
                    // Replace it with one persistent software encoder.
                    *allow_hardware = false; HARDWARE_STATUS.store(2, Ordering::Release);
                    *self = Self::software(frame, quality)?;
                    self.encode(frame, quality, true, allow_hardware)
                }
            },
            Self::Software(encoder) => {
                if keyframe { encoder.force_intra_frame(); }
                encoder.encode(frame).map(|bits| bits.to_vec()).map_err(|e| e.to_string())
            }
        }
    }
}
fn capture_worker(mut quality: Quality, stop: Arc<AtomicBool>, connected: Arc<AtomicBool>, requested_keyframe: Arc<AtomicBool>, frames: mpsc::Sender<(Vec<u8>, Duration)>, ready: std::sync::mpsc::SyncSender<Result<(), String>>) {
    let initialized = crate::capture::DesktopCapture::new(quality.width, quality.height).and_then(|mut capture| {
        let first = capture.frame()?.to_i420();
        EncoderPipeline::select(&first, quality, true).map(|encoder| (capture, encoder))
    });
    let (mut capture, mut encoder) = match initialized { Ok(value) => value, Err(error) => { let _ = ready.send(Err(error)); stop.store(true, Ordering::Release); return; } };
    let _ = ready.send(Ok(()));
    let mut allow_hardware = matches!(&encoder, EncoderPipeline::Hardware(_));
    let mut overload = 0u32; let mut frame_count = 0u32; let mut pending_keyframe = true;
    while !stop.load(Ordering::Acquire) {
        if !connected.load(Ordering::Acquire) { std::thread::sleep(Duration::from_millis(50)); pending_keyframe = true; continue; }
        let started = Instant::now(); let budget = Duration::from_secs_f64(1.0 / quality.fps as f64);
        let encoded = capture.frame().and_then(|frame| {
            let force = requested_keyframe.swap(false, Ordering::AcqRel) || pending_keyframe || frame_count % (quality.fps * 2) == 0;
            pending_keyframe = false;
            encoder.encode(&frame.to_i420(), quality, force, &mut allow_hardware)
        });
        frame_count = frame_count.wrapping_add(1);
        match encoded {
            Ok(data) if !data.is_empty() => {
                if frames.try_send((data, budget)).is_err() { pending_keyframe = true; overload += 1; }
            },
            Ok(_) => {}, Err(_) => { stop.store(true, Ordering::Release); break; },
        }
        if started.elapsed() > budget { overload += 1; } else { overload = overload.saturating_sub(1); }
        if overload >= 10 {
            if quality.width > 1280 { quality.width = 1280; quality.height = 720; quality.bitrate = 2_000_000; }
            else { quality.fps = (quality.fps / 2).max(10); }
            let replacement = crate::capture::DesktopCapture::new(quality.width, quality.height).and_then(|mut capture| {
                let first = capture.frame()?.to_i420();
                EncoderPipeline::select(&first, quality, allow_hardware).map(|encoder| (capture, encoder))
            });
            match replacement {
                Ok((new_capture, new_encoder)) => { capture = new_capture; encoder = new_encoder; allow_hardware = matches!(&encoder, EncoderPipeline::Hardware(_)); pending_keyframe = true; },
                Err(_) => { stop.store(true, Ordering::Release); break; },
            }
            overload = 0;
        }
        if let Some(wait) = budget.checked_sub(started.elapsed()) { std::thread::sleep(wait); }
    }
}

fn render_worker(receiver: std::sync::mpsc::Receiver<Vec<u8>>, stop: Arc<AtomicBool>, ready: std::sync::mpsc::SyncSender<Result<(), String>>) {
    let initialized = openh264::decoder::Decoder::new().map_err(|e| e.to_string()).and_then(|decoder| {
        let (width, height) = crate::capture::primary_size();
        minifb::Window::new("Home Hub — Phone screen (Esc to close)", width, height, minifb::WindowOptions { borderless: true, resize: false, scale_mode: minifb::ScaleMode::AspectRatioStretch, ..Default::default() }).map(|window| (decoder, window)).map_err(|e| e.to_string())
    });
    let (mut decoder, mut window) = match initialized { Ok(v) => v, Err(error) => { let _ = ready.send(Err(error)); stop.store(true, Ordering::Release); return; } };
    window.set_position(0, 0);
    let _ = ready.send(Ok(())); let mut pixels = Vec::new();
    while !stop.load(Ordering::Acquire) && crate::capture::desktop_available() && window.is_open() && !window.is_key_down(minifb::Key::Escape) {
        match receiver.recv_timeout(Duration::from_millis(15)) {
            Ok(access_unit) => match decoder.decode(&access_unit) {
                Ok(Some(frame)) => {
                    use openh264::formats::YUVSource;
                    let (w, h) = frame.dimensions();
                    if w > 1920 || h > 1920 || w.saturating_mul(h) > 1920 * 1088 { break; }
                    let mut rgba = vec![0; w * h * 4]; frame.write_rgba8(&mut rgba);
                    pixels.clear(); pixels.extend(rgba.chunks_exact(4).map(|p| (u32::from(p[0]) << 16) | (u32::from(p[1]) << 8) | u32::from(p[2])));
                    if window.update_with_buffer(&pixels, w, h).is_err() { break; }
                },
                Ok(None) => window.update(), Err(_) => break,
            },
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => window.update(),
            Err(_) => break,
        }
    }
    stop.store(true, Ordering::Release);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn rejects_public_ice_and_accepts_private_hosts() {
        assert!(validate_candidate("candidate:1 1 udp 1 192.168.1.3 50000 typ host").is_ok());
        assert!(validate_candidate("candidate:1 1 udp 1 8.8.8.8 50000 typ host").is_err());
        assert!(validate_candidate("candidate:1 1 udp 1 192.168.1.3 50000 typ relay").is_err());
    }
    #[test] fn mixed_public_private_offer_keeps_only_lan_hosts() {
        let offer = "v=0\r\na=candidate:1 1 udp 1 192.168.1.3 50000 typ host\r\na=candidate:2 1 udp 1 8.8.8.8 50000 typ host\r\n";
        let filtered = lan_offer(offer).unwrap();
        assert!(filtered.contains("192.168.1.3"));
        assert!(!filtered.contains("8.8.8.8"));
    }
    #[test] fn rejects_audio_data_and_wrong_direction() {
        assert!(validate_offer("v=0\r\nm=video 9 UDP/TLS/RTP/SAVPF 102\r\na=recvonly\r\n", "view").is_ok());
        assert!(validate_offer("v=0\r\nm=video 9 UDP/TLS/RTP/SAVPF 102\r\na=sendrecv\r\n", "view").is_err());
        assert!(validate_offer("v=0\r\nm=video 9 UDP/TLS/RTP/SAVPF 102\r\na=recvonly\r\nm=audio 9 RTP/AVP 0\r\n", "view").is_err());
    }
}
