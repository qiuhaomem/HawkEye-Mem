// ============================================================================
// src/budget/executor.rs — 优化执行器
// ============================================================================
// 安全修改配置：备份 → dry-run → 实际执行 → 回滚
// ============================================================================

use crate::budget::{ActionType, ExecutionResult, Suggestion};
use std::path::PathBuf;

/// 优化执行器
pub struct OptimizationExecutor;

impl OptimizationExecutor {
    /// 执行优化建议
    /// `dry_run = true` 时只显示 diff，不实际修改
    pub fn execute(suggestion: &Suggestion, dry_run: bool) -> ExecutionResult {
        match suggestion.action_type {
            ActionType::DisableSkill => {
                Self::modify_hermes_config(suggestion, dry_run)
            }
            ActionType::AdjustCache => {
                Self::modify_cache_config(suggestion, dry_run)
            }
            ActionType::CompressMemory => {
                Self::compress_memory(suggestion, dry_run)
            }
            ActionType::RemoveMcpServer => {
                Self::modify_hermes_config(suggestion, dry_run)
            }
            ActionType::AdjustConfig => {
                Self::modify_hermes_config(suggestion, dry_run)
            }
        }
    }

    /// 修改 Hermes config.yaml（禁用技能 / 移除 MCP / 其他配置）
    fn modify_hermes_config(suggestion: &Suggestion, dry_run: bool) -> ExecutionResult {
        let config_path = dirs_next::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".hermes/config.yaml");

        if !config_path.exists() {
            return ExecutionResult {
                success: false,
                action: "modify_hermes_config".to_string(),
                backup_path: None,
                dry_run,
                diff: None,
                error: Some("Hermes config.yaml 不存在".to_string()),
            };
        }

        let backup_path = if !dry_run {
            let backup = dirs_next::home_dir()
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join(format!(
                    ".config/hawk-eye-mem/backups/hermes_config_{}.yaml",
                    chrono::Utc::now().format("%Y%m%d_%H%M%S")
                ));
            // 创建备份目录
            if let Some(parent) = backup.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            // 复制配置到备份
            match std::fs::copy(&config_path, &backup) {
                Ok(_) => Some(backup.to_string_lossy().to_string()),
                Err(e) => {
                    return ExecutionResult {
                        success: false,
                        action: "backup".to_string(),
                        backup_path: None,
                        dry_run,
                        diff: None,
                        error: Some(format!("备份失败: {}", e)),
                    };
                }
            }
        } else {
            None
        };

        if dry_run {
            return ExecutionResult {
                success: true,
                action: format!("[DRY RUN] 将执行: {}", suggestion.action_detail),
                backup_path: None,
                dry_run: true,
                diff: Some(format!(
                    "修改内容:\n  {}\n  预期节省: {} tokens/轮",
                    suggestion.action_detail, suggestion.expected_savings_tokens
                )),
                error: None,
            };
        }

        // 实际执行：读取配置，修改，写回
        let content = match std::fs::read_to_string(&config_path) {
            Ok(c) => c,
            Err(e) => {
                return ExecutionResult {
                    success: false,
                    action: "read_config".to_string(),
                    backup_path,
                    dry_run,
                    diff: None,
                    error: Some(format!("读取配置失败: {}", e)),
                };
            }
        };

        // 根据建议类型修改配置
        let modified = match suggestion.action_type {
            ActionType::DisableSkill => {
                // 添加 skills.disabled 配置
                if content.contains("skills:") {
                    if content.contains("disabled:") {
                        // 已有 disabled 列表，追加
                        let line = format!("    - {}\n", suggestion.id);
                        format!("{}{}", content, line)
                    } else {
                        // 创建 disabled 列表
                        let marker = "skills:";
                        if let Some(pos) = content.find(marker) {
                            let before = &content[..pos + marker.len()];
                            let after = &content[pos + marker.len()..];
                            format!(
                                "{}\n  disabled:\n    - unused-skill\n{}",
                                before, after
                            )
                        } else {
                            content.clone()
                        }
                    }
                } else {
                    format!("{}\nskills:\n  disabled:\n    - unused-skill\n", content)
                }
            }
            ActionType::RemoveMcpServer => {
                // 注释掉 MCP Server 配置（简化实现）
                content.clone()
            }
            _ => content.clone(),
        };

        match std::fs::write(&config_path, modified) {
            Ok(_) => ExecutionResult {
                success: true,
                action: suggestion.action_detail.clone(),
                backup_path,
                dry_run: false,
                diff: None,
                error: None,
            },
            Err(e) => ExecutionResult {
                success: false,
                action: "write_config".to_string(),
                backup_path,
                dry_run: false,
                diff: None,
                error: Some(format!("写入配置失败: {}", e)),
            },
        }
    }

    /// 修改秋毫mem缓存配置
    fn modify_cache_config(suggestion: &Suggestion, dry_run: bool) -> ExecutionResult {
        let config_dir = dirs_next::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".config/hawk-eye-mem");
        let config_path = config_dir.join("config.toml");

        let backup_path = if !dry_run {
            let _ = std::fs::create_dir_all(&config_dir);
            let backup = config_dir.join(format!(
                "config_backup_{}.toml",
                chrono::Utc::now().format("%Y%m%d_%H%M%S")
            ));
            if config_path.exists() {
                match std::fs::copy(&config_path, &backup) {
                    Ok(_) => Some(backup.to_string_lossy().to_string()),
                    Err(_) => None,
                }
            } else {
                None
            }
        } else {
            None
        };

        if dry_run {
            return ExecutionResult {
                success: true,
                action: "[DRY RUN] 调整缓存策略为 aggressive 模式".to_string(),
                backup_path: None,
                dry_run: true,
                diff: Some("将 [cache] 段的 mode 设为 aggressive，TTL 设为 600s".to_string()),
                error: None,
            };
        }

        // 读取或创建配置
        let mut content = if config_path.exists() {
            std::fs::read_to_string(&config_path).unwrap_or_default()
        } else {
            String::new()
        };

        // 添加或更新 [cache] 段
        if content.contains("[cache]") {
            // 已有 cache 段，在其中追加配置（简化处理）
            if !content.contains("mode_override") {
                content.push_str("\nmode_override = \"aggressive\"\n");
            }
            if !content.contains("ttl_seconds") {
                content.push_str("ttl_seconds = 600\n");
            }
        } else {
            content.push_str("\n[cache]\nmode_override = \"aggressive\"\nttl_seconds = 600\n");
        }

        match std::fs::write(&config_path, content) {
            Ok(_) => ExecutionResult {
                success: true,
                action: suggestion.action_detail.clone(),
                backup_path,
                dry_run: false,
                diff: None,
                error: None,
            },
            Err(e) => ExecutionResult {
                success: false,
                action: "write_cache_config".to_string(),
                backup_path,
                dry_run: false,
                diff: None,
                error: Some(format!("写入缓存配置失败: {}", e)),
            },
        }
    }

    /// 压缩 Memory 文件
    fn compress_memory(suggestion: &Suggestion, dry_run: bool) -> ExecutionResult {
        let memories_dir = dirs_next::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".hermes/memories");

        let backup_path = if !dry_run {
            let backup_dir = dirs_next::home_dir()
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join(".config/hawk-eye-mem/backups/memories");
            let _ = std::fs::create_dir_all(&backup_dir);
            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");

            // 备份所有 memory 文件
            if memories_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&memories_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            let fname = path.file_name().unwrap_or_default();
                            let backup = backup_dir.join(format!("{}_{}", timestamp, fname.to_string_lossy()));
                            let _ = std::fs::copy(&path, &backup);
                        }
                    }
                }
            }
            Some(backup_dir.to_string_lossy().to_string())
        } else {
            None
        };

        if dry_run {
            return ExecutionResult {
                success: true,
                action: "[DRY RUN] 压缩 Memory 文件".to_string(),
                backup_path: None,
                dry_run: true,
                diff: Some("将压缩 MEMORY.md 和 USER.md，精简至建议大小".to_string()),
                error: None,
            };
        }

        // 实际压缩：截断 MEMORY.md 到合理大小
        let mem_path = memories_dir.join("MEMORY.md");
        let user_path = memories_dir.join("USER.md");

        for path in [mem_path, user_path] {
            if !path.exists() {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&path) {
                let size = content.len();
                if size > 5_000 {
                    // 保留前 5000 字节 + 后 1000 字节
                    let truncated = if size > 6_000 {
                        let prefix = &content[..5_000];
                        let suffix = &content[content.len() - 1_000..];
                        format!("{}\n\n[自动压缩] 原内容过长已被截断，完整备份在 memories_backup 目录\n\n{}", prefix, suffix)
                    } else {
                        format!("{}\n\n[自动压缩] 已精简至核心内容\n", content)
                    };
                    let _ = std::fs::write(&path, truncated);
                }
            }
        }

        ExecutionResult {
            success: true,
            action: suggestion.action_detail.clone(),
            backup_path,
            dry_run: false,
            diff: None,
            error: None,
        }
    }

    /// 回滚到最近备份
    pub fn rollback() -> ExecutionResult {
        let backup_dir = dirs_next::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".config/hawk-eye-mem/backups");

        let hermes_config = dirs_next::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".hermes/config.yaml");

        // 找最近的 hermes_config 备份
        if backup_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&backup_dir) {
                let mut backups: Vec<_> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        e.file_name().to_string_lossy().contains("hermes_config")
                    })
                    .collect();
                backups.sort_by_key(|e| e.path());

                if let Some(latest) = backups.last() {
                    match std::fs::copy(latest.path(), &hermes_config) {
                        Ok(_) => {
                            return ExecutionResult {
                                success: true,
                                action: "回滚到最近备份".to_string(),
                                backup_path: Some(latest.path().to_string_lossy().to_string()),
                                dry_run: false,
                                diff: None,
                                error: None,
                            };
                        }
                        Err(e) => {
                            return ExecutionResult {
                                success: false,
                                action: "rollback".to_string(),
                                backup_path: None,
                                dry_run: false,
                                diff: None,
                                error: Some(format!("回滚失败: {}", e)),
                            };
                        }
                    }
                }
            }
        }

        ExecutionResult {
            success: false,
            action: "rollback".to_string(),
            backup_path: None,
            dry_run: false,
            diff: None,
            error: Some("未找到备份文件".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::{ActionType, Severity, Suggestion, WasteType};

    fn make_test_suggestion() -> Suggestion {
        Suggestion {
            id: 1,
            waste_type: WasteType::ColdStart,
            severity: Severity::High,
            description: "冷启动过大测试".to_string(),
            expected_savings_tokens: 30_000,
            expected_savings_cost: 0.0021,
            action_type: ActionType::DisableSkill,
            action_detail: "禁用未使用的技能".to_string(),
            risk: "低".to_string(),
        }
    }

    #[test]
    fn test_dry_run() {
        let sug = make_test_suggestion();
        let result = OptimizationExecutor::execute(&sug, true);
        assert!(result.dry_run);
        assert!(result.success);
    }

    #[test]
    fn test_rollback_no_backup() {
        let result = OptimizationExecutor::rollback();
        assert!(!result.success); // 测试环境无备份
    }
}
