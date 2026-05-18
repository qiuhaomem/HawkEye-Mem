# 秋毫mem · Qiuhao Mem

> **内存洞察，秋毫不放。**  
> Memory Insight, Nothing Escapes.

**AI-Native 的内存监控 CLI 工具**，专为本地部署的大语言模型与 AI Agent 设计。

## 快速开始

```bash
# 安装（即将推出）
cargo install qiuhao-mem

# 使用
qiuhao                 # 人类可读输出
qiuhao --json          # Agent 可消费的 JSON 输出
qiuhao --interval 5    # 持续监控模式
```

## 核心能力

- ✅ 原生 JSON 输出，Agent 直接消费
- ✅ `safe_max_tokens` 估算 + `action` 语义指令
- ✅ MCP 协议原生支持，直接注册为 Agent Tool
- ✅ 零外部依赖，单个二进制文件
- ✅ 常驻 <2MB，单次采集 <1ms

## 项目结构

```
qiuhaomem/
├── README.md
└── docs/
    ├── 立项建议书_v1.0.md        # 项目立项完整文档
    └── 风险补遗与应对预案_v1.0.md # 风险分析与应对
```

## 状态

- [x] 第一阶段：调研与立项（已完成）
- [ ] 第二阶段：原型验证（进行中）
- [ ] 第三阶段：发布与社区
- [ ] 第四阶段：生态与商业化

## 开源协议

Apache-2.0

---

**GitHub**: github.com/qiuhaomem  
**官网**: qiuhao.dev
