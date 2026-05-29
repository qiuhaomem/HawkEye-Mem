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
        let actual_hit_rate = if total > 0 {
            (total_hits as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        let gap_percent = (target_hit_rate - actual_hit_rate).max(0.0);

        // Estimate daily miss tokens (rough: each miss ~500 tokens wasted)
        let days_f = days.max(1) as f64;
        let estimated_daily_miss_tokens = ((total_misses as f64 * 500.0) / days_f) as u64;

        // Gap classification based on patterns in the data
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

        // Analyze miss patterns by model
        let mut model_misses: HashMap<String, u64> = HashMap::new();
        for r in &reports {
            *model_misses.entry(r.model_name.clone()).or_insert(0) += r.miss_count;
        }

        // Classify gaps based on hit rate ranges
        if actual_hit_rate < 90.0 {
            gaps.push(GapCategory {
                name: "low_hit_rate".to_string(),
                description: format!(
                    "Hit rate {:.1}% is below 90% — major optimization needed",
                    actual_hit_rate
                ),
                percent_of_misses: 100.0,
                priority: "high".to_string(),
            });
            suggestions.push(FixSuggestion {
                priority: "high".to_string(),
                issue: "Hit rate critically low".to_string(),
                action: "Enable aggressive cache mode: hermes config set cache.mode aggressive"
                    .to_string(),
                expected_improvement: "Target: 95%+".to_string(),
            });
        } else if actual_hit_rate < 95.0 {
            // Medium gap — likely new session cold starts + tool output fluctuation
            let cold_start_pct = 40.0;
            let tool_output_pct = 35.0;
            let compress_pct = 25.0;
            gaps.push(GapCategory {
                name: "new_session_cold_start".to_string(),
                description: "New session prefix never matches — first 300-800 tokens per session"
                    .to_string(),
                percent_of_misses: cold_start_pct,
                priority: "high".to_string(),
            });
            gaps.push(GapCategory {
                name: "tool_output_fluctuation".to_string(),
                description: "Terminal output changes break context continuity".to_string(),
                percent_of_misses: tool_output_pct,
                priority: "medium".to_string(),
            });
            gaps.push(GapCategory {
                name: "context_compression".to_string(),
                description: "Post-compression summary differs from previous context".to_string(),
                percent_of_misses: compress_pct,
                priority: "medium".to_string(),
            });
            suggestions.push(FixSuggestion {
                priority: "high".to_string(),
                issue: "New session cold start".to_string(),
                action: "Keep SOUL.md first 2048 tokens stable across sessions".to_string(),
                expected_improvement: "+2-3% hit rate".to_string(),
            });
            suggestions.push(FixSuggestion {
                priority: "medium".to_string(),
                issue: "Tool output fluctuation".to_string(),
                action: "Reduce dynamic tool loading/unloading frequency".to_string(),
                expected_improvement: "+1-2% hit rate".to_string(),
            });
        } else if actual_hit_rate < target_hit_rate {
            // Small gap — fine-tuning needed
            let remaining_misses = total_misses as f64;
            gaps.push(GapCategory {
                name: "new_session_cold_start".to_string(),
                description: "Residual cold start misses — unavoidable per-session prefix"
                    .to_string(),
                percent_of_misses: 50.0,
                priority: "low".to_string(),
            });
            gaps.push(GapCategory {
                name: "model_switch".to_string(),
                description: "Model/provider switches reset cache prefix".to_string(),
                percent_of_misses: 30.0,
                priority: "low".to_string(),
            });
            gaps.push(GapCategory {
                name: "other".to_string(),
                description: "API retries, timeouts, edge cases".to_string(),
                percent_of_misses: 20.0,
                priority: "low".to_string(),
            });
            suggestions.push(FixSuggestion {
                priority: "low".to_string(),
                issue: "Near target — fine-tuning only".to_string(),
                action: "Set temperature to 0.0, minimize context compression".to_string(),
                expected_improvement: format!(
                    "+{:.1}% to reach {:.0}%",
                    gap_percent, target_hit_rate
                ),
            });
        } else {
            gaps.push(GapCategory {
                name: "at_target".to_string(),
                description: format!(
                    "Hit rate {:.1}% meets or exceeds {:.0}% target",
                    actual_hit_rate, target_hit_rate
                ),
                percent_of_misses: 0.0,
                priority: "ok".to_string(),
            });
        }

        // Per-model breakdown
        for (model, misses) in &model_misses {
            if *misses > 0 {
                let pct = (*misses as f64 / total_misses as f64) * 100.0;
                if pct > 10.0 {
                    gaps.push(GapCategory {
                        name: format!("model:{}", model),
                        description: format!(
                            "Model '{}' accounts for {:.0}% of misses",
                            model, pct
                        ),
                        percent_of_misses: pct,
                        priority: "medium".to_string(),
                    });
                }
            }
        }

        CacheGapReport {
            period_days: days,
            actual_hit_rate: (actual_hit_rate * 100.0).round() / 100.0,
            target_hit_rate,
            gap_percent: (gap_percent * 100.0).round() / 100.0,
            total_requests: total,
            total_misses,
            estimated_daily_miss_tokens,
            gaps,
            suggestions,
        }
    }
}
