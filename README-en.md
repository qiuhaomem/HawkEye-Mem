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
git clone https://github.com/qiuhaomem/-HawkEye-Mem.git
cd -HawkEye-Mem
cargo build --release
sudo cp target/release/hawk-eye-mem /usr/local/bin/
```

You'll need Rust: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`

**Pre-built binaries (coming soon)**

Download and run — no Rust toolchain needed.

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
```

---

## Integrating with your AI agent

**If you use Hermes**, register it as an MCP tool:

```bash
hermes mcp add hawk-eye-mem --command python3 --args scripts/hawkeye-mcp-server.py
```

Three tools show up automatically:

| Tool | What it does |
|------|-------------|
| `get_memory_status` | Full memory snapshot with agent guidance |
| `get_memory_metric` | Single metric: total, used, available, percent, pressure |
| `get_memory_guidance` | Just the advice — should I abort? reduce context? |

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

## Disclaimer

HawkEye Mem gives estimates, **not guarantees**. You assume the risk of any decisions made based on its output.

See [DISCLAIMER.md](./DISCLAIMER.md).

---

## License

[Apache-2.0](./LICENSE)

"HawkEye Mem" and "秋毫mem" are project trademarks.
