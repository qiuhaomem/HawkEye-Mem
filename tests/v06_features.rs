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

// tests/v06_features.rs — V0.6 新功能集成测试
// 测试 --heartbeat, --analyze-cache-gaps, --days, --target

use std::process::Command;

fn hawk_eye_binary() -> String {
    // Use debug build for tests
    let path =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug/hawk-eye-mem");
    path.to_string_lossy().to_string()
}

/// UT-V06-001: --heartbeat 输出单行 JSON
#[test]
fn test_heartbeat_output_is_valid_json() {
    let output = Command::new(hawk_eye_binary())
        .arg("--heartbeat")
        .output()
        .expect("Failed to run hawk-eye-mem --heartbeat");

    assert!(output.status.success(), "heartbeat command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stdout = stdout.trim();

    // Must be single line
    assert!(!stdout.is_empty(), "heartbeat output is empty");
    assert_eq!(
        stdout.lines().count(),
        1,
        "heartbeat must output exactly 1 line"
    );

    // Must be valid JSON
    let json: serde_json::Value =
        serde_json::from_str(stdout).expect("heartbeat output is not valid JSON");

    // Must contain required fields
    assert!(json.get("pressure").is_some(), "missing 'pressure' field");
    assert!(
        json.get("available_mb").is_some(),
        "missing 'available_mb' field"
    );
    assert!(
        json.get("used_percent").is_some(),
        "missing 'used_percent' field"
    );
    assert!(json.get("action").is_some(), "missing 'action' field");
    assert!(json.get("timestamp").is_some(), "missing 'timestamp' field");

    // pressure must be one of: low, medium, high, critical
    let pressure = json["pressure"].as_str().unwrap();
    assert!(
        ["low", "medium", "high", "critical"].contains(&pressure),
        "invalid pressure value: {}",
        pressure
    );

    // action must be one of: ok, monitor, reduce_context, abort_safely
    let action = json["action"].as_str().unwrap();
    assert!(
        ["ok", "monitor", "reduce_context", "abort_safely"].contains(&action),
        "invalid action value: {}",
        action
    );
}

/// UT-V06-002: --heartbeat 输出的 action 与 pressure 一致
#[test]
fn test_heartbeat_action_matches_pressure() {
    let output = Command::new(hawk_eye_binary())
        .arg("--heartbeat")
        .output()
        .expect("Failed to run hawk-eye-mem --heartbeat");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();

    let pressure = json["pressure"].as_str().unwrap();
    let action = json["action"].as_str().unwrap();

    let expected_action = match pressure {
        "low" => "ok",
        "medium" => "monitor",
        "high" => "reduce_context",
        "critical" => "abort_safely",
        _ => panic!("unknown pressure: {}", pressure),
    };

    assert_eq!(
        action, expected_action,
        "action '{}' doesn't match pressure '{}'",
        action, pressure
    );
}

/// UT-V06-003: --analyze-cache-gaps 输出包含必要字段
#[test]
fn test_analyze_cache_gaps_human_readable() {
    let output = Command::new(hawk_eye_binary())
        .arg("--analyze-cache-gaps")
        .output()
        .expect("Failed to run --analyze-cache-gaps");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Must contain report header
    assert!(stdout.contains("缓存差距分析报告"), "missing report header");
    assert!(stdout.contains("实际命中率"), "missing actual hit rate");
    assert!(stdout.contains("目标命中率"), "missing target hit rate");
    assert!(stdout.contains("差距"), "missing gap");
}

/// UT-V06-004: --analyze-cache-gaps --json 输出合法 JSON
#[test]
fn test_analyze_cache_gaps_json() {
    let output = Command::new(hawk_eye_binary())
        .args(["--analyze-cache-gaps", "--json"])
        .output()
        .expect("Failed to run --analyze-cache-gaps --json");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim())
        .expect("analyze-cache-gaps --json output is not valid JSON");

    // Check required fields
    assert!(json.get("period_days").is_some(), "missing period_days");
    assert!(
        json.get("actual_hit_rate").is_some(),
        "missing actual_hit_rate"
    );
    assert!(
        json.get("target_hit_rate").is_some(),
        "missing target_hit_rate"
    );
    assert!(json.get("gap_percent").is_some(), "missing gap_percent");
    assert!(json.get("gaps").is_some(), "missing gaps array");
    assert!(
        json.get("suggestions").is_some(),
        "missing suggestions array"
    );
}

/// UT-V06-005: --analyze-cache-gaps --days 1 天数参数生效
#[test]
fn test_analyze_cache_gaps_custom_days() {
    let output = Command::new(hawk_eye_binary())
        .args(["--analyze-cache-gaps", "--json", "--days", "1"])
        .output()
        .expect("Failed to run --analyze-cache-gaps --days 1");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();

    assert_eq!(
        json["period_days"].as_u64().unwrap(),
        1,
        "period_days should be 1"
    );
}

/// UT-V06-006: --analyze-cache-gaps --target 95 自定义目标生效
#[test]
fn test_analyze_cache_gaps_custom_target() {
    let output = Command::new(hawk_eye_binary())
        .args(["--analyze-cache-gaps", "--json", "--target", "95.0"])
        .output()
        .expect("Failed to run --analyze-cache-gaps --target 95");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();

    let target = json["target_hit_rate"].as_f64().unwrap();
    assert!(
        (target - 95.0).abs() < 0.01,
        "target should be 95.0, got {}",
        target
    );
}

/// UT-V06-007: --heartbeat --json 兼容性（heartbeat 本身就是 JSON）
#[test]
fn test_heartbeat_json_flag_compatible() {
    let output = Command::new(hawk_eye_binary())
        .args(["--heartbeat", "--json"])
        .output()
        .expect("Failed to run --heartbeat --json");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let _: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("heartbeat --json output is not valid JSON");
}

/// UT-V06-008: --analyze-cache-gaps 超大 --days 不崩溃
#[test]
fn test_analyze_cache_gaps_large_days() {
    let output = Command::new(hawk_eye_binary())
        .args(["--analyze-cache-gaps", "--days", "9999"])
        .output()
        .expect("Failed to run --analyze-cache-gaps --days 9999");

    assert!(output.status.success(), "large days caused crash");
}

/// UT-V06-009: --target 0 边缘值
#[test]
fn test_analyze_cache_gaps_target_zero() {
    let output = Command::new(hawk_eye_binary())
        .args(["--analyze-cache-gaps", "--json", "--target", "0"])
        .output()
        .expect("Failed to run --analyze-cache-gaps --target 0");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(json["target_hit_rate"].as_f64().unwrap(), 0.0);
}

/// UT-V06-010: --source 已移除（不在 help 中）
#[test]
fn test_source_flag_removed() {
    let output = Command::new(hawk_eye_binary())
        .arg("--help")
        .output()
        .expect("Failed to run --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("--source"),
        "--source flag should have been removed from CLI"
    );
}

/// UT-V06-011: --analyze-cache-gaps --json --days 1 --target 50 组合参数
#[test]
fn test_analyze_cache_gaps_combined_params() {
    let output = Command::new(hawk_eye_binary())
        .args(["--analyze-cache-gaps", "--json", "--days", "1", "--target", "50.0"])
        .output()
        .expect("Failed to run combined params");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(json["period_days"].as_u64().unwrap(), 1, "days should be 1");
    assert!((json["target_hit_rate"].as_f64().unwrap() - 50.0).abs() < 0.01, "target should be 50");
}
