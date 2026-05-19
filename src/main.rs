mod collector;
mod config;
mod engine;

use clap::Parser;
use collector::{linux::LinuxCollector, macos::MacosCollector, MemoryCollector};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "hawk-eye-mem", version = "0.1.0", about = "AI-Native memory monitoring")]
struct Cli {
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

    #[cfg(target_os = "linux")]
    let collector = LinuxCollector;
    #[cfg(target_os = "macos")]
    let collector = MacosCollector;
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let collector = UnsupportedCollector;

    let count = cli.count.unwrap_or(1);
    let interval = cli.interval.unwrap_or(0);
    let is_continuous = interval > 0 && count > 0;

    for i in 0..count {
        if i > 0 && interval > 0 {
            std::thread::sleep(std::time::Duration::from_secs(interval));
        }

        let start = std::time::Instant::now();
        let metrics = match collector.collect() {
            Ok(m) => m,
            Err(e) => {
                if cli.json || is_continuous {
                    let err_json = serde_json::json!({ "error": e.to_string() });
                    println!("{}", serde_json::to_string(&err_json).unwrap());
                } else {
                    eprintln!("Error: {}", e);
                }
                std::process::exit(1);
            }
        };
        let collect_duration = start.elapsed().as_secs_f64() * 1000.0;

        if is_continuous && !cli.metric.is_some() {
            // JSON Lines 模式
            let app_config = load_config(&cli);
            let result = calc_estimate(&metrics, &app_config);
            let output = build_json_output(&metrics, &result, collect_duration);
            println!("{}", serde_json::to_string(&output).unwrap());
        } else if let Some(metric) = &cli.metric {
            print_metric(&metrics, metric);
        } else {
            let app_config = load_config(&cli);
            let result = calc_estimate(&metrics, &app_config);
            let output = build_json_output(&metrics, &result, collect_duration);
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
    }
}

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

fn calc_estimate(metrics: &collector::MemoryMetrics, model_config: &Option<engine::ModelConfig>) -> engine::EstimationResult {
    engine::EstimationEngine::estimate(metrics.available_mb, model_config)
}

fn build_json_output(
    metrics: &collector::MemoryMetrics,
    result: &engine::EstimationResult,
    duration_ms: f64,
) -> serde_json::Value {
    let guidance = engine::guidance::GuidanceGenerator::generate(
        metrics.available_mb,
        metrics.used_percent,
        result.estimated_tokens,
        &result.confidence.to_string(),
    );
    serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "collection_duration_ms": (duration_ms * 10.0).round() / 10.0,
        "system": {
            "total_mb": metrics.total_mb,
            "used_mb": metrics.used_mb,
            "available_mb": metrics.available_mb,
            "used_percent": metrics.used_percent,
        },
        "agent_guidance": guidance,
    })
}

fn print_metric(metrics: &collector::MemoryMetrics, name: &str) {
    match name {
        "total_mb" => println!("{}", metrics.total_mb),
        "used_mb" => println!("{}", metrics.used_mb),
        "available_mb" => println!("{}", metrics.available_mb),
        "used_percent" => println!("{:.1}", metrics.used_percent),
        "pressure" => {
            let (pressure, _, _) = engine::guidance::GuidanceGenerator::classify(
                metrics.available_mb,
                metrics.used_percent,
            );
            println!("{}", pressure);
        }
        _ => {
            eprintln!("Unknown metric: {}", name);
            std::process::exit(1);
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
struct UnsupportedCollector;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
impl MemoryCollector for UnsupportedCollector {
    fn collect(&self) -> Result<collector::MemoryMetrics, collector::CollectError> {
        Err(collector::CollectError::UnsupportedPlatform)
    }
}
