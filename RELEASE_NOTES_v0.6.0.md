# 秋毫mem v0.6.0 — 精准打击·缓存差距·心跳·Token审计

**发布日期：** 2026-05-30

## 🌟 新增功能

### 🎯 缓存差距分析
让 Agent 知道缓存钱花在哪了！基于命中率数据自动诊断缺口：
- `--analyze-cache-gaps`：输出缓存命中率 vs 目标，差距百分比，日均miss token量
- 缺口分类：`new_session_cold_start` / `model_switch` / `other`，每种带占比
- 修复建议：具体配置调整命令 + 预期收益
- `--days`：自定义分析天数（默认 7）
- `--target`：自定义目标命中率（默认 99.0%）
- `--json`：JSON 结构输出，方便脚本消费

### 💓 单行心跳
为告警/监控系统提供极简接口：
- `--heartbeat`：输出单行 JSON `{"pressure","available_mb","action","timestamp","used_percent"}`
- 适合管道推送到外部监控系统

### ⚙️ 缓存配置段
可自定义的缓存阈值配置：
- `[cache]` 配置段：`target_hit_rate`、`warn_threshold`、`analysis_days`
- 配置值与 CLI 参数自动联动（CLI 参数优先）

### 🛠 MCP Server 升级
12 个工具 → **14 个工具**：
- `get_cache_gaps_analysis`：缓存差距分析（支持 days/target 参数）
- `get_heartbeat`：单行心跳（支持无参数调用）

## 🔧 修复与优化
- **15 个 cargo warnings 清零**：dead_code/unused_imports/unused_variables 全部清理
- **移除误导 flag**：`--source` 标记已从 Rust CLI 移除（Token审计通过 Python MCP 工具调用，详见 MCP `run_token_audit`）
- **配置文件热加载**：每次 CLI/MCP 调用重新读取 config.toml

## 📊 测试统计
- **332 个测试**（275 单元 + 22 CLI 集成 + 28 压力 + 7 V0.6 专项），全部通过
- 新增 7 个 V0.6 专项测试覆盖：heartbeat JSON 格式验证、analyze-cache-gaps 自定义参数

## 📦 下载
各平台二进制见 Assets 区域。

## ⚠️ 已知限制
- Token 审计功能通过 Python 脚本提供，非 Rust 原生（通过 `scripts/token_audit/` + MCP 调用）
- 缓存差距分析依赖历史数据积累，首次使用需先运行一段时间
- 配置热加载为按需读取模式（每次调用重新加载），非文件监听模式
