# 秋毫mem V0.2 · 测试用例文档

**文档编号**：HM-TC-002
**版本**：V1.0
**对应版本**：秋毫mem V0.2.0
**编写日期**：2026-05-20
**编写人**：测试经理
**测试范围**：单元测试、集成测试、回归测试、手动验收测试
**测试环境**：Linux (Ubuntu 22.04 x86_64)、macOS (Apple Silicon 14.x)


### 一、测试策略

| 层级 | 自动化 | 覆盖目标 | 执行频率 | 通过标准 |
|------|:------:|----------|----------|----------|
| 单元测试 | ✅ | 所有新增模块 + V0.1回归 | 每次commit | 100%通过 |
| 集成测试 | ✅ | CLI参数组合 + JSON输出格式 | 每次commit | 100%通过 |
| 回归测试 | ✅ | V0.1全部功能 | 每次commit | 100%通过 |
| 手动验收 | ❌ | 真实环境多维度场景 | 每阶段交付前 | 全部场景通过 |

**核心变更测试重点**：
- `ResourceCollector` trait 重构后的接口一致性
- `AssessmentEngine` 决策树降级逻辑
- GPU 采集的双方案（NVML / nvidia-smi 解析）切换
- 配置文件向后兼容性


### 二、V0.1 回归测试用例

确保 trait 重构后 V0.1 所有功能不受影响。

| 用例ID | 测试目标 | 前置条件 | 输入 | 预期结果 |
|--------|----------|----------|------|----------|
| **REG-001** | V0.1 JSON输出结构不变 | V0.2编译 | `hawk-eye-mem --json` | 包含 `system.memory`、`agent_guidance`；所有字段名与V0.1一致 |
| **REG-002** | `--metric available_mb` 仍可用 | 同上 | `hawk-eye-mem --metric available_mb` | stdout纯数字，返回码0 |
| **REG-003** | `--metric pressure` 仍可用 | 同上 | `hawk-eye-mem --metric pressure` | stdout为low/medium/high/critical之一 |
| **REG-004** | `--interval --count` 仍可用 | 同上 | `--json --interval 1 --count 2` | 输出2行JSON Lines |
| **REG-005** | 配置文件向后兼容 | V0.1配置文件 | `hawk-eye-mem --config v0.1_config.toml --json` | 正常加载，confidence=calibrated |
| **REG-006** | `--init-config` 生成新格式 | 无 | `hawk-eye-mem --init-config` | 生成包含`[model]`和`[directories]`的toml |
| **REG-007** | `_note` 字段仍存在 | 无 | `hawk-eye-mem --json` | `agent_guidance._note` 包含免责声明 |
| **REG-008** | 首次运行引导仍可用 | 删除`.onboarded` | `hawk-eye-mem` | 输出免责声明+快速引导 |


### 三、`ResourceCollector` trait 接口测试

| 用例ID | 测试目标 | 前置条件 | 输入 | 预期结果 |
|--------|----------|----------|------|----------|
| **UT-RC-001** | MemoryCollector返回独立结果 | 无 | `MemoryCollector::collect()` | 返回`CollectorOutput::Memory(MemoryMetrics)`，不修改共享状态 |
| **UT-RC-002** | DiskCollector返回独立结果 | 有效路径 | `DiskCollector::collect()` | 返回`CollectorOutput::Disk(DiskMetrics)` |
| **UT-RC-003** | CpuCollector返回独立结果 | 无 | `CpuCollector::collect()` | 返回`CollectorOutput::Cpu(CpuMetrics)` |
| **UT-RC-004** | 单个Collector失败不影响其他 | Mock失败Collector | `Registry::collect_all()` | 失败Collector对应字段为None，其他正常；stderr有warning |
| **UT-RC-005** | Registry正确组装ResourceSnapshot | 全部Collector成功 | `Registry::collect_all()` | `snapshot.memory`、`disk`、`cpu`均为Some |
| **UT-RC-006** | macOS上GpuCollector未注册（无feature） | macOS, 未启用gpu feature | `Registry::collect_all()` | `snapshot.gpu`为None |


### 四、磁盘监控测试

| 用例ID | 测试目标 | 前置条件 | 输入 | 预期结果 |
|--------|----------|----------|------|----------|
| **UT-DK-001** | 正常路径采集 | 有效路径存在 | `DiskCollector::collect()` | 返回`total_mb > 0`, `available_mb > 0`, `used_percent`合理 |
| **UT-DK-002** | 路径不存在 | 路径`/nonexistent` | 同上 | 返回Error，不panic |
| **UT-DK-003** | 路径无权限 | 路径`/root`(非root运行) | 同上 | 返回Error或available=null,error="permission_denied" |
| **UT-DK-004** | 自动检测huggingface目录 | `~/.cache/huggingface/`存在 | `DiskCollector::auto_detect()` | 路径包含huggingface |
| **UT-DK-005** | 自动检测ollama目录 | `~/.ollama/models/`存在 | 同上 | 路径包含ollama |
| **UT-DK-006** | 所有默认路径不存在 | 全部默认路径不存在 | 同上 | 返回Error，snapshot.disk为None |
| **UT-DK-007** | 磁盘压力判定-ok | available > 2×模型所需 | 判定pressure | `disk_pressure = ok` |
| **UT-DK-008** | 磁盘压力判定-warning | available 1.2~2×模型所需 | 同上 | `disk_pressure = warning` |
| **UT-DK-009** | 磁盘压力判定-critical | available < 1.2×模型所需 | 同上 | `disk_pressure = critical` |
| **UT-DK-010** | 增长率计算 | 连续两次采集 | 间隔1秒，available差100MB | `growth_rate_mb_per_hour ≈ 360000` |
| **UT-DK-011** | 路径脱敏(CR-08) | 路径`/home/user/.cache/models` | JSON输出 | 路径显示为`~/.cache/models` |


### 五、CPU监控测试

| 用例ID | 测试目标 | 前置条件 | 输入 | 预期结果 |
|--------|----------|----------|------|----------|
| **UT-CPU-001** | 负载平均值采集(Linux) | Linux | `CpuCollector::collect()` | `load_avg_1m/5m/15m`均为正浮点数 |
| **UT-CPU-002** | 负载平均值采集(macOS) | macOS | 同上 | 同上 |
| **UT-CPU-003** | 核心数获取 | 任意平台 | 同上 | `cores >= 1` |
| **UT-CPU-004** | CPU压力-low | load < cores | 判定pressure | `cpu_pressure = low` |
| **UT-CPU-005** | CPU压力-medium | cores ≤ load < 2×cores | 同上 | `cpu_pressure = medium` |
| **UT-CPU-006** | CPU压力-high | load ≥ 2×cores | 同上 | `cpu_pressure = high` |
| **UT-CPU-007** | Agent进程CPU（可匹配hermes） | hermes运行中 | 同上 | `agent_processes_percent > 0` |
| **UT-CPU-008** | Agent进程CPU（无可匹配进程） | 无Agent框架运行 | 同上 | `agent_processes_percent`为None或0 |
| **UT-CPU-009** | 自定义进程名匹配 | 配置`agent_process_names=["custom-agent"]` | 同上 | 匹配custom-agent进程 |


### 六、GPU监控测试（实验性）

| 用例ID | 测试目标 | 前置条件 | 输入 | 预期结果 |
|--------|----------|----------|------|----------|
| **UT-GPU-001** | NVML方案验证(8工时止损) | NVIDIA GPU + 驱动 | `GpuCollector::collect()` | 返回`name`、`vram_total_mb`、`vram_used_mb` |
| **UT-GPU-002** | nvidia-smi解析方案 | nvidia-smi在PATH | 同上 | CSV解析正确，字段匹配 |
| **UT-GPU-003** | nvidia-smi header匹配(CR-06) | nvidia-smi输出变化 | 同上 | 按header行匹配字段位置，不按硬编码列索引 |
| **UT-GPU-004** | nvidia-smi不在PATH | PATH中无nvidia-smi | 同上 | 返回Error，明确提示"nvidia-smi not found" |
| **UT-GPU-005** | 多GPU检测 | 2+ GPU | 同上 | 返回数组，每个GPU一个元素 |
| **UT-GPU-006** | 无GPU时静默跳过 | 无GPU/驱动 | 同上 | snapshot.gpu为None，无报错 |
| **UT-GPU-007** | 显存压力-low | available > 50% | 判定pressure | `gpu_pressure = low` |
| **UT-GPU-008** | 显存压力-medium | available 20-50% | 同上 | `gpu_pressure = medium` |
| **UT-GPU-009** | 显存压力-high | available < 20% | 同上 | `gpu_pressure = high` |
| **UT-GPU-010** | unsafe代码集中封装(CR-07) | 代码审查 | 检查源码 | 所有NVML unsafe在单一模块，顶部有`#[allow(unsafe_code)]`注释 |
| **UT-GPU-011** | 未启用gpu feature时不编译 | 默认编译 | `cargo build` | GPU模块不存在，`--features gpu`无效 |


### 七、部署评估引擎测试

| 用例ID | 测试目标 | 前置条件 | 输入 | 预期结果 |
|--------|----------|----------|------|----------|
| **UT-AE-001** | 全部资源充足→feasible | 充足资源快照 | 请求8B模型，Q4，8K上下文 | `verdict = feasible`，`constraints = []` |
| **UT-AE-002** | 内存不足→feasible_with_caveats | 内存不足快照 | 请求8B模型，Q4，8K上下文 | `verdict = feasible_with_caveats`，constraints包含memory项 |
| **UT-AE-003** | 磁盘不足→infeasible | 磁盘不足快照 | 请求8B模型 | `verdict = infeasible`，constraints包含disk项 |
| **UT-AE-004** | 显存不足→feasible_with_caveats | 显存不足快照 | 请求8B模型，GPU推理 | constraints包含gpu_vram项 |
| **UT-AE-005** | 决策树-降量化(CR-02) | 内存不足，量化可降 | 请求Q4_K_M | safe_options含Q3_K_M方案 |
| **UT-AE-006** | 决策树-降上下文(CR-02) | 内存不足，量化已最低 | 请求Q2_K，8K上下文 | safe_options含4K上下文方案 |
| **UT-AE-007** | 决策树-换小模型(CR-02) | 内存不足，量化+上下文已最低 | 请求8B，Q2_K，2K上下文 | safe_options含3B模型方案 |
| **UT-AE-008** | 决策树-磁盘不足无法降级 | 磁盘不足 | 请求任意模型 | safe_options提示清理磁盘空间 |
| **UT-AE-009** | 最多3个降级方案 | 多项不足 | 请求 | `safe_options`数组长度≤3 |
| **UT-AE-010** | constraint含环境侧建议(CR-03) | 内存差344MB | 请求 | suggestion含"释放至少344MB内存" |
| **UT-AE-011** | CPU约束仅警告不阻止 | CPU高负载 | 请求 | verdict仍为feasible或feasible_with_caveats，constraints含cpu但severity=warning |
| **UT-AE-012** | 模型名从内置库加载 | 内置库有llama3-8b | `--can-run --model llama3-8b` | 正确加载bytes_per_token等参数 |
| **UT-AE-013** | 手动参数覆盖内置参数 | 内置库有llama3-8b | `--can-run --model llama3-8b --model-size 9000000000` | 使用手动指定的size |


### 八、模型参数库测试

| 用例ID | 测试目标 | 前置条件 | 输入 | 预期结果 |
|--------|----------|----------|------|----------|
| **UT-ML-001** | 按名称加载 | 内置库 | `load_model("llama3-8b")` | 返回完整ModelConfig |
| **UT-ML-002** | 不存在的模型名 | 内置库 | `load_model("nonexistent")` | 返回Error，提示"未找到模型" |
| **UT-ML-003** | 8个模型全部可加载 | 内置库 | 遍历8个模型 | 全部加载成功 |
| **UT-ML-004** | 每个模型含必要字段 | 内置库 | 检查每个模型 | size_b、bytes_per_token、quantizations、min/max_context非空 |
| **UT-ML-005** | 数据来源标注(CR-07) | 内置库 | 检查source字段 | 每个模型有source和last_updated |
| **UT-ML-006** | `--list-models`输出 | 无 | CLI运行 | 彩色表格，含模型名/参数量/推荐量化/最低内存/来源 |
| **UT-ML-007** | `--list-models`颜色判定(CR-05) | 当前系统内存16GB | CLI运行 | 8B模型绿色，70B模型红色 |
| **UT-ML-008** | `--contribute-model`输出 | 无 | CLI运行 | 输出贡献模板，含必要字段说明 |


### 九、`--compare` 多模型对比测试

| 用例ID | 测试目标 | 前置条件 | 输入 | 预期结果 |
|--------|----------|----------|------|----------|
| **IT-CMP-001** | 双模型对比 | 内置库 | `--can-run --compare llama3-8b,qwen2-7b` | 输出两个模型的评估结果 |
| **IT-CMP-002** | 最多3个模型(CR-03) | 内置库 | `--can-run --compare a,b,c,d` | 报错或只评估前3个 |
| **IT-CMP-003** | 高亮推荐项(CR-04) | 一个能跑一个不能 | 同上 | 标注哪个更适合当前环境 |
| **IT-CMP-004** | 共享一次采集 | 内置库 | `--compare` 3个模型 | collection_duration_ms仅一个值，非3次采集 |


### 十、CLI集成测试

| 用例ID | 测试目标 | 前置条件 | 输入 | 预期结果 |
|--------|----------|----------|------|----------|
| **IT-CLI-020** | `--can-run` JSON输出 | 内置库 | `hawk-eye-mem --can-run --model llama3-8b --json` | 含`deployment_assessment`字段，不含`agent_guidance` |
| **IT-CLI-021** | `--can-run` 人类可读输出 | 内置库 | `hawk-eye-mem --can-run --model llama3-8b` | 彩色输出，✅/⚠️/❌标记 |
| **IT-CLI-022** | `--can-run` 与 `--json` 互斥 | 无 | `--can-run --json --interval 5` | 报错退出 |
| **IT-CLI-023** | `--model` 与 `--model-size` 互斥 | 无 | `--can-run --model llama3-8b --model-size 8000000000` | 报错退出 |
| **IT-CLI-024** | 新增参数 `--help` 可见 | 无 | `--help` | 列出`--can-run`、`--model`、`--compare`等 |
| **IT-CLI-025** | 磁盘路径自定义 | 配置文件 | `--config custom.toml --json` | disk.path使用自定义路径 |


### 十一、JSON输出结构测试

| 用例ID | 测试目标 | 前置条件 | 输入 | 预期结果 |
|--------|----------|----------|------|----------|
| **IT-JSON-020** | system.memory字段保留 | 无 | `--json` | 包含`total_mb`、`available_mb`、`used_percent`、`pressure` |
| **IT-JSON-021** | system.disk字段新增 | 磁盘路径可用 | `--json` | 包含`path`、`total_mb`、`available_mb`、`pressure` |
| **IT-JSON-022** | system.cpu字段新增 | 无 | `--json` | 包含`cores`、`load_avg_1m/5m/15m`、`pressure` |
| **IT-JSON-023** | system.gpu字段条件存在 | 有GPU+feature | `--json` | 数组，每元素含`name`、`vram_total_mb`、`vram_used_mb` |
| **IT-JSON-024** | system.gpu字段条件不存在 | 无GPU | `--json` | `system.gpu`字段不存在或为null |
| **IT-JSON-025** | deployment_assessment结构 | `--can-run` | JSON | 含`request`、`verdict`、`constraints`、`safe_options` |
| **IT-JSON-026** | agent_guidance含多维度action | 多维度压力 | `--json` | 含`action`(内存)、`disk_action`、`cpu_action` |


### 十二、手动验收测试

| 场景ID | 场景 | 环境 | 步骤 | 验收标准 |
|--------|------|------|------|----------|
| **MA-001** | 部署前评估-可部署 | 32GB Linux | `--can-run --model llama3-8b` | verdict=feasible，全部✅ |
| **MA-002** | 部署前评估-内存不足 | 8GB Mac | `--can-run --model llama3-70b` | verdict=infeasible或feasible_with_caveats，明确提示内存差距 |
| **MA-003** | 部署前评估-磁盘不足 | 磁盘<10GB | `--can-run --model llama3-8b` | verdict=infeasible，提示磁盘空间不足 |
| **MA-004** | 磁盘压力-正常 | 磁盘>50GB | `--json` | disk_pressure=ok |
| **MA-005** | 磁盘压力-临界 | 磁盘<5GB | `--json` | disk_pressure=critical |
| **MA-006** | CPU压力-正常 | 空闲 | `--json` | cpu_pressure=low |
| **MA-007** | CPU压力-高负载 | 运行stress | `--json` | cpu_pressure=high |
| **MA-008** | GPU监控-有GPU | NVIDIA GPU机器 | `--features gpu`编译后`--json` | gpu字段存在，显存数据准确 |
| **MA-009** | GPU监控-无GPU | 无GPU机器 | 同上 | gpu字段不存在，不报错 |
| **MA-010** | 多模型对比 | 任意 | `--can-run --compare llama3-8b,qwen2-7b,phi3-mini` | 3个模型对比输出，推荐最适合的 |
| **MA-011** | 配置文件向后兼容 | V0.1配置文件 | V0.2运行 | 正常加载，无报错 |
| **MA-012** | 5分钟上手V0.2 | 新环境 | 按README操作 | 安装→首次运行→`--can-run`→看懂输出 |


### 十三、测试覆盖率目标

| 模块 | 行覆盖率 | 分支覆盖率 | 备注 |
|------|:--------:|:----------:|------|
| `src/engine/mod.rs` (评估引擎) | >95% | >90% | 决策树所有分支 |
| `src/engine/guidance.rs` | 100% | 100% | 保持V0.1目标 |
| `src/collector/memory.rs` | >85% | >80% | 保持V0.1 |
| `src/collector/disk.rs` | >85% | >80% | 新增 |
| `src/collector/cpu.rs` | >85% | >80% | 新增 |
| `src/collector/gpu.rs` | >80% | >75% | 实验性，受限于硬件 |
| `src/collector/registry.rs` | >90% | >85% | 新增 |
| `src/models.rs` (参数库) | 100% | 100% | 纯数据+查找 |
| `src/config.rs` | >90% | >85% | 保持V0.1 |


### 十四、执行计划

| 阶段 | 测试类型 | 责任人 | 时间 |
|------|----------|--------|------|
| W1 | 回归测试(REG-001~008) + trait接口测试(UT-RC) | 开发+测试 | W1重构完成后立即执行 |
| W1-W2 | 磁盘+CPU单元测试 | 开发 | 随编码 |
| W2 | 评估引擎+模型参数库单元测试 | 开发 | 随编码 |
| W3 | GPU单元测试（有条件环境） | 开发 | GPU验证通过后 |
| W3-W4 | 集成测试(IT-CLI, IT-JSON, IT-CMP) | 开发+测试 | 全功能冻结后 |
| W4-W5 | 手动验收(MA-001~012) | 测试经理+产品经理 | 集成测试通过后 |
| W5 | 覆盖率检查+补充测试 | 测试经理 | 发布前 |


---

**测试经理总结**：V0.2测试用例覆盖了所有新增模块、架构重构回归、CLI参数组合、JSON输出结构。GPU测试因硬件环境限制，单元测试要求80%行覆盖，其余靠手动验收。决策树降级逻辑和Collector接口变更是测试重点。
