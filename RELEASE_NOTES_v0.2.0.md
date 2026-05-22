# HawkEye Mem v0.2.0 · 秋毫mem

> **这次更新，就为了一件事：在你花几小时下载模型之前，先告诉你，你的电脑能不能跑得动。**

## ✨ V0.2 新功能

### `--can-run` 部署前检测
花几小时下载一个模型，加载时崩了——这是最难受的。`--can-run` 在你下载之前就告诉你答案：

```bash
hawk-eye-mem --can-run --model llama3-8b
```

三种判定结果：
- ✅ **Feasible** — 放心下载
- ⚠️ **Feasible With Caveats** — 差一点，但可以降量化/减上下文/换小模型
- ❌ **Infeasible** — 差距太大

支持 `--json` 输出给 Agent 用，也支持 `--compare` 多模型对比。

### 多维度系统监控
- **磁盘监控** — 自动检测模型缓存目录，压力判定 Ok/Warning/Critical
- **CPU 监控** — 负载平均值 + 核心数 + 压力判定
- **GPU 监控（实验性）** — NVIDIA NVML + nvidia-smi 双路径，`--features gpu` 编译

### 模型参数库
内置 8 个主流模型参数（llama3-8b、qwen2-7b、deepseek-v2-lite 等），`--list-models` 查看，`--can-run` 自动匹配。

### 更多
- `--compare` 多模型对比，自动推荐最适合的模型
- `--list-models` 彩色表格，基于当前系统实时颜色判定
- 配置文件扩展：`[directories]` 支持自定义模型缓存路径

## 📦 安装

### Linux (x86-64)
```bash
# 静态链接版（推荐，任何 Linux 都能跑）
curl -L -o hawk-eye-mem https://github.com/qiuhaomem/HawkEye-Mem/releases/download/v0.2.0/hawk-eye-mem-v0.2.0-linux-x86-64-musl
chmod +x hawk-eye-mem
./hawk-eye-mem --help

# glibc 版
curl -L -o hawk-eye-mem https://github.com/qiuhaomem/HawkEye-Mem/releases/download/v0.2.0/hawk-eye-mem-v0.2.0-linux-x86-64-glibc
chmod +x hawk-eye-mem
./hawk-eye-mem --help
```

### macOS (ARM64)
```bash
curl -L -o hawk-eye-mem https://github.com/qiuhaomem/HawkEye-Mem/releases/download/v0.2.0/hawk-eye-mem-v0.2.0-macos-arm64
chmod +x hawk-eye-mem
xattr -d com.apple.quarantine hawk-eye-mem  # macOS 需要
./hawk-eye-mem --help
```

### 从源码
```bash
git clone https://github.com/qiuhaomem/HawkEye-Mem.git
cd -HawkEye-Mem
cargo install --path .
hawk-eye-mem --can-run --model llama3-8b
```

## 📊 测试
- **124 个测试**全部通过（76 单元 + 20 集成 + 28 极限压力）
- CI 三平台全绿（Linux glibc + Linux musl + macOS ARM64）
- 极限测试覆盖：参数组合、超大数值、连续监控稳定性、JSON Schema 完整性

## 🙏 致谢
- DeepSeek-TUI · Reasonix · Hermes Agent
- 所有参与 V0.2 评审的产品经理、技术负责人、安全专家、法务、PMO

---

> **给 Agent 开个天眼。**
