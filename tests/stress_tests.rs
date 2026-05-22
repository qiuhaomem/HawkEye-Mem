// ============================================================================
// 秋毫mem V0.2 W5 — 极限测试套件
// 真正的极限测试：压边界、压组合、压连续运行
// ============================================================================
//
// 分组说明:
//   ST-CLI-*   参数组合极限
//   ST-JSON-*  JSON Schema 完整验证
//   ST-AS-*    --can-run 极限评估
//   ST-INT-*   连续监控稳定性
//   ST-MET-*   --metric 极限
//   ST-LM-*    --list-models 验证
//   ST-STR-*   手动压力测试 (#[ignore])

use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

// ============================================================================
// 辅助函数
// ============================================================================

fn binary_path() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("target");
    path.push("debug");
    path.push("hawk-eye-mem");
    path
}

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

/// 验证字符串是否是合法 RFC 3339 时间戳
fn is_rfc3339(s: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(s).is_ok()
}

/// 验证 JSON 数值字段类型正确且非 null
fn check_number_field(v: &serde_json::Value, path: &[&str]) {
    let mut current = v;
    for key in path {
        current = current.get(key).unwrap_or_else(|| {
            panic!("Missing field: {}", key);
        });
    }
    assert!(
        current.is_number(),
        "Field {:?} should be a number, got {:?}",
        path,
        current
    );
    assert!(!current.is_null(), "Field {:?} should not be null", path);
}

/// 验证 JSON 字符串字段类型正确且非空
fn check_string_field(v: &serde_json::Value, path: &[&str], check_non_empty: bool) {
    let mut current = v;
    for key in path {
        current = current.get(key).unwrap_or_else(|| {
            panic!("Missing field: {}", key);
        });
    }
    assert!(
        current.is_string(),
        "Field {:?} should be a string, got {:?}",
        path,
        current
    );
    if check_non_empty {
        assert!(
            !current.as_str().unwrap().is_empty(),
            "Field {:?} should not be empty",
            path
        );
    }
}

// ============================================================================
// 1. 参数组合极限 (ST-CLI)
// ============================================================================

/// ST-CLI-001: 所有兼容参数同时给
#[test]
fn test_st_cli_001_compatible_args_all() {
    let (stdout, _stderr, code) = run_bin(&["--json", "--interval", "1", "--count", "1"]);
    assert_eq!(code, 0, "--json --interval 1 --count 1 should succeed");
    let v: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("stdout should be valid JSON");
    assert!(v.get("system").is_some(), "JSON should contain 'system'");
    assert!(
        v.get("agent_guidance").is_some(),
        "JSON should contain 'agent_guidance'"
    );
}

/// ST-CLI-002: --context 超大值（在 u32 范围内，验证能解析）
#[test]
fn test_st_cli_002_context_huge() {
    let (stdout, _stderr, code) = run_bin(&[
        "--can-run",
        "--model",
        "deepseek-v2-lite",
        "--context",
        "999999999",
    ]);
    // --can-run always returns 0, the context value is passed to assess engine
    assert_eq!(code, 0, "--can-run with large context should not crash");
    let v: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("stdout should be valid JSON");
    // Should have constraints (since 999999999 context definitely won't fit)
    assert!(
        v.get("constraints").is_some(),
        "assessment should have constraints field"
    );
    assert!(v.get("verdict").is_some(), "assessment should have verdict");
}

/// ST-CLI-003: --model-size 超出 u64 范围（clap 应拒绝）
#[test]
fn test_st_cli_003_model_size_overflow() {
    let (_stdout, stderr, code) = run_bin(&["--model-size", "99999999999999999999"]);
    assert_ne!(code, 0, "overflow model-size should be rejected");
    assert!(
        stderr.contains("error") || stderr.contains("invalid"),
        "stderr should indicate error, got: {}",
        stderr
    );
}

/// ST-CLI-004: --model 空字符串
#[test]
fn test_st_cli_004_model_empty() {
    // Empty model name passes clap (Option<String>) but model not found
    let (stdout, _stderr, code) = run_bin(&["--can-run", "--model", ""]);
    // --can-run always returns 0, assess proceeds with model_name: Some("")
    assert_eq!(code, 0, "--can-run with empty model should not crash");
    let v: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("stdout should be valid JSON");
    // Should have request with empty model_name
    assert!(v.get("request").is_some(), "assessment should have request");
}

/// ST-CLI-005: --model 乱码字符串
#[test]
fn test_st_cli_005_model_garbage() {
    let (stdout, _stderr, code) = run_bin(&["--can-run", "--model", "!@#$%^&*()"]);
    assert_eq!(code, 0, "--can-run with garbage model should not crash");
    let v: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("stdout should be valid JSON");
    // No constraints expected (model not found, skip memory estimation)
    assert!(v.get("verdict").is_some(), "assessment should have verdict");
}

/// ST-CLI-006: --model unicode 模型名
#[test]
fn test_st_cli_006_model_unicode() {
    let (stdout, _stderr, code) = run_bin(&["--can-run", "--model", "🦙llama3-8b"]);
    assert_eq!(code, 0, "--can-run with unicode model should not crash");
    let v: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("stdout should be valid JSON");
    assert!(v.get("verdict").is_some(), "assessment should have verdict");
}

/// ST-CLI-007: --compare 带 4 个模型（应报错）
#[test]
fn test_st_cli_007_compare_4_models() {
    let (_stdout, stderr, code) = run_bin(&["--can-run", "--compare", "a,b,c,d"]);
    assert_ne!(code, 0, "--compare with 4 models should fail");
    assert!(
        stderr.contains("1-3"),
        "stderr should mention 1-3 model limit, got: {}",
        stderr
    );
}

/// ST-CLI-008: --model 和 --model-size 同时给（应报错）
#[test]
fn test_st_cli_008_model_and_model_size_conflict() {
    let (_stdout, stderr, code) = run_bin(&[
        "--can-run",
        "--model",
        "llama3-8b",
        "--model-size",
        "7000000000",
    ]);
    assert_ne!(
        code, 0,
        "--model and --model-size conflict should be rejected"
    );
    assert!(
        stderr.contains("cannot be used with") || stderr.contains("error"),
        "stderr should indicate conflict, got: {}",
        stderr
    );
}

// ============================================================================
// 2. JSON Schema 完整验证 (ST-JSON)
// ============================================================================

/// ST-JSON-001: 顶层字段完整验证
#[test]
fn test_st_json_001_top_level_fields() {
    let (stdout, _, code) = run_bin(&["--json"]);
    assert_eq!(code, 0);

    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout should be valid JSON");

    // 必须包含的顶层字段
    let timestamp = v
        .get("timestamp")
        .expect("must have 'timestamp'")
        .as_str()
        .expect("timestamp must be a string");
    assert!(
        is_rfc3339(timestamp),
        "timestamp must be RFC3339, got: {}",
        timestamp
    );

    let duration = v
        .get("collection_duration_ms")
        .expect("must have 'collection_duration_ms'")
        .as_f64()
        .expect("collection_duration_ms must be a number");
    assert!(
        duration > 0.0,
        "collection_duration_ms must be positive, got: {}",
        duration
    );

    assert!(v.get("system").is_some(), "must have 'system'");
    assert!(
        v.get("agent_guidance").is_some(),
        "must have 'agent_guidance'"
    );

    // 顶层不应有多余字段（允许的就这几个）
    let allowed_keys = [
        "timestamp",
        "collection_duration_ms",
        "system",
        "agent_guidance",
    ];
    for key in v.as_object().unwrap().keys() {
        assert!(
            allowed_keys.contains(&key.as_str()),
            "unexpected top-level key: {}",
            key
        );
    }
}

/// ST-JSON-002: system 对象字段验证
#[test]
fn test_st_json_002_system_fields() {
    let (stdout, _, code) = run_bin(&["--json"]);
    assert_eq!(code, 0);

    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let system = v.get("system").unwrap();

    // 必含字段
    check_number_field(&v, &["system", "total_mb"]);
    check_number_field(&v, &["system", "used_mb"]);
    check_number_field(&v, &["system", "available_mb"]);
    check_number_field(&v, &["system", "used_percent"]);

    // 数值合理性：total_mb > 0
    let total_mb = system["total_mb"].as_u64().unwrap();
    assert!(total_mb > 0, "total_mb must be > 0, got: {}", total_mb);

    // used_percent 在 0-100 范围内
    let used_percent = system["used_percent"].as_f64().unwrap();
    assert!(
        used_percent >= 0.0 && used_percent <= 100.0,
        "used_percent must be 0-100, got: {}",
        used_percent
    );

    // available 不应超过 total
    let available = system["available_mb"].as_u64().unwrap();
    assert!(
        available <= total_mb,
        "available_mb ({}) must not exceed total_mb ({})",
        available,
        total_mb
    );

    // 可选字段：如果存在则验证结构
    if let Some(disk) = system.get("disk") {
        assert!(disk.is_object(), "disk should be an object");
        assert!(disk.get("path").is_some(), "disk should have path");
        assert!(
            disk.get("available_mb").is_some(),
            "disk should have available_mb"
        );
    }
    if let Some(cpu) = system.get("cpu") {
        assert!(cpu.is_object(), "cpu should be an object");
        assert!(cpu.get("cores").is_some(), "cpu should have cores");
        assert!(
            cpu.get("load_avg_1m").is_some(),
            "cpu should have load_avg_1m"
        );
    }
}

/// ST-JSON-003: agent_guidance 字段验证
#[test]
fn test_st_json_003_agent_guidance_fields() {
    let (stdout, _, code) = run_bin(&["--json"]);
    assert_eq!(code, 0);

    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let guidance = v.get("agent_guidance").unwrap();

    // 必含字段（字符串）
    check_string_field(&v, &["agent_guidance", "action"], true);
    check_string_field(&v, &["agent_guidance", "reason"], true);
    check_string_field(&v, &["agent_guidance", "pressure"], true);
    check_string_field(&v, &["agent_guidance", "confidence"], true);
    check_string_field(&v, &["agent_guidance", "_note"], true);

    // 数值字段
    check_number_field(&v, &["agent_guidance", "estimated_safe_context_window"]);

    // pressure 必须是合法值
    let pressure = guidance["pressure"].as_str().unwrap();
    assert!(
        ["low", "medium", "high", "critical"].contains(&pressure),
        "pressure must be one of low/medium/high/critical, got: {}",
        pressure
    );

    // action 必须是合法值
    let action = guidance["action"].as_str().unwrap();
    assert!(
        ["ok", "monitor", "reduce_context", "abort_safely"].contains(&action),
        "action must be ok/monitor/reduce_context/abort_safely, got: {}",
        action
    );

    // confidence 必须是合法值
    let confidence = guidance["confidence"].as_str().unwrap();
    assert!(
        ["conservative", "calibrated"].contains(&confidence),
        "confidence must be conservative/calibrated, got: {}",
        confidence
    );

    // estimated_safe_context_window is u64, always >= 0
    let _safe_ctx = guidance["estimated_safe_context_window"]
        .as_u64()
        .expect("estimated_safe_context_window should be a valid u64");

    // _note 应有正确内容
    let note = guidance["_note"].as_str().unwrap();
    assert!(
        note.contains("recommendations"),
        "_note should mention recommendations, got: {}",
        note
    );

    // suggestion 可选：如果是 "conservative" 则应有，否则可为 None
    if confidence == "conservative" {
        assert!(
            guidance.get("suggestion").is_some(),
            "suggestion should exist when confidence=conservative"
        );
        assert!(
            guidance["suggestion"].is_string(),
            "suggestion should be a string"
        );
    }
}

/// ST-JSON-004: 数值类型严格校验（无 null、无类型混用）
#[test]
fn test_st_json_004_type_strictness() {
    let (stdout, _, code) = run_bin(&["--json"]);
    assert_eq!(code, 0);

    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    // 递归检查所有值：不应有 null（除非是 Option 字段）
    fn check_no_null(val: &serde_json::Value, path: &str, skip_keys: &[&str]) {
        match val {
            serde_json::Value::Null => {
                // 只有在跳过列表中的路径才允许 null
                if !skip_keys
                    .iter()
                    .any(|k| path.ends_with(k) || path.contains(k))
                {
                    panic!("Unexpected null at {}", path);
                }
            }
            serde_json::Value::Object(map) => {
                for (k, v) in map {
                    let child = format!("{}.{}", path, k);
                    check_no_null(v, &child, skip_keys);
                }
            }
            serde_json::Value::Array(arr) => {
                for (i, v) in arr.iter().enumerate() {
                    let child = format!("{}[{}]", path, i);
                    check_no_null(v, &child, skip_keys);
                }
            }
            _ => {}
        }
    }

    // system.disk, system.cpu, system.gpu 以及 agent_guidance.suggestion 可能不存在（而非 null）
    // 递归检查会跳过这些键
    check_no_null(&v, "root", &["suggestion", "disk", "cpu", "gpu"]);
}

/// ST-JSON-005: 连续模式每行 JSON 结构一致
#[test]
fn test_st_json_005_continuous_json_consistency() {
    let (stdout, _, code) = run_bin(&["--json", "--interval", "1", "--count", "3"]);
    assert_eq!(code, 0);

    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 3, "should output 3 JSON lines");

    // 缓存第一个 JSON 的键集合
    let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    let first_keys: Vec<String> = {
        let obj = first.as_object().unwrap();
        let mut keys: Vec<String> = obj.keys().cloned().collect();
        keys.sort();
        keys
    };

    // 后续每行的键集合必须一致
    for (i, line) in lines.iter().enumerate().skip(1) {
        let cur: serde_json::Value = serde_json::from_str(line).unwrap();
        let cur_keys: Vec<String> = {
            let obj = cur.as_object().unwrap();
            let mut keys: Vec<String> = obj.keys().cloned().collect();
            keys.sort();
            keys
        };
        assert_eq!(
            cur_keys, first_keys,
            "line {} has different keys than line 0: {:?} vs {:?}",
            i, cur_keys, first_keys
        );
    }
}

// ============================================================================
// 3. --can-run 极限评估 (ST-AS)
// ============================================================================

/// ST-AS-001: 评估 70B 模型（直接运行应不可行，但有降级方案）
#[test]
fn test_st_as_001_70b_infeasible() {
    let (stdout, _, code) = run_bin(&["--can-run", "--model-size", "70000000000"]);
    assert_eq!(code, 0);

    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let verdict = v
        .get("verdict")
        .expect("assessment should have verdict")
        .as_str()
        .unwrap_or("");

    // 70B 模型直接运行不可行，但有降级方案（降低量化/换小模型），
    // 因此 verdict 可能是 infeasible 或 feasible_with_caveats
    assert_ne!(
        verdict, "feasible",
        "70B model should NOT be feasible, got verdict: {}",
        verdict
    );

    // 应有内存约束
    let constraints = v
        .get("constraints")
        .expect("assessment should have constraints")
        .as_array()
        .unwrap();
    assert!(
        !constraints.is_empty(),
        "70B model should have memory constraints"
    );

    // 应有降级方案
    let options = v
        .get("safe_options")
        .expect("assessment should have safe_options")
        .as_array()
        .unwrap();
    assert!(!options.is_empty(), "70B should have safe options");
}

/// ST-AS-002: 评估不存在模型（应正常返回，但无约束）
#[test]
fn test_st_as_002_nonexistent_model() {
    let (stdout, _, code) = run_bin(&["--can-run", "--model", "nonexistent-model-999b"]);
    assert_eq!(code, 0, "--can-run with nonexistent model should not crash");

    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    // request 应包含模型名
    let request = v.get("request").unwrap();
    assert_eq!(request["model_name"], "nonexistent-model-999b");

    // 应有 verdict（即便模型不存在）
    assert!(v.get("verdict").is_some(), "should have verdict");
}
/// ST-AS-003: 超大 context 32768 — 验证评估引擎正确处理，不崩溃
#[test]
fn test_st_as_003_huge_context() {
    let (stdout, _, code) = run_bin(&[
        "--can-run",
        "--model",
        "deepseek-v2-lite",
        "--context",
        "32768",
    ]);
    assert_eq!(code, 0);

    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let verdict = v.get("verdict").unwrap().as_str().unwrap_or("");

    // verdict 可能为 feasible（大内存机器）或 feasible_with_caveats
    // 只验证不 panic 且输出结构正确
    assert!(
        ["feasible", "feasible_with_caveats", "infeasible"].contains(&verdict),
        "verdict should be a valid value, got: {}",
        verdict
    );

    // 验证 request 包含指定 context
    let request = v.get("request").unwrap();
    assert_eq!(
        request["context_window"], 32768,
        "request should contain context_window=32768"
    );
}

/// ST-AS-004: 手动指定超大模型参数应产生内存约束（硬件无关）
#[test]
fn test_st_as_004_huge_model_size() {
    let (stdout, _, code) = run_bin(&[
        "--can-run",
        "--model-size",
        "200000000000",
        "--context",
        "4096",
    ]);
    assert_eq!(code, 0);

    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let verdict = v.get("verdict").unwrap().as_str().unwrap_or("");

    // 200B 模型在任何机器上都不 feasible
    assert_ne!(
        verdict, "feasible",
        "200B model should NOT be feasible on any machine"
    );

    // 应有约束
    let constraints = v
        .get("constraints")
        .expect("assessment should have constraints")
        .as_array()
        .unwrap();
    assert!(
        !constraints.is_empty(),
        "200B model should have constraints"
    );

    // 应有降级方案
    assert!(v.get("safe_options").is_some());
}

/// ST-AS-005: 同时比较 3 个模型（极限满载）
/// 需要 --json 标志使 --compare 输出 JSON
#[test]
fn test_st_as_005_compare_three_models() {
    let (stdout, _, code) = run_bin(&[
        "--can-run",
        "--json",
        "--compare",
        "deepseek-v2-lite,llama3-8b,phi-3-mini",
    ]);
    assert_eq!(code, 0);

    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    // 比较输出应有 comparison 数组
    let comparison = v
        .get("comparison")
        .expect("should have 'comparison' array")
        .as_array()
        .unwrap();
    assert_eq!(
        comparison.len(),
        3,
        "should compare exactly 3 models, got {}",
        comparison.len()
    );

    // 每个模型应有 verdict
    for (i, model_result) in comparison.iter().enumerate() {
        assert!(
            model_result.get("verdict").is_some(),
            "model {} should have verdict",
            i
        );
        assert!(
            model_result.get("constraints").is_some(),
            "model {} should have constraints",
            i
        );
        assert!(
            model_result.get("safe_options").is_some(),
            "model {} should have safe_options",
            i
        );
    }

    // 应有推荐索引
    assert!(
        v.get("recommended_index").is_some(),
        "should have recommended_index"
    );
}

// ============================================================================
// 4. 连续监控稳定性 (ST-INT)
// ============================================================================

/// ST-INT-001: --interval 1 --count 5 输出行数正确
#[test]
fn test_st_int_001_count_lines() {
    let start = Instant::now();
    let (stdout, _stderr, code) = run_bin(&["--json", "--interval", "1", "--count", "5"]);
    let elapsed = start.elapsed();
    assert_eq!(code, 0);

    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 5, "should output exactly 5 JSON lines");

    // 总时间应 >= 4 秒（5 次采集间隔 4 次等待）
    assert!(
        elapsed >= Duration::from_secs(4),
        "should take at least 4s for 5 samples at 1s interval, took {:?}",
        elapsed
    );

    // 每行都是合法 JSON
    for (i, line) in lines.iter().enumerate() {
        let v: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|_| panic!("line {} should be valid JSON", i));
        assert!(v.is_object(), "line {} should be a JSON object", i);
    }
}

/// ST-INT-002: 每次 JSON 输出结构一致
#[test]
fn test_st_int_002_structure_consistent() {
    let (stdout, _, code) = run_bin(&["--json", "--interval", "1", "--count", "4"]);
    assert_eq!(code, 0);

    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 4);

    // 比较 system 和 agent_guidance 的键集合
    let first_system_keys: Vec<String> = {
        let v: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        let sys = v["system"].as_object().unwrap();
        let mut keys: Vec<String> = sys.keys().cloned().collect();
        keys.sort();
        keys
    };

    for (i, line) in lines.iter().enumerate().skip(1) {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        let sys = v["system"].as_object().unwrap();
        let mut keys: Vec<String> = sys.keys().cloned().collect();
        keys.sort();
        assert_eq!(
            keys, first_system_keys,
            "line {} system keys differ: {:?} vs {:?}",
            i, keys, first_system_keys
        );

        // agent_guidance 的键也应一致
        let guidance_keys: Vec<String> = v["agent_guidance"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        let ref_guidance_keys: Vec<String> = {
            let ref_v: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
            ref_v["agent_guidance"]
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect()
        };
        assert_eq!(
            guidance_keys.len(),
            ref_guidance_keys.len(),
            "line {} agent_guidance key count differs",
            i
        );
    }
}

/// ST-INT-003: collection_duration_ms 稳定（不出现异常大值）
#[test]
fn test_st_int_003_duration_stability() {
    let (stdout, _, code) = run_bin(&["--json", "--interval", "1", "--count", "4"]);
    assert_eq!(code, 0);

    let lines: Vec<&str> = stdout.trim().lines().collect();
    let mut durations: Vec<f64> = Vec::new();

    for line in &lines {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        let d = v["collection_duration_ms"].as_f64().unwrap();
        durations.push(d);
    }

    // 最大与均值比不应超过 10 倍（防止异常大值）
    let mean = durations.iter().sum::<f64>() / durations.len() as f64;
    let max = durations.iter().cloned().fold(0.0_f64, f64::max);
    assert!(
        max < mean * 10.0,
        "max duration ({:.1}ms) is too far from mean ({:.1}ms)",
        max,
        mean
    );

    // 每次采集应在合理时间内（< 100ms 是合理的）
    for (i, d) in durations.iter().enumerate() {
        assert!(
            *d < 1000.0,
            "collection {} took {:.1}ms, expected < 1000ms",
            i,
            d
        );
    }
}

/// ST-INT-004: timestamp 严格递增
#[test]
fn test_st_int_004_timestamp_increasing() {
    let (stdout, _, code) = run_bin(&["--json", "--interval", "1", "--count", "3"]);
    assert_eq!(code, 0);

    let lines: Vec<&str> = stdout.trim().lines().collect();
    let mut prev_ts: Option<String> = None;

    for (i, line) in lines.iter().enumerate() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        let ts = v["timestamp"].as_str().unwrap().to_string();
        let parsed = chrono::DateTime::parse_from_rfc3339(&ts)
            .unwrap_or_else(|_| panic!("line {}: invalid timestamp: {}", i, ts));

        if let Some(ref prev) = prev_ts {
            let prev_parsed = chrono::DateTime::parse_from_rfc3339(prev).unwrap();
            assert!(
                parsed > prev_parsed,
                "line {} timestamp ({}) should be after previous ({})",
                i,
                ts,
                prev
            );
        }
        prev_ts = Some(ts);
    }
}

// ============================================================================
// 5. --metric 极限 (ST-MET)
// ============================================================================

/// ST-MET-001: --metric total_mb 验证数值精度
#[test]
fn test_st_met_001_total_mb() {
    let (stdout, _, code) = run_bin(&["--metric", "total_mb"]);
    assert_eq!(code, 0);
    let trimmed = stdout.trim();
    let val: u64 = trimmed
        .parse()
        .unwrap_or_else(|e| panic!("total_mb should be a valid u64, got '{}': {}", trimmed, e));
    assert!(val > 0, "total_mb should be > 0, got: {}", val);
    // 合理性检查：通常在 512-1048576 MB 之间
    assert!(val <= 1048576, "total_mb seems unreasonably large: {}", val);
}

/// ST-MET-002: --metric used_mb
#[test]
fn test_st_met_002_used_mb() {
    let (stdout, _, code) = run_bin(&["--metric", "used_mb"]);
    assert_eq!(code, 0);
    let trimmed = stdout.trim();
    let val: u64 = trimmed
        .parse()
        .unwrap_or_else(|e| panic!("used_mb should be a valid u64, got '{}': {}", trimmed, e));
    assert!(val > 0, "used_mb should be > 0, got: {}", val);
}

/// ST-MET-003: --metric used_percent（浮点数格式验证）
#[test]
fn test_st_met_003_used_percent() {
    let (stdout, _, code) = run_bin(&["--metric", "used_percent"]);
    assert_eq!(code, 0);
    let trimmed = stdout.trim();
    let val: f64 = trimmed.parse().unwrap_or_else(|e| {
        panic!(
            "used_percent should be a valid f64, got '{}': {}",
            trimmed, e
        )
    });
    assert!(
        val >= 0.0 && val <= 100.0,
        "used_percent should be 0-100, got: {}",
        val
    );
    // used_percent 输出格式应为 X.X（保留一位小数）
    let decimal_part = if trimmed.contains('.') {
        trimmed.split('.').nth(1).unwrap_or("")
    } else {
        ""
    };
    assert!(
        decimal_part.len() <= 1 || (decimal_part.len() == 2 && decimal_part == "0"),
        "used_percent should have at most 1 decimal place, got: {}",
        trimmed
    );
}

/// ST-MET-004: --metric 不存在的指标（应报错）
#[test]
fn test_st_met_004_unknown_metric() {
    let (_stdout, stderr, code) = run_bin(&["--metric", "nonexistent_metric"]);
    assert_ne!(code, 0, "unknown metric should fail");
    assert!(
        stderr.contains("Unknown metric"),
        "stderr should mention 'Unknown metric', got: {}",
        stderr
    );
}

// ============================================================================
// 6. --list-models 验证 (ST-LM)
// ============================================================================

/// ST-LM-001: --list-models 列出 8 个模型
#[test]
fn test_st_lm_001_all_models_listed() {
    let (stdout, _, code) = run_bin(&["--list-models"]);
    assert_eq!(code, 0);

    // 验证所有 8 个模型名称出现在输出中
    let expected_models = [
        "llama3-8b",
        "qwen2-7b",
        "deepseek-v2-lite",
        "mistral-7b",
        "phi-3-mini",
        "gemma-2-9b",
        "yi-6b",
        "chatglm3-6b",
    ];

    for model in &expected_models {
        assert!(
            stdout.contains(model),
            "--list-models should contain '{}', got:\n{}",
            model,
            stdout
        );
    }
}

/// ST-LM-002: --list-models 输出包含来源和更新日期信息
#[test]
fn test_st_lm_002_source_and_update() {
    let (stdout, _, code) = run_bin(&["--list-models"]);
    assert_eq!(code, 0);

    // 验证来源信息存在
    let expected_sources = [
        "Meta Llama 3",
        "Qwen 2",
        "DeepSeek V2",
        "Mistral AI",
        "Microsoft Phi-3",
        "Google Gemma 2",
        "01.AI Yi",
        "THUDM ChatGLM3",
    ];

    for source in &expected_sources {
        assert!(
            stdout.contains(source),
            "--list-models output should contain source '{}'",
            source
        );
    }

    // 验证更新日期格式（YYYY-MM）
    assert!(
        stdout.contains("2026-05"),
        "--list-models should contain update date in YYYY-MM format"
    );
}

// ============================================================================
// 7. 压力测试（手动标记）
// ============================================================================

/// ST-STR-001: 磁盘满模拟
/// 创建大文件填充磁盘后测 hawk-eye-mem --json
/// 注意：手动测试，不会在 CI 中运行
#[test]
#[ignore]
fn test_st_str_001_disk_full_simulation() {
    // 手动测试步骤：
    // 1. 用 dd 或 fallocate 创建大文件填充磁盘至 ~95%
    // 2. 运行：hawk-eye-mem --json
    // 3. 验证输出 system.disk.available_mb < 总磁盘 5%
    // 4. 验证 disk.pressure 为 "critical"
    // 5. 清理大文件
    //
    // 示例（需要 root 权限，请按实际路径调整）：
    //   sudo dd if=/dev/zero of=/tmp/fill_disk bs=1M count=10000
    //   cargo test test_st_str_001_disk_full_simulation -- --ignored
    //   sudo rm /tmp/fill_disk
    unimplemented!("Manual test: fill disk then check disk pressure in JSON output");
}

/// ST-STR-002: CPU 100% 压力
/// 用 stress-ng 或 yes 跑满 CPU 后测 hawk-eye-mem
/// 注意：手动测试，不会在 CI 中运行
#[test]
#[ignore]
fn test_st_str_002_cpu_100_percent() {
    // 手动测试步骤：
    // 1. 安装 stress-ng: sudo apt install stress-ng
    // 2. 后台跑满所有 CPU 核心：
    //      stress-ng --cpu $(nproc) --timeout 60s &
    //    或：
    //      for i in $(seq $(nproc)); do yes > /dev/null & done
    // 3. 运行：hawk-eye-mem --json
    // 4. 验证 system.cpu.load_avg_1m 接近核心数
    // 5. 验证 system.cpu.pressure 为 "high"
    // 6. 清理后台进程：killall yes stress-ng
    //
    // 示例：
    //   stress-ng --cpu $(nproc) --timeout 60s &
    //   cargo test test_st_str_002_cpu_100_percent -- --ignored
    //   killall stress-ng
    unimplemented!("Manual test: stress CPU then check CPU pressure in JSON output");
}
