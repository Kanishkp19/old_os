//! Storage copy verification and resumable migration checkpoints.

use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use hh_core::error::{Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MigrationFileEntry {
    pub rel_path: String,
    pub size: u64,
    pub blake3_hash: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageMigrationManifest {
    pub version: u32,
    pub source_root_label: String,
    pub target_root_id: String,
    pub files: Vec<MigrationFileEntry>,
    pub total_bytes: u64,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct MigrationCheckpoint {
    pub completed_paths: HashSet<String>,
    pub bytes_copied: u64,
    pub last_updated_ms: i64,
}

pub fn verify_file_copy(src: &Path, dst: &Path, expected_blake3: &str) -> Result<()> {
    let source_size = fs::metadata(src)?.len();
    let destination_size = fs::metadata(dst)?.len();
    if source_size != destination_size {
        return Err(Error::Integrity("Migration file size mismatch".into()));
    }
    let mut file = File::open(dst)?;
    let mut hasher = blake3::Hasher::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        hasher.update(&chunk[..count]);
    }
    if hasher.finalize().to_hex().as_str() != expected_blake3 {
        return Err(Error::Integrity("BLAKE3 hash mismatch".into()));
    }
    Ok(())
}

pub fn save_checkpoint(path: &Path, checkpoint: &MigrationCheckpoint) -> Result<()> {
    let data = serde_json::to_vec(checkpoint)
        .map_err(|_| Error::Internal("Could not serialize migration checkpoint".into()))?;
    let name = path
        .file_name()
        .ok_or_else(|| Error::BadRequest("Invalid checkpoint path".into()))?;
    let temporary = path.with_file_name(format!(
        ".{}.{}.tmp",
        name.to_string_lossy(),
        ulid::Ulid::new()
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&data)?;
        file.sync_all()?;
        drop(file);
        if path.exists() {
            super::atomic_replace(&temporary, path)?;
        } else {
            fs::rename(&temporary, path)?;
            #[cfg(unix)]
            if let Some(parent) = path.parent() {
                File::open(parent)?.sync_all()?;
            }
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub fn load_checkpoint(path: &Path) -> Result<MigrationCheckpoint> {
    let data = match fs::read(path) {
        Ok(data) => data,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(MigrationCheckpoint::default())
        }
        Err(error) => return Err(error.into()),
    };
    serde_json::from_slice(&data)
        .map_err(|_| Error::Integrity("Invalid migration checkpoint".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("hh-migration-{}", ulid::Ulid::new()));
        fs::create_dir(&dir).unwrap();
        dir
    }

    #[test]
    fn verifies_matching_copy() {
        let dir = test_dir();
        let src = dir.join("src");
        let dst = dir.join("dst");
        let content = b"verified bytes";
        fs::write(&src, content).unwrap();
        fs::write(&dst, content).unwrap();
        let hash = blake3::hash(content).to_hex().to_string();
        verify_file_copy(&src, &dst, &hash).unwrap();
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_altered_copy() {
        let dir = test_dir();
        let src = dir.join("src");
        let dst = dir.join("dst");
        fs::write(&src, b"source").unwrap();
        fs::write(&dst, b"broken").unwrap();
        let hash = blake3::hash(b"source").to_hex().to_string();
        assert!(matches!(
            verify_file_copy(&src, &dst, &hash),
            Err(Error::Integrity(_))
        ));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn checkpoint_roundtrip_and_replace() {
        let dir = test_dir();
        let path = dir.join("checkpoint.json");
        assert_eq!(
            load_checkpoint(&path).unwrap(),
            MigrationCheckpoint::default()
        );
        let checkpoint = MigrationCheckpoint {
            completed_paths: HashSet::from(["photos/a.jpg".into()]),
            bytes_copied: 42,
            last_updated_ms: 123,
        };
        save_checkpoint(&path, &checkpoint).unwrap();
        save_checkpoint(&path, &checkpoint).unwrap();
        assert_eq!(load_checkpoint(&path).unwrap(), checkpoint);
        fs::remove_dir_all(dir).unwrap();
    }
}
