pub mod cpu;
pub mod disk;
pub mod gpu;
pub mod linux;
pub mod macos;
pub mod registry;

use serde::Serialize;
use thiserror::Error;

// ============================================================================
// 资源指标结构体
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct MemoryMetrics {
    pub total_mb: u64,
    pub used_mb: u64,
    pub available_mb: u64,
    pub used_percent: f64,
    pub pressure: PressureLevel,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskMetrics {
    pub path: String,
    pub total_mb: u64,
    pub available_mb: u64,
    pub used_percent: f64,
    pub pressure: DiskPressure,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub growth_rate_mb_per_hour: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CpuMetrics {
    pub cores: u32,
    pub load_avg_1m: f64,
    pub load_avg_5m: f64,
    pub load_avg_15m: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_processes_percent: Option<f64>,
    pub pressure: CpuPressure,
}

#[derive(Debug, Clone, Serialize)]
pub struct GpuMetrics {
    pub name: String,
    pub vram_total_mb: u64,
    pub vram_used_mb: u64,
    pub pressure: GpuPressure,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temp_celsius: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub power_watts: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub utilization_gpu_percent: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub utilization_memory_percent: Option<u32>,
    #[serde(default)]
    pub throttle_warning: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub backend: String,
}

// ============================================================================
// 温度指标结构体（V0.3 Phase 5）
// ============================================================================

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum CpuThermalPressure {
    #[serde(rename = "normal")]
    Normal,
    #[serde(rename = "warning")]
    Warning,
    #[serde(rename = "critical")]
    Critical,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThermalMetrics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_temp_c: Option<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub gpu_temps_c: Vec<Option<f64>>,
    pub pressure: CpuThermalPressure,
    pub note: String,
}

// ============================================================================
// 多 Agent 检测结构体（V0.3 Phase 6）
// ============================================================================

/// Agent 进程信息
#[derive(Debug, Clone, Serialize)]
pub struct AgentProcess {
    pub name: String,
    pub pid: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_rss_mb: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_percent: Option<f64>,
}

/// 多 Agent 检测结果
#[derive(Debug, Clone, Serialize)]
pub struct AgentDetection {
    pub agents: Vec<AgentProcess>,
    pub count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_agent_memory_mb: Option<u64>,
    pub total_agent_cpu_percent: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_pressure: Option<String>,
    pub note: String,
}

// ============================================================================
// 压力等级枚举
// ============================================================================

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum PressureLevel {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "critical")]
    Critical,
}

impl std::fmt::Display for PressureLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PressureLevel::Low => write!(f, "low"),
            PressureLevel::Medium => write!(f, "medium"),
            PressureLevel::High => write!(f, "high"),
            PressureLevel::Critical => write!(f, "critical"),
        }
    }
}

impl PressureLevel {
    pub fn priority(&self) -> u8 {
        match self {
            PressureLevel::Low => 0,
            PressureLevel::Medium => 1,
            PressureLevel::High => 2,
            PressureLevel::Critical => 3,
        }
    }

    pub fn min(a: Self, b: Self) -> Self {
        if a.priority() <= b.priority() {
            a
        } else {
            b
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum DiskPressure {
    #[serde(rename = "ok")]
    Ok,
    #[serde(rename = "warning")]
    Warning,
    #[serde(rename = "critical")]
    Critical,
}

impl std::fmt::Display for DiskPressure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiskPressure::Ok => write!(f, "ok"),
            DiskPressure::Warning => write!(f, "warning"),
            DiskPressure::Critical => write!(f, "critical"),
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum CpuPressure {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
}

impl std::fmt::Display for CpuPressure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CpuPressure::Low => write!(f, "low"),
            CpuPressure::Medium => write!(f, "medium"),
            CpuPressure::High => write!(f, "high"),
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum GpuPressure {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
}

impl std::fmt::Display for GpuPressure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuPressure::Low => write!(f, "low"),
            GpuPressure::Medium => write!(f, "medium"),
            GpuPressure::High => write!(f, "high"),
        }
    }
}

// ============================================================================
// Collector 错误
// ============================================================================

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum CollectError {
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Unsupported platform")]
    UnsupportedPlatform,
    #[error("Failed to read resource info: {0}")]
    ReadFailed(String),
    #[error("Resource not available: {0}")]
    ResourceNotAvailable(String),
}

// ============================================================================
// Collector 输出枚举
// ============================================================================

#[allow(dead_code)]
#[derive(Debug)]
pub enum CollectorOutput {
    Memory(MemoryMetrics),
    Disk(DiskMetrics),
    Cpu(CpuMetrics),
    Gpu(Vec<GpuMetrics>),
    Thermal(ThermalMetrics),
    Agent(AgentDetection),
}

// ============================================================================
// ResourceCollector trait
// ============================================================================

pub trait ResourceCollector: Send + Sync {
    fn collect(&self) -> Result<CollectorOutput, CollectError>;
}

// ============================================================================
// 资源快照
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct ResourceSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<MemoryMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk: Option<DiskMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu: Option<CpuMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu: Option<Vec<GpuMetrics>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thermal: Option<ThermalMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agents: Option<AgentDetection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_runtime: Option<String>,
    pub timestamp: String,
    pub collection_duration_ms: f64,
}

// ============================================================================
// 单元测试 — AgentDetection 增强（V0.4）
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // UT-MAV-001: AgentDetection 可正确构造并序列化
    #[test]
    fn test_ut_mav_001_agent_detection_struct() {
        let detection = AgentDetection {
            agents: vec![
                AgentProcess {
                    name: "hermes".to_string(),
                    pid: 12345,
                    memory_rss_mb: Some(256),
                    cpu_percent: Some(2.5),
                },
                AgentProcess {
                    name: "claude-code".to_string(),
                    pid: 12346,
                    memory_rss_mb: Some(512),
                    cpu_percent: Some(1.5),
                },
            ],
            count: 2,
            total_agent_memory_mb: Some(768),
            total_agent_cpu_percent: 4.0,
            global_pressure: None,
            note: "test".to_string(),
        };

        let json = serde_json::to_value(&detection).unwrap();
        assert_eq!(json["count"], 2);
        assert_eq!(json["total_agent_memory_mb"], 768);
        assert_eq!(json["total_agent_cpu_percent"].as_f64().unwrap(), 4.0);
        assert_eq!(json["agents"].as_array().unwrap().len(), 2);
        assert_eq!(json["agents"][0]["name"], "hermes");
        assert_eq!(json["agents"][0]["pid"], 12345);
        assert_eq!(json["agents"][0]["memory_rss_mb"], 256);
        assert_eq!(json["agents"][0]["cpu_percent"].as_f64().unwrap(), 2.5);
    }

    // UT-MAV-003: 每个 Agent 包含完整资源详情
    #[test]
    fn test_ut_mav_003_agent_resource_details() {
        let agent = AgentProcess {
            name: "deepseek-tui".to_string(),
            pid: 99999,
            memory_rss_mb: Some(1024),
            cpu_percent: Some(5.0),
        };
        let json = serde_json::to_value(&agent).unwrap();
        assert_eq!(json["name"], "deepseek-tui");
        assert_eq!(json["pid"], 99999);
        assert_eq!(json["memory_rss_mb"], 1024);
        assert_eq!(json["cpu_percent"].as_f64().unwrap(), 5.0);
    }

    // UT-MAV-004: 总 Agent 内存在 JSON 中正确输出
    #[test]
    fn test_ut_mav_004_total_agent_memory_output() {
        let detection = AgentDetection {
            agents: vec![
                AgentProcess {
                    name: "agent-a".to_string(),
                    pid: 1,
                    memory_rss_mb: Some(100),
                    cpu_percent: Some(1.0),
                },
                AgentProcess {
                    name: "agent-b".to_string(),
                    pid: 2,
                    memory_rss_mb: Some(200),
                    cpu_percent: Some(2.0),
                },
            ],
            count: 2,
            total_agent_memory_mb: Some(300),
            total_agent_cpu_percent: 3.0,
            global_pressure: None,
            note: String::new(),
        };
        let json = serde_json::to_value(&detection).unwrap();
        assert_eq!(json["total_agent_memory_mb"], 300);
        assert_eq!(json["total_agent_cpu_percent"].as_f64().unwrap(), 3.0);
    }

    // UT-MAV-008: 无 Agent 运行时 total_agent_memory_mb 为 null（skip）
    #[test]
    fn test_ut_mav_008_no_agents() {
        let detection = AgentDetection {
            agents: vec![],
            count: 0,
            total_agent_memory_mb: None,
            total_agent_cpu_percent: 0.0,
            global_pressure: None,
            note: "no agents".to_string(),
        };
        let json = serde_json::to_value(&detection).unwrap();
        assert_eq!(json["count"], 0);
        // total_agent_memory_mb 应为 null（被 skip_serializing_if 隐藏）
        assert!(
            !json
                .as_object()
                .unwrap()
                .contains_key("total_agent_memory_mb"),
            "无 Agent 时 total_agent_memory_mb 应被序列化跳过"
        );
        // total_agent_cpu_percent 始终输出（即使 0.0）
        assert_eq!(json["total_agent_cpu_percent"].as_f64().unwrap(), 0.0);
        assert!(json["agents"].as_array().unwrap().is_empty());
    }

    // UT-MAV-009: 进程已退出 — memory_rss_mb 和 cpu_percent 可能为 None
    #[test]
    fn test_ut_mav_009_exited_process() {
        // 模拟进程已退出的情况：memory_rss_mb = None, cpu_percent = None
        let agent = AgentProcess {
            name: "ghost-agent".to_string(),
            pid: 0,
            memory_rss_mb: None,
            cpu_percent: None,
        };
        let json = serde_json::to_value(&agent).unwrap();
        assert_eq!(json["name"], "ghost-agent");
        assert_eq!(json["pid"], 0);
        // memory_rss_mb 和 cpu_percent 应为 null（被 skip_serializing_if 隐藏）
        assert!(
            !json.as_object().unwrap().contains_key("memory_rss_mb"),
            "None 的 memory_rss_mb 应被跳过"
        );
        assert!(
            !json.as_object().unwrap().contains_key("cpu_percent"),
            "None 的 cpu_percent 应被跳过"
        );
    }
}
