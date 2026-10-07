//! Revocation list: in-memory set of revoked cert serials, refreshed from
//! SQLite on revoke events (AGENTS.md §6 — the mTLS verifier consults this
//! set per handshake, so "Remove Device" cuts access immediately).

use std::collections::HashSet;
use std::sync::{Arc, RwLock};

use hh_core::error::Result;
use hh_db::Db;

#[derive(Clone)]
pub struct RevocationList {
    db: Db,
    inner: Arc<RwLock<HashSet<String>>>,
}

impl RevocationList {
    pub fn load(db: Db) -> Result<Self> {
        let list = Self { db, inner: Arc::new(RwLock::new(HashSet::new())) };
        list.refresh()?;
        Ok(list)
    }

    pub fn refresh(&self) -> Result<()> {
        let serials = self.db.revoked_serials()?;
        let mut set = self.inner.write().map_err(|_| hh_core::Error::Internal("revocation lock poisoned".into()))?;
        *set = serials.into_iter().collect();
        Ok(())
    }

    pub fn revoke(&self, serial: &str) {
        if let Ok(mut set) = self.inner.write() {
            set.insert(serial.to_string());
        }
    }

    pub fn is_revoked(&self, serial: &str) -> bool {
        self.inner.read().map(|s| s.contains(serial)).unwrap_or(true) // fail closed
    }
}
