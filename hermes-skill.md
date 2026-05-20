     1|---
     2|name: hawk-eye-mem
     3|description: "Use when you need to check system memory pressure, estimate safe context window size for LLM inference, or decide whether to reduce context / abort safely based on available memory. AI-Native memory monitoring CLI (秋毫mem)."
     4|version: 1.0.0
     5|author: 秋毫mem Team
     6|license: MIT
     7|metadata:
     8|  hermes:
     9|    tags: [memory, monitoring, llm, system, mlops, agent-tools]
    10|    related_skills: [llama-cpp, serving-llms-vllm]
    11|---
    12|
    13|# HawkEye Mem (秋毫mem)
    14|
    15|AI-Native memory monitoring CLI — let your Agent feel the RAM.
    16|
    17|## Overview
    18|
    19|秋毫mem (HawkEye Mem) is a zero-dependency, cross-platform CLI tool that reads memory metrics and provides **Agent-friendly decisions**. It tells your LLM Agent:
    20|
    21|- How much memory is available right now
    22|- Whether it's safe to load a model / run inference
    23|- What action to take: `ok` → `monitor` → `reduce_context` → `abort_safely`
    24|- Estimated safe context window size (tokens)
    25|
    26|**Binary name:** `hawk-eye-mem`
    27|**Repo:** `github.com/qiuhaomem/-HawkEye-Mem`
    28|
    29|## When to Use
    30|
    31|- Before launching an LLM inference session — check if enough RAM is available
    32|- During long-running Agent loops — periodically check pressure to avoid OOM
    33|- When the Agent detects slow response or potential swap thrashing
    34|- Before loading a large model (via `llama-cpp`, `vllm`, etc.) — confirm memory budget
    35|- In cron jobs / scheduled tasks that track system health
    36|
    37|## Usage
    38|
    39|### Quick Commands
    40|
    41|```bash
    42|# Full JSON output (for Agent consumption)
    43|hawk-eye-mem --json
    44|
    45|# Single metric for scripts
    46|hawk-eye-mem --metric available_mb     # pure number
    47|hawk-eye-mem --metric pressure         # low|medium|high|critical
    48|hawk-eye-mem --metric used_percent     # e.g. 39.3
    49|
    50|# Continuous monitoring (JSON Lines)
    51|hawk-eye-mem --json --interval 5 --count 12    # 12 samples, 5s apart
    52|hawk-eye-mem --json --interval 10 --count 0    # infinite (Ctrl+C to stop)
    53|
    54|# Custom model config for calibrated estimates
    55|hawk-eye-mem --config /path/to/config.toml --json
    56|hawk-eye-mem --init-config              # generate default config
    57|```
    58|
    59|### JSON Output Schema
    60|
    61|```json
    62|{
    63|  "timestamp": "2026-05-20T01:00:00+00:00",
    64|  "collection_duration_ms": 0.1,
    65|  "system": {
    66|    "total_mb": 7859,
    67|    "used_mb": 3059,
    68|    "available_mb": 4800,
    69|    "used_percent": 38.9
    70|  },
    71|  "agent_guidance": {
    72|    "pressure": "medium",
    73|    "estimated_safe_context_window": 1720320,
    74|    "confidence": "conservative",
    75|    "action": "monitor",
    76|    "reason": "Moderate: 4800MB available, 38.9% used. Continue monitoring.",
    77|    "suggestion": "Moderate memory pressure. Consider configuring model parameters for better accuracy."
    78|  }
    79|}
    80|```
    81|
    82|### Config File
    83|
    84|Default location: `~/.config/hawk-eye-mem/config.toml`
    85|
    86|```toml
    87|[model]
    88|bytes_per_token = 2048    # Your model's bytes per token
    89|margin = 30.0             # Safety margin percentage
    90|```
    91|
    92|## Agent Integration
    93|
    94|### As a Decision Tool
    95|
    96|The `agent_guidance` field is designed specifically for Agent consumption:
    97|
    98|| `action` | Meaning | Agent Should |
    99||----------|---------|-------------|
   100|| `ok` | Memory healthy | Proceed normally |
   101|| `monitor` | Moderate pressure | Continue but check again soon |
   102|| `reduce_context` | High pressure | Reduce context window, trim conversation history |
   103|| `abort_safely` | Critical (<2GB or >92% used) | Save state and abort immediately |
   104|
   105|### Metric Aliases (for `--metric`)
   106|
   107|| Name | Output | Type |
   108||------|--------|------|
   109|| `total_mb` | Total physical RAM | u64 |
   110|| `used_mb` | Used RAM | u64 |
   111|| `available_mb` | Available RAM | u64 |
   112|| `used_percent` | Usage percentage | f64 (one decimal) |
   113|| `pressure` | Pressure level | string |
   114|
   115|## MCP 集成

秋毫mem 提供了 MCP Server，可以将内存监控注册为 Hermes 的 MCP 工具，Agent 自动调用、无需手动执行命令。

### 安装 MCP Server

```bash
# 1. 确保 hawk-eye-mem 二进制在 PATH 中
hawk-eye-mem --version

# 2. 注册 MCP Server（路径替换为实际位置）
hermes mcp add hawk-eye-mem --command python3 --args /path/to/scripts/hawkeye-mcp-server.py

# 3. 启用全部 3 个工具（Y），然后新会话生效
```

### 可用工具

| 工具名 | 功能 | 参数 |
|--------|------|------|
| `get_memory_status` | 获取完整内存状态 + Agent 决策建议 | 无参数 |
| `get_memory_metric` | 获取单个指标 | `metric`: total_mb/used_mb/available_mb/used_percent/pressure |
| `get_memory_guidance` | 获取 Agent 决策建议（action/pressure/tokens） | 无参数 |

### 使用示例

注册后，Agent 会在以下场景**自动调用**这些工具：
- 用户问"还剩多少内存" → 调用 `get_memory_status`
- Agent 判断是否需要缩上下文 → 调用 `get_memory_guidance`
- 快速检查压力等级 → 调用 `get_memory_metric("pressure")`

## Common Pitfalls
   116|
   117|1. **Don't confuse `available_mb` with `free_mb`.** `available_mb` includes reclaimable cache/buffer memory — it's the real number for "how much can I use before swapping".
   118|2. **First run shows disclaimer.** The first invocation outputs a disclaimer to stderr + creates `~/.config/hawk-eye-mem/.onboarded`. Subsequent runs are silent.
   119|3. **Config file affects `confidence`.** Without config → `conservative` (30% margin). With config → `calibrated` (user-specified margin, no suggestion).
   120|4. **`--config` requires file to exist.** Default path (~/.config/hawk-eye-mem/config.toml) silently returns `None` if missing; explicit `--config /path` fails with error if not found.
   121|5. **`--init-config` is mutually exclusive** with `--json` and `--metric` — generates config then exits.
   122|6. **SIGINT in continuous mode** (`--interval --count 0`): Ctrl+C completes current cycle, prints "Interrupted by user" to stderr, exits cleanly (code 0).
   123|7. **Binary path:** After `cargo install`, the binary is at `~/.cargo/bin/hawk-eye-mem`. After local build, at `./target/release/hawk-eye-mem`. For system-wide access, copy to `/usr/local/bin/`.
   124|
   125|## Verification Checklist
   126|
   127|- [ ] `hawk-eye-mem --json` outputs valid JSON with `system` and `agent_guidance` fields
   128|- [ ] `hawk-eye-mem --metric available_mb` outputs a positive integer
   129|- [ ] `hawk-eye-mem --metric pressure` outputs one of `low|medium|high|critical`
   130|- [ ] Config file changes `confidence` from `conservative` to `calibrated`
   131|- [ ] `hawk-eye-mem --init-config` generates a valid config file
   132|- [ ] `hawk-eye-mem --json --interval 1 --count 0` runs indefinitely, SIGINT exits cleanly
   133|- [ ] Cross-platform: works on both Linux (`/proc/meminfo`) and macOS (`vm_stat + sysctl`)
   134|