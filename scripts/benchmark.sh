#!/usr/bin/env bash
# Runs the release build several times and records wall time and peak memory.
# Usage: scripts/benchmark.sh [input.csv] [runs]
set -euo pipefail

input="${1:-ad_data.csv}"
runs="${2:-5}"
binary=target/release/ad-aggregator

cargo build --release --quiet

if [[ "$(uname)" == "Darwin" ]]; then
    time_flags=(-l)
else
    time_flags=(-v)
fi

echo "date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "input: $input ($(wc -c < "$input" | tr -d ' ') bytes)"
echo "host: $(uname -sm)"
for run in $(seq 1 "$runs"); do
    echo "--- run $run"
    /usr/bin/time "${time_flags[@]}" "$binary" --input "$input" --output results 2>&1 \
        | grep -E "Processed|real|Elapsed|maximum resident|Maximum resident|peak memory"
done
