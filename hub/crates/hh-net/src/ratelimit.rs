//! Per-device rate limiting (API_SPEC §11: 20 req/s non-transfer).

use std::collections::HashMap;
use std::sync::Mutex;

use hh_core::time::now_ms;

const LIMIT_PER_SEC: u32 = 20;

pub struct RateLimiter {
    windows: Mutex<HashMap<String, (u64, u32)>>,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self { windows: Mutex::new(HashMap::new()) }
    }
}

impl RateLimiter {
    /// Returns true if the request is allowed for this device.
    pub fn check(&self, device_id: &str) -> bool {
        let now = now_ms() as u64 / 1000;
        let mut map = match self.windows.lock() {
            Ok(m) => m,
            Err(_) => return false, // fail closed
        };
        let entry = map.entry(device_id.to_string()).or_insert((now, 0));
        if entry.0 != now {
            *entry = (now, 0);
        }
        entry.1 += 1;
        entry.1 <= LIMIT_PER_SEC
    }

    /// Periodic cleanup so the map doesn't grow unboundedly (T9).
    pub fn gc(&self) {
        let now = now_ms() as u64 / 1000;
        if let Ok(mut map) = self.windows.lock() {
            map.retain(|_, (sec, _)| now - *sec < 60);
        }
    }
}
