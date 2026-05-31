#!/bin/bash
# Copyright 2026 秋毫mem Contributors
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

cd /home/lgl/projects/qiuhaomem

echo "开始 macOS 2核2G 模拟测试..."
echo "CPU限制: 200% (2核) | 内存限制: 2048MB"
echo ""

# 3 concurrent audits
python3 -m scripts.token_audit --token-audit > /tmp/test_mac_a.txt 2>&1 &
PID1=$!
python3 -m scripts.token_audit --token-audit > /tmp/test_mac_b.txt 2>&1 &
PID2=$!
python3 -m scripts.token_audit --token-audit > /tmp/test_mac_c.txt 2>&1 &
PID3=$!

echo "启动3个并发审计: PID=$PID1 $PID2 $PID3"
wait $PID1 $PID2 $PID3
echo "全部完成"
echo ""

# Results
echo "=== 结果汇总 ==="
for f in a b c; do
    lines=$(wc -l < /tmp/test_mac_$f.txt)
    total=$(grep '总消费' /tmp/test_mac_$f.txt | head -1)
    sessions=$(grep 'Sessions' /tmp/test_mac_$f.txt | head -1)
    echo "Process ${f^^}: ${lines} lines | ${total} | ${sessions}"
done

echo ""
echo "=== 数据一致性检查 ==="
TOTALS=$(grep '总消费' /tmp/test_mac_a.txt /tmp/test_mac_b.txt /tmp/test_mac_c.txt | sort -u | wc -l)
if [ "$TOTALS" -eq 1 ]; then
    echo "✅ 三进程输出完全一致！"
else
    echo "⚠️ 发现 $TOTALS 种不同结果"
fi

echo ""
echo "=== 系统资源使用 ==="
free -m | awk '/Mem/{print "内存: 已用", $3, "MB | 可用", $7, "MB"}'
