use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[cfg(target_os = "linux")]
use std::collections::HashSet;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
use thiserror::Error;

pub const MIN_INSTALL_SPACE_BYTES: u64 = 32 * 1024 * 1024 * 1024;
pub const WRITE_BLOCK_SIZE: usize = 4 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("Target disk not found: {0}")]
    NotFound(String),
    #[error("Target disk {0} is system root disk; refusing to overwrite")]
    TargetIsRoot(String),
    #[error("Target disk {target} has insufficient capacity: required {required} bytes, available {available} bytes")]
    InsufficientSpace {
        target: String,
        required: u64,
        available: u64,
    },
    #[error("Target disk {0} has mounted partitions: {1:?}")]
    ActiveMounts(String, Vec<String>),
    #[error(
        "Verification failed: image SHA-256 {expected} does not match readback SHA-256 {actual}"
    )]
    HashMismatch { expected: String, actual: String },
    #[error("Storage error: {0}")]
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PartitionInfo {
    pub name: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub mountpoint: Option<String>,
    pub fstype: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiskInfo {
    pub id: String,
    pub path: PathBuf,
    pub model: String,
    pub size_bytes: u64,
    pub removable: bool,
    pub is_system_root: bool,
    pub partitions: Vec<PartitionInfo>,
}

pub fn file_disk(path: &Path) -> Result<DiskInfo, StorageError> {
    let canonical = fs::canonicalize(path)?;
    let metadata = fs::metadata(&canonical)?;
    if !metadata.is_file() {
        return Err(StorageError::Other(
            "mock target must be a regular file".into(),
        ));
    }
    Ok(DiskInfo {
        id: canonical.display().to_string(),
        path: canonical,
        model: "File-based test target".into(),
        size_bytes: metadata.len(),
        removable: false,
        is_system_root: false,
        partitions: vec![],
    })
}

#[cfg(target_os = "linux")]
fn backing_devices(device: &str, found: &mut HashSet<String>) {
    if !found.insert(device.to_string()) {
        return;
    }
    let slaves = Path::new("/sys/dev/block").join(device).join("slaves");
    if let Ok(entries) = fs::read_dir(slaves) {
        for entry in entries.flatten() {
            if let Ok(next) = fs::read_to_string(entry.path().join("dev")) {
                backing_devices(next.trim(), found);
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn mount_records() -> Result<Vec<(String, String, String)>, StorageError> {
    let info = fs::read_to_string("/proc/self/mountinfo")?;
    Ok(info
        .lines()
        .filter_map(|line| {
            let (left, right) = line.split_once(" - ")?;
            let mut fields = left.split_whitespace();
            let device = fields.nth(2)?.to_string();
            let mountpoint = fields.nth(1)?.to_string();
            let fstype = right.split_whitespace().next()?.to_string();
            Some((device, mountpoint, fstype))
        })
        .collect())
}

#[cfg(target_os = "linux")]
pub fn enumerate_disks() -> Result<Vec<DiskInfo>, StorageError> {
    let sysfs = std::env::var_os("HOMEHUB_TEST_SYSFS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/sys/block"));
    let mock = std::env::var_os("HOMEHUB_TEST_SYSFS").is_some();
    let mounts = if mock { vec![] } else { mount_records()? };
    let root_device = mounts
        .iter()
        .find(|(_, path, _)| path == "/")
        .map(|(dev, _, _)| dev.clone());
    let mut root_backing = HashSet::new();
    if let Some(device) = &root_device {
        backing_devices(device, &mut root_backing);
    }
    let mut disks = Vec::new();
    for entry in fs::read_dir(sysfs)? {
        let entry = entry?;
        let id = entry.file_name().to_string_lossy().into_owned();
        if ["loop", "ram", "zram"]
            .iter()
            .any(|prefix| id.starts_with(prefix))
        {
            continue;
        }
        let base = entry.path();
        let sectors = fs::read_to_string(base.join("size"))?
            .trim()
            .parse::<u64>()
            .map_err(|_| StorageError::Other(format!("invalid size for {id}")))?;
        let model = fs::read_to_string(base.join("device/model"))
            .unwrap_or_else(|_| "Unknown".into())
            .trim()
            .to_string();
        let removable = fs::read_to_string(base.join("removable"))
            .unwrap_or_default()
            .trim()
            == "1";
        let mut partitions = Vec::new();
        let mut root = false;
        let disk_dev = fs::read_to_string(base.join("dev"))
            .unwrap_or_default()
            .trim()
            .to_string();
        if root_backing.contains(&disk_dev) {
            root = true;
        }
        for child in fs::read_dir(&base)? {
            let child = child?;
            let name = child.file_name().to_string_lossy().into_owned();
            if !child.path().join("partition").exists() {
                continue;
            }
            let dev = fs::read_to_string(child.path().join("dev"))?
                .trim()
                .to_string();
            let matching = mounts.iter().find(|(mount_dev, _, _)| mount_dev == &dev);
            if root_backing.contains(&dev) {
                root = true;
            }
            let size = fs::read_to_string(child.path().join("size"))?
                .trim()
                .parse::<u64>()
                .map_err(|_| StorageError::Other(format!("invalid size for {name}")))?;
            partitions.push(PartitionInfo {
                name: name.clone(),
                path: PathBuf::from(format!("/dev/{name}")),
                size_bytes: size.saturating_mul(512),
                mountpoint: matching.map(|(_, path, _)| path.clone()),
                fstype: matching.map(|(_, _, ty)| ty.clone()),
            });
        }
        disks.push(DiskInfo {
            id: id.clone(),
            path: if mock {
                base
            } else {
                PathBuf::from(format!("/dev/{id}"))
            },
            model,
            size_bytes: sectors.saturating_mul(512),
            removable,
            is_system_root: root,
            partitions,
        });
    }
    disks.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(disks)
}

#[cfg(not(target_os = "linux"))]
pub fn enumerate_disks() -> Result<Vec<DiskInfo>, StorageError> {
    // Real disk selection is Linux-only. Explicit regular files are accepted by the flow.
    Ok(Vec::new())
}

pub fn validate_target_disk(
    disk: &DiskInfo,
    image_size: u64,
    allow_small_for_test: bool,
) -> Result<(), StorageError> {
    if disk.is_system_root {
        return Err(StorageError::TargetIsRoot(disk.id.clone()));
    }
    let required = if allow_small_for_test {
        image_size
    } else {
        image_size.max(MIN_INSTALL_SPACE_BYTES)
    };
    if disk.size_bytes < required {
        return Err(StorageError::InsufficientSpace {
            target: disk.id.clone(),
            required,
            available: disk.size_bytes,
        });
    }
    let active: Vec<_> = disk
        .partitions
        .iter()
        .filter_map(|p| p.mountpoint.clone())
        .collect();
    if !active.is_empty() {
        return Err(StorageError::ActiveMounts(disk.id.clone(), active));
    }
    Ok(())
}

pub fn write_image_with_verify<R: Read, W: Read + Write + Seek>(
    mut source: R,
    mut target: W,
    image_size: u64,
    dry_run: bool,
) -> Result<String, StorageError> {
    let mut source_hash = Sha256::new();
    let mut buffer = vec![0; WRITE_BLOCK_SIZE];
    let mut remaining = image_size;
    while remaining > 0 {
        let count =
            usize::try_from(remaining.min(WRITE_BLOCK_SIZE as u64)).unwrap_or(WRITE_BLOCK_SIZE);
        source.read_exact(&mut buffer[..count])?;
        source_hash.update(&buffer[..count]);
        if !dry_run {
            target.write_all(&buffer[..count])?;
        }
        remaining -= count as u64;
    }
    let expected = format!("{:x}", source_hash.finalize());
    if dry_run {
        return Ok(expected);
    }
    target.flush()?;
    target.seek(SeekFrom::Start(0))?;
    let mut readback = Sha256::new();
    remaining = image_size;
    while remaining > 0 {
        let count =
            usize::try_from(remaining.min(WRITE_BLOCK_SIZE as u64)).unwrap_or(WRITE_BLOCK_SIZE);
        target.read_exact(&mut buffer[..count])?;
        readback.update(&buffer[..count]);
        remaining -= count as u64;
    }
    let actual = format!("{:x}", readback.finalize());
    if expected != actual {
        return Err(StorageError::HashMismatch { expected, actual });
    }
    Ok(expected)
}

pub fn open_target(path: &Path) -> Result<File, StorageError> {
    Ok(OpenOptions::new().read(true).write(true).open(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn disk(size: u64) -> DiskInfo {
        DiskInfo {
            id: "mock".into(),
            path: "mock".into(),
            model: "Mock".into(),
            size_bytes: size,
            removable: false,
            is_system_root: false,
            partitions: vec![],
        }
    }

    #[test]
    fn validation_checks_root_size_and_mounts() {
        let mut target = disk(8);
        assert!(validate_target_disk(&target, 4, true).is_ok());
        assert!(matches!(
            validate_target_disk(&target, 9, true),
            Err(StorageError::InsufficientSpace { .. })
        ));
        assert!(matches!(
            validate_target_disk(&target, 4, false),
            Err(StorageError::InsufficientSpace { .. })
        ));
        target.is_system_root = true;
        assert!(matches!(
            validate_target_disk(&target, 4, true),
            Err(StorageError::TargetIsRoot(_))
        ));
        target.is_system_root = false;
        target.partitions.push(PartitionInfo {
            name: "p1".into(),
            path: "p1".into(),
            size_bytes: 4,
            mountpoint: Some("/mnt".into()),
            fstype: None,
        });
        assert!(matches!(
            validate_target_disk(&target, 4, true),
            Err(StorageError::ActiveMounts(_, _))
        ));
    }

    #[test]
    fn dry_run_does_not_write() {
        let mut target = Cursor::new(vec![0x55; 4]);
        let hash = write_image_with_verify(&b"test"[..], &mut target, 4, true).unwrap();
        assert_eq!(hash, format!("{:x}", Sha256::digest(b"test")));
        assert_eq!(target.into_inner(), vec![0x55; 4]);
    }

    #[test]
    fn writes_and_verifies_readback() {
        let mut target = Cursor::new(vec![0; 4]);
        write_image_with_verify(&b"test"[..], &mut target, 4, false).unwrap();
        assert_eq!(target.into_inner(), b"test");
    }

    #[test]
    fn detects_corrupted_readback() {
        struct Corrupt(Cursor<Vec<u8>>);
        impl Read for Corrupt {
            fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
                self.0.read(b)
            }
        }
        impl Write for Corrupt {
            fn write(&mut self, b: &[u8]) -> io::Result<usize> {
                self.0.write(&vec![0; b.len()])
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        impl Seek for Corrupt {
            fn seek(&mut self, p: SeekFrom) -> io::Result<u64> {
                self.0.seek(p)
            }
        }
        assert!(matches!(
            write_image_with_verify(&b"test"[..], Corrupt(Cursor::new(vec![0; 4])), 4, false),
            Err(StorageError::HashMismatch { .. })
        ));
    }
}
