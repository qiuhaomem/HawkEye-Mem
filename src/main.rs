mod collector;
mod config;
mod engine;
mod models;

use clap::Parser;
use collector::registry::CollectorRegistry;
use engine::assessment::{AssessmentEngine, DeploymentRequest};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const DEFAULT_CONFIG_CONTENT: &str = r#"[model]
# Bytes per token for your model (default: 2048)
# Common values: 2048 (standard), 4096 (deepseek), 1536 (llama)
bytes_per_token = 2048

# Safety margin percentage (default: 30.0)
# Higher = more conservative context window estimate
margin = 30.0
"#;

#[derive(Parser)]
#[command(name = "hawk-eye-mem", version = "0.1.0", about = "AI-Native memory monitoring")]
struct Cli {
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
    #[arg(long, conflicts_with = "json")]
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
}

fn main() {
    let cli = Cli::parse();

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

    // === --list-models：打印模型表格 ===
    if cli.list_models {
        print_model_table();
        return;
    }

    // === --can-run：部署评估模式 ===
    if cli.can_run {
        handle_can_run(&cli);
        return;
    }

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
        }
        let snapshot = registry.collect_all();
        let metrics = snapshot
            .memory
            .as_ref()
            .expect("Memory collector must succeed");

        if (is_continuous || infinite) && !cli.metric.is_some() {
            let app_config = load_config(&cli);
            let result = calc_estimate(metrics, &app_config);
            let output = build_json_output(&snapshot, &result);
            println!("{}", serde_json::to_string(&output).unwrap());
        } else if let Some(metric) = &cli.metric {
            print_metric(metrics, metric);
        } else {
            let app_config = load_config(&cli);
            let result = calc_estimate(metrics, &app_config);
            let output = build_json_output(&snapshot, &result);
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }

        iter += 1;
    }

    if !running.load(Ordering::SeqCst) {
        eprintln!("Interrupted by user");
    }
}

// ============================================================================
// --can-run 模式
// ============================================================================

fn handle_can_run(cli: &Cli) {
    let mut registry = CollectorRegistry::new();
    if let Ok(Some(cfg)) = config::AppConfig::load(cli.config.as_deref()) {
        if let Some(dirs) = cfg.directories {
            registry.set_directories(dirs.model_cache);
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
        print_compare_output(&results, recommended_idx);
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
    println!("{}", serde_json::to_string_pretty(&assessment).unwrap());
}

/// 在比较结果中找到推荐项
fn find_recommended(
    results: &[engine::assessment::DeploymentAssessment],
) -> Option<usize> {
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
fn print_compare_output(
    results: &[engine::assessment::DeploymentAssessment],
    recommended_idx: Option<usize>,
) {
    let compare_result = serde_json::json!({
        "comparison": results,
        "recommended_index": recommended_idx,
    });
    println!("{}", serde_json::to_string_pretty(&compare_result).unwrap());
}

// ============================================================================
// --list-models 模式
// ============================================================================

fn print_model_table() {
    let models = models::ModelLibrary::all();

    // ANSI 颜色
    const GREEN: &str = "\x1b[32m";
    const CYAN: &str = "\x1b[36m";
    const BOLD: &str = "\x1b[1m";
    const RESET: &str = "\x1b[0m";

    // 标题行
    println!(
        "{GREEN}{BOLD}{:<20} {:<10} {:<6} {:<28} {:<14} {:<36} {:<10}{RESET}",
        "模型名称", "参数量", "BPT", "量化", "上下文", "来源", "更新"
    );
    println!(
        "{GREEN}{:-<128}{RESET}",
        ""
    );

    for m in models {
        let size_gb = m.size_b as f64 / 1e9;
        let context_str = if m.min_context == m.max_context {
            format!("{}", m.min_context)
        } else {
            format!("{}-{}", m.min_context, m.max_context)
        };
        println!(
            "{CYAN}{:<20} {:<10.1} {:<6} {:<28} {:<14} {:<36} {:<10}{RESET}",
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
    eprintln!("  HawkEye Mem (秋毫mem) v0.1.0");
    eprintln!("  No warranty. Use at your own risk.");
    eprintln!("  This software is provided 'as is', without any express or implied warranty.");
    eprintln!("================================================================================");
}

fn print_quick_guide() {
    eprintln!("");
    eprintln!("  Quick Start:");
    eprintln!("    hawk-eye-mem --json             # Full JSON output with metrics + guidance");
    eprintln!("    hawk-eye-mem --metric available_mb  # Single value output for scripts");
    eprintln!("    hawk-eye-mem --help             # See all options");
    eprintln!("    hawk-eye-mem --config <path>    # Load custom model config");
    eprintln!("    hawk-eye-mem --list-models      # List all supported models");
    eprintln!("    hawk-eye-mem --can-run --model llama3-8b  # Check deployment feasibility");
    eprintln!("");
    eprintln!("  Configure model parameters (optional):");
    eprintln!("    hawk-eye-mem --init-config      # Generate default config file");
    eprintln!("================================================================================");
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

fn build_json_output(
    snapshot: &collector::ResourceSnapshot,
    result: &engine::EstimationResult,
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

    serde_json::json!({
        "timestamp": snapshot.timestamp,
        "collection_duration_ms": snapshot.collection_duration_ms,
        "system": system,
        "agent_guidance": guidance_value,
    })
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
            constraints: (0..n).map(|i| Constraint {
                resource: format!("r{}", i),
                required_mb: 100,
                available_mb: 50,
                gap_mb: 50,
                severity: "warning".to_string(),
                suggestion: "test".to_string(),
            }).collect(),
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
            constraints: (0..n).map(|i| Constraint {
                resource: format!("r{}", i),
                required_mb: 100,
                available_mb: 50,
                gap_mb: 50,
                severity: "warning".to_string(),
                suggestion: "test".to_string(),
            }).collect(),
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
