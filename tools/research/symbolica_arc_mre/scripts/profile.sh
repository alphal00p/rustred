#!/usr/bin/env bash
# Where do the cycles and the cross-CCX cache fills go? perf record (user mode) of one run (Linux).
#   PERF=/path/perf VARIANT=shared WORK=full K=96 FIRST_CPU=128 OPS=20000 EXTRA="--ctx-offset 48" \
#     OUT=dir scripts/profile.sh
# Writes <OUT>/<variant>-<work>-k<K><TAG>.{cycles,xccx}.report.txt (+ annotations of the top symbols).
# Samples land on the instruction after the one that stalled (skid); read the annotations with that
# in mind. The perf.data files are large and not meant to be committed.
set -euo pipefail
HERE=$(cd "$(dirname "$0")/.." && pwd)
BIN=${BIN:-$HERE/target/release/symbolica-arc-mre}
PERF=${PERF:?set PERF to a perf binary}
VARIANT=${VARIANT:-shared}; WORK=${WORK:-full}; K=${K:-96}; FIRST_CPU=${FIRST_CPU:-128}; OPS=${OPS:-20000}
EXTRA=${EXTRA:-}; TAG=${TAG:-}
CYCLES_PERIOD=${CYCLES_PERIOD:-2000003}; XCCX_PERIOD=${XCCX_PERIOD:-2003}
OUT=${OUT:-$HERE/results/profile-$(date -u +%Y%m%dT%H%M%SZ)}
# Zen 4: demand data-cache fills served by the cache of another CCX, for addresses homed in the
# requester's NUMA node (near_cache) or in another node (far_cache). Other CPUs: set XCCX.
XCCX=${XCCX:-ls_dmnd_fills_from_sys.near_cache:u,ls_dmnd_fills_from_sys.far_cache:u}
export GLIBC_TUNABLES=${GLIBC_TUNABLES:-glibc.malloc.arena_max=$(nproc --all)}
mkdir -p "$OUT"
tag="$VARIANT-$WORK-k$K$TAG"
cpus="$FIRST_CPU-$((FIRST_CPU + K - 1))"
# shellcheck disable=SC2206
args=(--threads "$K" --variant "$VARIANT" --work "$WORK" --ops "$OPS" --cpus "$cpus" $EXTRA)

"$PERF" record -q -e cycles:u -c "$CYCLES_PERIOD" -o "$OUT/$tag.cycles.perf.data" -- \
  taskset -c "$FIRST_CPU" "$BIN" "${args[@]}" > "$OUT/$tag.cycles.out" 2> "$OUT/$tag.cycles.err"
"$PERF" report -i "$OUT/$tag.cycles.perf.data" --stdio --no-children --sort sym --percent-limit 0.5 \
  2>/dev/null | grep -v '^$' > "$OUT/$tag.cycles.report.txt"

"$PERF" record -q -e "$XCCX" -c "$XCCX_PERIOD" -o "$OUT/$tag.xccx.perf.data" -- \
  taskset -c "$FIRST_CPU" "$BIN" "${args[@]}" > "$OUT/$tag.xccx.out" 2> "$OUT/$tag.xccx.err"
"$PERF" report -i "$OUT/$tag.xccx.perf.data" --stdio --no-children --sort sym --percent-limit 0.5 \
  2>/dev/null | grep -v '^$' > "$OUT/$tag.xccx.report.txt"

# annotate the three hottest symbols of each profile (instruction-level location of the samples)
for kind in cycles xccx; do
  i=0
  grep -E '^ +[0-9.]+%' "$OUT/$tag.$kind.report.txt" | head -3 | sed -E 's/^ +[0-9.]+% +\[\.\] //' |
  while IFS= read -r sym; do
    i=$((i + 1))
    { echo "# symbol: $sym"; echo "# hottest instructions (percent of this symbol's samples; skid: the stalled"
      echo "# instruction is usually the one before the sampled one):"
      "$PERF" annotate -i "$OUT/$tag.$kind.perf.data" --stdio -s "$sym" 2>/dev/null |
        grep -E '^ +[0-9.]+ :' | sort -rn | head -25
    } > "$OUT/$tag.$kind.annotate.$i.txt" || true
  done
done
echo "profile written to $OUT ($tag)"
