#!/bin/bash
# 把所有 /usr/lib/x86_64-linux-gnu/ 开头的绝对路径
# 替换为 /home/lgl/.local/lib/
ARGS=()
for arg in "$@"; do
    # 如果参数是以/usr/lib/x86_64-linux-gnu/开头的绝对路径
    if [[ "$arg" == /usr/lib/x86_64-linux-gnu/* ]]; then
        # 替换路径
        NEW_ARG="${arg/\/usr\/lib\/x86_64-linux-gnu\//\/home\/lgl\/.local\/lib\/}"
        ARGS+=("$NEW_ARG")
    else
        ARGS+=("$arg")
    fi
done
exec /usr/bin/gcc-15 "${ARGS[@]}"
