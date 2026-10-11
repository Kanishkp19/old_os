use serde::Serialize;
use std::sync::OnceLock;
use std::time::Instant;

static STARTED: OnceLock<Instant> = OnceLock::new();

#[derive(Serialize)]
struct SmokeStatus {
    uptime_ms: u128,
    process_id: u32,
}

#[tauri::command]
fn get_smoke_status() -> SmokeStatus {
    SmokeStatus {
        uptime_ms: STARTED.get_or_init(Instant::now).elapsed().as_millis(),
        process_id: std::process::id(),
    }
}

fn main() {
    STARTED.get_or_init(Instant::now);
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_smoke_status])
        .run(tauri::generate_context!())
        .expect("failed to run Home Hub smoke app");
}
