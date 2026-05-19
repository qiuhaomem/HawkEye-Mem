use super::{CollectError, MemoryCollector, MemoryMetrics};
use std::process::Command;

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub struct MacosCollector;

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
impl MemoryCollector for MacosCollector {
    fn collect(&self) -> Result<MemoryMetrics, CollectError> {
        let vm_stat = Command::new("vm_stat")
            .output()
            .map_err(|e| CollectError::ReadFailed(e.to_string()))?;

        if !vm_stat.status.success() {
            return Err(CollectError::ReadFailed("vm_stat failed".into()));
        }

        let output = String::from_utf8_lossy(&vm_stat.stdout);
        let page_size = 16384u64;

        let mut _free_pages: u64 = 0;
        let mut active_pages: u64 = 0;
        let mut wired_pages: u64 = 0;
        let mut compressed_pages: u64 = 0;

        for line in output.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() < 2 { continue; }
            let key = parts[0].trim();
            let val = parts[1].trim().trim_end_matches('.');
            let num = val.parse::<u64>().unwrap_or(0);
            match key {
                "Pages free" => _free_pages = num,
                "Pages active" => active_pages = num,
                "Pages wired down" => wired_pages = num,
                "Pages stored in compressor" => compressed_pages = num,
                _ => {}
            }
        }

        let total_output = Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .map_err(|e| CollectError::ReadFailed(e.to_string()))?;
        let total_str = String::from_utf8_lossy(&total_output.stdout).trim().to_string();
        let total_bytes: u64 = total_str.parse().unwrap_or(8 * 1024 * 1024 * 1024);
        let total_mb = total_bytes / 1024 / 1024;

        let used_pages = active_pages + wired_pages + compressed_pages;
        let used_mb = (used_pages * page_size) / 1024 / 1024;
        let available_mb = total_mb.saturating_sub(used_mb);
        let used_percent = if total_mb > 0 {
            (used_mb as f64 / total_mb as f64 * 100.0 * 10.0).round() / 10.0
        } else {
            0.0
        };

        Ok(MemoryMetrics { total_mb, used_mb, available_mb, used_percent })
    }
}
