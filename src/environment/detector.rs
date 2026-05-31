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

//! 变更检测模块
//!
//! 实现 `detect_changes()` 的阈值判定逻辑（CR-02）：
//! - 内存：变化 > 4GB 或 > 20% 总内存（取较大者）
//! - CPU：变化 ≥ 2 核心
//! - GPU：增/减
//! - 磁盘：变化 ≥ 100GB 或 ≥ 30%
//! - 容器运行环境：变化

use crate::environment::{EnvironmentChange, EnvironmentFingerprint};

/// 执行变更检测，返回变更列表
#[allow(dead_code)]
pub fn detect_changes(
    current: &EnvironmentFingerprint,
    previous: &EnvironmentFingerprint,
) -> Vec<EnvironmentChange> {
    current.detect_changes(previous)
}

/// 判断变更报告中是否有实质性的升级/降级
#[allow(dead_code)]
pub fn has_significant_change(changes: &[EnvironmentChange]) -> bool {
    !changes.is_empty()
}

/// 合并多次变化并生成最终报告
#[allow(dead_code)]
pub fn build_change_report(
    _current_fp: &EnvironmentFingerprint,
    previous_fp: &EnvironmentFingerprint,
    changes: Vec<EnvironmentChange>,
) -> crate::environment::EnvironmentChangeReport {
    let recommendation = if changes.is_empty() {
        None
    } else {
        Some(EnvironmentFingerprint::generate_recommendation(&changes))
    };

    crate::environment::EnvironmentChangeReport {
        detected: !changes.is_empty(),
        previous_fingerprint_id: Some(previous_fp.id.clone()),
        changes,
        new_recommendation: recommendation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::EnvironmentFingerprint;

    // UT-ENV-017: 磁盘大幅变化触发
    #[test]
    fn test_ut_env_017_disk_big_change() {
        let old =
            EnvironmentFingerprint::generate("host", "linux", 4, 16384, vec![], 100_000, None);
        let new =
            EnvironmentFingerprint::generate("host", "linux", 4, 16384, vec![], 500_000, None);
        // 100GB→500GB: 差400GB > 100GB → 触发
        let changes = detect_changes(&new, &old);
        let disk_changes: Vec<_> = changes.iter().filter(|c| c.resource == "disk").collect();
        assert_eq!(disk_changes.len(), 1);
        assert_eq!(disk_changes[0].direction, "upgrade");
    }

    // UT-ENV-018: 容器环境切换
    #[test]
    fn test_ut_env_018_container_change() {
        let old = EnvironmentFingerprint::generate("host", "linux", 4, 8192, vec![], 100_000, None);
        let new = EnvironmentFingerprint::generate(
            "host",
            "linux",
            4,
            8192,
            vec![],
            100_000,
            Some("docker".to_string()),
        );
        let changes = detect_changes(&new, &old);
        let container_changes: Vec<_> = changes
            .iter()
            .filter(|c| c.resource == "container")
            .collect();
        assert_eq!(container_changes.len(), 1);
    }

    // 无变化
    #[test]
    fn test_no_changes_identical() {
        let fp = EnvironmentFingerprint::generate(
            "host",
            "linux",
            4,
            8192,
            vec!["GPU".to_string()],
            100_000,
            None,
        );
        let changes = detect_changes(&fp, &fp);
        assert!(changes.is_empty());
    }

    // build_change_report
    #[test]
    fn test_build_report_no_changes() {
        let fp = EnvironmentFingerprint::generate("host", "linux", 4, 8192, vec![], 100_000, None);
        let report = build_change_report(&fp, &fp, vec![]);
        assert!(!report.detected);
        assert!(report.new_recommendation.is_none());
    }

    #[test]
    fn test_build_report_with_changes() {
        let old =
            EnvironmentFingerprint::generate("host", "linux", 4, 16384, vec![], 100_000, None);
        let new =
            EnvironmentFingerprint::generate("host", "linux", 4, 65536, vec![], 100_000, None);
        let changes = detect_changes(&new, &old);
        let report = build_change_report(&new, &old, changes);
        assert!(report.detected);
        assert!(report.new_recommendation.is_some());
    }
}
