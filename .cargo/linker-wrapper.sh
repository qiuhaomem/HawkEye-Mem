#!/bin/bash
exec /usr/bin/gcc-15 "$@" \
  -L/tmp/libc6_full/usr/lib/x86_64-linux-gnu \
  -L/home/lgl/.local/lib
