# 秋毫mem v0.7.0 — 能力全景展示

**发布日期：** 2026-05-31

## 🌟 新增功能

### 🦅 能力全景展示 (`--onboarding`)
新用户一键展示秋毫mem所有亮点功能：

```
hawk-eye-mem --onboarding
```

聚合7大板块：系统体检 → 缓存策略 → Token花销总览 → 趋势分析 → 并发建议 → GPU/Agent/环境 → Agent决策指导

**特点：**
- 零 Token 消耗（所有数据本地采集）
- 带 emoji 状态图标的炫酷终端报告
- 支持 `--features budget` 编译以显示完整 Token 花销数据

### 🤖 MCP 新工具: `run_onboarding_showcase`
Agent 通过 MCP 调用一次性获取完整 JSON 全景数据，涵盖所有功能模块。

### 其他变动
- 新增 CLI 参数 `--onboarding`
- README 能力一览表和 MCP 工具列表同步更新 🆕
- 累计 **15 个 MCP 工具**

## 🧪 测试统计
- **93 项极限测试**全部通过（含正常/边界/压力/环境/组合6大维度）
- **135/135 断言 100%**
- 连续 50 次运行不崩，内存泄漏 0MB
- `--onboarding` 单次执行仅 ~33ms

## 📦 下载
各平台二进制见 Assets 区域。

## ⚠️ 已知限制
- Token 花销完整数据需编译 `--features budget`
- 测试框架基于外部调用，无法 mock 系统内部状态
