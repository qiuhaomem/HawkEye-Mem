[English](./README-en.md) | [中文](./README.md)

<div align="center">

<img src="./assets/hawk-eye-logo.svg" alt="HawkEye Mem" width="120">

# 🦅 HawkEye Mem — The Memory Sensor Your AI Never Had

<h3>Your AI has been crunching for three hours.<br>You get up for coffee. Come back — OOM. Dead. No checkpoint.</h3>

| 🚀 **Blazing Fast** | 🧠 **AI-Native** | 🔌 **Full Stack** | 📈 **Self-Learning** |
|:-:|:-:|:-:|:-:|

[![Rust](https://img.shields.io/badge/Rust-1.70%2B-000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![CLI](https://img.shields.io/badge/CLI-✓-3fb950?style=flat-square)](https://github.com/qiuhaomem/HawkEye-Mem)
[![MCP](https://img.shields.io/badge/MCP-✓-58a6ff?style=flat-square)](https://github.com/qiuhaomem/HawkEye-Mem)
[![License](https://img.shields.io/badge/License-Apache--2.0-d29922?style=flat-square)](./LICENSE)
[![Version](https://img.shields.io/badge/Version-v0.7.0-f0883e?style=flat-square)](https://github.com/qiuhaomem/HawkEye-Mem/releases)
[![Tests](https://img.shields.io/badge/Tests-336_✓-3fb950?style=flat-square)](https://github.com/qiuhaomem/HawkEye-Mem/actions)
[![OS](https://img.shields.io/badge/OS-Linux%20|%20macOS%20|%20Docker-8b949e?style=flat-square)]()

<a href="https://github.com/qiuhaomem/HawkEye-Mem"><img src="https://readme-typing-svg.demolab.com?font=Fira+Code&weight=600&size=18&duration=2500&pause=1000&color=58A6FF&center=true&vCenter=true&width=750&lines=HawkEye+Mem+%E2%80%94+AI-Agent+Memory+Sensor;%3C1ms+per+check+%C2%B7+Binary+%3C1MB;15+MCP+Tools+%C2%B7+Agent+Decision+Engine+Ready" alt="Typing SVG" /></a>

</div>

<br>

<table>
<tr>
<td>🚀 <b>Blazing Fast</b></td>
<td>Each check takes &lt; 1ms. Binary size &lt; 1MB. You won't even know it's there.</td>
</tr>
<tr>
<td>🧠 <b>AI-Native</b></td>
<td>Not another dashboard for humans. JSON output + MCP protocol — your AI agent reads and reacts autonomously.</td>
</tr>
<tr>
<td>🔌 <b>Full Stack Monitoring</b></td>
<td>Memory · CPU · GPU · Disk · Thermal · Agent processes · Trend analysis · Container-aware — one shot, full picture.</td>
</tr>
<tr>
<td>📈 <b>Gets Smarter Over Time</b></td>
<td>Dynamic calibration, environment fingerprinting, state machine — the more you use it, the more accurate it gets.</td>
</tr>
</table>

---

<div align="center">

## What makes it different from `free` or `htop`

</div>

Those are **dashboards for humans**. You look, you decide, you act.

HawkEye Mem is a **sensor for AI agents**. Right before memory runs out, it tells your agent one thing:

> "Abort safely. Save state. Stop now."

Your AI doesn't need to understand what "memory" means. It reads the `action` field — and does what it says. **From "human-in-the-loop" to "self-healing agents" — one sensor away.**

<br>

| Comparison | `free -h` / `htop` | HawkEye Mem |
|:-----------|:-------------------|:------------|
| Designed for | **Humans** | **AI Agents** |
| Output | Terminal text | JSON / MCP Tool |
| Decision flow | Read → think → act | Read `action` → just act |
| Overhead | Heavy each time | &lt; 1ms, &lt; 1MB |

---

<div align="center">

## Architecture at a glance

<img src="./assets/architecture.svg" alt="HawkEye Mem Architecture" width="860">

</div>

---

<div align="center">

## Install in 30 seconds

</div>

### Path A: Run locally

```bash
# From source
git clone https://github.com/qiuhaomem/HawkEye-Mem.git
cd HawkEye-Mem
cargo build --release
sudo cp target/release/hawk-eye-mem /usr/local/bin/

# Check your machine state
hawk-eye-mem --json
```

### Path B: Integrate with your AI framework

**Using Hermes** — register as MCP tool:

```bash
hermes mcp add hawk-eye-mem --command python3 --args scripts/hawkeye-mcp-server.py
```

15 tools appear instantly. Your agent can ask "got enough memory?" before every heavy task.

**Using another framework** — shell out:

```bash
hawk-eye-mem --json | jq '.agent_guidance.action'
```

---

<div align="center">

## What it can do

</div>

### 🚀 Feature Quick Reference

| Feature | Command | What it does |
|:--------|:--------|:-------------|
| **Full checkup** | `hawk-eye-mem --json` | Memory + CPU + GPU + Disk + Thermal + Agents, one shot |
| **Single metric** | `hawk-eye-mem --metric available_mb` | Just one number, nothing else |
| **Pressure level** | `hawk-eye-mem --metric pressure` | low / medium / high / critical |
| **Continuous monitor** | `hawk-eye-mem --json --interval 5` | Poll every 5 seconds with state machine |
| **Model pre-check** | `hawk-eye-mem --can-run --model llama3-8b` | Can your machine run that model? |
| **Model comparison** | `hawk-eye-mem --can-run --compare qwen2-7b,phi-3-mini` | Pick the best model for your hardware |
| **GPU status** | `hawk-eye-mem --gpu-list` | VRAM/temperature/power/utilization per card |
| **Model calibration** | `hawk-eye-mem --tokens-processed 4096 --model-name llama3-8b` | Tell it real token usage, gets smarter |
| **Environment fingerprint** | `hawk-eye-mem --env-fingerprint` | Detect hardware changes automatically |
| **Trend analysis** | `hawk-eye-mem --trend` | 7-day memory trajectory — rising or falling? |
| **Concurrency advice** | `hawk-eye-mem --suggest-concurrency --task-memory 512` | How many sub-agents can run in parallel? |
| **Remote collection** | `hawk-eye-mem --serve --port 9240` | One machine serves, fleet pulls |
| **Cache gap analysis** | `hawk-eye-mem --analyze-cache-gaps` | 97% hit rate but targeting 99%? Find the gap |
| **Heartbeat** | `hawk-eye-mem --heartbeat` | One-line JSON — cron-ready, alert-friendly |
| **Showcase** 🆕 | `hawk-eye-mem --onboarding` | One command to show ALL features — system/cache/tokens/trends/concurrency, blows agent's mind |

### 🤖 MCP Tools (15 total)

Register with Hermes and your agent gains these abilities:

| Tool | What it does |
|------|-------------|
| `get_memory_status` | Full system snapshot (mem/CPU/disk/GPU/thermal/agents) + guidance |
| `get_memory_metric` | Single metric: total/used/available/percent/pressure |
| `get_memory_guidance` | Just the advice — action, pressure, safe context window |
| `get_gpu_status` | GPU list with VRAM/temp/power/utilization per card |
| `get_thermal_status` | CPU/GPU temp: normal / warning / critical |
| `get_agent_processes` | Co-located AI agents + resource usage |
| `get_calibration_status` | Model calibration state (samples/confidence) |
| `get_environment_fingerprint` | Environment fingerprint — what machine is this |
| `get_trend_report` | Trend analysis — is memory going up or down? |
| `get_concurrency_suggestion` | Safe concurrency — how many sub-agents? |
| `get_cache_strategy` | Cache strategy: aggressive / balanced / conservative / emergency |
| `get_cache_gaps_analysis` | Cache gap analysis — hit rate vs target |
| `get_heartbeat` | One-line heartbeat JSON |
| `run_token_audit` | Token spend audit — where's the API budget going? |
| `run_onboarding_showcase` 🆕 | One-shot showcase — system/cache/tokens/trends/concurrency/GPU/Agent/env all in one JSON |

---

<div align="center">

## Pressure Levels — What your AI should do

</div>

| Level | Meaning | What your AI should do |
|:------|:--------|:-----------------------|
| 🟢 `low` | Plenty of RAM | Go ahead, don't worry |
| 🟡 `medium` | Getting warm | Proceed, but check again soon |
| 🟠 `high` | Running low | Reduce context, be frugal |
| 🔴 `critical` | About to blow | Save state, STOP NOW |

---

<div align="center">

## Version History

</div>

| Version | Code Name | Date | One-liner |
|:--------|:----------|:-----|:----------|
| v0.1.0 | — | 2026-05-18 | Born: memory monitoring + agent guidance |
| v0.2.0 | — | 2026-05-20 | `--can-run` model pre-check + CPU/disk |
| v0.3.0 | — | 2026-05-22 | GPU + thermal + calibration + state machine |
| v0.4.0 | — | 2026-05-22 | Env fingerprint + remote serve + container aware + trend |
| v0.5.0 | 🎣 Operation Fishing | 2026-05-26 | Cache strategy + cost report + concurrency |
| v0.6.0 | 🎯 Precision Strike | 2026-05-30 | Cache gap analysis + heartbeat + token audit |
| v0.7.0 | 🦅 Showcase | 2026-05-31 | `--onboarding` showcase + macOS release + 100 limit tests |

### v0.6.0「Precision Strike」Highlights

**Cache Gap Analysis** — 97% hit rate but targeting 99%? Find the missing 2%.

```bash
hawk-eye-mem --analyze-cache-gaps --days 7 --target 99
```

Output breaks down misses by category (cold start / model switch / other), each with percentage and fix recommendations.

**Heartbeat Mode** — one-line JSON, cron-ready:

```bash
hawk-eye-mem --heartbeat
# {"pressure":"low","available_mb":3257,"used_percent":58.6,"action":"ok","timestamp":"2026-05-29T17:31:04"}
```

**336 tests, all green** — every version runs the full suite. No technical debt.

### v0.7.0「🦅 Showcase」Highlights

**Onboarding Showcase** — New users can see ALL features in one command.

```bash
# CLI — a stunning terminal report for humans
hawk-eye-mem --onboarding

# MCP — full JSON data for agents
# Tool: run_onboarding_showcase (zero parameters)
```

`--onboarding` output aggregates 7 major sections: System Checkup → Cache Strategy → Token Spend Overview → Trend Analysis → Concurrency Advice → GPU/Agent/Environment → Agent Decision Guidance. Each section has emoji status icons, **zero token cost**.

**Multi-platform release** — Linux (GNU + musl) + macOS (Apple Silicon) across all three targets.

**100 limit tests** — 93 test cases, 135 assertions, 100% pass, `--onboarding` completes in ~33ms.

---

<div align="center">

## Tuning estimates (optional)

</div>

By default, HawkEye Mem is conservative — it'd rather leave extra headroom than let you crash. For more precise estimates:

```bash
hawk-eye-mem --init-config
```

Edit `~/.config/hawk-eye-mem/config.toml` with your model parameters.

Skipping this is fine. The conservative defaults are safe.

---

<div align="center">

## Performance

</div>

Each check takes under **1ms**. The binary is under **1MB**. Your agent won't even feel it.

---

<div align="center">

## Changelog

</div>

- **2026/05/31** — v0.7.0「🦅 Showcase」: `--onboarding` showcase + macOS release + 100 limit tests
- **2026/05/30** — v0.6.0「Precision Strike」: Cache gap analysis + heartbeat + token audit + 336 tests green
- **2026/05/26** — v0.5.0「🎣 Operation Fishing」: Cache strategy Hermes Skill + cost report + concurrency coupling
- **2026/05/22** — v0.4.0: Environment fingerprint + remote HTTP serve + container aware + trend analysis
- **2026/05/21** — v0.3.0: GPU monitoring + thermal detection + calibration + state machine
- **2026/05/20** — v0.2.0: Model pre-check `--can-run` + CPU/disk monitoring
- **2026/05/18** — v0.1.0: Project born, core memory monitoring pipeline running

---

<div align="center">

## Feedback

</div>

v0.7 is the product of continuous iteration. Tried the onboarding showcase? [Let us know](https://github.com/qiuhaomem/HawkEye-Mem/issues/1) — was it helpful? Did you discover something new?

Every piece of feedback directly shapes the next version's features.

---

<div align="center">

## Disclaimer

</div>

HawkEye Mem gives **estimates**, not guarantees. You assume the risk of any decisions made based on its output.

See [DISCLAIMER.md](./DISCLAIMER.md).

---

<div align="center">

## License

</div>

[Apache-2.0](./LICENSE)

"HawkEye Mem" and "秋毫mem" are project trademarks.

---

<p align="center">🦀 Built with Rust · 🐍 Python MCP · ❤️ by <a href="https://github.com/qiuhaomem">qiuhaomem</a></p>
