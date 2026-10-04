//! What differs on the web: clocks, the local time, threads.

/// Seconds on a steady clock.
#[cfg(not(target_arch = "wasm32"))]
pub fn now() -> f64 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

#[cfg(target_arch = "wasm32")]
pub fn now() -> f64 {
    gfx2d::mq::date::now()
}

/// Seconds since 1970.
pub fn unix_time() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    return std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    #[cfg(target_arch = "wasm32")]
    return gfx2d::mq::date::now() as u64;
}

/// The local time in a strftime `format` (UTC in the browser).
pub fn local_time(format: &str) -> String {
    #[cfg(not(target_arch = "wasm32"))]
    return chrono::Local::now().format(format).to_string();
    #[cfg(target_arch = "wasm32")]
    return utc_time(format, unix_time());
}

/// A time (seconds since 1970) in a strftime `format`: local time (UTC in the browser).
pub fn format_time(secs: u64, format: &str) -> String {
    #[cfg(not(target_arch = "wasm32"))]
    return chrono::DateTime::from_timestamp(secs as i64, 0).map_or_else(String::new, |t| {
        t.with_timezone(&chrono::Local).format(format).to_string()
    });
    #[cfg(target_arch = "wasm32")]
    return utc_time(format, secs);
}

/// The few strftime fields the game uses, for a time in seconds since 1970 (UTC).
#[cfg(any(target_arch = "wasm32", test))]
fn utc_time(format: &str, secs: u64) -> String {
    let days = secs / 86_400;
    let (h, m, s) = (secs / 3600 % 24, secs / 60 % 60, secs % 60);
    // civil from days (Howard Hinnant's algorithm)
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(mo <= 2);
    let h12 = match h % 12 {
        0 => 12,
        h => h,
    };
    format
        .replace("%Y", &format!("{y:04}"))
        .replace("%m", &format!("{mo:02}"))
        .replace("%d", &format!("{d:02}"))
        .replace("%H", &format!("{h:02}"))
        .replace("%-I", &h12.to_string())
        .replace("%M", &format!("{m:02}"))
        .replace("%S", &format!("{s:02}"))
        .replace("%p", if h < 12 { "AM" } else { "PM" })
}

/// Runs `job` on another thread (at once in the browser, which has none here).
pub fn spawn(job: impl FnOnce() + Send + 'static) {
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::spawn(job);
    #[cfg(target_arch = "wasm32")]
    job();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_times_like_strftime() {
        // 2026-10-03 15:04:05 UTC
        let t = 1_791_039_845;
        assert_eq!(utc_time("%Y-%m-%d_%H-%M-%S_", t), "2026-10-03_15-04-05_");
        assert_eq!(utc_time("%-I:%M:%S %p", t), "3:04:05 PM");
        assert_eq!(utc_time("%-I:%M:%S %p", 0), "12:00:00 AM");
    }
}
