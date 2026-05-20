[English](./README-en.md) | [中文](./README.md)

# 秋毫mem (HawkEye Mem)

**你的电脑跑着 AI 程序，跑了三个小时，你走开倒了杯水，回来一看——崩了。内存挤爆了。**

不是你的错，是那个 AI 程序自己不知道"快没内存了"。它闷头算，算到内存炸了，连个存档都没给你留。

你想提前看一眼还剩多少内存？可以，敲 `free -h`，出来一坨字符。你能看懂，AI 程序看不懂。它不是人，它不知道什么叫"危险"。

**秋毫mem 就是干这个的。**

它不是给你看的，是给你的 AI 程序看的。在内存快炸之前，它扔给 AI 程序一句话：

> "撑不住了，缩小上下文，赶紧存档。"

AI 程序拿到这句话，就知道自己该怎么做了。

就这么简单。

---

## 和 `free`、`htop` 的区别，一句话说清楚

那些工具是给人看的仪表盘。秋毫mem 是给 AI 程序装的传感器。

你去看仪表盘，然后手动调。秋毫mem 是让 AI 程序自己感知，自己调。

---

## 看一眼输出你就懂了

```json
{
  "agent_guidance": {
    "action": "reduce_context",
    "estimated_safe_context_window": 4096,
    "reason": "临界：内存不足，请立即中止以避免 OOM。"
  }
}
```

AI 程序拿到这个，不用理解"内存"是什么，不用算百分比，它只需要看 `action` 那个字段：`reduce_context`，然后照做。

---

## 什么时候用它

- 你在自己电脑上跑本地大模型
- 你让 AI 程序长时间干活，不能一直盯着
- 程序崩了几次你受不了了
- 你想让 AI 程序自己学会"看内存脸色"

装上，配好，它就在后台帮你盯着。AI 程序每次干活之前问它一句"还有内存吗"，它告诉你还能不能干、能干多大。

---

## 安装

**从源码编译**

```bash
git clone https://github.com/qiuhaomem/-HawkEye-Mem.git
cd -HawkEye-Mem
cargo build --release
sudo cp target/release/hawk-eye-mem /usr/local/bin/
```

需要先装 Rust：`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`

**直接下载二进制**

从 [GitHub Releases](https://github.com/qiuhaomem/-HawkEye-Mem/releases) 下载预编译的二进制文件：

```bash
# Linux (musl 静态链接，任何发行版都能跑)
curl -L -o hawk-eye-mem https://github.com/qiuhaomem/-HawkEye-Mem/releases/download/v0.2.0/hawk-eye-mem-v0.2.0-linux-x86-64-musl
chmod +x hawk-eye-mem
./hawk-eye-mem --help
```

---

## V0.2 新增：部署前检测

**花几小时下载一个模型，加载时崩了，这是最难受的。**
秋毫mem V0.2 新增了 `--can-run` 命令——在你下载模型之前，先评估你的机器能不能跑得动。

```bash
# 看你的机器能不能跑 Llama-3-8B
hawk-eye-mem --can-run --model llama3-8b

# JSON 输出给 AI Agent 用
hawk-eye-mem --can-run --model llama3-8b --json

# 看看几个模型里哪个最适合你的机器
hawk-eye-mem --can-run --compare llama3-8b,qwen2-7b,phi-3-mini

# 手动指定模型参数
hawk-eye-mem --can-run --model-size 7000000000 --quantization Q4_K_M --context 4096
```

`--can-run` 会告诉你三种结果：
- ✅ **可行** — 你的机器完全能跑，放心下载
- ⚠️ **有条件** — 差一点，但可以降量化/减上下文/换小模型
- ❌ **不可行** — 磁盘空间或内存差距太大

同时内置了 8 个主流模型的参数库（llama3-8b、qwen2-7b、deepseek-v2-lite 等），`--list-models` 查看完整列表：

```bash
hawk-eye-mem --list-models
```

此外，V0.2 还新增了**磁盘**和**CPU**监控。`hawk-eye-mem --json` 的输出现在包含 `system.cpu` 和 `system.disk`（如果有模型缓存目录的话）。

---

## 怎么用

```bash
# 给 AI 程序看（完整内存报告）
hawk-eye-mem --json

# 只看还剩多少内存
hawk-eye-mem --metric available_mb

# 看内存压力等级
hawk-eye-mem --metric pressure

# 每 5 秒查一次，采 10 次
hawk-eye-mem --json --interval 5 --count 10

# 一直盯着，按 Ctrl+C 停
hawk-eye-mem --json --interval 5 --count 0

# 排查部署前能否运行某个模型（V0.2 新增）
hawk-eye-mem --can-run --model llama3-8b

# 对比哪个模型最适合你的电脑
hawk-eye-mem --can-run --compare llama3-8b,qwen2-7b,phi-3-mini

# 列出内置支持的模型
hawk-eye-mem --list-models
```

---

## 怎么让你的 AI 程序用上它

**如果你用 Hermes**，注册成 MCP 工具就行：

```bash
hermes mcp add hawk-eye-mem --command python3 --args scripts/hawkeye-mcp-server.py
```

注册后 AI 程序就能直接调这三个工具了：

| 工具名 | 干嘛的 |
|--------|--------|
| `get_memory_status` | 看完整内存状态 + 建议 |
| `get_memory_metric` | 看单个指标（总内存、已用、可用、使用率、压力） |
| `get_memory_guidance` | 只看建议（该不该缩、安不安全） |

**如果你用别的框架**：直接 `hawk-eye-mem --json`，拿到 JSON 输出，读 `agent_guidance.action` 字段，照做就行。

---

## 给估算调准一点（可选）

秋毫mem 默认的估算是保守的，它宁可多留一些内存余量，也不会让你崩掉。如果你想让估算更精确，可以告诉它你用的是什么模型：

```bash
hawk-eye-mem --init-config
```

然后编辑 `~/.config/hawk-eye-mem/config.toml`，填上你的模型参数。

不配置也没关系，默认的保守估计足够安全。

---

## 压力水位是什么意思

| 水位 | 意思是 | AI 程序该怎么做 |
|------|--------|----------------|
| `low` | 内存充裕 | 放心跑 |
| `medium` | 还行，但要注意了 | 接着跑，勤问着点 |
| `high` | 不多了 | 缩小上下文，省着用 |
| `critical` | 马上炸了 | 赶紧存档，别再跑了 |

---

## 性能

不会拖慢你的系统。查一次不到 1 毫秒，二进制不到 1MB。

---

## 反馈

V0.2 是一个早期版本，我们正在收集反馈。如果你试用了 `--can-run`，请告诉我们：检测准不准？有没有帮到你？

👉 [反馈 Issue](https://github.com/qiuhaomem/-HawkEye-Mem/issues/1)

你的每一条反馈都会直接影响 V0.3 的功能规划。

---

## 注意

秋毫mem 给的建议是基于估算的，**不一定百分之百准确**。用它做的决策，风险你自己担着。

详细说明看 [DISCLAIMER.md](./DISCLAIMER.md)。

---

## 许可证

[Apache-2.0](./LICENSE)

"秋毫mem"和"HawkEye Mem"是项目商标。
