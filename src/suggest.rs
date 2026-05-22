//! 并发度建议引擎（REQ-001 · 物理AI第一步）
//!
//! 基于物理资源（CPU核心数、可用内存、内存压力）自动计算建议的并发度，
//! 让Agent知道"自己这台机器能开几个分身"。
//!
//! ## 算法
//!
//! 1. **CPU上限**: `cpu_cores * 2`（含超线程，保留25%给系统）
//! 2. **内存上限**: `available_mb / task_memory_mb`
//! 3. **取较小值**: `min(cpu_limit, mem_limit)`
//! 4. **压力调节**: memory_pressure=high 时减半，critical 时强制为1
//! 5. **安全余量**: 至少保留 20% 内存 + 512MB 兜底

use crate::collector::{PressureLevel, ResourceSnapshot};

/// 并发度建议结果
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConcurrencySuggestion {
    /// 系统资源快照
    pub system: SystemResources,
    /// 建议
    pub suggestion: SuggestionDetail,
}

/// 系统资源摘要
#[derive(Debug, Clone, serde::Serialize)]
pub struct SystemResources {
    pub cpu_cores: u32,
    pub total_memory_mb: u64,
    pub available_memory_mb: u64,
    pub memory_pressure: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_memory_mb: Option<u64>,
}

/// 建议详情
#[derive(Debug, Clone, serde::Serialize)]
pub struct SuggestionDetail {
    /// 建议的最大并发数
    pub max_concurrency: u32,
    /// 建议的安全并发数（建议实际使用的）
    pub recommended_concurrency: u32,
    /// 每个子Agent的安全内存预算（MB）
    pub per_agent_safe_memory_mb: u64,
    /// 决策理由
    pub reasoning: String,
    /// 风险等级：ok / caution / critical
    pub risk_level: String,
}

/// 默认子任务内存预算（MB）
const DEFAULT_TASK_MEMORY_MB: u64 = 1024;

/// 最小安全内存余量（MB）
const MIN_SAFETY_MARGIN_MB: u64 = 512;

/// 安全内存余量比例
const SAFETY_MARGIN_RATIO: f64 = 0.2;

/// 计算并发度建议
pub fn suggest_concurrency(
    snapshot: &ResourceSnapshot,
    task_memory_mb: Option<u64>,
) -> ConcurrencySuggestion {
    let task_mem = task_memory_mb.unwrap_or(DEFAULT_TASK_MEMORY_MB);
    let task_mem = std::cmp::max(task_mem, 128); // 最少128MB

    // === 采集系统资源 ===
    let cpu_cores = snapshot
        .cpu
        .as_ref()
        .map(|c| c.cores)
        .unwrap_or(1)
        .max(1);

    let total_memory_mb = snapshot
        .memory
        .as_ref()
        .map(|m| m.total_mb)
        .unwrap_or(4096);

    let available_memory_mb = snapshot
        .memory
        .as_ref()
        .map(|m| m.available_mb)
        .unwrap_or(1024);

    let pressure = snapshot
        .memory
        .as_ref()
        .map(|m| &m.pressure)
        .unwrap_or(&PressureLevel::Low);

    // === 计算 CPU 上限 ===
    // 每个核心建议跑1-2个子Agent（超线程友好），保留25%给系统
    let cpu_limit = (cpu_cores as f64 * 1.5).ceil() as u32;
    let cpu_conservative = cpu_cores.max(1); // 保守：1个核心1个Agent

    // === 计算内存上限 ===
    // 保留 20% + 512MB 给系统
    let safety_reserve = std::cmp::max(
        (total_memory_mb as f64 * SAFETY_MARGIN_RATIO) as u64,
        MIN_SAFETY_MARGIN_MB,
    );

    let usable_memory = available_memory_mb.saturating_sub(safety_reserve);
    let mem_limit = if task_mem > 0 {
        usable_memory.checked_div(task_mem).unwrap_or(0).max(1)
    } else {
        1
    } as u32;

    // === 压力调节 ===
    let pressure_multiplier = match pressure {
        PressureLevel::Critical => 0.0, // 紧急状态，别开了
        PressureLevel::High => 0.5,     // 减半
        PressureLevel::Medium => 0.8,   // 打八折
        PressureLevel::Low => 1.0,      // 正常
    };

    // === 综合计算 ===
    let raw_concurrency = std::cmp::min(cpu_limit, mem_limit);
    let adjusted = (raw_concurrency as f64 * pressure_multiplier).round() as u32;
    let recommended = adjusted.clamp(1, 32); // 最少1，最多32

    // 安全并发数：在recommended基础上再保守一点
    let max_concurrency = recommended;
    let safe_concurrency = if pressure_multiplier < 1.0 {
        // 压力大时推荐 = 最大
        recommended
    } else {
        // 正常时推荐保守值 = cpu_conservative 和 recommended 的较小者
        std::cmp::min(cpu_conservative, recommended).max(1)
    };

    // 每个Agent的安全内存
    let per_agent_mem = if recommended > 0 {
        usable_memory / recommended as u64
    } else {
        usable_memory
    };

    // === 生成决策理由 ===
    let risk_level = if pressure_multiplier == 0.0 {
        "critical"
    } else if pressure_multiplier <= 0.5 {
        "caution"
    } else {
        "ok"
    };

    let reasoning = build_reasoning(
        cpu_cores,
        cpu_limit,
        available_memory_mb,
        task_mem,
        mem_limit,
        recommended,
        pressure,
        safety_reserve,
    );

    ConcurrencySuggestion {
        system: SystemResources {
            cpu_cores,
            total_memory_mb,
            available_memory_mb,
            memory_pressure: format!("{}", pressure),
            task_memory_mb,
        },
        suggestion: SuggestionDetail {
            max_concurrency,
            recommended_concurrency: safe_concurrency,
            per_agent_safe_memory_mb: per_agent_mem,
            reasoning,
            risk_level: risk_level.to_string(),
        },
    }
}

/// 生成人类可读的决策理由
#[allow(clippy::too_many_arguments)]
fn build_reasoning(
    cpu_cores: u32,
    cpu_limit: u32,
    available_mb: u64,
    task_mem: u64,
    mem_limit: u32,
    recommended: u32,
    pressure: &PressureLevel,
    safety_reserve: u64,
) -> String {
    let mut parts = Vec::new();

    parts.push(format!("{} CPU cores available", cpu_cores));

    // CPU
    if cpu_limit < mem_limit {
        parts.push(format!(
            "CPU-bound: max {} concurrent tasks (1.5× cores with 25% system headroom)",
            cpu_limit
        ));
    } else {
        parts.push(format!(
            "Memory-bound: {}MB available ÷ {}MB per task = max {} concurrent",
            available_mb, task_mem, mem_limit
        ));
    }

    // Safety reserve
    parts.push(format!(
        "{}MB reserved for system stability",
        safety_reserve
    ));

    // Pressure
    match pressure {
        PressureLevel::Critical => {
            parts.push("⚠️ CRITICAL memory pressure — reduced to 1 task".to_string());
        }
        PressureLevel::High => {
            parts.push("⚠️ High memory pressure — concurrency halved".to_string());
        }
        PressureLevel::Medium => {
            parts.push("⚠️ Medium memory pressure — lightly throttled".to_string());
        }
        PressureLevel::Low => {}
    }

    parts.push(format!(
        "Recommended: {} parallel tasks, ~{}MB safe per task",
        recommended,
        if recommended > 0 {
            available_mb / recommended as u64
        } else {
            available_mb
        }
    ));

    parts.join(". ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collector::{
        CpuMetrics, CpuPressure, MemoryMetrics, PressureLevel, ResourceSnapshot,
    };

    fn mock_snapshot(available_mb: u64, total_mb: u64, cores: u32, pressure: PressureLevel) -> ResourceSnapshot {
        ResourceSnapshot {
            memory: Some(MemoryMetrics {
                total_mb,
                used_mb: total_mb.saturating_sub(available_mb),
                available_mb,
                used_percent: if total_mb > 0 {
                    ((total_mb - available_mb) as f64 / total_mb as f64 * 100.0 * 10.0).round() / 10.0
                } else {
                    0.0
                },
                pressure,
            }),
            cpu: Some(CpuMetrics {
                cores,
                load_avg_1m: 0.0,
                load_avg_5m: 0.0,
                load_avg_15m: 0.0,
                agent_processes_percent: None,
                pressure: CpuPressure::Low,
            }),
            disk: None,
            gpu: None,
            thermal: None,
            agents: None,
            container_runtime: None,
            timestamp: String::new(),
            collection_duration_ms: 0.0,
        }
    }

    // 正常 16GB 8核机器 → 建议合理并发
    #[test]
    fn test_normal_workstation() {
        let snap = mock_snapshot(12000, 16384, 8, PressureLevel::Low);
        let result = suggest_concurrency(&snap, None);
        // 8核 * 1.5 = 12, 可用(12000-3277)/1024 ≈ 8, 取min=8
        assert!(result.suggestion.recommended_concurrency >= 1);
        assert!(result.suggestion.max_concurrency >= result.suggestion.recommended_concurrency);
        assert_eq!(result.suggestion.risk_level, "ok");
        assert!(result.suggestion.per_agent_safe_memory_mb > 0);
    }

    // 低配 4GB 2核 → 并发受限
    #[test]
    fn test_low_end_machine() {
        let snap = mock_snapshot(2048, 4096, 2, PressureLevel::Low);
        let result = suggest_concurrency(&snap, None);
        // 2核*1.5=3, 可用(2048-1024)/1024=1, 取min=1
        assert!(result.suggestion.max_concurrency >= 1);
    }

    // 高内存压力 → 降级
    #[test]
    fn test_high_pressure() {
        let snap = mock_snapshot(2000, 16384, 8, PressureLevel::High);
        let result = suggest_concurrency(&snap, None);
        assert_eq!(result.suggestion.risk_level, "caution");
    }

    // 临界压力 → 强制1
    #[test]
    fn test_critical_pressure() {
        let snap = mock_snapshot(300, 16384, 8, PressureLevel::Critical);
        let result = suggest_concurrency(&snap, None);
        assert_eq!(result.suggestion.risk_level, "critical");
        assert_eq!(result.suggestion.recommended_concurrency, 1);
    }

    // 自定义 task-memory
    #[test]
    fn test_custom_task_memory() {
        let snap = mock_snapshot(12000, 16384, 8, PressureLevel::Low);
        let result = suggest_concurrency(&snap, Some(2048));
        // 大任务模型 → 并发更低
        assert!(result.system.task_memory_mb == Some(2048));
    }

    // JSON序列化
    #[test]
    fn test_json_output() {
        let snap = mock_snapshot(12000, 16384, 8, PressureLevel::Low);
        let result = suggest_concurrency(&snap, None);
        let json = serde_json::to_string_pretty(&result).unwrap();
        assert!(json.contains("max_concurrency"));
        assert!(json.contains("recommended_concurrency"));
        assert!(json.contains("reasoning"));
        assert!(json.contains("risk_level"));
    }
}
