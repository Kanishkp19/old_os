//! Compatibility database (M3): YAML in repo, matched by CPU/ Wi-Fi model
//! substrings → "supported" / "untested" / "unsupported" (TRD §11, FR-9.3).

use hh_core::error::Result;
use hh_core::platform::HardwareReport;
use serde::Deserialize;

use crate::HwService;

#[derive(Debug, Deserialize)]
pub struct CompatDb {
    pub supported: Vec<CompatEntry>,
    pub unsupported: Vec<CompatEntry>,
}

#[derive(Debug, Deserialize)]
pub struct CompatEntry {
    /// Substring matched (case-insensitive) against CPU model or device name.
    pub match_contains: String,
    pub note: Option<String>,
}

const BUNDLED: &str = include_str!("../../../../compat/compat-db.yaml");

impl HwService {
    pub fn compat_status(&self, report: &HardwareReport) -> Result<String> {
        let text = std::fs::read_to_string(self.cfg.data_dir.join("compat-db.yaml"))
            .unwrap_or_else(|_| BUNDLED.to_string());
        let db: CompatDb = serde_yaml::from_str(&text)
            .map_err(|e| hh_core::Error::Internal(format!("compat db parse: {e}")))?;
        let hay = report.cpu_model.to_lowercase();
        for e in &db.unsupported {
            if hay.contains(&e.match_contains.to_lowercase()) {
                return Ok("unsupported".into());
            }
        }
        for e in &db.supported {
            if hay.contains(&e.match_contains.to_lowercase()) {
                return Ok("supported".into());
            }
        }
        Ok("untested".into())
    }
}
