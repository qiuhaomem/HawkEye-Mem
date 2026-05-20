---
name: prompt-cache-strategy
description: "Use when you want to maximize API cost efficiency through prefix caching —适用于 DeepSeek、智谱GLM、Anthropic Claude 等支持 prefix caching 的模型。包含三大铁律、缓存预热、批量合并等实战策略。"
version: 1.0.0
author: 秋毫mem Team
license: MIT
metadata:
  hermes:
    tags: [cache, cost-saving, prompt-engineering, optimization, llm]
    related_skills: [hawk-eye-mem]
---

# 极致缓存命中策略

> 让 LLM API 的每一分钱都花在刀刃上。

## Overview

大模型 API 的 Prefix Caching（前缀缓存）机制：两次请求的 **开头部分（prefix）完全相同** 时，服务端复用第一次的计算结果，**只计费不同的部分**。

稳定使用后：**缓存命中率 90%+，综合成本降低 50-70%**。

支持此策略的模型/平台：
- **DeepSeek** — `usage.prompt_tokens_details.cached_tokens`
- **智谱 GLM** — 同上字段
- **Anthropic Claude** — `cache_creation_input_tokens` / `cache_read_input_tokens`
- **OpenAI** — 部分模型支持

## When to Use

- 每天高频率调用 LLM API（100+ 次/天）
- 使用固定的 system prompt / 工具集
- 希望降低 API 账单
- Agent 在长时间会话中反复调用

## 三大铁律

### 铁律一：System Prompt 焊死不动

System prompt 是整个请求的前缀骨架。它一动，缓存全丢。

| 做法 | 缓存影响 |
|------|---------|
| 固定一个 system prompt，不改一字 | ✅ 缓存拉满 |
| 每次微调几个字 | ❌ 缓存全丢 |
| 不同任务切不同 prompt | ❌ 各自独立，互不共享 |

### 铁律二：对话结构保持一致

消息顺序、工具 schema、角色排列——这些结构性内容也是 cache key 的一部分。

| 层面 | 稳定做法 |
|------|---------|
| 消息顺序 | 始终 system → user → assistant → user |
| 工具定义 | 写死后不增减顺序，新增加在末尾 |
| 输出格式 | 固定 schema / response_format |

### 铁律三：不用 `/new`，用 `/continue`

| 命令 | 缓存影响 |
|------|---------|
| `/new` | ❌ 缓存归零 |
| `/continue` | ✅ 历史 prefix 全命中 |

## 高级技巧

### 缓存预热

上班先发一条简单请求建立缓存，后续直接命中：

```bash
# Hermes 中发一条预热
hermes chat -q "ping"
```

预热后首 token 延迟降低 30-50%。

### 批量请求合并

```diff
- 3次独立请求：付 3 次 system prompt 的 prefix 费
+ 1次合并请求：付 1 次 prefix + 3 条问题的 token 费
```

### 缓存对齐区

在 system prompt 末尾留固定占位：

```
[CONTEXT_BOUNDARY]
Current task: [FILL]
```

`[CONTEXT_BOUNDARY]` 之前的内容全命中缓存。

### 工具注入

把工具的 `description` 写进 system prompt 而非每次传工具列表，减少 prefix 变化。

## 验证与监控

### 检查缓存命中

```bash
# DeepSeek / 智谱
usage.prompt_tokens_details.cached_tokens

# Anthropic Claude
usage.cache_read_input_tokens
```

在 Hermes 对话中输入 `/usage` 查看。

### 命中率公式

```
命中率 = cached_tokens / (cached_tokens + 未命中 prompt tokens)
```

**目标值：> 85%**

## 成本估算

```
原始成本 = 总输入 × 原价
实际成本 = 缓存命中 × 缓存价 + 未命中 × 原价
节省比 = (原始 - 实际) / 原始
```

缓存单价通常为原价的 **10%-30%**。

## Common Pitfalls

1. **往 system prompt 里加动态内容**（时间戳、随机数、session id）→ 缓存变废纸
2. **频繁 `/new` 开新会话** → 每次重建 prefix
3. **工具列表顺序变化** → 即使工具一样，顺序不同缓存也不命中
4. **切换模型/provider** → 缓存是模型粒度的，换了就丢
5. **以为缓存=免费** → 缓存通常按折扣价计费，不是完全免费

## Verification Checklist

- [ ] System prompt 已固定，不含动态内容
- [ ] 工具列表顺序稳定，新增加在末尾
- [ ] 会话使用 /continue 而非 /new
- [ ] /usage 查看 cached_tokens 占比 > 85%
- [ ] 预热请求已发送
- [ ] 批量请求合并已应用
