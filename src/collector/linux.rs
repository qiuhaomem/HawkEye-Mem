use super::{CollectError, MemoryCollector, MemoryMetrics};
use std::fs;

pub struct LinuxCollector;

impl MemoryCollector for LinuxCollector {
    fn collect(&self) -> Result<MemoryMetrics, CollectError> {
        let content = fs::read_to_string("/proc/meminfo")
            .map_err(|e| CollectError::ReadFailed(e.to_string()))?;

        let mut total_kb: u64 = 0;
        let mut available_kb: u64 = 0;

        for line in content.lines() {
            if let Some(val) = parse_meminfo_line(line, "MemTotal:") {
                total_kb = val;
            } else if let Some(val) = parse_meminfo_line(line, "MemAvailable:") {
                available_kb = val;
            }
        }

        if total_kb == 0 {
            return Err(CollectError::ReadFailed("MemTotal not found".into()));
        }

        let total_mb = total_kb / 1024;
        let available_mb = if available_kb > 0 { available_kb / 1024 } else { total_mb };
        let used_mb = total_mb.saturating_sub(available_mb);
        let used_percent = if total_mb > 0 {
            (used_mb as f64 / total_mb as f64 * 100.0 * 10.0).round() / 10.0
        } else {
            0.0
        };

        Ok(MemoryMetrics {
            total_mb,
            used_mb,
            available_mb,
            used_percent,
        })
    }
}

fn parse_meminfo_line(line: &str, prefix: &str) -> Option<u64> {
    if line.starts_with(prefix) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            return parts[1].parse::<u64>().ok();
        }
    }
    None
}
