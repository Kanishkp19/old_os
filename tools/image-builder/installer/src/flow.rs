use crate::{
    recovery::{self, PartitionTableBackup, RecoveryJournal},
    storage::{self, DiskInfo, StorageError},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{Seek, SeekFrom},
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FlowError {
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("Recovery error: {0}")]
    Recovery(#[from] recovery::RecoveryError),
    #[error("Flow error: {0}")]
    State(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstallMode {
    Replace,
    KeepWindows,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstallerPhase {
    Audit,
    TargetSelection,
    Confirmation,
    ImageWrite,
    PostInstall,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareAudit {
    pub total_ram_bytes: u64,
    pub meets_ram_requirement: bool,
    pub is_uefi: bool,
    pub detected_disks: Vec<DiskInfo>,
    pub qualified_targets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallPlan {
    pub target_path: PathBuf,
    pub image_path: PathBuf,
    pub mode: InstallMode,
    pub dry_run: bool,
    pub confirmed: bool,
    pub allow_small_for_test: bool,
    pub recovery_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallReport {
    pub phase: InstallerPhase,
    pub success: bool,
    pub target: String,
    pub mode: InstallMode,
    pub dry_run: bool,
    pub image_sha256: Option<String>,
    pub bytes_written: u64,
    pub error: Option<String>,
}

pub fn audit_system() -> Result<HardwareAudit, FlowError> {
    #[cfg(target_os = "linux")]
    let total_ram_bytes = {
        let info = fs::read_to_string("/proc/meminfo")?;
        info.lines()
            .find_map(|line| {
                line.strip_prefix("MemTotal:")
                    .and_then(|rest| rest.split_whitespace().next())
                    .and_then(|kb| kb.parse::<u64>().ok())
            })
            .ok_or_else(|| FlowError::State("cannot measure RAM".into()))?
            * 1024
    };
    #[cfg(not(target_os = "linux"))]
    let total_ram_bytes = 0;
    let detected_disks = storage::enumerate_disks()?;
    let qualified_targets = detected_disks
        .iter()
        .filter(|disk| storage::validate_target_disk(disk, 0, false).is_ok())
        .map(|disk| disk.id.clone())
        .collect();
    Ok(HardwareAudit {
        total_ram_bytes,
        meets_ram_requirement: total_ram_bytes >= 2 * 1024 * 1024 * 1024,
        is_uefi: cfg!(target_os = "linux") && Path::new("/sys/firmware/efi").exists(),
        detected_disks,
        qualified_targets,
    })
}

pub struct InstallerEngine {
    pub plan: InstallPlan,
    pub journal: RecoveryJournal,
}

impl InstallerEngine {
    pub fn new(plan: InstallPlan) -> Self {
        Self {
            plan,
            journal: RecoveryJournal::new(),
        }
    }

    pub fn execute(&mut self) -> Result<InstallReport, FlowError> {
        let mut report = InstallReport {
            phase: InstallerPhase::Audit,
            success: false,
            target: self.plan.target_path.display().to_string(),
            mode: self.plan.mode,
            dry_run: self.plan.dry_run,
            image_sha256: None,
            bytes_written: 0,
            error: None,
        };
        if let Err(error) = self.execute_inner(&mut report) {
            self.journal.record("failed", &error.to_string(), false);
            report.phase = InstallerPhase::Failed;
            report.error = Some(error.to_string());
        } else {
            report.phase = InstallerPhase::Completed;
            report.success = true;
            self.journal.record("completed", "image verified", true);
        }
        let journal_dir = self.plan.recovery_dir.clone().or_else(|| {
            if self.plan.dry_run {
                None
            } else {
                Some(Self::default_recovery_dir())
            }
        });
        if let Some(dir) = journal_dir {
            fs::create_dir_all(&dir)?;
            self.journal.save_to(&dir.join("journal.json"))?;
        }
        Ok(report)
    }

    fn execute_inner(&mut self, report: &mut InstallReport) -> Result<(), FlowError> {
        let image_size = fs::metadata(&self.plan.image_path)?.len();
        if image_size == 0 {
            return Err(FlowError::State("image is empty".into()));
        }
        let image = File::open(&self.plan.image_path)?;
        let target = fs::canonicalize(&self.plan.target_path)?;
        if fs::canonicalize(&self.plan.image_path)? == target {
            return Err(FlowError::State(
                "image and target must be different".into(),
            ));
        }
        let metadata = fs::metadata(&target)?;
        let file_target = metadata.is_file();
        if self.plan.allow_small_for_test && !file_target {
            return Err(FlowError::State(
                "--allow-small-target requires a regular file".into(),
            ));
        }
        if file_target && !self.plan.allow_small_for_test {
            return Err(FlowError::State(
                "file targets require --allow-small-target".into(),
            ));
        }
        let disk = if file_target {
            storage::file_disk(&target)?
        } else {
            storage::enumerate_disks()?
                .into_iter()
                .find(|disk| disk.path == target)
                .ok_or_else(|| StorageError::NotFound(target.display().to_string()))?
        };
        report.phase = InstallerPhase::TargetSelection;
        storage::validate_target_disk(&disk, image_size, self.plan.allow_small_for_test)?;
        if self.plan.mode == InstallMode::KeepWindows {
            return Err(FlowError::State(
                "keep-windows requires a partition-preserving installer".into(),
            ));
        }
        self.journal.record(
            "target",
            &format!(
                "{} ({}, {} bytes)",
                disk.path.display(),
                disk.model,
                disk.size_bytes
            ),
            true,
        );
        report.phase = InstallerPhase::Confirmation;
        if !self.plan.confirmed {
            return Err(FlowError::State(
                "Action requires explicit confirmation".into(),
            ));
        }
        report.phase = InstallerPhase::ImageWrite;
        if self.plan.dry_run {
            report.image_sha256 = Some(storage::write_image_with_verify(
                image,
                std::io::Cursor::new(Vec::new()),
                image_size,
                true,
            )?);
            self.journal
                .record("dry-run", "source hashed; target untouched", true);
            report.phase = InstallerPhase::PostInstall;
            return Ok(());
        }
        let recovery_dir = self
            .plan
            .recovery_dir
            .clone()
            .unwrap_or_else(Self::default_recovery_dir);
        fs::create_dir_all(&recovery_dir)?;
        let mut target_file = storage::open_target(&target)?;
        let backup = recovery::backup_partition_table(
            &mut target_file,
            &target.display().to_string(),
            &recovery_dir.join("partition-backup.json"),
        )?;
        self.journal
            .record("backup", &recovery_dir.display().to_string(), true);
        target_file.seek(SeekFrom::Start(0))?;
        let write_result =
            storage::write_image_with_verify(image, &mut target_file, image_size, false);
        match write_result {
            Ok(hash) => {
                report.image_sha256 = Some(hash);
                report.bytes_written = image_size;
                self.journal
                    .record("write", "SHA-256 readback verified", true);
            }
            Err(error) => {
                self.restore_after_failure(&mut target_file, &backup, &recovery_dir)?;
                return Err(error.into());
            }
        }
        report.phase = InstallerPhase::PostInstall;
        self.configure_boot(&disk)?;
        self.journal.save_to(&recovery_dir.join("journal.json"))?;
        Ok(())
    }

    fn restore_after_failure(
        &mut self,
        target: &mut File,
        backup: &PartitionTableBackup,
        dir: &Path,
    ) -> Result<(), FlowError> {
        let result = recovery::restore_partition_table(target, backup);
        self.journal.record(
            "rollback",
            if result.is_ok() {
                "partition header restored"
            } else {
                "partition header restore failed"
            },
            result.is_ok(),
        );
        self.journal.save_to(&dir.join("journal.json"))?;
        result?;
        Ok(())
    }

    fn default_recovery_dir() -> PathBuf {
        std::env::temp_dir().join(format!("homehub-installer-recovery-{}", std::process::id()))
    }

    fn configure_boot(&mut self, disk: &DiskInfo) -> Result<(), FlowError> {
        #[cfg(target_os = "linux")]
        if !self.plan.allow_small_for_test && Path::new("/sys/firmware/efi").exists() {
            let status = std::process::Command::new("efibootmgr")
                .args(["-c", "-d"])
                .arg(&disk.path)
                .args([
                    "-p",
                    "1",
                    "-L",
                    "Home Hub OS",
                    "-l",
                    "\\EFI\\BOOT\\BOOTX64.EFI",
                ])
                .status()?;
            if !status.success() {
                return Err(FlowError::State(
                    "EFI boot entry configuration failed".into(),
                ));
            }
            self.journal
                .record("boot", "EFI boot entry configured", true);
        } else {
            self.journal
                .record("boot", "image boot assets retained", true);
        }
        #[cfg(not(target_os = "linux"))]
        self.journal.record(
            "boot",
            &format!("file target {}", disk.path.display()),
            true,
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn fixture(confirmed: bool) -> (InstallPlan, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("homehub-flow-{}-{}", std::process::id(), confirmed));
        fs::create_dir_all(&dir).unwrap();
        let image = dir.join("image");
        let target = dir.join("target");
        File::create(&image).unwrap().write_all(b"test").unwrap();
        File::create(&target)
            .unwrap()
            .write_all(&[0x55; 8])
            .unwrap();
        (
            InstallPlan {
                target_path: target,
                image_path: image,
                mode: InstallMode::Replace,
                dry_run: true,
                confirmed,
                allow_small_for_test: true,
                recovery_dir: Some(dir.clone()),
            },
            dir,
        )
    }

    #[test]
    fn dry_run_preserves_target() {
        let (plan, dir) = fixture(true);
        let before = fs::read(&plan.target_path).unwrap();
        let report = InstallerEngine::new(plan.clone()).execute().unwrap();
        assert!(report.success);
        assert_eq!(report.bytes_written, 0);
        assert_eq!(fs::read(&plan.target_path).unwrap(), before);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unconfirmed_install_fails() {
        let (plan, dir) = fixture(false);
        let report = InstallerEngine::new(plan).execute().unwrap();
        assert!(!report.success);
        assert!(report.error.unwrap().contains("explicit confirmation"));
        fs::remove_dir_all(dir).unwrap();
    }
}
