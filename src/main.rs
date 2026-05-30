mod cache;
mod calibration;
mod collector;
mod commands;
mod config;
mod container;
mod engine;
mod environment;
mod gpu;
mod helpers;
mod models;
mod multi_agent;
mod remote;
mod state_machine;
mod suggest;
mod thermal;
mod trends;

use calibration::algorithm::CalibrationEngine;
use calibration::csv_store::CsvStore;
#[allow(unused_imports)]
use calibration::CalibrationStore;
use clap::Parser;
use collector::registry::CollectorRegistry;
use engine::assessment::{AssessmentEngine, DeploymentRequest};
use state_machine::{StateMachine, StateMachineConfig, StateTransition};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
#[allow(unused_imports)]
use trends::{HistoryStore, TrendAnalyzer};

const DEFAULT_CONFIG_CONTENT: &str = r#"[model]
# Bytes per token for your model (default: 2048)
# Common values: 2048 (standard), 4096 (deepseek), 1536 (llama)
bytes_per_token = 2048

# Safety margin percentage (default: 30.0)
# Higher = more conservative context window estimate
margin = 30.0

[cache]
# 目标缓存命中率（百分比，默认 99.0）
target_hit_rate = 99.0

# 警告触发阈值（百分比，默认 95.0）
warn_threshold = 95.0

# 默认分析天数（配合 --analyze-cache-gaps 使用，默认 7）
analysis_days = 7
"#;

#[derive(Parser)]
#[command(
    name = "hawk-eye-mem",
    version = "0.6.0",
    about = "AI-Native memory monitoring"
)]
pub struct Cli {
    // === 原有参数 ===
    #[arg(long, conflicts_with = "metric")]
    json: bool,
    #[arg(long)]
    metric: Option<String>,
    #[arg(long)]
    config: Option<String>,
    #[arg(long)]
    interval: Option<u64>,
    #[arg(long)]
    count: Option<u64>,
    #[arg(long, conflicts_with_all = &["json", "metric"])]
    init_config: bool,

    // === V0.2 W2 部署评估参数 ===
    #[arg(long)]
    can_run: bool,
    #[arg(long, conflicts_with = "model_size")]
    model: Option<String>,
    #[arg(long)]
    model_size: Option<u64>,
    #[arg(long)]
    quantization: Option<String>,
    #[arg(long)]
    context: Option<u32>,
    #[arg(long, requires = "can_run")]
    compare: Option<String>,
    #[arg(long)]
    list_models: bool,

    // === V0.3 校准相关参数 ===
    /// 本次推理实际处理的 token 数（CR-01 MCP Tool 传入）
    #[arg(long)]
    tokens_processed: Option<u64>,

    /// 校准的模型名（启用校准模式）
    #[arg(long)]
    model_name: Option<String>,

    /// 查看校准统计信息（需指定 --model-name）
    #[arg(long, requires = "model_name")]
    calibration_stats: bool,

    /// 清空指定模型的校准数据（需指定 --model-name）
    #[arg(long, requires = "model_name")]
    reset_calibration: bool,

    // === V0.3 Phase 6 新增参数 ===
    /// 列出检测到的 GPU 及其采集方式（NVML/nvidia-smi/ROCm/Metal）
    #[arg(long)]
    gpu_list: bool,

    // === V0.4 W1-W2 环境指纹参数 ===
    /// 输出当前环境指纹 JSON
    #[arg(long)]
    env_fingerprint: bool,

    /// 重置环境指纹（需 --force 跳过确认）
    #[arg(long)]
    reset_environment: bool,

    /// 跳过确认（用于 --reset-environment 脚本模式）
    #[arg(long)]
    force: bool,

    // === V0.4 W2 CR-06: 告警模式 ===
    /// 告警模式：仅当压力 critical 时输出最小化 JSON 单行（pressure/available_mb/action）
    #[arg(long)]
    alert: bool,

    // === REQ-001: 物理AI · 并发度建议 ===
    /// 根据系统资源建议最佳并发数
    #[arg(long)]
    suggest_concurrency: bool,

    /// 每个子任务的内存预算（MB），配合 --suggest-concurrency 使用（默认 1024MB）
    #[arg(long, requires = "suggest_concurrency")]
    task_memory: Option<u64>,

    // === V0.4 W2-W3: 远程采集 HTTP 服务端 ===
    /// 启动远程采集 HTTP 服务端模式
    #[arg(long)]
    serve: bool,
    /// HTTP 服务端监听端口（默认 9240，仅 --serve 模式下有效）
    #[arg(long, default_value = "9240")]
    port: u16,

    // === V0.4.1: 数据记录参数 ===
    /// 采集当前系统状态并记录到趋势历史（需要 >=10 个点才能生成趋势报告）
    #[arg(long)]
    record: bool,
    // 复用已有的 --tokens-processed 参数（在 calibration 段已声明）
    // 配合 --record 使用时自动记录 token 数

    // === V0.4 趋势分析参数 ===
    /// 输出内存趋势分析报告
    #[arg(long)]
    trend: bool,
    /// 清空历史记录数据
    #[arg(long)]
    clear_history: bool,

    // === V0.5 缓存策略参数 ===
    /// 输出当前推荐的缓存策略 JSON
    #[arg(long)]
    cache_strategy: bool,
    /// 输出 24 小时缓存命中统计
    #[arg(long)]
    cache_stats: bool,
    /// 清空缓存统计数据
    #[arg(long)]
    reset_cache_stats: bool,
    /// 查看模型缓存兼容性（可选参数：model@provider，不传则列出所有Provider）
    #[arg(long = "model-compat", num_args = 0..=1, default_missing_value = "")]
    model_compat: Option<String>,

    // === V0.6 缓存差距分析 ===
    /// 分析缓存命中率差距，输出缺口分类+修复建议
    #[arg(long)]
    analyze_cache_gaps: bool,

    /// 分析天数（默认 7 天），配合 --analyze-cache-gaps 使用
    #[arg(long, default_value = "7")]
    days: u32,

    /// 目标命中率（默认 99.0%），配合 --analyze-cache-gaps 使用
    #[arg(long, default_value = "99.0")]
    target: f64,

    // === V0.6 心跳模式 ===
    /// 输出单行心跳 JSON（pressure/available_mb/action/timestamp）
    #[arg(long)]
    heartbeat: bool,
}

fn main() {
    let cli = Cli::parse();

    // === --serve：远程采集 HTTP 服务端模式 ===
    if cli.serve {
        commands::system::handle_serve(&cli);
        return;
    }

    // 首次运行检查
    let onboarded_path = get_onboarded_path();
    if !onboarded_path.exists() {
        print_disclaimer();
        print_quick_guide();
        if let Some(parent) = onboarded_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&onboarded_path, b"onboarded");
    }

    // 处理 --init-config
    if cli.init_config {
        let config_path = dirs_next::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".config/hawk-eye-mem/config.toml");
        if config_path.exists() {
            eprintln!("Config already exists at {}", config_path.display());
            std::process::exit(0);
        }
        if let Some(parent) = config_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&config_path, DEFAULT_CONFIG_CONTENT).unwrap_or_else(|e| {
            eprintln!("Failed to write config: {}", e);
            std::process::exit(1);
        });
        eprintln!("Default config generated at {}", config_path.display());
        std::process::exit(0);
    }

    // === 各命令模块调度 ===
    if cli.list_models {
        commands::model::handle_list_models();
        return;
    }
    if cli.cache_strategy {
        commands::cache::handle_cache_strategy(&cli);
        return;
    }
    if cli.cache_stats {
        commands::cache::handle_cache_stats();
        return;
    }
    if cli.reset_cache_stats {
        commands::cache::handle_reset_cache_stats();
        return;
    }
    if cli.model_compat.is_some() {
        commands::cache::handle_model_compat(cli.model_compat.as_deref());
        return;
    }
    if cli.analyze_cache_gaps {
        commands::cache::handle_analyze_cache_gaps(&cli);
        return;
    }
    if cli.heartbeat {
        commands::system::handle_heartbeat(&cli);
        return;
    }
    if cli.can_run {
        commands::model::handle_can_run(&cli);
        return;
    }
    if cli.calibration_stats {
        commands::model::handle_calibration_stats(&cli);
        return;
    }
    if cli.reset_calibration {
        commands::model::handle_reset_calibration(&cli);
        return;
    }
    if cli.gpu_list {
        commands::system::handle_gpu_list();
        return;
    }
    if cli.alert {
        commands::system::handle_alert_mode(&cli);
        return;
    }
    if cli.suggest_concurrency {
        commands::system::handle_suggest_concurrency(&cli);
        return;
    }

    // === 环境指纹（需要 fingerprint_store 供后续使用）===
    let fingerprint_store = environment::store::FingerprintStore::new();
    if cli.env_fingerprint {
        commands::system::handle_env_fingerprint(&cli);
        return;
    }
    if cli.reset_environment {
        commands::system::handle_reset_environment(&cli, &fingerprint_store);
        return;
    }
    if cli.alert {
        commands::system::handle_alert_mode(&cli);
        return;
    }
    if cli.clear_history {
        commands::system::handle_clear_history(&cli);
        return;
    }
    if cli.trend {
        commands::system::handle_trend(&cli);
        return;
    }
    if cli.record {
        commands::system::handle_record(&cli);
        return;
    }

    // === V0.3 校准引擎初始化 ===
    let calibration_path = get_calibration_path();
    let csv_store = CsvStore::new(calibration_path, 100);
    let mut calibration_engine = CalibrationEngine::new(csv_store);
    let model_name = cli.model.clone().unwrap_or_else(|| "default".to_string());

    // === 原有的内存监控逻辑 ===
    let count = cli.count.unwrap_or(1);
    let interval = cli.interval.unwrap_or(0);
    let infinite = count == 0 && interval > 0;
    let is_continuous = interval > 0 && count > 0;

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    let mut iter = 0u64;
    let mut previous_snapshot: Option<collector::ResourceSnapshot> = None;

    // T10: 状态机集成 — 仅在连续监控模式（--interval）下启用
    let _use_state_machine = is_continuous || infinite;
    let mut state_machine: Option<StateMachine> = if _use_state_machine {
        Some(StateMachine::new(StateMachineConfig::default()))
    } else {
        None
    };
    // 记录上一次状态机的 action（状态未变化时沿用）
    let mut last_sm_action: &'static str = "ok";

    // === V0.4 W1-W2: 环境指纹 — 每次运行自动保存并检测变更 ===
    let env_store = environment::store::FingerprintStore::new();
    let (current_fp, environment_change_report) = {
        // 快速采集一次以生成指纹（硬件级信息，与循环中的采集正交）
        let mut registry = CollectorRegistry::new();
        if let Ok(Some(cfg)) = config::AppConfig::load(cli.config.as_deref()) {
            if let Some(dirs) = cfg.directories {
                registry.set_directories(dirs.model_cache);
            }
        }
        let snap = registry.collect_all();
        let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".to_string());
        let platform = std::env::consts::OS;
        let cpu_cores = num_cpus::get() as u32;
        let total_mem = snap.memory.as_ref().map(|m| m.total_mb).unwrap_or(0);
        let gpu_names: Vec<String> = snap
            .gpu
            .as_ref()
            .map(|g| g.iter().map(|gpu| gpu.name.clone()).collect())
            .unwrap_or_default();
        let disk_total = snap.disk.as_ref().map(|d| d.total_mb).unwrap_or(0);
        let container = container::ContainerDetector::detect_runtime();

        let fp = environment::EnvironmentFingerprint::generate(
            &hostname, platform, cpu_cores, total_mem, gpu_names, disk_total, container,
        );

        // 加载旧指纹做变更检测
        let previous_fp = env_store.load_previous().ok().flatten();
        let report = previous_fp.as_ref().and_then(|prev| {
            let changes = fp.detect_changes(prev);
            if changes.is_empty() {
                return None;
            }
            let recommendation =
                environment::EnvironmentFingerprint::generate_recommendation(&changes);
            eprintln!("[hawk-eye-mem] Environment change detected:");
            for c in &changes {
                eprintln!(
                    "  • {}: {} → {} ({})",
                    c.resource, c.previous_label, c.current_label, c.direction
                );
            }
            if !recommendation.is_empty() {
                eprintln!("  💡 {}", recommendation);
            }
            Some(environment::EnvironmentChangeReport {
                detected: true,
                previous_fingerprint_id: Some(prev.id.clone()),
                changes,
                new_recommendation: Some(recommendation),
            })
        });

        (fp, report)
    };

    // 保存新指纹
    let _ = env_store.save(&current_fp);

    while running.load(Ordering::SeqCst) && (infinite || iter < count) {
        if iter > 0 && interval > 0 {
            let chunk = std::time::Duration::from_millis(100);
            let total = std::time::Duration::from_secs(interval);
            let mut slept = std::time::Duration::ZERO;
            while slept < total && running.load(Ordering::SeqCst) {
                std::thread::sleep(chunk);
                slept += chunk;
            }
            if !running.load(Ordering::SeqCst) {
                break;
            }
        }

        let mut registry = CollectorRegistry::new();
        if let Ok(Some(cfg)) = config::AppConfig::load(cli.config.as_deref()) {
            if let Some(dirs) = cfg.directories {
                registry.set_directories(dirs.model_cache);
            }
            if let Some(ma) = cfg.multi_agent {
                registry.set_extra_agent_processes(ma.extra_process_names);
                registry.set_agent_custom_names(ma.names);
            }
        }
        let snapshot = registry.collect_all();
        let metrics = snapshot
            .memory
            .as_ref()
            .expect("Memory collector must succeed");

        // T10: 状态机更新 — 每次采集后调用
        let sm_transition = if let Some(ref mut sm) = &mut state_machine {
            let trans = sm.update(metrics, std::time::Instant::now());
            if trans != StateTransition::None {
                last_sm_action = trans.action();
            }
            Some(trans)
        } else {
            None
        };

        // T5: 当有 tokens_processed 且存在前一次快照时，记录校准数据
        let mut recorded_calibration = false;
        if let Some(tok) = cli.tokens_processed {
            if let Some(ref before) = previous_snapshot {
                if calibration_engine
                    .record_inference_from_snapshots(before, &snapshot, tok, &model_name)
                    .unwrap_or(None)
                    .is_some()
                {
                    recorded_calibration = true;
                }
            }
        }

        if (is_continuous || infinite) && cli.metric.is_none() {
            let app_config = load_config(&cli);
            let result = calc_estimate(metrics, &app_config);
            let output = build_json_output(
                &snapshot,
                &result,
                cli.tokens_processed,
                recorded_calibration,
                &sm_transition,
                last_sm_action,
                &environment_change_report,
            );
            println!("{}", serde_json::to_string(&output).unwrap());
        } else if let Some(metric) = &cli.metric {
            print_metric(metrics, metric);
        } else {
            let app_config = load_config(&cli);
            let result = calc_estimate(metrics, &app_config);
            let output = build_json_output(
                &snapshot,
                &result,
                cli.tokens_processed,
                recorded_calibration,
                &sm_transition,
                last_sm_action,
                &environment_change_report,
            );
            helpers::print_json(&output);
        }

        // 保存当前快照作为下一次的前一次快照
        previous_snapshot = Some(snapshot);
        iter += 1;
    }

    if !running.load(Ordering::SeqCst) {
        eprintln!("Interrupted by user");
    }
}

// ============================================================================
// --can-run 模式
// ============================================================================

#[allow(dead_code)]
fn handle_can_run(cli: &Cli) {
    let mut registry = CollectorRegistry::new();
    if let Ok(Some(cfg)) = config::AppConfig::load(cli.config.as_deref()) {
        if let Some(dirs) = cfg.directories {
            registry.set_directories(dirs.model_cache);
        }
        if let Some(ma) = cfg.multi_agent {
            registry.set_extra_agent_processes(ma.extra_process_names);
            registry.set_agent_custom_names(ma.names);
        }
    }
    let snapshot = registry.collect_all();

    // --compare：多模型比较
    if let Some(ref compare) = cli.compare {
        let model_names: Vec<&str> = compare.split(',').map(|s| s.trim()).collect();
        if model_names.is_empty() || model_names.len() > 3 {
            eprintln!("--compare 需要 1-3 个逗号分隔的模型名");
            std::process::exit(1);
        }
        let mut results: Vec<engine::assessment::DeploymentAssessment> = Vec::new();
        for name in &model_names {
            let req = DeploymentRequest {
                model_name: Some(name.to_string()),
                quantization: cli.quantization.clone(),
                context_window: cli.context,
                ..Default::default()
            };
            let assessment = AssessmentEngine::assess(&req, &snapshot);
            results.push(assessment);
        }
        let recommended_idx = find_recommended(&results);
        print_compare_output(&results, recommended_idx, cli.json);
        return;
    }

    // 单模型评估
    let req = DeploymentRequest {
        model_name: cli.model.clone(),
        model_size_b: cli.model_size,
        quantization: cli.quantization.clone(),
        context_window: cli.context,
    };
    let assessment = AssessmentEngine::assess(&req, &snapshot);
    helpers::print_json(&assessment);
}

// 增加 --source 过滤支持（已移除，token audit 通过 MCP 工具调用）
/// 在比较结果中找到推荐项
#[allow(dead_code)]
fn find_recommended(results: &[engine::assessment::DeploymentAssessment]) -> Option<usize> {
    use engine::assessment::Verdict;

    // 优先选择 Feasible
    let feasible: Vec<(usize, &engine::assessment::DeploymentAssessment)> = results
        .iter()
        .enumerate()
        .filter(|(_, a)| a.verdict == Verdict::Feasible)
        .collect();

    if feasible.is_empty() {
        // 如果没有 Feasible，选约束最少的
        return results
            .iter()
            .enumerate()
            .min_by_key(|(_, a)| a.constraints.len())
            .map(|(i, _)| i);
    }

    if feasible.len() == 1 {
        return feasible.into_iter().next().map(|(i, _)| i);
    }

    // 多个 Feasible，选约束最少的
    feasible
        .into_iter()
        .min_by_key(|(_, a)| a.constraints.len())
        .map(|(i, _)| i)
}

/// 输出比较结果
#[allow(dead_code)]
fn print_compare_output(
    results: &[engine::assessment::DeploymentAssessment],
    recommended_idx: Option<usize>,
    json_output: bool,
) {
    if json_output {
        // JSON 模式：保持原有格式
        let compare_result = serde_json::json!({
            "comparison": results,
            "recommended_index": recommended_idx,
        });
        helpers::print_json(&compare_result);
        return;
    }

    // 人类可读模式：彩色表格
    const GREEN: &str = "\x1b[32m";
    const YELLOW: &str = "\x1b[33m";
    const RED: &str = "\x1b[31m";
    const BOLD: &str = "\x1b[1m";
    const RESET: &str = "\x1b[0m";

    // 表头
    println!(
        "{BOLD}{:<20} {:<14} {:<40} {:<10}{RESET}",
        "模型名称", "判定结果", "约束摘要", "安全方案数"
    );
    println!("{BOLD}{:-<90}{RESET}", "");

    for (i, a) in results.iter().enumerate() {
        let color = match a.verdict {
            engine::assessment::Verdict::Feasible => GREEN,
            engine::assessment::Verdict::FeasibleWithCaveats => YELLOW,
            engine::assessment::Verdict::Infeasible => RED,
        };

        let verdict_str = match a.verdict {
            engine::assessment::Verdict::Feasible => "✅ 可行",
            engine::assessment::Verdict::FeasibleWithCaveats => "⚠️ 有条件",
            engine::assessment::Verdict::Infeasible => "❌ 不可行",
        };

        let star = if Some(i) == recommended_idx {
            " ⭐"
        } else {
            "   "
        };

        // 约束摘要：每个资源一行简要信息
        let constraints_summary = if a.constraints.is_empty() {
            "—".to_string()
        } else {
            a.constraints
                .iter()
                .map(|c| format!("{}: 缺{}MB", c.resource, c.gap_mb))
                .collect::<Vec<_>>()
                .join("; ")
        };

        // 从 request 中提取模型名
        let model_name = a
            .request
            .get("model_name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        println!(
            "{color}{:<20} {:<14} {:<40} {:<4}{}{RESET}",
            model_name,
            verdict_str,
            truncate_str(&constraints_summary, 38),
            a.safe_options.len(),
            star,
        );
    }
}

/// 截断字符串到指定宽度（中文字符计2宽度的近似处理）
#[allow(dead_code)]
fn truncate_str(s: &str, max_width: usize) -> String {
    if s.len() <= max_width {
        return s.to_string();
    }
    let mut result = String::new();
    let mut width = 0usize;
    for ch in s.chars() {
        let w = if ch.is_ascii() { 1 } else { 2 };
        if width + w > max_width.saturating_sub(3) {
            break;
        }
        result.push(ch);
        width += w;
    }
    result.push_str("...");
    result
}

// ============================================================================
// --alert 告警模式 (CR-06)
// ============================================================================

/// --alert 模式：仅当压力 critical 时输出最小化 JSON 单行
#[allow(dead_code)]
fn handle_alert_mode(cli: &Cli) {
    let mut registry = CollectorRegistry::new();
    if let Ok(Some(cfg)) = config::AppConfig::load(cli.config.as_deref()) {
        if let Some(dirs) = cfg.directories {
            registry.set_directories(dirs.model_cache);
        }
    }
    let snapshot = registry.collect_all();

    if let Some(ref metrics) = snapshot.memory {
        let (pressure, _, _) = crate::engine::guidance::GuidanceGenerator::classify(
            metrics.available_mb,
            metrics.used_percent,
        );

        if pressure == "critical" || pressure == "high" {
            let action = match pressure {
                "critical" => "abort_safely",
                "high" => "reduce_context",
                _ => "ok",
            };
            let alert = serde_json::json!({
                "pressure": pressure,
                "available_mb": metrics.available_mb,
                "action": action,
            });
            println!("{}", serde_json::to_string(&alert).unwrap());
        }
        // 非 critical 时无输出（CR-06: 只输出 critical 时的单行）
    }
}

// ============================================================================
// --list-models 模式
// ============================================================================
/// 模型列表表格
#[allow(dead_code)]
fn print_model_table() {
    use crate::engine::assessment::{AssessmentEngine, DeploymentRequest};
    let models = models::ModelLibrary::all();

    // 收集一次系统资源快照
    let mut registry = CollectorRegistry::new();
    if let Ok(Some(cfg)) = config::AppConfig::load(None) {
        if let Some(dirs) = cfg.directories {
            registry.set_directories(dirs.model_cache);
        }
    }
    let snapshot = registry.collect_all();

    // ANSI 颜色
    const GREEN: &str = "\x1b[32m";
    const YELLOW: &str = "\x1b[33m";
    const RED: &str = "\x1b[31m";
    const BOLD: &str = "\x1b[1m";
    const RESET: &str = "\x1b[0m";

    // 标题行
    println!(
        "{BOLD}{:<20} {:<10} {:<6} {:<28} {:<14} {:<36} {:<10}{RESET}",
        "模型名称", "参数量", "BPT", "量化", "上下文", "来源", "更新"
    );
    println!("{BOLD}{:-<128}{RESET}", "");

    for m in models {
        let req = DeploymentRequest {
            model_name: Some(m.name.clone()),
            quantization: None,
            context_window: Some(m.max_context),
            ..Default::default()
        };
        let assessment = AssessmentEngine::assess(&req, &snapshot);

        let color = match assessment.verdict {
            engine::assessment::Verdict::Feasible => GREEN,
            engine::assessment::Verdict::FeasibleWithCaveats => YELLOW,
            engine::assessment::Verdict::Infeasible => RED,
        };

        let size_gb = m.size_b as f64 / 1e9;
        let context_str = if m.min_context == m.max_context {
            format!("{}", m.min_context)
        } else {
            format!("{}-{}", m.min_context, m.max_context)
        };
        println!(
            "{color}{:<20} {:<10.1} {:<6} {:<28} {:<14} {:<36} {:<10}{RESET}",
            m.name,
            size_gb,
            m.bytes_per_token,
            m.quantizations.join(", "),
            context_str,
            m.source,
            m.last_updated,
        );
    }
}

// ============================================================================
// 原有辅助函数
// ============================================================================

fn get_onboarded_path() -> PathBuf {
    let home = dirs_next::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join(".config/hawk-eye-mem/.onboarded")
}

fn print_disclaimer() {
    eprintln!("================================================================================");
    eprintln!("  HawkEye Mem (秋毫mem) v0.2.0");
    eprintln!("  No warranty. Use at your own risk.");
    eprintln!("  This software is provided 'as is', without any express or implied warranty.");
    eprintln!("================================================================================");
}

fn print_quick_guide() {
    eprintln!();
    eprintln!("  Quick Start:");
    eprintln!("    hawk-eye-mem --json             # Full JSON output with metrics + guidance");
    eprintln!("    hawk-eye-mem --metric available_mb  # Single value output for scripts");
    eprintln!("    hawk-eye-mem --help             # See all options");
    eprintln!("    hawk-eye-mem --config <path>    # Load custom model config");
    eprintln!("    hawk-eye-mem --list-models      # List all supported models");
    eprintln!("    hawk-eye-mem --can-run --model llama3-8b  # Check deployment feasibility");
    eprintln!();
    eprintln!("  Configure model parameters (optional):");
    eprintln!("    hawk-eye-mem --init-config      # Generate default config file");
    eprintln!("================================================================================");
}

/// 从配置中读取 history.retention_days（默认 30）
pub fn load_history_retention(cli: &Cli) -> Option<u64> {
    match config::AppConfig::load(cli.config.as_deref()) {
        Ok(Some(cfg)) => cfg.history.and_then(|h| h.retention_days),
        _ => None,
    }
}

fn load_config(cli: &Cli) -> Option<engine::ModelConfig> {
    match config::AppConfig::load(cli.config.as_deref()) {
        Ok(Some(cfg)) => cfg.model.map(|m| engine::ModelConfig {
            bytes_per_token: m.bytes_per_token.unwrap_or(2048),
            margin: m.margin.unwrap_or(30.0),
        }),
        Ok(None) => None,
        Err(e) => {
            eprintln!("Warning: config error: {}", e);
            None
        }
    }
}

fn calc_estimate(
    metrics: &collector::MemoryMetrics,
    model_config: &Option<engine::ModelConfig>,
) -> engine::EstimationResult {
    engine::EstimationEngine::estimate(metrics.available_mb, model_config)
}

/// 获取校准数据存储目录：~/.config/hawk-eye-mem/calibration/
fn get_calibration_path() -> PathBuf {
    dirs_next::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config/hawk-eye-mem/calibration")
}

fn build_json_output(
    snapshot: &collector::ResourceSnapshot,
    result: &engine::EstimationResult,
    tokens_processed: Option<u64>,
    recorded: bool,
    sm_transition: &Option<StateTransition>,
    sm_action: &'static str,
    environment_changes: &Option<environment::EnvironmentChangeReport>,
) -> serde_json::Value {
    let metrics = snapshot
        .memory
        .as_ref()
        .expect("Memory collector must succeed");
    let guidance = engine::guidance::GuidanceGenerator::generate(
        metrics.available_mb,
        metrics.used_percent,
        result.estimated_tokens,
        &result.confidence.to_string(),
    );
    let mut guidance_value = serde_json::to_value(&guidance).unwrap();

    // T10: 状态机模式 — 用状态机的 action 覆盖即时判定的 action
    if sm_transition.is_some() {
        if let Some(obj) = guidance_value.as_object_mut() {
            obj.insert(
                "action".to_string(),
                serde_json::Value::String(sm_action.to_string()),
            );
            // 状态未变化时补充说明
            if sm_action == "no_change" {
                obj.insert(
                    "reason".to_string(),
                    serde_json::Value::String(format!(
                        "State unchanged ({}): {}MB available, {}% used. {}",
                        metrics.pressure,
                        metrics.available_mb,
                        metrics.used_percent,
                        "Monitoring continues — no transition needed.",
                    )),
                );
            }
        }
    }

    guidance_value["_note"] = serde_json::Value::String(
        "The following are recommendations only. The ultimate decision-making authority resides with the user."
            .to_string(),
    );

    let mut system = serde_json::json!({
        "total_mb": metrics.total_mb,
        "used_mb": metrics.used_mb,
        "available_mb": metrics.available_mb,
        "used_percent": metrics.used_percent,
    });

    if let Some(ref disk) = snapshot.disk {
        system["disk"] = serde_json::to_value(disk).unwrap();
    }
    if let Some(ref cpu) = snapshot.cpu {
        system["cpu"] = serde_json::to_value(cpu).unwrap();
    }
    if let Some(ref gpu) = snapshot.gpu {
        system["gpu"] = serde_json::to_value(gpu).unwrap();
    }
    if let Some(ref thermal) = snapshot.thermal {
        system["thermal"] = serde_json::to_value(thermal).unwrap();
    }
    if let Some(ref agents) = snapshot.agents {
        system["agents"] = serde_json::to_value(agents).unwrap();
    }

    let mut output = serde_json::json!({
        "timestamp": snapshot.timestamp,
        "collection_duration_ms": snapshot.collection_duration_ms,
        "system": system,
        "agent_guidance": guidance_value,
    });

    // T10: 状态机模式 — 输出当前状态机信息
    if let Some(transition) = sm_transition {
        output["machine_state"] = serde_json::json!({
            "state": sm_action,
            "transition": format!("{:?}", transition),
            "note": "状态机仅在 --interval 连续监控模式下生效。",
        });
    }

    // T5: MCP Tool 传入 tokens_processed 时，输出校准数据点状态
    if let Some(tok) = tokens_processed {
        if recorded {
            output["_calibration"] = serde_json::json!({
                "tokens_processed": tok,
                "status": "recorded",
                "note": "Calibration data point recorded. See --calibration-stats for status."
            });
        } else {
            output["_calibration"] = serde_json::json!({
                "tokens_processed": tok,
                "status": "skipped",
                "reason": "需要两次连续采集才能计算 delta。请重复此命令或在 --interval 循环中使用。",
                "note": "Calibration requires two consecutive snapshots. Run twice or use --interval mode."
            });
        }
    }

    // V0.4 W1-W2: 输出环境变更报告
    if let Some(ref report) = environment_changes {
        if report.detected {
            output["environment_change"] = serde_json::to_value(report).unwrap();
        }
    }

    output
}

fn print_metric(metrics: &collector::MemoryMetrics, name: &str) {
    match name {
        "total_mb" => println!("{}", metrics.total_mb),
        "used_mb" => println!("{}", metrics.used_mb),
        "available_mb" => println!("{}", metrics.available_mb),
        "used_percent" => println!("{:.1}", metrics.used_percent),
        "pressure" => println!("{}", metrics.pressure),
        _ => {
            eprintln!("Unknown metric: {}", name);
            std::process::exit(1);
        }
    }
}

// ============================================================================
// 单元测试 — CLI 集成测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // UT-CL-001: --list-models 输出包含模型列表
    #[test]
    fn test_ut_cl_001_list_models() {
        let models = models::ModelLibrary::all();
        assert!(models.len() >= 8, "应该有至少 8 个模型");
        // 验证特定模型存在
        assert!(models::ModelLibrary::find("llama3-8b").is_some());
        assert!(models::ModelLibrary::find("qwen2-7b").is_some());
        assert!(models::ModelLibrary::find("phi-3-mini").is_some());
    }

    // UT-CL-002: 单模型评估构造
    #[test]
    fn test_ut_cl_002_single_assessment() {
        let req = DeploymentRequest {
            model_name: Some("qwen2-7b".to_string()),
            quantization: Some("Q4_K_M".to_string()),
            context_window: Some(4096),
            ..Default::default()
        };
        // 只需验证 request 构造正确，序列化后包含模型名
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["model_name"], "qwen2-7b");
        assert_eq!(json["quantization"], "Q4_K_M");
        assert_eq!(json["context_window"], 4096);
    }

    // UT-CL-003: --compare 解析
    #[test]
    fn test_ut_cl_003_compare_split() {
        let input = "llama3-8b,qwen2-7b,mistral-7b";
        let names: Vec<&str> = input.split(',').map(|s| s.trim()).collect();
        assert_eq!(names.len(), 3);
        assert_eq!(names[0], "llama3-8b");
        assert_eq!(names[1], "qwen2-7b");
        assert_eq!(names[2], "mistral-7b");
    }

    // UT-CL-004: --compare 最多 3 个模型
    #[test]
    fn test_ut_cl_004_compare_max_3() {
        let input = "a,b,c,d";
        let names: Vec<&str> = input.split(',').map(|s| s.trim()).collect();
        assert!(names.len() > 3, "应超过 3 个");
    }

    // UT-CL-005: find_recommended 选择可行项
    #[test]
    fn test_ut_cl_005_find_recommended() {
        use engine::assessment::{Constraint, DeploymentAssessment, Verdict};

        let make = |v: Verdict, n: usize| DeploymentAssessment {
            request: serde_json::json!({}),
            verdict: v,
            constraints: (0..n)
                .map(|i| Constraint {
                    resource: format!("r{}", i),
                    required_mb: 100,
                    available_mb: 50,
                    gap_mb: 50,
                    severity: "warning".to_string(),
                    suggestion: "test".to_string(),
                })
                .collect(),
            safe_options: vec![],
        };

        let results = vec![
            make(Verdict::Infeasible, 2),
            make(Verdict::FeasibleWithCaveats, 1),
            make(Verdict::Feasible, 0),
            make(Verdict::Feasible, 1),
        ];

        let idx = find_recommended(&results);
        assert_eq!(idx, Some(2), "应选择约束最少的 Feasible (索引 2)");
    }

    // UT-CL-006: find_recommended 无可选项时选约束最少
    #[test]
    fn test_ut_cl_006_recommended_fallback() {
        use engine::assessment::{Constraint, DeploymentAssessment, Verdict};

        let make = |v: Verdict, n: usize| DeploymentAssessment {
            request: serde_json::json!({}),
            verdict: v,
            constraints: (0..n)
                .map(|i| Constraint {
                    resource: format!("r{}", i),
                    required_mb: 100,
                    available_mb: 50,
                    gap_mb: 50,
                    severity: "warning".to_string(),
                    suggestion: "test".to_string(),
                })
                .collect(),
            safe_options: vec![],
        };

        let results = vec![
            make(Verdict::Infeasible, 3),
            make(Verdict::Infeasible, 1),
            make(Verdict::Infeasible, 2),
        ];

        let idx = find_recommended(&results);
        assert_eq!(idx, Some(1), "应选择约束最少的 (索引 1)");
    }
}

// ============================================================================
// V0.5 --model-compat：读取 Provider 缓存兼容矩阵
// ============================================================================

use serde_json::Value;

/// Parse "model@provider" format into (provider, model) tuple.
/// Returns (provider_str, model_name).
fn parse_model_spec(spec: &str) -> (String, String) {
    if let Some(at_pos) = spec.rfind('@') {
        let model = &spec[..at_pos];
        let provider = &spec[at_pos + 1..];
        (provider.to_string(), model.to_string())
    } else {
        // No @ — treat entire string as model name
        (String::new(), spec.to_string())
    }
}

/// Check model cache compatibility.
/// If `model_spec` is Some("model@provider"), checks that specific model.
/// If `model_spec` is Some("model") without @, tries to find the model in all providers.
/// If `model_spec` is None, lists all providers.
pub fn check_model_compatibility(model_spec: Option<&str>) -> Value {
    // Read the provider_cache_compat.json
    let compat_path = dirs_next::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".hermes/skills/hermes-cache-strategy/provider_cache_compat.json");

    let compat: Value = match std::fs::read_to_string(&compat_path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
            serde_json::json!({
                "error": format!("Failed to parse provider_cache_compat.json: {}", e),
                "path": compat_path.to_string_lossy().to_string()
            })
        }),
        Err(e) => {
            return serde_json::json!({
                "error": format!("Failed to read provider_cache_compat.json: {}", e),
                "path": compat_path.to_string_lossy().to_string()
            });
        }
    };

    let model_spec = match model_spec {
        Some(s) if s.trim().is_empty() => {
            // No argument: list all providers (same as None)
            let providers = compat.get("providers").and_then(|p| p.as_object());
            let mut list = Vec::new();
            if let Some(providers) = providers {
                for (name, info) in providers {
                    let supports_prompt_caching = info
                        .get("supports_prompt_caching")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let models = info
                        .get("models")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|m| m.as_str().map(String::from))
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    list.push(serde_json::json!({
                        "name": name,
                        "supports_prompt_caching": supports_prompt_caching,
                        "models": models,
                    }));
                }
            }
            return serde_json::json!({
                "version": compat.get("version"),
                "providers": list,
            });
        }
        Some(s) => s,
        None => {
            // No argument: list all providers
            let providers = compat.get("providers").and_then(|p| p.as_object());
            let mut list = Vec::new();
            if let Some(providers) = providers {
                for (name, info) in providers {
                    let supports_prompt_caching = info
                        .get("supports_prompt_caching")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let models = info
                        .get("models")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|m| m.as_str().map(String::from))
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    list.push(serde_json::json!({
                        "name": name,
                        "supports_prompt_caching": supports_prompt_caching,
                        "models": models,
                    }));
                }
            }
            return serde_json::json!({
                "version": compat.get("version"),
                "providers": list,
            });
        }
    };

    // Has model spec — check specific model
    let (provider_from_spec, model_name) = parse_model_spec(model_spec);

    let providers = match compat.get("providers").and_then(|p| p.as_object()) {
        Some(p) => p,
        None => {
            return serde_json::json!({
                "model": model_spec,
                "error": "No providers found in compatibility matrix"
            });
        }
    };

    if provider_from_spec.is_empty() {
        // No provider specified — search all providers for this model
        let mut found = Vec::new();
        for (prov_name, info) in providers {
            if let Some(models) = info.get("models").and_then(|v| v.as_array()) {
                if models
                    .iter()
                    .any(|m| m.as_str().is_some_and(|m_name| m_name == model_name))
                    || models.iter().any(|m| m.as_str() == Some("*"))
                {
                    let supports_prompt_caching = info
                        .get("supports_prompt_caching")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    found.push(serde_json::json!({
                        "provider": prov_name,
                        "supports_prompt_caching": supports_prompt_caching,
                    }));
                }
            }
        }
        if found.is_empty() {
            return serde_json::json!({
                "model": model_spec,
                "found": false,
                "note": format!("Model '{}' not found in any provider's compatibility list", model_name),
            });
        }
        return serde_json::json!({
            "model": model_spec,
            "found": true,
            "providers": found,
        });
    }

    // Specific provider requested
    let provider_info = match providers.get(&provider_from_spec) {
        Some(info) => info,
        None => {
            return serde_json::json!({
                "model": model_spec,
                "provider": provider_from_spec,
                "error": format!("Provider '{}' not found in compatibility matrix", provider_from_spec),
            });
        }
    };

    let supports_prompt_caching = provider_info
        .get("supports_prompt_caching")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let models = provider_info
        .get("models")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.as_str().map(String::from))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let model_in_list = models.iter().any(|m| m == "*" || m == &model_name);
    let note = if !model_in_list {
        Some(format!(
            "Model '{}' not in provider '{}' known model list, using provider-level defaults",
            model_name, provider_from_spec
        ))
    } else if !supports_prompt_caching {
        Some(format!("{} 不支持 prompt caching", provider_from_spec))
    } else {
        None
    };

    serde_json::json!({
        "model": model_spec,
        "provider": provider_from_spec,
        "supports_prompt_caching": supports_prompt_caching,
        "supported_models": models,
        "note": note,
    })
}
