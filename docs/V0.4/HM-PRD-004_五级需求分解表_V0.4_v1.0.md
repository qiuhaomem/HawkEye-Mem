# 秋毫mem V0.4 · 五级需求分解表

**文档编号**：HM-PRD-004
**关联**：需求调研报告
**日期**：2026-05-21

---

## 应用端 1：环境指纹与迁移感知 —— Agent 搬家也不怕

| 一级 | 二级 | 三级 | 四级 | 五级 |
|------|------|------|------|------|
| **环境指纹** | **指纹生成** | **首次运行自动生成** | 采集：`hostname`、`platform`、`cpu_cores`、`total_memory_mb`、`gpu_names`、`disk_total_mb` | 指纹存储于 `~/.config/hawk-eye-mem/environment.json`；格式稳定，可序列化 |
| | | **指纹比对** | 每次启动或定期比对当前环境与存储指纹 | 差异分为：`upgrade`（资源增加）、`degrade`（资源减少）、`unchanged` |
| | **变更检测** | **触发条件** | 核心资源变化 > 20% 或关键设备增减 | 例如：内存从 16GB → 64GB 触发；GPU 从无到有触发；主机名变化不触发（仅 IP 无关） |
| | | **通知机制** | 检测到变更时，在 JSON 输出中增加 `environment_change` 字段 | 包含 `previous`、`current`、`direction`、`new_recommendation` |
| | **迁移建议** | **自动重新评估** | 环境变化后自动运行 `--can-run` 逻辑 | 基于新环境重新计算 `deployment_assessment` |
| | | **新建议输出** | 若原模型在新环境下可行，给出 `new_recommendation` | 例如："内存已升级至 64GB，您现在可以安全地使用 32K 上下文。" |
| | | **历史指纹存储** | 保留最近 3 次环境指纹 | 旧指纹自动覆盖，便于回溯变化历史 |
| | **手动指纹管理** | **`--env-fingerprint`** | 输出当前环境指纹 JSON | 用于备份、对比或远程采集时发送 |
| | | **`--reset-environment`** | 重置环境指纹 | 清空存储，下次启动重新生成 |

## 应用端 2：远程采集 —— 多机资源监控

| 一级 | 二级 | 三级 | 四级 | 五级 |
|------|------|------|------|------|
| **远程采集** | **服务端模式** | **`--serve` 命令** | 启动轻量 HTTP 服务，监听指定端口 | 默认端口 9240；仅提供 `/metrics` 端点，返回当前资源快照 JSON |
| | | **认证** | 可选 API Key 认证 | 通过配置文件 `[remote] api_key` 设置；无配置时无认证 |
| | | **安全** | 仅绑定 localhost 或内网地址 | 不支持公网暴露；提醒用户配置防火墙 |
| | **客户端模式** | **`--remote <url>`** | 从远程秋毫mem 实例拉取指标 | 返回 JSON 结构与本机一致；超时默认 5 秒 |
| | | **多机聚合** | `--remote` 可多次指定，或配置文件中预设多台机器 | 在 `agent_guidance` 基础上增加 `remote_nodes` 数组，每台机器一条 |
| | | **远程认证** | 支持 API Key | 通过 `--remote-key` 参数或 `HAWKEYE_REMOTE_KEY` 环境变量提供 |
| | **聚合视图** | **多机总览** | `hawk-eye-mem --remote host1,host2` | 输出每台机器的 `system` 摘要，以及全局最紧张的资源 |
| | | **全局建议** | 多机中最紧张的机器决定全局 `action` | 例如：三台机器中一台 critical，全局 action = `abort_safely` |
| | **商业化试点** | **`--remote` 功能标记** | 企业版功能，Apache-2.0 核心仍免费 | 若社区反弹强烈，改为完全开源+捐赠模式；在文档中注明试点阶段 |

## 应用端 3：历史趋势与资源预言机

| 一级 | 二级 | 三级 | 四级 | 五级 |
|------|------|------|------|------|
| **历史趋势** | **数据存储** | **本地时序数据库** | 使用 append-only 文件存储历史采样 | 文件路径：`~/.config/hawk-eye-mem/history.jsonl`；每条 JSON 一行；与校准 CSV 分离 |
| | | **采样策略** | `--interval` 模式下自动记录 | 每条包含：`timestamp`、`memory_available_mb`、`memory_pressure`、`cpu_load`、`disk_available_mb` 等核心字段 |
| | | **数据保留** | 默认保留 7 天 | 自动清理 7 天前的记录；可在配置中调整 `history_retention_days` |
| | | **数据压缩** | 对旧数据采样降精度 | 超过 24 小时的数据只保留 5 分钟平均值 |
| | **趋势分析** | **`--trend` 命令** | 分析历史趋势 | 输出：`memory_available_trend`（`increasing`/`stable`/`decreasing`）、变化速率、预计到达临界的时间 |
| | | **趋势输出** | JSON 增加 `trends` 字段 | 包含 `memory_available_7d_avg_mb`、`trend_direction`、`estimated_days_until_critical` |
| | **资源预言机** | **基于趋势的预测** | 线性回归或简单滑动平均 | 预测："按当前趋势，可用内存将在 14 天后降至临界值以下" |
| | | **预测触发告警** | 预测值低于临界阈值时，提前预警 | 不等待实际到达临界，而是提前在 `agent_guidance` 中增加 `prediction_warning` |
| | | **预言置信度** | 样本数越多置信度越高 | 少于 100 个采样点时标注 `confidence: "low"` |

## 应用端 4：容器与云环境适配

| 一级 | 二级 | 三级 | 四级 | 五级 |
|------|------|------|------|------|
| **容器适配** | **cgroup 感知** | **读取 cgroup 内存限制** | Linux 上读取 `/sys/fs/cgroup/memory/memory.limit_in_bytes` | 若值小于物理内存，以此作为 `total_mb` |
| | | **CPU 限制感知** | 读取 `cpu.cfs_quota_us` / `cpu.cfs_period_us` | 计算有效 CPU 核心数 |
| | | **Docker 环境检测** | 检测 `/.dockerenv` 或 cgroup 中包含 docker 关键字 | 输出中标注 `runtime: "docker"` |
| | **K8s 适配** | **Pod 资源限制** | 读取 `/sys/fs/cgroup/memory/memory.limit_in_bytes`（K8s 设置） | 尊重 `resources.limits` |
| | | **Pod 元数据** | 从环境变量或 downward API 读取 | 可选输出 `pod_name`、`namespace`（若存在） |
| | **无环境容器** | **无法检测时回退** | 无 cgroup 信息时使用物理内存 | 在输出中标注 `runtime: "unknown"` |
| | | **容器最小内存** | 512MB 限制下仍可用 | 压力判定使用低内存模式（V0.3 已实现） |

## 应用端 5：多 Agent 协调 —— 从感知到行动

| 一级 | 二级 | 三级 | 四级 | 五级 |
|------|------|------|------|------|
| **多 Agent 协调** | **全局资源视图** | **同机 Agent 列表** | V0.3 已实现进程检测 | 在 V0.4 中增加每个 Agent 的 `memory_quota_mb`、`context_limit`、`priority` |
| | **协调策略** | **简单分配算法** | 按优先级分配资源 | 优先级：用户在配置文件中为每个 Agent 指定 `priority` 值（1-10）；默认 5 |
| | | **降级策略生成** | 总资源紧张时输出协调建议 | 建议格式：`agent_a: reduce_context to 4096`，`agent_b: pause`，`agent_c: continue` |
| | | **不自动执行** | 建议仅供参考 | 不实际 kill 或 renice 进程；Agent 框架需自行消费建议 |
| | **JSON 输出** | **`multi_agent` 字段增强** | 增加 `global_strategy`、`resource_allocation` | 每个 Agent 获得分配的资源配额和建议 |
| | **配置文件** | **Agent 注册** | `[agents]` 段 | 可为每个 Agent 配置 `name`、`priority`、`max_memory_mb`、`max_context` |

## 应用端 6：新增 CLI 与配置文件

| 一级 | 二级 | 三级 | 四级 | 五级 |
|------|------|------|------|------|
| **CLI 新增** | **`--env-fingerprint`** | 输出当前环境指纹 | 单次执行 | JSON 或人类可读 |
| | **`--reset-environment`** | 重置环境指纹 | 需确认 | 交互式确认或 `--force` |
| | **`--serve`** | 启动 HTTP 服务 | 长期运行 | 需 `--port` 参数 |
| | **`--remote <url>`** | 远程采集 | 单次或间隔 | 支持逗号分隔多个 URL |
| | **`--remote-key <key>`** | 远程认证密钥 | 配合 `--remote` | |
| | **`--trend`** | 历史趋势分析 | 单次 | 输出趋势报告 |
| | **`--alert`** | 告警模式 | 持续 | 仅当 critical 时输出一行，配合管道 |
| | **`--model` 增强** | 迁移后自动建议 | 结合环境指纹 | |

### 配置文件扩展

```toml
[remote]
api_key = "your-secret-key"
nodes = ["http://192.168.1.10:9240", "http://192.168.1.11:9240"]

[history]
retention_days = 7

[agents]
[[agents]]
name = "hermes-main"
priority = 10
max_memory_mb = 8192

[[agents]]
name = "claude-code"
priority = 5
max_memory_mb = 4096
```

## V0.4 与 V0.3 对比

| 维度 | V0.3 | V0.4 |
|------|------|------|
| 环境感知 | 单机，越用越准 | 多机，搬家自己知道 |
| 远程监控 | 无 | HTTP 远程采集 + 聚合视图 |
| 历史数据 | 仅校准 CSV | 时序历史 + 趋势预测 |
| 多 Agent | 检测到多少 Agent | 协调资源分配 |
| 容器 | 不支持 | 完全适配 Docker/K8s |
| 商业化 | 无 | 远程采集企业版试点 |

## 优先级（MoSCoW）

| 优先级 | 需求 |
|--------|------|
| **Must Have** | 环境指纹与迁移检测、远程采集（基本 HTTP 模式）、Docker/K8s 容器适配 |
| **Should Have** | 历史趋势与趋势分析、多 Agent 协调策略、`--serve` / `--remote` / `--trend` CLI |
| **Could Have** | 告警模式 `--alert`、资源预言机高级预测 |
| **Won't Have** | Web 仪表盘（V1.0）、公有云服务（V1.0+） |

---

**产品经理总结**：V0.4 让秋毫mem 从"单机感知"升级为"集群感知"。Agent 搬家了，秋毫mem 自己知道，并告诉它新家能撑多大的事。远程采集让运维人员在一台机器上看全局，容器适配覆盖最常见的部署场景。历史趋势和多 Agent 协调进一步巩固护城河——这些能力组合起来，竞品要追赶至少需要 6 个月。商业化试点从远程采集开始，轻量且不影响核心用户体验。
