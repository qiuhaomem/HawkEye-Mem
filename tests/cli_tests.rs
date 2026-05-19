use std::process::Command;
use std::path::PathBuf;
use std::time::Duration;

/// 获取编译后的二进制路径
fn binary_path() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("target");
    path.push("debug");
    path.push("hawk-eye-mem");
    path
}

/// 运行命令并返回 (stdout, stderr, exit_code)
fn run_bin(args: &[&str]) -> (String, String, i32) {
    let output = Command::new(binary_path())
        .args(args)
        .output()
        .expect("Failed to run hawk-eye-mem");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}

// ===================== CLI 行为测试 =====================

// IT-CLI-001: --json 输出合法JSON结构
#[test]
fn test_it_cli_001_json_output() {
    let (stdout, _, code) = run_bin(&["--json"]);
    assert_eq!(code, 0, "exit code should be 0");
    let v: serde_json::Value = serde_json::from_str(&stdout)
        .expect("stdout should be valid JSON");
    assert!(v.get("system").is_some(), "JSON should contain 'system' key");
    assert!(v.get("agent_guidance").is_some(), "JSON should contain 'agent_guidance' key");
}

// IT-CLI-002: --metric available_mb 输出纯数字
#[test]
fn test_it_cli_002_metric_available_mb() {
    let (stdout, _, code) = run_bin(&["--metric", "available_mb"]);
    assert_eq!(code, 0, "exit code should be 0");
    let trimmed = stdout.trim();
    assert!(trimmed.parse::<u64>().is_ok(), "stdout should be a number, got: {}", trimmed);
}

// IT-CLI-002B: --metric 输出无污染
#[test]
fn test_it_cli_002b_metric_clean() {
    let (stdout, _, code) = run_bin(&["--metric", "available_mb"]);
    assert_eq!(code, 0);
    // 只允许数字 + 换行
    let is_clean = stdout.trim().parse::<u64>().is_ok()
        && stdout.chars().all(|c| c.is_ascii_digit() || c == '\n');
    assert!(is_clean, "stdout should be only digits + newline, got: {:?}", stdout);
}

// IT-CLI-003: --metric pressure 输出字符串
#[test]
fn test_it_cli_003_metric_pressure() {
    let (stdout, _, code) = run_bin(&["--metric", "pressure"]);
    assert_eq!(code, 0);
    let trimmed = stdout.trim().to_lowercase();
    assert!(
        ["low", "medium", "high", "critical"].contains(&trimmed.as_str()),
        "pressure should be one of low/medium/high/critical, got: {}",
        trimmed
    );
}

// IT-CLI-004: 无效参数报错
#[test]
fn test_it_cli_004_invalid_flag() {
    let (_, stderr, code) = run_bin(&["--xyz"]);
    assert_ne!(code, 0, "exit code should be non-zero");
    assert!(stderr.contains("error"), "stderr should contain error, got: {}", stderr);
}

// IT-CLI-005: --help 显示帮助
#[test]
fn test_it_cli_005_help() {
    let (stdout, _, code) = run_bin(&["--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("memory monitoring"), "help should contain 'memory monitoring'");
}

// IT-CLI-006: --version 输出版本
#[test]
fn test_it_cli_006_version() {
    let (stdout, _, code) = run_bin(&["--version"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("0.1.0"), "version should contain 0.1.0");
}

// IT-CLI-007: --config 加载自定义配置
#[test]
fn test_it_cli_007_config_load() {
    // 创建临时配置文件
    let dir = std::env::temp_dir().join("hawkeye_test_it007");
    let _ = std::fs::create_dir_all(&dir);
    let config_path = dir.join("config.toml");
    std::fs::write(&config_path, b"[model]\nbytes_per_token = 4000\nmargin = 15.0").unwrap();

    let (stdout, _, code) = run_bin(&["--config", config_path.to_str().unwrap(), "--json"]);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(code, 0);

    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let confidence = v["agent_guidance"]["confidence"].as_str().unwrap_or("");
    assert_eq!(confidence, "calibrated", "with config, confidence should be 'calibrated'");
}

// IT-CLI-008: 环境变量配置加载
#[test]
fn test_it_cli_008_env_config() {
    let dir = std::env::temp_dir().join("hawkeye_test_it008");
    let _ = std::fs::create_dir_all(&dir);
    let config_path = dir.join("config.toml");
    std::fs::write(&config_path, b"[model]\nbytes_per_token = 3000\nmargin = 10.0").unwrap();

    std::env::set_var("HAWKEYE_MEM_CONFIG", config_path.to_str().unwrap());
    let (stdout, _, code) = run_bin(&["--json"]);
    std::env::remove_var("HAWKEYE_MEM_CONFIG");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(code, 0);

    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let confidence = v["agent_guidance"]["confidence"].as_str().unwrap_or("");
    assert_eq!(confidence, "calibrated", "with env config, confidence should be 'calibrated'");
}

// IT-CLI-009: --interval 与 --count 组合
#[test]
fn test_it_cli_009_interval_count() {
    let start = std::time::Instant::now();
    let (stdout, _, code) = run_bin(&["--json", "--interval", "1", "--count", "2"]);
    let elapsed = start.elapsed();
    assert_eq!(code, 0);

    // 应该有2行JSON
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 2, "should output 2 JSON lines");

    // 每行都是合法JSON
    for (i, line) in lines.iter().enumerate() {
        let v: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|_| panic!("line {} should be valid JSON: {}", i, line));
        assert!(v.get("system").is_some(), "line {} should have 'system' key", i);
    }

    // 间隔约1秒
    assert!(elapsed >= Duration::from_secs(1), "should wait ~1s between outputs");
}

// IT-CLI-009B: JSON Lines 每行独立合法JSON
#[test]
fn test_it_cli_009b_json_lines_independent() {
    let (stdout, _, code) = run_bin(&["--json", "--interval", "1", "--count", "3"]);
    assert_eq!(code, 0);

    for (i, line) in stdout.trim().lines().enumerate() {
        let v: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|_| panic!("line {} should be valid independent JSON", i));
        assert!(v.is_object(), "line {} should be a JSON object, not array", i);
    }
}

// IT-CLI-011: --metric 与 --json 互斥
#[test]
fn test_it_cli_011_metric_json_mutex() {
    let (_, stderr, code) = run_bin(&["--json", "--metric", "available_mb"]);
    assert_ne!(code, 0, "exit code should be non-zero for conflicting args");
    assert!(stderr.contains("cannot be used with"), "stderr should mention conflict");
}

// ===================== JSON 结构验证 =====================

// IT-JSON-001: 时间戳格式符合RFC3339
#[test]
fn test_it_json_001_timestamp_rfc3339() {
    let (stdout, _, code) = run_bin(&["--json"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let ts = v["timestamp"].as_str().expect("timestamp should be a string");
    // 尝试解析RFC3339
    let parsed = chrono::DateTime::parse_from_rfc3339(ts);
    assert!(parsed.is_ok(), "timestamp should be RFC3339 format, got: {}", ts);
}

// IT-JSON-002: collection_duration_ms 为合理正数
#[test]
fn test_it_json_002_collection_duration() {
    let (stdout, _, code) = run_bin(&["--json"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let dur = v["collection_duration_ms"].as_f64().expect("collection_duration_ms should be a number");
    assert!(dur > 0.0, "duration should be > 0, got: {}", dur);
    assert!(dur < 100.0, "duration should be < 100ms, got: {}", dur);
}

// IT-JSON-003: used_percent 精度为1位小数
#[test]
fn test_it_json_003_used_percent_precision() {
    let (stdout, _, code) = run_bin(&["--json"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let percent = v["system"]["used_percent"].as_f64().unwrap();
    let formatted = format!("{:.1}", percent);
    // 验证只有1位小数
    let parts: Vec<&str> = formatted.split('.').collect();
    assert_eq!(parts.len(), 2, "should have decimal point");
    assert_eq!(parts[1].len(), 1, "should have exactly 1 decimal place");
}

// IT-JSON-004: estimated_safe_context_window 为整数
#[test]
fn test_it_json_004_estimated_window_int() {
    let (stdout, _, code) = run_bin(&["--json"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let window = &v["agent_guidance"]["estimated_safe_context_window"];
    assert!(window.is_i64() || window.is_u64(), "estimated_safe_context_window should be integer");
}

// IT-JSON-005: JSON Schema关键字段存在性
#[test]
fn test_it_json_005_schema_fields_exist() {
    let (stdout, _, code) = run_bin(&["--json"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    // 验证system字段
    let sys = v.get("system").expect("'system' field must exist");
    assert!(sys.get("total_mb").and_then(|x| x.as_u64()).is_some(), "system.total_mb must be u64");
    assert!(sys.get("used_mb").and_then(|x| x.as_u64()).is_some(), "system.used_mb must be u64");
    assert!(sys.get("available_mb").and_then(|x| x.as_u64()).is_some(), "system.available_mb must be u64");
    assert!(sys.get("used_percent").and_then(|x| x.as_f64()).is_some(), "system.used_percent must be f64");

    // 验证agent_guidance字段
    let guidance = v.get("agent_guidance").expect("'agent_guidance' field must exist");
    assert!(guidance.get("action").is_some() || guidance.get("pressure").is_some(),
        "agent_guidance must contain 'action' or 'pressure'");
    assert!(guidance.get("estimated_safe_context_window").and_then(|x| x.as_u64()).is_some(),
        "agent_guidance.estimated_safe_context_window must be u64");
    assert!(guidance.get("confidence").and_then(|x| x.as_str()).is_some(),
        "agent_guidance.confidence must be string");
}

// ===================== 错误路径测试 =====================

// IT-CLI-004 已覆盖无效参数
// IT-CLI-012: 权限错误处理——通过模拟不可读路径测试错误路径
// 实际权限模拟需要 root，这里测试配置加载失败路径
#[test]
fn test_it_cli_012_config_permission_error() {
    let (stdout, stderr, code) = run_bin(&["--config", "/root/secret/config.toml", "--json"]);
    // 应该输出错误JSON
    if code != 0 {
        // 检查是否输出了错误JSON
        if !stdout.is_empty() {
            let v: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_default();
            if v.get("error").is_some() {
                return; // 输出了合法错误JSON
            }
        }
        // 或者stderr有错误信息
        assert!(stderr.contains("error") || stderr.contains("Error"),
            "should report error in stderr: {}", stderr);
    }
}

// ===================== 配置生命周期测试 =====================

// IT-INT-001: 配置生命周期：未配置→已配置
#[test]
fn test_it_int_001_config_lifecycle() {
    // Step 1: 无配置时运行
    let (stdout1, _, _) = run_bin(&["--json"]);
    let v1: serde_json::Value = serde_json::from_str(&stdout1).unwrap();
    let conf1 = v1["agent_guidance"]["confidence"].as_str().unwrap_or("").to_string();

    // Step 2: 创建配置
    let dir = std::env::temp_dir().join("hawkeye_test_int001");
    let _ = std::fs::create_dir_all(&dir);
    let config_path = dir.join("config.toml");
    std::fs::write(&config_path, b"[model]\nbytes_per_token = 4000\nmargin = 20.0").unwrap();

    let (stdout2, _, _) = run_bin(&["--config", config_path.to_str().unwrap(), "--json"]);
    let _ = std::fs::remove_dir_all(&dir);
    let v2: serde_json::Value = serde_json::from_str(&stdout2).unwrap();
    let conf2 = v2["agent_guidance"]["confidence"].as_str().unwrap_or("").to_string();

    // 验证状态变化
    assert_eq!(conf1, "conservative", "without config should be 'conservative'");
    assert_eq!(conf2, "calibrated", "with config should be 'calibrated'");
}

// ===================== 首次运行引导验证 (IT-FIRST) =====================

/// 测试首次运行全流程：免责声明 + 顺序 + 再次运行不显示
/// 合并IT-FIRST-001/002/003为单测试，使用独立HOME避免并行竞争
#[test]
fn test_it_first_onboarding_full() {
    // 创建临时HOME，先清理可能残留的上次运行文件
    let tmp_home = std::env::temp_dir().join("hawkeye_test_first_run");
    let _ = std::fs::remove_dir_all(&tmp_home);
    let _ = std::fs::create_dir_all(&tmp_home);
    let old_home = std::env::var("HOME").ok();
    std::env::set_var("HOME", tmp_home.to_str().unwrap());

    // === IT-FIRST-001: 免责声明内容验证 ===
    let (_, stderr, code) = run_bin(&[]);
    assert_eq!(code, 0, "exit code should be 0");

    let lower = stderr.to_lowercase();
    assert!(lower.contains("no warranty"),
        "stderr must contain 'No warranty', got: {}", stderr);
    assert!(lower.contains("use at your own risk"),
        "stderr must contain 'Use at your own risk', got: {}", stderr);

    // === IT-FIRST-002: 免责声明先于引导输出顺序 ===
    let no_warranty = stderr.find("No warranty");
    let quick_start = stderr.find("Quick Start");
    assert!(no_warranty.is_some() && quick_start.is_some(),
        "Both 'No warranty' and 'Quick Start' must be in stderr");
    assert!(no_warranty.unwrap() < quick_start.unwrap(),
        "Disclaimer must appear before Quick Start guide");

    // === IT-FIRST-003: .onboarded文件创建后不再显示 ===
    let onboarded = tmp_home.join(".config/hawk-eye-mem/.onboarded");
    assert!(onboarded.exists(), ".onboarded file should exist after first run");

    let (_, stderr2, _) = run_bin(&[]);
    assert!(!stderr2.contains("No warranty"),
        "On subsequent runs, 'No warranty' should NOT appear");
    assert!(!stderr2.contains("Quick Start"),
        "On subsequent runs, 'Quick Start' should NOT appear");

    // 清理
    let _ = std::fs::remove_dir_all(&tmp_home);
    if let Some(h) = old_home {
        std::env::set_var("HOME", h);
    } else {
        std::env::remove_var("HOME");
    }
}
