//! Video frame plumbing for screen sharing (M4): RGBA↔I420 conversion,
//! scaling, and H.264 encode/decode via OpenH264 (BSD-licensed, bundled).
//!
//! Cross-platform and unit-tested (`cargo test -p hh-session`); the capture
//! backends and the WebRTC host build on this in `screen.rs`.

/// One RGBA8 video frame.
#[derive(Clone)]
pub struct RgbaFrame {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>, // rgba8888, row-major
}

impl RgbaFrame {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height, data: vec![0; width * height * 4] }
    }

    /// Preserve aspect ratio while downscaling to an even encoder size.
    pub fn scaled_to(&self, target_w: usize, target_h: usize) -> RgbaFrame {
        let ratio = (target_w as f64 / self.width.max(1) as f64).min(target_h as f64 / self.height.max(1) as f64).min(1.0);
        let w = (((self.width as f64 * ratio) as usize) / 2 * 2).max(2);
        let h = (((self.height as f64 * ratio) as usize) / 2 * 2).max(2);
        let mut out = RgbaFrame::new(w, h);
        for y in 0..h {
            let sy = y * self.height / h;
            for x in 0..w {
                let sx = x * self.width / w;
                let s = (sy * self.width + sx) * 4;
                let d = (y * w + x) * 4;
                out.data[d..d + 4].copy_from_slice(&self.data[s..s + 4]);
            }
        }
        out
    }

    /// RGBA → planar I420 (4:2:0). Odd sizes handled by clamping sample reads;
    /// callers normally pre-scale to even sizes anyway.
    pub fn to_i420(&self) -> I420Frame {
        let w = self.width;
        let h = self.height;
        let cw = (w + 1) / 2;
        let ch = (h + 1) / 2;
        let mut y = vec![0u8; w * h];
        let mut u = vec![0u8; cw * ch];
        let mut v = vec![0u8; cw * ch];
        for row in 0..h {
            for col in 0..w {
                let (r, g, b) = {
                    let p = (row * w + col) * 4;
                    (self.data[p] as u32, self.data[p + 1] as u32, self.data[p + 2] as u32)
                };
                // BT.601 limited-range-ish (matches common expectations)
                let yy = ((66 * r + 129 * g + 25 * b + 128) >> 8) + 16;
                y[row * w + col] = yy.clamp(0, 255) as u8;
                if row % 2 == 0 && col % 2 == 0 {
                    let mut rgb = [0i32; 3];
                    for yy in row..=(row + 1).min(h - 1) {
                        for xx in col..=(col + 1).min(w - 1) {
                            let p = (yy * w + xx) * 4;
                            for c in 0..3 { rgb[c] += self.data[p + c] as i32; }
                        }
                    }
                    let samples = ((row + 1).min(h - 1) - row + 1) * ((col + 1).min(w - 1) - col + 1);
                    let [r4, g4, b4] = rgb.map(|c| c / samples as i32);
                    let cb = ((-38 * r4 - 74 * g4 + 112 * b4 + 128) >> 8) + 128;
                    let cr = ((112 * r4 - 94 * g4 - 18 * b4 + 128) >> 8) + 128;
                    let ci = (row / 2) * cw + col / 2;
                    u[ci] = cb.clamp(0, 255) as u8;
                    v[ci] = cr.clamp(0, 255) as u8;
                }
            }
        }
        I420Frame { width: w, height: h, y, u, v }
    }
}

use openh264::formats::YUVSource;

/// Planar I420 frame (encoder/decoder native format).
pub struct I420Frame {
    pub width: usize,
    pub height: usize,
    pub y: Vec<u8>,
    pub u: Vec<u8>,
    pub v: Vec<u8>,
}

/// Encode one I420 frame to an H.264 access unit. The encoder is recreated
/// per call in this helper API (screen sharing creates one persistent encoder
/// in the WebRTC host loop instead — this entry point is for tests/tools).
pub fn encode_frame(frame: &I420Frame, bitrate_bps: u32) -> Result<Vec<u8>, String> {
    use openh264::encoder::{Encoder, EncoderConfig};
    use openh264::OpenH264API;

    let cfg = EncoderConfig::new()
        .set_bitrate_bps(bitrate_bps)
        .max_frame_rate(30.0)
        .usage_type(openh264::encoder::UsageType::ScreenContentRealTime);
    let mut encoder = Encoder::with_api_config(OpenH264API::from_source(), cfg)
        .map_err(|e| format!("openh264 encoder: {e}"))?;
    let bitstream = encoder.encode(frame).map_err(|e| format!("encode: {e}"))?;
    Ok(bitstream.to_vec())
}

/// Decode one H.264 access unit back to I420 (cast receiver path).
pub fn decode_frame(packet: &[u8]) -> Result<I420Frame, String> {
    use openh264::decoder::Decoder;
    let mut decoder = Decoder::new().map_err(|e| format!("openh264 decoder: {e}"))?;
    let yuv = decoder
        .decode(packet)
        .map_err(|e| format!("decode: {e}"))?
        .ok_or_else(|| "decode: no picture available yet".to_string())?;
    let (width, height) = yuv.dimensions();
    let (sy, su, sv) = yuv.strides();
    let compact = |plane: &[u8], stride: usize, w: usize, h: usize| -> Vec<u8> {
        plane.chunks(stride).take(h).flat_map(|row| row[..w].iter().copied()).collect()
    };
    Ok(I420Frame {
        width, height,
        y: compact(yuv.y(), sy, width, height),
        u: compact(yuv.u(), su, (width + 1) / 2, (height + 1) / 2),
        v: compact(yuv.v(), sv, (width + 1) / 2, (height + 1) / 2),
    })
}

impl YUVSource for I420Frame {
    fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }
    fn strides(&self) -> (usize, usize, usize) {
        (self.width, (self.width + 1) / 2, (self.width + 1) / 2)
    }
    fn y(&self) -> &[u8] {
        &self.y
    }
    fn u(&self) -> &[u8] {
        &self.u
    }
    fn v(&self) -> &[u8] {
        &self.v
    }
}

/// I420 → RGBA for the cast receiver window.
pub fn i420_to_rgba(f: &I420Frame) -> RgbaFrame {
    let w = f.width;
    let h = f.height;
    let cw = (w + 1) / 2;
    let mut out = RgbaFrame::new(w, h);
    for row in 0..h {
        for col in 0..w {
            let yy = f.y[row * w + col] as i32 - 16;
            let cb = f.u[(row / 2) * cw + col / 2] as i32 - 128;
            let cr = f.v[(row / 2) * cw + col / 2] as i32 - 128;
            let r = ((298 * yy + 409 * cr + 128) >> 8).clamp(0, 255) as u8;
            let g = ((298 * yy - 100 * cb - 208 * cr + 128) >> 8).clamp(0, 255) as u8;
            let b = ((298 * yy + 516 * cb + 128) >> 8).clamp(0, 255) as u8;
            let p = (row * w + col) * 4;
            out.data[p] = r;
            out.data[p + 1] = g;
            out.data[p + 2] = b;
            out.data[p + 3] = 255;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(w: usize, h: usize) -> RgbaFrame {
        let mut f = RgbaFrame::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let p = (y * w + x) * 4;
                f.data[p] = (x * 255 / w.max(1)) as u8;
                f.data[p + 1] = (y * 255 / h.max(1)) as u8;
                f.data[p + 2] = 128;
                f.data[p + 3] = 255;
            }
        }
        f
    }

    #[test]
    fn scaling_clamps_to_even_blocks() {
        let f = gradient(1920, 1080);
        let s = f.scaled_to(1280, 720);
        assert_eq!(s.width, 1280);
        assert_eq!(s.height, 720);
        let s2 = gradient(640, 480).scaled_to(1920, 1080);
        assert_eq!(s2.width, 640); // never upscales
    }

    #[test]
    fn i420_roundtrip_preserves_luma_shape() {
        let f = gradient(320, 240);
        let i420 = f.to_i420();
        assert_eq!(i420.y.len(), 320 * 240);
        assert_eq!(i420.u.len(), 160 * 120);
        let back = i420_to_rgba(&i420);
        // Gradient corners: blue channel was constant; luma increases to the right.
        let left = back.data[0];
        let right = back.data[(319 * 4) as usize];
        assert!(right >= left, "luma should rise left→right");
    }

    #[test]
    fn encode_produces_h264_and_decode_roundtrip() {
        let f = gradient(320, 240);
        let i420 = f.to_i420();
        let packet = encode_frame(&i420, 1_000_000).expect("encode");
        assert!(!packet.is_empty());
        let decoded = decode_frame(&packet).expect("decode");
        assert_eq!(decoded.width, 320);
        assert_eq!(decoded.height, 240);
    }
}

/// Pack validated even I420 planes into the NV12 layout required by MF.
pub fn i420_to_nv12(frame: &I420Frame) -> Result<Vec<u8>, String> {
    if frame.width == 0 || frame.height == 0 || frame.width % 2 != 0 || frame.height % 2 != 0 || frame.width > 1920 || frame.height > 1920 || frame.width * frame.height > 1920 * 1088 {
        return Err("invalid NV12 frame dimensions".into());
    }
    let pixels = frame.width * frame.height;
    if frame.y.len() != pixels || frame.u.len() != pixels / 4 || frame.v.len() != pixels / 4 { return Err("invalid I420 planes".into()); }
    let mut nv12 = Vec::with_capacity(pixels * 3 / 2); nv12.extend_from_slice(&frame.y);
    for (u, v) in frame.u.iter().zip(&frame.v) { nv12.push(*u); nv12.push(*v); }
    Ok(nv12)
}
#[cfg(test)]
mod nv12_tests {
    use super::*;
    #[test] fn packs_luma_then_interleaved_chroma() {
        let frame = I420Frame { width: 2, height: 2, y: vec![1,2,3,4], u: vec![5], v: vec![6] };
        assert_eq!(i420_to_nv12(&frame).unwrap(), vec![1,2,3,4,5,6]);
        let invalid = I420Frame { u: vec![], ..frame };
        assert!(i420_to_nv12(&invalid).is_err());
    }
}
