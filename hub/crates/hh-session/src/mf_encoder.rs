//! Native Media Foundation hardware H.264 encoder. Hardware-only enumeration,
//! asynchronous NeedInput/HaveOutput credits, CPU NV12 samples, bounded event
//! polling, and explicit COM/MF/transform ownership. No software MFT is labeled
//! hardware. Drivers that require GPU textures fall back to OpenH264.
use std::{mem::ManuallyDrop, ptr, time::{Duration, Instant}};
use windows::{core::{Interface, VARIANT}, Win32::{Foundation::E_NOTIMPL, Media::MediaFoundation::*, System::Com::{CoInitializeEx, CoUninitialize, CoTaskMemFree, COINIT_MULTITHREADED}}};
use crate::enc::I420Frame;

struct Runtime;
impl Runtime {
    fn new() -> Result<Self, String> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok().map_err(|e| e.to_string())?;
            if let Err(error) = MFStartup(MF_VERSION, MFSTARTUP_FULL) { CoUninitialize(); return Err(error.to_string()); }
        }
        Ok(Self)
    }
}
impl Drop for Runtime { fn drop(&mut self) { unsafe { let _ = MFShutdown(); CoUninitialize(); } } }
struct Activated(IMFActivate);
impl Drop for Activated { fn drop(&mut self) { unsafe { let _ = self.0.ShutdownObject(); } } }
struct Transform {
    _activation: Activated,
    transform: IMFTransform,
    events: IMFMediaEventGenerator,
    codec: ICodecAPI,
    input: u32,
    output: u32,
    input_credits: usize,
    output_credits: usize,
    width: usize,
    height: usize,
    fps: u32,
    timestamp: i64,
    sequence_header: Vec<u8>,
}
impl Drop for Transform {
    fn drop(&mut self) {
        unsafe {
            let _ = self.transform.ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0);
            let _ = self.transform.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0);
            if let Ok(shutdown) = self.transform.cast::<IMFShutdown>() { let _ = shutdown.Shutdown(); }
        }
        // Activated::drop also calls ShutdownObject; both paths are idempotent.
    }
}
/// Native transform is created, used and dropped on the capture thread.
pub struct HardwareEncoder { state: Transform, _runtime: Runtime }
impl HardwareEncoder {
    /// Select an actual hardware encoder only after it encoded a real frame.
    pub fn start(frame: &I420Frame, fps: u32, bitrate: u32) -> Result<(Self, Vec<u8>), String> {
        let runtime = Runtime::new()?;
        let candidates = unsafe {
            let input = MFT_REGISTER_TYPE_INFO { guidMajorType: MFMediaType_Video, guidSubtype: MFVideoFormat_NV12 };
            let output = MFT_REGISTER_TYPE_INFO { guidMajorType: MFMediaType_Video, guidSubtype: MFVideoFormat_H264 };
            let mut pointer = ptr::null_mut(); let mut count = 0;
            MFTEnumEx(MFT_CATEGORY_VIDEO_ENCODER, MFT_ENUM_FLAG_HARDWARE | MFT_ENUM_FLAG_SORTANDFILTER,
                Some(&input), Some(&output), &mut pointer, &mut count).map_err(|e| e.to_string())?;
            if pointer.is_null() {
                if count == 0 { return Err("no Media Foundation hardware H264 encoder".into()); }
                return Err("invalid hardware encoder enumeration".into());
            }
            let array = std::slice::from_raw_parts_mut(pointer, count as usize);
            // Take every returned COM reference before freeing its array.
            let candidates = array.iter_mut().filter_map(Option::take).collect::<Vec<_>>();
            CoTaskMemFree(Some(pointer.cast())); candidates
        };
        let deadline = Instant::now() + Duration::from_secs(2);
        for activation in candidates.into_iter().take(8) {
            if Instant::now() >= deadline { break; }
            if let Ok(mut state) = unsafe { Transform::new(activation, frame.width, frame.height, fps, bitrate) } {
                let remaining = deadline.saturating_duration_since(Instant::now()).min(Duration::from_millis(500));
                if let Ok(packet) = state.encode(frame, true, remaining) {
                    if !packet.is_empty() && crate::h264::validate_initial_packet(&packet).is_ok() { return Ok((Self { state, _runtime: runtime }, packet)); }
                }
            }
        }
        Err("hardware H264 encoder could not encode this desktop format".into())
    }
    pub fn encode(&mut self, frame: &I420Frame, keyframe: bool) -> Result<Vec<u8>, String> {
        self.state.encode(frame, keyframe, Duration::from_millis(250))
    }
}
impl Transform {
    unsafe fn new(activation: IMFActivate, width: usize, height: usize, fps: u32, bitrate: u32) -> Result<Self, String> {
        let activation = Activated(activation);
        let transform: IMFTransform = activation.0.ActivateObject().map_err(|e| e.to_string())?;
        let attrs = transform.GetAttributes().map_err(|e| e.to_string())?;
        // Hardware MFTs must implement the asynchronous contract.
        if attrs.GetUINT32(&MF_TRANSFORM_ASYNC).map_err(|e| e.to_string())? != 1 { return Err("hardware transform is not asynchronous".into()); }
        attrs.SetUINT32(&MF_TRANSFORM_ASYNC_UNLOCK, 1).map_err(|e| e.to_string())?;
        let _ = attrs.SetUINT32(&MF_LOW_LATENCY, 1);
        let events = transform.cast::<IMFMediaEventGenerator>().map_err(|e| e.to_string())?;
        let codec = transform.cast::<ICodecAPI>().map_err(|e| e.to_string())?;
        codec.IsSupported(&CODECAPI_AVEncVideoForceKeyFrame).map_err(|e| e.to_string())?;
        let _ = codec.SetValue(&CODECAPI_AVEncCommonLowLatency, &VARIANT::from(true));
        let _ = codec.SetValue(&CODECAPI_AVEncCommonRealTime, &VARIANT::from(true));
        let _ = codec.SetValue(&CODECAPI_AVEncMPVDefaultBPictureCount, &VARIANT::from(0u32));
        let _ = codec.SetValue(&CODECAPI_AVEncMPVGOPSize, &VARIANT::from(fps * 2));
        let (mut inputs, mut outputs) = (0, 0);
        transform.GetStreamCount(&mut inputs, &mut outputs).map_err(|e| e.to_string())?;
        if inputs != 1 || outputs != 1 { return Err("unsupported hardware stream layout".into()); }
        let mut input_ids = [0]; let mut output_ids = [0];
        if let Err(error) = transform.GetStreamIDs(&mut input_ids, &mut output_ids) {
            if error.code() != E_NOTIMPL { return Err(error.to_string()); }
        }
        let output_type = video_type(MFVideoFormat_H264, width, height, fps)?;
        output_type.SetUINT32(&MF_MT_AVG_BITRATE, bitrate).map_err(|e| e.to_string())?;
        output_type.SetUINT32(&MF_MT_MPEG2_PROFILE, 66).map_err(|e| e.to_string())?; // baseline: no B frames
        output_type.SetUINT32(&MF_MT_MPEG2_LEVEL, 40).map_err(|e| e.to_string())?;
        transform.SetOutputType(output_ids[0], &output_type, 0).map_err(|e| e.to_string())?;
        let input_type = video_type(MFVideoFormat_NV12, width, height, fps)?;
        input_type.SetUINT32(&MF_MT_DEFAULT_STRIDE, width as u32).map_err(|e| e.to_string())?;
        input_type.SetUINT32(&MF_MT_ALL_SAMPLES_INDEPENDENT, 1).map_err(|e| e.to_string())?;
        transform.SetInputType(input_ids[0], &input_type, 0).map_err(|e| e.to_string())?;
        transform.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0).map_err(|e| e.to_string())?;
        transform.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0).map_err(|e| e.to_string())?;
        Ok(Self { _activation: activation, transform, events, codec, input: input_ids[0], output: output_ids[0], input_credits: 0, output_credits: 0, width, height, fps, timestamp: 0, sequence_header: Vec::new() })
    }
    fn events(&mut self) -> Result<(), String> {
        for _ in 0..64 {
            let event = match unsafe { self.events.GetEvent(MF_EVENT_FLAG_NO_WAIT) } {
                Ok(event) => event,
                Err(e) if e.code() == MF_E_NO_EVENTS_AVAILABLE => return Ok(()),
                Err(e) => return Err(e.to_string()),
            };
            unsafe { event.GetStatus().map_err(|e| e.to_string())?.ok().map_err(|e| e.to_string())?; }
            match unsafe { event.GetType().map_err(|e| e.to_string())? } {
                kind if kind == METransformNeedInput.0 as u32 => self.input_credits = self.input_credits.saturating_add(1),
                kind if kind == METransformHaveOutput.0 as u32 => self.output_credits = self.output_credits.saturating_add(1),
                _ => {},
            }
            if self.input_credits > 64 || self.output_credits > 64 { return Err("hardware encoder event budget exceeded".into()); }
        }
        Ok(())
    }
    fn encode(&mut self, frame: &I420Frame, keyframe: bool, timeout: Duration) -> Result<Vec<u8>, String> {
        if (frame.width, frame.height) != (self.width, self.height) { return Err("hardware encoder dimensions changed".into()); }
        let deadline = Instant::now() + timeout;
        self.wait_for(false, deadline)?;
        // Low-latency baseline output must be consumed before another input.
        if self.output_credits != 0 { return Err("hardware encoder returned unexpected delayed frame".into()); }
        if keyframe { unsafe { self.codec.SetValue(&CODECAPI_AVEncVideoForceKeyFrame, &VARIANT::from(1u32)).map_err(|e| e.to_string())?; } }
        let nv12 = crate::enc::i420_to_nv12(frame)?;
        let sample = unsafe {
            let buffer = MFCreateMemoryBuffer(nv12.len() as u32).map_err(|e| e.to_string())?;
            write_buffer(&buffer, &nv12)?;
            let sample = MFCreateSample().map_err(|e| e.to_string())?;
            sample.AddBuffer(&buffer).map_err(|e| e.to_string())?;
            sample.SetSampleTime(self.timestamp).map_err(|e| e.to_string())?;
            sample.SetSampleDuration(10_000_000 / self.fps as i64).map_err(|e| e.to_string())?;
            sample
        };
        unsafe { self.transform.ProcessInput(self.input, &sample, 0).map_err(|e| e.to_string())?; }
        self.input_credits -= 1; self.timestamp += 10_000_000 / self.fps as i64;
        self.wait_for(true, deadline)?;
        self.output_credits -= 1;
        let mut packet = self.output()?;
        if keyframe {
            let current = unsafe { self.transform.GetOutputCurrentType(self.output).map_err(|e| e.to_string())? };
            if let Ok(size) = unsafe { current.GetBlobSize(&MF_MT_MPEG_SEQUENCE_HEADER) } {
                if size > 65536 { return Err("hardware sequence header too large".into()); }
                let mut header = vec![0; size as usize];
                unsafe { current.GetBlob(&MF_MT_MPEG_SEQUENCE_HEADER, &mut header, None).map_err(|e| e.to_string())?; }
                self.sequence_header = crate::h264::normalize_sequence_header(&header)?;
            }
            if !self.sequence_header.is_empty() {
                let mut combined = self.sequence_header.clone(); combined.append(&mut packet); packet = combined;
            }
        }
        Ok(packet)
    }
    fn wait_for(&mut self, output: bool, deadline: Instant) -> Result<(), String> {
        loop {
            self.events()?;
            if (if output { self.output_credits } else { self.input_credits }) > 0 { return Ok(()); }
            if Instant::now() >= deadline { return Err("hardware encoder event timed out".into()); }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    fn output(&mut self) -> Result<Vec<u8>, String> {
        for attempt in 0..2 {
            unsafe {
                let info = self.transform.GetOutputStreamInfo(self.output).map_err(|e| e.to_string())?;
                let provided = info.dwFlags & (MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 as u32 | MFT_OUTPUT_STREAM_CAN_PROVIDE_SAMPLES.0 as u32) != 0;
                let sample = if provided { None } else {
                    if info.cbSize as usize > crate::h264::MAX_ACCESS_UNIT { return Err("hardware output buffer too large".into()); }
                    let size = info.cbSize.max((self.width * self.height) as u32);
                    if info.cbAlignment > 4096 || info.cbAlignment > 0 && !info.cbAlignment.is_power_of_two() { return Err("unsupported hardware output alignment".into()); }
                    let buffer = MFCreateAlignedMemoryBuffer(size, info.cbAlignment.saturating_sub(1)).map_err(|e| e.to_string())?;
                    let sample = MFCreateSample().map_err(|e| e.to_string())?; sample.AddBuffer(&buffer).map_err(|e| e.to_string())?; Some(sample)
                };
                let mut output = [MFT_OUTPUT_DATA_BUFFER { dwStreamID: self.output, pSample: ManuallyDrop::new(sample), dwStatus: 0, pEvents: ManuallyDrop::new(None) }];
                let mut status = 0;
                let result = self.transform.ProcessOutput(0, &mut output, &mut status);
                // Both fields are caller-owned COM references even on failure.
                let sample = ManuallyDrop::take(&mut output[0].pSample);
                drop(ManuallyDrop::take(&mut output[0].pEvents));
                if let Err(error) = result {
                    if error.code() == MF_E_TRANSFORM_STREAM_CHANGE && attempt == 0 {
                        let media = self.transform.GetOutputAvailableType(self.output, 0).map_err(|e| e.to_string())?;
                        if media.GetGUID(&MF_MT_SUBTYPE).map_err(|e| e.to_string())? != MFVideoFormat_H264 || media.GetUINT64(&MF_MT_FRAME_SIZE).map_err(|e| e.to_string())? != (((self.width as u64) << 32) | self.height as u64) {
                            return Err("hardware encoder changed video format".into());
                        }
                        self.transform.SetOutputType(self.output, &media, 0).map_err(|e| e.to_string())?;
                        // Format negotiation does not consume the pending frame.
                        continue;
                    }
                    return Err(error.to_string());
                }
                let sample = sample.ok_or("hardware encoder produced no output sample")?;
                let bytes = read_buffer(&sample.ConvertToContiguousBuffer().map_err(|e| e.to_string())?)?;
                return crate::h264::normalize_access_unit(&bytes);
            }
        }
        Err("hardware encoder output format did not settle".into())
    }
}
unsafe fn video_type(subtype: windows::core::GUID, width: usize, height: usize, fps: u32) -> Result<IMFMediaType, String> {
    if width == 0 || height == 0 || width > 1920 || height > 1920 || width * height > 1920 * 1088 || fps == 0 || fps > 60 { return Err("invalid hardware media dimensions".into()); }
    let media = MFCreateMediaType().map_err(|e| e.to_string())?;
    media.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).map_err(|e| e.to_string())?;
    media.SetGUID(&MF_MT_SUBTYPE, &subtype).map_err(|e| e.to_string())?;
    media.SetUINT64(&MF_MT_FRAME_SIZE, ((width as u64) << 32) | height as u64).map_err(|e| e.to_string())?;
    media.SetUINT64(&MF_MT_FRAME_RATE, ((fps as u64) << 32) | 1).map_err(|e| e.to_string())?;
    media.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, (1 << 32) | 1).map_err(|e| e.to_string())?;
    media.SetUINT32(&MF_MT_INTERLACE_MODE, 2).map_err(|e| e.to_string())?;
    Ok(media)
}
struct Locked<'a>(&'a IMFMediaBuffer);
impl Drop for Locked<'_> { fn drop(&mut self) { unsafe { let _ = self.0.Unlock(); } } }
unsafe fn write_buffer(buffer: &IMFMediaBuffer, data: &[u8]) -> Result<(), String> {
    let mut pointer = ptr::null_mut(); let mut capacity = 0;
    buffer.Lock(&mut pointer, Some(&mut capacity), None).map_err(|e| e.to_string())?;
    let lock = Locked(buffer);
    if pointer.is_null() || (capacity as usize) < data.len() { return Err("hardware input buffer invalid".into()); }
    ptr::copy_nonoverlapping(data.as_ptr(), pointer, data.len()); drop(lock);
    buffer.SetCurrentLength(data.len() as u32).map_err(|e| e.to_string())
}
unsafe fn read_buffer(buffer: &IMFMediaBuffer) -> Result<Vec<u8>, String> {
    let mut pointer = ptr::null_mut(); let mut capacity = 0; let mut length = 0;
    buffer.Lock(&mut pointer, Some(&mut capacity), Some(&mut length)).map_err(|e| e.to_string())?;
    let _lock = Locked(buffer);
    if pointer.is_null() || length > capacity || length as usize > crate::h264::MAX_ACCESS_UNIT { return Err("hardware output buffer invalid".into()); }
    Ok(std::slice::from_raw_parts(pointer, length as usize).to_vec())
}
