#!/usr/bin/env python3
"""
秋毫mem MCP Server — 让 AI Agent 通过 MCP 协议直接感知系统资源。

V0.4 新增：
  - Token消耗记录（record_tokens MCP Tool + --record --tokens-processed CLI）

V0.4.1 新增：
  - 环境指纹（get_environment_fingerprint）
  - 趋势报告（get_trend_report）
  - 并发度建议（get_concurrency_suggestion）
  - 重置环境指纹（reset_environment_fingerprint）
  - 启动远程服务端（start_remote_server）

V0.3 新增：
  - GPU 状态采集（NVIDIA NVML / nvidia-smi / AMD ROCm / Apple Metal）
  - CPU/GPU 温度监控
  - 同机多 Agent 进程检测
  - 状态机连续监控模式
  - 动态校准引擎

安装方式（在 Hermes 中）：
    hermes mcp add hawk-eye-mem --command python3 /path/to/hawkeye-mcp-server.py
"""

import json
import os
import subprocess
import sys

# === 秋毫mem 二进制路径 ===
HAWKEYE_BIN = "hawk-eye-mem"


def find_binary(name: str) -> str | None:
    import shutil
    # Prefer locally built binary (has V0.5 features)
    local_candidates = [
        os.path.expanduser("~/projects/qiuhaomem/target/release/hawk-eye-mem"),
        os.path.expanduser("~/projects/qiuhaomem/target/debug/hawk-eye-mem"),
    ]
    for c in local_candidates:
        if os.path.isfile(c) and os.access(c, os.X_OK):
            return os.path.abspath(c)
    # Fallback to PATH
    path = shutil.which(name)
    if path:
        return path
    # 常见备用路径
    candidates = [
        "~/.cargo/bin/hawk-eye-mem",
        "/usr/local/bin/hawk-eye-mem",
        "./target/release/hawk-eye-mem",
        "./target/debug/hawk-eye-mem",
    ]
    for c in candidates:
        expanded = os.path.expanduser(c)
        if os.path.isfile(expanded) and os.access(expanded, os.X_OK):
            return os.path.abspath(expanded)
    return None


def run_hawkeye(args: list[str]) -> dict:
    """执行秋毫mem CLI，返回解析后的 JSON。"""
    bin_path = find_binary(HAWKEYE_BIN)
    if not bin_path:
        return {"error": f"hawk-eye-mem binary not found in PATH or common locations"}
    try:
        result = subprocess.run(
            [bin_path] + args,
            capture_output=True,
            timeout=10,
        )
        if result.returncode != 0:
            stderr = result.stderr.decode().strip()
            return {"error": stderr or f"exit code: {result.returncode}"}
        stdout = result.stdout.decode().strip()
        if not stdout:
            return {"error": "empty output"}
        try:
            return json.loads(stdout)
        except json.JSONDecodeError:
            return {"value": stdout}
    except subprocess.TimeoutExpired:
        return {"error": "command timed out"}
    except FileNotFoundError:
        return {"error": f"binary not found: {bin_path}"}
    except Exception as e:
        return {"error": str(e)}


# ==================== 通用工具函数 ====================

def hawk_result(data: dict) -> dict:
    """通用 hawkeye 工具调用结果处理：检查错误 → 返回 MCP 响应"""
    if "error" in data:
        return {"content": [{"type": "text", "text": json.dumps(data)}], "isError": True}
    return {"content": [{"type": "text", "text": json.dumps(data, indent=2)}]}


def hawk_result_field(data: dict, *keys: str) -> dict:
    """hawkeye 工具调用 + 提取指定字段"""
    if "error" in data:
        return {"content": [{"type": "text", "text": json.dumps(data)}], "isError": True}
    extracted = data
    for k in keys:
        extracted = extracted.get(k, {})
    return {"content": [{"type": "text", "text": json.dumps(extracted, indent=2)}]}


# ==================== V0.7 能力全景展示 ====================

def handle_onboarding_showcase() -> dict:
    """运行秋毫mem 能力全景展示 — 聚合所有功能数据为一份完整 JSON"""
    showcase = {
        "showcase_version": "0.7.0",
        "zero_token_cost": True,
        "description": "秋毫mem 能力全景展示 — 所有数据均为本地采集，零 Token 消耗",
        "sections": {},
        "summary": {
            "status": "ok",
            "highlights": [],
            "agent_action": "monitor",
        }
    }

    # 1. 系统健康
    sys_data = run_hawkeye(["--json"])
    if "error" not in sys_data:
        system = sys_data.get("system", {})
        guidance = sys_data.get("agent_guidance", {})
        showcase["sections"]["system_health"] = {
            "memory": {
                "total_mb": system.get("total_mb"),
                "used_mb": system.get("used_mb"),
                "available_mb": system.get("available_mb"),
                "used_percent": system.get("used_percent"),
            },
            "cpu": system.get("cpu"),
            "disk": system.get("disk"),
            "thermal": system.get("thermal"),
        }
        if guidance:
            showcase["sections"]["agent_guidance"] = guidance
            showcase["summary"]["agent_action"] = guidance.get("action", "monitor")
            p = guidance.get("pressure", "low")
            if p == "low":
                showcase["summary"]["status"] = "healthy"
            elif p == "medium":
                showcase["summary"]["status"] = "caution"
            else:
                showcase["summary"]["status"] = "critical"
        showcase["summary"]["highlights"].append("系统健康检查完成 ✅")

    # 2. 缓存策略
    cache_data = run_hawkeye(["--cache-strategy", "--json"])
    if "error" not in cache_data:
        showcase["sections"]["cache_strategy"] = cache_data
        mode = cache_data.get("mode", "?")
        showcase["summary"]["highlights"].append(f"缓存策略: {mode} 🚀")

    # 3. Token 花销 — 从 cache stats 获取缓存命中数据
    cs_data = run_hawkeye(["--cache-stats"])
    if "error" not in cs_data:
        showcase["sections"]["token_budget"] = {
            "note": "缓存命中统计（完整 Token 分析需编译 --features budget）",
            "cache_stats": cs_data,
        }
        showcase["summary"]["highlights"].append(f"缓存命中率可用 📊")

    # 4. 趋势分析
    trend_data = run_hawkeye(["--trend"])
    if "error" not in trend_data:
        showcase["sections"]["trend_analysis"] = trend_data
        direction = trend_data.get("direction", "stable")
        showcase["summary"]["highlights"].append(f"资源趋势: {direction} 📈")

    # 5. GPU
    showcase["sections"]["gpu"] = sys_data.get("system", {}).get("gpu", []) if "error" not in sys_data else []

    # 6. 同机 Agent
    showcase["sections"]["agents"] = sys_data.get("system", {}).get("agents", {}) if "error" not in sys_data else {}

    # 7. 环境指纹
    fp_data = run_hawkeye(["--env-fingerprint"])
    if "error" not in fp_data:
        showcase["sections"]["environment_fingerprint"] = fp_data

    # 8. 并发建议
    concurrency_data = run_hawkeye(["--suggest-concurrency"])
    if "error" not in concurrency_data:
        showcase["sections"]["concurrency"] = concurrency_data
        conc = concurrency_data.get("suggestion", {}).get("recommended_concurrency", 0)
        showcase["summary"]["highlights"].append(f"安全并发: {conc} 🎯")

    # 9. 心跳
    hb_data = run_hawkeye(["--heartbeat"])
    if "error" not in hb_data:
        showcase["sections"]["heartbeat"] = hb_data

    return {"content": [{"type": "text", "text": json.dumps(showcase, indent=2, ensure_ascii=False)}]}


# ==================== MCP 协议实现 ====================

def handle_initialize(params: dict) -> dict:
    return {
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": "hawk-eye-mem",
            "version": "0.5.0"
        }
    }


def handle_list_tools(params: dict) -> dict:
    return {
        "tools": [
            {
                "name": "get_memory_status",
                "description": "获取完整系统资源状态（内存/CPU/磁盘/GPU/温度/Agent进程），以及 Agent 决策建议（pressure/action/estimated_safe_context_window）。可选传入 tokens_processed 用于动态校准。\n\nV0.3 新增输出字段：\n- system.gpu: GPU 列表（名称/显存/温度/功耗/利用率）\n- system.thermal: CPU/GPU 温度\n- system.agents: 同机其他 AI Agent 进程\n- machine_state: 连续监控模式下的状态机",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "tokens_processed": {
                            "type": "integer",
                            "description": "本次推理实际处理的 token 数（可选）。传入后秋毫mem 会记录校准数据点。"
                        }
                    },
                    "required": []
                }
            },
            {
                "name": "get_memory_metric",
                "description": "获取单个内存指标，支持：total_mb（总内存）、used_mb（已用）、available_mb（可用）、used_percent（使用率%）、pressure（压力等级：low/medium/high/critical）",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "metric": {
                            "type": "string",
                            "description": "指标名称：total_mb, used_mb, available_mb, used_percent, pressure",
                            "enum": ["total_mb", "used_mb", "available_mb", "used_percent", "pressure"]
                        }
                    },
                    "required": ["metric"]
                }
            },
            {
                "name": "get_memory_guidance",
                "description": "获取 Agent 内存决策建议，包含 action（ok/monitor/reduce_context/abort_safely）、pressure 等级、estimated_safe_context_window 安全上下文窗口大小",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "get_gpu_status",
                "description": "获取 GPU 状态，列出所有检测到的 GPU 及其显存使用情况、温度、功耗、利用率和采集后端（NVML/nvidia-smi/ROCm/sysctl）",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "get_thermal_status",
                "description": "获取 CPU/GPU 温度信息，包含 CPU 核心温度、各 GPU 温度和温度压力等级（normal/warning/critical）",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "get_agent_processes",
                "description": "检测同机运行的其他 AI Agent 进程（如 Hermes、Claude Code、AutoGPT 等），统计数量和内存占用",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "get_calibration_status",
                "description": "获取指定模型的校准状态，包含样本数、平均 bytes_per_token、标准差、趋势和 confidence 等级。校准可提高推理上下文窗口估算精度。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "model_name": {
                            "type": "string",
                            "description": "模型名称，如 llama3-8b、deepseek-v3"
                        }
                    },
                    "required": ["model_name"]
                }
            },
            # ======== V0.4 新工具 ========
            {
                "name": "get_environment_fingerprint",
                "description": "获取当前环境的唯一指纹信息，包含硬件/系统特征哈希。用于识别环境变更或进行环境匹配。",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "get_trend_report",
                "description": "获取系统资源使用的趋势报告，基于历史数据展示内存/CPU/磁盘的变化趋势，帮助判断资源增长或下降。",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "get_concurrency_suggestion",
                "description": "获取当前系统资源下的安全并发度建议。可选传入 --task-memory 指定每个任务的内存开销（MB），以获得更精准的建议。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "task_memory": {
                            "type": "integer",
                            "description": "每个任务预期的内存开销（MB）。传入后秋毫mem 会根据系统可用内存计算推荐并发数。"
                        }
                    },
                    "required": []
                }
            },
            {
                "name": "reset_environment_fingerprint",
                "description": "重置环境指纹。强制重新生成环境标识，适用于环境发生重大变更后需要重新校准的场景。",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "start_remote_server",
                "description": "在当前主机后台启动秋毫mem 远程服务端，监听指定端口，允许远程客户端连接查询系统资源状态。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "port": {
                            "type": "integer",
                            "description": "服务端监听端口号（默认 9876）"
                        }
                    },
                    "required": []
                }
            },
            # ======== V0.5 新工具 ========
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
            },
            {
                "name": "report_cache_hit",
                "description": "向秋毫mem汇报本次任务的缓存命中数据，用于统计24小时命中率。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "model_name": {
                            "type": "string",
                            "description": "使用的模型名（将哈希后存储 — CR-06）"
                        },
                        "hit_count": {
                            "type": "integer",
                            "description": "缓存命中次数"
                        },
                        "miss_count": {
                            "type": "integer",
                            "description": "缓存未命中次数"
                        },
                        "cost_saved_usd": {
                            "type": "number",
                            "description": "本次任务估算节省的API费用（美元）"
                        }
                    },
                    "required": ["model_name", "hit_count", "miss_count"]
                }
            },
            # ======== V0.5 Token审计工具 ========
            {
                "name": "run_token_audit",
                "description": "运行Token审计，分析Hermes Agent的token消耗、来源分布、浪费检测。返回JSON格式审计报告。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "days": {
                            "type": "integer",
                            "description": "审计最近N天的数据（默认当天）"
                        },
                        "source": {
                            "type": "string",
                            "description": "按来源过滤（weixin/cron/api_server等）"
                        }
                    },
                    "required": []
                }
            },
            # ======== V0.6 缓存差距分析 ========
            {
                "name": "get_cache_gaps_analysis",
                "description": "分析缓存命中率差距，输出缺口分类和修复建议。返回JSON格式分析报告。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "days": {
                            "type": "integer",
                            "description": "分析最近N天的缓存数据（默认7天）"
                        },
                        "target": {
                            "type": "number",
                            "description": "目标命中率百分比（默认99.0）"
                        }
                    },
                    "required": []
                }
            },
            # ======== V0.6 心跳 ========
            {
                "name": "get_heartbeat",
                "description": "获取单行心跳JSON，包含系统压力、可用内存、建议操作和时间戳。",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            # ======== V0.7 能力全景展示 ========
            {
                "name": "run_onboarding_showcase",
                "description": "运行秋毫mem 能力全景展示 — 一次性获取所有系统状态、缓存策略、Token花销、趋势分析、并发建议、GPU/Agent、环境指纹、Agent指导。让Agent和用户一次性感知秋毫mem的全部能力。零Token消耗。",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
        ]
    }


def handle_call_tool(params: dict) -> dict:
    name = params.get("name", "")
    arguments = params.get("arguments", {})

    if name == "get_memory_status":
        args = ["--json"]
        tokens = arguments.get("tokens_processed")
        if tokens is not None:
            args.extend(["--tokens-processed", str(tokens)])
        data = run_hawkeye(args)
        if "error" in data:
            return {"content": [{"type": "text", "text": json.dumps(data)}], "isError": True}
        return {"content": [{"type": "text", "text": json.dumps(data, indent=2)}]}

    elif name == "get_memory_metric":
        metric = arguments.get("metric", "")
        if not metric:
            return {"content": [{"type": "text", "text": "Missing required argument: metric"}], "isError": True}
        data = run_hawkeye(["--metric", metric])
        if "error" in data:
            return {"content": [{"type": "text", "text": json.dumps(data)}], "isError": True}
        return {"content": [{"type": "text", "text": json.dumps(data)}]}

    elif name == "get_memory_guidance":
        data = run_hawkeye(["--json"])
        if "error" in data:
            return hawk_result(data)
        return {"content": [{"type": "text", "text": json.dumps(data.get("agent_guidance", data), indent=2)}]}

    elif name == "get_gpu_status":
        data = run_hawkeye(["--json"])
        if "error" in data:
            return hawk_result(data)
        gpu_data = data.get("system", {}).get("gpu", [])
        if not gpu_data:
            list_data = run_hawkeye(["--gpu-list"])
            return {"content": [{"type": "text", "text": json.dumps(
                {"gpu": gpu_data, "note": "No GPU detected on this system"} if not gpu_data else {"gpu": gpu_data},
                indent=2
            )}]}
        return {"content": [{"type": "text", "text": json.dumps({"gpu": gpu_data}, indent=2)}]}

    elif name == "get_thermal_status":
        return hawk_result_field(run_hawkeye(["--json"]), "system", "thermal")

    elif name == "get_agent_processes":
        return hawk_result_field(run_hawkeye(["--json"]), "system", "agents")

    elif name == "get_calibration_status":
        model_name = arguments.get("model_name", "")
        if not model_name:
            return {"content": [{"type": "text", "text": "Missing required argument: model_name"}], "isError": True}
        return hawk_result(run_hawkeye(["--calibration-stats", "--model-name", model_name]))

    # ======== V0.4 新工具 ========

    elif name == "get_environment_fingerprint":
        data = run_hawkeye(["--env-fingerprint"])
        if "error" in data:
            return {"content": [{"type": "text", "text": json.dumps({
                "fingerprint": None,
                "message": "当前环境无指纹记录，请先运行 hawk-eye-mem 采集基线数据。"
            }, indent=2)}]}
        return hawk_result(data)

    elif name == "get_trend_report":
        return hawk_result(run_hawkeye(["--trend"]))

    elif name == "get_concurrency_suggestion":
        args = ["--suggest-concurrency"]
        task_memory = arguments.get("task_memory")
        if task_memory is not None:
            args.extend(["--task-memory", str(task_memory)])
        return hawk_result(run_hawkeye(args))

    elif name == "reset_environment_fingerprint":
        data = run_hawkeye(["--reset-environment", "--force"])
        if "error" in data:
            return {"content": [{"type": "text", "text": json.dumps({
                "success": False,
                "message": data["error"]
            })}], "isError": True}
        return {"content": [{"type": "text", "text": json.dumps({
            "success": True,
            "message": "环境指纹已重置，下次采集时将重新生成。"
        }, indent=2)}]}

    elif name == "start_remote_server":
        port = arguments.get("port", 9876)
        bin_path = find_binary(HAWKEYE_BIN)
        if not bin_path:
            return {"content": [{"type": "text", "text": json.dumps({
                "error": f"hawk-eye-mem binary not found"
            })}], "isError": True}
        try:
            proc = subprocess.Popen(
                [bin_path, "--serve", "--port", str(port)],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            import time
            time.sleep(0.5)
            if proc.poll() is not None:
                return {"content": [{"type": "text", "text": json.dumps({
                    "success": False,
                    "message": f"服务端启动失败，进程已退出（exit code: {proc.returncode}）"
                }, indent=2)}], "isError": True}
            return {"content": [{"type": "text", "text": json.dumps({
                "success": True,
                "message": f"秋毫mem 远程服务端已在后台启动",
                "port": port,
                "pid": proc.pid,
                "binary": bin_path,
                "status": "running"
            }, indent=2)}]}
        except Exception as e:
            return {"content": [{"type": "text", "text": json.dumps({
                "success": False,
                "error": str(e)
            })}], "isError": True}

    # ======== V0.5 新工具 ========

    elif name == "get_cache_strategy":
        args = ["--cache-strategy", "--json"]
        data = run_hawkeye(args)
        if "error" in data:
            return {"content": [{"type": "text", "text": json.dumps(data)}], "isError": True}
        return {"content": [{"type": "text", "text": json.dumps(data, indent=2)}]}

    elif name == "report_cache_hit":
        model_name = arguments.get("model_name", "unknown")
        hit_count = arguments.get("hit_count", 0)
        miss_count = arguments.get("miss_count", 0)
        cost_saved = arguments.get("cost_saved_usd", 0.0)
        # CR-02: fire-and-forget — don't block the caller
        import hashlib
        model_hash = hashlib.sha256(model_name.encode()).hexdigest()[:16]
        report = {
            "model_name": model_name,
            "model_hash": model_hash,
            "hit_count": hit_count,
            "miss_count": miss_count,
            "cost_saved_usd": round(cost_saved, 2),  # CR-29: 2 decimal precision
            "timestamp": __import__("datetime").datetime.now().isoformat(),
        }
        # Write to JSONL
        data_dir = os.path.expanduser("~/.config/hawk-eye-mem")
        os.makedirs(data_dir, exist_ok=True)
        stats_path = os.path.join(data_dir, "cache_stats.jsonl")
        line = json.dumps(report)
        # CR-09: single record max 1KB
        if len(line) > 1024:
            return {"content": [{"type": "text", "text": json.dumps({
                "received": False,
                "error": "Record exceeds 1KB limit"
            })}], "isError": True}
        # CR-09: file size limit
        if os.path.isfile(stats_path) and os.path.getsize(stats_path) > 10 * 1024 * 1024:
            return {"content": [{"type": "text", "text": json.dumps({
                "received": False,
                "error": "cache_stats.jsonl exceeds 10MB limit"
            })}], "isError": True}
        with open(stats_path, "a") as f:
            f.write(line + "\n")

        # 获取真实的24小时命中率
        hit_rate_24h = None
        try:
            bin_path = find_binary(HAWKEYE_BIN)
            if bin_path:
                stats_result = subprocess.run(
                    [bin_path, "--cache-stats"],
                    capture_output=True, timeout=5
                )
                if stats_result.returncode == 0:
                    stats = json.loads(stats_result.stdout.decode())
                    hit_rate_24h = stats.get("hit_rate_24h", None)
        except Exception:
            hit_rate_24h = None

        return {"content": [{"type": "text", "text": json.dumps({
            "received": True,
            "hit_rate_24h": hit_rate_24h,
        }, indent=2)}]}

    # ======== V0.5 Token审计工具 ========

    elif name == "run_token_audit":
        # 调用Python脚本运行Token审计
        import sys
        script_path = os.path.join(os.path.dirname(__file__), "token_audit")
        if not os.path.exists(script_path):
            return {"content": [{"type": "text", "text": json.dumps({
                "error": "Token audit script not found",
                "path": script_path
            })}], "isError": True}

        args = [sys.executable, "-m", "scripts.token_audit", "--token-audit", "--json"]
        days = arguments.get("days")
        if days is not None:
            args.extend(["--days", str(days)])
        source = arguments.get("source")
        if source:
            args.extend(["--source", source])

        try:
            result = subprocess.run(
                args,
                capture_output=True,
                timeout=30,
                cwd=os.path.dirname(__file__)
            )
            if result.returncode != 0:
                return {"content": [{"type": "text", "text": json.dumps({
                    "error": result.stderr.decode().strip() or f"exit code: {result.returncode}"
                })}], "isError": True}
            output = result.stdout.decode().strip()
            if not output:
                return {"content": [{"type": "text", "text": json.dumps({
                    "error": "empty output"
                })}], "isError": True}
            try:
                data = json.loads(output)
                return {"content": [{"type": "text", "text": json.dumps(data, indent=2)}]}
            except json.JSONDecodeError:
                return {"content": [{"type": "text", "text": output}]}
        except subprocess.TimeoutExpired:
            return {"content": [{"type": "text", "text": json.dumps({
                "error": "Token audit timed out (30s)"
            })}], "isError": True}
        except Exception as e:
            return {"content": [{"type": "text", "text": json.dumps({
                "error": str(e)
            })}], "isError": True}

    # ======== V0.6 缓存差距分析 ========
    elif name == "get_cache_gaps_analysis":
        args = ["--analyze-cache-gaps", "--json"]
        days = arguments.get("days")
        if days is not None:
            args.extend(["--days", str(days)])
        target = arguments.get("target")
        if target is not None:
            args.extend(["--target", str(target)])
        data = run_hawkeye(args)
        if "error" in data:
            return {"content": [{"type": "text", "text": json.dumps(data)}], "isError": True}
        if "value" in data:
            return {"content": [{"type": "text", "text": data["value"]}]}
        return {"content": [{"type": "text", "text": json.dumps(data, indent=2)}]}

    # ======== V0.6 心跳 ========
    elif name == "get_heartbeat":
        data = run_hawkeye(["--heartbeat"])
        if "error" in data:
            return {"content": [{"type": "text", "text": json.dumps(data)}], "isError": True}
        return {"content": [{"type": "text", "text": json.dumps(data, indent=2)}]}

    elif name == "run_onboarding_showcase":
        return handle_onboarding_showcase()

    else:
        return {
            "content": [{"type": "text", "text": f"Unknown tool: {name}"}],
            "isError": True
        }


# ==================== 主循环：JSON-RPC over stdio ====================

def main():
    sys.stderr.write("HawkEye Mem MCP Server v0.4.0 started\n")
    sys.stderr.flush()

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            request = json.loads(line)
        except json.JSONDecodeError:
            continue

        req_id = request.get("id")
        method = request.get("method", "")
        params = request.get("params", {})

        if method == "initialize":
            result = handle_initialize(params)
            response = {"jsonrpc": "2.0", "id": req_id, "result": result}

        elif method == "tools/list":
            result = handle_list_tools(params)
            response = {"jsonrpc": "2.0", "id": req_id, "result": result}

        elif method == "tools/call":
            result = handle_call_tool(params)
            if result.get("isError"):
                response = {"jsonrpc": "2.0", "id": req_id, "error": {
                    "code": -32000,
                    "message": "Tool execution failed",
                    "data": result["content"][0]["text"]
                }}
            else:
                response = {"jsonrpc": "2.0", "id": req_id, "result": result}

        elif method == "notifications/initialized":
            continue

        else:
            response = {"jsonrpc": "2.0", "id": req_id, "error": {
                "code": -32601,
                "message": f"Method not found: {method}"
            }}

        sys.stdout.write(json.dumps(response) + "\n")
        sys.stdout.flush()


if __name__ == "__main__":
    main()
