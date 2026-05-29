// ============================================================================
// src/multi_agent.rs — MultiAgentDetector（V0.3 Phase 6）
//
// 检测同机其他 AI Agent 进程的资源占用情况。
// CR-06 约束：只检测不预警，不读取 cmdline（保护隐私）。
//
// 检测策略：
//   Linux:  /proc/[pid]/comm（只读进程名）
//   macOS:  ps -eo pid,comm（通用命令）
//
// 内置 Agent 列表：hermes, claude-code, autogpt, langchain,
//                  open-interpreter, deepseek-tui, reasonix
// ============================================================================

use crate::collector::{
    AgentDetection, AgentProcess, CollectError, CollectorOutput, ResourceCollector,
};

/// 内置已知 Agent 进程名列表（CR-06 不允许读取 cmdline，只能匹配 comm/进程名）
const KNOWN_AGENTS: &[&str] = &[
    "hermes",
    "claude-code",
    "autogpt",
    "langchain",
    "open-interpreter",
    "deepseek-tui",
    "reasonix",
];

/// 多 Agent 检测器
pub struct MultiAgentDetector {
    extra_processes: Vec<String>,
    /// V0.4: 用户自定义检测名称列表（不为空时替代 KNOWN_AGENTS）
    custom_names: Option<Vec<String>>,
}

impl MultiAgentDetector {
    pub fn new(extra_processes: Option<Vec<String>>) -> Self {
        Self {
            extra_processes: extra_processes.unwrap_or_default(),
            custom_names: None,
        }
    }

    /// V0.4: 设置自定义 Agent 名称列表（替代内置 KNOWN_AGENTS）
    pub fn set_custom_names(&mut self, names: Option<Vec<String>>) {
        self.custom_names = names;
    }

    /// 获取完整的待检测进程名列表（内置 + 用户配置）
    /// V0.4: 如果 custom_names 有值，则用它替代 KNOWN_AGENTS
    fn target_names(&self) -> Vec<String> {
        let mut names: Vec<String> = if let Some(ref custom) = self.custom_names {
            custom.clone()
        } else {
            KNOWN_AGENTS.iter().map(|s| s.to_string()).collect()
        };
        names.extend(self.extra_processes.iter().cloned());
        names
    }

    /// 检测 Agent 进程
    fn detect(&self) -> Vec<AgentProcess> {
        let targets = self.target_names();

        #[cfg(target_os = "linux")]
        {
            Self::detect_linux(&targets)
        }

        #[cfg(target_os = "macos")]
        {
            Self::detect_macos(&targets)
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            Vec::new()
        }
    }

    /// Linux: 遍历 /proc/[pid]/comm
    #[cfg(target_os = "linux")]
    fn detect_linux(targets: &[String]) -> Vec<AgentProcess> {
        let mut agents = Vec::new();

        let proc_dir = match std::fs::read_dir("/proc") {
            Ok(d) => d,
            Err(_) => return agents,
        };

        for entry in proc_dir.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let pid_str = match path.file_name() {
                Some(s) => s.to_str().unwrap_or(""),
                None => continue,
            };

            let pid: u32 = match pid_str.parse() {
                Ok(p) => p,
                Err(_) => continue,
            };

            // CR-06：只读取 comm，不读取 cmdline
            let comm_path = path.join("comm");
            let comm = match std::fs::read_to_string(comm_path) {
                Ok(c) => c.trim().to_string(),
                Err(_) => continue,
            };

            if targets.iter().any(|t| comm.contains(t) || comm == *t) {
                // 读取进程内存（RSS）和 CPU 百分比
                let memory_rss_mb = Self::read_proc_memory(pid);
                let cpu_percent = Self::read_cpu_percent(pid);
                agents.push(AgentProcess {
                    name: comm,
                    pid,
                    memory_rss_mb,
                    cpu_percent,
                });
            }
        }

        agents
    }

    /// Linux: 从 /proc/[pid]/status 读取 VmRSS
    #[cfg(target_os = "linux")]
    fn read_proc_memory(pid: u32) -> Option<u64> {
        let status_path = format!("/proc/{}/status", pid);
        let content = std::fs::read_to_string(status_path).ok()?;
        for line in content.lines() {
            if line.starts_with("VmRSS:") {
                // 格式: "VmRSS:   12345 kB"
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let kb: u64 = parts[1].parse().ok()?;
                    return Some(kb / 1024); // kB → MB
                }
            }
        }
        None
    }

    /// Linux: 从 /proc/[pid]/stat 读取 CPU 使用率（瞬时近似值）
    ///
    /// 计算公式：CPU% ≈ (utime + stime) / CLK_TCK / uptime_secs * 100 / cpu_cores
    /// 这是进程自启动以来的平均 CPU 使用率，非瞬时值，但对于一次采样足够参考。
    #[cfg(target_os = "linux")]
    fn read_cpu_percent(pid: u32) -> Option<f64> {
        // 读 /proc/[pid]/stat
        let stat = std::fs::read_to_string(format!("/proc/{}/stat", pid)).ok()?;

        // 进程名可能包含括号和空格，取最后一个 ')' 后面的内容
        let after_paren = stat.rsplit(')').next()?;
        let fields: Vec<&str> = after_paren.split_whitespace().collect();

        // 需要至少 13 个字段（indices 0..13）以访问 utime(index 11) 和 stime(index 12)
        if fields.len() < 13 {
            return None;
        }

        let utime: u64 = fields[11].parse().ok()?;
        let stime: u64 = fields[12].parse().ok()?;
        let total_ticks = utime + stime;

        // CLK_TCK（通常为 100）
        let clk_tck = unsafe { libc::sysconf(libc::_SC_CLK_TCK) } as f64;
        if clk_tck <= 0.0 {
            return None;
        }

        // 读系统 uptime
        let uptime_str = std::fs::read_to_string("/proc/uptime").ok()?;
        let uptime_secs: f64 = uptime_str.split_whitespace().next()?.parse().ok()?;
        if uptime_secs <= 0.0 {
            return None;
        }

        // CPU 核心数
        let cores = num_cpus::get() as f64;
        if cores <= 0.0 {
            return None;
        }

        // CPU% ≈ (total_ticks / clk_tck) / uptime_secs * 100 / cores
        let cpu_time_secs = total_ticks as f64 / clk_tck;
        let pct = cpu_time_secs / uptime_secs * 100.0 / cores;

        // 保留 2 位小数
        Some((pct * 100.0).round() / 100.0)
    }

    /// macOS: 使用 ps -eo pid,comm,rss,%cpu 列出所有进程并采集 RSS 与 CPU
    #[cfg(target_os = "macos")]
    fn detect_macos(targets: &[String]) -> Vec<AgentProcess> {
        let mut agents = Vec::new();

        let output = match std::process::Command::new("ps")
            .args(["-eo", "pid,comm,rss,%cpu"])
            .output()
        {
            Ok(o) if o.status.success() => o,
            _ => return agents,
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines().skip(1) {
            // 跳过标题行 "  PID COMM          RSS %CPU"
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // 输出格式示例: "1234 hermes 45678 2.5"
            // 使用 split_whitespace 分割，最后两个字段是 RSS(KB) 和 %CPU
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 4 {
                continue;
            }

            // PID 为第一个字段，进程名为第二个字段
            let pid: u32 = match parts[0].parse() {
                Ok(p) => p,
                Err(_) => continue,
            };
            let name = parts[1].to_string();

            // RSS 为倒数第二个字段（KB），转 MB
            let memory_rss_mb = parts[parts.len() - 2]
                .parse::<u64>()
                .ok()
                .map(|kb| kb / 1024);

            // %CPU 为最后一个字段
            let cpu_percent: Option<f64> = parts[parts.len() - 1].parse().ok();

            if targets.iter().any(|t| name.contains(t) || name == *t) {
                agents.push(AgentProcess {
                    name,
                    pid,
                    memory_rss_mb,
                    cpu_percent,
                });
            }
        }

        agents
    }
}

impl ResourceCollector for MultiAgentDetector {
    fn collect(&self) -> Result<CollectorOutput, CollectError> {
        let agents = self.detect();
        let count = agents.len();

        // V0.4: 计算所有 Agent 的内存和 CPU 总和
        let total_agent_memory_mb: Option<u64> = {
            let sum: u64 = agents.iter().filter_map(|a| a.memory_rss_mb).sum();
            if count > 0 {
                Some(sum)
            } else {
                None
            }
        };
        let total_agent_cpu_percent: f64 = agents.iter().filter_map(|a| a.cpu_percent).sum();

        Ok(CollectorOutput::Agent(AgentDetection {
            agents,
            count,
            total_agent_memory_mb,
            total_agent_cpu_percent,
            global_pressure: None,
            note: "Agent process detection for reference only (CR-06). Only process names are read, not command-line arguments.".to_string(),
        }))
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collector::{CollectorOutput, ResourceCollector};

    // UT-AG-001: 空 extra_processes 不崩溃
    #[test]
    fn test_ut_ag_001_empty_extra() {
        let detector = MultiAgentDetector::new(None);
        let result = detector.collect();
        assert!(result.is_ok(), "空配置不应崩溃: {:?}", result.err());
    }

    // UT-AG-002: targets 列表格式正确
    #[test]
    fn test_ut_ag_002_targets_format() {
        let detector = MultiAgentDetector::new(None);
        let targets = detector.target_names();
        // 至少包含内置列表
        assert!(targets.len() >= 7, "应有至少 7 个内置 Agent");
        assert!(targets.contains(&"hermes".to_string()));
        assert!(targets.contains(&"claude-code".to_string()));
        assert!(targets.contains(&"deepseek-tui".to_string()));
        assert!(targets.contains(&"reasonix".to_string()));
    }

    // UT-AG-003: extra_processes 追加到 targets
    #[test]
    fn test_ut_ag_003_extra_targets() {
        let detector =
            MultiAgentDetector::new(Some(vec!["my-agent".to_string(), "test".to_string()]));
        let targets = detector.target_names();
        assert!(
            targets.contains(&"my-agent".to_string()),
            "extra 应包含 my-agent"
        );
        assert!(targets.contains(&"hermes".to_string()), "内置列表仍在");
    }

    // UT-AG-004: detection note 包含 CR-06 说明
    #[test]
    fn test_ut_ag_004_note_reference_only() {
        let detector = MultiAgentDetector::new(None);
        let result = detector.collect().unwrap();
        if let CollectorOutput::Agent(detection) = result {
            assert!(
                detection.note.contains("reference only"),
                "note 应包含 'reference only': {}",
                detection.note
            );
        } else {
            panic!("应返回 Agent 变体");
        }
    }

    // UT-MAV-007: 自定义 Agent 名称替代内置 KNOWN_AGENTS
    #[test]
    fn test_ut_mav_007_custom_agent_names() {
        let mut detector = MultiAgentDetector::new(None);
        detector.set_custom_names(Some(vec!["my-custom-agent".to_string()]));
        let targets = detector.target_names();
        // 应只包含自定义名称（不含内置的 hermes 等）
        assert_eq!(targets.len(), 1, "自定义名称应仅有 1 个目标");
        assert!(targets.contains(&"my-custom-agent".to_string()));
        assert!(!targets.contains(&"hermes".to_string()), "内置名称不应存在");
        // extra_processes 仍可追加
        let mut detector2 = MultiAgentDetector::new(Some(vec!["extra-agent".to_string()]));
        detector2.set_custom_names(Some(vec!["custom1".to_string(), "custom2".to_string()]));
        let targets2 = detector2.target_names();
        assert_eq!(targets2.len(), 3, "2 自定义 + 1 extra = 3");
        assert!(targets2.contains(&"custom1".to_string()));
        assert!(targets2.contains(&"extra-agent".to_string()));
        assert!(!targets2.contains(&"hermes".to_string()));
    }

    // UT-MAV-008 variant: 空 agents 时 collect 返回 total_agent_memory_mb = None
    #[test]
    fn test_ut_mav_008_collect_no_agents() {
        let detector = MultiAgentDetector::new(None);
        let result = detector.collect().unwrap();
        if let CollectorOutput::Agent(detection) = result {
            // 不断言 count == 0（依赖平台，可能有匹配进程）
            // 只需验证结构体字段存在且类型正确
            let _ = detection.total_agent_memory_mb;
            let _ = detection.total_agent_cpu_percent;
            // 验证 JSON 序列化正常工作
            let json = serde_json::to_value(&detection).unwrap();
            assert!(
                json.get("total_agent_cpu_percent").is_some(),
                "total_agent_cpu_percent 必须输出"
            );
            assert!(json.get("agents").is_some(), "agents 必须输出");
            assert!(json.get("count").is_some(), "count 必须输出");
        } else {
            panic!("应返回 Agent 变体");
        }
    }
}
