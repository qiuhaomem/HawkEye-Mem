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
// src/collector/gpu.rs — GPU Collector 重导出层（V0.3 Phase 4）
//
// 自 V0.3 起，GPU 采集逻辑迁移至 src/gpu/ 模块。
// 此文件保留向后兼容的重导出和旧测试用例。
// ============================================================================

// 重导出新模块中的类型和函数
pub use crate::gpu::GpuCollector;

// ============================================================================
// 单元测试（从 V0.2 保留的兼容测试）
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collector::{CollectError, CollectorOutput, GpuPressure, ResourceCollector};
    use crate::gpu::parse_csv_line;

    // UT-GPU-001: 解析 nvidia-smi 单 GPU 输出
    #[test]
    fn test_ut_gpu_001_parse_single_gpu() {
        let csv = "\
name, memory.total, memory.used
NVIDIA GeForce RTX 3090, 24576, 10240
";
        let result = GpuCollector::parse_nvidia_smi_output(csv);
        assert!(
            result.is_ok(),
            "should parse single GPU: {:?}",
            result.err()
        );
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
        assert!(
            result.is_ok(),
            "should parse quoted name: {:?}",
            result.err()
        );
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
}
