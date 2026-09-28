#!/usr/bin/env bash
# One W0.3 harness run (see crates/rustred-app/src/application/routed_campaign/
# walking/reinspection.rs). Everything is passed by environment:
#   BIN      harness test binary (cargo test --release -p rustred-app --lib --no-run)
#   FIXTURE  reinspection fixture JSON          OUT   new output directory
#   CPUS     taskset CPU list                    THREADS K (default 1)
#   SINK ORDER SUBSET LIMIT PASSES PIN REPLICAS REPLICA_HOME CANCEL TAP  -> RUSTRED_HARNESS_*
#   NUMA     none | interleave:<nodes> | bind:<nodes> | membind:<nodes>   (numactl; membind leaves the CPU
#            affinity alone and only binds memory, e.g. a placement control with all pages on node 4)
#   NUMA_MAPS_EVERY  seconds between /proc/<pid>/numa_maps samples (per-node kB, all and non-file mappings)
#            written to numa_maps.tsv (default off). /proc/vmstat numa_* counters are always snapshotted
#            (vmstat.before/after; system-wide, so other users' page migrations are included).
#   ALLOC    glibc | mimalloc | census          (LD_PRELOAD)
#   PERF     none | stat | record-fp | record-dwarf | record-ev | strace-futex
#            (stat events: PERF_EVENTS overrides the default list; record-fp: PERF_FREQ,
#             PERF_EVENT (default cycles:u); record-ev: PERF_EVENT sampled every PERF_PERIOD
#             events with frame-pointer call chains, e.g. ls_dmnd_fills_from_sys.far_cache:u)
#   NICE     default 5
#   PERF_MMAP_PAGES  perf record ring pages per CPU (default 64): the per-user perf mlock budget
#            (perf_event_mlock_kb x CPUs) must hold two concurrent recordings of this user
# Writes OUT/{receipt.json,natives.jsonl,run.env,procstat.before,procstat.after,
# perf*.txt|perf.data}. Foreign load is judged afterwards from the /proc/stat
# snapshots of CPUS against the process's own CPU time (analyze.py).
set -euo pipefail
: "${BIN:?}" "${FIXTURE:?}" "${OUT:?}" "${CPUS:?}"
THREADS=${THREADS:-1}; SINK=${SINK:-count}; ORDER=${ORDER:-fixture}
NUMA=${NUMA:-none}; ALLOC=${ALLOC:-glibc}; PERF=${PERF:-stat}; NICE=${NICE:-5}
PERF_BIN=${PERF_BIN:-/nix/store/wizn21b9virxqcnm4n89b09pgqkaxfn3-perf-linux-7.2.5/bin/perf}
NUMACTL=${NUMACTL:-/nix/store/00p2pzg3i0bdlg9iab09jyr46lnvpi6n-numactl-2.0.18/bin/numactl}
MIMALLOC=${MIMALLOC:-/nix/store/6h8sd8vcrmcmaa76qflq3yxhk2c0i78x-mimalloc-3.4.5/lib/libmimalloc.so}
HERE=$(cd "$(dirname "$0")" && pwd)
parent=$(dirname "$OUT"); mkdir -p "$parent"
if [ -e "$OUT" ]; then echo "refusing to overwrite $OUT" >&2; exit 2; fi
stage="$parent/.stage-$(basename "$OUT")"; rm -rf "$stage"; mkdir -p "$stage"

env_args=(RUSTRED_HARNESS_FIXTURE="$FIXTURE" RUSTRED_HARNESS_OUT="$OUT"
  RUSTRED_HARNESS_THREADS="$THREADS" RUSTRED_HARNESS_SINK="$SINK" RUSTRED_HARNESS_ORDER="$ORDER"
  RAYON_NUM_THREADS=1 OMP_NUM_THREADS=1 SYMBOLICA_HIDE_BANNER=1)
[ -n "${SUBSET:-}" ] && env_args+=(RUSTRED_HARNESS_SUBSET="$SUBSET")
[ -n "${LIMIT:-}" ] && env_args+=(RUSTRED_HARNESS_LIMIT="$LIMIT")
[ -n "${PASSES:-}" ] && env_args+=(RUSTRED_HARNESS_PASSES="$PASSES")
[ -n "${PIN:-}" ] && env_args+=(RUSTRED_HARNESS_PIN="$PIN")
[ -n "${REPLICAS:-}" ] && env_args+=(RUSTRED_HARNESS_REPLICAS="$REPLICAS")
[ -n "${REPLICA_HOME:-}" ] && env_args+=(RUSTRED_HARNESS_REPLICA_HOME="$REPLICA_HOME")
[ -n "${CANCEL:-}" ] && env_args+=(RUSTRED_HARNESS_CANCEL_FRACTION="$CANCEL")
[ -n "${TAP:-}" ] && env_args+=(RUSTRED_HARNESS_TAP="$TAP")
[ -n "${MANIFEST:-}" ] && env_args+=(RUSTRED_HARNESS_MANIFEST="$MANIFEST")
[ -n "${OWNER_BASE:-}" ] && env_args+=(RUSTRED_HARNESS_OWNER_BASE="$OWNER_BASE")
[ -n "${QUERIES:-}" ] && env_args+=(RUSTRED_HARNESS_QUERIES="$QUERIES")
case "$ALLOC" in
  glibc) ;;
  mimalloc) env_args+=(LD_PRELOAD="$MIMALLOC") ;;
  census) env_args+=(LD_PRELOAD="$CENSUS_SO" MALLOC_CENSUS_OUT="$stage/malloc-census"
            MALLOC_CENSUS_GMP="$CENSUS_GMP" MALLOC_CENSUS_OTHER_C="$CENSUS_OTHER_C") ;;
  *) echo "unknown ALLOC $ALLOC" >&2; exit 2 ;;
esac
numa=()
case "$NUMA" in
  none) ;;
  interleave:*) numa=("$NUMACTL" --interleave="${NUMA#interleave:}") ;;
  bind:*) numa=("$NUMACTL" --cpunodebind="${NUMA#bind:}" --membind="${NUMA#bind:}") ;;
  membind:*) numa=("$NUMACTL" --membind="${NUMA#membind:}") ;;
  *) echo "unknown NUMA $NUMA" >&2; exit 2 ;;
esac
events=${PERF_EVENTS:-cycles:u,instructions:u,ls_any_fills_from_sys.dram_io_near:u,ls_any_fills_from_sys.dram_io_far:u,ls_any_fills_from_sys.far_cache:u,l2_cache_req_stat.ic_dc_miss_in_l2:u}
perf=()
case "$PERF" in
  none) ;;
  stat) perf=("$PERF_BIN" stat -x, -o "$stage/perfstat.csv" -e "$events" --) ;;
  record-fp) perf=("$PERF_BIN" record -m "${PERF_MMAP_PAGES:-64}" -e "${PERF_EVENT:-cycles:u}" -F "${PERF_FREQ:-499}" -g --call-graph fp -o "$stage/perf.data" --) ;;
  record-ev) perf=("$PERF_BIN" record -m "${PERF_MMAP_PAGES:-64}" -e "${PERF_EVENT:?}" -c "${PERF_PERIOD:?}" -g --call-graph fp -o "$stage/perf.data" --) ;;
  record-dwarf) perf=("$PERF_BIN" record -F "${PERF_FREQ:-99}" --call-graph dwarf,16384 -o "$stage/perf.data" --) ;;
  strace-futex) perf=("${STRACE_BIN:-/nix/store/qcl66q3nnbd9g6273qp258nnxmb7vwfg-strace-7.1/bin/strace}" -f --seccomp-bpf -e trace=futex -c -o "$stage/strace-futex.txt" --) ;;
  *) echo "unknown PERF $PERF" >&2; exit 2 ;;
esac
{
  printf 'BIN=%s\nBIN_SHA256=%s\n' "$BIN" "$(sha256sum "$BIN" | cut -d' ' -f1)"
  printf 'CPUS=%s\nNUMA=%s\nALLOC=%s\nPERF=%s\nNICE=%s\nHOST_MEMAVAILABLE_GIB=%s\n' "$CPUS" "$NUMA" "$ALLOC" "$PERF" "$NICE" \
    "$(awk '/MemAvailable/{print int($2/1048576)}' /proc/meminfo)"
  printf '%s\n' "${env_args[@]}"
} > "$stage/run.env"
# per-node kB of one process's numa_maps: "<t> all <node> <kB>" and "<t> nonfile <node> <kB>" lines
numa_maps_sample() {
  awk -v t="$2" '{ file = ($0 ~ / file=/); ps = 4
    for (i = 1; i <= NF; i++) if ($i ~ /^kernelpagesize_kB=/) { split($i, a, "="); ps = a[2] }
    for (i = 1; i <= NF; i++) if ($i ~ /^N[0-9]+=/) { split(substr($i, 2), b, "="); all[b[1]] += b[2] * ps
      if (!file) nf[b[1]] += b[2] * ps } }
    END { for (n in all) print t, "all", n, all[n]; for (n in nf) print t, "nonfile", n, nf[n] }' "/proc/$1/numa_maps"
}
grep '^numa_' /proc/vmstat > "$stage/vmstat.before" 2> /dev/null || true
grep '^cpu' /proc/stat > "$stage/procstat.before"
date +%s.%N > "$stage/start.unix"
set +e
# numactl before taskset: --cpunodebind would otherwise widen the affinity to the whole node
# (session D ccd12 put four 8-thread processes on the same 8 CPUs of each node that way).
nice -n "$NICE" "${numa[@]}" taskset -c "$CPUS" env "${env_args[@]}" "${perf[@]}" \
  "$BIN" reinspect_fixture --ignored --nocapture --test-threads 1 > "$stage/stdout" 2> "$stage/stderr" &
run_pid=$!
if [ -n "${NUMA_MAPS_EVERY:-}" ]; then
  (
    t0=$(date +%s); bin_pid=""
    while kill -0 "$run_pid" 2> /dev/null; do
      if [ -z "$bin_pid" ]; then
        for c in "$run_pid" $(pgrep -P "$run_pid" 2> /dev/null); do
          [ "$(readlink -f "/proc/$c/exe" 2> /dev/null)" = "$(readlink -f "$BIN")" ] && bin_pid=$c
        done
      fi
      [ -n "$bin_pid" ] && numa_maps_sample "$bin_pid" $(( $(date +%s) - t0 )) >> "$stage/numa_maps.tsv" 2> /dev/null
      sleep "$NUMA_MAPS_EVERY"
    done
  ) &
  sampler_pid=$!
fi
wait "$run_pid"
code=$?
[ -n "${sampler_pid:-}" ] && { kill "$sampler_pid" 2> /dev/null; wait "$sampler_pid" 2> /dev/null; }
set -e
date +%s.%N > "$stage/end.unix"
grep '^cpu' /proc/stat > "$stage/procstat.after"
grep '^numa_' /proc/vmstat > "$stage/vmstat.after" 2> /dev/null || true
echo "$code" > "$stage/exit_code"
if [ -d "$OUT" ]; then mv "$stage"/* "$OUT"/ && rmdir "$stage"; else mv "$stage" "$OUT"; fi
echo "run $OUT exit $code"
exit "$code"
