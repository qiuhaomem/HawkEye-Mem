# 秋毫mem V0.5 · 缓存策略模型兼容性 专项调研与整改

**调研日期**：2026-05-26
**调研人**：产品经理
**评审对象**：测试用例文档 V1.0（98 个用例）
**结论性质**：产品层面通过，需增加 8 条用户体验测试用例，确保缓存策略的模型兼容性信息透明、可感知、不误导。

---

### 一、Prompt Caching 支持现状

| Provider | 是否支持 Prompt Caching | 支持模型 | 不支持模型 |
|----------|------------------------|---------|-----------|
| **DeepSeek** | ✅ 支持 | V2/V3/V4 全系列，缓存命中价低至 1/10 |
| **Anthropic** | ✅ 支持 | Claude 4 Opus/Sonnet、Claude 3.7/3.5 Sonnet、Claude 3.5 Haiku、Claude 3 Haiku/Opus |
| **OpenAI** | ✅ 支持 | GPT-4o、GPT-4o-mini、o1-preview、o1-mini | 旧版模型（GPT-4-turbo 等） |
| **Google Gemini** | ✅ 支持 | Gemini 1.5 Pro、Gemini 1.5 Flash、Gemini 2.5 系列 | — |
| **Groq** | ⚠️ 部分支持 | 仅 Kimi K2、GPT-OSS 20B | 其他模型尚不支持 |
| **NVIDIA NIM** | ⚠️ 部分支持 | 部分模型支持 KV Cache Reuse | — |
| **阿里通义千问** | ✅ 支持 | qwen-plus、qwen-max |
| **本地推理引擎** | ✅ 支持 | vLLM、SGLang、TensorRT-LLM、LMDeploy 等 | — |

**重要发现**：
- Groq 目前**仅支持 Kimi K2 和 GPT-OSS 20B** 两个模型的 Prompt Caching，且官方提示"缓存不保证命中"
- OpenAI 的缓存是**自动生效的**，但只对 **GPT-4o 系列和 o1 系列** 生效
- Anthropic 需要**显式设置 `cache_control` breakpoints**，并非自动启用
- 本地推理引擎中，**多模态模型暂不支持 prefix cache**
- 华为云 ModelArts：**Qwen2.5 和 Qwen3 系列支持** prefix caching

---

### 二、用户风险场景

| 场景 | 用户行为 | 实际结果 | 用户感受 |
|------|---------|---------|---------|
| **不支持的模型** | 在 Groq 上用 Qwen3-32B，期望 99% 命中率 | Groq 不支持该模型的缓存，命中率 0% | "这工具骗人的" |
| **旧版模型** | 在 OpenAI 上用 GPT-4-turbo | 该模型不支持缓存 | "怎么没用" |
| **本地多模态模型** | 本地跑 LLaVA/Qwen-VL | vLLM 暂不支持多模态 prefix cache | "明明配了策略" |
| **需要显式配置的模型** | 用 Claude 但没设置 `cache_control` | 缓存未生效 | "策略没起作用" |

---

### 三、V0.5 整改方案

#### 3.1 秋毫mem端：新增模型缓存兼容性检测

在 `cache_strategy` 返回中增加两个字段：

```json
{
  "cache_strategy": {
    "mode": "aggressive",
    "ttl_seconds": 600,
    "max_cache_mb": 4096,
    "prefetch_enabled": true,
    "reason": "内存充裕（可用4.2GB），启用激进缓存模式，预计命中率99%+",

    "model_compatibility": {
      "model_name": "qwen3-32b",
      "provider": "groq",
      "prefix_caching_supported": false,
      "supported_by_provider": false,
      "warning": "当前模型在 Groq 上不支持 Prompt Caching。建议：切换至 DeepSeek V4 或 Anthropic Claude。"
    },

    "provider_cache_info": {
      "name": "groq",
      "prefix_caching_available": true,
      "automatic": true,
      "supported_models": ["moonshotai/kimi-k2-instruct-0905", "openai/gpt-oss-20b"],
      "notes": "缓存不保证命中（因内部路由策略）"
    }
  }
}
```

#### 3.2 Skill端：增加模型兼容性暴露点

新增**第六个暴露点——模型兼容性警告**：

```
[hermes-cache-strategy] 秋毫mem检测到你当前使用的模型（Qwen3-32B@Groq）不支持Prompt Caching
[hermes-cache-strategy] 缓存命中率将从99%+降至约0%。建议切换至DeepSeek V4或Anthropic Claude。
[hermes-cache-strategy] 输入 hawk-eye-mem --model-compat 查看所有模型的缓存支持情况。
```

#### 3.3 CLI新增参数

| 参数 | 功能 | 示例 |
|------|------|------|
| `--model-compat` | 查看指定模型的缓存兼容性 | `hawk-eye-mem --model-compat qwen3-32b@groq` |
| `--model-compat --provider groq` | 查看某Provider的所有模型缓存支持 | `hawk-eye-mem --model-compat --provider groq` |
| `--model-compat --list` | 列出所有已知Provider的缓存支持情况 | `hawk-eye-mem --model-compat --list` |

#### 3.4 成本报告中增加模型兼容性提示

在成本报告底部水印区增加一行（仅当模型不支持缓存时显示）：

```
║ ⚠️  当前模型不支持Prompt Caching，命中率受限          ║
║ 💡 切换至DeepSeek V4或Anthropic Claude可提升至99%+     ║
```

#### 3.5 预置Provider-模型缓存兼容矩阵

在 Skill 中预置一份 JSON 数据文件 `provider_cache_compat.json`，覆盖 8+ Provider。

---

### 四、测试用例增补

| 新增ID | 测试目标 | 前置条件 | 预期结果 |
|--------|----------|----------|----------|
| **UT-SKILL-065** | 不支持缓存模型→暴露点触发 | 在 Groq 上使用 Qwen3-32B | 输出模型兼容性警告文案 |
| **UT-SKILL-066** | 不支持缓存模型→建议切换Provider | 同上 | 文案含"建议切换至DeepSeek V4或Anthropic Claude" |
| **UT-SKILL-067** | 支持缓存模型→不触发警告 | 在 DeepSeek 上使用 DeepSeek V4 | 无模型兼容性警告 |
| **UT-SKILL-068** | 成本报告底部显示模型警告 | 不支持缓存的模型 | 水印区出现"⚠️ 当前模型不支持Prompt Caching" |
| **UT-SKILL-069** | 成本报告不显示模型警告 | 支持缓存的模型 | 水印区无模型警告行 |
| **IT-CLI-060** | `--model-compat qwen3-32b@groq` | CLI 查询 | 返回 JSON，含 `supported: false` 和 `warning` |
| **IT-CLI-061** | `--model-compat --provider groq` | CLI 查询 | 返回 Groq 支持的所有缓存模型列表 |
| **IT-CLI-062** | `--model-compat --list` | CLI 查询 | 返回所有 Provider 的缓存支持矩阵 |

---

### 五、需求增补：模型缓存兼容性标识

**增补编号**：REQ-013-SUP-002
**增补事由**：创始人提出"缓存策略不是对所有模型都有效，得标识清楚"

| 一级 | 二级 | 三级 | 四级 | 五级 |
|------|------|------|------|------|
| **模型兼容性** | **内置兼容矩阵** | **Provider-模型缓存支持库** | Skill 预置 `provider_cache_compat.json`，覆盖 8+ Provider | 数据来源：官方文档 + 社区实测；随 Skill 版本更新 |
| | | **自动模型检测** | Skill 启动时从 Hermes 配置获取当前模型和 Provider | 自动匹配兼容矩阵，判断是否支持 Prefix Caching |
| | **用户告知** | **不支持时主动警告** | 检测到不支持时输出警告和建议 | 一行为警告，一行为建议，一行引导使用 `--model-compat` |
| | | **支持时静默** | 支持缓存的模型不额外输出 | 不刷屏、不打扰 |
| | | **成本报告底部提示** | 报告水印区增加模型兼容性状态 | 仅不支持时显示 |
| | **CLI查询工具** | **`--model-compat`** | 查询指定模型/Provider 的缓存兼容性 | 三种模式：指定模型、指定 Provider、列出全部 |

---

### 📊 产品经理裁决与决议

| 编号 | 来源 | 整改项 | 类型 | 责任人 |
|:----:|:----:|--------|:----:|:------:|
| CR-16 | 创始人 | `cache_strategy` 增加 `model_compatibility` 和 `provider_cache_info` 字段 | 🔴 必须 | 技术负责人 |
| CR-17 | 创始人 | Skill 增加第六个暴露点：模型兼容性警告 | 🔴 必须 | 产品经理+技术 |
| CR-18 | 创始人 | 成本报告底部增加模型兼容性状态行 | 🔴 必须 | 产品经理 |
| CR-19 | 创始人 | 预置 `provider_cache_compat.json` 兼容矩阵 | 🔴 必须 | 技术负责人 |
| CR-20 | 创始人 | 新增 CLI 参数 `--model-compat` | 🟡 建议 | 技术负责人 |
| CR-21 | 创始人 | 增加 8 条模型兼容性测试用例 | 🔴 必须 | 测试经理 |

### ✅ 评审结论

**产品经理最终裁定**：V0.5 测试用例文档通过，但必须将模型兼容性标识纳入 V0.5 的 P0 需求。缓存策略确实是好东西，但必须像食品包装上的"成分表"和"适用人群"一样，让用户清楚知道这东西对他的场景是否有效。这不是削弱卖点，而是**增强信任**。
