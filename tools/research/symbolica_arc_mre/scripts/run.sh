#!/usr/bin/env bash
# Thread-scaling sweep of the Symbolica Arc<PolynomialContext> reproducer (Linux).
#
# One process per run; thread i is pinned to CPU FIRST_CPU+i, so K=8 fills one CCX (8 cores
# sharing an L3 on Zen 4c / EPYC 9754), K=24 three CCXs of one NUMA node, K=48 six CCXs on two
# nodes, K=96 twelve CCXs on three nodes. Adjust FIRST_CPU / KS to your topology (lscpu -e,
# CCX = CPUs sharing one L3; use physical cores only, no SMT siblings). The process starts on
# FIRST_CPU (taskset), so globals and the shared set are first touched on that CPU's node.
#
# Environment (all optional):
#   BIN=path          binary (default target/release/symbolica-arc-mre)
#   OUT=dir           output directory (default results/run-<utc>); runs.jsonl is appended to
#   FIRST_CPU=128     first CPU of the run (must start a CCX)
#   KS="1 8 24 48 96" thread counts
#   VARIANTS="shared private-ctx rehome private"
#   WORKS="full specialize split"
#   REPEATS=2         repeats; each repeat runs every (K, work, variant) once, interleaved
#   REPEAT_START=1    number of the first repeat (for drivers that interleave several builds)
#   OPS=20000         operations per thread (timed; a warm-up of OPS/10 runs first)
#   EXTRA="..."       extra arguments, e.g. "--ctx-offset 48" or "--ahash-source thread-local"
#   PERF=/path/perf   if set: perf stat (user mode) per run, counting only the timed loop
#                     (perf stat -D -1 --control fifo; the binary enables/disables the counters)
#   PERF_EVENTS=...   default: Zen 4 events (cycles, instructions, demand fills from the same CCX,
#                     from another CCX with the address in the same NUMA node (near_cache) or in
#                     another node (far_cache)). Other CPUs: e.g. PERF_EVENTS=cycles:u,instructions:u
#                     (Zen 3: ls_any_fills_from_sys.*; Intel: mem_load_l3_miss_retired.remote_fwd/
#                     remote_hitm or ocr.* events); the fill columns of summarize.sh then show "-".
#   LOCK=file LOCK_MIN_K=24 LOCK_WAIT=3600
#                     if LOCK is set, runs with K >= LOCK_MIN_K hold `flock LOCK` (shared hosts)
#   BUSY_SECS=1       window before/after each run in which the busy load of the run CPUs is sampled
#   SOURCE_NOTE="..." free text for host.txt (e.g. which Symbolica source / diff the binary uses)
#   LABEL=name        tag stored in every runs.jsonl line (e.g. the build); summarize.sh keys rows by it
# Sets GLIBC_TUNABLES=glibc.malloc.arena_max=<CPUs> unless already set: glibc 2.35-2.38 derive the
# arena limit from the calling thread's affinity, which is a single CPU for these pinned threads.
# Needs SYMBOLICA_LICENSE (the binary refuses to run without an active license).
set -euo pipefail

HERE=$(cd "$(dirname "$0")/.." && pwd)
BIN=${BIN:-$HERE/target/release/symbolica-arc-mre}
OUT=${OUT:-$HERE/results/run-$(date -u +%Y%m%dT%H%M%SZ)}
FIRST_CPU=${FIRST_CPU:-128}
KS=${KS:-"1 8 24 48 96"}
VARIANTS=${VARIANTS:-"shared private-ctx rehome private"}
WORKS=${WORKS:-"full specialize split"}
REPEATS=${REPEATS:-2}
REPEAT_START=${REPEAT_START:-1}
OPS=${OPS:-20000}
PERF=${PERF:-}
PERF_EVENTS=${PERF_EVENTS:-cycles:u,instructions:u,ls_dmnd_fills_from_sys.local_ccx:u,ls_dmnd_fills_from_sys.near_cache:u,ls_dmnd_fills_from_sys.far_cache:u}
LOCK=${LOCK:-}
LOCK_MIN_K=${LOCK_MIN_K:-24}
LOCK_WAIT=${LOCK_WAIT:-3600}
BUSY_SECS=${BUSY_SECS:-1}
EXTRA=${EXTRA:-}
SOURCE_NOTE=${SOURCE_NOTE:-}
LABEL=${LABEL:-}
export GLIBC_TUNABLES=${GLIBC_TUNABLES:-glibc.malloc.arena_max=$(nproc --all)}

[ -x "$BIN" ] || { echo "binary not found: $BIN (cargo build --release)" >&2; exit 1; }
[ -n "${SYMBOLICA_LICENSE:-}" ] || { echo "SYMBOLICA_LICENSE is not set" >&2; exit 1; }
if [ -n "$PERF" ] && ! "$PERF" stat -e "$PERF_EVENTS" -o /dev/null -- true 2>/dev/null; then
  echo "perf stat cannot count PERF_EVENTS=$PERF_EVENTS on this CPU; set PERF_EVENTS (see header)" >&2
  exit 1
fi
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

probe=$("$BIN" --threads 1 --ops 10 --ctx-offset any --map-offset any 2>&1 | grep -m1 "^RESULT " || true)
field() { sed -n "s/.*\"$1\":\"\\([^\"]*\\)\".*/\\1/p" <<< "$probe"; }
{
  echo "date_utc: $(date -u +%FT%TZ)"
  echo "host: $(hostname)"
  echo "kernel: $(uname -srvm)"
  grep -m1 'model name' /proc/cpuinfo
  lscpu | grep -E '^(CPU\(s\)|On-line|Thread|Core|Socket|NUMA node|L2|L3|Model name)'
  echo "binary: $BIN"
  echo "binary_sha256: $(sha256sum "$BIN" | cut -d' ' -f1)"
  echo "symbolica: $(field symbolica)"
  echo "libc: $(field libc) (ldd: $(ldd "$BIN" 2>/dev/null | grep -m1 -o '/[^ ]*libc.so[^ ]*' || echo n/a))"
  echo "source: ${SOURCE_NOTE:-n/a}"
  echo "rustc: ${RUSTC_VERSION:-$(rustc --version 2>/dev/null || echo n/a)}"
  echo "perf_event_paranoid: $(cat /proc/sys/kernel/perf_event_paranoid)"
  echo "numa_balancing: $(cat /proc/sys/kernel/numa_balancing 2>/dev/null || echo n/a)"
  echo "GLIBC_TUNABLES: $GLIBC_TUNABLES"
  echo "settings: FIRST_CPU=$FIRST_CPU KS='$KS' VARIANTS='$VARIANTS' WORKS='$WORKS' REPEATS=$REPEATS REPEAT_START=$REPEAT_START OPS=$OPS EXTRA='$EXTRA' BUSY_SECS=$BUSY_SECS PERF=${PERF:-none}"
  echo "perf_events: ${PERF:+$PERF_EVENTS (timed loop only)}"
} > "$OUT/host.txt"

run_one() {  # repeat k work variant
  local r=$1 k=$2 w=$3 v=$4
  local cpus="$FIRST_CPU-$((FIRST_CPU + k - 1))"
  local tag="r${r}-k${k}-${w}-${v}${EXTRA:+-$(tr -c "A-Za-z0-9\n" "_" <<< "$EXTRA")}"
  local pre post start rc=0
  pre=$(busy "$cpus" "$BUSY_SECS")
  start=$(date -u +%FT%TZ)
  local cmd=(taskset -c "$FIRST_CPU" "$BIN" --threads "$k" --variant "$v" --work "$w" --ops "$OPS" --cpus "$cpus")
  # shellcheck disable=SC2206
  [ -n "$EXTRA" ] && cmd+=($EXTRA)
  local fifo_dir=""
  if [ -n "$PERF" ]; then
    fifo_dir=$(mktemp -d "${TMPDIR:-/tmp}/mre-perf.XXXXXX")
    mkfifo "$fifo_dir/ctl" "$fifo_dir/ack"
    cmd=(env MRE_PERF_CTL="$fifo_dir/ctl" MRE_PERF_ACK="$fifo_dir/ack"
         "$PERF" stat -D -1 --control "fifo:$fifo_dir/ctl,$fifo_dir/ack" -x, -o "$OUT/$tag.perf.csv"
         -e "$PERF_EVENTS" -- "${cmd[@]}")
  fi
  if [ -n "$LOCK" ] && [ "$k" -ge "$LOCK_MIN_K" ]; then
    cmd=(flock -w "$LOCK_WAIT" "$LOCK" "${cmd[@]}")
  fi
  "${cmd[@]}" > "$OUT/$tag.out" 2> "$OUT/$tag.err" || rc=$?
  [ -n "$fifo_dir" ] && rm -rf "$fifo_dir"
  post=$(busy "$cpus" "$BUSY_SECS")
  if [ "$rc" -ne 0 ]; then
    echo "$tag FAILED rc=$rc (see $OUT/$tag.err)" >&2
    return 0
  fi
  local perf_json=""
  if [ -n "$PERF" ] && [ -f "$OUT/$tag.perf.csv" ]; then
    perf_json=$(awk -F, '$3 != "" && $1 ~ /^[0-9]+$/ { e = $3; sub(/:u$/, "", e); gsub(/[^A-Za-z0-9_]/, "_", e);
        printf ",\"perf_%s\":%s", e, $1 }' "$OUT/$tag.perf.csv")
  fi
  local line
  line=$(grep -m1 '^RESULT ' "$OUT/$tag.out" | sed 's/^RESULT {//')
  printf '{"repeat":%s,"start_utc":"%s","busy_pre_pct":%s,"busy_post_pct":%s,"label":"%s","extra":"%s"%s,%s\n' \
    "$r" "$start" "$pre" "$post" "$LABEL" "$EXTRA" "$perf_json" "$line" >> "$OUT/runs.jsonl"
  printf '%s k=%-3s %-10s %-11s cpu/op=%s ns  busy pre/post %s/%s%%\n' "$tag" "$k" "$w" "$v" \
    "$(sed -n 's/.*"cpu_ns_per_op":\([0-9.]*\).*/\1/p' <<< "$line")" "$pre" "$post"
}

for r in $(seq "$REPEAT_START" $((REPEAT_START + REPEATS - 1))); do
  for k in $KS; do
    for w in $WORKS; do
      for v in $VARIANTS; do
        run_one "$r" "$k" "$w" "$v"
      done
    done
  done
done
"$HERE/scripts/summarize.sh" "$OUT" > "$OUT/summary.md"
cat "$OUT/summary.md"
