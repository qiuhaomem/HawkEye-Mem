// ============================================================================
// src/gpu/mod.rs — GPU Collector trait + 工厂函数（V0.3 Phase 4）
//
// 根据运行环境自动选择最佳采集路径：
//   路径A（NVML）：    feature="gpu" 时，通过 NVML API 直读 NVIDIA GPU
//   路径B（nvidia-smi）：NVML 不可用时回退到命令解析
//   路径C（ROCm）：    feature="gpu" 时，通过 rocm-smi 解析 AMD GPU
//
// 所有 unsafe FFI 集中在 nvml.rs 单一模块（CR-01 安全要求）。
// rocm-smi 路径为纯 safe Rust 解析。
// ============================================================================

pub mod nvml;
pub mod amd;

use crate::collector::{CollectError, CollectorOutput, GpuMetrics, GpuPressure, ResourceCollector};

/// GPU 采集器工厂
///
/// 返回一个 Box<dyn ResourceCollector>，内部根据可用 GPU 后端自动选择路径。
/// 优先级：NVML > nvidia-smi > ROCm > None
#[allow(dead_code)]
pub fn create_gpu_collector() -> Box<dyn ResourceCollector> {
    Box::new(GpuCollector)
}

/// GPU 显存采集器
///
/// 无状态，可全局共享。检测 NVIDIA/AMD GPU 资源使用情况。
pub struct GpuCollector;

impl GpuCollector {
    // ========================================================================
    // 路径A：NVML（需要 feature = "gpu"）
    // ========================================================================

    /// 通过 NVML 读取 GPU 完整信息（VRAM + 温度 + 功耗 + 利用率）
    #[cfg(feature = "gpu")]
    fn collect_nvml() -> Result<Vec<GpuMetrics>, String> {
        nvml::collect_all()
    }

    // ========================================================================
    // 路径B：nvidia-smi 命令解析
    // ========================================================================

    /// 通过执行 `nvidia-smi` 并解析 CSV 输出来获取 GPU 信息
    fn collect_nvidia_smi() -> Result<Vec<GpuMetrics>, String> {
        let output = std::process::Command::new("nvidia-smi")
            .args([
                "--query-gpu=name,memory.total,memory.used,temperature.gpu,power.draw,utilization.gpu,utilization.memory",
                "--format=csv,nounits",
            ])
            .output()
            .map_err(|_| "GPU monitoring unavailable".to_string())?;

        if !output.status.success() {
            return Err("GPU monitoring unavailable".to_string());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Self::parse_nvidia_smi_output(&stdout)
    }

    /// 解析 nvidia-smi CSV 输出（CR-06：按 header 匹配列索引，不硬编码）
    pub(crate) fn parse_nvidia_smi_output(output: &str) -> Result<Vec<GpuMetrics>, String> {
        let mut lines = output.lines();

        // 第一行是 CSV header，解析列索引
        let header = lines.next().ok_or_else(|| "Empty nvidia-smi output".to_string())?;
        let headers = parse_csv_line(header)?;

        // 必需字段索引（降级：仅 name, memory.total, memory.used 为必需）
        let name_idx = headers
            .iter()
            .position(|h| h.trim().to_lowercase().contains("name"))
            .ok_or_else(|| "Missing 'name' column".to_string())?;
        let total_idx = headers
            .iter()
            .position(|h| h.trim().to_lowercase().contains("memory.total"))
            .ok_or_else(|| "Missing 'memory.total' column".to_string())?;
        let used_idx = headers
            .iter()
            .position(|h| h.trim().to_lowercase().contains("memory.used"))
            .ok_or_else(|| "Missing 'memory.used' column".to_string())?;

        // 可选字段索引（不存在时优雅降级为 None）
        let temp_idx = headers
            .iter()
            .position(|h| h.trim().to_lowercase().contains("temperature.gpu"));
        let power_idx = headers
            .iter()
            .position(|h| h.trim().to_lowercase().contains("power.draw"));
        let util_gpu_idx = headers
            .iter()
            .position(|h| h.trim().to_lowercase().contains("utilization.gpu"));
        let util_mem_idx = headers
            .iter()
            .position(|h| h.trim().to_lowercase().contains("utilization.memory"));

        let max_idx = [name_idx, total_idx, used_idx]
            .iter()
            .chain(temp_idx.iter())
            .chain(power_idx.iter())
            .chain(util_gpu_idx.iter())
            .chain(util_mem_idx.iter())
            .max()
            .copied()
            .unwrap_or(0);

        let mut metrics = Vec::new();

        for line in lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let cols = parse_csv_line(line)?;
            if cols.len() <= max_idx {
                continue;
            }

            let gpu_name = cols[name_idx].trim().to_string();
            let total_mb: u64 = cols[total_idx]
                .trim()
                .parse()
                .map_err(|_| "Failed to parse memory.total value".to_string())?;
            let used_mb: u64 = cols[used_idx]
                .trim()
                .parse()
                .map_err(|_| "Failed to parse memory.used value".to_string())?;

            let pressure = Self::calc_pressure(total_mb, used_mb);

            // 可选字段：温度（°C）—— [N/A] 或空字符串 → None
            let temp_celsius = temp_idx.and_then(|idx| {
                let v = cols.get(idx)?.trim();
                if v.is_empty() || v.eq_ignore_ascii_case("[N/A]") || v.eq_ignore_ascii_case("N/A") {
                    return None;
                }
                v.parse::<f64>().ok()
            });

            // 可选字段：功耗（W）—— [N/A] 或空字符串 → None
            let power_watts = power_idx.and_then(|idx| {
                let v = cols.get(idx)?.trim();
                if v.is_empty() || v.eq_ignore_ascii_case("[N/A]") || v.eq_ignore_ascii_case("N/A") {
                    return None;
                }
                v.parse::<f64>().ok()
            });

            // 可选字段：GPU 利用率（%）—— [N/A] 或空字符串 → None
            let utilization_gpu_percent = util_gpu_idx.and_then(|idx| {
                let v = cols.get(idx)?.trim();
                if v.is_empty() || v.eq_ignore_ascii_case("[N/A]") || v.eq_ignore_ascii_case("N/A") {
                    return None;
                }
                v.parse::<u32>().ok()
            });

            // 可选字段：显存利用率（%）—— [N/A] 或空字符串 → None
            let utilization_memory_percent = util_mem_idx.and_then(|idx| {
                let v = cols.get(idx)?.trim();
                if v.is_empty() || v.eq_ignore_ascii_case("[N/A]") || v.eq_ignore_ascii_case("N/A") {
                    return None;
                }
                v.parse::<u32>().ok()
            });

            // 温度节流警告
            let throttle_warning = temp_celsius.map_or(false, |t| t > 90.0);

            metrics.push(GpuMetrics {
                name: gpu_name,
                vram_total_mb: total_mb,
                vram_used_mb: used_mb,
                pressure,
                temp_celsius,
                power_watts,
                utilization_gpu_percent,
                utilization_memory_percent,
                throttle_warning,
                backend: "nvidia-smi".to_string(),
            });
        }

        if metrics.is_empty() {
            return Err("No GPU data found".to_string());
        }

        Ok(metrics)
    }

    // ========================================================================
    // 路径C：AMD ROCm（rocm-smi）
    // ========================================================================

    /// 通过执行 `rocm-smi` 并解析输出获取 AMD GPU 信息
    fn collect_rocm_smi(rocm_smi_path: Option<&str>) -> Result<Vec<GpuMetrics>, String> {
        let binary = rocm_smi_path.unwrap_or("rocm-smi");

        // rocm-smi --showmeminfo vram --csv 输出所有 GPU 的 VRAM 信息
        let output = std::process::Command::new(binary)
            .args([
                "--showmeminfo", "vram",
                "--showtemp",
                "--showuse",
                "--csv",
            ])
            .output()
            .map_err(|e| format!("Failed to execute {}: {}", binary, e))?;

        if !output.status.success() {
            return Err(format!("{} command failed", binary));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        amd::parse_rocm_smi_output(&stdout)
    }

    // ========================================================================
    // 压力判定
    // ========================================================================

    /// 根据可用显存百分比判定压力等级
    /// available > 50% → Low, 20-50% → Medium, < 20% → High
    pub fn calc_pressure(total_mb: u64, used_mb: u64) -> GpuPressure {
        let available_pct = if total_mb > 0 {
            (total_mb.saturating_sub(used_mb)) as f64 / total_mb as f64 * 100.0
        } else {
            0.0
        };

        if available_pct > 50.0 {
            GpuPressure::Low
        } else if available_pct >= 20.0 {
            GpuPressure::Medium
        } else {
            GpuPressure::High
        }
    }

    /// 温度压力判定（辅助函数，用于 guidance 等上层模块）
    pub fn calc_temperature_pressure(temp_celsius: f64) -> &'static str {
        if temp_celsius > 95.0 {
            "critical"
        } else if temp_celsius > 85.0 {
            "warning"
        } else {
            "normal"
        }
    }
}

impl ResourceCollector for GpuCollector {
    /// 采集 GPU 指标
    ///
    /// 流程：
    /// 1. 如果启用 gpu feature，先尝试 NVML 路径
    /// 2. NVML 失败或未启用 feature，回退到 nvidia-smi 解析
    /// 3. NVIDIA 路径都失败，尝试 ROCm
    /// 4. 都失败则返回 ResourceNotAvailable（不 panic）
    fn collect(&self) -> Result<CollectorOutput, CollectError> {
        // 路径A：NVML（需要 feature gpu）
        #[cfg(feature = "gpu")]
        if let Ok(metrics) = Self::collect_nvml() {
            return Ok(CollectorOutput::Gpu(metrics));
        }

        // 路径B：nvidia-smi（universal 回退）
        if let Ok(metrics) = Self::collect_nvidia_smi() {
            return Ok(CollectorOutput::Gpu(metrics));
        }

        // 路径C：AMD ROCm（feature gpu 时尝试）
        #[cfg(feature = "gpu")]
        {
            // 尝试从配置读取 rocm_smi_path
            let rocm_path = crate::config::AppConfig::load(None)
                .ok()
                .flatten()
                .and_then(|c| c.gpu)
                .and_then(|g| g.rocm_smi_path);
            if let Ok(metrics) = Self::collect_rocm_smi(rocm_path.as_deref()) {
                return Ok(CollectorOutput::Gpu(metrics));
            }
        }

        // 全部不可用 —— 脱敏错误信息（CR-07）
        Err(CollectError::ResourceNotAvailable("No GPU detected (NVIDIA/AMD)".into()))
    }
}

// ============================================================================
// CSV 解析工具（CR-06：支持引号包裹字段，不硬编码列索引）
// ============================================================================

/// 解析单行 CSV，支持双引号包裹的字段（含逗号）
///
/// nvidia-smi 输出中 GPU 名称可能包含逗号（如 "NVIDIA A100, 80GB SXM"），
/// 此时名称字段会被双引号包裹。此解析器正确处理这种场景。
pub(crate) fn parse_csv_line(line: &str) -> Result<Vec<String>, String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            '"' => {
                if in_quotes && i + 1 < chars.len() && chars[i + 1] == '"' {
                    // 转义的双引号（"" → "）
                    current.push('"');
                    i += 2;
                    continue;
                }
                in_quotes = !in_quotes;
            }
            ',' if !in_quotes => {
                fields.push(current.trim().to_string());
                current = String::new();
            }
            c => {
                current.push(c);
            }
        }
        i += 1;
    }
    fields.push(current.trim().to_string());

    Ok(fields)
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collector::ResourceCollector;

    // UT-GPU-001: 解析 nvidia-smi 单 GPU 输出（含新增字段）
    #[test]
    fn test_ut_gpu_001_parse_single_gpu() {
        let csv = "\
name, memory.total, memory.used, temperature.gpu, power.draw, utilization.gpu, utilization.memory
NVIDIA GeForce RTX 3090, 24576, 10240, 65, 250.0, 85, 40
";
        let result = GpuCollector::parse_nvidia_smi_output(csv);
        assert!(result.is_ok(), "should parse single GPU: {:?}", result.err());
        let metrics = result.unwrap();
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].name, "NVIDIA GeForce RTX 3090");
        assert_eq!(metrics[0].vram_total_mb, 24576);
        assert_eq!(metrics[0].vram_used_mb, 10240);
        assert_eq!(metrics[0].temp_celsius, Some(65.0));
        assert_eq!(metrics[0].power_watts, Some(250.0));
        assert_eq!(metrics[0].utilization_gpu_percent, Some(85));
        assert_eq!(metrics[0].utilization_memory_percent, Some(40));
        assert!(!metrics[0].throttle_warning, "65°C 不应触发节流警告");
        assert_eq!(metrics[0].backend, "nvidia-smi");
    }

    // UT-GPU-002: 解析多 GPU
    #[test]
    fn test_ut_gpu_002_parse_multi_gpu() {
        let csv = "\
name, memory.total, memory.used, temperature.gpu, power.draw, utilization.gpu, utilization.memory
NVIDIA A100, 81920, 40960, 55, 300.0, 90, 60
NVIDIA A100, 81920, 20480, 50, 280.0, 75, 35
";
        let result = GpuCollector::parse_nvidia_smi_output(csv);
        assert!(result.is_ok(), "should parse multi GPU");
        let metrics = result.unwrap();
        assert_eq!(metrics.len(), 2);
        assert_eq!(metrics[1].vram_used_mb, 20480);
        assert_eq!(metrics[1].temp_celsius, Some(50.0));
    }

    // UT-GPU-003: 解析含逗号（引号包裹）的 GPU 名称
    #[test]
    fn test_ut_gpu_003_parse_quoted_name() {
        let csv = "\
name, memory.total, memory.used, temperature.gpu, power.draw, utilization.gpu, utilization.memory
\"NVIDIA A100, 80GB SXM\", 81920, 40960, 55, 300.0, 90, 60
";
        let result = GpuCollector::parse_nvidia_smi_output(csv);
        assert!(result.is_ok(), "should parse quoted name: {:?}", result.err());
        let metrics = result.unwrap();
        assert_eq!(metrics[0].name, "NVIDIA A100, 80GB SXM");
    }

    // UT-GPU-004: 空输出返回错误
    #[test]
    fn test_ut_gpu_004_empty_output() {
        let csv = "name, memory.total, memory.used\n";
        let result = GpuCollector::parse_nvidia_smi_output(csv);
        assert!(result.is_err(), "empty data should error");
    }

    // UT-GPU-005: 列顺序无关（按 header 匹配）
    #[test]
    fn test_ut_gpu_005_column_order_independent() {
        let csv = "\
memory.total, temperature.gpu, name, memory.used
24576, 65, NVIDIA RTX 3090, 10240
";
        let result = GpuCollector::parse_nvidia_smi_output(csv);
        assert!(result.is_ok(), "should handle reordered columns");
        let metrics = result.unwrap();
        assert_eq!(metrics[0].name, "NVIDIA RTX 3090");
        assert_eq!(metrics[0].vram_total_mb, 24576);
        assert_eq!(metrics[0].vram_used_mb, 10240);
        assert_eq!(metrics[0].temp_celsius, Some(65.0));
        // power/utility 字段不存在 → None
        assert_eq!(metrics[0].power_watts, None);
        assert_eq!(metrics[0].utilization_gpu_percent, None);
    }

    // UT-GPU-006: 压力判定——Low
    #[test]
    fn test_ut_gpu_006_pressure_low() {
        let p = GpuCollector::calc_pressure(81920, 10000);
        assert_eq!(p, GpuPressure::Low);
    }

    // UT-GPU-007: 压力判定——Medium
    #[test]
    fn test_ut_gpu_007_pressure_medium() {
        let p = GpuCollector::calc_pressure(81920, 60000);
        assert_eq!(p, GpuPressure::Medium);
    }

    // UT-GPU-008: 压力判定——High
    #[test]
    fn test_ut_gpu_008_pressure_high() {
        let p = GpuCollector::calc_pressure(81920, 75000);
        assert_eq!(p, GpuPressure::High);
    }

    // UT-GPU-009: 压力判定——边界值（刚好 50%）
    #[test]
    fn test_ut_gpu_009_pressure_boundary_50() {
        let p = GpuCollector::calc_pressure(100, 50);
        assert_eq!(p, GpuPressure::Medium);
    }

    // UT-GPU-010: 压力判定——边界值（刚好 20%）
    #[test]
    fn test_ut_gpu_010_pressure_boundary_20() {
        let p = GpuCollector::calc_pressure(100, 80);
        assert_eq!(p, GpuPressure::Medium);
    }

    // UT-GPU-011: 编译控制——不含 feature gpu 时 gpu 模块仍可编译
    #[test]
    fn test_ut_gpu_011_compile_without_feature() {
        let _collector = GpuCollector;
        let csv = "\
name, memory.total, memory.used, temperature.gpu, power.draw, utilization.gpu, utilization.memory
NVIDIA RTX 4090, 24576, 8192, 70, 320.0, 95, 50
";
        let result = GpuCollector::parse_nvidia_smi_output(csv);
        assert!(result.is_ok());
        let metrics = result.unwrap();
        assert_eq!(metrics[0].vram_total_mb, 24576);
        assert_eq!(metrics[0].temp_celsius, Some(70.0));
    }

    // UT-GPU-012: parse_csv_line 处理空字段
    #[test]
    fn test_ut_gpu_012_parse_empty_field() {
        let fields = parse_csv_line("a,,c").unwrap();
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0], "a");
        assert_eq!(fields[1], "");
        assert_eq!(fields[2], "c");
    }

    // UT-GPU-013: parse_csv_line 处理转义引号
    #[test]
    fn test_ut_gpu_013_parse_escaped_quote() {
        let fields = parse_csv_line(r#""he said ""hello""", value"#).unwrap();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0], r#"he said "hello""#);
        assert_eq!(fields[1], "value");
    }

    // UT-GPU-014: 无 GPU 时 collect 返回 ResourceNotAvailable（不 panic）
    #[test]
    fn test_ut_gpu_014_collect_no_gpu() {
        let collector = GpuCollector;
        let result = collector.collect();
        match result {
            Err(CollectError::ResourceNotAvailable(msg)) => {
                assert!(msg.contains("GPU"), "message should mention GPU");
            }
            Ok(CollectorOutput::Gpu(metrics)) => {
                assert!(!metrics.is_empty(), "should have at least one GPU");
            }
            other => panic!("unexpected result: {:?}", other),
        }
    }

    // === V0.3 新增测试 ===

    // UT-GPU-015: 温度字段为 [N/A] 时解析为 None
    #[test]
    fn test_ut_gpu_015_temp_na() {
        let csv = "\
name, memory.total, memory.used, temperature.gpu
NVIDIA RTX 3060, 12288, 4096, [N/A]
";
        let result = GpuCollector::parse_nvidia_smi_output(csv);
        assert!(result.is_ok());
        let metrics = result.unwrap();
        assert_eq!(metrics[0].temp_celsius, None);
        assert!(!metrics[0].throttle_warning);
    }

    // UT-GPU-016: 温度 > 90°C 触发节流警告
    #[test]
    fn test_ut_gpu_016_throttle_warning() {
        let csv = "\
name, memory.total, memory.used, temperature.gpu
NVIDIA RTX 4090, 24576, 8192, 95
";
        let result = GpuCollector::parse_nvidia_smi_output(csv);
        assert!(result.is_ok());
        let metrics = result.unwrap();
        assert!(metrics[0].throttle_warning, "95°C 应触发节流警告");
    }

    // UT-GPU-017: 功耗字段为 0 时正常解析
    #[test]
    fn test_ut_gpu_017_power_zero() {
        let csv = "\
name, memory.total, memory.used, power.draw
NVIDIA RTX 3060, 12288, 1024, 0
";
        let result = GpuCollector::parse_nvidia_smi_output(csv);
        assert!(result.is_ok());
        let metrics = result.unwrap();
        assert_eq!(metrics[0].power_watts, Some(0.0));
    }

    // UT-GPU-018: 缺少所有可选列时仅基础字段有效
    #[test]
    fn test_ut_gpu_018_minimal_fields() {
        let csv = "\
name, memory.total, memory.used
NVIDIA RTX 3060, 12288, 4096
";
        let result = GpuCollector::parse_nvidia_smi_output(csv);
        assert!(result.is_ok());
        let metrics = result.unwrap();
        assert_eq!(metrics[0].name, "NVIDIA RTX 3060");
        assert_eq!(metrics[0].temp_celsius, None);
        assert_eq!(metrics[0].power_watts, None);
        assert_eq!(metrics[0].utilization_gpu_percent, None);
        assert_eq!(metrics[0].utilization_memory_percent, None);
        assert!(!metrics[0].throttle_warning);
    }

    // UT-GPU-019: 温度压力判定函数
    #[test]
    fn test_ut_gpu_019_temp_pressure() {
        assert_eq!(GpuCollector::calc_temperature_pressure(40.0), "normal");
        assert_eq!(GpuCollector::calc_temperature_pressure(85.0), "normal");
        assert_eq!(GpuCollector::calc_temperature_pressure(85.1), "warning");
        assert_eq!(GpuCollector::calc_temperature_pressure(90.0), "warning");
        assert_eq!(GpuCollector::calc_temperature_pressure(95.0), "warning");
        assert_eq!(GpuCollector::calc_temperature_pressure(95.1), "critical");
        assert_eq!(GpuCollector::calc_temperature_pressure(100.0), "critical");
    }
}
