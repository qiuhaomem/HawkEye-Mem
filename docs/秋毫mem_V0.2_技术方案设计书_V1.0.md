# 秋毫mem V0.2 · 技术方案设计书

**文档编号**：HM-TD-002
**版本**：V1.0
**日期**：2026-05-20
**编写人**：技术负责人

---

### 一、架构变更概述

V0.1 的核心架构：`MemoryCollector` trait → Linux/macOS 实现 → 内存指标 → 估算引擎 → 建议生成器。

V0.2 的核心架构变为：

```
┌─────────────────────────────────────────────────────┐
│                   CLI 入口 (main.rs)                  │
│   --json / --can-run / --metric / --interval ...     │
└──────────────┬──────────────────────────┬────────────┘
               │                          │
    ┌──────────▼──────────┐    ┌──────────▼──────────┐
    │   部署评估引擎       │    │   实时监控引擎       │
    │   AssessmentEngine  │    │   MonitoringEngine  │
    │   (--can-run)       │    │   (--json / --metric)│
    └──────────┬──────────┘    └──────────┬──────────┘
               │                          │
               └──────────┬───────────────┘
                          │
            ┌─────────────▼─────────────┐
            │     ResourceCollector      │
            │     (统一 trait)            │
            └─────────────┬─────────────┘
                          │
     ┌────────┬───────────┼───────────┬──────────┐
     │        │           │           │          │
┌────▼───┐ ┌──▼──┐ ┌─────▼────┐ ┌────▼───┐ ┌───▼───┐
│ Memory │ │Disk │ │  CPU     │ │  GPU   │ │未来   │
│Collect │ │Col. │ │  Col.    │ │  Col.  │ │扩展   │
└────────┘ └─────┘ └──────────┘ └────────┘ └───────┘
```

**核心变更**：
1. `MemoryCollector` trait 重命名为 `ResourceCollector`，返回统一的 `ResourceSnapshot`。
2. 新增 `DiskCollector`、`CpuCollector`、`GpuCollector`（实验性）。
3. 新增 `AssessmentEngine`，负责 `--can-run` 的部署评估逻辑。
4. 原有 `EstimationEngine` + `GuidanceGenerator` 合并为 `MonitoringEngine`。

---

### 二、核心数据结构重构

> **评审说明**：本版设计方案中 `collect(&self, snapshot: &mut ResourceSnapshot)` 的设计在评审时被 CR-01 修正为各 Collector 返回独立 `CollectorOutput`。下文的 `ResourceSnapshot` 结构体和 trait 定义保留原样，具体实现以评审纪要 CR-01 为准。

#### 2.1 统一资源快照 `ResourceSnapshot`

```rust
#[derive(Debug, Clone, Serialize)]
pub struct ResourceSnapshot {
    pub memory: Option<MemoryMetrics>,
    pub disk: Option<DiskMetrics>,
    pub cpu: Option<CpuMetrics>,
    pub gpu: Option<Vec<GpuMetrics>>,  // 多GPU
    pub timestamp: String,
    pub collection_duration_ms: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryMetrics {
    pub total_mb: u64,
    pub available_mb: u64,
    pub used_percent: f64,
    pub reclaimable_mb: u64,
    pub pressure: PressureLevel,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskMetrics {
    pub path: String,
    pub total_mb: u64,
    pub available_mb: u64,
    pub used_percent: f64,
    pub pressure: DiskPressure,
    pub growth_rate_mb_per_hour: Option<f64>,  // 仅连续监控时有值
}

#[derive(Debug, Clone, Serialize)]
pub struct CpuMetrics {
    pub cores: u32,
    pub load_avg_1m: f64,
    pub load_avg_5m: f64,
    pub load_avg_15m: f64,
    pub agent_processes_percent: Option<f64>,
    pub pressure: CpuPressure,
}

#[derive(Debug, Clone, Serialize)]
pub struct GpuMetrics {
    pub name: String,
    pub vram_total_mb: u64,
    pub vram_used_mb: u64,
    pub pressure: GpuPressure,
}
```

#### 2.2 统一 Collector trait

```rust
/// 资源采集器统一接口
/// 每个 Collector 负责填充 ResourceSnapshot 中自己对应的字段
pub trait ResourceCollector: Send + Sync {
    /// 采集资源指标，填充到 snapshot 中
    fn collect(&self, snapshot: &mut ResourceSnapshot) -> Result<()>;
}

// 示例：MemoryCollector 实现
impl ResourceCollector for MemoryCollector {
    fn collect(&self, snapshot: &mut ResourceSnapshot) -> Result<()> {
        let metrics = self.get_memory_metrics()?;
        snapshot.memory = Some(metrics);
        Ok(())
    }
}

/// 采集器注册中心：管理所有启用的 Collector
pub struct CollectorRegistry {
    collectors: Vec<Box<dyn ResourceCollector>>,
}

impl CollectorRegistry {
    pub fn new() -> Self { /* 根据平台和 features 注册 Collector */ }
    pub fn collect_all(&self) -> Result<ResourceSnapshot> { /* 串行采集 */ }
}
```

**设计要点**：
- 每个 Collector 独立采集，失败不影响其他 Collector（失败时对应字段置 `None` 并记录 warning 到 stderr）。
- `GpuCollector` 仅在 `#[cfg(feature = "gpu")]` 时编译注册。
- macOS 上 `DiskCollector` 监控路径改为 `~/.cache/` 下常见模型目录。

---

### 三、新增 Collector 实现方案

#### 3.1 DiskCollector

**数据源**：POSIX `statvfs`

```rust
use std::path::Path;

fn get_disk_metrics(path: &Path) -> Result<DiskMetrics> {
    let stat = statvfs(path)?;
    let total = stat.f_blocks * stat.f_bsize;
    let available = stat.f_bavail * stat.f_bsize;  // 非特权用户可用
    // ... 计算百分比、判定压力
}
```

**监控路径**：
1. 用户配置的 `directories.model_cache` 路径。
2. 未配置时自动检测：
   - `~/.cache/huggingface/`
   - `~/.ollama/models/`
   - `~/.lm-studio/models/`
   - `./models/`（当前目录）
3. 所有路径都不可访问时，`disk` 字段为 `None`。

**压力判定**（基于用户需求评审 CR-06 的阈值待定，先用产品经理建议的倍数法）：
- 可用空间 > 2×模型所需 → `ok`
- 可用空间 1.2×~2×模型所需 → `warning`
- 可用空间 < 1.2×模型所需 → `critical`

**连续监控下的增长率**：在 `--interval` 模式下，存储上一次采集值，计算 `growth_rate_mb_per_hour = (current_avail - previous_avail) / interval_hours`。

#### 3.2 CpuCollector

**数据源**：
- Linux: `/proc/loadavg`
- macOS: `sysctl vm.loadavg`

```rust
fn get_cpu_metrics() -> Result<CpuMetrics> {
    let cores = num_cpus::get() as u32;
    let load = read_loadavg()?;
    let agent_percent = get_agent_processes_cpu()?;  // 可选
    
    let pressure = if load.1m < cores as f64 { CpuPressure::Low }
                   else if load.1m < cores as f64 * 2.0 { CpuPressure::Medium }
                   else { CpuPressure::High };
    // ...
}
```

**Agent进程CPU**：遍历进程树，匹配已知Agent框架进程名（从配置文件读取），累加CPU使用率。此功能需要引入 `procfs` crate（仅 Linux）或使用 `sysctl`（macOS）。若无法获取，`agent_processes_percent` 为 `None`。

#### 3.3 GpuCollector（实验性）

**止损条件（评审 CR-01）**：
- W1 实现最简验证：用 NVML 绑定获取显存总量和已用量。
- 验证通过标准：在 musl target 下编译通过，且在至少有 1 块 NVIDIA GPU 的环境下运行正确。
- 如果 1 天内无法通过，**立即切换**为解析 `nvidia-smi --query-gpu=name,memory.total,memory.used --format=csv,noheader,nounits`。

> **评审更新（CR-10）**："1 天"定义为连续 8 个工时投入。超时自动触发切换。止损后原 NVML 方案保留分支，等社区贡献或后续版本启用。

**解析方案**（备用）：
```rust
fn parse_nvidia_smi() -> Result<Vec<GpuMetrics>> {
    let output = Command::new("nvidia-smi")
        .args(["--query-gpu=name,memory.total,memory.used", "--format=csv,noheader,nounits"])
        .output()?;
    // 解析 CSV
}
```

> **评审更新（CR-06）**：解析时按 CSV header 行匹配字段位置，而非硬编码列索引。若 `nvidia-smi` 不在 PATH 中，给出明确提示。

**压力判定**：
- 可用显存 > 50% → `low`
- 20% ~ 50% → `medium`
- < 20% → `high`

**编译控制**：`Cargo.toml`
```toml
[features]
default = []
gpu = ["dep:nvml-wrapper"]  # 或依赖 nvidia-smi 解析则无需额外依赖
```

运行时若无 NVIDIA GPU 或驱动，`GpuCollector` 静默跳过，`gpu` 字段为 `None`。

---

### 四、部署评估引擎 `AssessmentEngine`

#### 4.1 输入参数

```rust
pub struct DeploymentRequest {
    pub model_name: Option<String>,      // 从内置库查找
    pub model_size_b: Option<u64>,       // 手动输入
    pub quantization: Option<String>,    // Q4_K_M, Q5_0 ...
    pub context_window: Option<u32>,
}
```

优先级：若指定 `model_name`，从内置库加载参数；手动参数覆盖内置参数。

#### 4.2 评估逻辑

```rust
impl AssessmentEngine {
    pub fn assess(&self, request: &DeploymentRequest, snapshot: &ResourceSnapshot) -> DeploymentAssessment {
        let mut constraints = Vec::new();
        
        // 1. 内存约束
        if let (Some(mem), Some(required_mem)) = (&snapshot.memory, estimate_memory(request)) {
            if mem.available_mb < required_mem {
                constraints.push(Constraint { resource: "memory", ... });
            }
        }
        
        // 2. 磁盘约束（模型下载空间）
        if let (Some(disk), Some(download_size)) = (&snapshot.disk, estimate_download(request)) {
            if disk.available_mb < download_size {
                constraints.push(Constraint { resource: "disk", ... });
            }
        }
        
        // 3. GPU显存约束（如果请求了GPU且GPU可用）
        if let (Some(gpus), Some(required_vram)) = (&snapshot.gpu, estimate_vram(request)) {
            for gpu in gpus {
                if gpu.vram_available_mb < required_vram {
                    constraints.push(Constraint { resource: "gpu_vram", ... });
                }
            }
        }
        
        // 4. CPU约束（仅警告，不阻止部署）
        if let Some(cpu) = &snapshot.cpu {
            if cpu.load_avg_1m > cpu.cores as f64 * 0.8 {
                constraints.push(Constraint { resource: "cpu", severity: "warning", ... });
            }
        }
        
        // 生成 verdict 和 safe_options
        // ...
    }
}
```

**Verdict 判定**：
- 无约束 → `feasible`
- 有约束但可降级 → `feasible_with_caveats`
- 磁盘空间不足以下载模型 → `infeasible`
- 内存/显存差距超过50%且无降级方案 → `infeasible`

**降级方案生成**：
1. 降低量化（Q4→Q3→Q2）
2. 降低上下文窗口（每次减半，最少2048）
3. 换更小的模型（从内置库查找参数量更小的模型）
最多输出 3 个方案。

> **评审更新（CR-02）**：降级方案改为决策树逻辑，每个约束项独立生成，而非固定优先级。详见评审纪要。

> **评审更新（CR-03）**：每个 constraint 的 `suggestion` 字段增加环境侧补救建议，如"释放 XXX MB 内存即可部署当前配置"。

---

### 五、模型参数库

**数据文件**：`src/models.toml`（编译期嵌入）

```toml
[[models]]
name = "llama3-8b"
size_b = 8000000000
bytes_per_token = 2048
memory_overhead_mb = 512
quantizations = ["Q2_K", "Q3_K_M", "Q4_K_M", "Q5_K_M", "Q6_K", "Q8_0"]
min_context = 2048
max_context = 8192
source = "Meta Llama 3 官方文档"
last_updated = "2026-05"
```

首批预置 8 个模型，加载时按 `name` 匹配。

---

### 六、CLI 参数设计

| 参数 | 类型 | 说明 | 约束 |
|------|------|------|------|
| `--can-run` | flag | 触发部署评估模式 | 与 `--json` 互斥 |
| `--model` | string | 从内置库加载模型参数 | 与 `--model-size` 互斥 |
| `--model-size` | u64 | 手动指定参数量（字节） | 与 `--model` 互斥 |
| `--quantization` | string | 量化方法 | 需配合 `--can-run` |
| `--context` | u32 | 目标上下文窗口 | 需配合 `--can-run` |
| `--compare` | string | 多模型对比，逗号分隔 | 最多 3 个，需配合 `--can-run` |
| `--list-models` | flag | 列出内置模型 | 无 |
| `--features gpu` | 编译选项 | 启用GPU监控 | 需编译时指定 |

---

### 七、风险点与应对

| 风险 | 等级 | 应对 |
|------|:----:|------|
| NVML musl 静态链接不兼容 | 高 | W1 最简验证，1天止损切换 nvidia-smi 解析 |
| 磁盘路径检测在容器内失效 | 中 | 回退到当前目录，配置文件中可自定义路径 |
| `procfs` 引入增加依赖 | 低 | 仅在 Linux 下条件编译，macOS 用 sysctl |
| 模型参数库数据准确性 | 中 | 标注来源和更新时间，社区贡献需审核 |

---

### 八、工期预估（6周）

| 周次 | 任务 | 里程碑 |
|------|------|--------|
| W1 | `ResourceCollector` trait 重构 + 回归测试 | V0.1 功能不受影响 |
| W1-W2 | DiskCollector + CpuCollector 实现 | 磁盘/CPU 监控可用 |
| W2 | 模型参数库 + `--can-run` 评估引擎 | 部署评估可用 |
| W3 | GPU NVML 验证 + 实现（或切换 nvidia-smi） | GPU 实验性可用 |
| W3-W4 | `--compare` + `--list-models` 实现 | 多模型对比和列表 |
| W4-W5 | 集成测试 + 跨平台测试 | 全场景通过 |
| W5-W6 | 文档 + DISCLAIMER 更新 + 发布准备 | v0.2.0 发布 |

---

### 九、配置兼容性

V0.2 配置文件向后兼容 V0.1。新增字段均为可选。

```toml
# V0.2 新增
[model]
name = "llama3-8b"   # 可选，从内置库加载

[directories]
model_cache = "/home/user/.cache/huggingface"  # 可选
agent_process_names = ["hermes", "autogpt"]     # 可选
```

---

**技术负责人总结**：V0.2 的技术核心是 `ResourceCollector` trait 统一抽象 + `AssessmentEngine` 部署评估。GPU 实验性有明确的止损方案，不会成为阻塞项。工期 6 周，里程碑清晰。
