// ============================================================================
// GpuCollector — 实验性 GPU 显存采集器（V0.2 W3）
//
// 双路径实现：
//   路径A（feature-gated NVML）： 当 --features gpu 时，尝试用 nvml-wrapper
//   路径B（universal nvidia-smi）：回退到解析 nvidia-smi CSV 输出
//
// NVML 路径涉及底层 FFI 调用（nvml-wrapper 内部使用 unsafe），
// 因此 feature="gpu" 时模块级别允许 unsafe_code。
// nvidia-smi 路径完全 safe Rust。
// ============================================================================

#[cfg_attr(feature = "gpu", allow(unsafe_code))]
// allow(unsafe_code)：NVML 绑定（nvml-wrapper）底层依赖 unsafe FFI，
// 该模块是 unsafe 的唯一汇聚点。nvidia-smi 解析路径全为 safe Rust。

use super::{CollectError, CollectorOutput, GpuMetrics, GpuPressure, ResourceCollector};

/// GPU 显存采集器
///
/// 无状态，可全局共享。检测 NVIDIA GPU 显存使用情况。
pub struct GpuCollector;

impl GpuCollector {
    // ========================================================================
    // 路径A：NVML（需要 feature = "gpu"）
    // ========================================================================

    /// 通过 NVML 读取 GPU 显存信息
    #[cfg(feature = "gpu")]
    fn collect_nvml() -> Result<Vec<GpuMetrics>, String> {
        let nvml = nvml_wrapper::Nvml::init()
            .map_err(|e| format!("NVML init failed: {}", e))?;

        let device_count = nvml.device_count()
            .map_err(|e| format!("Failed to get device count: {}", e))?;

        let mut metrics = Vec::with_capacity(device_count as usize);
        for i in 0..device_count {
            let device = nvml.device_by_index(i)
                .map_err(|e| format!("Failed to get device {}: {}", i, e))?;
            let name = device.name()
                .map_err(|e| format!("Failed to get device name: {}", e))?;
            let mem_info = device.memory_info()
                .map_err(|e| format!("Failed to get memory info: {}", e))?;

            // nvml 返回的内存单位为 bytes，转为 MB
            let total_mb = mem_info.total / (1024 * 1024);
            let used_mb = mem_info.used / (1024 * 1024);

            let pressure = Self::calc_pressure(total_mb, used_mb);

            metrics.push(GpuMetrics {
                name,
                vram_total_mb: total_mb,
                vram_used_mb: used_mb,
                pressure,
            });
        }

        Ok(metrics)
    }

    // ========================================================================
    // 路径B：nvidia-smi 命令解析
    // ========================================================================

    /// 通过执行 `nvidia-smi` 并解析 CSV 输出来获取 GPU 显存信息
    fn collect_nvidia_smi() -> Result<Vec<GpuMetrics>, String> {
        let output = std::process::Command::new("nvidia-smi")
            .args([
                "--query-gpu=name,memory.total,memory.used",
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
    fn parse_nvidia_smi_output(output: &str) -> Result<Vec<GpuMetrics>, String> {
        let mut lines = output.lines();

        // 第一行是 CSV header，解析列索引
        let header = lines.next().ok_or_else(|| "Empty nvidia-smi output".to_string())?;
        let headers = parse_csv_line(header)?;

        let name_idx = headers
            .iter()
            .position(|h| h.trim().contains("name"))
            .ok_or_else(|| "Missing 'name' column".to_string())?;
        let total_idx = headers
            .iter()
            .position(|h| h.trim().contains("memory.total"))
            .ok_or_else(|| "Missing 'memory.total' column".to_string())?;
        let used_idx = headers
            .iter()
            .position(|h| h.trim().contains("memory.used"))
            .ok_or_else(|| "Missing 'memory.used' column".to_string())?;

        let max_idx = name_idx.max(total_idx).max(used_idx);
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

            metrics.push(GpuMetrics {
                name: gpu_name,
                vram_total_mb: total_mb,
                vram_used_mb: used_mb,
                pressure,
            });
        }

        if metrics.is_empty() {
            return Err("No GPU data found".to_string());
        }

        Ok(metrics)
    }

    // ========================================================================
    // 压力判定
    // ========================================================================

    /// 根据可用显存百分比判定压力等级
    /// available > 50% → Low, 20-50% → Medium, < 20% → High
    fn calc_pressure(total_mb: u64, used_mb: u64) -> GpuPressure {
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
}

impl ResourceCollector for GpuCollector {
    /// 采集 GPU 显存指标
    ///
    /// 流程：
    /// 1. 如果启用 gpu feature，先尝试 NVML 路径
    /// 2. NVML 失败或未启用 feature，回退到 nvidia-smi 解析
    /// 3. 都失败则返回 ResourceNotAvailable（不 panic）
    fn collect(&self) -> Result<CollectorOutput, CollectError> {
        // 路径A：NVML（需要 feature gpu）
        #[cfg(feature = "gpu")]
        if let Ok(metrics) = Self::collect_nvml() {
            return Ok(CollectorOutput::Gpu(metrics));
        }

        // 路径B：nvidia-smi（universal 回退）
        // CR-07 脱敏：失败时只输出通用提示，不包含路径/版本/驱动信息
        Self::collect_nvidia_smi()
            .map(CollectorOutput::Gpu)
            .map_err(|_| CollectError::ResourceNotAvailable("No NVIDIA GPU detected".into()))
    }
}

// ============================================================================
// CSV 解析工具（CR-06：支持引号包裹字段，不硬编码列索引）
// ============================================================================

/// 解析单行 CSV，支持双引号包裹的字段（含逗号）
///
/// nvidia-smi 输出中 GPU 名称可能包含逗号（如 "NVIDIA A100, 80GB SXM"），
/// 此时名称字段会被双引号包裹。此解析器正确处理这种场景。
fn parse_csv_line(line: &str) -> Result<Vec<String>, String> {
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

    // UT-GPU-001: 解析 nvidia-smi 单 GPU 输出
    #[test]
    fn test_ut_gpu_001_parse_single_gpu() {
        let csv = "\
name, memory.total, memory.used
NVIDIA GeForce RTX 3090, 24576, 10240
";
        let result = GpuCollector::parse_nvidia_smi_output(csv);
        assert!(result.is_ok(), "should parse single GPU: {:?}", result.err());
        let metrics = result.unwrap();
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].name, "NVIDIA GeForce RTX 3090");
        assert_eq!(metrics[0].vram_total_mb, 24576);
        assert_eq!(metrics[0].vram_used_mb, 10240);
    }

    // UT-GPU-002: 解析多 GPU
    #[test]
    fn test_ut_gpu_002_parse_multi_gpu() {
        let csv = "\
name, memory.total, memory.used
NVIDIA A100, 81920, 40960
NVIDIA A100, 81920, 20480
";
        let result = GpuCollector::parse_nvidia_smi_output(csv);
        assert!(result.is_ok(), "should parse multi GPU");
        let metrics = result.unwrap();
        assert_eq!(metrics.len(), 2);
        assert_eq!(metrics[1].vram_used_mb, 20480);
    }

    // UT-GPU-003: 解析含逗号（引号包裹）的 GPU 名称
    #[test]
    fn test_ut_gpu_003_parse_quoted_name() {
        let csv = "\
name, memory.total, memory.used
\"NVIDIA A100, 80GB SXM\", 81920, 40960
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
memory.total, name, memory.used
24576, NVIDIA RTX 3090, 10240
";
        let result = GpuCollector::parse_nvidia_smi_output(csv);
        assert!(result.is_ok(), "should handle reordered columns");
        let metrics = result.unwrap();
        assert_eq!(metrics[0].name, "NVIDIA RTX 3090");
        assert_eq!(metrics[0].vram_total_mb, 24576);
        assert_eq!(metrics[0].vram_used_mb, 10240);
    }

    // UT-GPU-006: 压力判定——Low
    #[test]
    fn test_ut_gpu_006_pressure_low() {
        // 可用 > 50% → Low（81920 总量中只用 10000，可用 88%）
        let p = GpuCollector::calc_pressure(81920, 10000);
        assert_eq!(p, GpuPressure::Low);
    }

    // UT-GPU-007: 压力判定——Medium
    #[test]
    fn test_ut_gpu_007_pressure_medium() {
        // 可用 20-50% → Medium（81920 总量中用了 60000，可用 27%）
        let p = GpuCollector::calc_pressure(81920, 60000);
        assert_eq!(p, GpuPressure::Medium);
    }

    // UT-GPU-008: 压力判定——High
    #[test]
    fn test_ut_gpu_008_pressure_high() {
        // 可用 < 20% → High（81920 总量中用了 75000，可用 8%）
        let p = GpuCollector::calc_pressure(81920, 75000);
        assert_eq!(p, GpuPressure::High);
    }

    // UT-GPU-009: 压力判定——边界值（刚好 50%）
    #[test]
    fn test_ut_gpu_009_pressure_boundary_50() {
        // 可用刚好 50% → Medium（< 50%，<= 50 不算 > 50）
        let p = GpuCollector::calc_pressure(100, 50);
        assert_eq!(p, GpuPressure::Medium);
    }

    // UT-GPU-010: 压力判定——边界值（刚好 20%）
    #[test]
    fn test_ut_gpu_010_pressure_boundary_20() {
        // 可用刚好 20% → Medium（>= 20% 属于 Medium）
        let p = GpuCollector::calc_pressure(100, 80);
        assert_eq!(p, GpuPressure::Medium);
    }

    // UT-GPU-011: 编译控制——不含 feature gpu 时 gpu 模块仍可编译
    // 本测试本身就在无 feature 环境下编译运行（default features = []），
    // 验证 GpuCollector 的 nvidia-smi 路径可以独立工作
    #[test]
    fn test_ut_gpu_011_compile_without_feature() {
        // 验证 GpuCollector 类型存在（编译即可），无需 feature gpu
        let _collector = GpuCollector;
        // 验证 nvidia-smi 解析函数可直接调用（无 feature 依赖）
        let csv = "\
name, memory.total, memory.used
NVIDIA RTX 4090, 24576, 8192
";
        let result = GpuCollector::parse_nvidia_smi_output(csv);
        assert!(result.is_ok());
        let metrics = result.unwrap();
        assert_eq!(metrics[0].vram_total_mb, 24576);
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
    // 此测试在 CI 和无 GPU 机器上运行，验证错误路径安全
    #[test]
    fn test_ut_gpu_014_collect_no_gpu() {
        let collector = GpuCollector;
        let result = collector.collect();
        match result {
            Err(CollectError::ResourceNotAvailable(msg)) => {
                // 错误信息脱敏（CR-07）：不包含路径、版本号、驱动信息
                assert!(msg.contains("GPU"), "message should mention GPU");
                assert!(!msg.contains('/'), "message should not contain paths");
            }
            // 如果真有 GPU，collect 可能成功也算正确
            Ok(CollectorOutput::Gpu(metrics)) => {
                assert!(!metrics.is_empty(), "should have at least one GPU");
            }
            other => panic!("unexpected result: {:?}", other),
        }
    }
}
