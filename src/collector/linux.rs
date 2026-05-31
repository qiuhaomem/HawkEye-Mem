// Copyright 2026 秋毫mem Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::{CollectError, CollectorOutput, MemoryMetrics, PressureLevel, ResourceCollector};
use std::fs;

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub struct LinuxCollector;

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
impl ResourceCollector for LinuxCollector {
    fn collect(&self) -> Result<CollectorOutput, CollectError> {
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
        let available_mb = if available_kb > 0 {
            available_kb / 1024
        } else {
            total_mb
        };
        let used_mb = total_mb.saturating_sub(available_mb);
        let used_percent = if total_mb > 0 {
            (used_mb as f64 / total_mb as f64 * 100.0 * 10.0).round() / 10.0
        } else {
            0.0
        };

        let pressure = classify_pressure(available_mb, used_percent, total_mb);

        Ok(CollectorOutput::Memory(MemoryMetrics {
            total_mb,
            used_mb,
            available_mb,
            used_percent,
            pressure,
        }))
    }
}

/// 压力判定（V0.3: 4-6GB 平滑过渡 + 紧急通道 CR-05）
///
/// - 总内存 ≤ 4GB: 纯百分比判定
/// - 4-6GB: 混合判定（百分比和绝对值取宽松 = 压力更低者）
/// - ≥ 6GB: 纯绝对值判定（带总内存比例）
///
/// CR-05 紧急通道独立于判定模式，在任何内存大小下都优先检查。
fn classify_pressure(available_mb: u64, used_percent: f64, total_mb: u64) -> PressureLevel {
    // CR-05: 紧急快速通道（全局安全网，独立于判定模式）
    let emergency_threshold = (total_mb / 10).min(512);
    if available_mb < 256 || available_mb < emergency_threshold {
        return PressureLevel::Critical;
    }

    let abs_level = classify_by_available(available_mb, total_mb);
    let pct_level = classify_by_percent(used_percent);

    if total_mb <= 4096 {
        // ≤ 4GB: 纯百分比
        pct_level
    } else if total_mb >= 6144 {
        // ≥ 6GB: 纯绝对值
        abs_level
    } else {
        // 4-6GB 过渡：取宽松（压力更低）者
        PressureLevel::min(abs_level, pct_level)
    }
}

/// 纯绝对值判定（紧急通道已在 classify_pressure 顶层处理）
///
/// 按 available 绝对值逐级判定（带总内存比例兜底）
fn classify_by_available(available_mb: u64, total_mb: u64) -> PressureLevel {
    if available_mb < 512 || available_mb < (total_mb / 5).min(1024) {
        PressureLevel::High
    } else if available_mb < 1024 || available_mb < (total_mb / 3).min(2048) {
        PressureLevel::Medium
    } else {
        PressureLevel::Low
    }
}

/// 纯百分比判定
fn classify_by_percent(used_percent: f64) -> PressureLevel {
    if used_percent >= 95.0 {
        PressureLevel::Critical
    } else if used_percent >= 90.0 {
        PressureLevel::High
    } else if used_percent >= 80.0 {
        PressureLevel::Medium
    } else {
        PressureLevel::Low
    }
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn parse_meminfo_line(line: &str, prefix: &str) -> Option<u64> {
    if line.starts_with(prefix) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            return parts[1].parse::<u64>().ok();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // 参数化测试：4种总内存 × 5种使用率
    // CR-04: 低内存判定参数化单元测试
    // 验证边界平滑过渡不出现跳变
    // ========================================================================

    // --- 2GB 机器 ---
    #[test]
    fn test_classify_2gb_10pct() {
        assert_eq!(classify_pressure(1843, 10.0, 2048), PressureLevel::Low);
    }
    #[test]
    fn test_classify_2gb_50pct() {
        assert_eq!(classify_pressure(1024, 50.0, 2048), PressureLevel::Low);
    }
    #[test]
    fn test_classify_2gb_80pct() {
        assert_eq!(classify_pressure(410, 80.0, 2048), PressureLevel::Medium);
    }
    #[test]
    fn test_classify_2gb_90pct() {
        // 2GB: available=300 >= emergency=204, 不触发紧急通道 → 纯百分比 90% → High
        assert_eq!(classify_pressure(300, 90.0, 2048), PressureLevel::High);
    }
    #[test]
    fn test_classify_2gb_95pct() {
        assert_eq!(classify_pressure(102, 95.0, 2048), PressureLevel::Critical);
    }

    // --- 4GB 机器 ---
    #[test]
    fn test_classify_4gb_10pct() {
        assert_eq!(classify_pressure(3686, 10.0, 4096), PressureLevel::Low);
    }
    #[test]
    fn test_classify_4gb_50pct() {
        assert_eq!(classify_pressure(2048, 50.0, 4096), PressureLevel::Low);
    }
    #[test]
    fn test_classify_4gb_80pct() {
        assert_eq!(classify_pressure(819, 80.0, 4096), PressureLevel::Medium);
    }
    #[test]
    fn test_classify_4gb_90pct() {
        assert_eq!(classify_pressure(410, 90.0, 4096), PressureLevel::High);
    }
    #[test]
    fn test_classify_4gb_95pct() {
        assert_eq!(classify_pressure(205, 95.0, 4096), PressureLevel::Critical);
    }

    // --- 8GB 机器 ---
    #[test]
    fn test_classify_8gb_10pct() {
        // 8GB ≥ 6144 → 绝对值: 7373 > 2048 → Low
        assert_eq!(classify_pressure(7373, 10.0, 8192), PressureLevel::Low);
    }
    #[test]
    fn test_classify_8gb_50pct() {
        // 绝对值: 4096 > 2048 → Low
        assert_eq!(classify_pressure(4096, 50.0, 8192), PressureLevel::Low);
    }
    #[test]
    fn test_classify_8gb_80pct() {
        // 绝对值: 1638 < 2048 → Medium
        assert_eq!(classify_pressure(1638, 80.0, 8192), PressureLevel::Medium);
    }
    #[test]
    fn test_classify_8gb_90pct() {
        // 绝对值: 819 < 1024 → High
        assert_eq!(classify_pressure(819, 90.0, 8192), PressureLevel::High);
    }
    #[test]
    fn test_classify_8gb_95pct() {
        // 紧急通道: 410 < 512 → Critical
        assert_eq!(classify_pressure(410, 95.0, 8192), PressureLevel::Critical);
    }

    // --- 16GB 机器 ---
    #[test]
    fn test_classify_16gb_10pct() {
        assert_eq!(classify_pressure(14746, 10.0, 16384), PressureLevel::Low);
    }
    #[test]
    fn test_classify_16gb_50pct() {
        // 绝对值: 8192 > 2048 → Low
        assert_eq!(classify_pressure(8192, 50.0, 16384), PressureLevel::Low);
    }
    #[test]
    fn test_classify_16gb_80pct() {
        // 绝对值: 3277 > 2048 → Low
        assert_eq!(classify_pressure(3277, 80.0, 16384), PressureLevel::Low);
    }
    #[test]
    fn test_classify_16gb_90pct() {
        // 绝对值: 1638 < 2048 → Medium
        assert_eq!(classify_pressure(1638, 90.0, 16384), PressureLevel::Medium);
    }
    #[test]
    fn test_classify_16gb_95pct() {
        // 绝对值: 819 < 1024 → High（512 < 819 < 1024, 紧急未触发）
        assert_eq!(classify_pressure(819, 95.0, 16384), PressureLevel::High);
    }

    // ========================================================================
    // CR-05: 紧急快速通道专用测试
    // ========================================================================

    #[test]
    fn test_emergency_2gb() {
        // 2GB: emergency_threshold = min(2048/10, 512) = 204
        // available=199 < 204 → Critical
        assert_eq!(classify_pressure(199, 10.0, 2048), PressureLevel::Critical);
    }

    #[test]
    fn test_emergency_16gb() {
        // 16GB: emergency_threshold = min(16384/10, 512) = 512
        // available=511 < 512 → Critical
        assert_eq!(classify_pressure(511, 10.0, 16384), PressureLevel::Critical);
    }

    #[test]
    fn test_emergency_not_triggered() {
        // 16GB: emergency_threshold = 512
        // available=2049 > 512, 绝对值: 2049 > 2048 → Low
        assert_eq!(classify_pressure(2049, 10.0, 16384), PressureLevel::Low);
    }

    // ========================================================================
    // 4-6GB 过渡区专用测试
    // ========================================================================

    #[test]
    fn test_transition_5gb_abs_higher() {
        // 5GB (5120MB) 过渡区: 混合取宽松
        // 紧急通道: emergency=min(512,512)=512, available=800 >= 512 → 不触发
        // 绝对值: 800 < 1024 → High
        // 百分比: 85% ≥ 80 → Medium
        // min(High, Medium) = Medium → 宽松取百分比
        assert_eq!(classify_pressure(800, 85.0, 5120), PressureLevel::Medium);
    }

    #[test]
    fn test_transition_5gb_pct_higher() {
        // 5GB: 绝对值 Low, 百分比 High → min(Low, High) = Low
        // 紧急通道: available=3000 >= 512 → 不触发
        // 绝对值: 3000 > 2048 → Low
        // 百分比: 92% ≥ 90 → High
        assert_eq!(classify_pressure(3000, 92.0, 5120), PressureLevel::Low);
    }

    #[test]
    fn test_transition_5gb_same() {
        // 5GB: 两者一致
        // 绝对值: 3000 > 2048 → Low
        // 百分比: 10% → Low
        assert_eq!(classify_pressure(3000, 10.0, 5120), PressureLevel::Low);
    }
}
