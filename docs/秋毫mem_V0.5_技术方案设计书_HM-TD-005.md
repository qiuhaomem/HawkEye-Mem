# 秋毫mem V0.5 · 技术方案设计书

**文档编号**：HM-TD-005
**版本**：V1.0
**日期**：2026-05-26
**编写人**：技术负责人

---

### 一、架构变更概述

V0.4核心架构回顾：CLI入口 → 环境指纹/远程采集/趋势分析/容器适配/多Agent视图 → ResourceCollector。

V0.5新增两个核心模块：

```
┌──────────────────────────────────────────────────────────────┐
│                        CLI 入口 (main.rs)                     │
│  + --cache-strategy / --cache-stats / --reset-cache-stats    │
└──────┬────────┬──────────┬──────────┬──────────┬─────────────┘
       │        │          │          │          │
┌──────▼──┐ ┌──▼───┐ ┌───▼───┐ ┌───▼────┐ ┌───▼──────┐
│ 环境指纹 │ │ 远程 │ │ 趋势  │ │ 缓存   │ │ Skill    │
│ 引擎    │ │ 采集 │ │ 分析  │ │ Advisor │ │ 交互协议 │
│ (V0.4)  │ │(V0.4)│ │(V0.4) │ │ (NEW)  │ │ (NEW)   │
└────┬────┘ └──┬───┘ └───┬───┘ └───┬────┘ └────┬─────┘
     │         │         │         │            │
     └─────────┼─────────┼─────────┼────────────┘
               │         │         │
          ┌────▼────┐ ┌─▼──────┐ ┌▼──────────┐
          │ 本地存储 │ │ MCP    │ │ Resource  │
          │ (JSON/  │ │ Server │ │ Collector │
          │  JSONL) │ │ (增强) │ │ (已有)    │
          └─────────┘ └────────┘ └───────────┘
```

**核心变更**：
1. **CacheAdvisor**：根据系统资源压力计算推荐缓存策略。
2. **MCP Server增强**：新增 `get_cache_strategy` 和 `report_cache_hit` 两个工具。
3. **Skill交互协议**：定义Skill与秋毫mem之间的数据交换格式。
4. **Skill侧自主安装**：引导用户确认后，Agent自动完成秋毫mem安装和MCP配置。

---

### 二、秋毫mem端：CacheAdvisor模块

#### 2.1 模块职责

```rust
pub struct CacheAdvisor;

impl CacheAdvisor {
    /// 根据系统资源压力计算缓存策略
    pub fn recommend(
        snapshot: &ResourceSnapshot,
        config: &CacheConfig,
    ) -> CacheStrategy {
        let mem = snapshot.memory.as_ref();
        let available_pct = mem.map(|m| m.available_mb as f64 / m.total_mb as f64 * 100.0)
            .unwrap_or(100.0);
        let pressure = mem.map(|m| &m.pressure);

        match pressure {
            // CR-05: emergency 模式只影响缓存，不影响其他工具
            Some(PressureLevel::Critical) | _ if available_pct < 5.0 => {
                CacheStrategy {
                    mode: CacheMode::Emergency,
                    ttl_seconds: 0,
                    max_cache_mb: 0,
                    prefetch_enabled: false,
                    reason: format!("内存危机（可用{:.1}%），立即清空缓存保命", available_pct),
                }
            }
            Some(PressureLevel::High) | _ if available_pct < 15.0 => {
                CacheStrategy {
                    mode: CacheMode::Conservative,
                    ttl_seconds: 60,
                    max_cache_mb: Self::calc_max_cache(mem, 0.05),
                    prefetch_enabled: false,
                    reason: format!("内存压力high（可用{:.1}%），切换保守缓存", available_pct),
                }
            }
            Some(PressureLevel::Medium) | _ if available_pct < 30.0 => {
                CacheStrategy {
                    mode: CacheMode::Balanced,
                    ttl_seconds: 300,
                    max_cache_mb: Self::calc_max_cache(mem, 0.10),
                    prefetch_enabled: true,
                    reason: format!("内存压力medium（可用{:.1}%），保持平衡缓存", available_pct),
                }
            }
            _ => {
                CacheStrategy {
                    mode: CacheMode::Aggressive,
                    ttl_seconds: 600,
                    max_cache_mb: Self::calc_max_cache(mem, 0.20),
                    prefetch_enabled: true,
                    reason: format!("内存充裕（可用{:.1}%），启用激进缓存，预计命中率99%+", available_pct),
                }
            }
        }
    }

    fn calc_max_cache(mem: Option<&MemoryMetrics>, ratio: f64) -> u64 {
        mem.map(|m| (m.available_mb as f64 * ratio) as u64).unwrap_or(0)
    }
}
```

#### 2.2 新增数据结构

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStrategy {
    pub mode: CacheMode,
    pub ttl_seconds: u64,
    pub max_cache_mb: u64,
    pub prefetch_enabled: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CacheMode {
    Aggressive,
    Balanced,
    Conservative,
    Emergency,
}
```

#### 2.3 缓存Stats收集器

```rust
pub struct CacheStatsCollector {
    store: CacheStatsStore,
}

impl CacheStatsCollector {
    /// Skill汇报缓存命中数据（CR-02：fire-and-forget）
    pub fn report(&self, report: CacheHitReport) {
        let _ = self.store.append(report);
    }

    /// 计算24小时命中率
    pub fn stats_24h(&self) -> CacheStats {
        let cutoff = Utc::now() - Duration::hours(24);
        let reports = self.store.read_since(cutoff).unwrap_or_default();

        let total_hits: u64 = reports.iter().map(|r| r.hit_count).sum();
        let total_misses: u64 = reports.iter().map(|r| r.miss_count).sum();
        let total_requests = total_hits + total_misses;

        CacheStats {
            hit_rate_24h: if total_requests > 0 {
                (total_hits as f64 / total_requests as f64 * 10000.0).round() / 100.0
            } else {
                0.0
            },
            total_requests_24h: total_requests,
            total_hits_24h: total_hits,
            estimated_savings_usd: Self::estimate_savings(total_hits, &reports),
        }
    }
}
```

**存储文件**：`~/.config/hawk-eye-mem/cache_stats.jsonl`（CR-06：模型名哈希脱敏）
**数据保留**：30天，超过30天的记录自动清理。

---

### 三、MCP Server 增强

#### 3.1 新增工具：`get_cache_strategy`

```json
{
  "name": "get_cache_strategy",
  "description": "获取当前系统资源状态下推荐的最佳缓存策略。返回激进/平衡/保守/紧急四种模式及对应参数。",
  "inputSchema": {
    "type": "object",
    "properties": {
      "model_name": {
        "type": "string",
        "description": "可选，指定模型名以获取针对该模型校准的策略"
      }
    },
    "required": []
  }
}
```

#### 3.2 新增工具：`report_cache_hit`

```json
{
  "name": "report_cache_hit",
  "description": "向秋毫mem汇报本次任务的缓存命中数据，用于统计24小时命中率。",
  "inputSchema": {
    "type": "object",
    "properties": {
      "model_name": { "type": "string", "description": "使用的模型名（将哈希后存储）" },
      "hit_count": { "type": "integer", "description": "缓存命中次数" },
      "miss_count": { "type": "integer", "description": "缓存未命中次数" },
      "cost_saved_usd": { "type": "number", "description": "本次任务估算节省的API费用（美元）" }
    },
    "required": ["model_name", "hit_count", "miss_count"]
  },
  "outputSchema": {
    "type": "object",
    "properties": {
      "received": { "type": "boolean" },
      "hit_rate_24h": { "type": "number", "description": "当前24小时缓存命中率" }
    }
  }
}
```

---

### 四、Skill端：hermes-cache-strategy

#### 4.1 文件结构

```
~/.hermes/skills/hermes-cache-strategy/
├── SKILL.md                 # Skill描述和触发条件
├── skill.py                 # 主逻辑
├── install.sh               # 秋毫mem自助安装脚本
├── config/
│   └── defaults.yaml        # 静态铁律（降级模式用）
├── i18n/
│   ├── zh.json              # 中文文案
│   └── en.json              # 英文文案
├── provider_cache_compat.json  # Provider-模型缓存兼容矩阵
└── test/
    └── simulate_100_tasks.sh # 自动化循环测试
```

#### 4.2 核心逻辑流程

Skill 核心逻辑包含以下模块：

1. **安装检测与自助安装**（`detect_qiuhao` / `offer_auto_install`）
   - 检测秋毫mem是否已安装
   - 引导用户确认安装（CR-15）
   - brew/curl 双路径安装（CR-07）
   - 自动配置MCP、验证连接

2. **缓存策略获取**（`get_strategy`）
   - CR-01：30秒本地缓存策略结果
   - 秋毫mem可用时通过MCP获取
   - 不可用时降级为静态铁律

3. **缓存模式应用**（`apply_strategy`）
   - CR-05：emergency只暂停API调用，不影响其他工具
   - aggressive/balanced/conservative/emergency 四种模式

4. **成本报告**（`generate_report`）
   - CR-12：含"以账单为准"声明
   - CR-02：fire-and-forget汇报缓存数据

5. **国际化文案**（CR-08）
   - i18n/zh.json 和 i18n/en.json
   - 根据系统 LANG 环境变量自动切换

**关键设计点**：
- CR-01（30秒缓存）：`self.cached_strategy` + `self.cached_strategy_at`
- CR-02（fire-and-forget）：`_report_to_qiuhao` 失败不抛异常不重试
- CR-05（emergency不阻止其他工具）：`pause_api_requests()` 只暂停API调用
- CR-06（模型名哈希）：秋毫mem端存储时哈希，Skill侧传明文

---

### 五、配置文件扩展

```toml
# V0.5 新增 [cache] 段（全部可选）
[cache]
# mode_override = "aggressive"     # 手动覆盖缓存模式
# max_cache_mb_override = 4096    # 手动覆盖最大缓存量
# stats_retention_days = 30       # 缓存统计数据保留天数
```

---

### 六、新增CLI参数

| 参数 | 功能 | 示例 |
|------|------|------|
| `--cache-strategy` | 输出当前推荐的缓存策略 | `hawk-eye-mem --cache-strategy` |
| `--cache-stats` | 输出24小时缓存命中统计 | `hawk-eye-mem --cache-stats` |
| `--reset-cache-stats` | 清空缓存统计数据 | `hawk-eye-mem --reset-cache-stats` |
| `--model-compat` | 查看模型缓存兼容性 | `hawk-eye-mem --model-compat qwen3-32b@groq` |

---

### 七、关键风险与应对

| 风险 | 等级 | 应对 |
|------|:----:|------|
| Hermes不支持运行时TTL修改 | 中 | CR-04：降级为Skill侧模拟TTL |
| Skill和秋毫mem版本不匹配 | 中 | MCP接口版本号检查 |
| 缓存Stats文件膨胀 | 低 | 30天自动清理，单文件最大10MB |
| 自助安装脚本在受限环境失败 | 中 | 每一步失败有明确fallback提示 |

---

### 八、工期预估（4周）

| 周次 | 任务 | 里程碑 |
|:----:|------|--------|
| W1前半周 | CR-13：冻结MCP接口定义 + CacheAdvisor实现 | `cache_strategy`字段可用 |
| W1后半周 | MCP `get_cache_strategy` + `report_cache_hit` + CacheStatsCollector | 秋毫mem端全部可用 |
| W2 | Skill主体逻辑：安装检测+自助安装+策略获取+模式应用 | Skill核心功能可用 |
| W3前半周 | 成本报告 + 水印 + 中英文文案（CR-08） | Skill完整功能可用 |
| W3后半周 | 配置扩展 + CLI参数 + 并发度与缓存联动（CR-03） | 全功能可用 |
| W4前半周 | 集成测试 + 自动化循环测试脚本（CR-14） | 测试通过 |
| W4后半周 | 文档 + DISCLAIMER更新 + 发布准备 | v0.5.0 发布 |

**技术负责人总结**：V0.5 的技术核心是"协同"——秋毫mem提供决策依据，Skill执行具体动作。秋毫mem新增的`CacheAdvisor`模块不到200行代码，核心逻辑是根据内存压力映射四种缓存模式。Skill侧约500行Python，重点是自助安装流程和缓存模式切换。两者通过MCP解耦，Skill降级时仍可独立运行。工期4周，风险可控。
