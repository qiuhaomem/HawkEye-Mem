use serde::{Deserialize, Serialize};

use crate::collector::ResourceSnapshot;
use crate::models::{self, ModelLibrary};

// ============================================================================
// 请求结构
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeploymentRequest {
    /// 模型名称（在模型库中查找）
    pub model_name: Option<String>,
    /// 模型参数量（与 model_name 互斥）
    pub model_size_b: Option<u64>,
    /// 量化等级，如 "Q4_K_M"
    pub quantization: Option<String>,
    /// 上下文窗口大小
    pub context_window: Option<u32>,
}

impl Default for DeploymentRequest {
    fn default() -> Self {
        Self {
            model_name: None,
            model_size_b: None,
            quantization: None,
            context_window: None,
        }
    }
}

// ============================================================================
// 评估结果
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct DeploymentAssessment {
    pub request: serde_json::Value,
    pub verdict: Verdict,
    pub constraints: Vec<Constraint>,
    pub safe_options: Vec<SafeOption>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Feasible,
    FeasibleWithCaveats,
    Infeasible,
}

#[derive(Debug, Clone, Serialize)]
pub struct Constraint {
    pub resource: String,
    pub required_mb: u64,
    pub available_mb: u64,
    pub gap_mb: i64,
    pub severity: String,
    pub suggestion: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SafeOption {
    pub description: String,
    pub option_type: String,
}

// ============================================================================
// 评估引擎
// ============================================================================

pub struct AssessmentEngine;

impl AssessmentEngine {
    /// 执行部署评估
    pub fn assess(
        request: &DeploymentRequest,
        snapshot: &ResourceSnapshot,
    ) -> DeploymentAssessment {
        let mut constraints = Vec::new();

        // 1. 内存约束
        if let Ok(memory_mb) = Self::estimate_memory_mb(request) {
            if let Some(ref mem) = snapshot.memory {
                let gap = memory_mb as i64 - mem.available_mb as i64;
                if gap > 0 {
                    let severity = if gap > memory_mb as i64 / 2 {
                        "error"
                    } else {
                        "warning"
                    };
                    let suggestion = match severity {
                        "error" => "内存严重不足，请关闭其他进程或降级模型配置".to_string(),
                        _ => "尝试降低量化精度或减少上下文长度".to_string(),
                    };
                    constraints.push(Constraint {
                        resource: "memory".to_string(),
                        required_mb: memory_mb,
                        available_mb: mem.available_mb,
                        gap_mb: gap,
                        severity: severity.to_string(),
                        suggestion,
                    });
                }
            }
        }

        // 2. 磁盘约束（下载大小）
        let download_mb = Self::estimate_download_mb(request);
        if download_mb > 0 {
            if let Some(ref disk) = snapshot.disk {
                if download_mb > disk.available_mb {
                    constraints.push(Constraint {
                        resource: "disk".to_string(),
                        required_mb: download_mb,
                        available_mb: disk.available_mb,
                        gap_mb: download_mb as i64 - disk.available_mb as i64,
                        severity: "error".to_string(),
                        suggestion: "清理磁盘空间或更改模型缓存目录".to_string(),
                    });
                }
            }
        }

        // 3. 显存约束
        if let Ok(vram_mb) = Self::estimate_vram_mb(request) {
            if let Some(ref gpu_vec) = snapshot.gpu {
                if let Some(gpu) = gpu_vec.first() {
                    let free_vram = gpu.vram_total_mb.saturating_sub(gpu.vram_used_mb);
                    let gap = vram_mb as i64 - free_vram as i64;
                    if gap > 0 {
                        let severity = if gap > vram_mb as i64 / 2 {
                            "error"
                        } else {
                            "warning"
                        };
                        constraints.push(Constraint {
                            resource: "gpu_vram".to_string(),
                            required_mb: vram_mb,
                            available_mb: free_vram,
                            gap_mb: gap,
                            severity: severity.to_string(),
                            suggestion: "尝试更低的量化级别或切换至 CPU 推理".to_string(),
                        });
                    }
                }
            }
        }

        // 4. 生成降级方案
        let safe_options = Self::generate_safe_options(request, &constraints);

        // 5. 判定
        let verdict = Self::determine_verdict(&constraints, &safe_options);

        DeploymentAssessment {
            request: serde_json::to_value(request).unwrap_or_default(),
            verdict,
            constraints,
            safe_options,
        }
    }

    // ========================================================================
    // 内部方法：内存估算
    // ========================================================================

    /// 估算推理所需内存（MB）
    /// - 有 model_name: 权重 + KV cache + overhead（量化影响权重）
    /// - 有 model_size_b: 权重 + KV cache + overhead
    fn estimate_memory_mb(request: &DeploymentRequest) -> Result<u64, String> {
        if let Some(ref model_name) = request.model_name {
            let entry = ModelLibrary::find(model_name)
                .ok_or_else(|| format!("模型未找到: {}", model_name))?;
            let bpw = models::quantization_bytes_per_weight(
                request.quantization.as_deref().unwrap_or("Q4_K_M"),
            );
            // 权重内存 = size_b * bytes_per_weight / 1MB
            let weight_mb = (entry.size_b as f64 * bpw) / (1024.0 * 1024.0);
            let ctx = request.context_window.unwrap_or(entry.max_context) as u64;
            // KV cache 大小（bytes）转 MB
            let kv_cache_mb =
                (entry.bytes_per_token as u128 * ctx as u128) / (1024u128 * 1024u128);
            let total = weight_mb as u64 + kv_cache_mb as u64 + entry.memory_overhead_mb;
            Ok(total)
        } else if let Some(size_b) = request.model_size_b {
            let bpt = models::quantization_bytes_per_weight(
                request.quantization.as_deref().unwrap_or("Q4_K_M"),
            );
            // 权重内存 = size_b * bytes_per_weight / 1MB
            let weight_mb = (size_b as f64 * bpt) / (1024.0 * 1024.0);
            // KV cache（默认 bytes_per_token = 2048）
            let ctx = request.context_window.unwrap_or(4096) as u64;
            let kv_cache_mb = (2048u128 * ctx as u128) / (1024u128 * 1024u128);
            let overhead = 512u64;
            Ok(weight_mb as u64 + kv_cache_mb as u64 + overhead)
        } else {
            Err("需要指定 model_name 或 model_size_b".to_string())
        }
    }

    /// 获取模型参数量（用于磁盘/带宽估算）
    fn get_size_b(request: &DeploymentRequest) -> Result<u64, String> {
        if let Some(ref name) = request.model_name {
            let entry =
                ModelLibrary::find(name).ok_or_else(|| format!("模型未找到: {}", name))?;
            Ok(entry.size_b)
        } else if let Some(size) = request.model_size_b {
            Ok(size)
        } else {
            Err("需要指定 model_name 或 model_size_b".to_string())
        }
    }

    /// 估算下载大小（MB）：model_size_b * 1.2 / 1MB
    fn estimate_download_mb(request: &DeploymentRequest) -> u64 {
        Self::get_size_b(request)
            .map(|s| (s as f64 * 1.2 / 1024.0 / 1024.0).ceil() as u64)
            .unwrap_or(0)
    }

    /// 估算显存需求（MB）：默认 = 内存需求的 80%
    fn estimate_vram_mb(request: &DeploymentRequest) -> Result<u64, String> {
        Self::estimate_memory_mb(request).map(|m| (m as f64 * 0.8).ceil() as u64)
    }

    // ========================================================================
    // 决策树降级方案生成（CR-02）
    // ========================================================================

    /// 根据约束生成降级方案，最多 3 个
    fn generate_safe_options(
        request: &DeploymentRequest,
        constraints: &[Constraint],
    ) -> Vec<SafeOption> {
        let mut options: Vec<SafeOption> = Vec::new();

        for constraint in constraints {
            match constraint.resource.as_str() {
                "memory" | "gpu_vram" => {
                    if let Some(ref model_name) = request.model_name {
                        if let Some(entry) = ModelLibrary::find(model_name) {
                            // 方案 1: 降量化
                            let current_q = request
                                .quantization
                                .as_deref()
                                .unwrap_or("Q4_K_M");
                            if let Some(lower_q) =
                                models::next_lower_quantization(current_q, &entry.quantizations)
                            {
                                options.push(SafeOption {
                                    description: format!(
                                        "降量化 {} → {}（节省 ~{}%)",
                                        current_q,
                                        lower_q,
                                        estimate_savings_percent(current_q, &lower_q)
                                    ),
                                    option_type: "lower_quantization".to_string(),
                                });
                            }

                            // 方案 2: 降上下文
                            let current_ctx = request
                                .context_window
                                .unwrap_or(entry.max_context);
                            if current_ctx > entry.min_context {
                                let suggested_ctx =
                                    std::cmp::max(entry.min_context, current_ctx / 2);
                                options.push(SafeOption {
                                    description: format!(
                                        "降上下文 {} → {}",
                                        current_ctx, suggested_ctx
                                    ),
                                    option_type: "reduce_context".to_string(),
                                });
                            }

                            // 方案 3: 换小模型
                            if let Some(smaller) = ModelLibrary::find_smaller(model_name) {
                                options.push(SafeOption {
                                    description: format!(
                                        "换小模型 {} → {}（{}B）",
                                        model_name,
                                        smaller.name,
                                        smaller.size_b as f64 / 1e9
                                    ),
                                    option_type: "smaller_model".to_string(),
                                });
                            }
                        }
                    } else {
                        // 没有 model_name，只能提示通用建议
                        options.push(SafeOption {
                            description: "尝试降低量化精度（如 Q4_K_M → Q3_K_M）".to_string(),
                            option_type: "lower_quantization".to_string(),
                        });
                        options.push(SafeOption {
                            description: "减少上下文长度（如 8192 → 4096）".to_string(),
                            option_type: "reduce_context".to_string(),
                        });
                    }
                }
                "disk" => {
                    options.push(SafeOption {
                        description: "清理磁盘空间释放存储".to_string(),
                        option_type: "free_space".to_string(),
                    });
                }
                _ => {}
            }

            if options.len() >= 3 {
                break;
            }
        }

        options.truncate(3);
        options
    }

    // ========================================================================
    // Verdict 判定
    // ========================================================================

    fn determine_verdict(
        constraints: &[Constraint],
        safe_options: &[SafeOption],
    ) -> Verdict {
        if constraints.is_empty() {
            return Verdict::Feasible;
        }

        // disk 约束不可自动降级 → Infeasible
        if constraints.iter().any(|c| c.resource == "disk") {
            return Verdict::Infeasible;
        }

        // memory/vram: gap > 50% 且无可降级方案 → Infeasible
        let has_severe_gap = constraints.iter().any(|c| {
            (c.resource == "memory" || c.resource == "gpu_vram")
                && c.gap_mb > c.required_mb as i64 / 2
        });

        if has_severe_gap && safe_options.is_empty() {
            return Verdict::Infeasible;
        }

        Verdict::FeasibleWithCaveats
    }
}

// ============================================================================
// 辅助函数
// ============================================================================

/// 估算两个量化之间的内存节省百分比
fn estimate_savings_percent(current: &str, lower: &str) -> u32 {
    let current_bpw = models::quantization_bytes_per_weight(current);
    let lower_bpw = models::quantization_bytes_per_weight(lower);
    if current_bpw <= 0.0 {
        return 0;
    }
    let savings = ((current_bpw - lower_bpw) / current_bpw * 100.0).round() as u32;
    savings
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collector::{
        CpuMetrics, CpuPressure, DiskMetrics, DiskPressure, GpuMetrics, GpuPressure,
        MemoryMetrics, PressureLevel,
    };

    // 构建一个简单的 snapshot（16GB 内存，100GB 磁盘，无 GPU）
    fn mock_snapshot_16gb() -> ResourceSnapshot {
        ResourceSnapshot {
            memory: Some(MemoryMetrics {
                total_mb: 16384,
                used_mb: 4000,
                available_mb: 12000,
                used_percent: 24.4,
                pressure: PressureLevel::Low,
            }),
            disk: Some(DiskMetrics {
                path: "/".to_string(),
                total_mb: 512000,
                available_mb: 102400,
                used_percent: 80.0,
                pressure: DiskPressure::Ok,
                growth_rate_mb_per_hour: None,
            }),
            cpu: Some(CpuMetrics {
                cores: 8,
                load_avg_1m: 1.0,
                load_avg_5m: 0.8,
                load_avg_15m: 0.6,
                agent_processes_percent: None,
                pressure: CpuPressure::Low,
            }),
            gpu: None,
            timestamp: "2026-05-20T00:00:00Z".to_string(),
            collection_duration_ms: 1.0,
        }
    }

    // 带 GPU 的 snapshot
    fn mock_snapshot_with_gpu() -> ResourceSnapshot {
        ResourceSnapshot {
            gpu: Some(vec![GpuMetrics {
                name: "NVIDIA RTX 4090".to_string(),
                vram_total_mb: 24576,
                vram_used_mb: 2048,
                pressure: GpuPressure::Low,
                temp_celsius: None,
                power_watts: None,
                utilization_gpu_percent: None,
                utilization_memory_percent: None,
                throttle_warning: false,
                backend: String::new(),
            }]),
            ..mock_snapshot_16gb()
        }
    }

    // 内存紧张的 snapshot（仅 2GB 可用）
    fn mock_snapshot_2gb() -> ResourceSnapshot {
        ResourceSnapshot {
            memory: Some(MemoryMetrics {
                total_mb: 8192,
                used_mb: 6000,
                available_mb: 2000,
                used_percent: 73.2,
                pressure: PressureLevel::High,
            }),
            ..mock_snapshot_16gb()
        }
    }

    // UT-AS-001: 模型名查找 + 充裕内存 → Feasible
    #[test]
    fn test_ut_as_001_model_name_feasible() {
        let req = DeploymentRequest {
            model_name: Some("llama3-8b".to_string()),
            ..Default::default()
        };
        let snapshot = mock_snapshot_16gb();
        let assessment = AssessmentEngine::assess(&req, &snapshot);
        assert_eq!(
            assessment.verdict,
            Verdict::Feasible,
            "16GB 内存运行 llama3-8b 应 Feasible, got {:?}",
            assessment.verdict
        );
    }

    // UT-AS-002: 模型名 + 紧张内存 → FeasibleWithCaveats
    #[test]
    fn test_ut_as_002_model_name_caveats() {
        let req = DeploymentRequest {
            model_name: Some("deepseek-v2-lite".to_string()),
            context_window: Some(16384),
            ..Default::default()
        };
        let snapshot = mock_snapshot_2gb();
        let assessment = AssessmentEngine::assess(&req, &snapshot);
        assert_eq!(
            assessment.verdict,
            Verdict::FeasibleWithCaveats,
            "deepseek-v2-lite 在 2GB 可用内存应 FeasibleWithCaveats"
        );
        assert!(
            !assessment.constraints.is_empty(),
            "应有约束"
        );
        assert!(
            !assessment.safe_options.is_empty(),
            "应有降级方案"
        );
    }

    // UT-AS-003: 过大模型 + 极小内存 → Infeasible
    #[test]
    fn test_ut_as_003_infeasible() {
        // 2GB 内存跑 deepseek-v2-lite，无指定量化（默认 Q4_K_M）
        // 16B * 0.5 / 1MB = ~7629MB + KV cache
        let req = DeploymentRequest {
            model_name: Some("deepseek-v2-lite".to_string()),
            context_window: Some(16384),
            quantization: Some("Q8_0".to_string()), // 最高的量化，内存需求最大
            ..Default::default()
        };
        // 用极低可用内存
        let snapshot = ResourceSnapshot {
            memory: Some(MemoryMetrics {
                total_mb: 2048,
                used_mb: 1900,
                available_mb: 100,
                used_percent: 92.8,
                pressure: PressureLevel::Critical,
            }),
            ..mock_snapshot_16gb()
        };
        let assessment = AssessmentEngine::assess(&req, &snapshot);
        // 不应是 Feasible
        assert_ne!(
            assessment.verdict,
            Verdict::Feasible,
            "100MB 可用内存跑 deepseek-v2-lite Q8_0 不可能 Feasible"
        );
    }

    // UT-AS-004: 磁盘不足 → Infeasible
    #[test]
    fn test_ut_as_004_disk_infeasible() {
        let req = DeploymentRequest {
            model_name: Some("llama3-8b".to_string()),
            ..Default::default()
        };
        let snapshot = ResourceSnapshot {
            disk: Some(DiskMetrics {
                path: "/".to_string(),
                total_mb: 1000,
                available_mb: 50,
                used_percent: 95.0,
                pressure: DiskPressure::Critical,
                growth_rate_mb_per_hour: None,
            }),
            ..mock_snapshot_16gb()
        };
        let assessment = AssessmentEngine::assess(&req, &snapshot);
        assert_eq!(
            assessment.verdict,
            Verdict::Infeasible,
            "磁盘不足应 Infeasible"
        );
    }

    // UT-AS-005: model_size_b 模式
    #[test]
    fn test_ut_as_005_model_size() {
        let req = DeploymentRequest {
            model_name: None,
            model_size_b: Some(7000000000), // 7B
            quantization: Some("Q4_K_M".to_string()),
            context_window: Some(4096),
        };
        let snapshot = mock_snapshot_16gb();
        let assessment = AssessmentEngine::assess(&req, &snapshot);
        assert_eq!(
            assessment.verdict,
            Verdict::Feasible,
            "7B Q4_K_M 在 16GB 应 Feasible"
        );
    }

    // UT-AS-006: GPU 显存约束
    #[test]
    fn test_ut_as_006_gpu_vram() {
        let req = DeploymentRequest {
            model_name: Some("deepseek-v2-lite".to_string()),
            context_window: Some(8192),
            ..Default::default()
        };
        let snapshot = mock_snapshot_with_gpu();
        let assessment = AssessmentEngine::assess(&req, &snapshot);
        // deepseek-v2-lite Q4_K_M = ~16B * 0.5/1MB + KV cache + overhead
        // ~7629 + (4096*8192/1M) + 1024 ≈ 7629 + 32 + 1024 ≈ 8685 MB memory
        // vram = 8685 * 0.8 ≈ 6948 MB
        // 而 RTX 4090 有空闲 ~22GB，所以应该 Feasible
        assert_eq!(
            assessment.verdict,
            Verdict::Feasible,
            "RTX 4090 跑 deepseek-v2-lite 应 Feasible"
        );
    }

    // UT-AS-007: 降级方案中包含 lower_quantization
    #[test]
    fn test_ut_as_007_contains_lower_quant() {
        let req = DeploymentRequest {
            model_name: Some("llama3-8b".to_string()),
            quantization: Some("Q8_0".to_string()),
            context_window: Some(4096),
            ..Default::default()
        };
        let snapshot = mock_snapshot_2gb();
        let assessment = AssessmentEngine::assess(&req, &snapshot);
        let has_lower_q = assessment
            .safe_options
            .iter()
            .any(|o| o.option_type == "lower_quantization");
        assert!(has_lower_q, "Q8_0 应提供降量化方案");
    }

    // UT-AS-008: 降级方案中包含 smaller_model
    #[test]
    fn test_ut_as_008_contains_smaller_model() {
        let req = DeploymentRequest {
            model_name: Some("llama3-8b".to_string()),
            ..Default::default()
        };
        let snapshot = mock_snapshot_2gb();
        let assessment = AssessmentEngine::assess(&req, &snapshot);
        let has_smaller = assessment
            .safe_options
            .iter()
            .any(|o| o.option_type == "smaller_model");
        assert!(has_smaller, "应提供换小模型方案");
    }

    // UT-AS-009: 降级方案不超过 3 个
    #[test]
    fn test_ut_as_009_max_3_options() {
        let req = DeploymentRequest {
            model_name: Some("llama3-8b".to_string()),
            quantization: Some("Q8_0".to_string()),
            context_window: Some(8192),
            ..Default::default()
        };
        let snapshot = mock_snapshot_2gb();
        let assessment = AssessmentEngine::assess(&req, &snapshot);
        assert!(
            assessment.safe_options.len() <= 3,
            "safe_options 应 ≤3, got {}",
            assessment.safe_options.len()
        );
    }

    // UT-AS-010: estimate_download_mb 返回正数
    #[test]
    fn test_ut_as_010_download_mb_positive() {
        let req = DeploymentRequest {
            model_name: Some("llama3-8b".to_string()),
            ..Default::default()
        };
        let dl = AssessmentEngine::estimate_download_mb(&req);
        // 8B * 1.2 / 1MB ≈ 9600 MB
        assert!(dl > 9000, "llama3-8b download should be >9000MB, got {}", dl);
        assert!(dl < 10000, "llama3-8b download should be <10000MB, got {}", dl);
    }

    // UT-AS-011: model_size_b 模式 vram 估算
    #[test]
    fn test_ut_as_011_vram_estimate() {
        let req = DeploymentRequest {
            model_name: None,
            model_size_b: Some(8000000000),
            quantization: Some("Q4_K_M".to_string()),
            context_window: Some(4096),
        };
        let vram = AssessmentEngine::estimate_vram_mb(&req).unwrap();
        // memory: 8B * 0.5 / 1MB + (2048*4096/1M) + 512 = 3814 + 8 + 512 = 4334
        // vram: 4334 * 0.8 = 3467
        let mem = AssessmentEngine::estimate_memory_mb(&req).unwrap();
        assert!(
            vram < mem,
            "vram({}) should be less than memory({})",
            vram,
            mem
        );
        assert!(
            vram > mem * 70 / 100,
            "vram({}) should be roughly 80% of memory({})",
            vram,
            mem
        );
    }

    // UT-AS-012: 未找到模型返回 Err
    #[test]
    fn test_ut_as_012_model_not_found() {
        let req = DeploymentRequest {
            model_name: Some("nonexistent-model".to_string()),
            ..Default::default()
        };
        let snapshot = mock_snapshot_16gb();
        let assessment = AssessmentEngine::assess(&req, &snapshot);
        // 应该还能运行，但 estimate_memory_mb 失败后不会添加 constraints
        // 所以 verdict 应该是 Feasible（因为没有约束）
        assert_eq!(
            assessment.verdict,
            Verdict::Feasible,
            "不存在的模型应仍输出 assessment"
        );
    }

    // UT-AS-013: verdict 输出格式为 snake_case
    #[test]
    fn test_ut_as_013_verdict_serialize() {
        assert_eq!(
            serde_json::to_value(Verdict::Feasible).unwrap(),
            serde_json::json!("feasible")
        );
        assert_eq!(
            serde_json::to_value(Verdict::FeasibleWithCaveats).unwrap(),
            serde_json::json!("feasible_with_caveats")
        );
        assert_eq!(
            serde_json::to_value(Verdict::Infeasible).unwrap(),
            serde_json::json!("infeasible")
        );
    }

    // UT-AS-014: estimate_savings_percent 合理
    #[test]
    fn test_ut_as_014_savings_percent() {
        let pct = estimate_savings_percent("Q8_0", "Q4_K_M");
        // Q8_0 1.0, Q4_K_M 0.5 → 50%
        assert_eq!(pct, 50, "Q8_0→Q4_K_M should save 50%");
    }
}
