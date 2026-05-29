use crate::calibration;
use crate::calibration::algorithm::CalibrationEngine;
use crate::calibration::csv_store::CsvStore;
use crate::calibration::CalibrationStore;
use crate::collector::registry::CollectorRegistry;
use crate::config;
use crate::engine::assessment::{
    AssessmentEngine, DeploymentAssessment, DeploymentRequest, Verdict,
};
use crate::helpers;
use crate::Cli;
use std::path::PathBuf;

// ============================================================================
// --can-run 模式
// ============================================================================

pub fn handle_can_run(cli: &Cli) {
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
        let mut results: Vec<DeploymentAssessment> = Vec::new();
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

/// 在比较结果中找到推荐项
fn find_recommended(results: &[DeploymentAssessment]) -> Option<usize> {
    // 优先选择 Feasible
    let feasible: Vec<(usize, &DeploymentAssessment)> = results
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
    results: &[DeploymentAssessment],
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
            Verdict::Feasible => GREEN,
            Verdict::FeasibleWithCaveats => YELLOW,
            Verdict::Infeasible => RED,
        };

        let verdict_str = match a.verdict {
            Verdict::Feasible => "✅ 可行",
            Verdict::FeasibleWithCaveats => "⚠️ 有条件",
            Verdict::Infeasible => "❌ 不可行",
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
// --calibration-stats：查看校准统计
// ============================================================================

pub fn handle_calibration_stats(cli: &Cli) {
    if let Some(ref model) = cli.model_name {
        let path = cal_base_path();
        let store = CsvStore::new(path.join("calibration"), 100);
        let engine = CalibrationEngine::new(store);
        let stats = engine.stats(model).unwrap_or_else(|e| {
            eprintln!("Failed to get calibration stats: {}", e);
            std::process::exit(1);
        });
        let params = engine.get_corrected_params(model).unwrap_or(None);

        println!("校准状态 — 模型: \"{}\"", model);
        println!("─────────────────────────────────────────");
        println!("{}", stats.stage);
        if let Some(ref p) = params {
            println!("加权平均:  {} bytes/token", p.avg_bytes_per_token);
            println!("标准差:    {}", p.calibration.std_dev);
            println!("趋势:      {}", p.calibration.trend);
            println!("安全边际:  {}%", p.safety_margin);
            println!("置信度:    {:?}", p.confidence);
        } else if stats.sample_count > 0 {
            println!(
                "样本不足:  还需 {} 次才能启动校准算法",
                10 - stats.sample_count.min(10)
            );
        }
    }
}

// ============================================================================
// --reset-calibration：清空校准数据
// ============================================================================

pub fn handle_reset_calibration(cli: &Cli) {
    if let Some(ref model) = cli.model_name {
        let path = cal_base_path();
        let store = CsvStore::new(path.join("calibration"), 100);
        let model_hash = calibration::hash_model_name(model).unwrap_or_else(|e| {
            eprintln!("Failed to hash model name: {}", e);
            std::process::exit(1);
        });
        store.clear_model(&model_hash).unwrap_or_else(|e| {
            eprintln!("Failed to clear calibration data: {}", e);
            std::process::exit(1);
        });
        println!("已清空模型 \"{}\" 的校准数据", model);
    }
}

// ============================================================================
// --list-models 模式
// ============================================================================

pub fn handle_list_models() {
    use crate::models::ModelLibrary;
    let models = ModelLibrary::all();

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
            Verdict::Feasible => GREEN,
            Verdict::FeasibleWithCaveats => YELLOW,
            Verdict::Infeasible => RED,
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
// 内部辅助函数
// ============================================================================

fn cal_base_path() -> PathBuf {
    dirs_next::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config/hawk-eye-mem")
}
