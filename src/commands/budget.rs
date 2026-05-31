// ============================================================================
// src/commands/budget.rs — Token 预算管家 CLI 命令
// ============================================================================

use crate::budget::collector::TokenCollector;
use crate::budget::analyzer::WasteAnalyzer;
use crate::budget::cost::CostCalculator;
use crate::budget::executor::OptimizationExecutor;
use crate::budget::{ActionType, Severity};
use crate::Cli;

pub fn handle_token_budget(cli: &Cli) {
    let subcommand = cli.token_budget.as_deref().unwrap_or("help");
    match subcommand {
        "status" => handle_status(),
        "waste" => handle_waste(),
        "suggest" => handle_suggest(cli),
        "apply" => handle_apply(cli),
        "help" | _ => print_help(),
    }
}

fn handle_status() {
    println!("\n╔══════════════════════════════════════╗");
    println!("║   💸 Token 预算管家 · 消耗总览      ║");
    println!("╚══════════════════════════════════════╝");

    let result = TokenCollector::collect();
    if result.records.is_empty() {
        println!("\n  ℹ️  {}\n", result.message);
        return;
    }

    let summary = crate::budget::collector::aggregate_tokens(&result.records);
    let calculator = CostCalculator::new();
    let total_cost = calculator.calculate_batch_cost(&result.records);

    println!("\n  📊 数据概览");
    println!("  ─────────────────────────────────────");
    println!("  Token 总量:   {:>12}", summary.total_input_tokens + summary.total_output_tokens);
    println!("  API 调用数:  {:>12}", summary.total_api_calls);
    println!("  缓存命中率:  {:>11.1}%", summary.cache_hit_rate);
    println!("  冷启动平均:  {:>12} tokens/次", summary.first_call_tokens_avg);
    println!("  数据源:      {:>12}", result.sources_used.join(", "));
    println!("\n  💰 费用估算");
    println!("  ─────────────────────────────────────");
    println!("  预计总费用:  ${:>10.4}", total_cost);

    if !summary.by_model.is_empty() {
        println!("\n  📦 按模型分布");
        println!("  ─────────────────────────────────────");
        for model in &summary.by_model {
            println!("  {:<30} {:>8} tokens  ${:.4}", model.model,
                model.input_tokens + model.output_tokens, model.cost_usd);
        }
    }
    println!();
}

fn handle_waste() {
    let result = TokenCollector::collect();
    if result.records.is_empty() {
        println!("\n  ℹ️  {}", result.message);
        return;
    }

    let summary = crate::budget::collector::aggregate_tokens(&result.records);
    let calculator = CostCalculator::new();
    let _total_cost = calculator.calculate_batch_cost(&result.records);
    let skills_count = estimate_skills_count();
    let memory_size_tokens = estimate_memory_size();
    let mcp_count = estimate_mcp_count();
    let report = WasteAnalyzer::analyze(&result.records, &summary, skills_count, memory_size_tokens, mcp_count);

    println!("\n╔══════════════════════════════════════╗");
    println!("║   🔍 Token 预算管家 · 浪费分析      ║");
    println!("╚══════════════════════════════════════╝");
    println!("\n  {}", report.message);

    if !report.wastes.is_empty() {
        println!("\n  📋 检测到的浪费场景:");
        println!("  ─────────────────────────────────────");
        for waste in &report.wastes {
            let severity_icon = match waste.severity {
                Severity::High => "🔴",
                Severity::Medium => "🟡",
                Severity::Low => "🟢",
            };
            println!("  {} {:20} {:>10} tokens", severity_icon, waste.description, waste.estimated_waste_tokens);
        }
    }
    println!();
}

fn handle_suggest(cli: &Cli) {
    let result = TokenCollector::collect();
    if result.records.is_empty() {
        println!("\n  ℹ️  {}", result.message);
        return;
    }

    let summary = crate::budget::collector::aggregate_tokens(&result.records);
    let _calculator = CostCalculator::new();
    let skills_count = estimate_skills_count();
    let memory_size_tokens = estimate_memory_size();
    let mcp_count = estimate_mcp_count();
    let report = WasteAnalyzer::analyze(&result.records, &summary, skills_count, memory_size_tokens, mcp_count);

    if cli.json {
        let json_output = serde_json::json!({
            "suggestions": report.suggestions.iter().map(|s| {
                serde_json::json!({
                    "id": s.id,
                    "waste_type": format!("{:?}", s.waste_type),
                    "severity": format!("{:?}", s.severity),
                    "description": s.description,
                    "expected_savings_tokens": s.expected_savings_tokens,
                    "expected_savings_cost": s.expected_savings_cost,
                    "action_detail": s.action_detail,
                    "risk": s.risk,
                })
            }).collect::<Vec<_>>(),
            "total_waste_tokens": report.total_waste_tokens,
            "message": report.message,
        });
        println!("{}", serde_json::to_string_pretty(&json_output).unwrap());
        return;
    }

    println!("\n╔══════════════════════════════════════╗");
    println!("║   💡 Token 预算管家 · 优化建议      ║");
    println!("╚══════════════════════════════════════╝");
    println!("\n  {}", report.message);

    if report.suggestions.is_empty() {
        println!("\n  暂无需要执行的优化~\n");
        return;
    }

    for sug in &report.suggestions {
        let sev_icon = match sug.severity {
            Severity::High => "🔴",
            Severity::Medium => "🟡",
            Severity::Low => "🟢",
        };
        let action_icon = match sug.action_type {
            ActionType::DisableSkill => "🎛️",
            ActionType::AdjustCache => "🧠",
            ActionType::CompressMemory => "📦",
            ActionType::RemoveMcpServer => "🔌",
            ActionType::AdjustConfig => "⚙️",
        };
        println!("\n  {}. {} {} {}", sug.id, sev_icon, action_icon, sug.description);
        println!("     📝 {}", sug.action_detail);
        println!("     💰 可节省 ~{} tokens（约 ${:.4}）", sug.expected_savings_tokens, sug.expected_savings_cost);
        println!("     ⚠️  风险: {}", sug.risk);
        println!("     💡 执行: hawk-eye-mem --token-budget apply {}", sug.id);
    }
    println!();
}

fn handle_apply(cli: &Cli) {
    let apply_args = cli.token_budget_args.as_deref().unwrap_or("");
    let parts: Vec<&str> = apply_args.split_whitespace().collect();
    let is_rollback = parts.first().map(|s| *s == "--rollback").unwrap_or(false);

    if is_rollback {
        println!("\n  🔄 正在回滚到最近备份...");
        let result = OptimizationExecutor::rollback();
        if result.success {
            println!("  ✅ 回滚成功！配置已恢复到备份版本");
            if let Some(path) = result.backup_path {
                println!("  📂 备份位置: {}", path);
            }
        } else {
            println!("  ❌ 回滚失败: {}", result.error.unwrap_or_default());
        }
        println!();
        return;
    }

    let result = TokenCollector::collect();
    let summary = crate::budget::collector::aggregate_tokens(&result.records);
    let _calculator = CostCalculator::new();
    let skills_count = estimate_skills_count();
    let memory_size_tokens = estimate_memory_size();
    let mcp_count = estimate_mcp_count();
    let report = WasteAnalyzer::analyze(&result.records, &summary, skills_count, memory_size_tokens, mcp_count);

    let suggestion_id: u32 = match parts.first().and_then(|s| s.parse().ok()) {
        Some(id) => id,
        None => {
            eprintln!("❌ 请指定建议 ID，如: hawk-eye-mem --token-budget \"apply 1\"");
            eprintln!("   查看可用建议: hawk-eye-mem --token-budget suggest");
            return;
        }
    };

    let suggestion = match report.suggestions.iter().find(|s| s.id == suggestion_id) {
        Some(s) => s,
        None => {
            eprintln!("❌ 无效的建议 ID: {}（有效范围 1-{}）", suggestion_id, report.suggestions.len());
            return;
        }
    };

    if cli.dry_run {
        println!("\n  🔍 [DRY RUN] 预览优化效果");
        println!("  ─────────────────────────────────────");
        println!("  🎯 建议: {}", suggestion.description);
        println!("  📝 操作: {}", suggestion.action_detail);
        println!("  💰 预期节省: ~{} tokens/轮", suggestion.expected_savings_tokens);
        println!("  ⚠️  风险: {}", suggestion.risk);
        println!("\n  这是干运行，配置不会被修改。");
        println!("  确认执行请加 --force 参数:\n    hawk-eye-mem --token-budget \"apply {}\" --force", suggestion_id);
    } else {
        println!("\n  ⚙️  正在执行优化...");
        let result = OptimizationExecutor::execute(suggestion, false);
        if result.success {
            println!("  ✅ 优化执行成功！");
            println!("  📝 操作: {}", result.action);
            if let Some(path) = result.backup_path {
                println!("  📂 配置已备份至: {}", path);
            }
        } else {
            println!("  ❌ 优化执行失败: {}", result.error.unwrap_or_default());
        }
    }
    println!();
}

fn print_help() {
    println!("\n╔══════════════════════════════════════╗");
    println!("║   💸 Token 预算管家                  ║");
    println!("╚══════════════════════════════════════╝");
    println!("\n  用法: hawk-eye-mem --token-budget <子命令> [参数]");
    println!("\n  子命令:");
    println!("    status     Token 消耗总览");
    println!("    waste      浪费分析");
    println!("    suggest    优化建议");
    println!("    apply ID   执行优化（配合 --dry-run / --force）");
    println!("\n  示例:");
    println!("    hawk-eye-mem --token-budget status");
    println!("    hawk-eye-mem --token-budget suggest");
    println!("    hawk-eye-mem --token-budget \"apply 1\" --dry-run");
    println!("    hawk-eye-mem --token-budget \"apply 1\" --force");
    println!("    hawk-eye-mem --token-budget \"apply --rollback\"");
    println!();
}

fn estimate_skills_count() -> u64 {
    let skills_dir = dirs_next::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join(".hermes/skills");
    if skills_dir.exists() {
        match std::fs::read_dir(&skills_dir) {
            Ok(entries) => entries.count() as u64,
            Err(_) => 20,
        }
    } else {
        20
    }
}

fn estimate_memory_size() -> u64 {
    let mem_dir = dirs_next::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join(".hermes/memories");
    let mut total_size: u64 = 0;
    if mem_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&mem_dir) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if meta.is_file() {
                        total_size += meta.len();
                    }
                }
            }
        }
    }
    total_size / 4
}

fn estimate_mcp_count() -> u64 {
    let config_path = dirs_next::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join(".hermes/config.yaml");
    if config_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            let count = content.lines()
                .filter(|l| l.trim().starts_with("- command") || l.trim().starts_with("command:"))
                .count() as u64;
            if count > 0 { return count; }
        }
    }
    3
}
