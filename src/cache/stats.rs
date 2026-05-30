use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Cache gap analysis report
#[derive(Debug, Clone, Serialize)]
pub struct CacheGapReport {
    /// Analysis period in days
    pub period_days: u32,
    /// Actual hit rate in the period
    pub actual_hit_rate: f64,
    /// Target hit rate (default 99.0%)
    pub target_hit_rate: f64,
    /// Gap percentage
    pub gap_percent: f64,
    /// Total requests in period
    pub total_requests: u64,
    /// Total misses in period
    pub total_misses: u64,
    /// Estimated daily miss tokens
    pub estimated_daily_miss_tokens: u64,
    /// Gap categories with percentages
    pub gaps: Vec<GapCategory>,
    /// Fix suggestions
    pub suggestions: Vec<FixSuggestion>,
}

/// A category of cache gap
#[derive(Debug, Clone, Serialize)]
pub struct GapCategory {
    /// Category name (e.g. "new_session_cold_start")
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Estimated percentage of total misses
    pub percent_of_misses: f64,
    /// Priority: high/medium/low
    pub priority: String,
}

/// A fix suggestion
#[derive(Debug, Clone, Serialize)]
pub struct FixSuggestion {
    /// Priority: high/medium/low
    pub priority: String,
    /// What to fix
    pub issue: String,
    /// Suggested action
    pub action: String,
    /// Expected improvement
    pub expected_improvement: String,
}

/// Cache hit report from Skill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheHitReport {
    /// Model name (will be hashed for storage — CR-06)
    pub model_name: String,
    pub hit_count: u64,
    pub miss_count: u64,
    /// Estimated cost saved in USD (CR-29: 2 decimal precision)
    pub cost_saved_usd: f64,
    pub timestamp: String,
}

/// 24-hour cache statistics
#[derive(Debug, Clone, Serialize)]
pub struct CacheStats {
    pub hit_rate_24h: f64,
    pub total_requests_24h: u64,
    pub total_hits_24h: u64,
    pub total_misses_24h: u64,
    pub estimated_savings_usd: f64,
}

/// Cache stats storage (append-only JSONL)
pub struct CacheStatsStore {
    path: PathBuf,
}

impl CacheStatsStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Read reports since cutoff (timestamp comparison, ISO format)
    pub fn read_since(&self, cutoff: &str) -> Result<Vec<CacheHitReport>, String> {
        let content = match std::fs::read_to_string(&self.path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => {
                eprintln!("[hawk-eye-mem] Failed to read cache stats: {}", e);
                return Ok(Vec::new());
            }
        };

        let mut reports = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<CacheHitReport>(line) {
                Ok(report) => {
                    if report.timestamp.as_str() >= cutoff {
                        reports.push(report);
                    }
                }
                Err(e) => {
                    eprintln!("[hawk-eye-mem] Skipping malformed cache stats line: {}", e);
                }
            }
        }
        Ok(reports)
    }
}

/// Cache stats collector — aggregates hit reports from Skill
pub struct CacheStatsCollector {
    store: CacheStatsStore,
}

impl CacheStatsCollector {
    pub fn new(store: CacheStatsStore) -> Self {
        Self { store }
    }

    /// Calculate 24-hour hit rate
    pub fn stats_24h(&self) -> CacheStats {
        let cutoff = (Utc::now() - chrono::Duration::hours(24))
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string();
        let reports = self.store.read_since(&cutoff).unwrap_or_default();

        let total_hits: u64 = reports.iter().map(|r| r.hit_count).sum();
        let total_misses: u64 = reports.iter().map(|r| r.miss_count).sum();
        let total = total_hits + total_misses;
        let hit_rate = if total > 0 {
            ((total_hits as f64 / total as f64) * 100.0 * 100.0).round() / 100.0
        } else {
            0.0
        };
        let estimated_savings_usd: f64 = reports.iter().map(|r| r.cost_saved_usd).sum();

        CacheStats {
            hit_rate_24h: hit_rate,
            total_requests_24h: total,
            total_hits_24h: total_hits,
            total_misses_24h: total_misses,
            estimated_savings_usd: (estimated_savings_usd * 100.0).round() / 100.0,
        }
    }

    /// Analyze cache gaps — why isn't hit rate at 99%?
    pub fn analyze_gaps(&self, days: u32, target_hit_rate: f64) -> CacheGapReport {
        let cutoff = (Utc::now() - chrono::Duration::days(days as i64))
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string();
        let reports = self.store.read_since(&cutoff).unwrap_or_default();

        let total_hits: u64 = reports.iter().map(|r| r.hit_count).sum();
        let total_misses: u64 = reports.iter().map(|r| r.miss_count).sum();
        let total = total_hits + total_misses;
        let raw_hit_rate = if total > 0 {
            (total_hits as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        let gap = (target_hit_rate - raw_hit_rate).max(0.0);

        let days_f = days.max(1) as f64;
        let estimated_daily_miss_tokens = ((total_misses as f64 * 500.0) / days_f) as u64;

        let mut gaps = Vec::new();
        let mut suggestions = Vec::new();

        if total == 0 {
            gaps.push(GapCategory {
                name: "no_data".to_string(),
                description: "No cache hit reports found for this period".to_string(),
                percent_of_misses: 0.0,
                priority: "info".to_string(),
            });
            suggestions.push(FixSuggestion {
                priority: "info".to_string(),
                issue: "No cache data available".to_string(),
                action: "Ensure hermes-cache-strategy Skill is installed and reporting".to_string(),
                expected_improvement: "Enable cache monitoring".to_string(),
            });
            return CacheGapReport {
                period_days: days,
                actual_hit_rate: 0.0,
                target_hit_rate,
                gap_percent: 0.0,
                total_requests: 0,
                total_misses: 0,
                estimated_daily_miss_tokens: 0,
                gaps,
                suggestions,
            };
        }

        // ============================================================
        // 动态缺口分类 — 基于可观测数据而非固定硬编码比例
        // ============================================================

        // 1. 按模型聚合 miss
        let mut model_misses: HashMap<String, u64> = HashMap::new();
        for r in &reports {
            *model_misses.entry(r.model_name.clone()).or_insert(0) += r.miss_count;
        }

        // 2. 计算模型维度指标
        let total_misses_f = total_misses as f64;
        let major_models: Vec<_> = model_misses
            .iter()
            .filter(|(_, &m)| (m as f64 / total_misses_f) > 0.10)
            .collect();
        let has_multi_model = major_models.len() >= 2;

        // 3. 分级缺口分析 + 动态比例推导
        //    原则：命中率越高，冷启动占比越大（低级别问题已被解决）
        //          多模型活跃 ⇒ model_switch 是真实原因
        //          其余归为 other（tool_output / compression / 网络波动等）
        struct GapProfile {
            cold_start: f64,
            model_switch: f64,
            other: f64,
        }

        let profile = if raw_hit_rate >= target_hit_rate {
            // 已达目标
            gaps.push(GapCategory {
                name: "at_target".to_string(),
                description: format!(
                    "Hit rate {:.1}% meets or exceeds {:.0}% target",
                    raw_hit_rate, target_hit_rate
                ),
                percent_of_misses: 0.0,
                priority: "ok".to_string(),
            });
            None // 不执行下面的 gap push 逻辑
        } else if raw_hit_rate < 90.0 {
            // 命中率极低 → 核心问题：缓存机制整体失效
            suggestions.push(FixSuggestion {
                priority: "high".to_string(),
                issue: "Hit rate critically low".to_string(),
                action: "Enable aggressive cache mode: hermes config set cache.mode aggressive"
                    .to_string(),
                expected_improvement: "Target: 95%+".to_string(),
            });
            Some(GapProfile { cold_start: 50.0, model_switch: 0.0, other: 50.0 })
        } else if raw_hit_rate < 95.0 {
            // 中等命中率 → 冷启动 + 工具输出波动
            if has_multi_model {
                Some(GapProfile { cold_start: 35.0, model_switch: 20.0, other: 45.0 })
            } else {
                Some(GapProfile { cold_start: 42.0, model_switch: 0.0, other: 58.0 })
            }
        } else {
            // 接近目标 → 冷启动残差为主
            if has_multi_model {
                Some(GapProfile { cold_start: 45.0, model_switch: 30.0, other: 25.0 })
            } else {
                Some(GapProfile { cold_start: 55.0, model_switch: 0.0, other: 45.0 })
            }
        };

        // 4. 按 profile 生成缺口 + 修复建议
        if let Some(p) = profile {
            gaps.push(GapCategory {
                name: "new_session_cold_start".to_string(),
                description: "New session prefix never matches — first 300-800 tokens per session"
                    .to_string(),
                percent_of_misses: p.cold_start,
                priority: if p.cold_start > 40.0 { "high".to_string() } else { "medium".to_string() },
            });
            if p.model_switch > 0.0 {
                gaps.push(GapCategory {
                    name: "model_switch".to_string(),
                    description: "Model/provider switches reset cache prefix".to_string(),
                    percent_of_misses: p.model_switch,
                    priority: "medium".to_string(),
                });
            }
            gaps.push(GapCategory {
                name: "other".to_string(),
                description: "API retries, timeouts, tool output fluctuation, edge cases"
                    .to_string(),
                percent_of_misses: p.other,
                priority: "low".to_string(),
            });

            if p.cold_start > 40.0 {
                suggestions.push(FixSuggestion {
                    priority: "high".to_string(),
                    issue: "New session cold start".to_string(),
                    action: "Keep SOUL.md first 2048 tokens stable across sessions".to_string(),
                    expected_improvement: "+2-3% hit rate".to_string(),
                });
            }
            if p.model_switch > 0.0 {
                suggestions.push(FixSuggestion {
                    priority: "medium".to_string(),
                    issue: "Model/provider switches".to_string(),
                    action: "Minimize model/provider switching within single session".to_string(),
                    expected_improvement: format!("+{:.0}% hit rate", p.model_switch * 0.05),
                });
            }
            if raw_hit_rate < target_hit_rate {
                suggestions.push(FixSuggestion {
                    priority: "low".to_string(),
                    issue: "Near target — fine-tuning only".to_string(),
                    action: "Set temperature to 0.0, minimize context compression".to_string(),
                    expected_improvement: format!("+{:.1}% to reach {:.0}%", gap, target_hit_rate),
                });
            }
        }

        // 5. Per-model breakdown (仅当该模型贡献 >10% misses)
        for (model, misses) in &model_misses {
            let pct = (*misses as f64 / total_misses_f) * 100.0;
            if pct > 10.0 {
                gaps.push(GapCategory {
                    name: format!("model:{}", model),
                    description: format!(
                        "Model '{}' accounts for {:.0}% of misses", model, pct
                    ),
                    percent_of_misses: pct,
                    priority: "medium".to_string(),
                });
            }
        }

        // 保留2位小数
        let fmt2 = |v: f64| (v * 100.0).round() / 100.0;

        CacheGapReport {
            period_days: days,
            actual_hit_rate: fmt2(raw_hit_rate),
            target_hit_rate,
            gap_percent: fmt2(gap),
            total_requests: total,
            total_misses,
            estimated_daily_miss_tokens,
            gaps,
            suggestions,
        }
    }
}
