use super::{CollectError, CollectorOutput, MemoryMetrics, PressureLevel, ResourceCollector};
use std::process::Command;

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub struct MacosCollector;

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
impl ResourceCollector for MacosCollector {
    fn collect(&self) -> Result<CollectorOutput, CollectError> {
        let vm_stat = Command::new("vm_stat")
            .output()
            .map_err(|e| CollectError::ReadFailed(e.to_string()))?;

        if !vm_stat.status.success() {
            return Err(CollectError::ReadFailed("vm_stat failed".into()));
        }

        let output = String::from_utf8_lossy(&vm_stat.stdout);
        let page_size = 16384u64;

        let mut free_pages: u64 = 0;
        let mut active_pages: u64 = 0;
        let mut wired_pages: u64 = 0;
        let mut compressor_occupied_pages: u64 = 0;
        let mut has_compressor_occupied = false;
        let mut inactive_pages: u64 = 0;
        let mut has_inactive = false;
        let mut speculative_pages: u64 = 0;
        let mut has_speculative = false;

        for line in output.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() < 2 { continue; }
            let key = parts[0].trim();
            let val = parts[1].trim().trim_end_matches('.');
            let num = val.parse::<u64>().unwrap_or(0u64);
            match key {
                "Pages free" => free_pages = num,
                "Pages active" => active_pages = num,
                "Pages wired down" => wired_pages = num,
                "Pages occupied by compressor" => { compressor_occupied_pages = num; has_compressor_occupied = true; },
                "Pages inactive" => { inactive_pages = num; has_inactive = true; },
                "Pages speculative" => { speculative_pages = num; has_speculative = true; },
                _ => {}
            }
        }
        // 旧版本 macOS 没有 "Pages occupied by compressor"，
        // 此时不把 compressed pages 计入 used（避免重复计算）
        let used_pages = if has_compressor_occupied {
            active_pages + wired_pages + compressor_occupied_pages
        } else {
            active_pages + wired_pages
        };

        let total_output = Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .map_err(|e| CollectError::ReadFailed(e.to_string()))?;
        let total_str = String::from_utf8_lossy(&total_output.stdout).trim().to_string();
        let total_bytes: u64 = total_str.parse().unwrap_or(8 * 1024 * 1024 * 1024);
        let total_mb = total_bytes / 1024 / 1024;

        let used_mb = (used_pages * page_size) / 1024 / 1024;
        // 直接用 free + inactive + speculative 计算可用内存，更准确
        let available_mb = if has_inactive && has_speculative {
            let avail_pages = free_pages + inactive_pages + speculative_pages;
            (avail_pages * page_size) / 1024 / 1024
        } else {
            total_mb.saturating_sub(used_mb)
        };
        let used_percent = if total_mb > 0 {
            ((used_mb as f64 / total_mb as f64 * 100.0 * 10.0).round() / 10.0).min(100.0)
        } else {
            0.0
        };

        let pressure = classify_pressure(available_mb, used_percent);

        Ok(CollectorOutput::Memory(MemoryMetrics { total_mb, used_mb, available_mb, used_percent, pressure }))
    }
}

/// 根据可用内存和已用百分比判定压力水位
fn classify_pressure(available_mb: u64, used_percent: f64) -> PressureLevel {
    if available_mb < 256 || used_percent > 95.0 {
        PressureLevel::Critical
    } else if available_mb < 512 || used_percent > 90.0 {
        PressureLevel::High
    } else if available_mb < 1024 || used_percent > 80.0 {
        PressureLevel::Medium
    } else {
        PressureLevel::Low
    }
}
