//! hh-photos: media metadata (EXIF), thumbnails, gallery index, backup
//! sources and diff (TRD §8, API_SPEC §7).
//!
//! Safety rule (TRD §6.4): `backup_items.status='verified'` only after the
//! Hub confirmed the whole-file root hash; "free phone storage" flows must
//! additionally re-check the hash client-side before deleting.

pub mod backup;
pub mod gallery;
pub mod meta;
pub mod thumbs;

use hh_core::Config;
use hh_db::Db;

#[derive(Clone)]
pub struct PhotoService {
    pub db: Db,
    pub cfg: Config,
}

impl PhotoService {
    pub fn new(db: Db, cfg: Config) -> Self {
        Self { db, cfg }
    }
}

pub(crate) fn db_e(e: rusqlite::Error) -> hh_core::Error {
    hh_core::Error::Db(e.to_string())
}
