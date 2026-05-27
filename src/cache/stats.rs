use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
}
