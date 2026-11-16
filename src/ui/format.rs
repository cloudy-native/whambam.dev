// SPDX-License-Identifier: MIT
//! Number formatting for the UI.

/// Latency in ms → "0", "850µs", "12.3ms", "1.23s".
pub fn latency(ms: f64) -> String {
    if ms <= 0.0 {
        "0".to_string()
    } else if ms < 1.0 {
        format!("{:.0}µs", ms * 1e3)
    } else if ms < 1e3 {
        format!("{ms:.1}ms")
    } else {
        format!("{:.2}s", ms / 1e3)
    }
}

/// Byte count → "512 B", "1.50 KiB", "2.00 MiB".
pub fn bytes(b: u64) -> String {
    const UNITS: [&str; 4] = ["KiB", "MiB", "GiB", "TiB"];
    if b < 1024 {
        return format!("{b} B");
    }
    let mut value = b as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.2} {}", UNITS[unit])
}

/// Compact axis number: "0.5", "42", "1.2k", "3.4M".
pub fn compact(v: f64) -> String {
    let a = v.abs();
    if a >= 1e6 {
        format!("{:.1}M", v / 1e6)
    } else if a >= 1e4 {
        format!("{:.0}k", v / 1e3)
    } else if a >= 1e3 {
        format!("{:.1}k", v / 1e3)
    } else if a >= 10.0 || a == 0.0 {
        format!("{v:.0}")
    } else {
        format!("{v:.1}")
    }
}

/// Time-axis label: one decimal for short spans so ticks stay evenly spaced.
pub fn seconds(v: f64, span: f64) -> String {
    if span < 10.0 {
        format!("{v:.1}s")
    } else {
        format!("{v:.0}s")
    }
}

/// `n` evenly spaced labels from `min` to `max`.
pub fn ticks(min: f64, max: f64, n: usize, fmt: impl Fn(f64) -> String) -> Vec<String> {
    let n = n.max(2);
    (0..n)
        .map(|i| fmt(min + (max - min) * i as f64 / (n - 1) as f64))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latency_picks_units() {
        assert_eq!(latency(0.25), "250µs");
        assert_eq!(latency(12.34), "12.3ms");
        assert_eq!(latency(1500.0), "1.50s");
    }

    #[test]
    fn bytes_use_binary_units() {
        assert_eq!(bytes(512), "512 B");
        assert_eq!(bytes(1536), "1.50 KiB");
        assert_eq!(bytes(3 * 1024 * 1024), "3.00 MiB");
    }

    #[test]
    fn compact_and_ticks() {
        assert_eq!(compact(0.0), "0");
        assert_eq!(compact(2.5), "2.5");
        assert_eq!(compact(1500.0), "1.5k");
        assert_eq!(compact(25_000.0), "25k");
        assert_eq!(compact(2_500_000.0), "2.5M");
        assert_eq!(ticks(0.0, 10.0, 3, compact), vec!["0", "5.0", "10"]);
        assert_eq!(seconds(1.25, 4.0), "1.2s");
        assert_eq!(seconds(12.4, 30.0), "12s");
    }
}
