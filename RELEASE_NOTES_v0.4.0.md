# 秋毫mem v0.4.0 — 环境指纹·远程采集·物理AI

**发布日期：** 2026-05-22

## 🌟 新增功能

### 🧬 环境指纹引擎（Phase 1）
Agent 搬家也不怕！每次运行时自动生成环境指纹（内存/CPU/GPU/磁盘/容器），检测到重大变化时主动通知：
- 指纹包含：内存总量、CPU核心数、GPU列表、磁盘总量、容器运行时
- 变更检测阈值：>4GB 或 >20%（核心资源变化）
- 检测到变化时输出 `environment_change` 字段 + 新环境部署建议
- 保留最近 3 次历史指纹，可回溯变化轨迹
- `--env-fingerprint`：输出当前指纹 JSON
- `--reset-environment`：重置（`--force` 跳过确认）

### 🌐 远程采集 HTTP 服务（Phase 2）
多机资源监控——在一台机器上看全局：
- `--serve`：启动轻量 HTTP 服务，默认端口 9240
- `/metrics` 端点返回当前资源快照 JSON
- 可选 API Key 认证（配置文件中 `[remote] api_key`）
- 仅绑定 localhost 或内网地址，不暴露公网
- `--port` 自定义端口号

### 📈 历史趋势与资源预言机（Phase 3）
存储 7 天历史数据，预测何时到达临界：
- `--trend`：输出趋势分析报告（方向/变化速率/预计到达临界时间）
- 线性回归趋势分析，输出 `trend_direction`（increasing/stable/decreasing）
- 趋势置信度：样本越多越准，<10 点提示数据不足
- `--clear-history`：清空历史记录
- 历史数据存于 `~/.config/hawk-eye-mem/history.jsonl`，append-only

### 🐳 容器适配增强（Phase 4）
Docker/K8s 环境深度适配：
- 自动检测 Docker（`/.dockerenv`）或 K8s 环境
- 读取 cgroup 内存/CPU 限制，尊重容器资源上限
- 容器内 `total_mb` 以 cgroup 限制为准（而非物理内存）
- 输出标注 `runtime: "docker"` / `"kubernetes"` / `"unknown"`
- 无环境容器回退到物理内存 + `unknown` 标记

### 👥 多 Agent 协调增强（Phase 5）
从"能看到谁"升级到"建议怎么分"：
- V0.3 进程检测 → V0.4 每个 Agent 资源详情
- 配置文件 `[agents]` 段：可注册 Agent 名称、优先级、内存上限
- 协调建议包含：各 Agent 的 `memory_quota_mb`、`context_limit`、建议动作
- 不自动执行，仅输出建议供 Agent 框架消费

### 🔔 告警模式（CR-06）
可消费的告警流，配合外部监控系统：
- `--alert`：仅当压力 critical 时输出最小化 JSON 单行
- 输出字段：`pressure`、`available_mb`、`action`
- 适合管道 → 告警系统（PagerDuty/钉钉等）

### 🤖 物理 AI · 并发度建议（REQ-001）
给 Agent 当物理大脑——告诉你开多少个并发任务最安全：
- `--suggest-concurrency`：基于系统资源建议最佳并发数
- `--task-memory`：每个子任务的内存预算（默认 1024MB）
- 综合评估内存/CPU/磁盘/GPU，输出保守、平衡、激进三档建议

### 🛠 MCP Server 升级
- 7 个工具 → **12 个工具**：新增环境指纹 × 2、远程采集 × 2、趋势 × 2、告警、并发建议、容器检测 → 全链路覆盖
- 完整集成秋毫mem V0.4 所有新能力

## 📊 测试统计
- **322 个测试**（270 单元 + 52 集成），全部通过
- 三平台 CI：Linux ✅ / macOS ⏳（runner 排队中）/ musl ✅
- 覆盖环境指纹、远程采集、趋势分析、容器适配、并发建议

## 📦 下载
各平台二进制见 Assets 区域。

## ⚠️ 已知限制
- 远程采集 HTTP 服务暂不支持 TLS（内网场景，建议配合反向代理）
- 趋势分析需要至少 10 个采样点，首次使用需先积累数据
- Docker/K8s 适配依赖 cgroup v2（Ubuntu 21.10+ / RHEL 9+）
- 多 Agent 协调为建议模式，不自动执行资源操作
- macOS CI runner 偶尔排队较长，非代码问题
