use super::{CollectError, CollectorOutput, CpuMetrics, CpuPressure, ResourceCollector};

/// CPU 采集器：检测系统负载和核心数
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub struct CpuCollector;

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
impl ResourceCollector for CpuCollector {
    fn collect(&self) -> Result<CollectorOutput, CollectError> {
        let (load_1m, load_5m, load_15m) = get_loadavg()?;
        let cores = num_cpus::get() as u32;

        // 压力判定
        let pressure = if load_1m < cores as f64 {
            CpuPressure::Low
        } else if load_1m < 2.0 * cores as f64 {
            CpuPressure::Medium
        } else {
            CpuPressure::High
        };

        Ok(CollectorOutput::Cpu(CpuMetrics {
            cores,
            load_avg_1m: load_1m,
            load_avg_5m: load_5m,
            load_avg_15m: load_15m,
            agent_processes_percent: None,
            pressure,
        }))
    }
}

/// 读取系统负载平均值
#[cfg(target_os = "linux")]
fn get_loadavg() -> Result<(f64, f64, f64), CollectError> {
    let content = std::fs::read_to_string("/proc/loadavg")
        .map_err(|e| CollectError::ReadFailed(format!("Failed to read /proc/loadavg: {}", e)))?;

    let parts: Vec<&str> = content.split_whitespace().collect();
    if parts.len() < 3 {
        return Err(CollectError::ReadFailed(format!(
            "Unexpected /proc/loadavg format: {}",
            content.trim()
        )));
    }

    let load_1m = parts[0]
        .parse::<f64>()
        .map_err(|e| CollectError::ReadFailed(format!("Failed to parse load_1m: {}", e)))?;
    let load_5m = parts[1]
        .parse::<f64>()
        .map_err(|e| CollectError::ReadFailed(format!("Failed to parse load_5m: {}", e)))?;
    let load_15m = parts[2]
        .parse::<f64>()
        .map_err(|e| CollectError::ReadFailed(format!("Failed to parse load_15m: {}", e)))?;

    Ok((load_1m, load_5m, load_15m))
}

/// macOS 版本：通过 sysctl vm.loadavg 获取负载
#[cfg(target_os = "macos")]
fn get_loadavg() -> Result<(f64, f64, f64), CollectError> {
    use std::process::Command;

    let output = Command::new("sysctl")
        .args(["-n", "vm.loadavg"])
        .output()
        .map_err(|e| CollectError::ReadFailed(format!("Failed to run sysctl: {}", e)))?;

    if !output.status.success() {
        return Err(CollectError::ReadFailed("sysctl vm.loadavg failed".into()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // sysctl vm.loadavg 输出格式: "{ 1.23 0.98 0.76 }"
    let trimmed = stdout
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim();
    let parts: Vec<&str> = trimmed.split_whitespace().collect();

    if parts.len() < 3 {
        return Err(CollectError::ReadFailed(format!(
            "Unexpected sysctl output: {}",
            stdout.trim()
        )));
    }

    let load_1m = parts[0]
        .parse::<f64>()
        .map_err(|e| CollectError::ReadFailed(format!("Failed to parse load_1m: {}", e)))?;
    let load_5m = parts[1]
        .parse::<f64>()
        .map_err(|e| CollectError::ReadFailed(format!("Failed to parse load_5m: {}", e)))?;
    let load_15m = parts[2]
        .parse::<f64>()
        .map_err(|e| CollectError::ReadFailed(format!("Failed to parse load_15m: {}", e)))?;

    Ok((load_1m, load_5m, load_15m))
}

/// 不支持的平台
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn get_loadavg() -> Result<(f64, f64, f64), CollectError> {
    Err(CollectError::UnsupportedPlatform)
}
