# 秋毫mem V0.6 · 「精准打击」Kanban 任务看板

**文档编号**：HM-KANBAN-V0.6
**创建日期**：2026-05-29
**状态**：🔄 进行中

---

## 📋 任务看板

### 🔴 M1：缓存差距分析（P0，核心）

| ID | 任务 | 描述 | 估时 | 状态 |
|----|------|------|------|:----:|
| T-001 | `--analyze-cache-gaps` CLI | 读取 cache_stats.jsonl，分析 miss 分布，输出缺口分类+修复建议 | 3h | 🔄 |
| T-002 | 缺口分类算法 | 按 miss 原因分类：新会话冷启动/tool输出波动/上下文压缩/其他 | 2h | ⏳ |
| T-003 | 修复建议生成 | 基于缺口分布生成具体配置调整命令 | 1h | ⏳ |
| T-004 | `analyze_cache_gaps` MCP 工具 | MCP Server 注册新工具 | 1h | ⏳ |

### 🟡 M2：Token 审计增强（P0）

| ID | 任务 | 描述 | 估时 | 状态 |
|----|------|------|------|:----:|
| T-005 | `--days N` 参数 | 按天数过滤审计数据 | 1h | 🔄 |
| T-006 | `--source <src>` 参数 | 按来源过滤（weixin/cron/api_server） | 1h | ⏳ |
| T-007 | 浪费 TOP5 输出 | 输出最浪费 token 的会话/来源排行 | 1h | ⏳ |

### 🟢 M3：自监控与配置（P1）

| ID | 任务 | 描述 | 估时 | 状态 |
|----|------|------|------|:----:|
| T-008 | `--heartbeat` 命令 | 单行 JSON 输出（pressure/available_mb/action/timestamp） | 1h | ⏳ |
| T-009 | `[cache]` 配置段 | 自定义阈值：命中率目标、告警阈值 | 1h | ⏳ |
| T-010 | 配置文件热加载 | 读取 ~/.config/hawk-eye-mem/config.toml 的 [cache] 段 | 0.5h | ⏳ |

### 🔵 M4：性能优化+测试（P2）

| ID | 任务 | 描述 | 估时 | 状态 |
|----|------|------|------|:----:|
| T-011 | CLI 启动性能优化 | 目标 <50ms 冷启动，lazy init 非必要模块 | 1h | ⏳ |
| T-012 | 集成测试补充 | 缓存差距分析+Token审计增强+heartbeat 测试 | 2h | ⏳ |
| T-013 | README V0.6 更新 | 中英文 README 更新新功能 | 1h | ⏳ |
| T-014 | Release Notes V0.6 | 发布说明 | 0.5h | ⏳ |

---

## 📊 进度统计

| 里程碑 | 任务数 | 已完成 | 进度 |
|--------|:------:|:------:|:----:|
| M1 缓存差距分析 | 4 | 0 | 0% |
| M2 Token审计增强 | 3 | 0 | 0% |
| M3 自监控+配置 | 3 | 0 | 0% |
| M4 性能+测试 | 4 | 0 | 0% |
| **总计** | **14** | **0** | **0%** |

---

## 🎯 验收标准

1. `hawk-eye-mem --analyze-cache-gaps` 输出缺口分类+修复建议
2. `hawk-eye-mem --token-audit --days 7 --source weixin` 按条件过滤
3. `hawk-eye-mem --heartbeat` 输出单行 JSON
4. MCP Server 15 个工具全部可用
5. ≥ 35 tests 全绿
6. README 中英文更新
