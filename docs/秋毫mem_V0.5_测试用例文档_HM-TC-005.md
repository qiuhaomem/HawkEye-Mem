# 秋毫mem V0.5 · 测试用例文档

**文档编号**：HM-TC-005
**版本**：V1.0
**对应版本**：秋毫mem V0.5.0
**编写日期**：2026-05-26
**编写人**：测试经理
**测试范围**：CacheAdvisor模块、MCP增强、缓存Skill交互、自助安装流程、五个暴露点、成本报告、中英文文案、V0.4回归
**总用例数**：**98 个**

---

### 一、测试策略

| 层级 | 自动化 | 覆盖目标 | 通过标准 |
|------|:------:|----------|----------|
| 单元测试 | ✅ | CacheAdvisor、MCP工具、缓存Stats收集器 | 100%通过 |
| 集成测试 | ✅ | Skill与秋毫mem交互、降级运行、CLI参数 | 100%通过 |
| 用户体验测试 | ⚠️ 部分自动化 | 五个暴露点文案、自助安装流程、成本报告水印 | 全部通过 |
| 安全测试 | ✅ | SHA256校验、Stats文件大小限制、模型名脱敏 | 全部通过 |
| 手动验收 | ❌ | 真实环境自助安装、100任务循环测试 | 全部通过 |

---

### 二、V0.4 回归测试

| 用例ID | 测试目标 | 预期结果 |
|--------|----------|----------|
| REG-030 | `--json` 基础结构不变 | 含system、agent_guidance、新增cache_strategy字段 |
| REG-031 | `--can-run` 仍可用 | 输出部署评估 |
| REG-032 | `--env-fingerprint` 仍可用 | 环境指纹正常生成 |
| REG-033 | `--serve` 仍可用 | HTTP服务正常 |
| REG-034 | `--remote` 仍可用 | 远程采集正常 |
| REG-035 | `--trend` 仍可用 | 趋势分析正常 |
| REG-036 | V0.4配置文件兼容 | 正常加载，新增[cache]段使用默认值 |

---

### 三、CacheAdvisor 单元测试（11个）

| 用例ID | 测试目标 | 前置条件 | 预期结果 |
|--------|----------|----------|----------|
| UT-CACHE-001 | 仅接收MemoryPressure参数(CR-22) | total=16384, avail=12000, pressure=low | mode=aggressive, ttl=600s, prefetch=true |
| UT-CACHE-002 | 内存充裕→aggressive | avail=50%总内存 | mode=aggressive, max_cache=avail×20% |
| UT-CACHE-003 | 内存中等→balanced | avail=20%总内存 | mode=balanced, ttl=300s, prefetch=true |
| UT-CACHE-004 | 内存紧张→conservative | avail=10%总内存 | mode=conservative, ttl=60s, prefetch=false |
| UT-CACHE-005 | 内存危机→emergency | avail=3%总内存 | mode=emergency, ttl=0s, max_cache=0, prefetch=false |
| UT-CACHE-006 | pressure=critical→emergency | pressure=critical | mode=emergency |
| UT-CACHE-007 | 边界值：avail=15% | avail=15% | mode=conservative（不含边界） |
| UT-CACHE-008 | 边界值：avail=30% | avail=30% | mode=aggressive或balanced |
| UT-CACHE-009 | reason包含具体数值 | avail=12% | reason含"可用12.0%" |
| UT-CACHE-010 | 2GB低内存环境 | total=2048, avail=600 | 低内存模式生效 |
| UT-CACHE-011 | 无内存数据时回退 | memory=None | mode=aggressive（安全默认值） |

**紧急模式穿透测试（CR-24）**：

| 用例ID | 测试目标 | 前置条件 | 预期结果 |
|--------|----------|----------|----------|
| UT-CACHE-015 | 紧急模式不缓存 | 首次aggressive缓存，第二次返回emergency | 第二次不读缓存 |
| UT-CACHE-016 | 非紧急模式30秒缓存 | 首次aggressive，20秒后再查 | 返回缓存结果 |
| UT-CACHE-017 | 紧急模式立即响应 | 内存从50%骤降至3% | 下次查询≤30ms返回emergency |

---

### 四、MCP 工具测试（12个）

**get_cache_strategy（5个）**：

| 用例ID | 测试目标 | 前置条件 | 预期结果 |
|--------|----------|----------|----------|
| UT-MCP-001 | 基本调用返回完整字段 | MCP Server运行 | JSON含mode/ttl/max_cache/prefetch/reason |
| UT-MCP-002 | 返回protocol_version(CR-23) | 同上 | JSON含`protocol_version: 1` |
| UT-MCP-003 | 可选model_name参数 | 传入model_name | 根据校准数据微调策略 |
| UT-MCP-004 | 无model_name时使用默认 | 不传参数 | 使用全局资源状态判定 |
| UT-MCP-005 | MCP Server不可用时时Skill降级 | 秋毫mem未运行 | Skill返回静态策略 |

**report_cache_hit（7个）**：

| 用例ID | 测试目标 | 前置条件 | 预期结果 |
|--------|----------|----------|----------|
| UT-MCP-010 | 正常汇报命中数据 | 传入hit/miss/cost | 返回`received: true`，写入jsonl |
| UT-MCP-011 | 模型名哈希脱敏(CR-06) | model_name="llama3-8b" | 存储model_hash为16位十六进制 |
| UT-MCP-012 | 返回24小时命中率 | 有历史数据 | hit_rate_24h正确计算 |
| UT-MCP-013 | 汇报失败不影响Skill(CR-02) | 秋毫mem未运行 | Skill降级，不抛异常 |
| UT-MCP-014 | cost_saved_usd精度2位(CR-29) | cost=1.23456 | 存储为1.23 |
| UT-MCP-015 | 单条记录最大1KB(CR-30) | 超长请求 | 拒绝写入，返回错误 |
| UT-MCP-016 | 文件超10MB停止接收(CR-30) | cache_stats.jsonl=10.1MB | 返回false，stderr告警 |

---

### 五、Skill 交互测试

**秋毫mem检测（4个）**：

| 用例ID | 测试目标 | 前置条件 | 预期结果 |
|--------|----------|----------|----------|
| UT-SKILL-001 | 检测到秋毫mem | hawk-eye-mem在PATH | self.mcp_available=true |
| UT-SKILL-002 | 未检测到秋毫mem | 未安装 | self.mcp_available=false |
| UT-SKILL-003 | 引导文案输出(CR-08) | 未安装，LANG=zh_CN | 中文引导文案 |
| UT-SKILL-004 | 引导文案输出(CR-08) | 未安装，LANG=en_US | 英文引导文案 |

**自助安装流程（16个）**：

| 用例ID | 测试目标 | 前置条件 | 预期结果 |
|--------|----------|----------|----------|
| UT-SKILL-010 | 用户确认Y→自动安装 | 输入Y | 自动执行brew install |
| UT-SKILL-011 | 用户输入yes→自动安装 | 输入yes | 同Y |
| UT-SKILL-012 | 用户直接回车→自动安装 | 输入空 | 默认Y |
| UT-SKILL-013 | 用户拒绝→跳过 | 输入n | 输出降级提示 |
| UT-SKILL-014 | brew不可用→curl安装(CR-07) | which brew失败 | 自动走curl下载路径 |
| UT-SKILL-015 | curl下载成功+验证 | 下载完成 | SHA256校验通过 |
| UT-SKILL-016 | SHA256校验失败(CR-28) | 校验和不匹配 | 拒绝安装，提示手动 |
| UT-SKILL-017 | 安装步骤阶段性提示(CR-25) | 安装进行中 | 输出阶段性反馈 |
| UT-SKILL-018 | 安装完成后MCP配置 | 安装成功 | 配置hawk-eye-mem |
| UT-SKILL-019 | MCP连接验证 | 配置完成 | 验证通过 |
| UT-SKILL-020 | 安装失败回退 | brew install失败 | 降级运行 |
| UT-SKILL-021 | 需要sudo时提示手动 | mv失败 | 输出手动命令 |

**自助安装文案验收**：

| 用例ID | 场景 | 预期文案关键词 |
|--------|------|---------------|
| UT-SKILL-022 | 引导确认 | "是否允许我帮你自动安装？[Y/n]" |
| UT-SKILL-023 | 安装成功 | "秋毫mem已就绪，缓存命中率目标99%+。以后每次任务我都会用它帮你省钱。" |
| UT-SKILL-024 | 安装失败 | "请手动执行: brew install hawk-eye-mem" |
| UT-SKILL-025 | 用户拒绝 | "已跳过安装。我将使用静态缓存策略（命中率约60%）" |

**缓存策略获取与应用（6个）**：

| 用例ID | 测试目标 | 前置条件 | 预期结果 |
|--------|----------|----------|----------|
| UT-SKILL-030 | 正常获取策略 | MCP可用 | 获取aggressive/balanced/conservative之一 |
| UT-SKILL-031 | 30秒缓存生效(CR-01) | 20秒内第二次查询 | 返回缓存结果 |
| UT-SKILL-032 | 紧急模式穿透(CR-24) | 策略为emergency | 不读缓存，实时查询 |
| UT-SKILL-033 | 降级到静态策略 | MCP不可用 | 返回balanced静态策略 |
| UT-SKILL-034 | emergency暂停API(CR-05) | mode=emergency | API暂停，其他工具不受影响 |
| UT-SKILL-035 | emergency保存状态 | mode=emergency | 任务状态被保存 |

**成本报告（6个）**：

| 用例ID | 测试目标 | 前置条件 | 预期结果 |
|--------|----------|----------|----------|
| UT-SKILL-040 | 报告含完整数据 | 任务完成 | 总调用/命中/命中率/节省/本月累计 |
| UT-SKILL-041 | 水印-已安装(CR-26) | 秋毫mem就绪 | "秋毫mem已就绪 \| 当前内存压力: low" |
| UT-SKILL-042 | 水印-未安装(CR-26) | 秋毫mem未装 | "安装: brew install hawk-eye-mem" |
| UT-SKILL-043 | 水印上方吸引行(CR-09) | — | "📊 此报告由 秋毫mem 提供技术支持" |
| UT-SKILL-044 | "以账单为准"声明(CR-12) | — | "📝 节省金额为估算值，以API提供商账单为准" |
| UT-SKILL-045 | 报告JSON模式(CR-27) | `--json` | 嵌入report._note字段 |

**五个钓鱼暴露点（11个）**：

| 用例ID | 暴露点 | 触发条件 | 预期文案关键词 |
|--------|--------|----------|---------------|
| UT-SKILL-050 | 首次运行 | 秋毫mem未安装 | "秋毫mem未安装，缓存命中率约60%" |
| UT-SKILL-051 | 内存充裕 | mode=aggressive | "秋毫mem报告内存充裕，启用激进缓存（命中率99.7%）" |
| UT-SKILL-052 | 内存紧张 | mode=conservative | "秋毫mem报告内存压力high，切换保守缓存" |
| UT-SKILL-053 | 危机时刻 | mode=emergency | "秋毫mem发现内存快撑不住了，已紧急清空缓存" |
| UT-SKILL-054 | 动态校准 | confidence→calibrated | "秋毫mem动态校准完成，缓存精度提升15%" |

**中英文文案验收**：

| 用例ID | 暴露点 | 中文关键词 | 英文关键词 |
|--------|--------|-----------|-----------|
| UT-SKILL-055 | 首次运行(中文) | "是否允许我帮你自动安装？[Y/n]" | — |
| UT-SKILL-056 | 首次运行(英文) | — | "Allow me to install it for you? [Y/n]" |
| UT-SKILL-057 | 内存充裕(中文) | "秋毫mem报告内存充裕，启用激进缓存（命中率99.7%）" | — |
| UT-SKILL-058 | 内存充裕(英文) | — | "aggressive cache mode (hit rate 99.7%)" |
| UT-SKILL-059 | 危机(中文) | "秋毫mem发现内存快撑不住了，已帮你紧急清空缓存" | — |
| UT-SKILL-060 | 危机(英文) | — | "emergency cache cleared" |

---

### 六、CLI 集成测试（8个）

| 用例ID | 测试目标 | 输入 | 预期结果 |
|--------|----------|------|----------|
| IT-CLI-050 | `--cache-strategy` | 人类可读 | 彩色终端含mode/TTL/reason |
| IT-CLI-051 | `--cache-strategy --json` | JSON模式 | 完整cache_strategy JSON |
| IT-CLI-052 | `--cache-stats` | 有历史数据 | 显示24h命中率/请求数/节省金额 |
| IT-CLI-053 | `--cache-stats --json` | JSON模式 | 含hit_rate_24h/total_requests等 |
| IT-CLI-054 | `--reset-cache-stats` | 确认 | 清空cache_stats.jsonl |
| IT-CLI-055 | 新增参数`--help`可见 | `--help` | 列出V0.5新参数 |
| IT-CLI-056 | `[cache]`段配置生效 | mode_override=aggressive | 覆盖自动策略 |
| IT-CLI-057 | `[cache]`段可选 | 无[cache]段 | 使用默认值 |

---

### 七、JSON 输出结构测试（5个）

| 用例ID | 测试目标 | 预期结果 |
|--------|----------|----------|
| IT-JSON-050 | system层新增cache_strategy | `--json`含cache_strategy字段 |
| IT-JSON-051 | cache_strategy含protocol_version(CR-23) | `protocol_version: 1` |
| IT-JSON-052 | cache_strategy结构完整 | mode/ttl/max_cache/prefetch/reason |
| IT-JSON-053 | cache_strategy不含原始数据(CR-10) | 不输出history.jsonl原始行 |
| IT-JSON-054 | 暴露点文案嵌入字段(CR-27) | JSON模式下无额外stdout输出 |

---

### 八、安全测试（7个）

| 用例ID | 测试目标 | 前置条件 | 预期结果 |
|--------|----------|----------|----------|
| SEC-010 | curl SHA256校验通过(CR-28) | 正确校验和 | 安装继续 |
| SEC-011 | curl SHA256校验失败(CR-28) | 错误校验和 | 拒绝安装，提示手动 |
| SEC-012 | cache_stats.jsonl模型名脱敏(CR-06) | 写入记录 | model_hash为16位十六进制 |
| SEC-013 | cache_stats.jsonl单条≤1KB(CR-30) | 超大记录 | 拒绝写入 |
| SEC-014 | cache_stats.jsonl≤10MB(CR-30) | 文件超限 | 停止接收，stderr告警 |
| SEC-015 | cost_saved_usd精度2位(CR-29) | 小数超2位 | 四舍五入存储 |
| SEC-016 | 隐私模式不上报cost(CR-29) | privacy_mode=true | cost_saved_usd为null |

---

### 九、用户体验专项测试（22个）

**首次安装体验（4个）**：

| 用例ID | 场景 | 验收标准 |
|--------|------|----------|
| UX-001 | 首次安装完整流程 | 检测→引导→确认→安装→MCP配置→验证→成功文案，全程无卡顿 |
| UX-002 | 首次安装用户拒绝 | Skill降级运行，不报错 |
| UX-003 | 网络慢时安装体验 | curl下载有进度感，不超时崩溃 |
| UX-004 | 已安装时跳过引导 | 直接进入策略模式，无安装引导 |

**成本报告体验（3个）**：

| 用例ID | 场景 | 验收标准 |
|--------|------|----------|
| UX-010 | 终端人类可读性 | 框线对齐，emoji正确显示，颜色区分 |
| UX-011 | 截图传播友好性 | 水印清晰，吸引视线行可见 |
| UX-012 | 多次任务后对比 | "本月累计节省"增长直观 |

**暴露点体验（3个）**：

| 用例ID | 场景 | 验收标准 |
|--------|------|----------|
| UX-020 | 不打扰 | 文案一行，不阻断任务，不重复刷屏 |
| UX-021 | 有记忆点 | 用户试用后能说出"秋毫mem救了我" |
| UX-022 | 不误导 | 用户不会误以为秋毫mem是付费工具 |

---

### 十、自动化验收测试（2个）

| 用例ID | 测试目标 | 验收标准 |
|--------|----------|----------|
| AUTO-001 | 100任务循环测试(CR-14) | 全部完成、命中率准确、暴露点触发正确、水印完整、无泄漏无崩溃 |
| AUTO-002 | 长期运行稳定性(24h) | 无泄漏、无panic、无僵尸进程 |

---

### 十一、测试覆盖率目标

| 模块 | 行覆盖率 | 分支覆盖率 |
|------|:--------:|:----------:|
| `cache/advisor.rs` | >95% | >90% |
| `cache/stats.rs` | >90% | >85% |
| `mcp/cache_strategy.rs` | >90% | >85% |
| `skill`（Python侧） | >85% | >80% |

---

### 十二、执行计划

| 阶段 | 测试类型 | 时间 |
|:----:|----------|:----:|
| W1 | CacheAdvisor单元 + MCP单元 | 随编码 |
| W2 | Skill安装检测/自助安装单元 | 随编码 |
| W3 | Skill缓存策略/成本报告单元 | 随编码 |
| W3-W4 | 集成测试 + 回归测试 | W3开始 |
| W4前半周 | 用户体验专项(UX-001~022) + 安全测试(SEC-010~016) | W4 |
| W4后半周 | 自动化验收(AUTO-001~002) + 覆盖率检查 + 发布 | W4结束 |

**总计**：98 个（单元 41 + 集成 14 + 回归 7 + 安全 7 + 用户体验 22 + 自动化 2 + JSON结构 5）
