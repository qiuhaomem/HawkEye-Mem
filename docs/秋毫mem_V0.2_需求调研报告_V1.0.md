# 秋毫mem V0.2 · 需求调研报告

**文档编号**：HM-PRD-002
**版本**：V1.0
**日期**：2026-05-20
**编写人**：高级产品经理

---

### 一、调研背景与目标

V0.1 验证了核心闭环：Agent 运行时内存监控 → 语义建议 → Agent 自主调整。V0.2 的核心命题是：**将秋毫mem 从"运行中报警"升级为"部署前评估 + 运行中监控"的完整环境感知层。**

本次调研围绕三个新增监控维度（磁盘、CPU、GPU）和一个新增交互模式（部署前评估）展开。


### 二、用户调研

#### 2.1 访谈摘要

| 来源 | 用户类型 | 痛点 | 频率 |
|------|----------|------|------|
| M1 Mac 5GB 测试用户 | 本地模型开发者 | 不知道自己的机器能跑多大模型，每次都要试 | 每次部署新模型 |
| r/LocalLLaMA 社区 | 本地模型爱好者 | 下载了模型跑不起来，浪费带宽和时间 | 高频 |
| GitHub Issues（竞品） | Agent 开发者 | Agent 部署后频繁 OOM，缺少部署前检查 | 高频 |
| 企业内部试用人 | 运维转 AI | 多个 Agent 抢资源，没有全局视角 | 中频 |

#### 2.2 用户原声

> "我下了 Llama-3-8B，4 个小时下载完，加载时 OOM 了。我要能提前知道跑不动，我就下 3B 版本了。" —— Reddit 用户

> "每次部署新 Agent 之前，我都得手动算：内存够不够、硬盘够不够、CPU 会不会跑满。没有工具一次告诉我。" —— 内部试用用户


### 三、社区调研

#### 3.1 竞品功能缺口

| 竞品 | 磁盘监控 | CPU监控 | GPU监控 | 部署前评估 | 语义建议 |
|------|----------|---------|---------|-----------|----------|
| `free` | ❌ | ❌ | ❌ | ❌ | ❌ |
| `htop` | ❌ | ✅ | ❌ | ❌ | ❌ |
| `nvidia-smi` | ❌ | ❌ | ✅ | ❌ | ❌ |
| `glances` | ✅ | ✅ | ⚠️ | ❌ | ❌ |
| `ollama` | ❌ | ❌ | ⚠️ | ❌ | ❌ |
| **秋毫mem V0.2** | ✅ | ✅ | ✅ (实验性) | ✅ | ✅ |

**核心发现**：没有任何工具提供"部署前评估"能力。这是明确的蓝海。

#### 3.2 社区热点话题

| 话题 | 来源 | 热度 | 与V0.2关联 |
|------|------|------|-----------|
| "我这配置能跑XX模型吗" | Reddit/知乎 | 极高 | 直接对应 `--can-run` |
| 模型下载后加载失败 | GitHub Issues | 高 | 磁盘+内存联合评估 |
| 多Agent资源争抢 | Hermes社区 | 中高 | V0.3，但V0.2需预留架构 |
| Apple Silicon统一内存监控 | r/LocalLLaMA | 中 | GPU Collector 预留 |


### 四、技术可行性

| 监控维度 | 数据源 | 采集难度 | 稳定性 | 跨平台 |
|----------|--------|----------|--------|--------|
| 内存 | `/proc/meminfo` / `sysctl` | 低 | 高 | ✅ |
| 磁盘 | `statvfs` | 低 | 高 | ✅ (POSIX) |
| CPU | `/proc/loadavg` / `sysctl` | 低 | 高 | ✅ |
| GPU (NVIDIA) | NVML 或 `nvidia-smi` 解析 | 中 | 中 | ⚠️ 仅 NVIDIA |
| GPU (Apple Silicon) | Metal API | 高 | 低 | ⚠️ 仅 macOS |
| GPU (AMD) | ROCm | 高 | 低 | ⚠️ 仅 Linux+AMD |

**结论**：内存、磁盘、CPU 三个维度在 V0.2 可以稳定交付。GPU 实验性纳入，标记 `--features gpu`。


### 五、V0.2 与 V0.1 对比

| 维度 | V0.1 | V0.2 |
|------|------|------|
| 监控资源 | 仅内存 | 内存 + 磁盘 + CPU + GPU(实验性) |
| 交互模式 | 被动查询 | 部署前评估 + 被动查询 |
| 核心命令 | `--json` `--metric` | 新增 `--can-run` |
| Agent 感知 | 运行时何时降级 | 部署前能否跑 + 运行时何时降级 |
| 用户覆盖 | 已在跑Agent的开发者 | 准备部署Agent的开发者 + 已在跑的 |
| 类比 | 烟雾报警器 | 看房检测 + 烟雾报警器 |


### 六、需求优先级（MoSCoW）

| 优先级 | 需求 |
|--------|------|
| **Must Have** | `--can-run` 部署评估、磁盘监控、CPU监控、模型参数库 |
| **Should Have** | GPU显存实验性（NVIDIA）、压力阈值v2校准 |
| **Could Have** | ~~多Agent基础检测~~ → **延后至V0.3**（经评审决定） |
| **Won't Have** | GPU完整支持（AMD/Apple Silicon）、动态校准、守护进程 |


## 秋毫mem V0.2 · 五级需求分解表

---

### 应用端 1：AI Agent 部署前环境评估

| 一级 | 二级 | 三级 | 四级 | 五级 |
|------|------|------|------|------|
| **Agent部署前评估** | **--can-run 命令** | **模型参数输入** | `--can-run --model llama3-8b` | 从内置模型参数库查找；也支持手动输入 `--model-size 8000000000 --quantization Q4_K_M --context 8192` |
| | | | `--can-run --model llama3-8b --json` | JSON输出 `deployment_assessment` 字段；不输出 `agent_guidance` |
| | | **verdict 判定** | 判定能否部署 | 三档：`feasible`（全部通过）/ `feasible_with_caveats`（有警告）/ `infeasible`（不通过）；判定逻辑见约束表 |
| | | **约束检测** | 逐资源检测并输出 `constraints` 数组 | 每项包含：`resource`、`required_mb`、`available_mb`、`gap_mb`、`suggestion`；只有 `gap_mb > 0` 时才输出该项 |
| | | **降级方案生成** | 输出 `safe_options` | 按优先级生成：降低量化→降低上下文→换小模型；最多输出 3 个方案 |
| | **模型参数库** | **预置模型配置** | 内置 8 个主流模型 | 首批覆盖：Llama-3-8B、Qwen2-7B、DeepSeek-V2-Lite、Mistral-7B、Phi-3-Mini、Gemma-2-9B、Yi-6B、ChatGLM3-6B |
| | | | 每模型包含 | `model_size_b`、`quantizations`（含每量化的 `bytes_per_token`、`memory_overhead_mb`）、`min_context`、`max_context` |
| | | **社区贡献入口** | `hawk-eye-mem --list-models` | 列出已支持的模型和参数来源；标注"官方预置"/"社区贡献" |
| | | | `hawk-eye-mem --contribute-model` | 输出贡献模板，引导用户提交PR |

**约束条件**：`--can-run` 仅做评估，不修改任何系统配置，不下载任何文件，不在磁盘上写任何内容（与 `--init-config` 明确区分）。

**verdict 判定逻辑**：

| 条件 | verdict |
|------|---------|
| 所有资源满足（gap_mb ≤ 0） | `feasible` |
| 有资源不满足，但有降级方案 | `feasible_with_caveats` |
| 磁盘空间不足以下载模型 | `infeasible` |
| 内存/显存差距超过50%且无降级方案 | `infeasible` |

---

### 应用端 2：AI Agent 运行时多维度监控

| 一级 | 二级 | 三级 | 四级 | 五级 |
|------|------|------|------|------|
| **Agent运行时监控** | **磁盘监控** | **模型缓存目录空间** | 自动检测 `~/.cache/huggingface/`、`~/.ollama/models/` 等常见目录 | 若目录不存在，该字段不输出；若存在但无权限，输出 `available_mb: null, error: "permission_denied"` |
| | | **磁盘压力判定** | 判定 `disk_pressure` | 三档：`ok`（可用>模型所需2倍）/ `warning`（可用<2倍）/ `critical`（可用<模型所需1.2倍） |
| | | **缓存膨胀监控** | 前后两次采集对比 | 若在 `--interval` 模式下检测到缓存增长，输出 `growth_rate_mb_per_hour`；仅当两次采集都有值时计算 |
| | **CPU监控** | **负载平均值** | 读取 1m/5m/15m 负载 | Linux: `/proc/loadavg`；macOS: `sysctl vm.loadavg`；输出为三个浮点数 |
| | | **Agent进程CPU** | 检测当前进程树CPU使用率 | 遍历进程树，累加CPU%；若无法获取，输出 `null` 不报错 |
| | | **CPU压力判定** | 判定 `cpu_pressure` | 三档：`low`（负载<核心数）/ `medium`（负载=核心数~2倍）/ `high`（负载>2倍核心数） |
| | **GPU监控（实验性）** | **NVIDIA显存** | NVML直接采集或 `nvidia-smi` 解析 | 需编译时启用 `--features gpu`；运行时若无 NVIDIA 驱动或 GPU，静默不输出 `gpu` 字段 |
| | | | **GPU止损条件** | W1 做 NVML 最简验证：编译通过+运行成功则继续，卡住超 1 天切 `nvidia-smi` 解析 |
| | | **GPU压力判定** | 判定 `gpu_pressure` | 三档：`low`（可用>50%）/ `medium`（可用20-50%）/ `high`（可用<20%） |
| | **内存监控增强** | **阈值v2校准** | 基于 V0.1 的 M1 5GB 实测数据调整判定边界 | 低内存机器（<8GB）的 `critical` 触发条件放宽：`available < 1.5GB` 或 `used > 95%` |
| | **多Agent检测** | — | **已延后至 V0.3** | 经 P M O 裁决，V0.2 集中精力做磁盘/CPU/`--can-run` |

**约束条件**：所有新 Collector 必须实现 `ResourceCollector` trait。采集失败时返回 `null` 或 `error` 字段，不阻塞其他维度采集。

---

### 应用端 3：人类开发者部署前快速检测

| 一级 | 二级 | 三级 | 四级 | 五级 |
|------|------|------|------|------|
| **开发者快速检测** | **终端友好输出** | **人类可读 `--can-run`** | `hawk-eye-mem --can-run --model llama3-8b` | 终端彩色输出：✅资源充足 / ⚠️有警告 / ❌无法部署；逐项显示差多少、怎么调 |
| | | **多模型对比** | `hawk-eye-mem --can-run --compare llama3-8b,qwen2-7b` | 表格输出两个模型的资源对比，标注哪个更适合当前环境；**最多 3 个模型** |
| | **配置引导** | **首次部署引导** | 当用户首次运行 `--can-run` 且未配置模型参数时 | 提示："检测到你还没配置模型参数，秋毫mem已用保守值估算。运行 `--init-config` 获取更精准的评估。" |

---

### 应用端 4：JSON 输出结构 V0.2

| 一级 | 二级 | 三级 | 四级 | 五级 |
|------|------|------|------|------|
| **JSON输出结构** | **system字段扩展** | **磁盘子字段** | `system.disk.model_cache_mb` | 模型缓存目录总容量（MB） |
| | | | `system.disk.available_mb` | 模型缓存目录剩余空间（MB） |
| | | | `system.disk.pressure` | `ok` / `warning` / `critical` |
| | | **CPU子字段** | `system.cpu.cores` | 逻辑核心数 |
| | | | `system.cpu.load_avg_1m/5m/15m` | 负载平均值 |
| | | | `system.cpu.pressure` | `low` / `medium` / `high` |
| | | **GPU子字段** | `system.gpu` | 数组，每个元素包含 `name`、`vram_total_mb`、`vram_used_mb`、`pressure`；无GPU或未编译时此字段不存在 |
| | **deployment_assessment 字段** | **请求回显** | `request` | 回显用户请求的模型参数 |
| | | **评估结论** | `verdict` | `feasible` / `feasible_with_caveats` / `infeasible` |
| | | **约束列表** | `constraints` | 数组，仅包含不满足或临界满足的资源项 |
| | | **降级方案** | `safe_options` | 数组，最多3个，按优先级排序 |
| | **agent_guidance 增强** | **多维度建议** | `agent_guidance.action` | 保持原有内存action；新增 `disk_action`、`cpu_action` 字段，各自独立判定 |
| | | | `agent_guidance._note` | 保留 V0.1 的免责声明字段 |

**约束条件**：`deployment_assessment` 字段仅在 `--can-run` 模式输出。运行时 `--json` 只输出 `system` + `agent_guidance`，不包含 `deployment_assessment`。

---

### 应用端 5：安装与配置

| 一级 | 二级 | 三级 | 四级 | 五级 |
|------|------|------|------|------|
| **安装与配置** | **编译选项** | **GPU特性** | `cargo build --features gpu` | 启用 NVIDIA GPU 采集；不加此 feature 不编译 GPU 模块 |
| | **配置文件扩展** | **模型参数段** | `[model]` 段 | 新增字段 `name`，用于从内置参数库匹配；若设置 `name`，则 `bytes_per_token` 和 `safety_margin_percent` 可省略 |
| | | **目录路径段** | `[directories]` 段 | 可自定义 `model_cache`（模型缓存目录路径）、`agent_process_names`（要监控的Agent进程名列表） |

**约束条件**：`--features gpu` 编译失败时给出明确提示，不应影响基础功能的编译和运行。配置文件向后兼容 V0.1。

---

### 应用端 6：新增 CLI 参数总览

| 参数 | 功能 | 示例 |
|------|------|------|
| `--can-run` | 部署前评估 | `hawk-eye-mem --can-run --model llama3-8b` |
| `--model` | 指定模型名称（从内置库查找） | `--model qwen2-7b` |
| `--model-size` | 手动指定模型参数量 | `--model-size 8000000000` |
| `--quantization` | 手动指定量化方法 | `--quantization Q4_K_M` |
| `--context` | 手动指定目标上下文长度 | `--context 8192` |
| `--compare` | 多模型对比评估（最多3个） | `--compare llama3-8b,qwen2-7b` |
| `--list-models` | 列出内置支持的模型 | `hawk-eye-mem --list-models` |
| `--contribute-model` | 输出模型贡献模板 | `hawk-eye-mem --contribute-model` |

---

### 版本对比总结

| 维度 | V0.1 | V0.2 |
|------|------|------|
| 监控资源 | 仅内存 | 内存 + 磁盘 + CPU + GPU(实验性) |
| 交互模式 | 被动查询 | 部署前评估 + 被动查询 |
| 核心命令 | `--json` `--metric` | 新增 `--can-run` 等 8 个参数 |
| Agent感知 | 运行时何时降级 | 部署前能否跑 + 运行时何时降级 |
| 用户覆盖 | 已在跑Agent的开发者 | 准备部署Agent的开发者 + 已在跑的 |

---

**产品经理总结**：V0.2 的核心交付是两个字——"看房"。Agent 住进去之前，秋毫mem 先把房子看一遍，告诉它能住多大、差多少、怎么调。磁盘、CPU、GPU 都是为了把"看房"这件事做完整。技术负责人接手技术方案设计的时候，优先保证 `--can-run` 和磁盘/CPU 两个 Collector 的交付，GPU 放实验性，别让它成为阻塞项。
