//! EXIF/creation metadata extraction (TRD §8). Fallback order:
//! EXIF DateTimeOriginal → container time → file mtime → upload time.
//! GPS is stored but only displayed on explicit user opt-in (SECURITY §8).

use std::path::Path;

use hh_core::error::Result;
use hh_core::time::now_ms;

#[derive(Debug, Clone, Default)]
pub struct MediaMeta {
    pub taken_at: i64,
    pub taken_at_source: &'static str,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_ms: Option<u64>,
    pub orientation: Option<u32>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub gps_lat: Option<f64>,
    pub gps_lon: Option<f64>,
}

pub fn extract(path: &Path, mime: Option<&str>) -> Result<MediaMeta> {
    let mut meta = MediaMeta { taken_at: now_ms(), taken_at_source: "upload", ..Default::default() };

    // mtime fallback (better than upload time).
    if let Ok(md) = std::fs::metadata(path) {
        if let Ok(mt) = md.modified() {
            if let Ok(d) = mt.duration_since(std::time::UNIX_EPOCH) {
                meta.taken_at = d.as_millis() as i64;
                meta.taken_at_source = "mtime";
            }
        }
    }

    let is_image = mime.map(|m| m.starts_with("image/")).unwrap_or(false);
    if is_image {
        extract_exif(path, &mut meta);
        // Dimensions via image crate as fallback (EXIF covers JPEG/TIFF).
        if meta.width.is_none() {
            if let Ok((w, h)) = image::image_dimensions(path) {
                meta.width = Some(w);
                meta.height = Some(h);
            }
        }
    }
    // Videos: duration/dimensions come from the container (MP4 mvhd) —
    // extracted by the thumbnail worker when ffmpeg is bundled (optional, TRD §2).
    Ok(meta)
}

fn extract_exif(path: &Path, meta: &mut MediaMeta) {
    let Ok(file) = std::fs::File::open(path) else { return };
    let mut buf = std::io::BufReader::new(file);
    let Ok(exif) = exif::Reader::new().read_from_container(&mut buf) else { return };

    if let Some(f) = exif.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY) {
        if let exif::Value::Ascii(ref v) = f.value {
            if let Some(first) = v.first() {
                if let Ok(s) = std::str::from_utf8(first) {
                    if let Some(ms) = parse_exif_datetime(s) {
                        meta.taken_at = ms;
                        meta.taken_at_source = "exif";
                    }
                }
            }
        }
    }
    if let Some(f) = exif.get_field(exif::Tag::PixelXDimension, exif::In::PRIMARY) {
        meta.width = f.value.get_uint(0);
    }
    if let Some(f) = exif.get_field(exif::Tag::PixelYDimension, exif::In::PRIMARY) {
        meta.height = f.value.get_uint(0);
    }
    if let Some(f) = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY) {
        meta.orientation = f.value.get_uint(0);
    }
    meta.camera_make = exif
        .get_field(exif::Tag::Make, exif::In::PRIMARY)
        .map(|f| f.display_value().to_string());
    meta.camera_model = exif
        .get_field(exif::Tag::Model, exif::In::PRIMARY)
        .map(|f| f.display_value().to_string());

    // GPS (stored, opt-in display — SECURITY §8).
    let lat = exif.get_field(exif::Tag::GPSLatitude, exif::In::PRIMARY);
    let lat_ref = exif.get_field(exif::Tag::GPSLatitudeRef, exif::In::PRIMARY);
    let lon = exif.get_field(exif::Tag::GPSLongitude, exif::In::PRIMARY);
    let lon_ref = exif.get_field(exif::Tag::GPSLongitudeRef, exif::In::PRIMARY);
    if let (Some(lat), Some(lon)) = (lat, lon) {
        meta.gps_lat = dms_to_deg(&lat.value, lat_ref.map(|f| f.display_value().to_string()).as_deref());
        meta.gps_lon = dms_to_deg(&lon.value, lon_ref.map(|f| f.display_value().to_string()).as_deref());
    }
}

/// "YYYY:MM:DD HH:MM:SS" → epoch ms (interpreted as UTC; camera TZ unknown).
fn parse_exif_datetime(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.len() < 19 {
        return None;
    }
    let y: i64 = s.get(0..4)?.parse().ok()?;
    let mo: i64 = s.get(5..7)?.parse().ok()?;
    let d: i64 = s.get(8..10)?.parse().ok()?;
    let h: i64 = s.get(11..13)?.parse().ok()?;
    let mi: i64 = s.get(14..16)?.parse().ok()?;
    let sec: i64 = s.get(17..19)?.parse().ok()?;
    // days-from-civil (Hinnant) → epoch ms
    let y_adj = if mo <= 2 { y - 1 } else { y };
    let era = if y_adj >= 0 { y_adj } else { y_adj - 399 } / 400;
    let yoe = y_adj - era * 400;
    let mo_adj = if mo > 2 { mo - 3 } else { mo + 9 };
    let doy = (153 * mo_adj + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400_000 + (h * 3600 + mi * 60 + sec) * 1000)
}

fn dms_to_deg(v: &exif::Value, reference: Option<&str>) -> Option<f64> {
    if let exif::Value::Rational(parts) = v {
        if parts.len() >= 3 {
            let deg = parts[0].to_f64() + parts[1].to_f64() / 60.0 + parts[2].to_f64() / 3600.0;
            let sign = match reference {
                Some("S") | Some("W") => -1.0,
                _ => 1.0,
            };
            return Some(sign * deg);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exif_datetime() {
        assert_eq!(parse_exif_datetime("1970:01:01 00:00:00"), Some(0));
        assert_eq!(parse_exif_datetime("2026:10:05 00:00:00"), Some(1_791_158_400_000));
        assert_eq!(parse_exif_datetime("bad"), None);
    }
}
