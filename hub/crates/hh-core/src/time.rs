//! Time helpers. All stored times are UTC unix epoch milliseconds
//! (BACKEND_SCHEMA conventions). Never trust client clocks for security
//! decisions (AGENTS.md §6).

/// Current UTC time in epoch milliseconds.
pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// (year, month) in UTC for a epoch-ms timestamp. Used for Photos/Videos
/// folder layout (TRD §7.1). Civil-from-days algorithm (Howard Hinnant).
pub fn year_month(epoch_ms: i64) -> (i32, u32) {
    let days = epoch_ms.div_euclid(86_400_000);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = (mp + if mp < 10 { 3 } else { -9 }) as u32;
    let year = (y + if month <= 2 { 1 } else { 0 }) as i32;
    (year, month)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_dates() {
        // 2026-10-05T00:00:00Z = 1_791_196_800_000 ms
        assert_eq!(year_month(1_791_196_800_000), (2026, 10));
        // 1970-01-01
        assert_eq!(year_month(0), (1970, 1));
        // 2000-02-29T12:00Z (leap day)
        assert_eq!(year_month(951_825_600_000), (2000, 2));
        // 1999-12-31
        assert_eq!(year_month(946_598_400_000), (1999, 12));
    }
}
