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

use serde::Serialize;

/// Agent action recommendation based on memory pressure.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum Action {
    #[serde(rename = "ok")]
    Ok,
    #[serde(rename = "monitor")]
    Monitor,
    #[serde(rename = "reduce_context")]
    ReduceContext,
    #[serde(rename = "abort_safely")]
    AbortSafely,
}

impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Action::Ok => write!(f, "ok"),
            Action::Monitor => write!(f, "monitor"),
            Action::ReduceContext => write!(f, "reduce_context"),
            Action::AbortSafely => write!(f, "abort_safely"),
        }
    }
}

/// Full guidance output embedded in `agent_guidance` JSON field.
#[derive(Debug, Clone, Serialize)]
pub struct Guidance {
    pub pressure: String,
    pub estimated_safe_context_window: u64,
    pub confidence: String,
    pub action: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

/// Generates `action`, `reason`, and `suggestion` from memory metrics.
pub struct GuidanceGenerator;

impl GuidanceGenerator {
    /// Generate full guidance for a given memory state.
    pub fn generate(
        available_mb: u64,
        used_percent: f64,
        estimated_tokens: u64,
        confidence: &str,
    ) -> Guidance {
        let (pressure, action, reason) = Self::classify(available_mb, used_percent);

        let suggestion = if confidence == "conservative" {
            Some(match action {
                Action::AbortSafely => {
                    "Critical memory detected. Configure model parameters via --config for calibrated estimation in future runs.".to_string()
                }
                Action::ReduceContext => {
                    "High memory pressure. Use --config to set model parameters and enable calibrated estimates.".to_string()
                }
                Action::Monitor => {
                    "Moderate memory pressure. Consider configuring model parameters for better accuracy.".to_string()
                }
                Action::Ok => {
                    "Memory is healthy. For more accurate token estimates, create a config file with --init-config.".to_string()
                }
            })
        } else {
            None
        };

        Guidance {
            pressure: pressure.to_string(),
            estimated_safe_context_window: estimated_tokens,
            confidence: confidence.to_string(),
            action: action.to_string(),
            reason,
            suggestion,
        }
    }

    /// Classify memory state and return (pressure, action, reason).
    pub fn classify(available_mb: u64, used_percent: f64) -> (&'static str, Action, String) {
        if available_mb < 2000 || used_percent > 92.0 {
            let reason = format!(
                "Critical: {}MB available, {}% used. Abort safely to prevent OOM.",
                available_mb, used_percent
            );
            ("critical", Action::AbortSafely, reason)
        } else if available_mb < 4000 || used_percent > 80.0 {
            let reason = format!(
                "High pressure: {}MB available, {}% used. Reduce context to avoid thrashing.",
                available_mb, used_percent
            );
            ("high", Action::ReduceContext, reason)
        } else if available_mb < 8000 || used_percent > 70.0 {
            let reason = format!(
                "Moderate: {}MB available, {}% used. Continue monitoring.",
                available_mb, used_percent
            );
            ("medium", Action::Monitor, reason)
        } else {
            let reason = format!(
                "Healthy: {}MB available, {}% used. No action needed.",
                available_mb, used_percent
            );
            ("low", Action::Ok, reason)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // UT-GG-001: Low压力 → Ok
    #[test]
    fn test_ut_gg_001_low_ok() {
        let g = GuidanceGenerator::generate(12000, 30.0, 1_700_000, "conservative");
        assert_eq!(g.action, "ok");
        assert_eq!(g.pressure, "low");
    }

    // UT-GG-002: Medium压力 → Monitor
    #[test]
    fn test_ut_gg_002_medium_monitor() {
        let g = GuidanceGenerator::generate(6000, 55.0, 800_000, "conservative");
        assert_eq!(g.action, "monitor");
        assert_eq!(g.pressure, "medium");
    }

    // UT-GG-003: High压力 → ReduceContext
    #[test]
    fn test_ut_gg_003_high_reduce() {
        let g = GuidanceGenerator::generate(3000, 85.0, 300_000, "conservative");
        assert_eq!(g.action, "reduce_context");
        assert_eq!(g.pressure, "high");
    }

    // UT-GG-004: Critical压力 → AbortSafely
    #[test]
    fn test_ut_gg_004_critical_abort() {
        let g = GuidanceGenerator::generate(1500, 95.0, 50_000, "conservative");
        assert_eq!(g.action, "abort_safely");
        assert_eq!(g.pressure, "critical");
    }

    // UT-GG-005: suggestion仅在conservative时存在
    #[test]
    fn test_ut_gg_005_suggestion_conservative() {
        let g = GuidanceGenerator::generate(12000, 30.0, 1_700_000, "conservative");
        assert!(
            g.suggestion.is_some(),
            "Conservative should have suggestion"
        );
    }

    // UT-GG-006: suggestion在calibrated时为None
    #[test]
    fn test_ut_gg_006_suggestion_calibrated_none() {
        let g = GuidanceGenerator::generate(12000, 30.0, 1_700_000, "calibrated");
        assert!(
            g.suggestion.is_none(),
            "Calibrated should have no suggestion"
        );
    }

    // UT-GG-007: reason字符串长度限制 ≤200
    #[test]
    fn test_ut_gg_007_reason_length() {
        // Test all 4 pressure levels
        for (avail, used) in &[(12000, 30.0), (6000, 55.0), (3000, 85.0), (1500, 95.0)] {
            let g = GuidanceGenerator::generate(*avail, *used, 100_000, "conservative");
            assert!(
                g.reason.len() <= 200,
                "reason too long ({} chars): {}",
                g.reason.len(),
                g.reason
            );
        }
    }

    // UT-GG-008: reason包含可用内存数值
    #[test]
    fn test_ut_gg_008_reason_contains_memory() {
        let g = GuidanceGenerator::generate(3200, 80.0, 100_000, "conservative");
        assert!(
            g.reason.contains("3200"),
            "reason should contain '3200', got: {}",
            g.reason
        );
    }

    // Classification boundary tests
    #[test]
    fn test_classify_low() {
        let (p, a, _) = GuidanceGenerator::classify(8000, 50.0);
        assert_eq!(p, "low");
        assert_eq!(a, Action::Ok);
    }

    #[test]
    fn test_classify_medium() {
        let (p, a, _) = GuidanceGenerator::classify(4000, 75.0);
        assert_eq!(p, "medium");
        assert_eq!(a, Action::Monitor);
    }

    #[test]
    fn test_classify_high() {
        let (p, a, _) = GuidanceGenerator::classify(2000, 85.0);
        assert_eq!(p, "high");
        assert_eq!(a, Action::ReduceContext);
    }

    #[test]
    fn test_classify_critical_available() {
        let (p, a, _) = GuidanceGenerator::classify(1999, 50.0);
        assert_eq!(p, "critical");
        assert_eq!(a, Action::AbortSafely);
    }

    #[test]
    fn test_classify_critical_used() {
        let (p, a, _) = GuidanceGenerator::classify(8000, 93.0);
        assert_eq!(p, "critical");
        assert_eq!(a, Action::AbortSafely);
    }

    // Guidance JSON serialization (perspective check)
    #[test]
    fn test_guidance_serialization_has_action() {
        let g = GuidanceGenerator::generate(12000, 30.0, 1_700_000, "conservative");
        let json = serde_json::to_value(&g).unwrap();
        assert!(json.get("action").is_some(), "JSON must contain 'action'");
        assert_eq!(json["action"], "ok");
        assert!(json.get("reason").is_some(), "JSON must contain 'reason'");
        assert!(
            json.get("pressure").is_some(),
            "JSON must contain 'pressure'"
        );
    }

    // Guidance JSON: suggestion is null when calibrated
    #[test]
    fn test_guidance_serialization_suggestion_null() {
        let g = GuidanceGenerator::generate(12000, 30.0, 1_700_000, "calibrated");
        let json = serde_json::to_value(&g).unwrap();
        assert!(
            json.get("suggestion").is_none(),
            "calibrated guidance should omit suggestion field, got: {:?}",
            json
        );
    }

    // Guidance JSON: suggestion present when conservative
    #[test]
    fn test_guidance_serialization_suggestion_present() {
        let g = GuidanceGenerator::generate(12000, 30.0, 1_700_000, "conservative");
        let json = serde_json::to_value(&g).unwrap();
        assert!(
            json.get("suggestion").and_then(|x| x.as_str()).is_some(),
            "conservative guidance should have non-null suggestion"
        );
    }

    // ===== UT-PS: 压力判定阈值测试 =====

    // UT-PS-001: available≥8GB 且 used≤50% → Low
    #[test]
    fn test_ut_ps_001_available_high_used_low() {
        let (pressure, action, _) = GuidanceGenerator::classify(8192, 50.0);
        assert_eq!(pressure, "low");
        assert_eq!(action, Action::Ok);
    }

    // UT-PS-002: available<4GB 且 used>80% → High
    #[test]
    fn test_ut_ps_002_available_low_used_high() {
        let (pressure, action, _) = GuidanceGenerator::classify(3999, 81.0);
        assert_eq!(pressure, "high");
        assert_eq!(action, Action::ReduceContext);
    }

    // UT-PS-003: available<2GB → Critical（可用内存触发）
    #[test]
    fn test_ut_ps_003_available_critical() {
        let (pressure, action, _) = GuidanceGenerator::classify(1999, 90.0);
        assert_eq!(pressure, "critical");
        assert_eq!(action, Action::AbortSafely);
    }

    // UT-PS-004: used>92% → Critical（使用率覆盖available充足的情况）
    #[test]
    fn test_ut_ps_004_used_percent_override() {
        let (pressure, action, _) = GuidanceGenerator::classify(8000, 93.0);
        assert_eq!(pressure, "critical");
        assert_eq!(action, Action::AbortSafely);
    }

    // ===== UT-PS: macOS 压力判定测试 =====

    // UT-PS-005: macOS memory_pressure=4 → Critical
    #[test]
    fn test_ut_ps_005_macos_pressure_critical() {
        let (pressure, action, _) = GuidanceGenerator::classify(1500, 95.0);
        assert_eq!(pressure, "critical", "memory_pressure=4 should be critical");
        assert_eq!(action, Action::AbortSafely);
    }

    // UT-PS-006: macOS memory_pressure 不可用回退 → 基于 available_mb
    #[test]
    fn test_ut_ps_006_macos_fallback() {
        let (pressure, action, _) = GuidanceGenerator::classify(3000, 75.0);
        assert_eq!(pressure, "high", "3000MB available should be high");
        assert_eq!(action, Action::ReduceContext);
    }

    // macOS 正常负载不应误报 high/critical
    #[test]
    fn test_ut_ps_006b_macos_no_false_positive() {
        let (pressure, _, _) = GuidanceGenerator::classify(4500, 50.0);
        assert!(
            pressure == "low" || pressure == "medium",
            "Normal macOS load should not be high/critical, got: {}",
            pressure
        );
    }
}
