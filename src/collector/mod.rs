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
    /// 压力优先级数值（0=最低压力，3=最高压力）
    pub fn priority(&self) -> u8 {
        match self {
            PressureLevel::Low => 0,
            PressureLevel::Medium => 1,
            PressureLevel::High => 2,
            PressureLevel::Critical => 3,
        }
    }

    /// 返回两者中压力更低（更宽松）的一个
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
// Collector 输出枚举（CR-01：各 Collector 返回独立结果）
// ============================================================================

#[allow(dead_code)]
#[derive(Debug)]
pub enum CollectorOutput {
    Memory(MemoryMetrics),
    Disk(DiskMetrics),
    Cpu(CpuMetrics),
    Gpu(Vec<GpuMetrics>),
}

// ============================================================================
// ResourceCollector trait（统一接口）
// ============================================================================

pub trait ResourceCollector: Send + Sync {
    fn collect(&self) -> Result<CollectorOutput, CollectError>;
}

// ============================================================================
// 资源快照（Registry 组装后的结果）
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
    pub timestamp: String,
    pub collection_duration_ms: f64,
}
