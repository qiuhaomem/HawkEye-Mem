#!/usr/bin/env bash
#
# 秋毫mem MCP 一键安装脚本
# 给同事的 MacBook / 其他机器用的
#
# 用法:
#   curl -fsSL https://raw.githubusercontent.com/qiuhaomem/HawkEye-Mem/main/scripts/install-hawkeye-mcp.sh | bash
#   或直接在项目里: bash scripts/install-hawkeye-mcp.sh
#

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

echo -e "${CYAN}═══════════════════════════════════${NC}"
echo -e "${CYAN}  秋毫mem MCP 一键安装  v0.3.0${NC}"
echo -e "${CYAN}═══════════════════════════════════${NC}"
echo ""

# ── 检测系统 ──
OS="$(uname -s)"
ARCH="$(uname -m)"
echo -e "${YELLOW}🔍 检测系统:${NC} $OS $ARCH"

BIN_NAME="hawk-eye-mem"
REPO="qiuhaomem/HawkEye-Mem"
VERSION="v0.3.0"
INSTALL_DIR="${HOME}/.cargo/bin"
MCP_SCRIPT="hawkeye-mcp-server.py"

# ── 确定下载文件名 ──
case "$OS" in
  Linux)
    case "$ARCH" in
      x86_64|amd64) DOWNLOAD_FILE="${BIN_NAME}" ;;  # Linux x86_64 glibc
      aarch64|arm64) echo -e "${RED}❌ Linux ARM64 暂不支持，请从源码编译${NC}"; exit 1 ;;
      *) echo -e "${RED}❌ 不支持的架构: $ARCH${NC}"; exit 1 ;;
    esac
    ;;
  Darwin)
    case "$ARCH" in
      arm64|aarch64) DOWNLOAD_FILE="${BIN_NAME}-macos-arm64" ;;
      x86_64) DOWNLOAD_FILE="${BIN_NAME}-macos-x86_64" ;;
      *) echo -e "${RED}❌ 不支持的架构: $ARCH${NC}"; exit 1 ;;
    esac
    ;;
  *)
    echo -e "${RED}❌ 不支持的系统: $OS${NC}"
    exit 1
    ;;
esac

DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${DOWNLOAD_FILE}"

# ── 创建安装目录 ──
mkdir -p "$INSTALL_DIR"

# ── 下载或编译 ──
if command -v cargo &>/dev/null; then
  # 有 Rust 环境，从源码编译（最稳）
  echo -e "${YELLOW}📦 检测到 Rust 工具链，从源码编译...${NC}"
  if [ -d "$(dirname "$0")/.." ] && [ -f "$(dirname "$0")/../Cargo.toml" ]; then
    # 在项目目录里
    cd "$(dirname "$0")/.."
    cargo install --path . --force 2>&1 | sed 's/^/   /'
  else
    cargo install --git "https://github.com/${REPO}.git" --force 2>&1 | sed 's/^/   /'
  fi
  echo -e "${GREEN}✅ 编译安装成功！${NC}"
else
  # 没有 Rust，尝试下载预编译 binary
  echo -e "${YELLOW}📥 下载预编译 binary: ${DOWNLOAD_URL}${NC}"
  if curl -fsSL "$DOWNLOAD_URL" -o "${INSTALL_DIR}/${BIN_NAME}"; then
    chmod +x "${INSTALL_DIR}/${BIN_NAME}"
    echo -e "${GREEN}✅ 下载成功！${NC}"
  else
    echo -e "${RED}❌ 下载失败！Release 中可能没有 macOS binary。${NC}"
    echo -e "${YELLOW}💡 请先安装 Rust 工具链: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh${NC}"
    echo -e "${YELLOW}   然后重新运行此脚本${NC}"
    exit 1
  fi
fi

# ── 验证 binary ──
echo ""
echo -e "${YELLOW}🔍 验证安装...${NC}"
BIN_PATH="$(which ${BIN_NAME} 2>/dev/null || echo "${INSTALL_DIR}/${BIN_NAME}")"
if "$BIN_PATH" --version 2>&1; then
  echo -e "${GREEN}✅ Binary 验证通过！${NC}"
else
  echo -e "${RED}❌ Binary 异常${NC}"
  exit 1
fi

# ── 下载 MCP Server 脚本 ──
echo ""
echo -e "${YELLOW}📜 下载 MCP Server 脚本...${NC}"
MCP_DIR="${HOME}/.hermes/scripts"
mkdir -p "$MCP_DIR"

if curl -fsSL "https://raw.githubusercontent.com/${REPO}/main/scripts/hawkeye-mcp-server.py" -o "${MCP_DIR}/${MCP_SCRIPT}"; then
  echo -e "${GREEN}✅ MCP Server 脚本下载成功！${NC}"
else
  echo -e "${RED}❌ 下载失败！请手动下载:${NC}"
  echo "   https://raw.githubusercontent.com/${REPO}/main/scripts/hawkeye-mcp-server.py"
  echo "   保存到 ${MCP_DIR}/${MCP_SCRIPT}"
  exit 1
fi

# ── 注册 Hermes MCP ──
echo ""
echo -e "${YELLOW}🔌 注册 Hermes MCP...${NC}"
if command -v hermes &>/dev/null; then
  # 先移除旧的（如果有）
  hermes mcp remove hawk-eye-mem 2>/dev/null || true
  # 注册新的
  hermes mcp add hawk-eye-mem \
    --command python3 \
    --args "${MCP_DIR}/${MCP_SCRIPT}" 2>&1
  echo -e "${GREEN}✅ MCP 注册成功！${NC}"
else
  echo -e "${YELLOW}⚠️ 未检测到 Hermes CLI，手动注册方式：${NC}"
  echo "   hermes mcp add hawk-eye-mem --command python3 --args ${MCP_DIR}/${MCP_SCRIPT}"
fi

# ── 测试 ──
echo ""
echo -e "${YELLOW}🧪 运行测试...${NC}"
# warning 走 stderr，不影响 stdout 的 JSON
"$BIN_PATH" --json 2>/dev/null | python3 -c "
import sys, json
raw = sys.stdin.read().strip()
# 从第一个 { 开始解析（跳过 warning 行，虽然上面 2>/dev/null 了）
start = raw.find('{')
if start >= 0:
    data = json.loads(raw[start:])
    s = data['system']
    g = data['agent_guidance']
    print(f'  内存: {s[\"total_mb\"]}MB 总, {s[\"used_mb\"]}MB 已用 ({s[\"used_percent\"]}%)')
    print(f'  建议: {g[\"action\"]} (压力: {g[\"pressure\"]}, 置信度: {g[\"confidence\"]})')
    print(f'  安全上下文: {g[\"estimated_safe_context_window\"]} tokens')
    print(f'  CPU: {s[\"cpu\"][\"cores\"]}核, 负载 {s[\"cpu\"][\"load_avg_1m\"]}')
    if s.get('thermal'):
        print(f'  温度: CPU {s[\"thermal\"][\"cpu_temp_c\"]}°C, 压力 {s[\"thermal\"][\"pressure\"]}')
    if s.get('gpu') and len(s['gpu']) > 0:
        print(f'  GPU: {len(s[\"gpu\"])} 块')
    if s.get('agents') and s['agents']['count'] > 0:
        agents = ', '.join([a['name'] for a in s['agents']['agents']])
        print(f'  Agent: {s[\"agents\"][\"count\"]} 个 ({agents})')
else:
    print('  无法解析 JSON 输出')
    print(raw[:200])
    sys.exit(1)
" && echo -e "${GREEN}✅ 测试通过！秋毫mem 跑起来了！${NC}" || echo -e "${YELLOW}⚠️ 测试请自行检查输出${NC}"

# ── 完成 ──
echo ""
echo -e "${CYAN}═══════════════════════════════════${NC}"
echo -e "${GREEN}🎉 秋毫mem MCP 安装完成！${NC}"
echo -e "${CYAN}═══════════════════════════════════${NC}"
echo ""
echo -e "下次启动 Hermes 后就能用这些工具了："
echo -e "  ${CYAN}get_memory_status${NC}      — 完整系统状态"
echo -e "  ${CYAN}get_gpu_status${NC}         — GPU 状态"
echo -e "  ${CYAN}get_thermal_status${NC}     — 温度监控"
echo -e "  ${CYAN}get_agent_processes${NC}    — 同机 Agent 检测"
echo -e "  ${CYAN}get_memory_metric${NC}      — 单指标查询"
echo -e "  ${CYAN}get_memory_guidance${NC}    — 决策建议"
echo -e "  ${CYAN}get_calibration_status${NC} — 校准状态"
echo ""
echo -e "如果 Hermes 正在运行，重启一下就能生效："
echo -e "  ${YELLOW}hermes restart${NC}"
echo ""
