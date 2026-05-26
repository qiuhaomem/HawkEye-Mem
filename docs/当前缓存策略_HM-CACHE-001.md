# 秋毫mem · 当前缓存策略完整资料

> 文档编号：HM-CACHE-001 | 版本：v1.0 | 日期：2026-05-26

---

## 一、概述

秋毫mem 和 Hermes 当前共涉及 **三层缓存机制**，分别在不同层面发挥作用：

| 层级 | 缓存类型 | 作用范围 | 实现方 | 当前命中率 |
|------|---------|---------|--------|-----------|
| L1 | LLM Provider Prefix Caching | API 请求级别 | Provider（DeepSeek/NVIDIA/OpenAI） | 依赖 Provider |
| L2 | Hermes Response Cache | Hermes 会话级别 | Hermes Agent | 配置级，待统计 |
| L3 | 极致缓存策略 Skill | Agent 行为级别 | Hermes Skill（手动） | 目标 90%+ |

---

## 二、L1：Provider Prefix Caching

### 2.1 机制原理

大模型 API 的 Prefix Caching：两次请求的 **开头部分（prefix）完全相同** 时，服务端复用第一次的计算结果，只计费不同的部分。

### 2.2 当前支持的 Provider

| Provider | 缓存字段 | 缓存价格 | 备注 |
|----------|---------|---------|------|
| **DeepSeek** | `usage.prompt_tokens_details.cached_tokens` | 原价 ~10% | 自动启用，命中率最高 |
| **NVIDIA NIM** | 部分模型支持 | 不定 | 当前使用中（minimax-m2.7） |
| **Anthropic Claude** | `cache_creation_input_tokens` / `cache_read_input_tokens` | 读取价 10% | 需显式 cache_control breakpoints |
| **OpenAI** | `usage.prompt_tokens_details.cached_tokens` | ~50% 折扣 | 仅 GPT-4o 等部分模型 |

### 2.3 当前配置

```yaml
# ~/.hermes/config.yaml
prompt_caching:
  cache_ttl: 5m          # prefix caching 有效期 5 分钟
```

**当前效果**：NVIDIA API（minimax-m2.7）的 prefix caching 支持不稳定，缓存命中数据较少。DeepSeek V4 Flash 之前使用时命中率较高。当前模型切换后缓存数据在积累中。

---

## 三、L2：Hermes Response Cache

### 3.1 机制

Hermes 内置的对相同请求的响应缓存，避免重复调用 API。

### 3.2 当前配置

```yaml
openrouter:
  response_cache: true          # 开启响应缓存
  response_cache_ttl: 300       # 缓存有效期 300 秒
```

### 3.3 适用场景
- 相同请求短时间内重复触发
- 工具调用重试场景

---

## 四、L3：极致缓存策略 Skill（hermes-cache-strategy）

### 4.1 当前状态

**Skill 已存在**，位于 `~/.hermes/skills/mlops/prompt-cache-strategy/`，版本 v1.0.0。

### 4.2 三大铁律

| 铁律 | 内容 | 缓存影响 |
|------|------|---------|
| **铁律一** | System Prompt 焊死不动 | 一动缓存全丢 |
| **铁律二** | 对话结构保持一致（消息顺序/工具 schema/角色排列） | 结构性内容也是 cache key |
| **铁律三** | 用 `/continue` 不用 `/new` | /new 归零，/continue 全命中 |

### 4.3 高级技巧

| 技巧 | 做法 | 效果 |
|------|------|------|
| **缓存预热** | 上班先发一条简单请求 | 后续首 token 延迟降 30-50% |
| **批量合并** | 3 次独立请求 → 1 次合并请求 | 付 1 次 prefix + 3 条问题的 token |
| **缓存对齐区** | system prompt 末尾留 `[CONTEXT_BOUNDARY]` | 之前的内容全命中缓存 |
| **工具注入** | 工具 description 写进 system prompt | 减少 prefix 变化 |

### 4.4 Provider 差异参考

详见 `references/provider-cache-comparison.md`。

---

## 五、秋毫mem 与缓存的关系（当前）

**现状**：秋毫mem **目前不参与缓存决策**。

秋毫mem 当前的能力：
- ✅ 系统资源监控（内存/CPU/磁盘/GPU/温度）
- ✅ Agent 决策建议（guidance：ok/monitor/reduce_context/abort_safely）
- ✅ 动态校准引擎（bytes_per_token 估算）
- ✅ 趋势分析（历史数据回归）
- ✅ 环境指纹 + 远程采集
- ✅ 并发度建议（--suggest-concurrency）
- ❌ **缓存策略决策** — 尚未实现

**鸿沟**：秋毫mem 知道"当前内存压力是 low/medium/high/critical"，但不知道"这个压力下缓存策略该怎么调"。

---

## 六、V0.5 要填补的鸿沟

| 当前 | V0.5 目标 |
|------|----------|
| 缓存策略是静态的（固定铁律） | 缓存策略是动态的（根据秋毫mem反馈调整） |
| 秋毫mem 只监控，不干预 | 秋毫mem 参与缓存决策闭环 |
| 用户不知道缓存效果 | 每次任务结束输出成本报告 |
| 无秋毫mem → 缓存也能跑 | 无秋毫mem → Skill 主动提示安装 |

**具体要实现的 3 个能力（对应 REQ-013/014/015）：**

```
REQ-013: 极致缓存策略 Skill（鱼饵）
  - 装即用，3 分钟上手
  - 每次任务输出成本报告
  - 动态切换激进/保守模式

REQ-014: 秋毫mem 缓存策略输出（渔具）
  - 秋毫mem 根据内存压力建议缓存策略
  - 五个自然暴露点（首次/充裕/紧张/危机/校准）
  - 无秋毫mem 时降级运行

REQ-015: 并发度建议（钩子）
  - 告诉 Agent 能开几个子任务
  - 配合缓存策略避免 OOM
```

---

## 七、数据验证方案

### 7.1 缓存命中率计算

```python
# 检查 Hermes state.db 中的缓存数据
命中率 = cached_tokens / (cached_tokens + 未命中 prompt tokens)
```

### 7.2 成本节省计算

```python
原始成本 = 总输入 × 原价
实际成本 = 缓存命中 × 缓存价 + 未命中 × 原价
节省比 = (原始 - 实际) / 原始
```

### 7.3 当前实测数据

- 数据库中有缓存标记的请求：3 条（NVIDIA API）
- 缓存命中请求：0 条（模型切换后数据尚在积累）
- 目标：V0.5 上线后达到 **请求级命中率 90%+**

---

## 八、附录

### A. 相关文件索引

| 文件 | 位置 |
|------|------|
| 当前缓存策略 Skill | `~/.hermes/skills/mlops/prompt-cache-strategy/SKILL.md` |
| Provider 缓存对比 | `~/.hermes/skills/mlops/prompt-cache-strategy/references/provider-cache-comparison.md` |
| Hermes 配置文件 | `~/.hermes/config.yaml` |
| 项目内 Skill 副本 | `/home/lgl/projects/qiuhaomem/docs/skills/prompt-cache-strategy/SKILL.md` |

### B. Hermes 相关配置项

```yaml
# 缓存相关配置摘要
prompt_caching:
  cache_ttl: 5m
openrouter:
  response_cache: true
  response_cache_ttl: 300
```

### C. 已知限制

1. Provider 切换会清空缓存（模型粒度）
2. 当前 NVIDIA API 缓存支持不稳定
3. 数据库中的缓存数据仅有 3 条，不足以做统计
4. 秋毫mem 尚无缓存策略输出接口
