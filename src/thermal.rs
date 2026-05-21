// ============================================================================
// src/thermal.rs — ThermalCollector（V0.3 Phase 5）
//
// CPU/GPU 温度采集模块。
//   采集源：
//     Linux:   /sys/class/thermal/thermal_zone*/temp
//     macOS:   pmset -g thermlog / SMC（Apple Silicon 暂返回 None）
//   不暴露系统温度传感器路径——只采集聚合后的温度值。
//
// W4 裁剪评估（CR-05）：温度只采集不预警，输出说明文案
//   "Temperature data is for reference only; automatic alerts will
//    be provided in a future release."
// ============================================================================

use crate::collector::{
    CollectError, CollectorOutput, CpuThermalPressure, ResourceCollector, ThermalMetrics,
};

/// 温度采集器
pub struct ThermalCollector;

impl ThermalCollector {
    /// Linux: 读取 /sys/class/thermal/thermal_zone*/temp
    /// 取所有 thermal zone 的最高温度作为 CPU 温度
    #[cfg(target_os = "linux")]
    fn cpu_temp_linux() -> Option<f64> {
        let zones = std::fs::read_dir("/sys/class/thermal/").ok()?;
        zones
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                if !path.file_name()?.to_str()?.starts_with("thermal_zone") {
                    return None;
                }
                let temp_str = std::fs::read_to_string(path.join("temp")).ok()?;
                let temp: f64 = temp_str.trim().parse().ok()?;
                Some(temp / 1000.0) // millidegree → degree
            })
            .max_by(|a, b| a.partial_cmp(b).unwrap())
    }

    /// macOS: 通过 pmset -g thermlog 获取
    /// 若不可用则静默返回 None
    #[cfg(target_os = "macos")]
    fn cpu_temp_macos() -> Option<f64> {
        let output = std::process::Command::new("pmset")
            .args(["-g", "thermlog"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        // pmset 输出格式因版本而异，暂不精确解析
        None
    }

    /// 根据温度值判定压力等级
    /// < 80°C → Normal, 80~95°C → Warning, > 95°C → Critical
    fn calc_pressure(temp: Option<f64>) -> CpuThermalPressure {
        match temp {
            Some(t) if t >= 95.0 => CpuThermalPressure::Critical,
            Some(t) if t >= 80.0 => CpuThermalPressure::Warning,
            _ => CpuThermalPressure::Normal,
        }
    }
}

impl ResourceCollector for ThermalCollector {
    fn collect(&self) -> Result<CollectorOutput, CollectError> {
        let cpu_temp_c = {
            #[cfg(target_os = "linux")]
            {
                Self::cpu_temp_linux()
            }
            #[cfg(target_os = "macos")]
            {
                Self::cpu_temp_macos()
            }
            #[cfg(not(any(target_os = "linux", target_os = "macos")))]
            {
                None
            }
        };

        let pressure = Self::calc_pressure(cpu_temp_c);

        let metrics = ThermalMetrics {
            cpu_temp_c,
            gpu_temps_c: Vec::new(),
            pressure,
            note: "Temperature data is for reference only; automatic alerts will be provided in a future release (V0.4).".to_string(),
        };

        Ok(CollectorOutput::Thermal(metrics))
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // UT-TH-003: 温度路径不存在（在 CI/容器中 cpu_temp_c 应为 None）
    #[test]
    fn test_ut_th_003_no_thermal_path() {
        let collector = ThermalCollector;
        let result = collector.collect();
        assert!(result.is_ok(), "温度采集不因不可用而失败: {:?}", result.err());
    }

    // UT-TH-004: 温度压力判定 — Normal
    #[test]
    fn test_ut_th_004_pressure_normal() {
        assert_eq!(ThermalCollector::calc_pressure(Some(65.0)), CpuThermalPressure::Normal);
        assert_eq!(ThermalCollector::calc_pressure(Some(79.9)), CpuThermalPressure::Normal);
        assert_eq!(ThermalCollector::calc_pressure(None), CpuThermalPressure::Normal);
    }

    // UT-TH-005: 温度压力判定 — Warning
    #[test]
    fn test_ut_th_005_pressure_warning() {
        assert_eq!(ThermalCollector::calc_pressure(Some(80.0)), CpuThermalPressure::Warning);
        assert_eq!(ThermalCollector::calc_pressure(Some(88.0)), CpuThermalPressure::Warning);
        assert_eq!(ThermalCollector::calc_pressure(Some(94.9)), CpuThermalPressure::Warning);
    }

    // UT-TH-006: 温度压力判定 — Critical
    #[test]
    fn test_ut_th_006_pressure_critical() {
        assert_eq!(ThermalCollector::calc_pressure(Some(95.0)), CpuThermalPressure::Critical);
        assert_eq!(ThermalCollector::calc_pressure(Some(100.0)), CpuThermalPressure::Critical);
    }

    // UT-TH-007: 温度 note 包含说明文案（CR-05）
    #[test]
    fn test_ut_th_007_w4_note() {
        let collector = ThermalCollector;
        let result = collector.collect().unwrap();
        if let CollectorOutput::Thermal(metrics) = result {
            assert!(
                metrics.note.contains("reference only"),
                "说明文案应包含 'reference only': {}",
                metrics.note
            );
        } else {
            panic!("应返回 Thermal 变体");
        }
    }

    // UT-TH-011: 温度超过 100°C 不溢出
    #[test]
    fn test_ut_th_011_over_100_pressure() {
        let pressure = ThermalCollector::calc_pressure(Some(110.0));
        assert_eq!(pressure, CpuThermalPressure::Critical, "110°C 应为 Critical");
    }

    // UT-TH-012: 温度值为 0 不报错
    #[test]
    fn test_ut_th_012_zero_temp() {
        let pressure = ThermalCollector::calc_pressure(Some(0.0));
        assert_eq!(pressure, CpuThermalPressure::Normal, "0°C 应为 Normal");
    }
}
