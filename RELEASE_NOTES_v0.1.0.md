# Hawk-Eye Mem v0.1.0 · 秋毫

> AI-Native memory monitoring CLI — let your Agent feel the RAM.

**秋毫mem** 是一个面向 AI Agent 的内存监控 CLI 工具。让大模型（LLM）在耗尽上下文窗口之前感知内存压力，主动做出调整——而不是死机了才报错。

## ✨ 功能亮点

- **跨平台内存采集** — Linux (`/proc/meminfo`) + macOS (`vm_stat` + `sysctl`)
- **Agent 智能引导** — 5级压力判断 + 4种建议动作（ok / monitor / reduce_context / abort_safely）
- **Machine-readable 输出** — `--json` 全量 JSON + `--metric <name>` 单值提取
- **彩色终端输出** — 压力等级颜色编码 + 仪表盘式内存布局
- **SIGINT 优雅退出** — `Ctrl+C` 不丢数据，flush stderr 后干净退出
- **配置文件** — `--init-config` 生成带注释的默认配置 + 3级配置优先级
- **置信度分级** — `conservative`（保守） / `calibrated`（校准）两档
- **首次运行引导** — 免责声明 + Quick Start 提示
- **Hermes Agent 集成** — 原生 Skill + MCP Server 双通道

## 📦 安装

### Linux (x86-64)

```bash
# 静态链接版（musl，不依赖 glibc，任何 Linux 都能跑）
curl -L -o hawk-eye-mem \
  https://github.com/qiuhaomem/-HawkEye-Mem/releases/download/v0.1.0/hawk-eye-mem-v0.1.0-linux-x86-64-musl
chmod +x hawk-eye-mem
./hawk-eye-mem --help
```

### macOS (ARM64)

```bash
curl -L -o hawk-eye-mem \
  https://github.com/qiuhaomem/-HawkEye-Mem/releases/download/v0.1.0/hawk-eye-mem-v0.1.0-macos-arm64
chmod +x hawk-eye-mem
xattr -d com.apple.quarantine hawk-eye-mem  # macOS 需要
./hawk-eye-mem --help
```

### 从源码安装

```bash
git clone https://github.com/qiuhaomem/-HawkEye-Mem.git
cd -HawkEye-Mem
cargo install --path .
hawk-eye-mem
```

## 🚀 快速开始

```bash
# 查看内存状态
hawk-eye-mem

# JSON 输出（给 AI Agent 用）
hawk-eye-mem --json

# 只取某个指标
hawk-eye-mem --metric available_mb

# 持续监控（每 2 秒一次，共 5 次）
hawk-eye-mem --interval 2 --count 5

# 校准模式（压力阈值更激进，适合 32GB+ 大内存机器）
hawk-eye-mem --confidence calibrated

# 生成配置文件
hawk-eye-mem --init-config
```

## 🧩 项目架构

```
hawk-eye-mem/
├── src/
│   ├── main.rs              # CLI 入口 + SIGINT
│   ├── collector/
│   │   ├── mod.rs           # 采集器 trait + 平台分派
│   │   ├── linux.rs         # Linux /proc/meminfo 采集
│   │   └── macos.rs         # macOS vm_stat + sysctl 采集
│   ├── guidance.rs          # Agent 引导引擎（压力+置信度）
│   └── config.rs            # TOML 配置 + 3级优先级
├── tests/                   # 33 单元测试 + 20 集成测试
├── scripts/
│   └── hawkeye-mcp-server.py  # MCP Server
└── docs/
    ├── 国产化工具链验证报告_V1.0.md
    └── skills/
        └── prompt-cache-strategy/SKILL.md
```

## 🧪 测试状态

- **53 个测试**（33 单元 + 20 集成）全平台通过 ✅
- **CI 双平台** Linux (glibc + musl) + macOS ARM64 ✅
- **macOS 实机验证** 同事 M1 MacBook 8GB 实测通过 ✅

## 🙏 致谢

- 工具链：DeepSeek-TUI · Reasonix · Cursor Pro
- 架构指导：Hermes Agent 社区

---

> **秋毫之末，察内存之变。** — 让每个 Agent 都装上一双敏锐的眼睛。
