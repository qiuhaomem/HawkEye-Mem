#!/bin/bash
# 添加sysroot和库路径
ARGS=("$@")
HAS_SYSROOT=false
for arg in "${ARGS[@]}"; do
    if [[ "$arg" == "--sysroot"* ]]; then
        HAS_SYSROOT=true
        break
    fi
done

if [ "$HAS_SYSROOT" = false ]; then
    exec /usr/bin/gcc-15 "--sysroot=/home/lgl/.local/sysroot" \
        "-B/home/lgl/.local/sysroot/usr/lib/x86_64-linux-gnu/" \
        "${ARGS[@]}"
else
    exec /usr/bin/gcc-15 "${ARGS[@]}"
fi
