# 秋毫mem v0.3.0 — 温度+GPU完全体+多Agent检测

**发布日期：** 2026-05-21

## 🌟 新增功能

### 🎮 GPU 完全体（Phase 4）
- NVIDIA NVML 直绑（温度/功耗/利用率采集）
- NVIDIA nvidia-smi 降级路径
- AMD ROCm 支持（rocm-smi CSV 解析）
- Apple Silicon 支持（Metal API → sysctl 回退）
- GPU 温度/功耗节流警告
- `--gpu-list` 列出检测到的 GPU 及采集后端

### 🌡️ 温度监控（Phase 5）
- CPU 温度采集（Linux: `/sys/class/thermal/`，macOS: `pmset`）
- GPU 温度采集（各 GPU Collector 内部上报）
- 温度压力等级：Normal / Warning / Critical
- CR-05：只采集不预警，附带说明文案

### 👥 多 Agent 进程检测（Phase 6）
- 内置 7 个已知 Agent 进程名（hermes/claude-code/autogpt/等）
- Linux：`/proc/[pid]/comm` + VmRSS
- macOS：`ps -eo pid,comm`
- CR-06：只检测不预警，不读 cmdline
- 配置 `extra_process_names` 可扩展

### 🚦 连续监控状态机（Phase 2）
- 三态模型：Normal → Warning → Critical
- 双条件转换：时间 + 连续采样次数
- 紧急快速通道（available < 512MB 或 used > 98%）
- 仅在 `--interval` 模式下激活

### 🎯 动态校准引擎（Phase 0+1+3）
- CalibrationStore trait + CsvStore 实现（flock 文件锁）
- 加权平均修正算法（最新优先）
- confidence 自动升降级（10样本 + CV≤20%）
- 叙事性进度条（五阶段：还没开始/刚开始/.../已校准）
- MCP Tool 集成：`--tokens-processed` 自动校准
- `--calibration-stats` / `--reset-calibration`

### 📝 配置扩展（Phase 2+6）
- `[calibration]`：enabled / max_samples / min_samples_for_calibrated
- `[state_machine]`：warning_seconds / critical_seconds / 等
- `[multi_agent]`：enabled / extra_process_names
- `[gpu]`：rocm_smi_path

### 🛠 MCP Server 升级
- 3 个工具 → **7 个工具**：新增 get_gpu_status / get_thermal_status / get_agent_processes / get_calibration_status
- 完整集成秋毫mem V0.3 所有新能力

## 📊 测试统计
- **233 个测试**全部通过（183 单元 + 22 CLI + 28 压力）
- 三平台 CI：Linux ✅ / macOS ⏳（runner 排队中）/ musl ✅

## 📦 下载

各平台二进制见 Assets 区域。

## ⚠️ 已知限制
- 温度 V0.3 只采集不预警，自动预警将在 V0.4 提供
- Apple Metal API 通过 sysctl 回退，完整 Metal FFI 绑定待 V0.4
- macOS CI runner 偶尔排队较长，非代码问题
