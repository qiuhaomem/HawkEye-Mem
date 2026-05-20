mod collector;
mod config;
mod engine;

use clap::Parser;
#[cfg(target_os = "linux")]
use collector::linux::LinuxCollector;
#[cfg(target_os = "macos")]
use collector::macos::MacosCollector;
use collector::MemoryCollector;
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

    // 处理 --init-config：生成默认配置文件后退出
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

    #[cfg(target_os = "linux")]
    let collector = LinuxCollector;
    #[cfg(target_os = "macos")]
    let collector = MacosCollector;
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let collector = UnsupportedCollector;

    let count = cli.count.unwrap_or(1);
    let interval = cli.interval.unwrap_or(0);
    let infinite = count == 0 && interval > 0;
    let is_continuous = interval > 0 && count > 0;

    // SIGINT handler: set an atomic flag for graceful shutdown
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    let mut iter = 0u64;
    while running.load(Ordering::SeqCst) && (infinite || iter < count) {
        if iter > 0 && interval > 0 {
            // Chunked sleep (100ms slices) so we stay responsive to SIGINT
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

        let start = std::time::Instant::now();
        let metrics = match collector.collect() {
            Ok(m) => m,
            Err(e) => {
                if cli.json || is_continuous || infinite {
                    let err_json = serde_json::json!({ "error": e.to_string() });
                    println!("{}", serde_json::to_string(&err_json).unwrap());
                } else {
                    eprintln!("Error: {}", e);
                }
                std::process::exit(1);
            }
        };
        let collect_duration = start.elapsed().as_secs_f64() * 1000.0;

        if (is_continuous || infinite) && !cli.metric.is_some() {
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

        iter += 1;
    }

    if !running.load(Ordering::SeqCst) {
        eprintln!("Interrupted by user");
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
    let mut guidance_value = serde_json::to_value(&guidance).unwrap();
    guidance_value["_note"] = serde_json::Value::String(
        "The following are recommendations only. The ultimate decision-making authority resides with the user.".to_string()
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
        "agent_guidance": guidance_value,
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