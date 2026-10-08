//! Configuration and data-directory layout (TRD §7.1, §13).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Friendly name, e.g. "Kanishk's Home Hub".
    pub hub_name: String,
    /// Program data dir: `%ProgramData%\HomeHub` on Windows.
    pub data_dir: PathBuf,
    /// Library root (user-chosen; default recommended drive with most free space).
    pub library_root: PathBuf,
    /// Log directory (size-rotated).
    pub log_dir: PathBuf,
    /// Optional second-copy target (external drive).
    pub second_copy_root: Option<PathBuf>,
    /// Feature switches derived from settings + hardware audit.
    pub features: FeatureFlags,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeatureFlags {
    pub photos: bool,
    pub remote: bool,
    pub screen: bool,
    pub hotspot: bool,
    pub wol: bool,
    pub telemetry_opt_in: bool,
}

impl Default for Config {
    fn default() -> Self {
        let data_dir = default_data_dir();
        let library_root = default_library_root();
        Self {
            hub_name: default_hub_name(),
            log_dir: data_dir.join("logs"),
            data_dir,
            library_root,
            second_copy_root: None,
            features: FeatureFlags {
                photos: true,
                remote: false, // off by default; per-device opt-in (SECURITY §6)
                screen: false,
                hotspot: false,
                wol: true,
                telemetry_opt_in: false,
            },
        }
    }
}

impl Config {
    /// Create all directories. Never touches existing user data (FR-1.4).
    pub fn ensure_dirs(&self) -> Result<()> {
        for d in [
            &self.data_dir,
            &self.log_dir,
            &self.library_root,
            &self.library_dir(),
            &self.tmp_dir(),
            &self.thumbs_dir(),
        ] {
            std::fs::create_dir_all(d).map_err(Error::Io)?;
        }
        Ok(())
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("hub.db")
    }

    /// Partial uploads. MUST be on the same volume as the library so the
    /// finalize rename is atomic (AGENTS.md §6).
    pub fn tmp_dir(&self) -> PathBuf {
        self.library_root.join(".hh-tmp")
    }

    pub fn thumbs_dir(&self) -> PathBuf {
        self.library_root.join(".hh-thumbs")
    }

    pub fn library_dir(&self) -> PathBuf {
        self.library_root.join("Library")
    }

    /// Path of the DPAPI-protected (Windows) / 0600 (unix) Hub CA key blob.
    /// Never stored raw in hub.db (BACKEND_SCHEMA §1).
    pub fn ca_key_path(&self) -> PathBuf {
        self.data_dir.join("ca.key.enc")
    }

    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        if let Some(parent)=path.parent(){std::fs::create_dir_all(parent)?;}
        let text=serde_json::to_vec_pretty(self).map_err(|e|Error::Internal(format!("config encode: {e}")))?;
        let temp=path.with_extension(format!("json-{}.tmp",ulid::Ulid::new()));
        let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(&temp)?;
        use std::io::Write;
        file.write_all(&text)?;file.sync_all()?;drop(file);
        std::fs::rename(temp,path)?;
        Ok(())
    }

    pub fn load_or_create(path:&std::path::Path, defaults:Self)->Result<Self>{
        match std::fs::read_to_string(path){
            Ok(text)=>serde_json::from_str(&text).map_err(|e|Error::Internal(format!("config parse: {e}"))),
            Err(e) if e.kind()==std::io::ErrorKind::NotFound=>{defaults.save(path)?;Ok(defaults)},
            Err(e)=>Err(e.into()),
        }
    }
    pub fn load_or_default(path: &std::path::Path) -> Result<Self> { Self::load_or_create(path,Self::default()) }

}

#[cfg(windows)]
fn default_data_dir() -> PathBuf {
    std::env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
        .join("HomeHub")
}

#[cfg(not(windows))]
fn default_data_dir() -> PathBuf {
    // Dev/unix builds (Phase 4 will use /var/lib/homehub).
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".homehub")
}

#[cfg(windows)]
fn default_library_root() -> PathBuf {
    // TODO(Windows): pick fixed drive with most free space; warn if C: < 50 GB free.
    PathBuf::from(r"C:\HomeHubData")
}

#[cfg(not(windows))]
fn default_library_root() -> PathBuf {
    default_data_dir().join("data")
}

fn default_hub_name() -> String {
    let user = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "My".into());
    format!("{user}'s Home Hub")
}
