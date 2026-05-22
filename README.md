[English](./README-en.md) | [中文](./README.md)

# 秋毫mem (HawkEye Mem)

**你的电脑跑着 AI 程序，跑了三个小时，你走开倒了杯水，回来一看——崩了。内存挤爆了。**

不是你的错，是那个 AI 程序自己不知道"快没内存了"。它闷头算，算到内存炸了，连个存档都没给你留。

你想提前看一眼还剩多少内存？可以，敲 `free -h`，出来一坨字符。你能看懂，AI 程序看不懂。它不是人，它不知道什么叫"危险"。

**秋毫mem 就是干这个的。**

它不是给你看的，是给你的 AI 程序看的。在内存快炸之前，它扔给 AI 程序一句话：

> "撑不住了，缩小上下文，赶紧存档。"

AI 程序拿到这句话，就知道自己该怎么做了。

就这么简单。

---

## 和 `free`、`htop` 的区别，一句话说清楚

那些工具是给人看的仪表盘。秋毫mem 是给 AI 程序装的传感器。

你去看仪表盘，然后手动调。秋毫mem 是让 AI 程序自己感知，自己调。

---

## 看一眼输出你就懂了

```json
{
  "agent_guidance": {
    "action": "reduce_context",
    "estimated_safe_context_window": 4096,
    "reason": "临界：内存不足，请立即中止以避免 OOM。"
  }
}
```

AI 程序拿到这个，不用理解"内存"是什么，不用算百分比，它只需要看 `action` 那个字段：`reduce_context`，然后照做。

---

## 什么时候用它

- 你在自己电脑上跑本地大模型
- 你让 AI 程序长时间干活，不能一直盯着
- 程序崩了几次你受不了了
- 你想让 AI 程序自己学会"看内存脸色"

装上，配好，它就在后台帮你盯着。AI 程序每次干活之前问它一句"还有内存吗"，它告诉你还能不能干、能干多大。

---

## 安装

**从源码编译**

```bash
git clone https://github.com/qiuhaomem/HawkEye-Mem.git
cd HawkEye-Mem
cargo build --release
sudo cp target/release/hawk-eye-mem /usr/local/bin/
```

需要先装 Rust：`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`

**直接下载二进制**

从 [GitHub Releases](https://github.com/qiuhaomem/HawkEye-Mem/releases) 下载预编译的二进制文件：

```bash
# Linux (musl 静态链接，任何发行版都能跑)
curl -L -o hawk-eye-mem https://github.com/qiuhaomem/HawkEye-Mem/releases/download/v0.4.0/hawk-eye-mem-v0.4.0-linux-x86-64-musl
chmod +x hawk-eye-mem
./hawk-eye-mem --help
```

---

## V0.2 新增：部署前检测

**花几小时下载一个模型，加载时崩了，这是最难受的。**
秋毫mem V0.2 新增了 `--can-run` 命令——在你下载模型之前，先评估你的机器能不能跑得动。

```bash
# 看你的机器能不能跑 Llama-3-8B
hawk-eye-mem --can-run --model llama3-8b

# JSON 输出给 AI Agent 用
hawk-eye-mem --can-run --model llama3-8b --json

# 看看几个模型里哪个最适合你的机器
hawk-eye-mem --can-run --compare llama3-8b,qwen2-7b,phi-3-mini

# 手动指定模型参数
hawk-eye-mem --can-run --model-size 7000000000 --quantization Q4_K_M --context 4096
```

`--can-run` 会告诉你三种结果：
- ✅ **可行** — 你的机器完全能跑，放心下载
- ⚠️ **有条件** — 差一点，但可以降量化/减上下文/换小模型
- ❌ **不可行** — 磁盘空间或内存差距太大

同时内置了 8 个主流模型的参数库（llama3-8b、qwen2-7b、deepseek-v2-lite 等），`--list-models` 查看完整列表：

```bash
hawk-eye-mem --list-models
```

此外，V0.2 还新增了**磁盘**和**CPU**监控。`hawk-eye-mem --json` 的输出现在包含 `system.cpu` 和 `system.disk`（如果有模型缓存目录的话）。

---

## V0.3 新功能

### 🎮 GPU 监控 — 你的大模型吃多少显存，一眼看清

跑大模型最怕什么？显存爆了。秋毫mem V0.3 开始能看到 GPU 了——NVIDIA、AMD、Apple Silicon 全支持，每个 GPU 的显存、温度、功耗、利用率一目了然。

```bash
# 列出所有 GPU 和采集后端
hawk-eye-mem --gpu-list

# JSON 输出里直接带 GPU 信息
hawk-eye-mem --json
```

### 🔥 温度监控 — 别让你的机器烧起来

GPU 跑到 90°C 还在暴力推理？秋毫mem V0.3 增加了 CPU/GPU 温度检测，分三档告警：`normal`（正常）、`warning`（有点热了）、`critical`（快到红线了）。

```bash
# 看温度
hawk-eye-mem --json
# 输出里有 system.thermal.cpu_temp_c 和 pressure
```

### 🎯 模型校准 — 越用越准

秋毫mem 估算上下文窗口时，默认的 `bytes_per_token` 是保守值。V0.3 引入了**动态校准**——你每跑一次推理，告诉它实际用了多少 token，它自己学习，越估越准。

```bash
# 创建一个校准记录
hawk-eye-mem --tokens-processed 4096 --model-name llama3-8b

# 看校准状态
hawk-eye-mem --calibration-stats --model-name llama3-8b

# 重置
hawk-eye-mem --reset-calibration --model-name llama3-8b
```

### 🔄 状态机 — 连续监控更聪明

V0.2 的 `--interval` 连续监控只是定时采样。V0.3 引入了状态机——检测到压力持续上涨就升级警告等级，恢复正常就降级，不会一惊一乍的。

```bash
# 连续监控，状态机自动生效
hawk-eye-mem --json --interval 5
```

### 👥 多 Agent 检测

一台机器跑多个 AI Agent？互相抢资源都不知道。秋毫mem V0.3 能检测同机运行的 Agent 进程，汇总 CPU 和内存占用。

```bash
# 输出里多了 system.agents 字段
hawk-eye-mem --json
```

所有 V0.3 功能开箱即用，不需要额外配置。如果想调参，编辑 `~/.config/hawk-eye-mem/config.toml`，有 `[gpu]`、`[calibration]`、`[state_machine]`、`[multi_agent]` 四个配置段。

---

## V0.4 新功能

### 🏠 环境指纹 — Agent 搬家也不怕

你的 Agent 在一台机器上跑得好好的，换台机器——崩了。因为新机器的内存变了、CPU 变了、GPU 也没了。
秋毫mem V0.4 引入了**环境指纹**。首次运行自动采集机器配置生成指纹，之后每次启动自动比对。
升级了告诉你"可以跑更大模型"，降级了告诉你"建议缩上下文"，绝不让 Agent 硬撑。

```bash
# 查看当前环境指纹
hawk-eye-mem --env-fingerprint

# 重新采集环境指纹
hawk-eye-mem --reset-environment
```

### 🌐 远程采集 — 一台机器看全局

集群里几十台机器，每台都去 SSH 登录看？太累了。
秋毫mem V0.4 可以用 `--serve` 启动 HTTP 服务，暴露 `/metrics` 端点，其他机器直接拉取指标。
API Key 认证 + 速率限制，默认 9240 端口，安全又省心。

```bash
# 启动采集服务
hawk-eye-mem --serve --api-key your-secret-key

# 另一台机器拉取指标
curl http://192.168.1.100:9240/metrics -H "X-API-Key: your-secret-key"
```

### 📦 容器适配 — Docker/K8s 里也能正确感知

在容器里跑 `free -h`，看到的是宿主机的内存，不是你容器的限制。
秋毫mem V0.4 能自动检测 cgroup v1/v2 的内存和 CPU 限制，不把宿主机内存当自己的。
Docker、Kubernetes、Podman 全兼容，开箱即用。

```bash
# 在容器里直接用，自动识别 cgroup 限制
hawk-eye-mem --json
```

### 📈 趋势分析 — 有记忆了

以前秋毫mem 只能看当前这一瞬间。现在它能记住过去 7 天的数据。
用 `--trend` 看内存走势：是上升、稳定还是下降？连续上涨就要警惕了。

```bash
# 查看内存趋势
hawk-eye-mem --trend

# 清空历史数据
hawk-eye-mem --clear-history
```

### 🤖 多Agent 增强 — 连自己的 Agent 兄弟也管

一个机器上跑了好几个 Agent？秋毫mem V0.4 能同时监控所有 Agent 进程的 CPU 和内存占用，加总告诉你总量。
还可以给每个 Agent 起名字，一眼看出来谁在吃资源。

```bash
# 查看所有 Agent 的占用情况（带自定义名称）
hawk-eye-mem --json --agents --agent-name "my-bot-1"
```

### 🚨 告警模式

要把秋毫mem 接进告警系统？V0.4 的 `--alert` 模式输出最小化 JSON，只有最关键的信息。
适配 Prometheus Alertmanager、飞书、钉钉、Slack 等任何告警通道。

```bash
# 告警模式，输出最小化 JSON
hawk-eye-mem --alert
```

### 🤯 物理AI 第一步

想让你的 Agent 框架知道"这台机器到底能跑几个子 Agent"？
秋毫mem V0.4 的 `--suggest-concurrency` 基于实时物理资源（内存、CPU、GPU），智能算出可以并行跑多少个子 Agent。

```bash
# 看看这台机器能同时跑几个 Agent
hawk-eye-mem --suggest-concurrency
```

---

## 怎么用

```bash
# 给 AI 程序看（完整内存报告）
hawk-eye-mem --json

# 只看还剩多少内存
hawk-eye-mem --metric available_mb

# 看内存压力等级
hawk-eye-mem --metric pressure

# 每 5 秒查一次，采 10 次
hawk-eye-mem --json --interval 5 --count 10

# 一直盯着，按 Ctrl+C 停
hawk-eye-mem --json --interval 5 --count 0

# 排查部署前能否运行某个模型
hawk-eye-mem --can-run --model llama3-8b

# 对比哪个模型最适合你的电脑
hawk-eye-mem --can-run --compare llama3-8b,qwen2-7b,phi-3-mini

# 列出内置支持的模型
hawk-eye-mem --list-models

# 看 GPU 状态
hawk-eye-mem --gpu-list

# 模型校准
hawk-eye-mem --tokens-processed 4096 --model-name llama3-8b
hawk-eye-mem --calibration-stats --model-name llama3-8b

# 环境指纹
hawk-eye-mem --env-fingerprint

# 趋势分析
hawk-eye-mem --trend

# 并发度建议
hawk-eye-mem --suggest-concurrency --task-memory 512

# 启动远程采集服务
hawk-eye-mem --serve --port 9240
```

---

## 怎么让你的 AI 程序用上它

**如果你用 Hermes**，注册成 MCP 工具就行：

```bash
hermes mcp add hawk-eye-mem --command python3 --args scripts/hawkeye-mcp-server.py
```

注册后 AI 程序就能直接调这 12 个工具了：

| 工具名 | 干嘛的 |
|--------|--------|
| `get_memory_status` | 看完整系统状态（内存/CPU/磁盘/GPU/温度/Agent）+ 建议 |
| `get_memory_metric` | 看单个指标（总内存、已用、可用、使用率、压力） |
| `get_memory_guidance` | 只看建议（该不该缩、安不安全、能跑多少token） |
| `get_gpu_status` | GPU 列表 + 每张卡的显存/温度/功耗/利用率 |
| `get_thermal_status` | CPU/GPU 温度，分 normal/warning/critical 三档 |
| `get_agent_processes` | 同机运行的 AI Agent 列表 + 占用 |
| `get_calibration_status` | 看模型校准状态 |
| `get_environment_fingerprint` | 环境指纹——当前机器是谁 |
| `get_trend_report` | 趋势分析——内存涨了还是跌了 |
| `get_concurrency_suggestion` | 并发度建议——能跑几个子Agent |
| `reset_environment_fingerprint` | 重置环境指纹 |
| `start_remote_server` | 启动远程采集 HTTP 服务 |

**如果你用别的框架**：直接 `hawk-eye-mem --json`，拿到 JSON 输出，读 `agent_guidance.action` 字段，照做就行。

---

## 给估算调准一点（可选）

秋毫mem 默认的估算是保守的，它宁可多留一些内存余量，也不会让你崩掉。如果你想让估算更精确，可以告诉它你用的是什么模型：

```bash
hawk-eye-mem --init-config
```

然后编辑 `~/.config/hawk-eye-mem/config.toml`，填上你的模型参数。

不配置也没关系，默认的保守估计足够安全。

---

## 压力水位是什么意思

| 水位 | 意思是 | AI 程序该怎么做 |
|------|--------|----------------|
| `low` | 内存充裕 | 放心跑 |
| `medium` | 还行，但要注意了 | 接着跑，勤问着点 |
| `high` | 不多了 | 缩小上下文，省着用 |
| `critical` | 马上炸了 | 赶紧存档，别再跑了 |

---

## 性能

不会拖慢你的系统。查一次不到 1 毫秒，二进制不到 1MB。

---

## 反馈

V0.2 是一个早期版本，我们正在收集反馈。如果你试用了 `--can-run`，请告诉我们：检测准不准？有没有帮到你？

👉 [反馈 Issue](https://github.com/qiuhaomem/HawkEye-Mem/issues/1)

你的每一条反馈都会直接影响 V0.3 的功能规划。

---

## 注意

秋毫mem 给的建议是基于估算的，**不一定百分之百准确**。用它做的决策，风险你自己担着。

详细说明看 [DISCLAIMER.md](./DISCLAIMER.md)。

---

## 许可证

[Apache-2.0](./LICENSE)

"秋毫mem"和"HawkEye Mem"是项目商标。
