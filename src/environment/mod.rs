//! 环境指纹引擎
//!
//! V0.4 核心模块：Agent 跨环境迁移时自动感知变化，重新评估部署建议。
//!
//! ## 模块结构
//!
//! - `mod.rs`: `EnvironmentFingerprint` 结构体、`EnvironmentChange`、指纹生成
//! - `detector.rs`: 变更检测 + 阈值判定（CR-02）
//! - `store.rs`: 指纹文件存储、轮转、HMAC签名

pub mod detector;
pub mod store;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// 环境指纹：一台机器的资源快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentFingerprint {
    /// 指纹 ID：SHA256(hostname + machine_id)，稳定不变
    pub id: String,
    /// 指纹创建时间 (RFC3339)
    pub created_at: String,
    /// 主机名哈希（前16位hex，脱敏）
    pub hostname: String,
    /// 操作系统：linux / macos / windows
    pub platform: String,
    /// CPU 核心数
    pub cpu_cores: u32,
    /// 总内存 (MB)
    pub total_memory_mb: u64,
    /// GPU 名称列表
    pub gpu_names: Vec<String>,
    /// 磁盘总容量 (MB)
    pub disk_total_mb: u64,
    /// 容器运行时：docker / kubernetes / null
    pub container_runtime: Option<String>,
}

/// 环境变化描述
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentChange {
    /// 变化的资源名：memory / cpu / gpu / disk / container
    pub resource: String,
    /// 变化前的值
    pub previous_label: String,
    /// 变化后的值
    pub current_label: String,
    /// 变化方向：upgrade / degrade
    pub direction: String,
    /// 变化幅度 (MB 或 核心数)
    pub delta: f64,
}

/// 变更检测结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentChangeReport {
    /// 是否检测到变更
    pub detected: bool,
    /// 前一次指纹 ID
    pub previous_fingerprint_id: Option<String>,
    /// 变更列表
    pub changes: Vec<EnvironmentChange>,
    /// 基于变化的建议文案
    pub new_recommendation: Option<String>,
}

/// 主机名脱敏：SHA256 → 前16位hex
pub fn hash_hostname(hostname: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(hostname.as_bytes());
    hex::encode(&hasher.finalize()[..8]) // 16 hex chars = 8 bytes
}

/// 生成稳定指纹 ID：SHA256(hostname)
pub fn generate_fingerprint_id(hostname: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(hostname.as_bytes());
    hex::encode(&hasher.finalize()[..16]) // 32 hex chars = 16 bytes
}

impl EnvironmentFingerprint {
    /// 从资源快照生成环境指纹
    pub fn generate(
        hostname: &str,
        platform: &str,
        cpu_cores: u32,
        total_memory_mb: u64,
        gpu_names: Vec<String>,
        disk_total_mb: u64,
        container_runtime: Option<String>,
    ) -> Self {
        let hostname_hash = hash_hostname(hostname);

        Self {
            id: generate_fingerprint_id(hostname),
            created_at: Utc::now().to_rfc3339(),
            hostname: hostname_hash,
            platform: platform.to_string(),
            cpu_cores,
            total_memory_mb,
            gpu_names,
            disk_total_mb,
            container_runtime,
        }
    }

    /// 检测当前指纹与之前指纹的差异（CR-02 阈值规则）
    pub fn detect_changes(&self, previous: &Self) -> Vec<EnvironmentChange> {
        let mut changes = Vec::new();

        // --- 内存变化 ---
        // CR-02: 绝对值变化 > 4GB 或 > 20% 总内存（取较大者）
        let mem_diff = (self.total_memory_mb as i64 - previous.total_memory_mb as i64).unsigned_abs();
        let mem_threshold = std::cmp::max(
            4096u64,                               // 4GB 绝对阈值
            previous.total_memory_mb * 20 / 100, // 20% 相对阈值
        );
        if mem_diff >= mem_threshold {
            changes.push(EnvironmentChange {
                resource: "memory".to_string(),
                previous_label: format!("{}MB", previous.total_memory_mb),
                current_label: format!("{}MB", self.total_memory_mb),
                direction: if self.total_memory_mb > previous.total_memory_mb {
                    "upgrade".to_string()
                } else {
                    "degrade".to_string()
                },
                delta: mem_diff as f64,
            });
        }

        // --- CPU 核心数变化 ≥2 ---
        let cpu_diff = (self.cpu_cores as i32 - previous.cpu_cores as i32).abs();
        if cpu_diff >= 2 {
            changes.push(EnvironmentChange {
                resource: "cpu".to_string(),
                previous_label: format!("{} cores", previous.cpu_cores),
                current_label: format!("{} cores", self.cpu_cores),
                direction: if self.cpu_cores > previous.cpu_cores {
                    "upgrade".to_string()
                } else {
                    "degrade".to_string()
                },
                delta: cpu_diff as f64,
            });
        }

        // --- GPU 增减 ---
        if self.gpu_names != previous.gpu_names {
            let added: Vec<&str> = self
                .gpu_names
                .iter()
                .filter(|g| !previous.gpu_names.contains(g))
                .map(|s| s.as_str())
                .collect();
            let removed: Vec<&str> = previous
                .gpu_names
                .iter()
                .filter(|g| !self.gpu_names.contains(g))
                .map(|s| s.as_str())
                .collect();

            if !added.is_empty() {
                changes.push(EnvironmentChange {
                    resource: "gpu".to_string(),
                    previous_label: format!("{:?}", previous.gpu_names),
                    current_label: format!("{:?}", self.gpu_names),
                    direction: "upgrade".to_string(),
                    delta: added.len() as f64,
                });
            }
            if !removed.is_empty() {
                changes.push(EnvironmentChange {
                    resource: "gpu".to_string(),
                    previous_label: format!("{:?}", previous.gpu_names),
                    current_label: format!("{:?}", self.gpu_names),
                    direction: "degrade".to_string(),
                    delta: removed.len() as f64,
                });
            }
        }

        // --- 磁盘大幅变化（≥100GB 或 ≥30%） ---
        let disk_diff = (self.disk_total_mb as i64 - previous.disk_total_mb as i64).unsigned_abs();
        let disk_threshold = std::cmp::max(
            102400u64,                         // 100GB
            previous.disk_total_mb * 30 / 100, // 30%
        );
        if disk_diff >= disk_threshold {
            changes.push(EnvironmentChange {
                resource: "disk".to_string(),
                previous_label: format!("{}MB", previous.disk_total_mb),
                current_label: format!("{}MB", self.disk_total_mb),
                direction: if self.disk_total_mb > previous.disk_total_mb {
                    "upgrade".to_string()
                } else {
                    "degrade".to_string()
                },
                delta: disk_diff as f64,
            });
        }

        // --- 容器运行时变化 ---
        if self.container_runtime != previous.container_runtime {
            let direction = match (&self.container_runtime, &previous.container_runtime) {
                // 从物理机/VM 进入容器 → degrade（资源受限）
                (Some(_), None) => "degrade",
                // 从容器的限制中出来 → upgrade
                (None, Some(_)) => "upgrade",
                // 容器类型切换
                _ => "degrade",
            };
            changes.push(EnvironmentChange {
                resource: "container".to_string(),
                previous_label: previous
                    .container_runtime
                    .clone()
                    .unwrap_or_else(|| "bare-metal".to_string()),
                current_label: self
                    .container_runtime
                    .clone()
                    .unwrap_or_else(|| "bare-metal".to_string()),
                direction: direction.to_string(),
                delta: 0.0,
            });
        }

        changes
    }

    /// 基于变更生成推荐文案（CR-04 动态文案）
    pub fn generate_recommendation(changes: &[EnvironmentChange]) -> String {
        if changes.is_empty() {
            return String::new();
        }

        let mut parts: Vec<String> = Vec::new();

        for change in changes {
            match change.resource.as_str() {
                "memory" => {
                    if change.direction == "upgrade" {
                        let new_mb: u64 = change
                            .current_label
                            .trim_end_matches("MB")
                            .parse()
                            .unwrap_or(0);
                        if new_mb >= 48000 {
                            parts.push(format!(
                                "Memory has increased to {}. You can now safely run larger models (e.g., 70B).",
                                change.current_label
                            ));
                        } else if new_mb >= 24000 {
                            parts.push(format!(
                                "Memory has increased to {}. Consider upgrading to a larger model or increasing context window.",
                                change.current_label
                            ));
                        } else {
                            parts.push(format!(
                                "Memory has increased from {} to {}. You may increase your context window.",
                                change.previous_label, change.current_label
                            ));
                        }
                    } else {
                        parts.push(format!(
                            "Memory decreased from {} to {}. Consider reducing context window or switching to a smaller model.",
                            change.previous_label, change.current_label
                        ));
                    }
                }
                "cpu" => {
                    if change.direction == "upgrade" {
                        parts.push(format!(
                            "CPU cores increased to {}. Better parallel processing available.",
                            change.current_label
                        ));
                    } else {
                        parts.push(format!(
                            "CPU cores reduced to {}. Consider lowering concurrent request count.",
                            change.current_label
                        ));
                    }
                }
                "gpu" => {
                    if change.direction == "upgrade" {
                        parts.push(
                            "New GPU detected — you can now switch to GPU inference backend."
                                .to_string(),
                        );
                    } else {
                        parts.push("GPU removed — inference will fall back to CPU.".to_string());
                    }
                }
                "disk" => {
                    if change.direction == "upgrade" {
                        parts.push(format!(
                            "Disk space increased to {}. More room for model caches.",
                            change.current_label
                        ));
                    } else {
                        parts.push(format!(
                            "Disk space decreased to {}. Consider cleaning model caches.",
                            change.current_label
                        ));
                    }
                }
                "container" => {
                    parts.push(format!(
                        "Runtime environment changed from {} to {}. Resource limits may apply.",
                        change.previous_label, change.current_label
                    ));
                }
                _ => {}
            }
        }

        if parts.is_empty() {
            return String::new();
        }

        parts.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // UT-ENV-001: 首次运行生成指纹
    #[test]
    fn test_ut_env_001_generate_fingerprint() {
        let fp = EnvironmentFingerprint::generate(
            "my-host",
            "linux",
            8,
            16384,
            vec!["RTX 3060".to_string()],
            512_000,
            None,
        );
        assert_eq!(fp.platform, "linux");
        assert_eq!(fp.cpu_cores, 8);
        assert_eq!(fp.total_memory_mb, 16384);
        assert_eq!(fp.gpu_names.len(), 1);
        assert!(fp.gpu_names[0].contains("3060"));
        assert_eq!(fp.disk_total_mb, 512_000);
        assert!(fp.container_runtime.is_none());
        assert!(!fp.id.is_empty());
        assert!(!fp.created_at.is_empty());
    }

    // UT-ENV-002: 主机名脱敏
    #[test]
    fn test_ut_env_002_hostname_hash() {
        let fp1 = EnvironmentFingerprint::generate(
            "production-server-01",
            "linux",
            4,
            8192,
            vec![],
            256_000,
            None,
        );
        let fp2 = EnvironmentFingerprint::generate(
            "production-server-01",
            "linux",
            4,
            8192,
            vec![],
            256_000,
            None,
        );
        // 同一主机名 → 相同哈希
        assert_eq!(fp1.hostname, fp2.hostname);
        // 是 hex 字符串（16字符）
        assert_eq!(fp1.hostname.len(), 16);
        // 不是明文
        assert!(!fp1.hostname.contains("production"));
    }

    // UT-ENV-003: 指纹ID稳定
    #[test]
    fn test_ut_env_003_fingerprint_id_stable() {
        let id1 = generate_fingerprint_id("my-machine");
        let id2 = generate_fingerprint_id("my-machine");
        assert_eq!(id1, id2);

        let id3 = generate_fingerprint_id("other-machine");
        assert_ne!(id1, id3);
    }

    // UT-ENV-007: 无GPU环境
    #[test]
    fn test_ut_env_007_no_gpu() {
        let fp =
            EnvironmentFingerprint::generate("no-gpu-box", "linux", 2, 4096, vec![], 100_000, None);
        assert!(fp.gpu_names.is_empty());
    }

    // UT-ENV-011: 内存小幅变化不触发（差2GB < 4GB且 < 20%）
    #[test]
    fn test_ut_env_011_small_mem_change_no_trigger() {
        let old =
            EnvironmentFingerprint::generate("host", "linux", 4, 16384, vec![], 100_000, None);
        let new =
            EnvironmentFingerprint::generate("host", "linux", 4, 18432, vec![], 100_000, None);
        // 16GB→18GB: 差2GB < 4GB, 占比12.5% < 20% → 不触发
        let changes = new.detect_changes(&old);
        assert!(changes.iter().all(|c| c.resource != "memory"));
    }

    // UT-ENV-010: 内存大幅升级触发
    #[test]
    fn test_ut_env_010_mem_big_upgrade() {
        let old =
            EnvironmentFingerprint::generate("host", "linux", 4, 16384, vec![], 100_000, None);
        let new =
            EnvironmentFingerprint::generate("host", "linux", 4, 65536, vec![], 100_000, None);
        // 16GB→64GB: 差48GB > 4GB → 触发
        let changes = new.detect_changes(&old);
        let mem_changes: Vec<_> = changes.iter().filter(|c| c.resource == "memory").collect();
        assert_eq!(mem_changes.len(), 1);
        assert_eq!(mem_changes[0].direction, "upgrade");
    }

    // UT-ENV-012: 内存变化超过20%触发
    #[test]
    fn test_ut_env_012_mem_20_percent_threshold() {
        let old =
            EnvironmentFingerprint::generate("host", "linux", 4, 16384, vec![], 100_000, None);
        let new =
            EnvironmentFingerprint::generate("host", "linux", 4, 20480, vec![], 100_000, None);
        // 16GB→20GB: 差4GB=4GB阈值, 占比25%>20% → 触发
        let changes = new.detect_changes(&old);
        let mem_changes: Vec<_> = changes.iter().filter(|c| c.resource == "memory").collect();
        assert_eq!(mem_changes.len(), 1);
        assert_eq!(mem_changes[0].direction, "upgrade");
    }

    // UT-ENV-013: CPU核心数变化≥2触发
    #[test]
    fn test_ut_env_013_cpu_change_trigger() {
        let old = EnvironmentFingerprint::generate("host", "linux", 4, 8192, vec![], 100_000, None);
        let new = EnvironmentFingerprint::generate("host", "linux", 8, 8192, vec![], 100_000, None);
        let changes = new.detect_changes(&old);
        let cpu_changes: Vec<_> = changes.iter().filter(|c| c.resource == "cpu").collect();
        assert_eq!(cpu_changes.len(), 1);
        assert_eq!(cpu_changes[0].direction, "upgrade");
    }

    // UT-ENV-014: CPU核心数变化<2不触发
    #[test]
    fn test_ut_env_014_cpu_small_change_no_trigger() {
        let old = EnvironmentFingerprint::generate("host", "linux", 4, 8192, vec![], 100_000, None);
        let new = EnvironmentFingerprint::generate("host", "linux", 5, 8192, vec![], 100_000, None);
        let changes = new.detect_changes(&old);
        assert!(changes.iter().all(|c| c.resource != "cpu"));
    }

    // UT-ENV-015: GPU增加触发
    #[test]
    fn test_ut_env_015_gpu_added() {
        let old = EnvironmentFingerprint::generate("host", "linux", 4, 8192, vec![], 100_000, None);
        let new = EnvironmentFingerprint::generate(
            "host",
            "linux",
            4,
            8192,
            vec!["RTX 4090".to_string()],
            100_000,
            None,
        );
        let changes = new.detect_changes(&old);
        let gpu_changes: Vec<_> = changes.iter().filter(|c| c.resource == "gpu").collect();
        assert!(!gpu_changes.is_empty());
        assert!(gpu_changes.iter().any(|c| c.direction == "upgrade"));
    }

    // UT-ENV-016: GPU减少触发
    #[test]
    fn test_ut_env_016_gpu_removed() {
        let old = EnvironmentFingerprint::generate(
            "host",
            "linux",
            4,
            8192,
            vec!["RTX 4090".to_string()],
            100_000,
            None,
        );
        let new = EnvironmentFingerprint::generate("host", "linux", 4, 8192, vec![], 100_000, None);
        let changes = new.detect_changes(&old);
        let gpu_changes: Vec<_> = changes.iter().filter(|c| c.resource == "gpu").collect();
        assert!(!gpu_changes.is_empty());
        assert!(gpu_changes.iter().any(|c| c.direction == "degrade"));
    }

    // UT-ENV-019A: 多维度同时变更
    #[test]
    fn test_ut_env_019a_multi_dimension_change() {
        let old =
            EnvironmentFingerprint::generate("host", "linux", 4, 16384, vec![], 100_000, None);
        let new = EnvironmentFingerprint::generate(
            "host",
            "linux",
            8,
            32768,
            vec!["RTX 4090".to_string()],
            100_000,
            None,
        );
        let changes = new.detect_changes(&old);
        // 至少3个change（memory+cpu+gpu）
        assert!(changes.len() >= 3);
        assert!(changes.iter().any(|c| c.resource == "memory"));
        assert!(changes.iter().any(|c| c.resource == "cpu"));
        assert!(changes.iter().any(|c| c.resource == "gpu"));
    }

    // UT-ENV-020: 升级后动态文案（内存大幅升级）
    #[test]
    fn test_ut_env_020_upgrade_recommendation() {
        let mem_change = EnvironmentChange {
            resource: "memory".to_string(),
            previous_label: "16384MB".to_string(),
            current_label: "65536MB".to_string(),
            direction: "upgrade".to_string(),
            delta: 49152.0,
        };
        let text = EnvironmentFingerprint::generate_recommendation(&[mem_change]);
        assert!(text.contains("larger models") || text.contains("70B"));
    }

    // UT-ENV-021: 降级后警告
    #[test]
    fn test_ut_env_021_degrade_recommendation() {
        let mem_change = EnvironmentChange {
            resource: "memory".to_string(),
            previous_label: "65536MB".to_string(),
            current_label: "8192MB".to_string(),
            direction: "degrade".to_string(),
            delta: 57344.0,
        };
        let text = EnvironmentFingerprint::generate_recommendation(&[mem_change]);
        assert!(text.contains("reducing context") || text.contains("smaller model"));
    }

    // UT-ENV-022: GPU新增建议
    #[test]
    fn test_ut_env_022_gpu_added_recommendation() {
        let gpu_change = EnvironmentChange {
            resource: "gpu".to_string(),
            previous_label: "[]".to_string(),
            current_label: "[\"RTX 4090\"]".to_string(),
            direction: "upgrade".to_string(),
            delta: 1.0,
        };
        let text = EnvironmentFingerprint::generate_recommendation(&[gpu_change]);
        assert!(text.contains("GPU inference"));
    }

    // UT-ENV-023: GPU移除建议
    #[test]
    fn test_ut_env_023_gpu_removed_recommendation() {
        let gpu_change = EnvironmentChange {
            resource: "gpu".to_string(),
            previous_label: "[\"RTX 4090\"]".to_string(),
            current_label: "[]".to_string(),
            direction: "degrade".to_string(),
            delta: 1.0,
        };
        let text = EnvironmentFingerprint::generate_recommendation(&[gpu_change]);
        assert!(text.contains("CPU"));
    }

    // UT-ENV-024: 多维度变化建议
    #[test]
    fn test_ut_env_024_multi_dimension_recommendation() {
        let changes = vec![
            EnvironmentChange {
                resource: "memory".to_string(),
                previous_label: "16384MB".to_string(),
                current_label: "65536MB".to_string(),
                direction: "upgrade".to_string(),
                delta: 49152.0,
            },
            EnvironmentChange {
                resource: "gpu".to_string(),
                previous_label: "[]".to_string(),
                current_label: "[\"RTX 4090\"]".to_string(),
                direction: "upgrade".to_string(),
                delta: 1.0,
            },
        ];
        let text = EnvironmentFingerprint::generate_recommendation(&changes);
        assert!(text.contains("larger models") || text.contains("70B"));
        assert!(text.contains("GPU inference"));
    }
}
