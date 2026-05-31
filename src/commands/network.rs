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
// src/commands/network.rs — --network 命令：采集并展示网络状态
// ============================================================================

use crate::collector::network::InterfaceMetrics;
use crate::collector::registry::CollectorRegistry;
use crate::Cli;

/// 格式化字节数为人类可读字符串
fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.1} GB", bytes as f64 / 1_000_000_000.0)
    } else if bytes >= 1_000_000 {
        format!("{:.0} MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.1} KB", bytes as f64 / 1_000.0)
    } else {
        format!("{} B", bytes)
    }
}

/// 格式化速度（kbps）为人类可读字符串
fn format_speed(kbps: Option<f64>) -> String {
    match kbps {
        Some(v) if v >= 1000.0 => format!("{:.2} MB/s", v / 1000.0),
        Some(v) => format!("{:.0} KB/s", v),
        None => "--".to_string(),
    }
}

/// 获取接口类型的中文显示名
fn if_type_label(if_type: &str) -> &str {
    match if_type {
        "ethernet" => "以太网",
        "wifi" => "无线",
        "loopback" => "回环",
        "bridge" => "桥接",
        "tunnel" => "隧道",
        _ => if_type,
    }
}

/// 处理 --network 命令：采集网络数据并格式化输出
pub fn handle_network(_cli: &Cli) {
    let registry = CollectorRegistry::new();
    let snapshot = registry.collect_all();

    let network_metrics = match snapshot.network {
        Some(n) => n,
        None => {
            eprintln!("⚠️  无法采集网络数据（非 Linux 平台或读取失败）");
            return;
        }
    };

    println!("\n📡 网络状态");
    println!("{}", "━".repeat(30));

    for iface in &network_metrics.interfaces {
        print_interface(iface, &network_metrics.latency);
    }
}

/// 打印单个接口的详细信息
fn print_interface(iface: &InterfaceMetrics, latency: &Option<crate::collector::network::LatencyMetrics>) {
    let label = if_type_label(&iface.if_type);

    // 第一行：接口名 (类型)  ↑ 速率  ✓/✗
    let speed_str = match iface.speed_mbps {
        Some(s) => format!("↑ {}Mbps", s),
        None => "↑ 无".to_string(),
    };
    let status_icon = if iface.state == "up" { "✓" } else { "✗" };

    println!(
        "{} ({})  {}  {}",
        iface.name, label, speed_str, status_icon
    );

    // 第二行：实时下载/上传速率
    if iface.if_type == "loopback" {
        // 回环接口：简化输出
        println!(
            "    状态: {}  |  接收: {}  |  发送: {}",
            iface.state,
            format_bytes(iface.rx_bytes),
            format_bytes(iface.tx_bytes),
        );
    } else {
        let rx_speed = format_speed(iface.rx_speed_kbps);
        let tx_speed = format_speed(iface.tx_speed_kbps);
        println!(
            "    下载: {}  |  上传: {}",
            rx_speed, tx_speed,
        );

        // 延迟信息（只对非回环接口显示）
        if let Some(lt) = latency {
            let delay_str = match lt.ping_ms {
                Some(ms) => format!("{:.1}ms → {}", ms, lt.target),
                None => "超时/不可达".to_string(),
            };
            println!("    延迟: {}", delay_str);
        }

        // IP/MAC
        let ip_str = iface.ip.as_deref().unwrap_or("-");
        let mac_str = iface.mac.as_deref().unwrap_or("-");
        println!("    IP: {}  |  MAC: {}", ip_str, mac_str);

        // 累计流量 + 错误
        println!(
            "    接收: {}  |  发送: {}  |  错误: {}",
            format_bytes(iface.rx_bytes),
            format_bytes(iface.tx_bytes),
            iface.rx_errors + iface.tx_errors,
        );
    }

    println!();
}
