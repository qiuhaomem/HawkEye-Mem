use serde::Deserialize;
use std::sync::OnceLock;

// ============================================================================
// 模型条目
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct ModelEntry {
    pub name: String,
    pub size_b: u64,
    pub bytes_per_token: u64,
    pub memory_overhead_mb: u64,
    pub quantizations: Vec<String>,
    pub min_context: u32,
    pub max_context: u32,
    pub source: String,
    pub last_updated: String,
}

#[derive(Debug, Deserialize)]
struct ModelsFile {
    models: Vec<ModelEntry>,
}

// ============================================================================
// 模型库（编译期嵌入 + 懒加载）
// ============================================================================

pub struct ModelLibrary;

static MODELS: OnceLock<Vec<ModelEntry>> = OnceLock::new();

fn load_models() -> &'static Vec<ModelEntry> {
    MODELS.get_or_init(|| {
        let raw = include_str!("models.toml");
        let parsed: ModelsFile = toml::from_str(raw).expect("models.toml 解析失败");
        parsed.models
    })
}

impl ModelLibrary {
    /// 按名称查找模型（不区分大小写）
    pub fn find(name: &str) -> Option<&'static ModelEntry> {
        let name_lower = name.to_lowercase();
        load_models()
            .iter()
            .find(|m| m.name.to_lowercase() == name_lower)
    }

    /// 列出所有模型
    pub fn all() -> &'static Vec<ModelEntry> {
        load_models()
    }

    /// 查找更小的模型（用于降级方案）
    /// 返回 size_b 比当前模型小的模型中最大的那个
    pub fn find_smaller(name: &str) -> Option<&'static ModelEntry> {
        let models = load_models();
        let name_lower = name.to_lowercase();

        // 找到当前模型
        let current = models
            .iter()
            .find(|m| m.name.to_lowercase() == name_lower)?;
        let current_size = current.size_b;

        // 找 size_b 更小的模型中最大的
        models
            .iter()
            .filter(|m| m.size_b < current_size)
            .max_by_key(|m| m.size_b)
    }
}

// ============================================================================
// 量化相关工具
// ============================================================================

/// 获取量化对应的 bytes_per_weight
pub fn quantization_bytes_per_weight(q: &str) -> f64 {
    match q.to_uppercase().as_str() {
        "Q8_0" => 1.0,
        "Q6_K" => 0.75,
        "Q5_K_M" => 0.625,
        "Q4_K_M" => 0.5,
        "Q3_K_M" => 0.375,
        "Q2_K" => 0.25,
        _ => 0.5, // 未知量化默认 Q4_K_M
    }
}

/// 获取模型支持的量化中更低一级的量化名
pub fn next_lower_quantization(current: &str, quantizations: &[String]) -> Option<String> {
    let order = ["Q8_0", "Q6_K", "Q5_K_M", "Q4_K_M", "Q3_K_M", "Q2_K"];
    let current_upper = current.to_uppercase();

    // 找到当前量化在顺序中的位置
    let pos = order.iter().position(|&q| q == current_upper.as_str())?;

    // 找下一个更低的量化（顺序中更靠后的）
    for &q in &order[pos + 1..] {
        if quantizations.iter().any(|sq| sq.to_uppercase() == q) {
            return Some(q.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // UT-ML-001: 查找存在的模型
    #[test]
    fn test_ut_ml_001_find_existing() {
        let model = ModelLibrary::find("llama3-8b");
        assert!(model.is_some(), "llama3-8b should exist");
        assert_eq!(model.unwrap().size_b, 8000000000);
    }

    // UT-ML-002: 查找不存在的模型返回 None
    #[test]
    fn test_ut_ml_002_find_nonexistent() {
        let model = ModelLibrary::find("nonexistent-model-999b");
        assert!(model.is_none(), "nonexistent model should return None");
    }

    // UT-ML-003: ALL() 返回 8 个模型
    #[test]
    fn test_ut_ml_003_all_count() {
        let all = ModelLibrary::all();
        assert_eq!(all.len(), 8, "should have 8 models");
    }

    // UT-ML-004: FIND_SMALLER 返回下一个更小的模型
    #[test]
    fn test_ut_ml_004_find_smaller() {
        let smaller = ModelLibrary::find_smaller("llama3-8b");
        assert!(smaller.is_some(), "llama3-8b should have a smaller model");
        // llama3-8b(8B) 比它小的最大模型应该是 qwen2-7b or mistral-7b(7B)
        let size = smaller.unwrap().size_b;
        assert!(size < 8000000000, "smaller model should have fewer params");
        assert!(
            size >= 7000000000,
            "should be the largest among smaller ones (>=7B)"
        );
    }

    // UT-ML-005: 最小模型 find_smaller 返回 None
    #[test]
    fn test_ut_ml_005_smallest_no_smaller() {
        let smallest = ModelLibrary::find_smaller("phi-3-mini");
        assert!(
            smallest.is_none(),
            "phi-3-mini (3.8B) should have no smaller model"
        );
    }

    // UT-ML-006: 不区分大小写查找
    #[test]
    fn test_ut_ml_006_case_insensitive() {
        let m1 = ModelLibrary::find("LLAMA3-8B");
        let m2 = ModelLibrary::find("Llama3-8b");
        assert!(m1.is_some() && m2.is_some());
        assert_eq!(m1.unwrap().name, "llama3-8b");
    }

    // UT-ML-007: 量化 bps 映射正确
    #[test]
    fn test_ut_ml_007_quantization_bps() {
        assert!((quantization_bytes_per_weight("Q8_0") - 1.0).abs() < 1e-6);
        assert!((quantization_bytes_per_weight("Q6_K") - 0.75).abs() < 1e-6);
        assert!((quantization_bytes_per_weight("Q5_K_M") - 0.625).abs() < 1e-6);
        assert!((quantization_bytes_per_weight("Q4_K_M") - 0.5).abs() < 1e-6);
        assert!((quantization_bytes_per_weight("Q3_K_M") - 0.375).abs() < 1e-6);
        assert!((quantization_bytes_per_weight("Q2_K") - 0.25).abs() < 1e-6);
        assert!((quantization_bytes_per_weight("unknown") - 0.5).abs() < 1e-6);
    }

    // UT-ML-008: next_lower_quantization 返回更低量化
    #[test]
    fn test_ut_ml_008_next_lower_quant() {
        let qs: Vec<String> = vec![
            "Q2_K".into(),
            "Q3_K_M".into(),
            "Q4_K_M".into(),
            "Q5_K_M".into(),
            "Q6_K".into(),
            "Q8_0".into(),
        ];
        let lower = next_lower_quantization("Q6_K", &qs);
        assert_eq!(lower, Some("Q5_K_M".to_string()));

        let lowest = next_lower_quantization("Q2_K", &qs);
        assert_eq!(lowest, None);
    }

    // UT-ML-009: next_lower 跳过不支持的量化
    #[test]
    fn test_ut_ml_009_next_lower_skip_missing() {
        // gemma-2-9b 没有 Q6_K
        let qs: Vec<String> = vec![
            "Q2_K".into(),
            "Q3_K_M".into(),
            "Q4_K_M".into(),
            "Q5_K_M".into(),
            "Q8_0".into(),
        ];
        // Q8_0 的下一个应该是 Q6_K，但不存在，所以跳转到 Q5_K_M
        let lower = next_lower_quantization("Q8_0", &qs);
        assert!(lower.is_some());
        let q = lower.unwrap();
        assert_eq!(q, "Q5_K_M");
    }
}
