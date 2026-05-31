// Copyright 2026 秋毫mem Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// ============================================================================
// src/commands/onboarding.rs — 秋毫mem 能力全景展示（--onboarding）
// ============================================================================
//
// 新用户一键展示所有亮点功能，震惊 agent 和用户！
// 聚合：系统体检 → 缓存策略 → Token花销 → 趋势分析 →
//       并发建议 → GPU/Agent → 环境指纹 → Agent指导
// ============================================================================

use crate::cache;
use crate::collector::registry::CollectorRegistry;
use crate::collector;
use crate::collector::ResourceCollector;
use crate::config;
use crate::engine::guidance::GuidanceGenerator;
use crate::environment;
use crate::gpu;
use crate::suggest;
use crate::trends::{HistoryStore, TrendAnalyzer};
use crate::Cli;

// ============================================================================
// --onboarding：能力全景展示
// ============================================================================

pub fn handle_onboarding(cli: &Cli) {
    // 一次性采集所有系统数据
    let mut registry = CollectorRegistry::new();
    if let Ok(Some(cfg)) = config::AppConfig::load(cli.config.as_deref()) {
        if let Some(dirs) = cfg.directories {
            registry.set_directories(dirs.model_cache);
        }
    }
    let snapshot = registry.collect_all();

    // 顶部标题
    println!();
    println!(
        "  ╔══════════════════════════════════════════════════════════╗"
    );
    println!(
        "  ║            🦅  秋毫mem  ·  能力全景展示                  ║"
    );
    println!(
        "  ║           HawkEye Mem  —  AI-Native Monitor             ║"
    );
    println!(
        "  ╚══════════════════════════════════════════════════════════╝"
    );
    println!(
        "  🌐  版本 {}  |  零 Token 消耗  |  全本地采集",
        env!("CARGO_PKG_VERSION")
    );
    println!();

    // ── 1. 系统体检 ──────────────────────────────────────────────
    print_section("📊", "系统体检");
    if let Some(ref mem) = snapshot.memory {
        let mem_pct = mem.used_percent;
        let mem_status = if mem_pct < 50.0 { "✅" } else if mem_pct < 80.0 { "⚠️" } else { "🔴" };
        println!("  📈  内存: {:>4.0} / {:<4} MB ({:.1}%)   {}", mem.used_mb, mem.total_mb, mem_pct, mem_status);

        let cpu = snapshot.cpu.as_ref();
        let cpu_load = cpu.map(|c| c.load_avg_1m).unwrap_or(0.0f64);
        let cpu_cores = cpu.map(|c| c.cores).unwrap_or(0);
        let cpu_status = if cpu_load < cpu_cores as f64 * 0.8 { "✅" } else { "⚠️" };
        println!("  🖥️   CPU: {:>2} 核  |  负载 {:.2}/{:>3.2}/{:.2}  {}",
            cpu_cores, cpu_load,
            cpu.map(|c| c.load_avg_5m).unwrap_or(0.0f64),
            cpu.map(|c| c.load_avg_15m).unwrap_or(0.0f64),
            cpu_status);

        if let Some(ref disk) = snapshot.disk {
            let disk_pct = disk.used_percent;
            let disk_status = if disk_pct < 70.0 { "✅" } else if disk_pct < 90.0 { "⚠️" } else { "🔴" };
            println!("  💾  磁盘: {:>5} MB 可用 ({:.1}%)  {}",
                disk.available_mb, 100.0 - disk_pct, disk_status);
        }
    }

    if let Some(ref thermal) = snapshot.thermal {
        let cpu_temp = thermal.cpu_temp_c.unwrap_or(0.0);
        let thermal_icon = match cpu_temp {
            t if t < 70.0 => "🌡️",
            t if t < 85.0 => "🔥",
            _ => "💥",
        };
        println!("  {}  CPU 温度: {:.0}°C  {}", thermal_icon, cpu_temp,
            if cpu_temp < 70.0 { "✅" } else { "⚠️" });
    }
    println!();

    // ── 2. 缓存策略 ──────────────────────────────────────────────
    print_section("🧠", "缓存策略");
    print_cache_strategy(&snapshot);
    println!();

    // ── 3. Token 花销总览 ────────────────────────────────────────
    print_section("💰", "Token 花销总览");
    print_token_overview();
    println!();

    // ── 4. 趋势分析 ──────────────────────────────────────────────
    print_section("📈", "趋势分析");
    print_trend_report();
    println!();

    // ── 5. 并发建议 ──────────────────────────────────────────────
    print_section("🎯", "并发建议");
    print_concurrency(&snapshot, cli);
    println!();

    // ── 6. GPU 状态 + Agent 检测 + 环境指纹 ────────────────────
    print_section("🖥️", "GPU · Agent · 环境");
    print_gpu_info();
    print_agent_info(&snapshot);
    print_env_fingerprint();
    println!();

    // ── 网络状态（V0.7.1）────────────────────────────
    print_section("📡", "网络状态");
    print_network_status();
    println!();

    // ── 7. Agent 指导 ────────────────────────────────────────────
    print_section("💡", "Agent 决策指导");
    if let Some(ref mem) = snapshot.memory {
        print_agent_guidance(mem);
    }
    println!();

    // ── 页脚 ─────────────────────────────────────────────────────
    println!("  ══════════════════════════════════════════════════════════");
    println!("  ✨  秋毫mem 零 Token 消耗 · 所有数据均为本地采集");
    println!("  📖  完整文档: github.com/qiuhaomem/HawkEye-Mem");
    println!("  🚀  一键安装: curl -fsSL https://tinyurl.com/... | bash");
    println!();
}

// ============================================================================
// 辅助函数
// ============================================================================

fn print_section(icon: &str, title: &str) {
    let line = "─".repeat(48);
    println!("  {} {} {}", icon, title, line);
}

fn print_cache_strategy(snapshot: &collector::ResourceSnapshot) {
    if let Some(ref mem) = snapshot.memory {
        let pressure = cache::MemoryPressure::from(mem);
        let strategy = cache::CacheAdvisor::recommend(&pressure);
        let mode_str = strategy.mode.to_string();
        let mode_icon = match mode_str.as_str() {
            "aggressive" => "🚀",
            "balanced" => "⚖️",
            "conservative" => "🛡️",
            "emergency" => "🆘",
            _ => "❓",
        };
        println!("  模式:    {:12}   {}", mode_str, mode_icon);
        println!("  TTL:     {} 秒", strategy.ttl_seconds);
        println!("  缓存上限: {} MB", strategy.max_cache_mb);
        println!("  预取:    {}",
            if strategy.prefetch_enabled { "已启用 ✅" } else { "未启用 ❌" });
        println!("  原因:    {}", strategy.reason);
    } else {
        println!("  📭  无法获取内存数据");
    }
}

fn print_token_overview() {
    #[cfg(feature = "budget")]
    {
        use crate::budget::collector::TokenCollector;
        use crate::budget::cost::CostCalculator;

        let result = TokenCollector::collect();
        if result.records.is_empty() {
            println!("  📭  暂无 Token 数据（首次运行时会自动采集）");
            return;
        }
        let summary = crate::budget::collector::aggregate_tokens(&result.records);
        let calculator = CostCalculator::new();
        let total_cost = calculator.calculate_batch_cost(&result.records);
        let total_tokens = summary.total_input_tokens + summary.total_output_tokens;
        let num_days = result.records.len().max(1);

        println!("  总消耗:   {:>12} tokens", total_tokens);
        println!("  总费用:   ${:>10.4}", total_cost);
        println!("  API 调用: {:>12}", summary.total_api_calls);
        println!("  缓存命中: {:>10.1}%", summary.cache_hit_rate);
        println!("  冷启动平均: {:>8} tokens/次", summary.first_call_tokens_avg);

        if !summary.by_model.is_empty() {
            println!();
            println!("  📦  按模型分布:");
            let max_models = 5.min(summary.by_model.len());
            for model in summary.by_model.iter().take(max_models) {
                let mt = model.input_tokens + model.output_tokens;
                let pct = if total_tokens > 0 { mt as f64 / total_tokens as f64 * 100.0 } else { 0.0 };
                println!("     {:<25} {:>10} tokens  ({:.1}%)  ${:.4}",
                    model.model, mt, pct, model.cost_usd);
            }
            if summary.by_model.len() > max_models {
                println!("     ... 还有 {} 个模型", summary.by_model.len() - max_models);
            }
        }
        println!();

        let daily_avg = total_tokens as f64 / num_days as f64;
        let daily_cost = total_cost / num_days as f64;
        println!("  📊  日均: {:>10.0} tokens  (${:.4})", daily_avg, daily_cost);
        println!("  📅  月估: {:>10.0} tokens  (${:.4})", daily_avg * 30.0, daily_cost * 30.0);

        // 缓存节省估算
        let saved_cost = total_cost / (1.0 - summary.cache_hit_rate / 100.0).max(0.01) - total_cost;
        println!("  💰  缓存节省: ~${:.2}", saved_cost);
    }

    #[cfg(not(feature = "budget"))]
    {
        println!("  📭  编译时未启用 budget feature，无法读取 Token 数据");
        println!("  💡  安装完整版: cargo install --features budget");
        println!();
        println!("  🌟  Token 数据包括:");
        println!("     · 总消耗/总费用/调用次数");
        println!("     · 缓存命中率/冷启动开销");
        println!("     · 按模型分布/日均/月估");
        println!("     · 缓存节省估算");
    }
}

fn print_trend_report() {
    let retention_days = 30;
    let store = HistoryStore::new(retention_days);
    match store.read_all() {
        Ok(points) => {
            if points.len() < 10 {
                println!("  📭  数据不足 (需要 ≥10 采样点，当前 {} 个)", points.len());
                println!("  💡  采集: hawk-eye-mem --record （建议添加 cron 自动采集）");
                return;
            }
            match TrendAnalyzer::analyze(&points) {
                Some(report) => {
                    let dir_icon = match report.direction.as_str() {
                        "increasing" => "📈",
                        "decreasing" => "📉",
                        _ => "➡️",
                    };
                    println!("  {}  方向: {}  (slope: {:.2} MB/min)", dir_icon, report.direction, report.slope_mb_per_minute);
                    if let Some(days) = report.days_until_critical {
                        println!("  ⏰  预估临界: {:.0} 天后", days);
                    } else {
                        println!("  ⏰  预估临界: 无风险 ✅");
                    }
                    println!("  📊  数据点: {} 个  |  置信度: {}  |  紧迫度: {}",
                        points.len(), report.confidence, report.urgency);
                }
                None => { println!("  📭  趋势分析暂时不可用"); }
            }
        }
        Err(e) => { println!("  ❌  读取趋势数据失败: {}", e); }
    }
}

fn print_concurrency(snapshot: &collector::ResourceSnapshot, cli: &Cli) {
    let result = suggest::suggest_concurrency(snapshot, cli.task_memory);
    let concurrency = result.suggestion.recommended_concurrency;
    let risk = result.suggestion.risk_level;
    let risk_icon = match risk.as_str() {
        "ok" => "✅",
        "caution" => "⚠️",
        "critical" => "🔴",
        _ => "❓",
    };
    println!("  当前可安全运行: {} 个并发任务  {}", concurrency, risk_icon);
    println!("  🔧  每任务安全内存: {} MB", result.suggestion.per_agent_safe_memory_mb);
    println!("  理由: {}", result.suggestion.reasoning);
}

fn print_gpu_info() {
    let gpu_collector = gpu::GpuCollector;
    match gpu_collector.collect() {
        Ok(collector::CollectorOutput::Gpu(gpus)) if !gpus.is_empty() => {
            for gpu in &gpus {
                let usage_pct = if gpu.vram_total_mb > 0 {
                    gpu.vram_used_mb as f64 / gpu.vram_total_mb as f64 * 100.0
                } else { 0.0 };
                println!("  🎮  {}  {:.0}°C  {}MB/{}MB ({:.0}%)  {}",
                    gpu.name, gpu.temp_celsius.unwrap_or(0.0),
                    gpu.vram_used_mb, gpu.vram_total_mb, usage_pct, gpu.backend);
            }
        }
        _ => { println!("  🖥️  未检测到 GPU（或当前系统无 GPU）"); }
    }
}

fn print_agent_info(snapshot: &collector::ResourceSnapshot) {
    if let Some(ref agents) = snapshot.agents {
        if !agents.agents.is_empty() {
            println!("  🤖  同机 Agent:");
            for agent in &agents.agents {
                println!("     · {} (PID: {}, {} MB)", agent.name, agent.pid, agent.memory_rss_mb.unwrap_or(0));
            }
            println!("     共计 {} 个 Agent, {} MB 内存占用", agents.count, agents.total_agent_memory_mb.unwrap_or(0));
        } else {
            println!("  🤖  未检测到其他 Agent 进程");
        }
    } else {
        println!("  🤖  Agent 检测功能未启用");
    }
}

fn print_env_fingerprint() {
    let fp_store = environment::store::FingerprintStore::new();
    match fp_store.load_current() {
        Ok(Some(fp)) => {
            let id_short = if fp.id.len() > 16 { format!("{}...", &fp.id[..16]) } else { fp.id.clone() };
            println!("  🆔  环境指纹: {}", id_short);
            println!("     主机: {}  |  平台: {}  |  CPU: {} 核  |  内存: {} MB",
                fp.hostname, fp.platform, fp.cpu_cores, fp.total_memory_mb);
        }
        _ => { println!("  🆔  环境指纹: 未生成（运行一次 hawk-eye-mem 即可创建）"); }
    }
}

fn print_network_status() {
    let registry = CollectorRegistry::new();
    let snapshot = registry.collect_all();
    if let Some(ref net) = snapshot.network {
        for iface in &net.interfaces {
            let status_icon = if iface.state == "up" { "✅" } else { "❌" };
            let speed_str = if let Some(s) = iface.speed_mbps {
                format!("↑ {}Mbps", s)
            } else { "--".to_string() };
            println!("  🖥️  {} ({})  {}  {}", iface.name, iface.if_type, speed_str, status_icon);

            if let (Some(rx), Some(tx)) = (iface.rx_speed_kbps, iface.tx_speed_kbps) {
                println!("     下载: {:.2} MB/s  |  上传: {:.2} MB/s", rx/1024.0, tx/1024.0);
            }

            if let Some(ref ip) = iface.ip {
                println!("     IP: {}", ip);
            }
        }

        if let Some(ref lat) = net.latency {
            if let Some(ms) = lat.ping_ms {
                println!("  📶  延迟: {:.1}ms → {}", ms, lat.target);
            } else {
                println!("  📶  延迟: 不可达");
            }
        }
    } else {
        println!("  📡  网络信息不可用");
    }
}

fn print_agent_guidance(mem: &collector::MemoryMetrics) {
    let (pressure, action, _context_reason) =
        GuidanceGenerator::classify(mem.available_mb, mem.used_percent);
    let action_str = action.to_string();

    // 估算安全上下文窗口: 可用内存(MB) → bytes → /bytes_per_token → *安全余量
    let safe_window = (mem.available_mb as f64 * 1024.0 * 1024.0 / 2048.0 * 0.7) as u64;

    let pressure_icon = match pressure {
        "low" => "🟢",
        "medium" => "🟡",
        "high" => "🟠",
        "critical" => "🔴",
        _ => "❓",
    };
    println!("  {}  压力等级: {}  |  建议行动: {}", pressure_icon, pressure, action_str);
    println!("  📐  安全上下文: ~{} tokens", safe_window);
    println!();
    println!("  📋  行动说明:");
    match action_str.as_str() {
        "ok" => println!("     ✅ 一切正常, 放心使用！"),
        "monitor" => println!("     👀 中等压力, 建议关注"),
        "reduce_context" => println!("     ⚠️ 高压力, 建议减少上下文"),
        "abort_safely" => println!("     🔴 内存危急, 建议安全中止"),
        _ => println!("     未知行动"),
    }
}
