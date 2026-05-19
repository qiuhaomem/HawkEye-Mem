#!/bin/bash
ARGS=()
for arg in "$@"; do
    # 把所有 /usr/lib/x86_64-linux-gnu/ 替换为实际路径
    NEW_ARG="${arg//\/usr\/lib\/x86_64-linux-gnu\//\/home\/lgl\/.local\/lib\/}"
    ARGS+=("$NEW_ARG")
done
exec /usr/bin/x86_64-linux-gnu-ld.bfd "${ARGS[@]}"
