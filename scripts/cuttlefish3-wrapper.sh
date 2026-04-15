#!/usr/bin/bash
# Wrapper for cuttlefish3: intercepts -t <N> and sets PARLAY_NUM_THREADS,
# then forwards the remaining arguments to the real binary.
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

args=()
i=1
while [ $i -le $# ]; do
    arg="${!i}"
    if [ "$arg" = "-t" ]; then
        i=$((i + 1))
        export PARLAY_NUM_THREADS="${!i}"
    else
        args+=("$arg")
    fi
    i=$((i + 1))
done

exec "$SCRIPT_DIR/cuttlefish3-bin" "${args[@]}"
