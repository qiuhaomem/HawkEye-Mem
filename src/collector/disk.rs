use super::{CollectError, CollectorOutput, DiskMetrics, DiskPressure, ResourceCollector};
use std::path::Path;

/// 磁盘采集器：检测模型缓存目录的磁盘空间
pub struct DiskCollector {
    /// 手动配置的模型缓存路径
    model_cache_path: Option<String>,
}

impl DiskCollector {
    pub fn new(model_cache_path: Option<String>) -> Self {
        Self { model_cache_path }
    }

    /// 确定要监控的路径：优先使用配置路径，否则自动检测
    fn resolve_path(&self) -> Option<String> {
        // 1. 配置路径
        if let Some(ref path) = self.model_cache_path {
            if Path::new(path).exists() {
                return Some(path.clone());
            }
        }

        // 2. 自动检测：常见模型缓存路径
        let candidates = vec![
            "~/.cache/huggingface/",
            "~/.ollama/models/",
            "~/.lm-studio/models/",
            "./models/",
        ];

        for candidate in candidates {
            let expanded = if candidate.starts_with("~/") {
                if let Some(home) = dirs_next::home_dir() {
                    home.join(&candidate[2..]).to_string_lossy().to_string()
                } else {
                    continue;
                }
            } else {
                candidate.to_string()
            };

            if Path::new(&expanded).exists() {
                return Some(expanded);
            }
        }

        None
    }

    /// 对路径进行脱敏：/home/xxx/ → ~/
    fn sanitize_path(path: &str) -> String {
        if let Some(home) = dirs_next::home_dir() {
            let home_str = home.to_string_lossy().to_string();
            if path.starts_with(&home_str) {
                return path.replacen(&home_str, "~", 1);
            }
        }
        path.to_string()
    }

    /// 使用 POSIX statvfs（Linux）获取磁盘信息
    #[cfg(target_os = "linux")]
    fn get_disk_info(path: &str) -> Result<(u64, u64), CollectError> {
        let mut vfs: libc::statvfs = unsafe { std::mem::zeroed() };
        let c_path = std::ffi::CString::new(path)
            .map_err(|_| CollectError::ReadFailed("Invalid path".into()))?;
        let ret = unsafe { libc::statvfs(c_path.as_ptr(), &mut vfs) };
        if ret != 0 {
            return Err(CollectError::ReadFailed(format!(
                "statvfs failed for {}: {}",
                path,
                std::io::Error::last_os_error()
            )));
        }

        let block_size = vfs.f_frsize as u64;
        let total_blocks = vfs.f_blocks;
        let avail_blocks = vfs.f_bavail;

        let total_mb = (total_blocks * block_size) / (1024 * 1024);
        let avail_mb = (avail_blocks * block_size) / (1024 * 1024);

        Ok((total_mb, avail_mb))
    }

    /// macOS 版本：使用 statfs（BSD 风格）
    #[cfg(target_os = "macos")]
    fn get_disk_info(path: &str) -> Result<(u64, u64), CollectError> {
        let mut vfs: libc::statfs = unsafe { std::mem::zeroed() };
        let c_path = std::ffi::CString::new(path)
            .map_err(|_| CollectError::ReadFailed("Invalid path".into()))?;
        let ret = unsafe { libc::statfs(c_path.as_ptr(), &mut vfs) };
        if ret != 0 {
            return Err(CollectError::ReadFailed(format!(
                "statfs failed for {}: {}",
                path,
                std::io::Error::last_os_error()
            )));
        }

        let block_size = vfs.f_bsize as u64;
        let total_blocks = vfs.f_blocks;
        let avail_blocks = vfs.f_bavail;

        let total_mb = (total_blocks * block_size) / (1024 * 1024);
        let avail_mb = (avail_blocks * block_size) / (1024 * 1024);

        Ok((total_mb, avail_mb))
    }

    /// 不支持的平台兜底
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    fn get_disk_info(_path: &str) -> Result<(u64, u64), CollectError> {
        Err(CollectError::UnsupportedPlatform)
    }
}

impl ResourceCollector for DiskCollector {
    fn collect(&self) -> Result<CollectorOutput, CollectError> {
        let target_path = self.resolve_path().ok_or_else(|| {
            CollectError::ResourceNotAvailable("No model cache directory found".into())
        })?;

        let (total_mb, available_mb) = Self::get_disk_info(&target_path)?;

        let used_percent = if total_mb > 0 {
            let used_mb = total_mb.saturating_sub(available_mb);
            (used_mb as f64 / total_mb as f64 * 100.0 * 10.0).round() / 10.0
        } else {
            0.0
        };

        // 压力判定：假设模型缓存所需空间 = 10240 MB（10GB）
        // 可用 > 2倍所需 → Ok，1.2~2倍 → Warning，< 1.2倍 → Critical
        let required_mb: u64 = 10240;
        let pressure = if available_mb > required_mb * 2 {
            DiskPressure::Ok
        } else if available_mb > (required_mb as f64 * 1.2) as u64 {
            DiskPressure::Warning
        } else {
            DiskPressure::Critical
        };

        let sanitized_path = Self::sanitize_path(&target_path);

        Ok(CollectorOutput::Disk(DiskMetrics {
            path: sanitized_path,
            total_mb,
            available_mb,
            used_percent,
            pressure,
            growth_rate_mb_per_hour: None,
        }))
    }
}
