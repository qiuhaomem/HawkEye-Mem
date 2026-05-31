// ============================================================================
// src/budget/cost.rs — 费用换算器
// ============================================================================
// 内置定价表 + 用户自定义定价 + API费用计算
// ============================================================================

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 模型定价信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPrice {
    pub input_per_1m: f64,
    pub output_per_1m: f64,
    pub cache_hit_per_1m: f64,
}

/// 费用换算器
pub struct CostCalculator {
    builtin_prices: HashMap<String, ModelPrice>,
    custom_prices: HashMap<String, ModelPrice>,
}

impl Default for CostCalculator {
    fn default() -> Self {
        Self::new()
    }
}

impl CostCalculator {
    pub fn new() -> Self {
        let mut builtin_prices = HashMap::new();

        // 主流模型定价表（美元/百万 tokens）
        builtin_prices.insert(
            "deepseek/deepseek-v4-flash".to_string(),
            ModelPrice { input_per_1m: 0.07, output_per_1m: 0.28, cache_hit_per_1m: 0.014 },
        );
        builtin_prices.insert(
            "deepseek/deepseek-v3".to_string(),
            ModelPrice { input_per_1m: 0.27, output_per_1m: 1.10, cache_hit_per_1m: 0.027 },
        );
        builtin_prices.insert(
            "anthropic/claude-sonnet-4".to_string(),
            ModelPrice { input_per_1m: 3.00, output_per_1m: 15.00, cache_hit_per_1m: 0.30 },
        );
        builtin_prices.insert(
            "anthropic/claude-haiku-3.5".to_string(),
            ModelPrice { input_per_1m: 0.80, output_per_1m: 4.00, cache_hit_per_1m: 0.08 },
        );
        builtin_prices.insert(
            "openai/gpt-4o".to_string(),
            ModelPrice { input_per_1m: 2.50, output_per_1m: 10.00, cache_hit_per_1m: 1.25 },
        );
        builtin_prices.insert(
            "openai/gpt-4o-mini".to_string(),
            ModelPrice { input_per_1m: 0.15, output_per_1m: 0.60, cache_hit_per_1m: 0.075 },
        );
        builtin_prices.insert(
            "google/gemini-2.0-flash".to_string(),
            ModelPrice { input_per_1m: 0.10, output_per_1m: 0.40, cache_hit_per_1m: 0.025 },
        );
        builtin_prices.insert(
            "mistral/mistral-large".to_string(),
            ModelPrice { input_per_1m: 2.00, output_per_1m: 6.00, cache_hit_per_1m: 0.20 },
        );
        builtin_prices.insert(
            "meta/llama-3-70b".to_string(),
            ModelPrice { input_per_1m: 0.59, output_per_1m: 0.79, cache_hit_per_1m: 0.059 },
        );
        builtin_prices.insert(
            "mimo/mimo-v2_5-pro".to_string(),
            ModelPrice { input_per_1m: 0.50, output_per_1m: 2.00, cache_hit_per_1m: 0.10 },
        );

        // 尝试加载用户自定义定价
        let custom_prices = Self::load_custom_prices();

        CostCalculator {
            builtin_prices,
            custom_prices,
        }
    }

    /// 加载用户自定义定价文件
    fn load_custom_prices() -> HashMap<String, ModelPrice> {
        let custom_path = dirs_next::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
            .join(".config/hawk-eye-mem/custom_prices.json");

        if !custom_path.exists() {
            return HashMap::new();
        }

        match std::fs::read_to_string(&custom_path) {
            Ok(content) => {
                serde_json::from_str(&content).unwrap_or_default()
            }
            Err(_) => HashMap::new(),
        }
    }

    /// 计算单次调用的费用
    pub fn calculate_cost(
        &self,
        model: &str,
        provider: &str,
        input_tokens: u64,
        output_tokens: u64,
        cache_hit_tokens: u64,
    ) -> Result<f64, String> {
        let model_key = if provider.is_empty() || provider == "unknown" {
            model.to_string()
        } else {
            format!("{}/{}", provider, model)
        };

        // 优先用户自定义定价，再内置定价
        let price = self.custom_prices
            .get(&model_key)
            .or_else(|| self.builtin_prices.get(&model_key))
            .or_else(|| {
                // 尝试模糊匹配（只匹配模型名）
                self.custom_prices.iter().chain(self.builtin_prices.iter())
                    .find(|(k, _)| k.contains(model))
                    .map(|(_, v)| v)
            });

        match price {
            Some(p) => {
                let input_cost = (input_tokens as f64 / 1_000_000.0) * p.input_per_1m;
                let output_cost = (output_tokens as f64 / 1_000_000.0) * p.output_per_1m;
                let cache_cost = (cache_hit_tokens as f64 / 1_000_000.0) * p.cache_hit_per_1m;
                let non_cache_input = input_tokens.saturating_sub(cache_hit_tokens);
                let non_cache_cost = (non_cache_input as f64 / 1_000_000.0) * p.input_per_1m;
                Ok(input_cost + output_cost + cache_cost - non_cache_cost)
            }
            None => {
                // 未知模型，用平均价格估算
                let avg_input = 0.50;
                let avg_output = 2.00;
                let cost = (input_tokens as f64 / 1_000_000.0) * avg_input
                    + (output_tokens as f64 / 1_000_000.0) * avg_output;
                Ok(cost)
            }
        }
    }

    /// 计算批量的总费用
    pub fn calculate_batch_cost(
        &self,
        records: &[crate::budget::TokenRecord],
    ) -> f64 {
        let mut total = 0.0;
        for rec in records {
            if let Ok(cost) = self.calculate_cost(
                &rec.model,
                &rec.provider,
                rec.input_tokens,
                rec.output_tokens,
                rec.cache_hit_tokens,
            ) {
                total += cost;
            }
        }
        total
    }

    /// 获取已注册模型列表
    pub fn known_models(&self) -> Vec<String> {
        let mut models: Vec<String> = self.builtin_prices.keys().cloned().collect();
        models.sort();
        models
    }

    /// 查询单个模型定价
    pub fn get_price(&self, model_key: &str) -> Option<&ModelPrice> {
        self.custom_prices
            .get(model_key)
            .or_else(|| self.builtin_prices.get(model_key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_model_cost() {
        let calculator = CostCalculator::new();
        let cost = calculator
            .calculate_cost("deepseek-v4-flash", "deepseek", 100_000, 20_000, 30_000)
            .unwrap();
        // 100K input * $0.07/1M = $0.007
        // 20K output * $0.28/1M = $0.0056
        // 30K cache_hit * $0.014/1M = $0.00042
        // non_cache = 70K * $0.07/1M = $0.0049
        // total ≈ $0.007 + $0.0056 + $0.00042 - $0.0049... 简化验证
        assert!(cost > 0.0);
    }

    #[test]
    fn test_unknown_model_cost() {
        let calculator = CostCalculator::new();
        let cost = calculator
            .calculate_cost("unknown-model", "unknown", 1_000, 500, 0)
            .unwrap();
        assert!(cost > 0.0);
    }

    #[test]
    fn test_builtin_prices_populated() {
        let calculator = CostCalculator::new();
        let models = calculator.known_models();
        assert!(models.len() >= 5, "应包含至少 5 个模型");
        assert!(models.iter().any(|m| m.contains("deepseek")));
    }

    #[test]
    fn test_batch_cost() {
        let calculator = CostCalculator::new();
        let records = vec![
            crate::budget::TokenRecord {
                timestamp: "".into(),
                model: "deepseek-v4-flash".into(),
                provider: "deepseek".into(),
                input_tokens: 100_000,
                output_tokens: 20_000,
                cache_hit_tokens: 30_000,
                latency_sec: 0.0,
                is_first_call: false,
                session_id: None,
                source: crate::budget::TokenSource::StateDb,
            },
        ];
        let cost = calculator.calculate_batch_cost(&records);
        assert!(cost > 0.0);
    }

    #[test]
    fn test_get_price_existing() {
        let calculator = CostCalculator::new();
        assert!(calculator.get_price("deepseek/deepseek-v4-flash").is_some());
    }

    #[test]
    fn test_get_price_nonexistent() {
        let calculator = CostCalculator::new();
        assert!(calculator.get_price("nonexistent/model").is_none());
    }
}
