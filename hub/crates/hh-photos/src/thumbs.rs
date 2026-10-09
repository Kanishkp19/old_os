//! Thumbnail pipeline (TRD §8): 256px and 1024px JPEGs (deviation from the
//! spec's WebP — the `image` crate's WebP encoder is lossless-only; JPEG is
//! smaller for photos. Update API_SPEC if WebP becomes practical).
//!
//! Runs idle-priority: the worker is driven by hh-service with yields and
//! battery checks; HEIC without a decoder degrades to `unsupported`
//! (AGENTS.md §6 — never assume the WIC HEIF extension).

use std::path::PathBuf;

use hh_core::error::Result;
use rusqlite::params;

use crate::{db_e, PhotoService};

impl PhotoService {
    /// Generate thumbnails for up to `max` pending media rows.
    pub fn process_thumb_queue(&self, max: u32) -> Result<u32> {
        let pending: Vec<(String, String, String)> = {
            let c = self.db.lock()?;
            let mut st = c
                .prepare(
                    "SELECT m.file_id, f.rel_path, m.type FROM media m
                     JOIN files f ON f.id = m.file_id
                     WHERE m.thumb_status='pending' AND f.deleted_at IS NULL
                     LIMIT ?1",
                )
                .map_err(db_e)?;
            let rows = st
                .query_map(params![max as i64], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .map_err(db_e)?
                .collect::<std::result::Result<Vec<(String, String, String)>, _>>()
                .map_err(db_e)?;
            rows
        };

        let mut done = 0;
        for (file_id, rel, kind) in pending {
            let src = hh_core::paths::jail_join(&self.cfg.library_dir(), &rel)?;
            match generate(&src, &self.cfg.thumbs_dir(), &file_id, &kind) {
                Ok((p256, p1024, phash)) => {
                    let c = self.db.lock()?;
                    c.execute(
                        "UPDATE media SET thumb_status='ready', thumb_256_path=?2, thumb_1024_path=?3,
                                phash=COALESCE(phash, ?4)
                         WHERE file_id=?1",
                        params![
                            file_id,
                            p256.to_string_lossy().to_string(),
                            p1024.to_string_lossy().to_string(),
                            phash.map(|h| h as i64)
                        ],
                    )
                    .map_err(db_e)?;
                    done += 1;
                }
                Err(ThumbError::Unsupported) => {
                    let c = self.db.lock()?;
                    c.execute(
                        "UPDATE media SET thumb_status='unsupported' WHERE file_id=?1",
                        params![file_id],
                    )
                    .map_err(db_e)?;
                }
                Err(ThumbError::Other(_)) => {
                    let c = self.db.lock()?;
                    c.execute(
                        "UPDATE media SET thumb_status='failed' WHERE file_id=?1",
                        params![file_id],
                    )
                    .map_err(db_e)?;
                }
                Err(ThumbError::Retryable(_)) => break,
            }
        }
        Ok(done)
    }

    pub fn thumb_path(&self, file_id: &str, size: u32) -> Result<Option<PathBuf>> {
        let c = self.db.lock()?;
        let (status, p256, p1024): (String, Option<String>, Option<String>) = c
            .query_row(
                "SELECT m.thumb_status, m.thumb_256_path, m.thumb_1024_path FROM media m
                 JOIN files f ON f.id=m.file_id WHERE m.file_id=?1 AND f.deleted_at IS NULL",
                params![file_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map_err(|_| hh_core::Error::NotFound(format!("media {file_id}")))?;
        if status != "ready" {
            return Ok(None); // client shows placeholder
        }
        let path=match size {
            256 => p256.map(PathBuf::from),
            _ => p1024.map(PathBuf::from),
        };
        if path.as_ref().is_some_and(|path|!path.is_file()) {
            c.execute("UPDATE media SET thumb_status='pending',thumb_256_path=NULL,thumb_1024_path=NULL WHERE file_id=?1",[file_id]).map_err(db_e)?;
            return Ok(None);
        }
        Ok(path)
    }
}

enum ThumbError {
    Unsupported,
    Other(String),
    Retryable(String),
}

impl std::fmt::Display for ThumbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThumbError::Unsupported => write!(f, "unsupported format"),
            ThumbError::Other(e) => write!(f, "{e}"),
            ThumbError::Retryable(e) => write!(f, "{e}"),
        }
    }
}

fn generate(src: &std::path::Path, thumbs_dir: &std::path::Path, file_id: &str, kind:&str) -> std::result::Result<(PathBuf, PathBuf, Option<u64>), ThumbError> {
    if kind=="video" {return Err(ThumbError::Unsupported);}
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if matches!(ext.as_str(), "heic" | "heif" | "dng" | "cr2" | "nef" | "arw") {
        return Err(ThumbError::Unsupported); // honest placeholder (TRD §8)
    }
    let img = image::open(src).map_err(|e| match e {
        image::ImageError::Unsupported(_)=>ThumbError::Unsupported,
        other=>ThumbError::Other(other.to_string()),
    })?;
    std::fs::create_dir_all(thumbs_dir).map_err(|e| ThumbError::Retryable(e.to_string()))?;

    let out = |size: u32| -> std::result::Result<PathBuf, ThumbError> {
        let t = img.thumbnail(size, size);
        let path = thumbs_dir.join(format!("{file_id}_{size}.jpg"));
        let temp = thumbs_dir.join(format!("{file_id}_{size}_{}.part",ulid::Ulid::new()));
        let result=(|| {
            let mut file=std::fs::File::create(&temp).map_err(|e|ThumbError::Retryable(e.to_string()))?;
            t.write_to(&mut file,image::ImageFormat::Jpeg).map_err(|e|ThumbError::Retryable(e.to_string()))?;
            file.sync_all().map_err(|e|ThumbError::Retryable(e.to_string()))?;
            if path.exists() {std::fs::remove_file(&path).map_err(|e|ThumbError::Retryable(e.to_string()))?;}
            std::fs::rename(&temp,&path).map_err(|e|ThumbError::Retryable(e.to_string()))?;
            Ok::<(),ThumbError>(())
        })();
        if result.is_err() {let _=std::fs::remove_file(&temp);}
        result?;
        Ok(path)
    };
    let p256 = out(256)?;
    let p1024 = out(1024)?;
    // dHash from the already-decoded image (W2.3): 64-bit difference hash for
    // the "Similar photos" detector (media.phash, migration 0004 index).
    let phash = dhash64(&img);
    Ok((p256, p1024, phash))
}

/// 64-bit dHash: 9×8 grayscale downscale, horizontal gradient bits.
pub fn dhash64(img: &image::DynamicImage) -> Option<u64> {
    let gray = img.to_luma8();
    let small = image::imageops::resize(&gray, 9, 8, image::imageops::FilterType::Triangle);
    let mut hash: u64 = 0;
    let mut bit = 0;
    for y in 0..8u32 {
        for x in 0..8u32 {
            let left = small.get_pixel(x, y).0[0] as i16;
            let right = small.get_pixel(x + 1, y).0[0] as i16;
            if left > right {
                hash |= 1u64 << bit;
            }
            bit += 1;
        }
    }
    Some(hash)
}

/// Hamming distance between two dHash values (0 = identical). Images within
/// HAMMING_NEAR are "similar" for the duplicate detector (W2.3).
pub fn hamming(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

pub const HAMMING_NEAR: u32 = 8;
