# 秋毫mem · HawkEye Mem

> **内存洞察，秋毫不放。**  
> Memory Insight, Nothing Escapes.

**AI-Native 的内存监控 CLI 工具**，专为本地部署的大语言模型与 AI Agent 设计。

让 Agent 感知物理内存——知道还剩多少、能不能跑、该不该缩上下文、要不要保命退出。

## 快速开始

```bash
# 从源码安装
cargo install --path .

# 或者下载预编译二进制
# (从 GitHub Releases 页面下载对应平台版本)

# 使用
hawk-eye-mem --json              # 完整 JSON 输出（给 Agent 用）
hawk-eye-mem --metric available_mb  # 纯数字（给脚本用）
hawk-eye-mem --help              # 看全部参数
```

## Hermes Agent 集成

让 Hermes 能调用秋毫mem 感知内存：

### 1. 安装二进制

```bash
# 编译安装
cargo install --path .

# 或者下载预编译包后：
sudo cp hawk-eye-mem /usr/local/bin/
```

### 2. 安装 Hermes Skill

```bash
# 从仓库安装 Skill
mkdir -p ~/.hermes/skills/mlops/hawk-eye-mem
cp hermes-skill.md ~/.hermes/skills/mlops/hawk-eye-mem/SKILL.md
```

### 3. 使用

在 Hermes 中，秋毫mem 会在以下场景自动被调用：

- **启动 LLM 推理前** — 检查可用内存是否足够
- **长对话中** — 周期性检查内存压力，避免 OOM
- **Agent 决策** — 根据 `agent_guidance` 字段的 `action` 值采取行动：
  - `ok` → 继续正常操作
  - `monitor` → 继续但要关注
  - `reduce_context` → 缩减上下文窗口
  - `abort_safely` → 保存状态，立即退出

## 核心能力

- ✅ 原生 JSON 输出，Agent 直接消费
- ✅ `estimated_safe_context_window` 估算 + `action` 语义指令
- ✅ 四级压力判定（low → medium → high → critical）
- ✅ 保守/校准双模式（无配置自动保守，有配置更精确）
- ✅ SIGINT 优雅退出，连续监控放心跑
- ✅ 零外部依赖，单个二进制文件
- ✅ 跨平台：Linux (`/proc/meminfo`) + macOS (`vm_stat` + `sysctl`)

## CLI 参数

| 参数 | 说明 |
|------|------|
| `--json` | 完整 JSON 输出（含 system + agent_guidance） |
| `--metric <name>` | 极简输出：total_mb / used_mb / available_mb / used_percent / pressure |
| `--config <path>` | 加载自定义模型配置 |
| `--init-config` | 生成默认配置文件到 ~/.config/hawk-eye-mem/config.toml |
| `--interval <sec>` | 连续监控间隔（秒） |
| `--count <N>` | 采集次数（0 = 无限，需配合 --interval） |

## 项目结构

```
hawk-eye-mem/
├── Cargo.toml
├── README.md
├── hermes-skill.md            # Hermes Agent Skill 文件
├── src/
│   ├── main.rs                # CLI 入口 + SIGINT 处理
│   ├── config.rs              # 配置加载（文件/环境变量）
│   ├── collector/
│   │   ├── mod.rs             # MemoryMetrics + MemoryCollector trait
│   │   ├── linux.rs           # Linux 采集（/proc/meminfo）
│   │   └── macos.rs           # macOS 采集（vm_stat + sysctl）
│   └── engine/
│       ├── mod.rs             # EstimationEngine（上下文窗口估算）
│       └── guidance.rs        # GuidanceGenerator（压力判定 + 建议）
├── tests/
│   └── cli_tests.rs           # 20 个集成测试
├── docs/                      # 完整项目文档
│   ├── 立项建议书_v1.0.md
│   ├── 技术方案设计书_v1.1.md
│   └── ...（更多文档）
└── .github/workflows/
    └── test.yml               # CI: Linux + macOS 双平台
```

## 状态

- [x] 第一阶段：调研与立项（已完成）
- [x] 第二阶段：原型验证（已完成）
  - [x] W1: 项目脚手架 + CLI + Linux 采集
  - [x] W2: macOS 采集 + 估算引擎 + 建议生成器
  - [x] W3: 全功能冻结（--init-config + SIGINT）
  - [x] W4: Hermes Skill 集成
- [ ] 第三阶段：发布与社区（进行中）
- [ ] 第四阶段：生态与商业化

## 开源协议

Apache-2.0

---

**GitHub**: github.com/qiuhaomem/-HawkEye-Mem  
**官网**: qiuhao.dev
