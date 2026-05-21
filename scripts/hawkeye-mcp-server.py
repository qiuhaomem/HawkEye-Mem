#!/usr/bin/env python3
"""
秋毫mem MCP Server — 让 AI Agent 通过 MCP 协议直接感知物理内存。

安装方式（在 Hermes 中）：
    hermes mcp add hawk-eye-mem --command python3 /path/to/hawkeye-mcp-server.py

然后 Hermes 会自动发现以下工具，Agent 可直接调用。
"""

import json
import os
import subprocess
import sys

# === 秋毫mem 二进制路径 ===
# 优先从 PATH 查找，找不到则尝试常见安装位置
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
        # 项目目录
        os.path.expanduser("~/projects/qiuhaomem/target/release/hawk-eye-mem"),
    ]
    for c in candidates:
        expanded = c.replace("~", str(subprocess.check_output(["echo", "~"]).decode().strip()))
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
        # 尝试解析 JSON
        try:
            return json.loads(stdout)
        except json.JSONDecodeError:
            # 可能是纯文本输出（如 --metric）
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
            "version": "0.1.0"
        }
    }


def handle_list_tools(params: dict) -> dict:
    return {
        "tools": [
            {
                "name": "get_memory_status",
                "description": "获取完整系统内存状态，包含总内存、已用、可用、使用率，以及 Agent 决策建议（pressure/action/estimated_safe_context_window）。可选传入 tokens_processed 用于动态校准数据采集。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "tokens_processed": {
                            "type": "integer",
                            "description": "本次推理实际处理的 token 数（可选）。传入后秋毫mem 会记录校准数据点，用于动态估算参数修正。",
                            "required": false
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

    else:
        return {
            "content": [{"type": "text", "text": f"Unknown tool: {name}"}],
            "isError": True
        }


# ==================== 主循环：JSON-RPC over stdio ====================

def main():
    """MCP Server 主循环：读取 stdin 的 JSON-RPC 请求，处理并返回。"""
    # 先输出服务器信息到 stderr（不干扰 stdio 协议）
    sys.stderr.write("HawkEye Mem MCP Server started\n")
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

        # 处理请求
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
            # 忽略初始化通知
            continue

        else:
            response = {"jsonrpc": "2.0", "id": req_id, "error": {
                "code": -32601,
                "message": f"Method not found: {method}"
            }}

        # 输出响应到 stdout（MCP 协议）
        sys.stdout.write(json.dumps(response) + "\n")
        sys.stdout.flush()


if __name__ == "__main__":
    main()
