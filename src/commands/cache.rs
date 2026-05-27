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
            println!("   预取: {}", if strategy.prefetch_enabled { "✅" } else { "❌" });
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
