#!/usr/bin/env bash
# Thread-scaling sweep of the Symbolica Arc<PolynomialContext> reproducer.
#
# One process per run; thread i is pinned to CPU FIRST_CPU+i, so K=8 fills one CCX (8 cores
# sharing an L3 on Zen 4c / EPYC 9754), K=24 three CCXs of one NUMA node, K=48 six CCXs on two
# nodes, K=96 twelve CCXs on three nodes. Adjust FIRST_CPU / KS to your topology (lscpu -e,
# CCX = CPUs sharing one L3; use physical cores only, no SMT siblings).
#
# Environment (all optional):
#   BIN=path          binary (default target/release/symbolica-arc-mre)
#   OUT=dir           output directory (default results/run-<utc>)
#   FIRST_CPU=128     first CPU of the run (must start a CCX)
#   KS="1 8 24 48 96" thread counts
#   VARIANTS="shared private private-ctx rehome"
#   WORKS="full specialize split"
#   REPEATS=2         repeats; each repeat runs every (K, work, variant) once, interleaved
#   OPS=40000         operations per thread
#   PERF=/path/perf   if set: perf stat (user mode) per run with PERF_EVENTS
#   LOCK=file LOCK_MIN_K=24 LOCK_WAIT=3600
#                     if LOCK is set, runs with K >= LOCK_MIN_K hold `flock LOCK` (shared hosts)
#   EXTRA="..."       extra arguments for the binary (e.g. "--npolys 8192")
# Needs SYMBOLICA_LICENSE (the binary refuses to run without an active license).
set -euo pipefail

HERE=$(cd "$(dirname "$0")/.." && pwd)
BIN=${BIN:-$HERE/target/release/symbolica-arc-mre}
OUT=${OUT:-$HERE/results/run-$(date -u +%Y%m%dT%H%M%SZ)}
FIRST_CPU=${FIRST_CPU:-128}
KS=${KS:-"1 8 24 48 96"}
VARIANTS=${VARIANTS:-"shared private private-ctx rehome"}
WORKS=${WORKS:-"full specialize split"}
REPEATS=${REPEATS:-2}
OPS=${OPS:-40000}
PERF=${PERF:-}
PERF_EVENTS=${PERF_EVENTS:-cycles:u,instructions:u,ls_dmnd_fills_from_sys.local_ccx:u,ls_dmnd_fills_from_sys.near_cache:u,ls_dmnd_fills_from_sys.far_cache:u}
LOCK=${LOCK:-}
LOCK_MIN_K=${LOCK_MIN_K:-24}
LOCK_WAIT=${LOCK_WAIT:-3600}
EXTRA=${EXTRA:-}

[ -x "$BIN" ] || { echo "binary not found: $BIN (cargo build --release)" >&2; exit 1; }
[ -n "${SYMBOLICA_LICENSE:-}" ] || { echo "SYMBOLICA_LICENSE is not set" >&2; exit 1; }
mkdir -p "$OUT"

# busy percentage (user+nice+system+irq+softirq+steal) of a CPU range a-b over $2 seconds
busy() {
  local lo=${1%-*} hi=${1#*-} secs=$2
  snap() { awk -v lo="$lo" -v hi="$hi" '$1 ~ /^cpu[0-9]+$/ { c = substr($1, 4) + 0;
      if (c >= lo && c <= hi) { b += $2+$3+$4+$7+$8+$9; t += $2+$3+$4+$5+$6+$7+$8+$9 } }
      END { print b + 0, t + 0 }' /proc/stat; }
  local b0 t0 b1 t1
  read -r b0 t0 < <(snap); sleep "$secs"; read -r b1 t1 < <(snap)
  awk -v b=$((b1 - b0)) -v t=$((t1 - t0)) 'BEGIN { printf "%.1f", (t > 0 ? 100 * b / t : 0) }'
}

{
  echo "date_utc: $(date -u +%FT%TZ)"
  echo "host: $(hostname)"
  echo "kernel: $(uname -srvm)"
  grep -m1 'model name' /proc/cpuinfo
  lscpu | grep -E '^(CPU\(s\)|On-line|Thread|Core|Socket|NUMA node|L2|L3|Model name)'
  echo "binary: $BIN"
  echo "binary_sha256: $(sha256sum "$BIN" | cut -d' ' -f1)"
  echo "symbolica: $("$BIN" --threads 1 --ops 10 2>&1 | grep -m1 -o 'symbolica-v[^ ]*' || true)"
  echo "rustc: ${RUSTC_VERSION:-$(rustc --version 2>/dev/null || echo n/a)}"
  echo "perf_event_paranoid: $(cat /proc/sys/kernel/perf_event_paranoid)"
  echo "numa_balancing: $(cat /proc/sys/kernel/numa_balancing 2>/dev/null || echo n/a)"
  echo "settings: FIRST_CPU=$FIRST_CPU KS='$KS' VARIANTS='$VARIANTS' WORKS='$WORKS' REPEATS=$REPEATS OPS=$OPS EXTRA='$EXTRA' PERF=${PERF:-none}"
  echo "perf_events: ${PERF:+$PERF_EVENTS}"
} > "$OUT/host.txt"

run_one() {  # repeat k work variant
  local r=$1 k=$2 w=$3 v=$4
  local cpus="$FIRST_CPU-$((FIRST_CPU + k - 1))"
  local tag="r${r}-k${k}-${w}-${v}"
  local pre post start
  pre=$(busy "$cpus" 1)
  start=$(date -u +%FT%TZ)
  local cmd=("$BIN" --threads "$k" --variant "$v" --work "$w" --ops "$OPS" --cpus "$cpus")
  # shellcheck disable=SC2206
  [ -n "$EXTRA" ] && cmd+=($EXTRA)
  if [ -n "$PERF" ]; then
    cmd=("$PERF" stat -x, -o "$OUT/$tag.perf.csv" -e "$PERF_EVENTS" -- "${cmd[@]}")
  fi
  if [ -n "$LOCK" ] && [ "$k" -ge "$LOCK_MIN_K" ]; then
    cmd=(flock -w "$LOCK_WAIT" "$LOCK" "${cmd[@]}")
  fi
  "${cmd[@]}" > "$OUT/$tag.out" 2> "$OUT/$tag.err"
  post=$(busy "$cpus" 1)
  local perf_json=""
  if [ -n "$PERF" ] && [ -f "$OUT/$tag.perf.csv" ]; then
    perf_json=$(awk -F, '$3 != "" && $1 ~ /^[0-9]+$/ { e = $3; sub(/:u$/, "", e); gsub(/[^A-Za-z0-9_]/, "_", e);
        printf ",\"perf_%s\":%s", e, $1 }' "$OUT/$tag.perf.csv")
  fi
  local line
  line=$(grep -m1 '^RESULT ' "$OUT/$tag.out" | sed 's/^RESULT {//')
  printf '{"repeat":%s,"start_utc":"%s","busy_pre_pct":%s,"busy_post_pct":%s%s,%s\n' \
    "$r" "$start" "$pre" "$post" "$perf_json" "$line" >> "$OUT/runs.jsonl"
  printf '%s k=%-3s %-10s %-11s cpu/op=%s ns  busy pre/post %s/%s%%\n' "$tag" "$k" "$w" "$v" \
    "$(sed -n 's/.*"cpu_ns_per_op":\([0-9.]*\).*/\1/p' <<< "$line")" "$pre" "$post"
}

for r in $(seq 1 "$REPEATS"); do
  for k in $KS; do
    for w in $WORKS; do
      for v in $VARIANTS; do
        run_one "$r" "$k" "$w" "$v"
      done
    done
  done
done
"$HERE/scripts/summarize.sh" "$OUT" | tee "$OUT/summary.md"
