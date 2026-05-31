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

use super::{CollectError, CollectorOutput, ResourceCollector};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

// ============================================================================
// 网络指标结构体
// ============================================================================

/// 网络指标（V0.7 Phase 1）
#[derive(Debug, Clone, serde::Serialize)]
pub struct NetworkMetrics {
    pub interfaces: Vec<InterfaceMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency: Option<LatencyMetrics>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct InterfaceMetrics {
    pub name: String,
    pub state: String,           // up/down
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_mbps: Option<u64>, // 协商速率
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mac: Option<String>,
    pub if_type: String,         // ethernet/wifi/loopback/tunnel
    pub rx_bytes: u64,           // 累计接收字节
    pub tx_bytes: u64,           // 累计发送字节
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rx_speed_kbps: Option<f64>, // 实时下载速率
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_speed_kbps: Option<f64>, // 实时上传速率
    pub rx_errors: u64,
    pub tx_errors: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LatencyMetrics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ping_ms: Option<f64>,
    pub target: String,
}

// ============================================================================
// 速率差值计算：存储上次采集值
// ============================================================================

/// 每次采集的快照，用于计算实时速率差值
struct RateSnapshot {
    rx_bytes: u64,
    tx_bytes: u64,
    timestamp: Instant,
}

/// 按接口名存储上一次采集值
static PREVIOUS_RATES: Mutex<Option<HashMap<String, RateSnapshot>>> = Mutex::new(None);

/// 计算实时速率（kbps），如果前一次采集不存在则返回 None
fn compute_speed_kbps(
    name: &str,
    rx_bytes: u64,
    tx_bytes: u64,
) -> (Option<f64>, Option<f64>) {
    let now = Instant::now();

    let mut guard = match PREVIOUS_RATES.lock() {
        Ok(g) => g,
        Err(_) => return (None, None),
    };

    let prev = guard.as_mut().and_then(|map| map.remove(name));

    if let Some(prev) = prev {
        let elapsed = now.duration_since(prev.timestamp).as_secs_f64();
        if elapsed <= 0.0 {
            return (None, None);
        }

        let rx_delta = rx_bytes.saturating_sub(prev.rx_bytes);
        let tx_delta = tx_bytes.saturating_sub(prev.tx_bytes);

        // bytes/s -> kbps: * 8 / 1000
        let rx_speed = (rx_delta as f64 * 8.0 / 1000.0) / elapsed;
        let tx_speed = (tx_delta as f64 * 8.0 / 1000.0) / elapsed;

        // 更新为新值
        let entry = guard.get_or_insert_with(HashMap::new);
        entry.insert(
            name.to_string(),
            RateSnapshot {
                rx_bytes,
                tx_bytes,
                timestamp: now,
            },
        );

        (Some((rx_speed * 10.0).round() / 10.0), Some((tx_speed * 10.0).round() / 10.0))
    } else {
        // 第一次采集，只存储不返回速率
        let entry = guard.get_or_insert_with(HashMap::new);
        entry.insert(
            name.to_string(),
            RateSnapshot {
                rx_bytes,
                tx_bytes,
                timestamp: now,
            },
        );
        (None, None)
    }
}

// ============================================================================
// 网络收集器
// ============================================================================

/// 网络采集器：读取 /proc/net/dev 和 /sys/class/net/
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub struct NetworkCollector;

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
impl ResourceCollector for NetworkCollector {
    fn collect(&self) -> Result<CollectorOutput, CollectError> {
        let interfaces = collect_interfaces()?;
        let latency = measure_latency();

        Ok(CollectorOutput::Network(NetworkMetrics {
            interfaces,
            latency,
        }))
    }
}

// ============================================================================
// 接口信息收集（Linux 实现）
// ============================================================================

#[cfg(target_os = "linux")]
fn collect_interfaces() -> Result<Vec<InterfaceMetrics>, CollectError> {
    // 1. 获取 /sys/class/net/ 下的所有接口名
    let net_dir = std::path::Path::new("/sys/class/net");
    let entries = std::fs::read_dir(net_dir)
        .map_err(|e| CollectError::ReadFailed(format!("Cannot read /sys/class/net: {}", e)))?;

    // 2. 读取 /proc/net/dev 流量计数
    let proc_net = std::fs::read_to_string("/proc/net/dev")
        .map_err(|e| CollectError::ReadFailed(format!("Cannot read /proc/net/dev: {}", e)))?;

    // 3. 尝试获取 IP 信息（非关键，失败可忽略）
    let ip_map = get_ip_addresses();

    let mut interfaces = Vec::new();

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let name = entry.file_name().to_string_lossy().to_string();

        // 跳过虚拟接口：点对点（如 docker, veth）
        let if_type = detect_if_type(&name);
        let state = read_state(&name);
        let mac = read_mac(&name);
        let speed = read_speed(&name);
        let ip = ip_map.get(&name).cloned();

        // 从 /proc/net/dev 读取流量
        let (rx_bytes, tx_bytes, rx_errors, tx_errors) =
            parse_proc_net_dev(&proc_net, &name).unwrap_or((0, 0, 0, 0));

        // 计算实时速率
        let (rx_speed_kbps, tx_speed_kbps) = compute_speed_kbps(&name, rx_bytes, tx_bytes);

        interfaces.push(InterfaceMetrics {
            name,
            state,
            speed_mbps: speed,
            ip,
            mac,
            if_type,
            rx_bytes,
            tx_bytes,
            rx_speed_kbps,
            tx_speed_kbps,
            rx_errors,
            tx_errors,
        });
    }

    Ok(interfaces)
}

#[cfg(not(target_os = "linux"))]
fn collect_interfaces() -> Result<Vec<InterfaceMetrics>, CollectError> {
    Err(CollectError::UnsupportedPlatform)
}

// ============================================================================
// 延迟检测
// ============================================================================

/// 对 1.1.1.1 执行 ping 以测量延迟
fn measure_latency() -> Option<LatencyMetrics> {
    let result = std::process::Command::new("ping")
        .args(["-c", "1", "-W", "2", "1.1.1.1"])
        .output();

    match result {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // 解析 ping 输出，提取 time=XX ms
            // 格式可能为: "64 bytes from 1.1.1.1: icmp_seq=1 ttl=56 time=12.3 ms"
            if let Some(line) = stdout.lines().find(|l| l.contains("time=")) {
                if let Some(time_str) = line.split("time=").nth(1) {
                    let time_str = time_str.trim_end_matches(" ms").trim();
                    if let Ok(ms) = time_str.parse::<f64>() {
                        return Some(LatencyMetrics {
                            ping_ms: Some((ms * 10.0).round() / 10.0),
                            target: "1.1.1.1".to_string(),
                        });
                    }
                }
            }
            // 解析失败但仍成功 ping，返回空值
            Some(LatencyMetrics {
                ping_ms: None,
                target: "1.1.1.1".to_string(),
            })
        }
        _ => {
            // ping 失败（超时/不可达/命令不存在）
            Some(LatencyMetrics {
                ping_ms: None,
                target: "1.1.1.1".to_string(),
            })
        }
    }
}

// ============================================================================
// Linux /sys/class/net/ 辅助函数
// ============================================================================

/// 检测接口类型
#[cfg(target_os = "linux")]
fn detect_if_type(name: &str) -> String {
    // 1. loopback
    if name == "lo" {
        return "loopback".to_string();
    }

    // 2. 判断是否无线（/sys/class/net/<name>/wireless/ 存在）
    let wireless_path = format!("/sys/class/net/{}/wireless", name);
    if std::path::Path::new(&wireless_path).exists() {
        return "wifi".to_string();
    }

    // 3. 判断是否 bridge
    let bridge_path = format!("/sys/class/net/{}/bridge", name);
    if std::path::Path::new(&bridge_path).exists() {
        return "bridge".to_string();
    }

    // 4. 常见虚拟隧道/容器接口
    if name.starts_with("docker")
        || name.starts_with("veth")
        || name.starts_with("br-")
        || name.starts_with("tun")
        || name.starts_with("tap")
        || name.starts_with("bond")
        || name.starts_with("ovs-")
        || name.starts_with("gre")
        || name.starts_with("gretap")
        || name.contains("tunnel")
    {
        return "tunnel".to_string();
    }

    // 5. 默认以太网
    "ethernet".to_string()
}

#[cfg(not(target_os = "linux"))]
fn detect_if_type(_name: &str) -> String {
    String::new()
}

/// 读取接口状态（up/down）
#[cfg(target_os = "linux")]
fn read_state(name: &str) -> String {
    let path = format!("/sys/class/net/{}/operstate", name);
    std::fs::read_to_string(&path)
        .ok()
        .map(|s| {
            let s = s.trim().to_lowercase();
            if s == "up" { "up".to_string() } else { "down".to_string() }
        })
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(not(target_os = "linux"))]
fn read_state(_name: &str) -> String {
    String::new()
}

/// 读取 MAC 地址
#[cfg(target_os = "linux")]
fn read_mac(name: &str) -> Option<String> {
    let path = format!("/sys/class/net/{}/address", name);
    std::fs::read_to_string(&path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "00:00:00:00:00:00")
}

#[cfg(not(target_os = "linux"))]
fn read_mac(_name: &str) -> Option<String> {
    None
}

/// 读取协商速率（Mbps）
#[cfg(target_os = "linux")]
fn read_speed(name: &str) -> Option<u64> {
    let path = format!("/sys/class/net/{}/speed", name);
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .filter(|&v| v > 0 && v != 4294967295) // 4294967295 = -1 as u32, 表示未知
}

#[cfg(not(target_os = "linux"))]
fn read_speed(_name: &str) -> Option<u64> {
    None
}

// ============================================================================
// /proc/net/dev 解析
// ============================================================================

/// 从 /proc/net/dev 中解析指定接口的流量计数
fn parse_proc_net_dev(content: &str, iface: &str) -> Option<(u64, u64, u64, u64)> {
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with(iface) {
            continue;
        }
        // 格式: "  eth0: rx_bytes rx_packets ... tx_bytes tx_packets ..."
        // 去掉接口名和冒号后取数值
        let after_colon = trimmed.split(':').nth(1)?;
        let parts: Vec<&str> = after_colon.split_whitespace().collect();
        if parts.len() >= 10 {
            let rx_bytes = parts[0].parse::<u64>().ok()?;
            let rx_errors = parts[2].parse::<u64>().ok()?;
            let tx_bytes = parts[8].parse::<u64>().ok()?;
            let tx_errors = parts[10].parse::<u64>().ok()?;
            return Some((rx_bytes, tx_bytes, rx_errors, tx_errors));
        }
    }
    None
}

// ============================================================================
// IP 地址收集
// ============================================================================

/// 尝试收集所有接口的 IP 地址
/// 优先从 /proc/net/fib_trie 读取（本机路由），失败则回退到 hostname -I
fn get_ip_addresses() -> HashMap<String, String> {
    // 方法 1: 从 /proc/net/fib_trie 解析（更精确，按接口）
    if let Some(ip_map) = parse_fib_trie() {
        return ip_map;
    }

    // 方法 2: 回退到 hostname -I（简单但只有主 IP）
    let mut map = HashMap::new();
    if let Ok(output) = std::process::Command::new("hostname")
        .arg("-I")
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let ip = stdout.trim().split_whitespace().next().map(|s| s.to_string());
            if let Some(ip) = ip {
                map.insert("primary".to_string(), ip);
            }
        }
    }
    map
}

/// 从 /proc/net/fib_trie 解析 IP -> 接口名映射
/// 格式较复杂，简单实现：找 LOCAL 路由中每个 interface 的 preferred_source
fn parse_fib_trie() -> Option<HashMap<String, String>> {
    let content = std::fs::read_to_string("/proc/net/fib_trie").ok()?;
    let mut map = HashMap::new();
    let mut current_iface: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        // 寻找 "Local:" 标记
        if trimmed == "Local:" {
            continue;
        }
        // 行首无缩进表示新接口段
        if !line.starts_with(' ') && !line.starts_with('\t') && !trimmed.is_empty() {
            // 可能是接口名行：例如 "+-- 0.0.0.0/0 1.2.3.4 eth0"
            // 简单启发：取最后一个看起来像接口名的 token
            if let Some(last) = trimmed.split_whitespace().last() {
                if !last.contains('.') && !last.contains('/') && last.len() < 20 {
                    current_iface = Some(last.to_string());
                }
            }
            continue;
        }
        // 寻找带 /32 的 LOCAL 条目
        if trimmed.contains("/32") && trimmed.contains("LOCAL") {
            if let Some(ref iface) = current_iface {
                // 提取 IP：行中第一个点分十进制格式
                for token in trimmed.split_whitespace() {
                    if let Some(pos) = token.find('/') {
                        let ip_candidate = &token[..pos];
                        if ip_candidate.chars().filter(|&c| c == '.').count() == 3 {
                            let is_ipv4 = ip_candidate
                                .split('.')
                                .all(|octet| octet.parse::<u8>().is_ok());
                            if is_ipv4 {
                                map.insert(iface.clone(), ip_candidate.to_string());
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    if map.is_empty() { None } else { Some(map) }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // ========================================================================
    // UT-NET-001: InterfaceMetrics 结构体能正确构造和序列化
    // ========================================================================
    #[test]
    fn test_ut_net_001_interface_struct() {
        let iface = InterfaceMetrics {
            name: "eth0".to_string(),
            state: "up".to_string(),
            speed_mbps: Some(1000),
            ip: Some("192.168.1.1".to_string()),
            mac: Some("00:11:22:33:44:55".to_string()),
            if_type: "ethernet".to_string(),
            rx_bytes: 123456789,
            tx_bytes: 987654321,
            rx_speed_kbps: Some(1500.5),
            tx_speed_kbps: Some(800.3),
            rx_errors: 0,
            tx_errors: 2,
        };

        let json = serde_json::to_value(&iface).unwrap();
        assert_eq!(json["name"], "eth0");
        assert_eq!(json["state"], "up");
        assert_eq!(json["speed_mbps"], 1000);
        assert_eq!(json["ip"], "192.168.1.1");
        assert_eq!(json["mac"], "00:11:22:33:44:55");
        assert_eq!(json["if_type"], "ethernet");
        assert_eq!(json["rx_bytes"], 123456789);
        assert_eq!(json["tx_bytes"], 987654321);
        assert_eq!(json["rx_speed_kbps"].as_f64().unwrap(), 1500.5);
        assert_eq!(json["tx_speed_kbps"].as_f64().unwrap(), 800.3);
        assert_eq!(json["rx_errors"], 0);
        assert_eq!(json["tx_errors"], 2);
    }

    // ========================================================================
    // UT-NET-002: NetworkMetrics 包含接口列表和延迟
    // ========================================================================
    #[test]
    fn test_ut_net_002_network_metrics() {
        let metrics = NetworkMetrics {
            interfaces: vec![
                InterfaceMetrics {
                    name: "eth0".to_string(),
                    state: "up".to_string(),
                    speed_mbps: Some(1000),
                    ip: Some("10.0.0.1".to_string()),
                    mac: Some("aa:bb:cc:dd:ee:ff".to_string()),
                    if_type: "ethernet".to_string(),
                    rx_bytes: 100,
                    tx_bytes: 200,
                    rx_speed_kbps: None,
                    tx_speed_kbps: None,
                    rx_errors: 0,
                    tx_errors: 0,
                },
            ],
            latency: Some(LatencyMetrics {
                ping_ms: Some(15.3),
                target: "1.1.1.1".to_string(),
            }),
        };

        let json = serde_json::to_value(&metrics).unwrap();
        assert_eq!(json["interfaces"].as_array().unwrap().len(), 1);
        assert_eq!(json["latency"]["ping_ms"].as_f64().unwrap(), 15.3);
        assert_eq!(json["latency"]["target"], "1.1.1.1");
    }

    // ========================================================================
    // UT-NET-003: LatencyMetrics ping 失败时 ping_ms 为 null
    // ========================================================================
    #[test]
    fn test_ut_net_003_latency_none() {
        let latency = LatencyMetrics {
            ping_ms: None,
            target: "1.1.1.1".to_string(),
        };
        let json = serde_json::to_value(&latency).unwrap();
        assert!(json["target"].as_str().unwrap().contains("1.1.1.1"));
        // ping_ms 应为 null（被 skip_serializing_if 隐藏）
        assert!(
            !json.as_object().unwrap().contains_key("ping_ms"),
            "None 的 ping_ms 应被跳过"
        );
    }

    // ========================================================================
    // UT-NET-004: /proc/net/dev 解析测试
    // ========================================================================
    #[test]
    fn test_ut_net_004_parse_proc_net_dev() {
        let sample = "Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
    lo: 1234567    5678    0    0    0     0          0         0  7654321    4321    0    0    0     0       0          0
  eth0: 10000000 50000    1    2    0     0          0         0  20000000 30000    3    4    0     0       0          0
";

        let result = parse_proc_net_dev(sample, "eth0");
        assert!(result.is_some());
        let (rx, tx, rx_err, tx_err) = result.unwrap();
        assert_eq!(rx, 10_000_000);
        assert_eq!(tx, 20_000_000);
        assert_eq!(rx_err, 1);
        assert_eq!(tx_err, 3);

        // lo 接口
        let result_lo = parse_proc_net_dev(sample, "lo");
        assert!(result_lo.is_some());
        let (rx, tx, _, _) = result_lo.unwrap();
        assert_eq!(rx, 1_234_567);
        assert_eq!(tx, 7_654_321);

        // 不存在接口
        let result_none = parse_proc_net_dev(sample, "nonexist");
        assert!(result_none.is_none());
    }

    // ========================================================================
    // UT-NET-005: 速率计算 — 第一次返回 None，第二次正常计算
    // ========================================================================
    #[test]
    fn test_ut_net_005_rate_computation() {
        // 清理全局状态
        if let Ok(mut guard) = PREVIOUS_RATES.lock() {
            *guard = None;
        }

        // 第一次采集：返回 None
        let (rx_speed, tx_speed) = compute_speed_kbps("test0", 1000, 2000);
        assert!(rx_speed.is_none());
        assert!(tx_speed.is_none());

        // 模拟 1 秒后第二次采集，字节增加
        std::thread::sleep(Duration::from_millis(100));

        let (rx_speed, tx_speed) = compute_speed_kbps("test0", 2000, 4000);
        assert!(rx_speed.is_some());
        assert!(tx_speed.is_some());

        // rx delta = 1000 bytes in ~0.1s -> 1000*8/1000/0.1 = 80 kbps
        // tx delta = 2000 bytes -> 160 kbps
        // 由于计时有浮动，只验证大于 0
        assert!(rx_speed.unwrap() > 0.0);
        assert!(tx_speed.unwrap() > 0.0);

        // 清理
        if let Ok(mut guard) = PREVIOUS_RATES.lock() {
            *guard = None;
        }
    }

    // ========================================================================
    // UT-NET-006: 接口类型检测
    // ========================================================================
    #[test]
    fn test_ut_net_006_detect_if_type_synthetic() {
        // 仅测试命名规则部分（不依赖文件系统）
        // "lo" 硬编码为 loopback
        // docker/veth/br- 等为 tunnel
        // 其余为 ethernet
        assert_eq!(detect_if_type("lo"), "loopback");
        // docker 接口
        assert_eq!(detect_if_type("docker0"), "tunnel");
        assert_eq!(detect_if_type("veth1234"), "tunnel");
        assert_eq!(detect_if_type("br-abcdef"), "tunnel");
        assert_eq!(detect_if_type("tun0"), "tunnel");
        assert_eq!(detect_if_type("tap0"), "tunnel");
        assert_eq!(detect_if_type("eth0"), "ethernet");
        assert_eq!(detect_if_type("ens33"), "ethernet");
    }
}
