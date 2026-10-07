//! hh-hw: hardware audit + ratings + compatibility DB + Wake-on-LAN +
//! network inventory (TRD §11, M3).
//!
//! Hardware-honest (README principle 4): unsupported hardware is allowed
//! with a warning; features degrade instead of promising the impossible.

pub mod audit;
pub mod compat;
pub mod network;
pub mod wake;

use hh_core::Config;
use hh_db::Db;

#[derive(Clone)]
pub struct HwService {
    pub db: Db,
    pub cfg: Config,
}

impl HwService {
    pub fn new(db: Db, cfg: Config) -> Self {
        Self { db, cfg }
    }
}

pub(crate) fn db_e(e: rusqlite::Error) -> hh_core::Error {
    hh_core::Error::Db(e.to_string())
}
