pub mod linux;
pub mod macos;

use thiserror::Error;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct MemoryMetrics {
    pub total_mb: u64,
    pub used_mb: u64,
    pub available_mb: u64,
    pub used_percent: f64,
}

#[derive(Debug, Error)]
pub enum CollectError {
    #[error("Permission denied: {0}")]
    #[allow(dead_code)]
    PermissionDenied(String),
    #[error("Unsupported platform")]
    #[allow(dead_code)]
    UnsupportedPlatform,
    #[error("Failed to read memory info: {0}")]
    ReadFailed(String),
}

pub trait MemoryCollector {
    fn collect(&self) -> Result<MemoryMetrics, CollectError>;
}
