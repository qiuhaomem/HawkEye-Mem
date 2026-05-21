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

use crate::collector::{AgentDetection, AgentProcess, CollectError, CollectorOutput, ResourceCollector};

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
}

impl MultiAgentDetector {
    pub fn new(extra_processes: Option<Vec<String>>) -> Self {
        Self {
            extra_processes: extra_processes.unwrap_or_default(),
        }
    }

    /// 获取完整的待检测进程名列表（内置 + 用户配置）
    fn target_names(&self) -> Vec<String> {
        let mut names: Vec<String> = KNOWN_AGENTS.iter().map(|s| s.to_string()).collect();
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
                // 读取进程内存（RSS）
                let memory_rss_mb = Self::read_proc_memory(pid);
                let cpu_percent = None; // CPU 百分比暂不实现（需要间隔采样）
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

    /// macOS: 使用 ps -eo pid,comm 列出所有进程
    #[cfg(target_os = "macos")]
    fn detect_macos(targets: &[String]) -> Vec<AgentProcess> {
        let mut agents = Vec::new();

        let output = match std::process::Command::new("ps")
            .args(["-eo", "pid,comm"])
            .output()
        {
            Ok(o) if o.status.success() => o,
            _ => return agents,
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines().skip(1) {
            // 跳过标题行 "  PID COMM"
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.splitn(2, char::is_whitespace).collect();
            if parts.len() != 2 {
                continue;
            }

            let pid: u32 = match parts[0].parse() {
                Ok(p) => p,
                Err(_) => continue,
            };
            let name = parts[1].trim().to_string();

            if targets.iter().any(|t| name.contains(t) || name == *t) {
                agents.push(AgentProcess {
                    name,
                    pid,
                    memory_rss_mb: None, // macOS RSS 采集较复杂，暂不实现
                    cpu_percent: None,
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

        // 计算全局压力（所有 Agent 内存和 > 总内存 80% 时标记）
        let _total_rss: u64 = agents.iter().filter_map(|a| a.memory_rss_mb).sum();
        // 暂不判定全局压力（留待 V0.4 实现内存阈值比较）

        Ok(CollectorOutput::Agent(AgentDetection {
            agents,
            count,
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
    use crate::collector::{CollectError, CollectorOutput, ResourceCollector};

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
        let detector = MultiAgentDetector::new(Some(vec!["my-agent".to_string(), "test".to_string()]));
        let targets = detector.target_names();
        assert!(targets.contains(&"my-agent".to_string()), "extra 应包含 my-agent");
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
}
