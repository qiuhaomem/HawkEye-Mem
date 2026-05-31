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

// 校准引擎模块 - V0.3 动态校准核心

use anyhow::Result;

pub mod algorithm;
pub mod csv_store;
#[derive(Debug, Clone)]
pub struct CalibrationPoint {
    pub timestamp: u64,        // Unix timestamp（秒）
    pub bytes_per_token: u64,  // 单次推理的KV cache字节数
    pub tokens_processed: u64, // 本次推理处理的token数
}

// === CalibrationStore trait ===
#[allow(dead_code)]
pub trait CalibrationStore: Send + Sync {
    /// 追加一条校准数据，model_name是原始模型名（内部会自动哈希）
    fn append(&self, point: CalibrationPoint, model_name: &str) -> Result<()>;
    /// 读取指定模型哈希的所有校准数据，按时间升序排列
    fn read_by_model(&self, model_hash: &str) -> Result<Vec<CalibrationPoint>>;
    /// 清空指定模型哈希的校准数据
    fn clear_model(&self, model_hash: &str) -> Result<()>;
    /// 返回所有已记录的模型哈希列表
    fn model_hashes(&self) -> Result<Vec<String>>;
}

// === 模型名哈希工具 ===
/// 将模型名哈希为16位hex字符串
/// 拼接本机salt（/etc/machine-id前8位）以防止CSV被跨机关联分析
pub fn hash_model_name(name: &str) -> Result<String> {
    use sha2::{Digest, Sha256};

    // 尝试读取本机salt
    let salt = std::fs::read_to_string("/etc/machine-id")
        .ok()
        .and_then(|s| s.trim().get(..8).map(String::from))
        .unwrap_or_default();

    let input = format!("{}:{}", name, salt);
    let hash = Sha256::digest(input.as_bytes());
    Ok(hex::encode(&hash[..8])) // 前8字节 = 16 hex字符
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_consistency() {
        let h1 = hash_model_name("llama3-8b").unwrap();
        let h2 = hash_model_name("llama3-8b").unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 16);
    }

    #[test]
    fn test_hash_different_models() {
        let h1 = hash_model_name("llama3-8b").unwrap();
        let h2 = hash_model_name("deepseek-v3").unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_hash_format() {
        let h = hash_model_name("test-model").unwrap();
        // 必须是小写hex
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(h.len(), 16);
    }
}
