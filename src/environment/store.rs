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

//! 环境指纹存储模块
//!
//! 管理 fingerprint 文件的存储、加载和历史轮转。
//!
//! **存储路径**：`~/.config/hawk-eye-mem/environment.json`
//! **历史轮转**：保留最近3次，文件名为 environment.json / environment.1.json / environment.2.json
//! **完整性保护**：HMAC-SHA256 签名（require `ring`，当前用 SHA256 hash 替代）

use crate::environment::EnvironmentFingerprint;
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// 指纹存储管理
pub struct FingerprintStore {
    /// 指纹文件目录
    base_dir: PathBuf,
}

/// HMAC 签名及指纹数据
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct StoredFingerprint {
    /// 指纹本体
    fingerprint: EnvironmentFingerprint,
    /// SHA256 签名（数据完整性校验）
    signature: String,
}

/// 生成简单签名：SHA256(fingerprint_json + machine_id)
fn generate_signature(fp_json: &str, machine_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(fp_json.as_bytes());
    hasher.update(machine_id.as_bytes());
    hex::encode(hasher.finalize())
}

/// 尝试获取 machine-id（Linux）或等效标识
fn get_machine_id() -> String {
    // Linux /etc/machine-id
    if let Ok(id) = std::fs::read_to_string("/etc/machine-id") {
        return id.trim().to_string();
    }
    // macOS: hostname作为回退
    if let Ok(hostname) = std::env::var("HOSTNAME") {
        return hostname;
    }
    // 尝试 /etc/hostname
    if let Ok(content) = std::fs::read_to_string("/etc/hostname") {
        return content.trim().to_string();
    }
    "unknown-machine".to_string()
}

impl FingerprintStore {
    /// 创建指纹存储，路径为 `~/.config/hawk-eye-mem/`
    pub fn new() -> Self {
        let home = dirs_next::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let base_dir = home.join(".config/hawk-eye-mem");
        Self { base_dir }
    }

    /// 指定目录创建
    #[allow(dead_code)]
    pub fn new_with_dir(dir: PathBuf) -> Self {
        Self { base_dir: dir }
    }

    /// 获取当前指纹文件路径
    fn current_path(&self) -> PathBuf {
        self.base_dir.join("environment.json")
    }

    /// 获取历史指纹文件路径（0=当前, 1=历史1, 2=历史2）
    fn history_path(&self, index: u8) -> PathBuf {
        if index == 0 {
            self.current_path()
        } else {
            self.base_dir.join(format!("environment.{}.json", index))
        }
    }

    /// 保存指纹并轮转（保留最近3次）
    pub fn save(&self, fingerprint: &EnvironmentFingerprint) -> Result<()> {
        // 创建目录
        std::fs::create_dir_all(&self.base_dir)
            .with_context(|| format!("Failed to create dir: {}", self.base_dir.display()))?;

        // 轮转：environment.2.json → 删除
        let p2 = self.history_path(2);
        if p2.exists() {
            let _ = std::fs::remove_file(&p2);
        }
        // environment.1.json → environment.2.json
        let p1 = self.history_path(1);
        if p1.exists() {
            let _ = std::fs::rename(&p1, &p2);
        }
        // environment.json → environment.1.json
        let p0 = self.current_path();
        if p0.exists() {
            let _ = std::fs::rename(&p0, &p1);
        }

        // 生成签名并写入
        let fp_json =
            serde_json::to_string(fingerprint).context("Failed to serialize fingerprint")?;
        let machine_id = get_machine_id();
        let signature = generate_signature(&fp_json, &machine_id);

        let stored = StoredFingerprint {
            fingerprint: fingerprint.clone(),
            signature,
        };

        let content = serde_json::to_string_pretty(&stored)
            .context("Failed to serialize stored fingerprint")?;
        std::fs::write(&p0, content)
            .with_context(|| format!("Failed to write fingerprint: {}", p0.display()))?;

        Ok(())
    }

    /// 加载当前指纹（含签名验证）
    pub fn load_current(&self) -> Result<Option<EnvironmentFingerprint>> {
        self.load_index(0)
    }

    /// 加载第 index 个历史指纹（0=当前, 1=历史1, 2=历史2）
    fn load_index(&self, index: u8) -> Result<Option<EnvironmentFingerprint>> {
        let path = self.history_path(index);
        if !path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read: {}", path.display()))?;

        let stored: StoredFingerprint = serde_json::from_str(&content)
            .context("Failed to parse fingerprint file (may have been corrupted)")?;

        // 签名验证
        let fp_json = serde_json::to_string(&stored.fingerprint)
            .context("Failed to serialize for signature check")?;
        let machine_id = get_machine_id();
        let expected_sig = generate_signature(&fp_json, &machine_id);

        if stored.signature != expected_sig {
            // 验签失败：可能文件被篡改或在不同机器间复制
            eprintln!(
                "Warning: Environment fingerprint signature mismatch — \
                 file may have been tampered with or copied from another machine. \
                 A new fingerprint will be generated."
            );
            // UT-ENV-031: 验签失败时返回 None，触发重新生成
            return Ok(None);
        }

        Ok(Some(stored.fingerprint))
    }

    /// 加载前一次指纹（environment.1.json），用于变更检测
    pub fn load_previous(&self) -> Result<Option<EnvironmentFingerprint>> {
        self.load_index(1)
    }

    /// 重置所有指纹文件
    pub fn reset(&self) -> Result<()> {
        for i in 0..=2u8 {
            let path = self.history_path(i);
            if path.exists() {
                std::fs::remove_file(&path)
                    .with_context(|| format!("Failed to remove: {}", path.display()))?;
            }
        }
        Ok(())
    }

    /// 检查是否有已保存的指纹
    #[allow(dead_code)]
    pub fn has_current(&self) -> bool {
        self.current_path().exists()
    }
}

impl Default for FingerprintStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // UT-ENV-030: 指纹文件轮转
    #[test]
    fn test_ut_env_030_rotation() {
        let tmp = TempDir::new().unwrap();
        let store = FingerprintStore::new_with_dir(tmp.path().join("hawkeye"));

        let fp1 = EnvironmentFingerprint::generate("host", "linux", 4, 8192, vec![], 100_000, None);
        let fp2 =
            EnvironmentFingerprint::generate("host", "linux", 8, 16384, vec![], 200_000, None);
        let fp3 =
            EnvironmentFingerprint::generate("host", "linux", 16, 32768, vec![], 300_000, None);
        let fp4 =
            EnvironmentFingerprint::generate("host", "linux", 32, 65536, vec![], 400_000, None);

        // 保存4次 → 应保留最近3次
        store.save(&fp1).unwrap();
        store.save(&fp2).unwrap();
        store.save(&fp3).unwrap();
        store.save(&fp4).unwrap();

        // 当前指纹 = fp4
        let current = store.load_current().unwrap().unwrap();
        assert_eq!(current.total_memory_mb, 65536);

        // 前一次 = fp3
        let prev = store.load_previous().unwrap().unwrap();
        assert_eq!(prev.total_memory_mb, 32768);

        // 历史2 = fp2
        let hist2 = store.load_index(2).unwrap().unwrap();
        assert_eq!(hist2.total_memory_mb, 16384);
    }

    // UT-ENV-031: HMAC 签名验证
    #[test]
    fn test_ut_env_031_signature_validation() {
        let tmp = TempDir::new().unwrap();
        let store = FingerprintStore::new_with_dir(tmp.path().join("hawkeye2"));

        let fp = EnvironmentFingerprint::generate("host", "linux", 4, 8192, vec![], 100_000, None);
        store.save(&fp).unwrap();

        // 篡改文件
        let path = store.current_path();
        let content = std::fs::read_to_string(&path).unwrap();
        let tampered = content.replace("8192", "16384");
        std::fs::write(&path, tampered).unwrap();

        // 验签应失败 → 返回 None
        let result = store.load_current().unwrap();
        assert!(
            result.is_none(),
            "Tampered file should fail signature check"
        );
    }

    // UT-ENV-032: 签名密钥基于机器ID
    #[test]
    fn test_ut_env_032_signature_machine_dependent() {
        let tmp = TempDir::new().unwrap();
        let store = FingerprintStore::new_with_dir(tmp.path().join("hawkeye3"));

        let fp = EnvironmentFingerprint::generate("host", "linux", 4, 8192, vec![], 100_000, None);
        store.save(&fp).unwrap();

        // 只要文件在当前机器上就能正常读取
        let result = store.load_current().unwrap();
        assert!(result.is_some(), "Same machine should verify signature OK");
    }

    // UT-ENV-034: 重置
    #[test]
    fn test_ut_env_034_reset() {
        let tmp = TempDir::new().unwrap();
        let store = FingerprintStore::new_with_dir(tmp.path().join("hawkeye4"));

        let fp = EnvironmentFingerprint::generate("host", "linux", 4, 8192, vec![], 100_000, None);
        store.save(&fp).unwrap();
        assert!(store.has_current());

        store.reset().unwrap();
        assert!(!store.has_current());
    }
}
