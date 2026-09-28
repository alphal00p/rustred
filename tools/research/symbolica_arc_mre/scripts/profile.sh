#!/usr/bin/env bash
# Where do the cycles and the cross-CCX cache fills go? perf record (user mode) of one run.
#   PERF=/path/perf VARIANT=shared WORK=full K=96 FIRST_CPU=128 OPS=20000 OUT=dir scripts/profile.sh
# Writes <OUT>/<variant>-<work>-k<K>.{cycles,xccx}.report.txt (+ annotations of the top symbols).
# The perf.data files are large and not meant to be committed.
set -euo pipefail
HERE=$(cd "$(dirname "$0")/.." && pwd)
BIN=${BIN:-$HERE/target/release/symbolica-arc-mre}
PERF=${PERF:?set PERF to a perf binary}
VARIANT=${VARIANT:-shared}; WORK=${WORK:-full}; K=${K:-96}; FIRST_CPU=${FIRST_CPU:-128}; OPS=${OPS:-20000}
OUT=${OUT:-$HERE/results/profile-$(date -u +%Y%m%dT%H%M%SZ)}
# Zen 4: demand data-cache fills served by a cache of another CCX, same node (near) or other node (far)
XCCX=${XCCX:-ls_dmnd_fills_from_sys.near_cache:u,ls_dmnd_fills_from_sys.far_cache:u}
mkdir -p "$OUT"
tag="$VARIANT-$WORK-k$K"
cpus="$FIRST_CPU-$((FIRST_CPU + K - 1))"
args=(--threads "$K" --variant "$VARIANT" --work "$WORK" --ops "$OPS" --cpus "$cpus")

"$PERF" record -q -e cycles:u -c 2000003 -o "$OUT/$tag.cycles.perf.data" -- "$BIN" "${args[@]}" \
  > "$OUT/$tag.cycles.out" 2> "$OUT/$tag.cycles.err"
"$PERF" report -i "$OUT/$tag.cycles.perf.data" --stdio --no-children --sort sym --percent-limit 0.5 \
  2>/dev/null | grep -v '^$' > "$OUT/$tag.cycles.report.txt"

"$PERF" record -q -e "$XCCX" -c 2003 -o "$OUT/$tag.xccx.perf.data" -- "$BIN" "${args[@]}" \
  > "$OUT/$tag.xccx.out" 2> "$OUT/$tag.xccx.err"
"$PERF" report -i "$OUT/$tag.xccx.perf.data" --stdio --no-children --sort sym --percent-limit 0.5 \
  2>/dev/null | grep -v '^$' > "$OUT/$tag.xccx.report.txt"

# annotate the three hottest symbols of each profile (instruction-level location of the samples)
for kind in cycles xccx; do
  i=0
  grep -E '^ +[0-9.]+%' "$OUT/$tag.$kind.report.txt" | head -3 | sed -E 's/^ +[0-9.]+% +\[\.\] //' |
  while IFS= read -r sym; do
    i=$((i + 1))
    { echo "# symbol: $sym"; echo "# hottest instructions (percent of this symbol's samples):"
      "$PERF" annotate -i "$OUT/$tag.$kind.perf.data" --stdio -s "$sym" 2>/dev/null |
        grep -E '^ +[0-9.]+ :' | sort -rn | head -25
    } > "$OUT/$tag.$kind.annotate.$i.txt" || true
  done
done
echo "profile written to $OUT ($tag)"
