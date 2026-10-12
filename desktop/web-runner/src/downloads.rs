//! Quarantine records keep downloaded bytes out of user files until approval.
use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum QuarantineStatus {
    PendingApproval,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QuarantineRecord {
    pub id: String,
    pub url: String,
    pub filename: String,
    pub quarantine_path: PathBuf,
    pub status: QuarantineStatus,
}

pub struct QuarantineManager {
    quarantine_dir: PathBuf,
    records: HashMap<String, QuarantineRecord>,
}

impl QuarantineManager {
    pub fn new(quarantine_dir: PathBuf) -> Self {
        Self {
            quarantine_dir,
            records: HashMap::new(),
        }
    }

    pub fn register_download(
        &mut self,
        url: &str,
        filename: &str,
    ) -> Result<QuarantineRecord, String> {
        crate::control::sanitize_origin(url)?;
        let filename: String = filename
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || ".-_".contains(*c))
            .take(120)
            .collect();
        let filename = if filename.is_empty() || filename == "." || filename == ".." {
            "download".into()
        } else {
            filename
        };
        fs::create_dir_all(&self.quarantine_dir).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.quarantine_dir, fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
        }
        let id = ulid::Ulid::new().to_string();
        let quarantine_path = self.quarantine_dir.join(format!("{id}-{filename}"));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options.open(&quarantine_path).map_err(|e| e.to_string())?;
        let record = QuarantineRecord {
            id: id.clone(),
            url: url.into(),
            filename,
            quarantine_path,
            status: QuarantineStatus::PendingApproval,
        };
        self.records.insert(id, record.clone());
        Ok(record)
    }

    pub fn approve_download(&mut self, id: &str, target_dir: &Path) -> Result<PathBuf, String> {
        let record = self.records.get_mut(id).ok_or("Unknown download")?;
        if record.status != QuarantineStatus::PendingApproval {
            return Err("Download is not pending".into());
        }
        fs::create_dir_all(target_dir).map_err(|e| e.to_string())?;
        let destination = target_dir.join(&record.filename);
        let mut source = fs::File::open(&record.quarantine_path).map_err(|e| e.to_string())?;
        let mut target = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
            .map_err(|e| e.to_string())?;
        if let Err(error) = std::io::copy(&mut source, &mut target).and_then(|_| target.sync_all())
        {
            let _ = fs::remove_file(&destination);
            return Err(error.to_string());
        }
        fs::remove_file(&record.quarantine_path).map_err(|e| e.to_string())?;
        record.status = QuarantineStatus::Approved;
        Ok(destination)
    }

    pub fn reject_download(&mut self, id: &str) -> Result<(), String> {
        let record = self.records.get_mut(id).ok_or("Unknown download")?;
        if record.status != QuarantineStatus::PendingApproval {
            return Err("Download is not pending".into());
        }
        fs::remove_file(&record.quarantine_path).map_err(|e| e.to_string())?;
        record.status = QuarantineStatus::Rejected;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dirs() -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("hh-web-runner-test-{}", ulid::Ulid::new()));
        (root.join("quarantine"), root.join("files"))
    }
    #[test]
    fn register_creates_placeholder() {
        let (quarantine, _) = dirs();
        let mut manager = QuarantineManager::new(quarantine.clone());
        let record = manager
            .register_download("https://example.com/file", "../file.txt")
            .unwrap();
        assert!(record.quarantine_path.is_file());
        assert_eq!(record.status, QuarantineStatus::PendingApproval);
        assert!(!record.filename.contains('/'));
        fs::remove_dir_all(quarantine.parent().unwrap()).unwrap();
    }
    #[test]
    fn approval_copies_then_removes_quarantine() {
        let (quarantine, files) = dirs();
        let mut manager = QuarantineManager::new(quarantine.clone());
        let record = manager
            .register_download("https://example.com/file", "file.txt")
            .unwrap();
        fs::write(&record.quarantine_path, b"content").unwrap();
        let destination = manager.approve_download(&record.id, &files).unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"content");
        assert!(!record.quarantine_path.exists());
        fs::remove_dir_all(quarantine.parent().unwrap()).unwrap();
    }
    #[test]
    fn rejection_removes_quarantine() {
        let (quarantine, _) = dirs();
        let mut manager = QuarantineManager::new(quarantine.clone());
        let record = manager
            .register_download("https://example.com/file", "file.txt")
            .unwrap();
        manager.reject_download(&record.id).unwrap();
        assert!(!record.quarantine_path.exists());
        fs::remove_dir_all(quarantine.parent().unwrap()).unwrap();
    }
}
