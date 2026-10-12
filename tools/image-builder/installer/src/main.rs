pub mod flow;
pub mod recovery;
pub mod storage;

use clap::{Parser, ValueEnum};
use flow::{InstallMode, InstallPlan, InstallerEngine};
use std::{
    io::{self, Write},
    path::PathBuf,
};

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeArg {
    Replace,
    KeepWindows,
}

impl From<ModeArg> for InstallMode {
    fn from(mode: ModeArg) -> Self {
        match mode {
            ModeArg::Replace => Self::Replace,
            ModeArg::KeepWindows => Self::KeepWindows,
        }
    }
}

#[derive(Parser, Debug)]
#[command(
    name = "homehub-installer",
    about = "Home Hub OS Native Live Installer Engine"
)]
pub struct Cli {
    #[arg(long)]
    pub audit: bool,
    #[arg(long)]
    pub list_disks: bool,
    #[arg(long)]
    pub target: Option<PathBuf>,
    #[arg(long)]
    pub image: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = ModeArg::Replace)]
    pub mode: ModeArg,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub yes: bool,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub allow_small_target: bool,
    #[arg(long)]
    pub recovery_dir: Option<PathBuf>,
}

fn print_value<T: serde::Serialize + std::fmt::Debug>(value: &T, json: bool) -> Result<(), String> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(value).map_err(|e| e.to_string())?
        );
    } else {
        println!("{value:#?}");
    }
    Ok(())
}

fn run(cli: Cli) -> Result<bool, String> {
    if cli.audit {
        print_value(&flow::audit_system().map_err(|e| e.to_string())?, cli.json)?;
        return Ok(true);
    }
    if cli.list_disks {
        print_value(
            &storage::enumerate_disks().map_err(|e| e.to_string())?,
            cli.json,
        )?;
        return Ok(true);
    }
    let target = cli.target.ok_or("--target is required".to_string())?;
    let image = cli.image.ok_or("--image is required".to_string())?;
    let canonical = std::fs::canonicalize(&target).map_err(|e| e.to_string())?;
    let disk = if canonical.is_file() {
        storage::file_disk(&canonical).map_err(|e| e.to_string())?
    } else {
        storage::enumerate_disks()
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|disk| disk.path == canonical)
            .ok_or_else(|| format!("target disk not found: {}", canonical.display()))?
    };
    eprintln!(
        "Target: {} | model: {} | size: {} bytes",
        disk.path.display(),
        disk.model,
        disk.size_bytes
    );
    let confirmed = if cli.yes {
        true
    } else {
        eprint!("Type ERASE to confirm installation on this target: ");
        io::stderr().flush().map_err(|e| e.to_string())?;
        let mut response = String::new();
        io::stdin()
            .read_line(&mut response)
            .map_err(|e| e.to_string())?;
        response.trim() == "ERASE"
    };
    let plan = InstallPlan {
        target_path: target,
        image_path: image,
        mode: cli.mode.into(),
        dry_run: cli.dry_run,
        confirmed,
        allow_small_for_test: cli.allow_small_target,
        recovery_dir: cli.recovery_dir,
    };
    let report = InstallerEngine::new(plan)
        .execute()
        .map_err(|e| e.to_string())?;
    print_value(&report, cli.json)?;
    Ok(report.success)
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    match run(Cli::parse()) {
        Ok(true) => {}
        Ok(false) => std::process::exit(1),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
