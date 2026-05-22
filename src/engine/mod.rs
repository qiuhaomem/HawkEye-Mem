pub mod assessment;
pub mod guidance;

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum Confidence {
    Conservative,
    Calibrated,
}

impl std::fmt::Display for Confidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Confidence::Conservative => write!(f, "conservative"),
            Confidence::Calibrated => write!(f, "calibrated"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelConfig {
    pub bytes_per_token: u64,
    pub margin: f64,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            bytes_per_token: 2048,
            margin: 30.0,
        }
    }
}

pub struct EstimationEngine;

impl EstimationEngine {
    pub fn estimate(available_mb: u64, config: &Option<ModelConfig>) -> EstimationResult {
        if available_mb < 256 {
            return EstimationResult {
                estimated_tokens: 0,
                confidence: Confidence::Conservative,
            };
        }

        let (bpt, margin, confidence) = match config {
            Some(cfg) => (cfg.bytes_per_token, cfg.margin, Confidence::Calibrated),
            None => (2048u64, 30.0, Confidence::Conservative),
        };

        let available_bytes = (available_mb as u128) * 1024 * 1024;
        let usable_ratio = 1.0 - (margin / 100.0);
        let estimated = (available_bytes as f64 * usable_ratio) / bpt as f64;

        EstimationResult {
            estimated_tokens: estimated as u64,
            confidence,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EstimationResult {
    pub estimated_tokens: u64,
    pub confidence: Confidence,
}

#[cfg(test)]
mod tests {
    use super::*;

    // UT-EE-001: 默认配置16GB → estimated_tokens > 0, Conservative
    #[test]
    fn test_ut_ee_001_default_16gb() {
        let result = EstimationEngine::estimate(16384, &None);
        assert!(
            result.estimated_tokens > 0,
            "estimated_tokens should be > 0"
        );
        assert_eq!(result.confidence, Confidence::Conservative);
        // 公式: 16384*1024^2/2048*0.7 = 5,872,025,600/2048*0.7
        let expected = ((16384u128 * 1024 * 1024) as f64 * 0.7 / 2048.0) as u64;
        let diff = if result.estimated_tokens > expected {
            result.estimated_tokens - expected
        } else {
            expected - result.estimated_tokens
        };
        assert!(
            diff <= 1,
            "公式偏差过大: got {}, expected {}",
            result.estimated_tokens,
            expected
        );
    }

    // UT-EE-002: 自定义参数 → Calibrated
    #[test]
    fn test_ut_ee_002_custom_config() {
        let config = Some(ModelConfig {
            bytes_per_token: 4096,
            margin: 20.0,
        });
        let result = EstimationEngine::estimate(16384, &config);
        assert_eq!(result.confidence, Confidence::Calibrated);
        let expected = (16384u128 * 1024 * 1024) as f64 * 0.8 / 4096.0;
        let diff = (result.estimated_tokens as f64 - expected).abs();
        assert!(
            diff < 1.0,
            "公式偏差过大: {} vs {}",
            result.estimated_tokens,
            expected
        );
    }

    // UT-EE-003: 极低内存512MB
    #[test]
    fn test_ut_ee_003_low_memory_512mb() {
        let result = EstimationEngine::estimate(512, &None);
        assert!(
            result.estimated_tokens < 200_000,
            "512MB should estimate <200K tokens, got {}",
            result.estimated_tokens
        );
    }

    // UT-EE-004: 零内存/极低内存防护
    #[test]
    fn test_ut_ee_004_zero_memory_guard() {
        let r1 = EstimationEngine::estimate(0, &None);
        assert_eq!(r1.estimated_tokens, 0, "0MB should return 0 tokens");
        assert_eq!(r1.confidence, Confidence::Conservative);

        let r2 = EstimationEngine::estimate(128, &None);
        assert_eq!(r2.estimated_tokens, 0, "128MB should return 0 tokens");
        assert_eq!(r2.confidence, Confidence::Conservative);
    }

    // UT-EE-005: 边界值4GB、8GB
    #[test]
    fn test_ut_ee_005_boundary_values() {
        let r4gb = EstimationEngine::estimate(4096, &None);
        let expected_4gb = ((4096u128 * 1024 * 1024) as f64 * 0.7 / 2048.0) as u64;
        let diff_4gb = if r4gb.estimated_tokens > expected_4gb {
            r4gb.estimated_tokens - expected_4gb
        } else {
            expected_4gb - r4gb.estimated_tokens
        };
        assert!(
            diff_4gb <= 1,
            "4GB估算偏差: got {}, expected {}",
            r4gb.estimated_tokens,
            expected_4gb
        );
        assert_eq!(r4gb.confidence, Confidence::Conservative);

        let r8gb = EstimationEngine::estimate(8192, &None);
        let expected_8gb = ((8192u128 * 1024 * 1024) as f64 * 0.7 / 2048.0) as u64;
        let diff_8gb = if r8gb.estimated_tokens > expected_8gb {
            r8gb.estimated_tokens - expected_8gb
        } else {
            expected_8gb - r8gb.estimated_tokens
        };
        assert!(
            diff_8gb <= 1,
            "8GB估算偏差: got {}, expected {}",
            r8gb.estimated_tokens,
            expected_8gb
        );
        assert_eq!(r8gb.confidence, Confidence::Conservative);

        assert!(
            r8gb.estimated_tokens > r4gb.estimated_tokens,
            "8GB应比4GB估算更多tokens"
        );
    }
}
