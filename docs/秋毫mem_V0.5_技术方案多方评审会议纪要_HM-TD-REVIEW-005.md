# 秋毫mem V0.5 · 技术方案 多方评审会议纪要

**评审对象**：V0.5 技术方案设计书（HM-TD-005）
**会议主持**：技术负责人
**参与方**：架构师、产品经理、安全专家、PMO
**创始人列席**
**结论性质**：通过，含 7 条整改要求

---

### 👤 架构师

方案把秋毫mem和Skill的交互设计得很清晰，MCP解耦也符合一贯彻的架构原则。但有三个设计问题需要修正。

**第一，CacheAdvisor直接依赖`ResourceSnapshot`，但V0.4的`ResourceSnapshot`是一个完整的、包含所有可能字段的大结构体。** 缓存策略只需要内存数据，不需要GPU温度、磁盘IO、多Agent列表等。如果每次调用都要构造完整的`ResourceSnapshot`再传给`CacheAdvisor`，等于为了拿一个字段做了一次全量采集。建议`CacheAdvisor`只接收一个最小化的`MemoryPressure`参数（pressure + available_mb + total_mb），由上层调用方从`ResourceSnapshot`中提取后传入。降低耦合，也让单元测试更好写。

**第二，MCP的`get_cache_strategy`工具没有版本号字段。** 如果V0.6修改了输出格式，Skill会解析失败。建议在返回JSON中增加`protocol_version: 1`。Skill在解析前先检查版本号，不匹配时降级运行并提示用户升级。

**第三，Skill侧30秒缓存`cache_strategy`（CR-01）在紧急模式下可能延误响应。** 如果上次查询结果是`aggressive`，缓存30秒内还没过期，此时内存突然被打满到`critical`，Skill会一直沿用`aggressive`直到缓存过期。建议对`mode=emergency`或`pressure=critical`的返回结果**不做缓存**，每次实时查询。

**结论**：通过。要求CacheAdvisor只接收MemoryPressure参数、MCP增加protocol_version、紧急模式不缓存或立即穿透。

---

### 👤 产品经理

技术方案的模块划分没问题，但从用户体验角度看，有几个文案和流程细节需要补上。

**第一，自助安装流程中，Agent自动执行`brew install`或`curl`下载，这可能需要较长时间。** 当前设计没有进度提示。建议在安装步骤中输出"正在下载... (约1.5MB)"和"安装完成 ✅"的阶段性反馈。

**第二，成本报告底部的"安装: brew install hawk-eye-mem"这一行，如果用户已经装了秋毫mem还提示就很奇怪。** 建议Skill检测秋毫mem状态：已安装时水印显示"秋毫mem已就绪 | 当前内存压力: low"，未安装时显示安装引导。

**第三，暴露点文案没有明确**何时输出、输出到stdout还是stderr、是否在JSON模式下也输出。** 建议：暴露点文案在人类可读模式下输出到stdout；在JSON模式下，文案嵌入`cache_strategy.reason`字段或`report._note`字段，不单独输出。

**结论**：通过。要求安装步骤增加进度提示、水印动态显示、暴露点文案JSON模式嵌入字段。

---

### 👤 安全专家

技术方案中自助安装和缓存Stats有几个安全细节需要加固。

**第一，自助安装脚本通过curl下载二进制，但没有校验文件完整性。** 如果GitHub Releases被劫持或中间人攻击，用户可能安装被篡改的二进制。建议在Skill中硬编码预期的SHA256校验和，下载后自动校验。

**第二，缓存Stats数据`cost_saved_usd`是基于用户API费率的估算。** 如果用户的定价被推断出来，可能间接泄露API使用规模。建议只保留2位小数，标记为可选（隐私模式下不上报）。

**第三，MCP的`report_cache_hit`接口如果被恶意调用，可以往`cache_stats.jsonl`注入大量垃圾数据撑爆磁盘。** 建议增加写入保护：单条记录最大1KB，文件超过10MB自动停止接收并告警。

**结论**：通过。要求curl下载SHA256校验、cost_saved_usd精度限制+隐私模式、文件大小限制。

---

### 👤 PMO

工期4周的预估合理，但有几个执行层面的决策需要确认。

**第一，Skill和秋毫mem由同一个人开发还是分拆？** 技术方案暗示了两个并行轨道：秋毫mem端（Rust）和Skill端（Python）。建议创始人决定资源分配方式。

**第二，Hermes运行时TTL修改的可行性（CR-04）需要在V0.5正式开发前确认。** 如果Hermes社区不支持，Skill侧模拟TTL的开发量约0.5天，不影响4周总工期。建议W1周五前确认。

**第三，如果V0.4延期超过2周，V0.5的4周工期会被压缩到不足2周。** 建议在V0.5的W1设置一个"Go/No-Go"决策点。

**结论**：通过。要求创始人确认资源分配、W1周五前确认Hermes TTL可行性、W1设置Go/No-Go决策点。

---

### 📊 技术负责人裁决与整改决议

| 编号 | 来源 | 整改项 | 类型 | 责任人 |
|:----:|:----:|--------|:----:|:------:|
| CR-22 | 架构师 | CacheAdvisor只接收MemoryPressure最小参数 | 🔴 必须 | 技术负责人 |
| CR-23 | 架构师 | MCP `get_cache_strategy`增加`protocol_version`字段 | 🔴 必须 | 技术负责人 |
| CR-24 | 架构师 | 紧急模式不缓存或立即穿透（30秒内失效） | 🔴 必须 | 技术负责人 |
| CR-25 | 产品经理 | 自助安装增加阶段性进度提示 | 🟡 建议 | 产品经理+技术 |
| CR-26 | 产品经理 | 成本报告水印根据秋毫mem状态动态显示 | 🔴 必须 | 产品经理 |
| CR-27 | 产品经理 | 暴露点文案在JSON模式下嵌入字段，不污染stdout | 🔴 必须 | 技术负责人 |
| CR-28 | 安全专家 | curl下载增加SHA256校验 | 🔴 必须 | 技术负责人 |
| CR-29 | 安全专家 | cost_saved_usd存储精度限制 + 隐私模式可选 | 🟡 建议 | 安全专家+技术 |
| CR-30 | 安全专家 | cache_stats.jsonl单条<1KB，总文件<10MB，超限告警 | 🔴 必须 | 技术负责人 |
| CR-31 | PMO | 创始人确认Skill/秋毫mem开发资源分配 | 🔴 必须 | 创始人 |
| CR-32 | PMO | W1周五前确认Hermes运行时TTL修改可行性 | 🔴 必须 | 技术负责人 |
| CR-33 | PMO | V0.5 W1设置Go/No-Go决策点（若V0.4延期则自动裁剪P1） | 🔴 必须 | PMO |

**新增测试点**：
- CacheAdvisor仅接收MemoryPressure参数的单元测试（CR-22）
- MCP protocol_version版本不匹配时Skill降级测试（CR-23）
- 紧急模式缓存穿透验证（CR-24）
- curl下载SHA256校验通过/失败场景（CR-28）
- cache_stats.jsonl超10MB自动停止并告警（CR-30）

---

### ✅ 评审结论

**技术负责人最终裁定**：V0.5 技术方案通过，准予进入开发阶段。

**开发顺序**：
- **W1前半周**：CR-31（资源分配）+ CR-32（Hermes TTL预研）+ CR-33（Go/No-Go）+ CacheAdvisor（CR-22）+ MCP工具（CR-23）
- **W1后半周**：CacheStatsCollector + JSON输出 + 紧急模式穿透（CR-24）
- **W2**：Skill主体逻辑 + SHA256校验（CR-28）+ JSON模式嵌入（CR-27）
- **W3前半周**：成本报告 + 动态水印（CR-26）+ 中英文文案
- **W3后半周**：配置扩展 + CLI参数 + 文件大小限制（CR-30）
- **W4前半周**：集成测试 + 自动化循环测试
- **W4后半周**：文档 + DISCLAIMER更新 + 发布
