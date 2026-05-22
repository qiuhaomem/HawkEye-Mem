// 紧急快速通道（CR-08 安全专家要求）
//
// 当 available_mb < 512MB 或 used_percent > 98% 时，
// 立即跃迁到 Critical 状态，绕过所有时间窗口。
// 这是给 Agent 的最后一道保险。

use crate::collector::{MemoryMetrics, PressureLevel};

/// 检查是否满足紧急触发条件
///
/// 返回 `true` 时，调用方应**立即**将状态机跃迁到 Critical。
///
/// # 触发条件（任一满足即可）
/// - 可用内存 < emergency_available_mb（默认 512 MB）
/// - 已用百分比 > emergency_used_percent（默认 98%）
pub fn is_emergency(
    metrics: &MemoryMetrics,
    emergency_available_mb: u64,
    emergency_used_percent: f64,
) -> bool {
    metrics.available_mb < emergency_available_mb || metrics.used_percent > emergency_used_percent
}

/// 紧急跃迁时使用的压力等级（固定为 Critical）
#[allow(dead_code)]
pub fn emergency_pressure() -> PressureLevel {
    PressureLevel::Critical
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_metrics(available_mb: u64, used_percent: f64) -> MemoryMetrics {
        MemoryMetrics {
            total_mb: 16000,
            used_mb: (16000u64.saturating_sub(available_mb)),
            available_mb,
            used_percent,
            pressure: PressureLevel::Low,
        }
    }

    // UT-EM-001: 可用内存 < 512MB → 紧急
    #[test]
    fn test_emergency_low_available() {
        let m = make_metrics(400, 90.0);
        assert!(is_emergency(&m, 512, 98.0), "400MB < 512MB 应触发紧急");
    }

    // UT-EM-002: 已用百分比 > 98% → 紧急
    #[test]
    fn test_emergency_high_used() {
        let m = make_metrics(2000, 99.0);
        assert!(is_emergency(&m, 512, 98.0), "99% > 98% 应触发紧急");
    }

    // UT-EM-003: 两者都正常 → 不紧急
    #[test]
    fn test_emergency_normal() {
        let m = make_metrics(4000, 70.0);
        assert!(!is_emergency(&m, 512, 98.0), "正常状态不应触发紧急");
    }

    // UT-EM-004: 边界值：刚好 512MB / 98% → 不触发（严格小于/大于）
    #[test]
    fn test_emergency_boundary() {
        let m = make_metrics(512, 98.0);
        assert!(
            !is_emergency(&m, 512, 98.0),
            "512MB 不小于 512，98% 不大于 98"
        );
    }

    // UT-EM-005: 配置可调阈值
    #[test]
    fn test_emergency_custom_threshold() {
        let m = make_metrics(1000, 90.0);
        // 使用更保守的阈值
        assert!(is_emergency(&m, 1500, 85.0), "自定义阈值应生效");
        assert!(!is_emergency(&m, 500, 95.0), "自定义阈值应不触发");
    }
}
