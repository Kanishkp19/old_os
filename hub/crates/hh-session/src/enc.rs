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

    /// Nearest-neighbor downscale to the target size (encoder input size
    /// must be even; we round to multiples of 16 for H.264 friendliness).
    pub fn scaled_to(&self, target_w: usize, target_h: usize) -> RgbaFrame {
        let w = (target_w.min(self.width) / 16 * 16).max(16);
        let h = (target_h.min(self.height) / 16 * 16).max(16);
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
                    let r2 = self.data[((row + 1).min(h - 1) * w + col) * 4] as u32;
                    let g2 = self.data[((row + 1).min(h - 1) * w + col) * 4 + 1] as u32;
                    let b2 = self.data[((row + 1).min(h - 1) * w + col) * 4 + 2] as u32;
                    let r4 = ((r + r2) / 2) as i32;
                    let g4 = ((g + g2) / 2) as i32;
                    let b4 = ((b + b2) / 2) as i32;
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
    Ok(I420Frame {
        width: yuv.dimensions().0,
        height: yuv.dimensions().1,
        y: yuv.y().to_vec(),
        u: yuv.u().to_vec(),
        v: yuv.v().to_vec(),
    })
}

impl YUVSource for I420Frame {
    fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }
    fn strides(&self) -> (usize, usize, usize) {
        (self.width, self.width / 2, self.width / 2)
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
