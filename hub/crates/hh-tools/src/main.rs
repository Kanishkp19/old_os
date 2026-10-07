//! hh-tools: developer and CI tooling (AGENTS.md §9, TEST_PLAN §1).
//!
//! Subcommands:
//! - `fake-client pair --qr "<payload>"` — pair against a hub pairing window
//! - `fake-client upload --file <path>` — chunked, verified upload with resume
//! - `fake-client discover` — UDP broadcast fallback probe (NW-02)
//! - `bench transfer --size 2GiB` — throughput benchmark (TEST_PLAN §4)
//! - `wake --mac <mac> --broadcast <ip>` — send a WoL magic packet
//! - `gen-test-data --dir <dir> --photos 1000` — synthetic media corpus

use std::path::PathBuf;
use std::time::Instant;

use clap::{Parser, Subcommand};

mod fake_client;

#[derive(Parser)]
#[command(name = "hh-tools", about = "Home Hub dev/test tooling")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Scripted client for CI (pair/upload/resume).
    FakeClient {
        #[command(subcommand)]
        action: fake_client::Action,
    },
    /// Throughput benchmark with a synthetic file.
    Bench {
        #[arg(long, default_value = "2GiB")]
        size: String,
        #[arg(long)]
        hub: Option<String>,
    },
    /// Send a Wake-on-LAN magic packet.
    Wake {
        #[arg(long)]
        mac: String,
        #[arg(long, default_value = "255.255.255.255")]
        broadcast: String,
    },
    /// Print the BLAKE3 hash of a file (fault-injection harness helper).
    Hash { path: PathBuf },
    /// Generate a synthetic test corpus (photos/videos/docs).
    GenTestData {
        #[arg(long)]
        dir: PathBuf,
        #[arg(long, default_value_t = 100)]
        photos: u32,
        #[arg(long, default_value_t = 10)]
        videos: u32,
    },
}

fn parse_size(s: &str) -> u64 {
    let s = s.trim().to_uppercase();
    for (suffix, mul) in [("GIB", 1 << 30), ("MIB", 1 << 20), ("KIB", 1 << 10), ("GB", 1_000_000_000u64), ("MB", 1_000_000), ("KB", 1_000)] {
        if let Some(num) = s.strip_suffix(suffix) {
            return num.trim().parse::<u64>().unwrap_or(0) * mul;
        }
    }
    s.parse().unwrap_or(0)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Pin the rustls provider (aws-lc-rs can also enter the feature graph).
    let _ = rustls::crypto::ring::default_provider().install_default();
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::FakeClient { action } => fake_client::run(action).await?,
        Cmd::Bench { size, hub } => {
            let bytes = parse_size(&size);
            println!("bench: generating {} bytes synthetic file...", bytes);
            let tmp = std::env::temp_dir().join("hh-bench.bin");
            fake_client::write_synthetic(&tmp, bytes)?;
            let hub = hub.unwrap_or_else(|| "127.0.0.1:47800".into());
            let start = Instant::now();
            fake_client::upload_file(&hub, &tmp).await?;
            let secs = start.elapsed().as_secs_f64();
            println!(
                "bench: {} bytes in {:.1}s = {:.1} MB/s",
                bytes,
                secs,
                bytes as f64 / secs / 1_048_576.0
            );
        }
        Cmd::Hash { path } => {
            let data = std::fs::read(&path)?;
            println!("{}", blake3::hash(&data).to_hex());
        }
        Cmd::Wake { mac, broadcast } => {
            hh_hw::wake::send_wake(&mac, &broadcast)?;
            println!("magic packet sent to {mac} via {broadcast}");
        }
        Cmd::GenTestData { dir, photos, videos } => {
            std::fs::create_dir_all(&dir)?;
            for i in 0..photos {
                // Minimal valid JPEG header + payload (enough for import scans,
                // not for decode — use real samples for EXIF tests).
                let mut data = vec![0xFF, 0xD8, 0xFF, 0xE0];
                data.extend_from_slice(format!("HH-TEST-PHOTO-{i}").as_bytes());
                data.resize(4096, 0xAB);
                data.extend_from_slice(&[0xFF, 0xD9]);
                std::fs::write(dir.join(format!("IMG_{i:04}.jpg")), data)?;
            }
            for i in 0..videos {
                let data = vec![0u8; 1 << 20];
                std::fs::write(dir.join(format!("VID_{i:04}.mp4")), data)?;
            }
            println!("generated {photos} photos + {videos} videos in {}", dir.display());
        }
    }
    Ok(())
}
