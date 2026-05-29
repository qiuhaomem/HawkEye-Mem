// ============================================================================
// src/commands/system.rs — 系统命令（GPU/环境/趋势/报警/服务端）
// ============================================================================

use crate::collector::registry::CollectorRegistry;
use crate::collector::{self};
use crate::config;
use crate::environment;
use crate::gpu;
use crate::helpers;
use crate::remote;
use crate::suggest;
use crate::trends::{HistoryPoint, HistoryStore, TrendAnalyzer};

use crate::Cli;

// ============================================================================
// --gpu-list：列出检测到的 GPU
// ============================================================================

pub fn handle_gpu_list() {
    use collector::ResourceCollector;
    let gpu_collector = gpu::GpuCollector;
    match gpu_collector.collect() {
        Ok(collector::CollectorOutput::Gpu(gpus)) => {
            println!("\n  GPU List:");
            println!("  {:-<60}", "");
            for gpu in &gpus {
                println!(
                    "  {:<30} | {}MB/{}MB | {}",
                    gpu.name, gpu.vram_used_mb, gpu.vram_total_mb, gpu.backend
                );
            }
            println!();
        }
        Err(e) => {
            eprintln!("  No GPU detected: {}", e);
        }
        _ => {
            eprintln!("  Unexpected output from GPU collector");
        }
    }
}

// ============================================================================
// --alert：告警模式 (CR-06)
// ============================================================================

pub fn handle_alert_mode(cli: &Cli) {
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
    }
}

// ============================================================================
// --env-fingerprint：输出环境指纹
// ============================================================================

pub fn handle_env_fingerprint(_cli: &Cli) {
    let fingerprint_store = environment::store::FingerprintStore::new();
    match fingerprint_store.load_current() {
        Ok(Some(fp)) => {
            helpers::print_json(&fp);
        }
        Ok(None) => {
            eprintln!("No environment fingerprint found. Run 'hawk-eye-mem' without --env-fingerprint to generate one.");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("Failed to read environment fingerprint: {}", e);
            std::process::exit(1);
        }
    }
}

// ============================================================================
// --reset-environment：重置环境指纹
// ============================================================================

pub fn handle_reset_environment(
    cli: &Cli,
    fingerprint_store: &environment::store::FingerprintStore,
) {
    if !cli.force {
        eprint!("Reset environment fingerprint? This will remove all stored fingerprints. [y/N]: ");
        use std::io::Write;
        std::io::stdout().flush().ok();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        if input.trim().to_lowercase() != "y" {
            eprintln!("Cancelled.");
            std::process::exit(0);
        }
    }
    match fingerprint_store.reset() {
        Ok(_) => {
            println!("Environment fingerprint has been reset.");
        }
        Err(e) => {
            eprintln!("Failed to reset environment fingerprint: {}", e);
            std::process::exit(1);
        }
    }
}

// ============================================================================
// --clear-history：清空历史记录
// ============================================================================

pub fn handle_clear_history(cli: &Cli) {
    let retention_days = crate::load_history_retention(cli).unwrap_or(30);
    let store = HistoryStore::new(retention_days);
    match store.clear() {
        Ok(_) => {
            println!("History data cleared.");
        }
        Err(e) => {
            eprintln!("Failed to clear history: {}", e);
            std::process::exit(1);
        }
    }
}

// ============================================================================
// --trend：输出趋势分析报告
// ============================================================================

pub fn handle_trend(cli: &Cli) {
    let retention_days = crate::load_history_retention(cli).unwrap_or(30);
    let store = HistoryStore::new(retention_days);
    let points = match store.read_all() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to read history: {}", e);
            std::process::exit(1);
        }
    };

    match TrendAnalyzer::analyze(&points) {
        Some(report) => {
            helpers::print_json(&report);
        }
        None => {
            eprintln!(
                "Insufficient data for trend analysis (need >= 10 points, found {})",
                points.len()
            );
            std::process::exit(0);
        }
    }
}

// ============================================================================
// --record：采集并记录趋势数据
// ============================================================================

pub fn handle_record(cli: &Cli) {
    let mut registry = CollectorRegistry::new();
    if let Ok(Some(cfg)) = config::AppConfig::load(cli.config.as_deref()) {
        if let Some(dirs) = cfg.directories {
            registry.set_directories(dirs.model_cache);
        }
    }
    let snapshot = registry.collect_all();
    let memory = snapshot
        .memory
        .as_ref()
        .expect("Memory collector must succeed");
    let cpu = snapshot.cpu.as_ref().map(|c| c.load_avg_1m).unwrap_or(0.0);
    let disk = snapshot.disk.as_ref().map(|d| d.available_mb).unwrap_or(0);

    let pressure_str = memory.pressure.to_string();
    let point = HistoryPoint {
        timestamp: snapshot.timestamp.clone(),
        memory_available_mb: memory.available_mb,
        memory_pressure: pressure_str,
        cpu_load: cpu,
        disk_available_mb: disk,
        tokens_processed: cli.tokens_processed,
    };
    let store = HistoryStore::new(30);
    match store.record(&point) {
        Ok(_) => {
            let points = store.read_all().unwrap_or_default();
            println!(
                "✓ Recorded system state to history ({} points total)",
                points.len()
            );
        }
        Err(e) => {
            eprintln!("Failed to record history: {}", e);
            std::process::exit(1);
        }
    }
}

// ============================================================================
// --suggest-concurrency：建议并发度 (REQ-001)
// ============================================================================

pub fn handle_suggest_concurrency(cli: &Cli) {
    let mut registry = CollectorRegistry::new();
    if let Ok(Some(cfg)) = config::AppConfig::load(cli.config.as_deref()) {
        if let Some(dirs) = cfg.directories {
            registry.set_directories(dirs.model_cache);
        }
    }
    let snapshot = registry.collect_all();
    let result = suggest::suggest_concurrency(&snapshot, cli.task_memory);
    helpers::print_json(&result);
}

// ============================================================================
// --serve：远程采集 HTTP 服务端模式
// ============================================================================

pub fn handle_serve(cli: &Cli) {
    let api_key = config::AppConfig::load(None)
        .ok()
        .flatten()
        .and_then(|c| c.remote)
        .and_then(|r| r.api_key);
    let server = remote::RemoteServer::new(cli.port, api_key);
    server.start().unwrap_or_else(|e| {
        eprintln!("Failed to start server: {}", e);
        std::process::exit(1);
    });
}

// ============================================================================
// --heartbeat：单行心跳 JSON
// ============================================================================

pub fn handle_heartbeat(cli: &Cli) {
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

    let pressure = match &metrics.pressure {
        collector::PressureLevel::Low => "low",
        collector::PressureLevel::Medium => "medium",
        collector::PressureLevel::High => "high",
        collector::PressureLevel::Critical => "critical",
    };

    let action = match &metrics.pressure {
        collector::PressureLevel::Low => "ok",
        collector::PressureLevel::Medium => "monitor",
        collector::PressureLevel::High => "reduce_context",
        collector::PressureLevel::Critical => "abort_safely",
    };

    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    let output = serde_json::json!({
        "pressure": pressure,
        "available_mb": metrics.available_mb,
        "used_percent": metrics.used_percent,
        "action": action,
        "timestamp": timestamp,
    });

    println!("{}", serde_json::to_string(&output).unwrap());
}
