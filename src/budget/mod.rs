// ============================================================================
// src/budget/mod.rs — Token 预算管家 模块导出
// ============================================================================
// V0.6.1 Token预算管家：监控Token消耗 → 识别浪费 → 给出建议 → 一键执行
// ============================================================================

pub mod collector;
pub mod analyzer;
pub mod cost;
pub mod executor;

use serde::{Deserialize, Serialize};

// ============================================================================
// 核心数据类型
// ============================================================================

/// 单次 API 调用的 Token 记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenRecord {
    pub timestamp: String,
    pub model: String,
    pub provider: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_hit_tokens: u64,
    pub latency_sec: f64,
    pub is_first_call: bool,
    pub session_id: Option<String>,
    pub source: TokenSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TokenSource {
    StateDb,
    AgentLog,
}

/// 聚合后的 Token 摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSummary {
    pub period_hours: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_hit_tokens: u64,
    pub total_api_calls: u64,
    pub cache_hit_rate: f64,
    pub estimated_cost_usd: f64,
    pub by_model: Vec<ModelTokenBreakdown>,
    pub cold_start_ratio: f64,
    pub first_call_tokens_avg: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelTokenBreakdown {
    pub model: String,
    pub provider: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_hit_tokens: u64,
    pub api_calls: u64,
    pub cost_usd: f64,
}

/// 浪费场景
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasteEntry {
    pub id: String,
    pub waste_type: WasteType,
    pub severity: Severity,
    pub estimated_waste_tokens: u64,
    pub estimated_waste_cost: f64,
    pub description: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WasteType {
    ColdStart,       // 冷启动过大
    LowCacheHit,     // 缓存命中率低
    SkillBloat,      // 技能冗余
    MemoryBloat,     // Memory 臃肿
    McpRedundancy,   // MCP 冗余
    ContextWaste,    // 上下文窗口浪费
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Severity {
    High,
    Medium,
    Low,
}

/// 优化建议
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub id: u32,
    pub waste_type: WasteType,
    pub severity: Severity,
    pub description: String,
    pub expected_savings_tokens: u64,
    pub expected_savings_cost: f64,
    pub action_type: ActionType,
    pub action_detail: String,
    pub risk: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ActionType {
    DisableSkill,
    AdjustCache,
    CompressMemory,
    RemoveMcpServer,
    AdjustConfig,
}

/// 优化执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub action: String,
    pub backup_path: Option<String>,
    pub dry_run: bool,
    pub diff: Option<String>,
    pub error: Option<String>,
}
