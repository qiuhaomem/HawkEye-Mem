// Copyright 2026 秋毫mem Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// ============================================================================
// src/gpu/nvml.rs — NVIDIA NVML 直接绑定（增强版，V0.3 T15）
//
// 使用 nvml-wrapper crate 直接调用 NVML API，替代 nvidia-smi 解析。
// 采集字段：name, vram_total_mb, vram_used_mb, temperature_c, power_watts,
//           utilization_gpu_percent, utilization_memory_percent
//
// 安全性：
// - 所有 unsafe FFI 集中在单一模块（CR-01）
// - 每设备独立 try/catch，单个 GPU 传感器失败不影响其他
// - NVML 初始化失败时自动降级到 nvidia-smi 解析
// ============================================================================

#![cfg(feature = "gpu")]

use crate::collector::{GpuMetrics, GpuPressure};
use nvml_wrapper::{Device, Nvml};

/// NVML 全量采集入口
///
/// 初始化 NVML，遍历所有 GPU 设备，采集 VRAM + 温度 + 功耗 + 利用率。
/// 每个传感器的读取独立包裹在 try 中，失败则对应字段置 None。
pub fn collect_all() -> Result<Vec<GpuMetrics>, String> {
    let nvml = Nvml::init().map_err(|e| format!("NVML init failed: {}", e))?;

    let device_count = nvml
        .device_count()
        .map_err(|e| format!("Failed to get device count: {}", e))?;

    let mut metrics = Vec::with_capacity(device_count as usize);
    for i in 0..device_count {
        let device = nvml
            .device_by_index(i)
            .map_err(|e| format!("Failed to get device {}: {}", i, e))?;

        let gpu_metrics = collect_single(&device, i);
        metrics.push(gpu_metrics);
    }

    Ok(metrics)
}

/// 采集单个 GPU 设备的全部指标
///
/// 每个字段独立 try，失败时静默降级为 None。
fn collect_single(device: &Device, index: u32) -> GpuMetrics {
    // 设备名称
    let name = device
        .name()
        .unwrap_or_else(|_| format!("NVIDIA GPU {}", index));

    // VRAM 信息（bytes → MB）
    let (total_mb, used_mb) = device
        .memory_info()
        .map(|mem| {
            let total = mem.total / (1024 * 1024);
            let used = mem.used / (1024 * 1024);
            (total, used)
        })
        .unwrap_or((0, 0));

    // 压力判定
    let pressure = calc_pressure(total_mb, used_mb);

    // 温度（°C）—— 独立降级
    let temp_celsius = device
        .temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu)
        .map(|t| t as f64)
        .ok();

    // 功耗（W）—— 独立降级，mW → W
    let power_watts = device.power_usage().map(|p| p as f64 / 1000.0).ok();

    // GPU 利用率（%）—— 独立降级
    let (util_gpu, util_mem) = device
        .utilization_rates()
        .map(|u| (Some(u.gpu), Some(u.memory)))
        .unwrap_or((None, None));

    // 温度节流警告
    let throttle_warning = temp_celsius.map_or(false, |t| t > 90.0);

    GpuMetrics {
        name,
        vram_total_mb: total_mb,
        vram_used_mb: used_mb,
        pressure,
        temp_celsius,
        power_watts,
        utilization_gpu_percent: util_gpu,
        utilization_memory_percent: util_mem,
        throttle_warning,
        backend: "nvml".to_string(),
    }
}

/// NVML 路径的压力判定（与 nvidia-smi 路径共享逻辑）
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

#[cfg(test)]
mod tests {
    use super::*;

    // UT-NVML-001: 压力判定——Low
    #[test]
    fn test_ut_nvml_001_pressure_low() {
        let p = calc_pressure(24576, 4096);
        assert_eq!(p, GpuPressure::Low);
    }

    // UT-NVML-002: 压力判定——High
    #[test]
    fn test_ut_nvml_002_pressure_high() {
        let p = calc_pressure(24576, 22000);
        assert_eq!(p, GpuPressure::High);
    }

    // UT-NVML-003: 零显存边界
    #[test]
    fn test_ut_nvml_003_zero_memory() {
        let p = calc_pressure(0, 0);
        assert_eq!(p, GpuPressure::High);
    }
}
