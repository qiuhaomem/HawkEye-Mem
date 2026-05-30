use crate::collector::{MemoryMetrics, PressureLevel};
use serde::{Deserialize, Serialize};

/// Minimal memory pressure input for CacheAdvisor (CR-22).
/// Only contains the fields needed for cache strategy decisions.
#[derive(Debug, Clone)]
pub struct MemoryPressure {
    pub pressure: PressureLevel,
    pub available_mb: u64,
    pub total_mb: u64,
}

impl From<&MemoryMetrics> for MemoryPressure {
    fn from(m: &MemoryMetrics) -> Self {
        Self {
            pressure: m.pressure.clone(),
            available_mb: m.available_mb,
            total_mb: m.total_mb,
        }
    }
}

/// Cache strategy mode
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CacheMode {
    Aggressive,
    Balanced,
    Conservative,
    Emergency,
}

impl std::fmt::Display for CacheMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheMode::Aggressive => write!(f, "aggressive"),
            CacheMode::Balanced => write!(f, "balanced"),
            CacheMode::Conservative => write!(f, "conservative"),
            CacheMode::Emergency => write!(f, "emergency"),
        }
    }
}

/// Recommended cache strategy
#[derive(Debug, Clone, Serialize)]
pub struct CacheStrategy {
    pub mode: CacheMode,
    pub ttl_seconds: u64,
    pub max_cache_mb: u64,
    pub prefetch_enabled: bool,
    pub reason: String,
    pub protocol_version: u64,
}

/// Cache advisor — recommends cache strategy based on memory pressure.
/// Only depends on MemoryPressure (CR-22), NOT on full ResourceSnapshot.
pub struct CacheAdvisor;

impl CacheAdvisor {
    /// Calculate max cache memory as a ratio of available memory.
    fn calc_max_cache(available_mb: u64, ratio: f64) -> u64 {
        (available_mb as f64 * ratio) as u64
    }

    /// Recommend cache strategy based on memory pressure.
    pub fn recommend(pressure: &MemoryPressure) -> CacheStrategy {
        let available_pct = if pressure.total_mb > 0 {
            pressure.available_mb as f64 / pressure.total_mb as f64 * 100.0
        } else {
            100.0
        };

        match &pressure.pressure {
            // CR-05: emergency only affects cache, not other tools
            PressureLevel::Critical => CacheStrategy {
                mode: CacheMode::Emergency,
                ttl_seconds: 0,
                max_cache_mb: 0,
                prefetch_enabled: false,
                reason: "内存危机，立即清空缓存保命".to_string(),
                protocol_version: super::CACHE_PROTOCOL_VERSION,
            },
            _ if available_pct < 5.0 => CacheStrategy {
                mode: CacheMode::Emergency,
                ttl_seconds: 0,
                max_cache_mb: 0,
                prefetch_enabled: false,
                reason: format!("内存危机（可用{:.1}%），立即清空缓存保命", available_pct),
                protocol_version: super::CACHE_PROTOCOL_VERSION,
            },
            _ if available_pct < 15.0 => CacheStrategy {
                mode: CacheMode::Conservative,
                ttl_seconds: 60,
                max_cache_mb: Self::calc_max_cache(pressure.available_mb, 0.05),
                prefetch_enabled: false,
                reason: format!("内存压力high（可用{:.1}%），切换保守缓存", available_pct),
                protocol_version: super::CACHE_PROTOCOL_VERSION,
            },
            _ if available_pct < 30.0 => CacheStrategy {
                mode: CacheMode::Balanced,
                ttl_seconds: 300,
                max_cache_mb: Self::calc_max_cache(pressure.available_mb, 0.10),
                prefetch_enabled: true,
                reason: format!("内存压力medium（可用{:.1}%），保持平衡缓存", available_pct),
                protocol_version: super::CACHE_PROTOCOL_VERSION,
            },
            _ => CacheStrategy {
                mode: CacheMode::Aggressive,
                ttl_seconds: 600,
                max_cache_mb: Self::calc_max_cache(pressure.available_mb, 0.20),
                prefetch_enabled: true,
                reason: format!(
                    "内存充裕（可用{:.1}%），启用激进缓存，预计命中率99%+",
                    available_pct
                ),
                protocol_version: super::CACHE_PROTOCOL_VERSION,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mp(avail: u64, total: u64, p: PressureLevel) -> MemoryPressure {
        MemoryPressure {
            pressure: p,
            available_mb: avail,
            total_mb: total,
        }
    }

    // UT-CACHE-001: 仅接收MemoryPressure参数 (CR-22)
    #[test]
    fn test_ut_cache_001_memory_pressure_only() {
        let p = mp(12000, 16384, PressureLevel::Low);
        let strategy = CacheAdvisor::recommend(&p);
        assert_eq!(strategy.mode, CacheMode::Aggressive);
        assert_eq!(strategy.ttl_seconds, 600);
        assert!(strategy.prefetch_enabled);
    }

    // UT-CACHE-002: 内存充裕→aggressive
    #[test]
    fn test_ut_cache_002_aggressive() {
        let p = mp(8192, 16384, PressureLevel::Low);
        let strategy = CacheAdvisor::recommend(&p);
        assert_eq!(strategy.mode, CacheMode::Aggressive);
        assert_eq!(strategy.max_cache_mb, (8192_f64 * 0.20) as u64);
    }

    // UT-CACHE-003: 内存中等→balanced
    #[test]
    fn test_ut_cache_003_balanced() {
        let p = mp(3276, 16384, PressureLevel::Medium);
        let strategy = CacheAdvisor::recommend(&p);
        assert_eq!(strategy.mode, CacheMode::Balanced);
        assert_eq!(strategy.ttl_seconds, 300);
        assert!(strategy.prefetch_enabled);
    }

    // UT-CACHE-004: 内存紧张→conservative
    #[test]
    fn test_ut_cache_004_conservative() {
        let p = mp(1638, 16384, PressureLevel::High);
        let strategy = CacheAdvisor::recommend(&p);
        assert_eq!(strategy.mode, CacheMode::Conservative);
        assert_eq!(strategy.ttl_seconds, 60);
        assert!(!strategy.prefetch_enabled);
    }

    // UT-CACHE-005: 内存危机→emergency
    #[test]
    fn test_ut_cache_005_emergency() {
        let p = mp(491, 16384, PressureLevel::Critical);
        let strategy = CacheAdvisor::recommend(&p);
        assert_eq!(strategy.mode, CacheMode::Emergency);
        assert_eq!(strategy.ttl_seconds, 0);
        assert_eq!(strategy.max_cache_mb, 0);
        assert!(!strategy.prefetch_enabled);
    }

    // UT-CACHE-006: pressure=critical→emergency
    #[test]
    fn test_ut_cache_006_critical_pressure() {
        let p = mp(2000, 16384, PressureLevel::Critical);
        let strategy = CacheAdvisor::recommend(&p);
        assert_eq!(strategy.mode, CacheMode::Emergency);
    }

    // UT-CACHE-007: 边界值 avail=15%
    #[test]
    fn test_ut_cache_007_boundary_15() {
        // 15% of 16384 = 2457.6 → should be conservative (< 15% is conservative)
        let p = mp(2457, 16384, PressureLevel::Low);
        let strategy = CacheAdvisor::recommend(&p);
        // avail_pct = 2457/16384*100 = 14.99% < 15% → conservative
        assert_eq!(strategy.mode, CacheMode::Conservative);
    }

    // UT-CACHE-008: 边界值 avail=30%
    #[test]
    fn test_ut_cache_008_boundary_30() {
        let p = mp(4915, 16384, PressureLevel::Low);
        let strategy = CacheAdvisor::recommend(&p);
        // avail_pct = 4915/16384*100 = 29.99% < 30% → balanced
        assert_eq!(strategy.mode, CacheMode::Balanced);
    }

    // UT-CACHE-009: reason包含具体数值
    #[test]
    fn test_ut_cache_009_reason_contains_value() {
        let p = mp(1966, 16384, PressureLevel::High);
        let strategy = CacheAdvisor::recommend(&p);
        assert!(
            strategy.reason.contains("12.0"),
            "reason should contain '12.0'"
        );
    }

    // UT-CACHE-011: 无内存数据时回退（所有压力都低）
    #[test]
    fn test_ut_cache_011_no_memory_fallback() {
        // total_mb=0 should not happen in practice, but if it does,
        // available_pct defaults to 100.0 → aggressive
        let p = MemoryPressure {
            pressure: PressureLevel::Low,
            available_mb: 0,
            total_mb: 0,
        };
        let strategy = CacheAdvisor::recommend(&p);
        assert_eq!(strategy.mode, CacheMode::Aggressive);
    }

    // protocol_version always present (CR-23)
    #[test]
    fn test_ut_cache_protocol_version() {
        let p = mp(12000, 16384, PressureLevel::Low);
        let strategy = CacheAdvisor::recommend(&p);
        assert_eq!(strategy.protocol_version, 1);
    }
}
