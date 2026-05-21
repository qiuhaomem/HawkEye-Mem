#!/usr/bin/env python3
"""
秋毫mem MCP Server — 让 AI Agent 通过 MCP 协议直接感知系统资源。

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
    path = shutil.which(name)
    if path:
        return path
    # 常见备用路径
    candidates = [
        "~/.cargo/bin/hawk-eye-mem",
        "/usr/local/bin/hawk-eye-mem",
        "./target/release/hawk-eye-mem",
        "./target/debug/hawk-eye-mem",
        os.path.expanduser("~/projects/qiuhaomem/target/release/hawk-eye-mem"),
        os.path.expanduser("~/projects/qiuhaomem/target/debug/hawk-eye-mem"),
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


# ==================== MCP 协议实现 ====================

def handle_initialize(params: dict) -> dict:
    return {
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": "hawk-eye-mem",
            "version": "0.3.0"
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
            return {"content": [{"type": "text", "text": json.dumps(data)}], "isError": True}
        guidance = data.get("agent_guidance", data)
        return {"content": [{"type": "text", "text": json.dumps(guidance, indent=2)}]}

    elif name == "get_gpu_status":
        data = run_hawkeye(["--json"])
        if "error" in data:
            return {"content": [{"type": "text", "text": json.dumps(data)}], "isError": True}
        gpu_data = data.get("system", {}).get("gpu", [])
        if not gpu_data:
            # Fallback: try --gpu-list
            list_data = run_hawkeye(["--gpu-list"])
            return {"content": [{"type": "text", "text": json.dumps(
                {"gpu": gpu_data, "note": "No GPU detected on this system"} if not gpu_data else {"gpu": gpu_data},
                indent=2
            )}]}
        return {"content": [{"type": "text", "text": json.dumps({"gpu": gpu_data}, indent=2)}]}

    elif name == "get_thermal_status":
        data = run_hawkeye(["--json"])
        if "error" in data:
            return {"content": [{"type": "text", "text": json.dumps(data)}], "isError": True}
        thermal_data = data.get("system", {}).get("thermal", {})
        return {"content": [{"type": "text", "text": json.dumps(thermal_data, indent=2)}]}

    elif name == "get_agent_processes":
        data = run_hawkeye(["--json"])
        if "error" in data:
            return {"content": [{"type": "text", "text": json.dumps(data)}], "isError": True}
        agents_data = data.get("system", {}).get("agents", {})
        return {"content": [{"type": "text", "text": json.dumps(agents_data, indent=2)}]}

    elif name == "get_calibration_status":
        model_name = arguments.get("model_name", "")
        if not model_name:
            return {"content": [{"type": "text", "text": "Missing required argument: model_name"}], "isError": True}
        data = run_hawkeye(["--calibration-stats", "--model-name", model_name])
        if "error" in data:
            return {"content": [{"type": "text", "text": json.dumps(data)}], "isError": True}
        return {"content": [{"type": "text", "text": json.dumps(data, indent=2)}]}

    else:
        return {
            "content": [{"type": "text", "text": f"Unknown tool: {name}"}],
            "isError": True
        }


# ==================== 主循环：JSON-RPC over stdio ====================

def main():
    sys.stderr.write("HawkEye Mem MCP Server v0.3.0 started\n")
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
