[English](./README-en.md) | [中文](./README.md)

# HawkEye Mem (秋毫mem)

**Your AI has been crunching for three hours. You get up for coffee. Come back — OOM. Dead. No checkpoint.**

Not your fault. That AI program had no idea it was running out of memory. It just kept going until the kernel killed it. No save, no warning, nothing.

You could check memory with `free -h` — but that's for humans. Your AI can't read that. It doesn't know what "danger" means.

**HawkEye Mem is the sensor your AI never had.**

It doesn't talk to you. It talks to your AI. Right before memory runs out, it tells your program one thing:

> "Abort safely. Save state. Stop now."

Your AI reads that, and it knows exactly what to do.

That's it.

---

## What makes it different from `free` or `htop`

Those are dashboards for humans. HawkEye Mem is a sensor for AI agents.

You look at a dashboard, then decide. HawkEye Mem lets your AI feel the pressure and decide for itself.

---

## One look at the output and you get it

```json
{
  "agent_guidance": {
    "action": "abort_safely",
    "estimated_safe_context_window": 4096,
    "reason": "Critical: 0MB available, 95% used. Abort safely to prevent OOM."
  }
}
```

Your AI doesn't need to understand what "memory" means. It doesn't calculate percentages. It just reads the `action` field and does what it says.

---

## When you need this

- You run local LLMs on your machine
- Your AI agent runs long tasks and you can't watch it 24/7
- You've had enough crashes
- You want your AI to be self-aware about memory

Install it, configure it, and it watches in the background. Before every heavy task, your AI asks "got enough memory?" — HawkEye Mem tells it yes or no, and how much room it has.

---

## Install

**From source**

```bash
git clone https://github.com/qiuhaomem/HawkEye-Mem.git
cd HawkEye-Mem
cargo build --release
sudo cp target/release/hawk-eye-mem /usr/local/bin/
```

You'll need Rust: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`

**Pre-built binaries**

Download from [GitHub Releases](https://github.com/qiuhaomem/HawkEye-Mem/releases):

```bash
# Linux (musl, statically linked, runs on any distro)
curl -L -o hawk-eye-mem https://github.com/qiuhaomem/HawkEye-Mem/releases/download/v0.4.0/hawk-eye-mem-v0.4.0-linux-x86-64-musl
chmod +x hawk-eye-mem
./hawk-eye-mem --help
```

---

## V0.2 New: Deployment Pre-Check

**Spending hours downloading a model only to have it crash on load — that's the worst.**
HawkEye Mem V0.2 introduces the `--can-run` command — assess whether your machine can run a model before you download it.

```bash
# Check if your machine can run Llama-3-8B
hawk-eye-mem --can-run --model llama3-8b

# JSON output for AI Agents
hawk-eye-mem --can-run --model llama3-8b --json

# Compare which model fits your machine best
hawk-eye-mem --can-run --compare llama3-8b,qwen2-7b,phi-3-mini

# Manually specify model parameters
hawk-eye-mem --can-run --model-size 7000000000 --quantization Q4_K_M --context 4096
```

`--can-run` reports one of three results:
- ✅ **Feasible** — Your machine can handle it, go ahead and download
- ⚠️ **Feasible with caveats** — Close, but you may need lower quantization, shorter context, or a smaller model
- ❌ **Infeasible** — Disk space or RAM gap is too large

It also includes a built-in library of 8 popular model specs (llama3-8b, qwen2-7b, deepseek-v2-lite, etc.). Use `--list-models` to see the full list:

```bash
hawk-eye-mem --list-models
```

Additionally, V0.2 adds **disk** and **CPU** monitoring. `hawk-eye-mem --json` output now includes `system.cpu` and `system.disk` (if a model cache directory is configured).

---

## V0.3 What's New

### 🎮 GPU Monitoring — See What Your LLM Is Eating

Running local LLMs and wondering if you'll OOM the GPU? V0.3 adds GPU detection — NVIDIA, AMD, and Apple Silicon. Every card's VRAM, temperature, power draw, and utilization.

```bash
# List all GPUs with detection backend
hawk-eye-mem --gpu-list

# JSON output includes GPU info
hawk-eye-mem --json
```

### 🔥 Thermal Monitoring — Don't Let Your Machine Melt

GPU running at 90°C while you're not watching? V0.3 adds CPU/GPU temperature with three alert levels: `normal`, `warning`, `critical`.

```bash
# Temperature shows up in JSON output
hawk-eye-mem --json
# Look for system.thermal.cpu_temp_c and pressure
```

### 🎯 Model Calibration — Gets Smarter Over Time

HawkEye Mem's default `bytes_per_token` is conservative. V0.3 introduces **dynamic calibration** — tell it how many tokens you actually processed, and it adjusts its estimates. The more you use it, the more accurate it gets.

```bash
# Record a calibration data point
hawk-eye-mem --tokens-processed 4096 --model-name llama3-8b

# Check calibration state
hawk-eye-mem --calibration-stats --model-name llama3-8b

# Reset calibration data
hawk-eye-mem --reset-calibration --model-name llama3-8b
```

### 🔄 State Machine — Smarter Continuous Monitoring

V0.2's `--interval` mode was a dumb sampler. V0.3 adds a state machine: sustained pressure upgrades the alert level, recovery downgrades it. No more false alarms.

```bash
# Continuous monitoring with state machine
hawk-eye-mem --json --interval 5
```

### 👥 Multi-Agent Detection

Multiple AI agents on one machine fighting for RAM? V0.3 detects co-located agent processes and aggregates their CPU and memory usage.

```bash
# JSON output includes system.agents
hawk-eye-mem --json
```

All V0.3 features work out of the box. If you want to tune, edit `~/.config/hawk-eye-mem/config.toml` — there are `[gpu]`, `[calibration]`, `[state_machine]`, and `[multi_agent]` sections.

---

## V0.4 What's New

### 🏠 Environment Fingerprint — Move Your Agent, No Sweat

Your agent works fine on one machine. Migrate it to another — boom. Different RAM, different CPU, maybe no GPU at all.
HawkEye Mem V0.4 introduces **environment fingerprinting**. First run captures your machine's hardware profile. Every subsequent startup compares against it.
If the hardware got upgraded, it says "you can run bigger models now." Downgraded? "Tighten your context window." Your agent never flies blind.

```bash
# View current fingerprint
hawk-eye-mem --env-fingerprint

# Rescan and reset
hawk-eye-mem --reset-environment
```

### 🌐 Remote Collection — One Machine, Full Fleet View

SSH'ing into every box in your cluster to check memory? That's a waste of time.
Fire up `--serve` on a collector node, and every other machine hits the `/metrics` endpoint to push their stats.
API Key auth + rate limiting on port 9240. Secure, simple, centralized.

```bash
# Start the metrics server
hawk-eye-mem --serve --api-key your-secret-key

# Pull from another machine
curl http://192.168.1.100:9240/metrics -H "X-API-Key: your-secret-key"
```

### 📦 Container Aware — Works Inside Docker & K8s

Run `free -h` inside a container and you see the host's RAM — not your container's cgroup limit.
HawkEye Mem V0.4 auto-detects cgroup v1/v2 memory and CPU limits. It knows the container's actual ceiling, not the host's.
Compatible with Docker, Kubernetes, and Podman. No config needed.

```bash
# Just use it — container limits detected automatically
hawk-eye-mem --json
```

### 📈 Trend Analysis — It Has a Memory Now

Old HawkEye Mem only saw the present moment. V0.4 remembers the past 7 days.
`--trend` shows you the trajectory: rising, stable, or falling. A steady climb means trouble is brewing.

```bash
# See memory trends over the past week
hawk-eye-mem --trend

# Wipe the history
hawk-eye-mem --clear-history
```

### 🤖 Multi-Agent Awareness — Watch Your Whole Swarm

Running multiple agents on one box? V0.4 tracks CPU and memory per agent process and rolls up the total.
Give each agent a name and see at a glance who's the resource hog.

```bash
# Monitor all agents with custom names
hawk-eye-mem --json --agents --agent-name "my-bot-1"
```

### 🚨 Alert Mode — Minimal JSON for Alerting

Plugging HawkEye Mem into your alert pipeline? `--alert` outputs only the essentials — compact JSON designed for Prometheus Alertmanager, PagerDuty, Slack, or any webhook.

```bash
# Minimal JSON, perfect for alerting systems
hawk-eye-mem --alert
```

### 🤯 Physical AI — First Steps

Want your agent orchestrator to know "how many sub-agents can this machine actually run?"
`--suggest-concurrency` crunches real-time RAM, CPU, and GPU data and tells you the optimal parallel agent count.

```bash
# How many agents can I run right now?
hawk-eye-mem --suggest-concurrency
```

---

## Usage

```bash
# Full memory report for your AI
hawk-eye-mem --json

# Just one number: how much RAM is free
hawk-eye-mem --metric available_mb

# What's the pressure level
hawk-eye-mem --metric pressure

# Poll every 5 seconds, 10 times
hawk-eye-mem --json --interval 5 --count 10

# Keep watching until you hit Ctrl+C
hawk-eye-mem --json --interval 5 --count 0

# Check if your machine can run a model (new in V0.2)
hawk-eye-mem --can-run --model llama3-8b

# Compare which model fits your machine best
hawk-eye-mem --can-run --compare llama3-8b,qwen2-7b,phi-3-mini

# List all supported models
hawk-eye-mem --list-models

# Check GPU status
hawk-eye-mem --gpu-list

# Model calibration
hawk-eye-mem --tokens-processed 4096 --model-name llama3-8b
hawk-eye-mem --calibration-stats --model-name llama3-8b

# Environment fingerprint
hawk-eye-mem --env-fingerprint

# Trend analysis
hawk-eye-mem --trend

# Concurrency suggestion
hawk-eye-mem --suggest-concurrency --task-memory 512

# Start remote collection server
hawk-eye-mem --serve --port 9240
```

---

## Integrating with your AI agent

**If you use Hermes**, register it as an MCP tool:

```bash
hermes mcp add hawk-eye-mem --command python3 --args scripts/hawkeye-mcp-server.py
```

Twelve tools show up automatically:

| Tool | What it does |
|------|-------------|
| `get_memory_status` | Full system snapshot (memory/CPU/disk/GPU/thermal/agents) + guidance |
| `get_memory_metric` | Single metric: total, used, available, percent, pressure |
| `get_memory_guidance` | Just the advice — action, pressure, safe context window |
| `get_gpu_status` | GPU list with VRAM/temperature/power/utilization per card |
| `get_thermal_status` | CPU/GPU temperature: normal/warning/critical |
| `get_agent_processes` | Co-located AI agents + resource usage |
| `get_calibration_status` | Model calibration state |
| `get_environment_fingerprint` | Environment fingerprint — what machine is this |
| `get_trend_report` | Trend analysis — is memory going up or down |
| `get_concurrency_suggestion` | Safe concurrency — how many sub-agents can run |
| `reset_environment_fingerprint` | Reset environment fingerprint |
| `start_remote_server` | Start remote collection HTTP service |

**If you use another framework**, just shell out to `hawk-eye-mem --json`, parse the output, and follow `agent_guidance.action`.

---

## Tuning estimates (optional)

By default, HawkEye Mem is conservative — it'd rather leave extra headroom than let you crash. If you want more precise estimates, tell it about your model:

```bash
hawk-eye-mem --init-config
```

Then edit `~/.config/hawk-eye-mem/config.toml` with your model parameters.

Skipping this is fine. The conservative defaults are safe.

---

## What the pressure levels mean

| Level | What it means | What your AI should do |
|-------|---------------|------------------------|
| `low` | Plenty of RAM | Go ahead |
| `medium` | Getting warm | Proceed, but check again soon |
| `high` | Running low | Reduce context, be frugal |
| `critical` | About to blow | Save state, stop NOW |

---

## Performance

It won't slow you down. Each check takes under 1ms. The binary is under 1MB.

---

## Feedback

V0.2 is an early release and we're actively collecting feedback. If you've tried `--can-run`, let us know: how accurate was it? Did it help?

👉 [Feedback Issue](https://github.com/qiuhaomem/HawkEye-Mem/issues/1)

Every piece of feedback directly shapes V0.3's feature planning.

---

## Disclaimer

HawkEye Mem gives estimates, **not guarantees**. You assume the risk of any decisions made based on its output.

See [DISCLAIMER.md](./DISCLAIMER.md).

---

## License

[Apache-2.0](./LICENSE)

"HawkEye Mem" and "秋毫mem" are project trademarks.
