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
// src/stats.rs — 本地埋点统计模块
//
// 记录 CLI 每次调用的命令名、总调用次数、异常退出次数。
// 存储路径: ~/.config/hawk-eye-mem/usage_stats.json
// 使用 flock 保证并发安全（参考 trends/mod.rs 的 HistoryStore 实现模式）。
// ============================================================================

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;

// ============================================================================
// 数据结构
// ============================================================================

/// 本地埋点统计数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageStats {
    pub total_runs: u64,
    pub commands: HashMap<String, u64>,
    pub errors: u64,
}

impl Default for UsageStats {
    fn default() -> Self {
        Self {
            total_runs: 0,
            commands: HashMap::new(),
            errors: 0,
        }
    }
}

// ============================================================================
// 存储路径
// ============================================================================

/// 获取统计数据文件的存储路径
fn stats_path() -> PathBuf {
    let home = dirs_next::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join(".config/hawk-eye-mem/usage_stats.json")
}

/// 确保存储目录存在
fn ensure_dir() -> Result<(), String> {
    let path = stats_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create stats directory: {e}"))?;
    }
    Ok(())
}

// ============================================================================
// 核心接口
// ============================================================================

/// 记录一次命令调用
pub fn record(command: &str) {
    let mut stats = load();
    stats.total_runs += 1;
    *stats.commands.entry(command.to_string()).or_insert(0) += 1;
    save(&stats);
}

/// 记录一次异常退出
pub fn record_error() {
    let mut stats = load();
    stats.errors += 1;
    save(&stats);
}

/// 从文件加载统计数据
pub fn load() -> UsageStats {
    let path = stats_path();
    if !path.exists() {
        return UsageStats::default();
    }

    let file = match File::open(&path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!(
                "[hawk-eye-mem] Warning: cannot open stats file: {e}"
            );
            return UsageStats::default();
        }
    };

    // 共享锁 — 允许并发读取
    if let Err(e) = file.lock_shared() {
        eprintln!(
            "[hawk-eye-mem] Warning: failed to lock stats file (shared): {e}"
        );
        return UsageStats::default();
    }

    let mut content = String::new();
    // Use a reference so file is not consumed (needed for unlock below)
    if let Err(e) = (&file).take(1024 * 1024).read_to_string(&mut content) {
        eprintln!(
            "[hawk-eye-mem] Warning: failed to read stats file: {e}"
        );
        let _ = file.unlock();
        return UsageStats::default();
    }

    if let Err(e) = file.unlock() {
        eprintln!(
            "[hawk-eye-mem] Warning: failed to unlock stats file: {e}"
        );
    }

    match serde_json::from_str::<UsageStats>(&content) {
        Ok(stats) => stats,
        Err(e) => {
            eprintln!(
                "[hawk-eye-mem] Warning: malformed stats file ({e}), resetting"
            );
            UsageStats::default()
        }
    }
}

/// 保存统计数据到文件（互斥锁）
pub fn save(stats: &UsageStats) {
    if let Err(e) = ensure_dir() {
        eprintln!("[hawk-eye-mem] Warning: {e}");
        return;
    }

    let path = stats_path();
    let file = match OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)
    {
        Ok(f) => f,
        Err(e) => {
            eprintln!(
                "[hawk-eye-mem] Warning: cannot open stats file for writing: {e}"
            );
            return;
        }
    };

    // 互斥锁 — 写操作独占
    if let Err(e) = file.lock_exclusive() {
        eprintln!(
            "[hawk-eye-mem] Warning: failed to lock stats file (exclusive): {e}"
        );
        return;
    }

    let json = match serde_json::to_string_pretty(stats) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "[hawk-eye-mem] Warning: failed to serialize stats: {e}"
            );
            let _ = file.unlock();
            return;
        }
    };

    if let Err(e) = writeln!(&file, "{json}") {
        eprintln!(
            "[hawk-eye-mem] Warning: failed to write stats file: {e}"
        );
    }

    if let Err(e) = file.unlock() {
        eprintln!(
            "[hawk-eye-mem] Warning: failed to unlock stats file: {e}"
        );
    }
}

impl UsageStats {
    /// 保存此 UsageStats 实例到文件
    pub fn save(&self) {
        save(self);
    }
}

/// 清空所有数据
pub fn reset() {
    let empty = UsageStats::default();
    save(&empty);
}

/// 格式化输出统计数据
pub fn print_stats() {
    let stats = load();

    println!("📊 秋毫mem 使用统计");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    if stats.total_runs == 0 {
        println!("暂无数据 — 开始使用后自动记录");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("所有数据本地存储 · 零 Token 消耗");
        return;
    }

    println!("总运行次数:   {} 次", format_number(stats.total_runs));

    // 按调用次数降序排列命令
    let mut sorted_commands: Vec<(&String, &u64)> = stats.commands.iter().collect();
    sorted_commands.sort_by(|a, b| b.1.cmp(a.1));

    // 计算"其他"分组：只显示 top 5，其余归入"其他"
    let top_n = 5;
    let displayed: u64 = sorted_commands
        .iter()
        .take(top_n)
        .map(|(_, &count)| count)
        .sum();
    let other_count = stats.total_runs.saturating_sub(displayed);

    for (cmd, &count) in sorted_commands.iter().take(top_n) {
        let pct = (count as f64 / stats.total_runs as f64) * 100.0;
        println!(
            "  --{:<15} {:>6} 次  ({:.1}%)",
            cmd,
            format_number(count),
            pct
        );
    }

    if other_count > 0 {
        let pct = (other_count as f64 / stats.total_runs as f64) * 100.0;
        println!(
            "  其他{:<17} {:>6} 次  ({:.1}%)",
            "",
            format_number(other_count),
            pct
        );
    }

    // 异常统计
    let error_pct = (stats.errors as f64 / stats.total_runs as f64) * 100.0;
    println!();
    println!(
        "异常次数:      {} 次  ({:.1}%)",
        format_number(stats.errors),
        error_pct
    );

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("所有数据本地存储 · 零 Token 消耗");
}

// ============================================================================
// 工具函数
// ============================================================================

/// 将数字格式化为带千分位分隔符的字符串
fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建一个用于测试的临时 stats 路径
    fn test_path() -> PathBuf {
        PathBuf::from("/tmp/hawk-eye-mem-test-stats.json")
    }

    /// 测试：从空文件加载应返回默认值
    #[test]
    fn test_load_empty() {
        let _ = fs::remove_file(test_path());
        let stats = load();
        assert_eq!(stats.total_runs, 0);
        assert!(stats.commands.is_empty());
        assert_eq!(stats.errors, 0);
    }

    /// 测试：reset 后数据应清空
    #[test]
    fn test_reset() {
        // 先写入一些数据
        let mut stats = UsageStats::default();
        stats.total_runs = 100;
        stats.commands.insert("json".to_string(), 50);
        stats.errors = 2;
        save(&stats);

        // 重置
        reset();

        // 验证已清空
        let loaded = load();
        assert_eq!(loaded.total_runs, 0);
        assert!(loaded.commands.is_empty());
        assert_eq!(loaded.errors, 0);
    }

    /// 测试：record 能正确记录命令
    #[test]
    fn test_record() {
        reset();

        record("json");
        record("heartbeat");
        record("json");

        let stats = load();
        assert_eq!(stats.total_runs, 3);
        assert_eq!(*stats.commands.get("json").unwrap(), 2);
        assert_eq!(*stats.commands.get("heartbeat").unwrap(), 1);
    }

    /// 测试：record_error 递增错误计数
    #[test]
    fn test_record_error() {
        reset();

        record("json");
        record_error();
        record_error();

        let stats = load();
        assert_eq!(stats.total_runs, 1);
        assert_eq!(stats.errors, 2);
    }

    /// 测试：format_number 千分位格式化
    #[test]
    fn test_format_number() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(1), "1");
        assert_eq!(format_number(12), "12");
        assert_eq!(format_number(123), "123");
        assert_eq!(format_number(1234), "1,234");
        assert_eq!(format_number(12345), "12,345");
        assert_eq!(format_number(1234567), "1,234,567");
    }

    /// 测试：print_stats 不会 panic
    #[test]
    fn test_print_stats_no_panic() {
        reset();

        // 空数据
        print_stats();

        // 有数据
        record("json");
        record("heartbeat");
        record("json");
        print_stats();
    }

    /// 测试：save 方法语法
    #[test]
    fn test_save_method() {
        reset();

        let mut stats = UsageStats::default();
        stats.total_runs = 42;
        stats.commands.insert("test".to_string(), 42);
        stats.save();

        let loaded = load();
        assert_eq!(loaded.total_runs, 42);
    }
}
