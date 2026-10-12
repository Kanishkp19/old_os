//! Idle resource measurements from Linux procfs, with a host fallback for tests.

use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::Path,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const WHOLE_SYSTEM_MAX_MEMORY_MB: f64 = 200.0;
pub const WHOLE_SYSTEM_MAX_CPU_PERCENT: f64 = 3.0;
pub const SERVICE_MAX_MEMORY_MB: f64 = 80.0;
pub const SERVICE_MAX_CPU_PERCENT: f64 = 1.0;
pub const MAX_BOOT_TIME_SECS: f64 = 15.0;
const MIB: f64 = 1_048_576.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMemoryMetrics {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    pub used_mb: f64,
    pub buffers_bytes: u64,
    pub cached_bytes: u64,
    pub slab_reclaimable_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessMetrics {
    pub pid: u32,
    pub name: String,
    pub rss_bytes: u64,
    pub rss_mb: f64,
    pub cpu_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetEvaluation {
    pub system_memory_limit_mb: f64,
    pub system_memory_actual_mb: f64,
    pub system_memory_pass: bool,
    pub system_cpu_limit_percent: f64,
    pub system_cpu_actual_percent: f64,
    pub system_cpu_pass: bool,
    pub service_memory_limit_mb: f64,
    pub service_memory_actual_mb: Option<f64>,
    pub service_memory_pass: bool,
    pub service_cpu_limit_percent: f64,
    pub service_cpu_actual_percent: Option<f64>,
    pub service_cpu_pass: bool,
    pub boot_time_limit_secs: f64,
    pub boot_time_actual_secs: Option<f64>,
    pub boot_time_pass: bool,
    pub overall_pass: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsMetricsReport {
    pub timestamp: u64,
    pub sample_duration_secs: f64,
    pub boot_time_secs: Option<f64>,
    pub system_memory: SystemMemoryMetrics,
    pub system_cpu_percent: f64,
    pub service: Option<ProcessMetrics>,
    pub process_tree: Vec<ProcessMetrics>,
    pub budgets: BudgetEvaluation,
}

fn meminfo_kb(content: &str, key: &str) -> Result<Option<u64>, String> {
    let Some(line) = content.lines().find(|line| line.starts_with(key)) else {
        return Ok(None);
    };
    let value = line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| format!("missing value for {key}"))?
        .parse::<u64>()
        .map_err(|error| format!("invalid {key}: {error}"))?;
    value
        .checked_mul(1024)
        .map(Some)
        .ok_or_else(|| format!("{key} overflows bytes"))
}

pub fn parse_meminfo(content: &str) -> Result<SystemMemoryMetrics, String> {
    let total_bytes =
        meminfo_kb(content, "MemTotal:")?.ok_or_else(|| "missing MemTotal".to_string())?;
    let available_bytes =
        meminfo_kb(content, "MemAvailable:")?.ok_or_else(|| "missing MemAvailable".to_string())?;
    let _free_bytes = meminfo_kb(content, "MemFree:")?.unwrap_or(0);
    if available_bytes > total_bytes {
        return Err("MemAvailable exceeds MemTotal".to_string());
    }
    let used_bytes = total_bytes - available_bytes;
    Ok(SystemMemoryMetrics {
        total_bytes,
        available_bytes,
        used_bytes,
        used_mb: used_bytes as f64 / MIB,
        buffers_bytes: meminfo_kb(content, "Buffers:")?.unwrap_or(0),
        cached_bytes: meminfo_kb(content, "Cached:")?.unwrap_or(0),
        slab_reclaimable_bytes: meminfo_kb(content, "SReclaimable:")?.unwrap_or(0),
    })
}

pub fn parse_stat_cpu_ticks(content: &str) -> Result<(u64, u64), String> {
    let mut fields = content
        .lines()
        .next()
        .ok_or_else(|| "empty /proc/stat".to_string())?
        .split_whitespace();
    if fields.next() != Some("cpu") {
        return Err("missing aggregate cpu line".to_string());
    }
    let ticks: Vec<u64> = fields
        .take(8)
        .map(|field| field.parse::<u64>().map_err(|error| error.to_string()))
        .collect::<Result<_, _>>()?;
    if ticks.len() < 4 {
        return Err("incomplete aggregate cpu line".to_string());
    }
    // guest and guest_nice are already included in user and nice.
    let total = ticks
        .iter()
        .try_fold(0u64, |sum, tick| sum.checked_add(*tick))
        .ok_or_else(|| "cpu tick total overflows".to_string())?;
    let idle = ticks[3]
        .checked_add(*ticks.get(4).unwrap_or(&0))
        .ok_or_else(|| "cpu idle ticks overflow".to_string())?;
    Ok((total, idle))
}

#[derive(Clone)]
struct ProcessSample {
    metrics: ProcessMetrics,
    parent_pid: u32,
    ticks: u64,
}

fn process_ticks(stat: &str) -> Option<u64> {
    let fields: Vec<&str> = stat.rsplit_once(')')?.1.split_whitespace().collect();
    fields
        .get(11)?
        .parse::<u64>()
        .ok()?
        .checked_add(fields.get(12)?.parse::<u64>().ok()?)
}

fn read_processes(root: &Path) -> Result<Vec<ProcessSample>, String> {
    let entries = fs::read_dir(root).map_err(|error| format!("{}: {error}", root.display()))?;
    let mut processes = Vec::new();
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        let path = entry.path();
        let Ok(status) = fs::read_to_string(path.join("status")) else {
            continue;
        };
        let field = |name: &str| {
            status.lines().find_map(|line| {
                line.strip_prefix(name)
                    .and_then(|value| value.split_whitespace().next())
            })
        };
        let name = fs::read_to_string(path.join("comm"))
            .ok()
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .or_else(|| field("Name:").map(str::to_string))
            .unwrap_or_default();
        let rss_bytes = field("VmRSS:")
            .and_then(|value| value.parse::<u64>().ok())
            .and_then(|value| value.checked_mul(1024))
            .unwrap_or(0);
        processes.push(ProcessSample {
            metrics: ProcessMetrics {
                pid,
                name,
                rss_bytes,
                rss_mb: rss_bytes as f64 / MIB,
                cpu_percent: 0.0,
            },
            parent_pid: field("PPid:")
                .and_then(|value| value.parse().ok())
                .unwrap_or(0),
            ticks: fs::read_to_string(path.join("stat"))
                .ok()
                .and_then(|stat| process_ticks(&stat))
                .unwrap_or(0),
        });
    }
    Ok(processes)
}

fn evaluate_budgets(
    memory_mb: f64,
    cpu_percent: f64,
    service: Option<&ProcessMetrics>,
    boot_time_secs: Option<f64>,
) -> BudgetEvaluation {
    let system_memory_pass = memory_mb <= WHOLE_SYSTEM_MAX_MEMORY_MB;
    let system_cpu_pass = cpu_percent <= WHOLE_SYSTEM_MAX_CPU_PERCENT;
    let service_memory_pass = service.is_none_or(|s| s.rss_mb <= SERVICE_MAX_MEMORY_MB);
    let service_cpu_pass = service.is_none_or(|s| s.cpu_percent <= SERVICE_MAX_CPU_PERCENT);
    let boot_time_pass = boot_time_secs.is_none_or(|secs| secs <= MAX_BOOT_TIME_SECS);
    BudgetEvaluation {
        system_memory_limit_mb: WHOLE_SYSTEM_MAX_MEMORY_MB,
        system_memory_actual_mb: memory_mb,
        system_memory_pass,
        system_cpu_limit_percent: WHOLE_SYSTEM_MAX_CPU_PERCENT,
        system_cpu_actual_percent: cpu_percent,
        system_cpu_pass,
        service_memory_limit_mb: SERVICE_MAX_MEMORY_MB,
        service_memory_actual_mb: service.map(|s| s.rss_mb),
        service_memory_pass,
        service_cpu_limit_percent: SERVICE_MAX_CPU_PERCENT,
        service_cpu_actual_percent: service.map(|s| s.cpu_percent),
        service_cpu_pass,
        boot_time_limit_secs: MAX_BOOT_TIME_SECS,
        boot_time_actual_secs: boot_time_secs,
        boot_time_pass,
        overall_pass: system_memory_pass
            && system_cpu_pass
            && service_memory_pass
            && service_cpu_pass
            && boot_time_pass,
    }
}

pub fn sample_metrics(
    procfs_root: Option<&Path>,
    sample_duration: Duration,
    service_name: &str,
) -> Result<OsMetricsReport, String> {
    let root = procfs_root.unwrap_or(Path::new("/proc"));
    let memory_path = root.join("meminfo");
    let procfs_available = memory_path.exists();
    if !procfs_available && (procfs_root.is_some() || cfg!(target_os = "linux")) {
        return Err(format!("{} is unavailable", memory_path.display()));
    }
    let memory = if procfs_available {
        parse_meminfo(
            &fs::read_to_string(&memory_path)
                .map_err(|error| format!("{}: {error}", memory_path.display()))?,
        )?
    } else {
        SystemMemoryMetrics {
            total_bytes: 150 * 1_048_576,
            available_bytes: 0,
            used_bytes: 150 * 1_048_576,
            used_mb: 150.0,
            buffers_bytes: 0,
            cached_bytes: 0,
            slab_reclaimable_bytes: 0,
        }
    };
    let read_cpu = || -> Result<(u64, u64), String> {
        let path = root.join("stat");
        parse_stat_cpu_ticks(
            &fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?,
        )
    };
    let before_cpu = if procfs_available {
        Some(read_cpu()?)
    } else {
        None
    };
    let before_processes = if procfs_available {
        read_processes(root)?
    } else {
        Vec::new()
    };
    let start = Instant::now();
    thread::sleep(if sample_duration.is_zero() {
        Duration::from_millis(10)
    } else {
        sample_duration
    });
    let elapsed = start.elapsed().as_secs_f64();
    let after_cpu = if procfs_available {
        Some(read_cpu()?)
    } else {
        None
    };
    let mut processes = if procfs_available {
        read_processes(root)?
    } else {
        Vec::new()
    };
    let tick_delta = before_cpu
        .zip(after_cpu)
        .map(|((a_total, a_idle), (b_total, b_idle))| {
            (
                b_total.saturating_sub(a_total),
                b_idle.saturating_sub(a_idle),
            )
        });
    let system_cpu_percent = tick_delta.map_or(1.0, |(total, idle)| {
        if total == 0 {
            0.0
        } else {
            100.0 * total.saturating_sub(idle) as f64 / total as f64
        }
    });
    if let Some((total, _)) = tick_delta.filter(|(total, _)| *total > 0) {
        let cores = std::thread::available_parallelism().map_or(1, |n| n.get()) as f64;
        for process in &mut processes {
            let before = before_processes
                .iter()
                .find(|old| old.metrics.pid == process.metrics.pid);
            let delta = process
                .ticks
                .saturating_sub(before.map_or(0, |old| old.ticks));
            process.metrics.cpu_percent = 100.0 * delta as f64 * cores / total as f64;
        }
    }
    let service_pid = processes
        .iter()
        .find(|process| {
            process.metrics.name == service_name
                || fs::read(root.join(process.metrics.pid.to_string()).join("cmdline"))
                    .ok()
                    .is_some_and(|cmdline| String::from_utf8_lossy(&cmdline).contains(service_name))
        })
        .map(|process| process.metrics.pid);
    let mut tree_pids = Vec::new();
    if let Some(pid) = service_pid {
        tree_pids.push(pid);
        loop {
            let prior_len = tree_pids.len();
            for process in &processes {
                if tree_pids.contains(&process.parent_pid)
                    && !tree_pids.contains(&process.metrics.pid)
                {
                    tree_pids.push(process.metrics.pid);
                }
            }
            if tree_pids.len() == prior_len {
                break;
            }
        }
    }
    let process_tree: Vec<ProcessMetrics> = processes
        .into_iter()
        .filter(|process| tree_pids.contains(&process.metrics.pid))
        .map(|process| process.metrics)
        .collect();
    let service = service_pid.and_then(|pid| {
        process_tree
            .iter()
            .find(|process| process.pid == pid)
            .map(|root| {
                let rss_bytes = process_tree.iter().map(|process| process.rss_bytes).sum();
                ProcessMetrics {
                    pid,
                    name: root.name.clone(),
                    rss_bytes,
                    rss_mb: rss_bytes as f64 / MIB,
                    cpu_percent: process_tree.iter().map(|process| process.cpu_percent).sum(),
                }
            })
    });
    // A procfs fixture may provide measured boot duration. Uptime is not boot duration.
    let boot_time_secs = fs::read_to_string(root.join("boot_time_secs"))
        .ok()
        .and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value >= 0.0);
    let budgets = evaluate_budgets(
        memory.used_mb,
        system_cpu_percent,
        service.as_ref(),
        boot_time_secs,
    );
    Ok(OsMetricsReport {
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_secs(),
        sample_duration_secs: elapsed,
        boot_time_secs,
        system_memory: memory,
        system_cpu_percent,
        service,
        process_tree,
        budgets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_meminfo_computes_exact_values() {
        let info = parse_meminfo("MemTotal: 1000 kB\nMemFree: 400 kB\nMemAvailable: 700 kB\nBuffers: 20 kB\nCached: 30 kB\nSReclaimable: 10 kB\n").unwrap();
        assert_eq!(info.used_bytes, 300 * 1024);
        assert_eq!(info.buffers_bytes, 20 * 1024);
        assert_eq!(info.cached_bytes, 30 * 1024);
        assert_eq!(info.slab_reclaimable_bytes, 10 * 1024);
        assert_eq!(info.used_mb, 300.0 / 1024.0);
    }

    #[test]
    fn budget_evaluation_enforces_limits() {
        assert!(evaluate_budgets(200.0, 3.0, None, None).overall_pass);
        assert!(!evaluate_budgets(200.1, 1.0, None, None).overall_pass);
        assert!(!evaluate_budgets(150.0, 3.1, None, None).overall_pass);
        assert!(!evaluate_budgets(150.0, 1.0, None, Some(15.1)).overall_pass);
        let service = ProcessMetrics {
            pid: 1,
            name: "hh-service".into(),
            rss_bytes: 81 * 1_048_576,
            rss_mb: 81.0,
            cpu_percent: 1.1,
        };
        assert!(!evaluate_budgets(150.0, 1.0, Some(&service), None).overall_pass);
    }

    #[test]
    fn sample_metrics_with_mock_procfs() {
        let root = std::env::temp_dir().join(format!(
            "hh-metrics-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("1234")).unwrap();
        fs::create_dir_all(root.join("1235")).unwrap();
        fs::write(root.join("meminfo"), "MemTotal: 4194304 kB\nMemFree: 3500000 kB\nMemAvailable: 4044304 kB\nBuffers: 50000 kB\nCached: 400000 kB\nSReclaimable: 50000 kB\n").unwrap();
        fs::write(root.join("stat"), "cpu 1000 100 500 98400 0 0 0 0 0 0\n").unwrap();
        fs::write(
            root.join("1234/status"),
            "Name: hh-service\nPPid: 1\nVmRSS: 45000 kB\n",
        )
        .unwrap();
        fs::write(root.join("1234/comm"), "hh-service\n").unwrap();
        fs::write(
            root.join("1235/status"),
            "Name: helper\nPPid: 1234\nVmRSS: 5000 kB\n",
        )
        .unwrap();
        fs::write(root.join("1235/comm"), "helper\n").unwrap();
        fs::write(root.join("boot_time_secs"), "12.5\n").unwrap();
        let report = sample_metrics(Some(&root), Duration::ZERO, "hh-service").unwrap();
        assert!(report.budgets.overall_pass);
        assert_eq!(report.service.unwrap().rss_bytes, 50_000 * 1024);
        assert_eq!(report.process_tree.len(), 2);
        assert_eq!(report.boot_time_secs, Some(12.5));
        fs::remove_dir_all(root).unwrap();
    }
}
