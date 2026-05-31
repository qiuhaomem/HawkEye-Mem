---
name: hawk-eye-mem
description: "Use when you need to check system memory pressure, GPU status, CPU/GPU temperature, co-located AI Agent processes, environment fingerprint, remote node metrics, trend analysis, container/cgroup awareness, or estimate safe context window size for LLM inference. AI-Native system resource monitoring CLI (秋毫mem). V0.4 adds environment fingerprinting, remote HTTP server, trend analysis, container adaptation, and alert mode."
version: 0.4.0
author: 秋毫mem Team
license: Apache-2.0
metadata:
  hermes:
    tags: [memory, monitoring, llm, system, mlops, gpu, thermal, agent-tools]
    related_skills: [llama-cpp, serving-llms-vllm, prompt-cache-strategy]
---

# HawkEye Mem (秋毫mem) v0.4

AI-Native system resource monitoring CLI — RAM, GPU, temperature, Agent processes, environment fingerprint, remote collection, trends, container awareness.

秋毫mem (HawkEye Mem) is a zero-dependency, cross-platform CLI tool that reads system resource metrics and provides **Agent-friendly decisions**. It tells your LLM Agent:

- How much memory is available right now
- Whether it's safe to load a model / run inference
- GPU status (VRAM, temperature, power, utilization) — NVIDIA, AMD, Apple Silicon
- CPU/GPU temperature with pressure levels
- What other AI Agents are running on the same machine
- Estimated safe context window size (tokens)
- Calibrated model parameters via dynamic learning
- **Environment fingerprint** — detect when Agent moves between machines
- **Remote collection** — aggregate metrics from multiple machines
- **Trend analysis** — predict resource exhaustion
- **Container awareness** — respect Docker/K8s cgroup limits

**Binary name:** `hawk-eye-mem`
**Repo:** `github.com/qiuhaomem/HawkEye-Mem`

## Overview

秋毫mem (HawkEye Mem) is a zero-dependency, cross-platform CLI tool that reads memory metrics and provides **Agent-friendly decisions**. It tells your LLM Agent:

- How much memory is available right now
- Whether it's safe to load a model / run inference
- What action to take: `ok` → `monitor` → `reduce_context` → `abort_safely`
- Estimated safe context window size (tokens)

## New in V0.4

| Feature | What | Why |
|---------|------|-----|
| 🏠 **Environment Fingerprint** | Auto-detect machine changes (CPU/RAM/GPU/disk) | Agent "搬家也不怕" — knows when environment changes |
| 🌐 **Remote HTTP Server** | `--serve` lightweight HTTP + `/metrics` endpoint | Multi-machine resource aggregation |
| 📈 **Trend Analysis** | `--trend` linear regression + urgency prediction | Predict when memory will run out |
| 📦 **Container Adapter** | cgroup v1/v2 memory+CPU limit detection | Respect Docker/K8s resource limits |
| 🚨 **Alert Mode** | `--alert` minimal JSON output for critical pressure | Pipe to external monitoring |
| 🗺️ **Environment Change JSON** | `environment_change` field in JSON output | Agent reads migration recommendations |

## When to Use

- Before launching an LLM inference session — check if enough RAM is available
- During long-running Agent loops — periodically check pressure to avoid OOM
- When the Agent detects slow response or potential swap thrashing
- Before loading a large model (via `llama-cpp`, `vllm`, etc.) — confirm memory budget
- In cron jobs / scheduled tasks that track system health
- When the Agent moves to a new machine — detect environment changes automatically

## Usage

### Quick Commands

```bash
# Full JSON output (for Agent consumption) — now includes gpu/thermal/agents/env/container
hawk-eye-mem --json

# Single metric for scripts
hawk-eye-mem --metric available_mb     # pure number
hawk-eye-mem --metric pressure         # low|medium|high|critical

# GPU status
hawk-eye-mem --gpu-list                # List GPUs with backend

# Calibration
hawk-eye-mem --calibration-stats --model-name llama3-8b
hawk-eye-mem --reset-calibration --model-name llama3-8b  # Clear calibration data
hawk-eye-mem --tokens-processed 4096 --model-name llama3-8b  # Record calibration point

# Continuous monitoring (JSON Lines)
hawk-eye-mem --json --interval 5 --count 12    # 12 samples, 5s apart
hawk-eye-mem --json --interval 10 --count 0    # infinite (Ctrl+C to stop)

# Custom model config for calibrated estimates
hawk-eye-mem --config /path/to/config.toml --json
hawk-eye-mem --init-config              # generate default config

# === V0.4 New Commands ===

# Environment fingerprint
hawk-eye-mem --env-fingerprint              # Show current env fingerprint JSON
hawk-eye-mem --reset-environment --force    # Clear stored fingerprints

# Remote HTTP server (lightweight, no extra deps)
hawk-eye-mem --serve --port 9240            # Start /metrics + /full HTTP endpoint
# curl http://127.0.0.1:9240/metrics        # Minimal system metrics
# curl http://127.0.0.1:9240/full           # Full resource snapshot

# Trend analysis
hawk-eye-mem --trend                        # Trend report with urgency
hawk-eye-mem --clear-history                # Clear history data

# Alert mode (pipe-friendly)
hawk-eye-mem --alert                        # Silent unless critical → JSON one-liner
hawk-eye-mem --alert --interval 60          # Periodic alert check

# The --env-fingerprint is AUTOMATIC — every run saves fingerprint + detects changes
# Environment changes appear in stderr and JSON `environment_change` field
```

### CLI 参数表格

| 参数 | 功能 | 版本 |
|------|------|:----:|
| `--json` | 完整 JSON 输出（含 system/agent_guidance） | V0.1 |
| `--metric <NAME>` | 单指标查询（total_mb/used_mb/available_mb/used_percent/pressure） | V0.1 |
| `--interval <SECS>` | 连续监控间隔（秒） | V0.1 |
| `--count <N>` | 采样次数（0=无限） | V0.1 |
| `--gpu-list` | 列出 GPU 及采集后端 | V0.3 |
| `--calibration-stats` | 校准统计 | V0.3 |
| `--reset-calibration` | 重置校准数据 | V0.3 |
| `--tokens-processed <N>` | 记录校准数据点 | V0.3 |
| `--model-name <NAME>` | 指定模型名称 | V0.3 |
| `--config <PATH>` | 指定配置文件 | V0.1 |
| `--init-config` | 生成默认配置 | V0.1 |
| `--env-fingerprint` | 输出当前环境指纹 JSON | V0.4 |
| `--reset-environment` | 重置环境指纹（需 --force） | V0.4 |
| `--serve` | 启动远程采集 HTTP 服务 | V0.4 |
| `--port` | HTTP 服务端口（默认 9240） | V0.4 |
| `--trend` | 输出趋势分析报告 | V0.4 |
| `--clear-history` | 清空历史数据 | V0.4 |
| `--alert` | 告警模式（仅 critical 时输出） | V0.4 |
| `--suggest-concurrency` | 根据系统资源建议并发数 | V0.4 |
| `--task-memory` | 配合 --suggest-concurrency 使用 | V0.4 |

### JSON Output Schema (V0.4)

```json
{
  "timestamp": "2026-05-22T01:00:00+00:00",
  "collection_duration_ms": 0.1,
  "system": {
    "total_mb": 7859,
    "used_mb": 3059,
    "available_mb": 4800,
    "used_percent": 38.9,
    "memory": { ... },
    "cpu": { "cores": 8, "load_avg_1m": 1.2, ... },
    "disk": { "path": "/", "total_mb": 512000, ... },
    "container_runtime": "docker",          // NEW V0.4: null if bare-metal
    "gpu": [{
      "name": "NVIDIA RTX 4090",
      "vram_total_mb": 24564,
      "vram_used_mb": 10240,
      "vram_used_percent": 41.7,
      "temperature_c": 65.0,
      "power_w": 250.0,
      "utilization_percent": 45.0,
      "backend": "nvml"
    }],
    "thermal": {
      "cpu_temp_c": 45.0,
      "gpu_temps_c": [65.0],
      "pressure": "normal",
      "note": "Temperature data is for reference only"
    },
    "agents": {
      "agents": [{"name": "hermes", "pid": 1234, "memory_mb": 256}],
      "count": 1,
      "total_agent_memory_mb": 256,
      "total_agent_cpu_percent": 1.5,
      "note": "Agent process detection for reference only"
    }
  },
  "agent_guidance": {
    "pressure": "medium",
    "estimated_safe_context_window": 1720320,
    "confidence": "conservative",
    "action": "monitor",
    "reason": "Moderate: 4800MB available, 38.9% used. Continue monitoring.",
    "suggestion": "Moderate memory pressure. Consider configuring model parameters for better accuracy."
  },
  "machine_state": {
    "state": "normal",
    "transition": "None",
    "note": "状态机仅在 --interval 连续监控模式下生效。"
  },
  "environment_change": {                   // NEW V0.4: only when detected
    "detected": true,
    "previous_fingerprint_id": "abc...",
    "changes": [{
      "resource": "memory",
      "previous_label": "16384MB",
      "current_label": "65536MB",
      "direction": "upgrade"
    }],
    "new_recommendation": "Memory has increased to 64GB. You can now safely run larger models."
  }
}
```

### Config File (V0.4)

Default location: `~/.config/hawk-eye-mem/config.toml`

```toml
[model]
bytes_per_token = 2048
margin = 30.0

[calibration]
enabled = true
max_samples = 100
min_samples_for_calibrated = 10

[state_machine]
warning_seconds = 30
critical_seconds = 60
recovery_seconds = 120
min_samples_warning = 3
min_samples_critical = 5

[multi_agent]
enabled = true
extra_process_names = ["my-agent", "test-agent"]

[gpu]
rocm_smi_path = "/opt/rocm/bin/rocm-smi"

# === V0.4 New Sections ===

[remote]
api_key = "your-secret-key"       # For --serve HTTP auth
nodes = ["http://192.168.1.10:9240", "http://192.168.1.11:9240"]

[history]
retention_days = 7                 # Auto-cleanup threshold
auto_record = true                 # Auto-record in --interval mode
```

## Agent Integration

### As a Decision Tool

The `agent_guidance` field is designed specifically for Agent consumption:

| `action` | Meaning | Agent Should |
|----------|---------|-------------|
| `ok` | Memory healthy | Proceed normally |
| `monitor` | Moderate pressure | Continue but check again soon |
| `reduce_context` | High pressure | Reduce context window, trim conversation history |
| `abort_safely` | Critical (<2GB or >92% used) | Save state and abort immediately |

### Metric Aliases (for `--metric`)

| Name | Output | Type |
|------|--------|------|
| `total_mb` | Total physical RAM | u64 |
| `used_mb` | Used RAM | u64 |
| `available_mb` | Available RAM | u64 |
| `used_percent` | Usage percentage | f64 (one decimal) |
| `pressure` | Pressure level | string |

## MCP 集成

秋毫mem provides a stdio MCP server with **7 tools**:

### 安装 MCP Server

```bash
# Quick install (macOS/Linux): run this on target machine
curl -fsSL https://raw.githubusercontent.com/qiuhaomem/HawkEye-Mem/main/scripts/install-hawkeye-mcp.sh | bash

# Or manual registration
hermes mcp add hawk-eye-mem --command python3 --args /path/to/hawkeye-mcp-server.py
```

### 可用工具

| 工具名 | 功能 | 参数 | 版本 |
|--------|------|------|:----:|
| `get_memory_status` | 完整资源状态（内存/CPU/磁盘/GPU/温度/Agent进程/容器运行时） | `tokens_processed` (可选) | V0.1, V0.3↑, V0.4↑ |
| `get_memory_metric` | 单指标查询 | `metric`: total_mb/used_mb/available_mb/used_percent/pressure | V0.1 |
| `get_calibration_status` | 校准状态（样本数/bytes_per_token/标准差/趋势/confidence） | `model_name` | V0.3 |
| `get_gpu_status` | GPU状态（名称/显存/温度/功耗/利用率/后端） | 无 | V0.3 |
| `get_agent_processes` | Agent进程检测（数量/内存占用） | 无 | V0.3 |
| `get_thermal_status` | 温度状态（CPU/GPU温度/压力等级） | 无 | V0.4 |
| `get_memory_guidance` | 决策建议（action/pressure/safe_context_window） | 无 | V0.1 |

### 使用示例

注册后，Agent 会在以下场景**自动调用**这些工具：
- 用户问"还剩多少内存" → 调用 `get_memory_status`
- Agent 判断是否需要缩上下文 → 调用 `get_memory_guidance`
- 快速检查压力等级 → 调用 `get_memory_metric("pressure")`
- 查看 GPU 状态 → 调用 `get_gpu_status`
- 检查温度 → 调用 `get_thermal_status`
- 检测同机 Agent → 调用 `get_agent_processes`
- 查看校准详情 → 调用 `get_calibration_status`

### 🐶 Dogfood First — MCP 安装与验证

**铁律：先给自己装上吃狗粮，再推给别人。**

当你需要部署秋毫mem MCP 到新机器时：

1. **自己机器上先装** — `cargo install --path .` + `hermes mcp add ...`
2. **本地验证 three ways**:
   - CLI 测试: `hawk-eye-mem --json` — 输出了 JSON 吗？有 system/agent_guidance 字段吗？
   - MCP stdio 协议测试: `echo '...' | python3 scripts/hawkeye-mcp-server.py` — 返回工具列表吗？
   - MCP 实际调用测试: `echo '...get_memory_status...' | python3 scripts/hawkeye-mcp-server.py` — 返回正常数据吗？
3. **确认没问题了再搞别人** — 推安装脚本或写说明

### 🚀 给同事/其他机器安装

```bash
# 一键脚本（推荐——自动检测 OS/ARCH，编译/下载 binary，注册 MCP，跑测试）
curl -fsSL https://raw.githubusercontent.com/qiuhaomem/HawkEye-Mem/main/scripts/install-hawkeye-mcp.sh | bash
```

脚本能力：
- 检测系统架构（Linux x86_64 / macOS ARM64 / macOS x86_64）
- 有 Rust → `cargo install --git` 从源码编译（最稳）
- 无 Rust → 尝试下载预编译 binary（Release 中目前只有 Linux binary）
- 自动下载 MCP Server 脚本到 `~/.hermes/scripts/`
- 注册 Hermes MCP
- 运行验证测试

**⚠️ macOS 注意事项：**
- Release v0.3.0 仅有 Linux binary，macOS binary 需 CI 跑完才上传
- 同事需要 Rust 工具链：`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- 安装脚本会自动走 `cargo install --git` 路径

### MCP Update Workflow

After adding new CLI features, always do (in order):
1. Update `scripts/hawkeye-mcp-server.py` — add new tools or extend existing ones
2. Update this SKILL.md — document new features and tools
3. Re-register MCP: `yes | hermes mcp add hawk-eye-mem --command python3 --args /path/to/hawkeye-mcp-server.py`
4. **Dogfood**: 本地验证 three ways（CLI + stdio 协议 + 实际调用）
5. Push and trigger CI, then confirm macOS binary is also uploaded to Release
6. **Then** push install script to others

See `references/mcp-tool-design-and-update.md` for detailed design principles, binary path handling, and CI/release workflow.

## Common Pitfalls

1. **Don't confuse `available_mb` with `free_mb`.** `available_mb` includes reclaimable cache/buffer memory — it's the real number for "how much can I use before swapping".
2. **First run shows disclaimer.** The first invocation outputs a disclaimer to stderr + creates `~/.config/hawk-eye-mem/.onboarded`. Subsequent runs are silent.
3. **Config file affects `confidence`.** Without config → `conservative` (30% margin). With config → `calibrated` (user-specified margin, no suggestion).
4. **`--config` requires file to exist.** Default path (~/.config/hawk-eye-mem/config.toml) silently returns `None` if missing; explicit `--config /path` fails with error if not found.
5. **`--init-config` is mutually exclusive** with `--json` and `--metric` — generates config then exits.
6. **SIGINT in continuous mode** (`--interval --count 0`): Ctrl+C completes current cycle, prints "Interrupted by user" to stderr, exits cleanly (code 0).
7. **Binary path:** After `cargo install`, the binary is at `~/.cargo/bin/hawk-eye-mem`. After local build, at `./target/release/hawk-eye-mem`. For system-wide access, copy to `/usr/local/bin/`.
8. **GPU collection** on macOS requires `feature = "gpu"` compilation flag (Metal API via sysctl fallback).
9. **Temperature** is "reference only" in V0.3 — automatic alerts come in V0.4.
10. **Multi-agent detection** only reads process names (comm), never cmdline arguments (CR-06 privacy).
11. **Calibration** requires at least 2 consecutive snapshots to compute delta. Use `--interval` mode or call twice.
12. **State machine** is only active in `--interval` / `--count 0` continuous monitoring mode.
13. **`-D warnings` in CI** treats all warnings as errors. Use `#[allow(dead_code)]` for cross-platform conditional compilation stubs.
14. **macOS memory calculation** uses `Pages occupied by compressor` (physical), NOT `Pages stored in compressor` (logical).
15. **macOS page_size** is now auto-detected from `vm_stat` header (fixes Intel Mac 4096 vs Apple Silicon 16384).
16. **`--model-size` parameter** is in billions (e.g. `--model-size 70` = 70B), NOT bytes. Code auto-multiplies by 1B internally.
17. **Environment fingerprint** is auto-saved on every normal run. `--env-fingerprint` reads saved data.
18. **`--serve` binds to 127.0.0.1** for security. Binding 0.0.0.0 prints ERROR and exits (CR-05).
19. **`--alert`** only outputs when pressure is critical/high. Silent otherwise (pipe-friendly).
20. **Trend analysis** requires ≥10 data points for a meaningful report. `--interval` mode auto-records.
21. **Container cgroup** detection is automatic — cgroup v1 (`memory.limit_in_bytes`) and v2 (`memory.max`) both supported. Values ≥ physical memory treated as "no limit".

## Verification Checklist

- [ ] `hawk-eye-mem --json` outputs valid JSON with `system` and `agent_guidance` fields
- [ ] `hawk-eye-mem --metric available_mb` outputs a positive integer
- [ ] `hawk-eye-mem --metric pressure` outputs one of `low|medium|high|critical`
- [ ] Config file changes `confidence` from `conservative` to `calibrated`
- [ ] `hawk-eye-mem --init-config` generates a valid config file
- [ ] `hawk-eye-mem --json --interval 1 --count 0` runs indefinitely, SIGINT exits cleanly
- [ ] Cross-platform: works on both Linux (`/proc/meminfo`) and macOS (`vm_stat + sysctl`)
- [ ] `hawk-eye-mem --gpu-list` detects GPU or says "No GPU detected"
- [ ] `hawk-eye-mem --calibration-stats --model-name test` shows calibration state
- [ ] Config with `[calibration]`, `[state_machine]`, `[multi_agent]`, `[gpu]`, `[remote]`, `[history]` sections parses correctly
- [ ] MCP tools all accessible
- [ ] **V0.4:** `hawk-eye-mem --env-fingerprint` returns valid JSON
- [ ] **V0.4:** `hawk-eye-mem --reset-environment --force` clears fingerprint
- [ ] **V0.4:** `hawk-eye-mem --serve --port 9999` starts and responds on `/metrics`
- [ ] **V0.4:** `hawk-eye-mem --trend` returns trend report (or "insufficient_data")
- [ ] **V0.4:** `hawk-eye-mem --alert` silent when pressure is low
- [ ] **V0.4:** Inside Docker: `container_runtime = "docker"`, total_mb respects cgroup limits

## 引用文件

- `references/adding-new-collector-workflow.md` — 新增 ResourceCollector 的 7步流程 + 常见错误
- `references/mcp-tool-design-and-update.md` — MCP 工具设计原则 + 更新工作流 + Binary 路径处理 + CI/Release 流程
