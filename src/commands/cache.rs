use crate::cache;
use crate::helpers;
use crate::Cli;
use std::path::PathBuf;

/// 处理 --cache-strategy
pub fn handle_cache_strategy(cli: &Cli) {
    let registry = crate::collector::registry::CollectorRegistry::new();
    let snapshot = registry.collect_all();
    if let Some(ref mem) = snapshot.memory {
        let pressure = cache::MemoryPressure::from(mem);
        let strategy = cache::CacheAdvisor::recommend(&pressure);
        if cli.json {
            helpers::print_json(&strategy);
        } else {
            println!("📊 缓存策略: {}", strategy.mode);
            println!("   TTL: {}s", strategy.ttl_seconds);
            println!("   最大缓存: {}MB", strategy.max_cache_mb);
            println!(
                "   预取: {}",
                if strategy.prefetch_enabled {
                    "✅"
                } else {
                    "❌"
                }
            );
            println!("   原因: {}", strategy.reason);
            println!("   协议版本: v{}", strategy.protocol_version);
        }
    } else {
        eprintln!("无法获取内存信息");
    }
}

/// 处理 --cache-stats
pub fn handle_cache_stats() {
    let stats_path = dirs_next::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config/hawk-eye-mem/cache_stats.jsonl");
    let store = cache::CacheStatsStore::new(stats_path);
    let collector = cache::CacheStatsCollector::new(store);
    let stats = collector.stats_24h();
    helpers::print_json(&stats);
}

/// 处理 --reset-cache-stats
pub fn handle_reset_cache_stats() {
    let stats_path = dirs_next::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config/hawk-eye-mem/cache_stats.jsonl");
    match std::fs::write(&stats_path, "") {
        Ok(_) => println!("Cache stats have been reset."),
        Err(e) => {
            eprintln!("Failed to reset cache stats: {}", e);
            std::process::exit(1);
        }
    }
}

/// 处理 --model-compat
pub fn handle_model_compat(model_compat: Option<&str>) {
    let result = crate::check_model_compatibility(model_compat);
    helpers::print_json(&result);
}

/// 处理 --analyze-cache-gaps
pub fn handle_analyze_cache_gaps(cli: &crate::Cli) {
    let stats_path = dirs_next::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config/hawk-eye-mem/cache_stats.jsonl");
    let store = cache::CacheStatsStore::new(stats_path);
    let collector = cache::CacheStatsCollector::new(store);
    let report = collector.analyze_gaps(cli.days, cli.target);

    if cli.json {
        helpers::print_json(&report);
        return;
    }

    // Human-readable output
    println!("═══════════════════════════════════════");
    println!("  缓存差距分析报告 · 过去 {} 天", report.period_days);
    println!("═══════════════════════════════════════");
    println!("  实际命中率:  {:.1}%", report.actual_hit_rate);
    println!("  目标命中率:  {:.0}%", report.target_hit_rate);
    println!("  差距:        {:.1}%", report.gap_percent);
    println!("  总请求:      {}", report.total_requests);
    println!("  总miss:      {}", report.total_misses);
    println!(
        "  日均miss:    ~{} tokens",
        report.estimated_daily_miss_tokens
    );
    println!();

    if !report.gaps.is_empty() {
        println!("  📊 缺口分类:");
        for gap in &report.gaps {
            let icon = match gap.priority.as_str() {
                "high" => "🔴",
                "medium" => "🟡",
                "low" => "🟢",
                "ok" => "✅",
                _ => "ℹ️",
            };
            println!(
                "    {} {} ({:.0}%) — {}",
                icon, gap.name, gap.percent_of_misses, gap.description
            );
        }
        println!();
    }

    if !report.suggestions.is_empty() {
        println!("  💡 修复建议:");
        for (i, s) in report.suggestions.iter().enumerate() {
            println!("    {}. [{}] {}", i + 1, s.priority.to_uppercase(), s.issue);
            println!("       → {}", s.action);
            println!("       预期: {}", s.expected_improvement);
        }
    }
    println!("═══════════════════════════════════════");
}
