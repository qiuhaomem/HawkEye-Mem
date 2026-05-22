// ============================================================================
// src/gpu/amd.rs — AMD ROCm Collector（V0.3 T16）
//
// 通过 rocm-smi CLI 解析 AMD GPU 显存、温度和利用率。
// rocm-smi 不在 PATH 时静默跳过（不报错）。
//
// 采集字段：name, vram_total_mb, vram_used_mb, temperature_c,
//           utilization_gpu_percent
//
// 约束：
// - 纯 safe Rust 解析
// - 标记"需社区反馈"（CR-10 PMO 要求）
// - 编译条件：feature = "gpu"
// ============================================================================

use crate::collector::{GpuMetrics, GpuPressure};

/// 解析 rocm-smi CSV 输出
///
/// rocm-smi --showmeminfo vram --showtemp --showuse --csv 的输出格式：
/// ```csv
/// device,VRAM Total (B),VRAM Used (B),Temperature (Sensor edge) (C),GPU use (%)
/// card0,17163091968,8581545984,45.0,85
/// ```
///
/// 注：rocm-smi 各版本输出格式可能不同，此解析器采用宽松匹配策略。
#[allow(dead_code)]
pub fn parse_rocm_smi_output(output: &str) -> Result<Vec<GpuMetrics>, String> {
    let mut lines = output.lines();

    // 跳过 header 行（查找包含 "device" 的行作为表头）
    let header_line = loop {
        match lines.next() {
            Some(line) if line.trim().is_empty() => continue,
            Some(line) if line.to_lowercase().contains("device") => break line,
            Some(_) => continue,
            None => return Err("Empty rocm-smi output".to_string()),
        }
    };

    let headers = super::parse_csv_line(header_line)?;

    // 解析各列索引（不区分大小写模糊匹配）
    let name_idx = headers
        .iter()
        .position(|h| h.trim().to_lowercase().contains("device"))
        .ok_or_else(|| "rocm-smi output missing 'device' column".to_string())?;

    let total_idx = headers
        .iter()
        .position(|h| {
            let h = h.trim().to_lowercase();
            h.contains("vram total") || h.contains("total")
        })
        .ok_or_else(|| "rocm-smi output missing VRAM total column".to_string())?;

    let used_idx = headers
        .iter()
        .position(|h| {
            let h = h.trim().to_lowercase();
            h.contains("vram used") || h.contains("used")
        })
        .ok_or_else(|| "rocm-smi output missing VRAM used column".to_string())?;

    // 可选列
    let temp_idx = headers
        .iter()
        .position(|h| h.trim().to_lowercase().contains("temp"));

    let gpu_use_idx = headers.iter().position(|h| {
        let h = h.trim().to_lowercase();
        h.contains("gpu use") || h.contains("utilization")
    });

    let max_idx = [name_idx, total_idx, used_idx]
        .iter()
        .chain(temp_idx.iter())
        .chain(gpu_use_idx.iter())
        .max()
        .copied()
        .unwrap_or(0);

    let mut metrics = Vec::new();

    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let cols = super::parse_csv_line(line)?;
        if cols.len() <= max_idx {
            continue;
        }

        let gpu_name = cols[name_idx].trim().to_string();

        // VRAM: rocm-smi 返回字节数，需转为 MB
        // 部分版本可能直接返回 MB，做自适应检测
        let total_raw: u64 = cols[total_idx]
            .trim()
            .replace(',', "")
            .parse()
            .map_err(|_| format!("Failed to parse VRAM total: {}", cols[total_idx].trim()))?;
        let used_raw: u64 = cols[used_idx]
            .trim()
            .replace(',', "")
            .parse()
            .map_err(|_| format!("Failed to parse VRAM used: {}", cols[used_idx].trim()))?;

        // 自适应单位：> 1TB 的字节值 → 肯定是 bytes；< 1TB → 可能已经是 MB
        let (total_mb, used_mb) = if total_raw > 1_000_000_000 {
            (total_raw / (1024 * 1024), used_raw / (1024 * 1024))
        } else {
            (total_raw, used_raw)
        };

        // 压力判定
        let pressure = if total_mb > 0 {
            let available_pct = (total_mb.saturating_sub(used_mb)) as f64 / total_mb as f64 * 100.0;
            if available_pct > 50.0 {
                GpuPressure::Low
            } else if available_pct >= 20.0 {
                GpuPressure::Medium
            } else {
                GpuPressure::High
            }
        } else {
            GpuPressure::High
        };

        // 温度（°C）
        let temp_celsius = temp_idx.and_then(|idx| {
            let v = cols.get(idx)?.trim();
            if v.is_empty() || v.eq_ignore_ascii_case("N/A") {
                return None;
            }
            v.parse::<f64>().ok()
        });

        // GPU 利用率（%）
        let utilization_gpu_percent = gpu_use_idx.and_then(|idx| {
            let v = cols.get(idx)?.trim();
            if v.is_empty() || v.eq_ignore_ascii_case("N/A") {
                return None;
            }
            v.parse::<u32>().ok()
        });

        // 温度节流警告（AMD GPU 通常在 95°C+ 开始节流）
        let throttle_warning = temp_celsius.is_some_and(|t| t > 90.0);

        metrics.push(GpuMetrics {
            name: gpu_name,
            vram_total_mb: total_mb,
            vram_used_mb: used_mb,
            pressure,
            temp_celsius,
            power_watts: None, // rocm-smi 不直接提供功耗
            utilization_gpu_percent,
            utilization_memory_percent: None, // rocm-smi 不直接提供显存利用率
            throttle_warning,
            backend: "rocm-smi".to_string(),
        });
    }

    if metrics.is_empty() {
        return Err("No AMD GPU data found in rocm-smi output".to_string());
    }

    Ok(metrics)
}

#[cfg(test)]
mod tests {
    use super::*;

    // UT-AMD-001: 解析 rocm-smi 单 GPU 输出（bytes 格式）
    #[test]
    fn test_ut_amd_001_parse_single_gpu_bytes() {
        let csv = "\
device,VRAM Total (B),VRAM Used (B),Temperature (Sensor edge) (C),GPU use (%)
card0,17163091968,8581545984,45.0,85
";
        let result = parse_rocm_smi_output(csv);
        assert!(
            result.is_ok(),
            "should parse single AMD GPU: {:?}",
            result.err()
        );
        let metrics = result.unwrap();
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].name, "card0");
        // 17163091968 B / (1024*1024) ≈ 16384 MB
        assert!(
            (metrics[0].vram_total_mb as i64 - 16368).abs() <= 20,
            "total_mb={} expected ~16368",
            metrics[0].vram_total_mb
        );
        assert_eq!(metrics[0].temp_celsius, Some(45.0));
        assert_eq!(metrics[0].utilization_gpu_percent, Some(85));
        assert!(!metrics[0].throttle_warning);
        assert_eq!(metrics[0].backend, "rocm-smi");
    }

    // UT-AMD-002: 解析 rocm-smi 多 GPU 输出
    #[test]
    fn test_ut_amd_002_parse_multi_gpu() {
        let csv = "\
device,VRAM Total (B),VRAM Used (B),Temperature (Sensor edge) (C),GPU use (%)
card0,17163091968,8581545984,45.0,85
card1,8589934592,2147483648,38.0,30
";
        let result = parse_rocm_smi_output(csv);
        assert!(result.is_ok(), "should parse multi AMD GPU");
        let metrics = result.unwrap();
        assert_eq!(metrics.len(), 2);
        assert_eq!(metrics[1].name, "card1");
        assert_eq!(metrics[1].temp_celsius, Some(38.0));
        // card1: 8GB ≈ 8192MB
        assert!(
            (metrics[1].vram_total_mb as i64 - 8192).abs() <= 10,
            "card1 total_mb={} expected ~8192",
            metrics[1].vram_total_mb
        );
    }

    // UT-AMD-003: 空输出返回错误
    #[test]
    fn test_ut_amd_003_empty_output() {
        let csv = "device,VRAM Total (B),VRAM Used (B)\n";
        let result = parse_rocm_smi_output(csv);
        assert!(result.is_err(), "empty data should error");
    }

    // UT-AMD-004: 温度字段不可用时为 None
    #[test]
    fn test_ut_amd_004_temp_unavailable() {
        let csv = "\
device,VRAM Total (B),VRAM Used (B)
card0,17163091968,8581545984
";
        let result = parse_rocm_smi_output(csv);
        assert!(result.is_ok());
        let metrics = result.unwrap();
        assert_eq!(metrics[0].temp_celsius, None);
        assert_eq!(metrics[0].utilization_gpu_percent, None);
    }

    // UT-AMD-005: 温度 > 90°C 触发节流警告
    #[test]
    fn test_ut_amd_005_throttle_warning() {
        let csv = "\
device,VRAM Total (B),VRAM Used (B),Temperature (Sensor edge) (C),GPU use (%)
card0,17163091968,12884901888,95.0,100
";
        let result = parse_rocm_smi_output(csv);
        assert!(result.is_ok());
        let metrics = result.unwrap();
        assert!(metrics[0].throttle_warning, "95°C 应触发节流警告");
        assert_eq!(metrics[0].temp_celsius, Some(95.0));
    }

    // UT-AMD-006: 温度 N/A 视为 None
    #[test]
    fn test_ut_amd_006_temp_na() {
        let csv = "\
device,VRAM Total (B),VRAM Used (B),Temperature (Sensor edge) (C),GPU use (%)
card0,17163091968,8581545984,N/A,50
";
        let result = parse_rocm_smi_output(csv);
        assert!(result.is_ok());
        let metrics = result.unwrap();
        assert_eq!(metrics[0].temp_celsius, None);
    }

    // UT-AMD-007: VRAM 已使用 MB 格式的自适应
    #[test]
    fn test_ut_amd_007_vram_mb_format() {
        // 部分 rocm-smi 版本输出 MB 而非 bytes（< 1TB 的值）
        let csv = "\
device,VRAM Total (B),VRAM Used (B)
card0,16384,8192
";
        let result = parse_rocm_smi_output(csv);
        assert!(result.is_ok());
        let metrics = result.unwrap();
        // 16384 < 1e9，识别为 MB 格式，不做 bytes→MB 转换
        assert_eq!(metrics[0].vram_total_mb, 16384);
        assert_eq!(metrics[0].vram_used_mb, 8192);
    }
}
