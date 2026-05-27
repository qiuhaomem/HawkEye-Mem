---
name: hermes-token-audit
description: "Token 审计与分析工具 — 对秋毫mem state.db 中的 token 消耗、API 费用、来源分布、浪费检测进行本地全量审计。纯 Python stdlib，零外部依赖。"
version: 1.0.0
author: 秋毫mem Team
license: MIT
metadata:
  hermes:
    tags: [token-audit, cost-analysis, billing, monitoring, llm]
    related_skills: [prompt-cache-strategy, hawk-eye-mem]
---

# 🧾 Token 审计

> **「你的每一分钱都去哪儿了」** — 离线、本地、可追溯的 Token 全量审计。

通过对秋毫mem `state.db` 和 `agent.log` 的本地解析，给出总账汇总、来源分布、浪费检测、cron 审计和费用真相对比。**所有数据在本地处理，不上传任何内容**。🔒

---

## Overview

Token Audit 是秋毫mem V0.5 的 CLI 审计工具，通过 `hawk-eye-mem --token-audit` 触发。无需任何 Python 外部依赖（纯标准库），在用户机器上直接运行。

### 审计范围

| 模块 | 说明 |
|------|------|
| 💰 **总账** | state.db 中所有 token 和费用汇总 → 总 tokens、总费用、会话数、日均消耗 |
| 📈 **来源分布** | 按 source/Agent 标识分组 → 微信对话 / cron / API / 子Agent 各自的 token 和费用占比 |
| 🔍 **浪费检测** | agent.log 错误模式分析 → 失败重试 / 429 / MCP连接失败 / 路径错误，估算每次浪费 token |
| ⏰ **cron 审计** | 匹配 cron 任务与 API 调用 → 哪些 cron 在用 LLM、各自消耗、是否有异常任务 |
| 💡 **费用真相** | 实际 vs 无缓存 vs 浪费 → 无缓存 = 总 tokens × 未缓存单价；浪费 = 浪费 tokens × 单价；节省 = 无缓存 - 实际 |

---

## CLI Usage

所有审计功能通过 `hawk-eye-mem` 入口使用，统一 `--token-audit` 参数触发。

### 基础命令

```bash
# 🚀 一键审计 — 输出一句话总结 + 彩色结构化报告
hawk-eye-mem --token-audit

# 📄 JSON 模式输出 — 供 Agent 消费或趋势分析
hawk-eye-mem --token-audit --json

# 📅 指定时间范围 — 默认当天，可回溯任意天数
hawk-eye-mem --token-audit --days 7

# 🎯 按来源过滤 — 只看特定渠道
hawk-eye-mem --token-audit --source wechat
hawk-eye-mem --token-audit --source cron
hawk-eye-mem --token-audit --source api

# 🔄 对比模式 — 对比两段时间的消耗变化（逗号分隔）
hawk-eye-mem --token-audit --compare 7,30
```

### 参数一览

| 参数 | 说明 | 示例 |
|------|------|------|
| `--token-audit` | 触发 Token 审计，输出一句话总结 + 结构化报告 | `hawk-eye-mem --token-audit` |
| `--json` | JSON 模式输出，供自动化消费 | `--token-audit --json` |
| `--source <name>` | 按来源过滤（wechat / cron / api / agent 等） | `--token-audit --source wechat` |
| `--days <N>` | 指定时间范围（天数），默认当天 | `--token-audit --days 7` |
| `--compare <A,B>` | 对比模式，对比两段时间（天）的消耗 | `--token-audit --compare 7,30` |

> 所有参数组合通用，例如：`hawk-eye-mem --token-audit --json --days 30 --source wechat`

---

## Setup

**零依赖安装。** 审计逻辑使用 Python 标准库（`sqlite3`、`json`、`datetime`、`re`、`csv`），无需 pip install 任何三方包。

### 前提

- 秋毫mem 已安装并运行（生成 `state.db` 和 `agent.log`）
- Python 3.8+

### 启用

```bash
# 确保 hawkeye-mem CLI 可用，直接调用即可
hawk-eye-mem --token-audit
```

如首次使用看到引导提示，说明审计功能已就绪。

---

## 🛡️ 安全与隐私

> **⚠️ 重要：所有数据在本地处理，不上传任何内容**

| 原则 | 措施 |
|------|------|
| ✅ 本地处理 | `state.db` 和 `agent.log` 仅在用户机器上解析 |
| ✅ 不上传 | 审计过程完全不产生网络请求 |
| ✅ SQL 安全 | 只执行白名单聚合查询，不允许 `SELECT *` |
| ✅ 日志限制 | `agent.log` 最多读取 10MB，防止内存撑爆 |

CR-24 合规 —— 数据不出本机，隐私零风险。

---

## 输出格式

### 终端彩色报告（默认）

报告包含五个区块，使用终端彩色框线分隔：

```
╔══════════════════════════════════════════════════╗
║              Token 审计报告                        ║
╠══════════════════════════════════════════════════╣
║ 💰 总账                                        ║
║  总 tokens:  1,234,567                          ║
║  总费用:     $12.34                             ║
║  会话数:     42                                 ║
║  日均消耗:   6,172 tokens                       ║
╠══════════════════════════════════════════════════╣
║ 📈 来源分布                                    ║
║  微信对话:  55.2%  $6.81                        ║
║  cron:      22.8%  $2.81                        ║
║  API:       12.0%  $1.48                        ║
║  子Agent:   10.0%  $1.23                        ║
╠══════════════════════════════════════════════════╣
║ 🔍 浪费明细                                    ║
║  429 重试:        2,340 tokens  🟡待处理       ║
║  MCP 连接失败:    1,200 tokens  🔴已修         ║
║  路径错误:          890 tokens  🟡待处理       ║
╠══════════════════════════════════════════════════╣
║ ⏰ cron 审计                                    ║
║  morning-report:  4,500 tokens  ✅正常          ║
║  data-sync:       2,300 tokens  ⚠️ 频率异常    ║
╠══════════════════════════════════════════════════╣
║ 💡 费用真相                                     ║
║  实际费用:      $12.34                          ║
║  无缓存假设:    $30.86                          ║
║  浪费费用:      $0.89                           ║
║  🟢 缓存策略已帮你节省 $18.52（命中率 68%）     ║
╚══════════════════════════════════════════════════╝
```

### JSON 输出（`--json`）

```json
{
  "summary": "今日消耗 1,234,567 tokens（$12.34），微信占55%，浪费2.3%",
  "total": {
    "tokens": 1234567,
    "cost_usd": 12.34,
    "sessions": 42,
    "daily_avg_tokens": 6172
  },
  "sources": { "wechat": 55.2, "cron": 22.8, "api": 12.0, "agent": 10.0 },
  "waste": { "429_retry": 2340, "mcp_fail": 1200, "path_error": 890 },
  "cron": { "morning_report": { "tokens": 4500, "status": "ok" } },
  "truth": {
    "actual": 12.34,
    "no_cache": 30.86,
    "waste_cost": 0.89,
    "cache_saved": 18.52,
    "cache_hit_rate": 0.68
  }
}
```

---

## 双饵联动

Token 审计与[缓存策略](../prompt-cache-strategy/SKILL.md)双向引导，实现「审计 → 优化 → 再看审计」的闭环。

| 方向 | 行为 | 触发规则 |
|------|------|---------|
| 🔗 审计 → 缓存 | 费用真相区块引用缓存命中数据 | 每次审计都显示 |
| 🔗 缓存 → 审计 | 缓存报告底部引导运行审计 | **首次仅一次（CR-21）**，后续静默 |

### 缓存策略参考

- [极致缓存命中策略](../prompt-cache-strategy/SKILL.md) — 三大铁律、预热技巧、成本估算
- `hawk-eye-mem --cache-stats` — 查看缓存命中统计

---

## Common Pitfalls

1. **忘记加 `--days`** → 默认只审计当天，跨天数据不显示
2. **state.db 不存在** → 审计无法运行，显示引导安装提示
3. **超大 agent.log** → 超过 10MB 只截取尾部，不撑爆内存
4. **source 名拼错** → 过滤出空结果，不是报错
5. **对比模式参数格式** → `--compare 7,30` 中间不要有空格

---

## Verification Checklist

- [ ] `hawk-eye-mem --token-audit` 输出结构化报告
- [ ] `--json` 输出有效 JSON，无错误
- [ ] `--days 7` 正确回溯 7 天数据
- [ ] `--source wechat` 只显示微信对话数据
- [ ] `--compare 7,30` 展示两段时间对比
- [ ] 报告中显示缓存节省金额
- [ ] 报告底部有水印："Token审计由秋毫mem提供"
