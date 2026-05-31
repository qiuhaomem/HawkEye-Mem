// ============================================================================
// src/budget/analyzer.rs — 浪费分析引擎
// ============================================================================
// 识别 6 种典型浪费场景，生成优化建议
// ============================================================================

use crate::budget::{
    ActionType, Severity, Suggestion, TokenRecord, TokenSummary, WasteEntry, WasteType,
};

/// 浪费分析结果
#[derive(Debug)]
pub struct AnalysisReport {
    pub wastes: Vec<WasteEntry>,
    pub suggestions: Vec<Suggestion>,
    pub total_waste_tokens: u64,
    pub total_waste_cost: f64,
    pub data_sufficient: bool,
    pub message: String,
}

/// 浪费分析引擎
pub struct WasteAnalyzer;

impl WasteAnalyzer {
    /// 分析 Token 记录，识别浪费场景
    pub fn analyze(
        records: &[TokenRecord],
        summary: &TokenSummary,
        skills_count: u64,
        memory_size_tokens: u64,
        mcp_count: u64,
    ) -> AnalysisReport {
        let mut wastes = Vec::new();
        let mut suggestions = Vec::new();
        let mut suggestion_id: u32 = 1;

        // 场景 1: 冷启动过大
        if let Some(waste) = Self::check_cold_start(summary) {
            let desc = format!("首轮调用平均 {:} tokens，超过 30K 阈值", summary.first_call_tokens_avg);
            let detail = "建议精简 skills.list，禁用不常用的技能，减少 system prompt 注入量".to_string();

            if let Some(sug) = Self::make_suggestion(
                suggestion_id,
                WasteType::ColdStart,
                waste.severity.clone(),
                &desc,
                waste.estimated_waste_tokens,
                waste.estimated_waste_cost,
                ActionType::DisableSkill,
                &detail,
                "低",
            ) {
                suggestions.push(sug);
                suggestion_id += 1;
            }
            wastes.push(waste);
        }

        // 场景 2: 缓存命中率低
        if let Some(waste) = Self::check_cache_hit(summary) {
            let desc = format!("缓存命中率 {:.1}%，低于 50% 阈值", summary.cache_hit_rate);
            let detail = "建议调整缓存策略模式为 aggressive，增加 TTL，开启预取功能".to_string();

            if let Some(sug) = Self::make_suggestion(
                suggestion_id,
                WasteType::LowCacheHit,
                waste.severity.clone(),
                &desc,
                waste.estimated_waste_tokens,
                waste.estimated_waste_cost,
                ActionType::AdjustCache,
                &detail,
                "中",
            ) {
                suggestions.push(sug);
                suggestion_id += 1;
            }
            wastes.push(waste);
        }

        // 场景 3: 技能冗余
        if let Some(waste) = Self::check_skill_bloat(summary, skills_count) {
            let desc = format!("注册了 {skills_count} 个技能，大多数未被引用");
            let detail = "建议禁用不常用的技能（skills.disabled 配置），可将冷启动 input 减少 5-10K tokens".to_string();

            if let Some(sug) = Self::make_suggestion(
                suggestion_id,
                WasteType::SkillBloat,
                waste.severity.clone(),
                &desc,
                waste.estimated_waste_tokens,
                waste.estimated_waste_cost,
                ActionType::DisableSkill,
                &detail,
                "低",
            ) {
                suggestions.push(sug);
                suggestion_id += 1;
            }
            wastes.push(waste);
        }

        // 场景 4: Memory 臃肿
        if let Some(waste) = Self::check_memory_bloat(memory_size_tokens) {
            let desc = format!("Memory 注入量约 {memory_size_tokens} tokens，超过 5K 阈值");
            let detail = "建议精简 memory 文件，归档旧内容，控制注入量在 3K tokens 以内".to_string();

            if let Some(sug) = Self::make_suggestion(
                suggestion_id,
                WasteType::MemoryBloat,
                waste.severity.clone(),
                &desc,
                waste.estimated_waste_tokens,
                waste.estimated_waste_cost,
                ActionType::CompressMemory,
                &detail,
                "中",
            ) {
                suggestions.push(sug);
                suggestion_id += 1;
            }
            wastes.push(waste);
        }

        // 场景 5: MCP 冗余
        if let Some(waste) = Self::check_mcp_redundancy(mcp_count) {
            let desc = format!("运行了 {mcp_count} 个 MCP Server，部分未被使用");
            let detail = "建议移除未使用的 MCP Server，可减少冷启动延迟和 input tokens".to_string();

            if let Some(sug) = Self::make_suggestion(
                suggestion_id,
                WasteType::McpRedundancy,
                waste.severity.clone(),
                &desc,
                waste.estimated_waste_tokens,
                waste.estimated_waste_cost,
                ActionType::RemoveMcpServer,
                &detail,
                "低",
            ) {
                suggestions.push(sug);
                suggestion_id += 1;
            }
            wastes.push(waste);
        }

        // 按节省金额排序（从高到低）
        suggestions.sort_by(|a, b| {
            b.expected_savings_cost
                .partial_cmp(&a.expected_savings_cost)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let total_waste_tokens = wastes.iter().map(|w| w.estimated_waste_tokens).sum();
        let total_waste_cost = wastes.iter().map(|w| w.estimated_waste_cost).sum();

        let data_sufficient = !records.is_empty();
        let message = if !data_sufficient {
            "Token 数据不足，无法进行浪费分析，请先使用 Hermes Agent 进行对话".to_string()
        } else if wastes.is_empty() {
            "未检测到明显的 Token 浪费场景，您的配置已经很优化了".to_string()
        } else {
            format!("检测到 {} 个浪费场景，预计可节省 {} tokens（约 ${:.4}）", wastes.len(), total_waste_tokens, total_waste_cost)
        };

        AnalysisReport {
            wastes,
            suggestions,
            total_waste_tokens,
            total_waste_cost,
            data_sufficient,
            message,
        }
    }

    fn check_cold_start(summary: &TokenSummary) -> Option<WasteEntry> {
        if summary.first_call_tokens_avg > 30_000 {
            let wasted = summary.first_call_tokens_avg - 20_000; // 理想值约 20K
            Some(WasteEntry {
                id: "cold_start".into(),
                waste_type: WasteType::ColdStart,
                severity: Severity::High,
                estimated_waste_tokens: wasted * 2, // 每天约2次冷启动
                estimated_waste_cost: 0.0,
                description: format!("冷启动过大（首轮 {:} tokens）", summary.first_call_tokens_avg),
                detail: format!("超过 30K 阈值 {} tokens", wasted),
            })
        } else {
            None
        }
    }

    fn check_cache_hit(summary: &TokenSummary) -> Option<WasteEntry> {
        if summary.cache_hit_rate < 50.0 && summary.total_api_calls > 5 {
            let non_cache = summary.total_input_tokens.saturating_sub(summary.total_cache_hit_tokens);
            let waste_rate = (50.0 - summary.cache_hit_rate) / 100.0;
            let wasted = (non_cache as f64 * waste_rate) as u64;
            Some(WasteEntry {
                id: "low_cache_hit".into(),
                waste_type: WasteType::LowCacheHit,
                severity: Severity::High,
                estimated_waste_tokens: wasted,
                estimated_waste_cost: 0.0,
                description: format!("缓存命中率低（{:.1}%）", summary.cache_hit_rate),
                detail: format!("应该有 {:.1}% 的请求命中缓存但实际只有 {:.1}%", 50.0, summary.cache_hit_rate),
            })
        } else {
            None
        }
    }

    fn check_skill_bloat(_summary: &TokenSummary, skills_count: u64) -> Option<WasteEntry> {
        if skills_count > 50 {
            let wasted = (skills_count - 30) * 100; // 每个冗余技能约 100 tokens
            Some(WasteEntry {
                id: "skill_bloat".into(),
                waste_type: WasteType::SkillBloat,
                severity: Severity::Medium,
                estimated_waste_tokens: wasted,
                estimated_waste_cost: 0.0,
                description: format!("技能数量过多（{} 个）", skills_count),
                detail: format!("建议精简到 30 个以下，可减少约 {wasted} tokens/轮"),
            })
        } else {
            None
        }
    }

    fn check_memory_bloat(memory_size_tokens: u64) -> Option<WasteEntry> {
        if memory_size_tokens > 5_000 {
            let wasted = memory_size_tokens - 3_000;
            Some(WasteEntry {
                id: "memory_bloat".into(),
                waste_type: WasteType::MemoryBloat,
                severity: Severity::Medium,
                estimated_waste_tokens: wasted,
                estimated_waste_cost: 0.0,
                description: format!("Memory 注入量 {memory_size_tokens} tokens"),
                detail: format!("超出建议值 5K 阈值，可压缩 {wasted} tokens/轮"),
            })
        } else {
            None
        }
    }

    fn check_mcp_redundancy(mcp_count: u64) -> Option<WasteEntry> {
        if mcp_count > 3 {
            let wasted = (mcp_count - 1) * 5_000; // 每个冗余 MCP 约 5K tokens（冷启动）
            Some(WasteEntry {
                id: "mcp_redundancy".into(),
                waste_type: WasteType::McpRedundancy,
                severity: Severity::Low,
                estimated_waste_tokens: wasted,
                estimated_waste_cost: 0.0,
                description: format!("MCP Server 数量过多（{} 个）", mcp_count),
                detail: format!("建议保留核心 Server，移除 {wasted} tokens 的冗余注入"),
            })
        } else {
            None
        }
    }

    fn make_suggestion(
        id: u32,
        waste_type: WasteType,
        severity: Severity,
        description: &str,
        savings_tokens: u64,
        savings_cost: f64,
        action_type: ActionType,
        action_detail: &str,
        risk: &str,
    ) -> Option<Suggestion> {
        Some(Suggestion {
            id,
            waste_type,
            severity,
            description: description.to_string(),
            expected_savings_tokens: savings_tokens,
            expected_savings_cost: savings_cost,
            action_type,
            action_detail: action_detail.to_string(),
            risk: risk.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::{TokenSource, TokenSummary};

    fn make_summary(first_call_avg: u64, cache_rate: f64, total_calls: u64, total_in: u64, total_cache: u64) -> TokenSummary {
        TokenSummary {
            period_hours: 24,
            total_input_tokens: total_in,
            total_output_tokens: 0,
            total_cache_hit_tokens: total_cache,
            total_api_calls: total_calls,
            cache_hit_rate: cache_rate,
            estimated_cost_usd: 0.0,
            by_model: vec![],
            cold_start_ratio: 0.0,
            first_call_tokens_avg: first_call_avg,
        }
    }

    #[test]
    fn test_cold_start_detected() {
        let summary = make_summary(45_000, 80.0, 10, 200_000, 160_000);
        let report = WasteAnalyzer::analyze(&[], &summary, 20, 2_000, 2);
        assert!(!report.wastes.is_empty());
        assert!(report.wastes.iter().any(|w| matches!(w.waste_type, WasteType::ColdStart)));
    }

    #[test]
    fn test_cold_start_not_detected() {
        let summary = make_summary(20_000, 80.0, 10, 200_000, 160_000);
        let report = WasteAnalyzer::analyze(&[], &summary, 20, 2_000, 2);
        assert!(!report.wastes.iter().any(|w| matches!(w.waste_type, WasteType::ColdStart)));
    }

    #[test]
    fn test_low_cache_hit() {
        let summary = make_summary(20_000, 30.0, 20, 200_000, 60_000);
        let report = WasteAnalyzer::analyze(&[], &summary, 20, 2_000, 2);
        assert!(report.wastes.iter().any(|w| matches!(w.waste_type, WasteType::LowCacheHit)));
    }

    #[test]
    fn test_skill_bloat() {
        let summary = make_summary(20_000, 80.0, 10, 200_000, 160_000);
        let report = WasteAnalyzer::analyze(&[], &summary, 80, 2_000, 2);
        assert!(report.wastes.iter().any(|w| matches!(w.waste_type, WasteType::SkillBloat)));
    }

    #[test]
    fn test_memory_bloat() {
        let summary = make_summary(20_000, 80.0, 10, 200_000, 160_000);
        let report = WasteAnalyzer::analyze(&[], &summary, 20, 8_000, 2);
        assert!(report.wastes.iter().any(|w| matches!(w.waste_type, WasteType::MemoryBloat)));
    }

    #[test]
    fn test_multiple_wastes_sorted() {
        let summary = make_summary(45_000, 30.0, 20, 200_000, 60_000);
        let report = WasteAnalyzer::analyze(&[], &summary, 80, 8_000, 5);
        assert!(report.wastes.len() >= 4);
        // 按优先级排序
        for i in 1..report.suggestions.len() {
            let severity_order = |s: &Severity| -> u8 {
                match s {
                    Severity::High => 3,
                    Severity::Medium => 2,
                    Severity::Low => 1,
                }
            };
            assert!(
                severity_order(&report.suggestions[i - 1].severity)
                    >= severity_order(&report.suggestions[i].severity)
            );
        }
    }

    #[test]
    fn test_no_waste() {
        let summary = make_summary(20_000, 99.0, 20, 200_000, 198_000);
        let report = WasteAnalyzer::analyze(&[], &summary, 30, 3_000, 2);
        assert!(report.wastes.is_empty());
    }

    #[test]
    fn test_empty_data() {
        let summary = make_summary(0, 0.0, 0, 0, 0);
        let report = WasteAnalyzer::analyze(&[], &summary, 0, 0, 0);
        assert!(!report.data_sufficient);
    }
}
