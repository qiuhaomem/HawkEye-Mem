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
// src/budget/collector.rs — Token 数据采集器
// ============================================================================
// 双源采集：Hermes state.db（SQLite）+ agent.log（JSON Lines）
// ============================================================================

use crate::budget::{TokenRecord, TokenSource};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 采集结果
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CollectionResult {
    pub records: Vec<TokenRecord>,
    pub sources_used: Vec<String>,
    pub fallback: bool,
    pub message: String,
}

/// Token 数据采集器
pub struct TokenCollector;

impl TokenCollector {
    /// 采集所有可用数据源的 Token 记录
    pub fn collect() -> CollectionResult {
        let mut result = CollectionResult::default();
        let mut all_records: Vec<TokenRecord> = Vec::new();

        // 1. 尝试 state.db
        let hermes_home = dirs_next::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".hermes");
        let state_db_path = hermes_home.join("state.db");

        if state_db_path.exists() {
            match Self::collect_from_state_db(&state_db_path) {
                Ok(records) => {
                    all_records.extend(records);
                    result.sources_used.push("state.db".to_string());
                }
                Err(e) => {
                    result.fallback = true;
                    result.message = format!("state.db 降级: {}", e);
                }
            }
        }

        // 2. 尝试 agent.log + agent.log.1
        let log_path = hermes_home.join("logs/agent.log");
        let log_path_1 = hermes_home.join("logs/agent.log.1");

        if log_path.exists() || log_path_1.exists() {
            let log_records = Self::collect_from_agent_log(&log_path, &log_path_1);
            if !log_records.is_empty() {
                let existing_keys: std::collections::HashSet<String> = all_records
                    .iter()
                    .map(|r| format!("{}_{}_{}", r.timestamp, r.model, r.input_tokens))
                    .collect();
                for rec in log_records {
                    let key = format!("{}_{}_{}", rec.timestamp, rec.model, rec.input_tokens);
                    if !existing_keys.contains(&key) {
                        all_records.push(rec);
                    }
                }
                result.sources_used.push("agent.log".to_string());
            }
        }

        if all_records.is_empty() && !result.sources_used.is_empty() {
            result.message = "数据源已读取但无 Token 记录，请先使用 Hermes Agent 进行对话".to_string();
        } else if all_records.is_empty() {
            result.message = "未检测到 Hermes Agent 数据（无 state.db 或 agent.log）".to_string();
        }

        result.records = all_records;
        result
    }

    /// 从 state.db 采集
    fn collect_from_state_db(db_path: &PathBuf) -> Result<Vec<TokenRecord>, String> {
        let _ = db_path;
        #[cfg(feature = "budget")]
        {
            match Self::query_state_db(db_path) {
                Ok(records) => return Ok(records),
                Err(e) => return Err(e),
            }
        }
        #[cfg(not(feature = "budget"))]
        {
            let _ = db_path;
            Err("budget feature 未启用，需编译时开启 --features budget".to_string())
        }
    }

    #[cfg(feature = "budget")]
    fn query_state_db(db_path: &PathBuf) -> Result<Vec<TokenRecord>, String> {
        let conn = rusqlite::Connection::open(db_path)
            .map_err(|e| format!("打开 state.db 失败: {}", e))?;

        let has_sessions: bool = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='sessions'")
            .and_then(|mut stmt| stmt.exists([]))
            .unwrap_or(false);

        if !has_sessions {
            return Err("state.db 中无 sessions 表".to_string());
        }

        let cols: Vec<String> = conn
            .prepare("PRAGMA table_info(sessions)")
            .and_then(|mut stmt| {
                stmt.query_map([], |row| row.get::<_, String>(1))
                    .map(|rows| rows.filter_map(|r| r.ok()).collect())
            })
            .unwrap_or_default();

        let has_input_tokens = cols.contains(&"input_tokens".to_string());
        let has_cache = cols.contains(&"cache_read_tokens".to_string());

        if !has_input_tokens {
            return Err("state.db schema 不兼容，缺少 input_tokens 列".to_string());
        }

        let query = if has_cache {
            "SELECT created_at, model, provider, input_tokens, output_tokens, \
             cache_read_tokens FROM sessions WHERE created_at >= datetime('now', '-7 days') \
             ORDER BY created_at DESC LIMIT 1000"
        } else {
            "SELECT created_at, model, provider, input_tokens, output_tokens, \
             0 FROM sessions WHERE created_at >= datetime('now', '-7 days') \
             ORDER BY created_at DESC LIMIT 1000"
        };

        let mut stmt = conn
            .prepare(query)
            .map_err(|e| format!("SQL 准备失败: {}", e))?;

        let records: Vec<TokenRecord> = stmt
            .query_map([], |row| {
                let created_at: String = row.get(0)?;
                let model: String = row.get(1)?;
                let provider: String = row.get(2)?;
                let input_tokens: u64 = row.get::<_, i64>(3).unwrap_or(0) as u64;
                let output_tokens: u64 = row.get::<_, i64>(4).unwrap_or(0) as u64;
                let cache_hit: u64 = row.get::<_, i64>(5).unwrap_or(0) as u64;
                Ok(TokenRecord {
                    timestamp: created_at,
                    model,
                    provider,
                    input_tokens,
                    output_tokens,
                    cache_hit_tokens: cache_hit,
                    latency_sec: 0.0,
                    is_first_call: false,
                    session_id: None,
                    source: TokenSource::StateDb,
                })
            })
            .map_err(|e| format!("SQL 查询失败: {}", e))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(records)
    }

    /// 从 agent.log 采集
    fn collect_from_agent_log(log_path: &PathBuf, log_path_1: &PathBuf) -> Vec<TokenRecord> {
        let mut records = Vec::new();
        for path in [log_path, log_path_1] {
            if !path.exists() { continue; }
            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            for line in content.lines() {
                if let Some(rec) = Self::parse_log_line(line) {
                    records.push(rec);
                }
            }
        }
        records
    }

    /// 解析单行 agent.log
    fn parse_log_line(line: &str) -> Option<TokenRecord> {
        if !line.contains("API call #") {
            return None;
        }

        let model = extract_field(line, "model=")?;
        let in_val: u64 = extract_field(line, " in=")?.parse().ok()?;
        let out_val: u64 = extract_field(line, " out=")?.parse().ok()?;
        let cache_part = extract_field(line, " cache=")?;
        let cache_hit: u64 = cache_part.split('/').next()?.parse().ok()?;
        let latency_val: f64 = extract_field(line, " latency=")?.trim_end_matches('s').parse().ok()?;

        Some(TokenRecord {
            timestamp: String::new(),
            model: model.to_string(),
            provider: String::new(),
            input_tokens: in_val,
            output_tokens: out_val,
            cache_hit_tokens: cache_hit,
            latency_sec: latency_val,
            is_first_call: line.contains("API call #1"),
            session_id: None,
            source: TokenSource::AgentLog,
        })
    }
}

/// 从字符串中提取字段值
fn extract_field<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let start = line.find(prefix)?;
    let value_start = start + prefix.len();
    let remaining = &line[value_start..];
    let end = remaining.find(' ').unwrap_or(remaining.len());
    Some(&remaining[..end])
}

/// 聚合 Token 记录
pub fn aggregate_tokens(records: &[TokenRecord]) -> crate::budget::TokenSummary {
    let mut total_in = 0u64;
    let mut total_out = 0u64;
    let mut total_cache = 0u64;
    let total_calls = records.len() as u64;
    let mut first_call_tokens_sum = 0u64;
    let mut first_call_count = 0u64;
    let mut by_model: std::collections::HashMap<String, (u64, u64, u64, u64)> =
        std::collections::HashMap::new();

    for rec in records {
        total_in += rec.input_tokens;
        total_out += rec.output_tokens;
        total_cache += rec.cache_hit_tokens;

        if rec.is_first_call {
            first_call_tokens_sum += rec.input_tokens;
            first_call_count += 1;
        }

        let key = format!("{}@{}", rec.model, rec.provider);
        let entry = by_model.entry(key).or_insert((0, 0, 0, 0));
        entry.0 += rec.input_tokens;
        entry.1 += rec.output_tokens;
        entry.2 += rec.cache_hit_tokens;
        entry.3 += 1;
    }

    let total_all = total_in + total_out + total_cache;
    let cache_hit_rate = if total_all > 0 {
        total_cache as f64 / (total_in + total_cache) as f64 * 100.0
    } else {
        0.0
    };

    let cold_start_ratio = if total_calls > 0 && first_call_count > 0 {
        first_call_tokens_sum as f64 / total_in as f64 * 100.0
    } else {
        0.0
    };

    let model_breakdown: Vec<crate::budget::ModelTokenBreakdown> = by_model
        .into_iter()
        .map(|(key, (in_t, out_t, cache_t, calls))| {
            let parts: Vec<&str> = key.split('@').collect();
            let model = parts.first().unwrap_or(&"unknown").to_string();
            let provider = parts.get(1).unwrap_or(&"unknown").to_string();
            crate::budget::ModelTokenBreakdown {
                model,
                provider,
                input_tokens: in_t,
                output_tokens: out_t,
                cache_hit_tokens: cache_t,
                api_calls: calls,
                cost_usd: 0.0,
            }
        })
        .collect();

    crate::budget::TokenSummary {
        period_hours: 168,
        total_input_tokens: total_in,
        total_output_tokens: total_out,
        total_cache_hit_tokens: total_cache,
        total_api_calls: total_calls,
        cache_hit_rate,
        estimated_cost_usd: 0.0,
        by_model: model_breakdown,
        cold_start_ratio,
        first_call_tokens_avg: if first_call_count > 0 {
            first_call_tokens_sum / first_call_count
        } else {
            0
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_log_line_valid() {
        let line = "2026-05-31 01:03:14,874 INFO [session] API call #1: model=deepseek-v4-flash provider=deepseek in=20852 out=265 total=21117 latency=6.8s cache=2304/20852";
        let rec = TokenCollector::parse_log_line(line);
        assert!(rec.is_some());
        let rec = rec.unwrap();
        assert_eq!(rec.model, "deepseek-v4-flash");
        assert_eq!(rec.input_tokens, 20852);
        assert_eq!(rec.output_tokens, 265);
        assert_eq!(rec.cache_hit_tokens, 2304);
        assert!(rec.is_first_call);
    }

    #[test]
    fn test_parse_log_line_non_api_call() {
        let line = "2026-05-31 01:00:18,962 INFO run_agent: OpenAI client closed";
        assert!(TokenCollector::parse_log_line(line).is_none());
    }

    #[test]
    fn test_extract_field() {
        assert_eq!(extract_field("model=abc rest", "model="), Some("abc"));
        assert_eq!(extract_field("no match", "xyz="), None);
    }

    #[test]
    fn test_aggregate_empty() {
        let summary = aggregate_tokens(&[]);
        assert_eq!(summary.total_api_calls, 0);
        assert_eq!(summary.total_input_tokens, 0);
    }

    #[test]
    fn test_aggregate_single() {
        let records = vec![TokenRecord {
            timestamp: "2026-05-31".into(),
            model: "test-model".into(),
            provider: "test-provider".into(),
            input_tokens: 100,
            output_tokens: 50,
            cache_hit_tokens: 30,
            latency_sec: 1.0,
            is_first_call: true,
            session_id: None,
            source: TokenSource::StateDb,
        }];
        let summary = aggregate_tokens(&records);
        assert_eq!(summary.total_api_calls, 1);
        assert_eq!(summary.total_input_tokens, 100);
        assert_eq!(summary.total_output_tokens, 50);
        assert!(summary.cache_hit_rate > 0.0);
    }
}
