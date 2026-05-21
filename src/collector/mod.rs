pub mod linux;
pub mod macos;
pub mod registry;
pub mod disk;
pub mod cpu;
pub mod gpu;

use thiserror::Error;
use serde::Serialize;

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
        if a.priority() <= b.priority() { a } else { b }
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
    pub timestamp: String,
    pub collection_duration_ms: f64,
}
