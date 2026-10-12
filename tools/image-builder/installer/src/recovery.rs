use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

const BACKUP_BYTES: usize = 1024 * 1024;

#[derive(Debug, Error)]
pub enum RecoveryError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("Backup error: {0}")]
    Backup(String),
    #[error("Restore error: {0}")]
    Restore(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionTableBackup {
    pub target_path: String,
    pub timestamp: u64,
    pub sector_bytes_hex: String,
    pub length: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecoveryJournal {
    pub entries: Vec<JournalEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub step: String,
    pub timestamp: u64,
    pub detail: String,
    pub success: bool,
}

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn backup_partition_table<R: Read + Seek>(
    mut reader: R,
    target_path: &str,
    backup_file: &Path,
) -> Result<PartitionTableBackup, RecoveryError> {
    reader.seek(SeekFrom::Start(0))?;
    let mut bytes = vec![0; BACKUP_BYTES];
    let mut length = 0;
    while length < bytes.len() {
        let n = reader.read(&mut bytes[length..])?;
        if n == 0 {
            break;
        }
        length += n;
    }
    bytes.truncate(length);
    if bytes.is_empty() {
        return Err(RecoveryError::Backup("empty target".into()));
    }
    let backup = PartitionTableBackup {
        target_path: target_path.into(),
        timestamp: timestamp(),
        sector_bytes_hex: hex_encode(&bytes),
        length,
    };
    let json = serde_json::to_vec(&backup).map_err(|e| RecoveryError::Backup(e.to_string()))?;
    fs::write(backup_file, json)?;
    Ok(backup)
}

pub fn restore_partition_table<W: Write + Seek>(
    mut writer: W,
    backup: &PartitionTableBackup,
) -> Result<(), RecoveryError> {
    let bytes = hex_decode(&backup.sector_bytes_hex)?;
    if bytes.len() != backup.length || bytes.len() > BACKUP_BYTES {
        return Err(RecoveryError::Restore("invalid backup length".into()));
    }
    writer.seek(SeekFrom::Start(0))?;
    writer.write_all(&bytes)?;
    writer.flush()?;
    Ok(())
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 15) as usize] as char);
    }
    out
}

fn hex_decode(hex: &str) -> Result<Vec<u8>, RecoveryError> {
    let bytes = hex.as_bytes();
    if bytes.len() & 1 != 0 {
        return Err(RecoveryError::Restore("invalid backup hex".into()));
    }
    bytes
        .iter()
        .step_by(2)
        .enumerate()
        .map(|(index, first)| {
            let hi = (*first as char)
                .to_digit(16)
                .ok_or_else(|| RecoveryError::Restore("invalid backup hex".into()))?;
            let lo = (bytes[index * 2 + 1] as char)
                .to_digit(16)
                .ok_or_else(|| RecoveryError::Restore("invalid backup hex".into()))?;
            Ok(((hi << 4) | lo) as u8)
        })
        .collect()
}

impl RecoveryJournal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, step: &str, detail: &str, success: bool) {
        self.entries.push(JournalEntry {
            step: step.into(),
            timestamp: timestamp(),
            detail: detail.into(),
            success,
        });
    }

    pub fn save_to(&self, path: &Path) -> Result<(), RecoveryError> {
        let bytes =
            serde_json::to_vec_pretty(self).map_err(|e| RecoveryError::Backup(e.to_string()))?;
        fs::write(path, bytes)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn backup_restore_roundtrip() {
        let original = vec![0xa5; 512];
        let backup_path =
            std::env::temp_dir().join(format!("homehub-backup-{}", std::process::id()));
        let backup = backup_partition_table(Cursor::new(&original), "mock", &backup_path).unwrap();
        let mut target = Cursor::new(vec![0; 512]);
        restore_partition_table(&mut target, &backup).unwrap();
        assert_eq!(target.into_inner(), original);
        fs::remove_file(backup_path).unwrap();
    }

    #[test]
    fn journal_serializes() {
        let mut journal = RecoveryJournal::new();
        journal.record("audit", "ok", true);
        assert_eq!(
            serde_json::to_value(&journal).unwrap()["entries"][0]["step"],
            "audit"
        );
    }
}
