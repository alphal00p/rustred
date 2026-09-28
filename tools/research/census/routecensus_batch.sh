#!/usr/bin/env bash
# W0 Route-side coverage census (routecensus lane) receipt run. Read-only on
# every input; outputs go to $OUT.
# usage: routecensus_batch.sh CENSUS_BIN OUT [STEP...]
#   steps (default all): gen7 gen7-sat gen6 gen3 c5f; optional: gen7-sat-seed2 gen7-w300 gen7-apply c5f-apply
#   gen7-hits c5f-hits (census route-hits on the rows file of the gen7/c5f step in $ROWS_DIR, default OUT/..)
# Inputs: block clones under the lane worktree TMP (gen3/gen6/gen7 of the v2
# checkpoint, the C-5F control), never written. The v2 heartbeat series is
# the W0.7 receipt's v2-series.txt (copied into OUT).
# Pin it, e.g. nice -n 19 taskset -c 40-51,296-307 routecensus_batch.sh ...
# Every step writes STEP.metrics.txt: wall, user+sys, max RSS (GNU time),
# busy jiffies on the run CPUs (/proc/stat) and the foreign busy CPUs derived
# from them, and the summed schedstat run delay of the census threads.
set -euo pipefail
bin=$(realpath "$1"); out=$2; shift 2
steps=${*:-gen7 gen7-sat gen6 gen3 c5f}
wt=/common/dev/rustred/.claude/worktrees/fable51-rc
series_src=/common/dev/rustred/TMP/w0/census/receipt-v4/v2-series.txt
cpus=${RC_CPUS:-40-51,296-307}
threads=${RAYON_NUM_THREADS:-24}
mkdir -p "$out"
cd "$out"
[[ -s v2-series.txt ]] || cp "$series_src" v2-series.txt
sha256sum "$bin" > census-bin.sha256
[[ -f $bin.build.txt ]] && cp "$bin.build.txt" census-build.txt
{ echo "worktree HEAD $(git -C "$wt" rev-parse HEAD)"; git -C "$wt" status --porcelain --untracked-files=no; } > census-git.txt
cpulist() { # expand "a-b,c-d" into "cpuA cpuB ..."
  local IFS=,; for r in $1; do if [[ $r == *-* ]]; then seq -f 'cpu%g' "${r%-*}" "${r#*-}"; else echo "cpu$r"; fi; done
}
busy() { # summed non-idle jiffies (user nice system irq softirq steal) of the run CPUs
  local want; want=$(cpulist "$cpus" | tr '\n' ' ')
  awk -v want=" $want" '{ if (index(want, " " $1 " ")) s += $2 + $3 + $4 + $7 + $8 + $9 } END { print s }' /proc/stat
}
run() { # run NAME ARGS...: census ARGS > NAME.json 2> NAME.err, with metrics
  local o=$1; shift
  local b0 t0 pid rd=0 x
  b0=$(busy); t0=$(date +%s.%N)
  RAYON_NUM_THREADS=$threads /run/current-system/sw/bin/time -v -o "$o.time" "$bin" "$@" > "$o.json" 2> "$o.err" &
  local tpid=$!
  while kill -0 "$tpid" 2> /dev/null; do
    pid=$(pgrep -P "$tpid" | head -1 || true)
    if [[ -n $pid ]]; then
      x=$( { cat /proc/"$pid"/task/*/schedstat 2> /dev/null || true; } | awk '{ s += $2 } END { print s + 0 }')
      [[ $x -gt 0 ]] && rd=$x
    fi
    sleep 2
  done
  wait "$tpid"
  local b1 t1; b1=$(busy); t1=$(date +%s.%N)
  local us wall rss
  us=$(awk -F': ' '/User time|System time/ { s += $2 } END { print s }' "$o.time")
  rss=$(awk -F': ' '/Maximum resident/ { print $2 }' "$o.time")
  wall=$(awk -v a="$t0" -v b="$t1" 'BEGIN { print b - a }')
  awk -v w="$wall" -v us="$us" -v db="$((b1 - b0))" -v rss="$rss" -v rd="$rd" -v c="$cpus" -v th="$threads" 'BEGIN {
    printf "wall_s %.1f\nuser_sys_s %.1f\nmax_rss_kb %s\nrun_cpus %s\nrayon_threads %s\n", w, us, rss, c, th;
    printf "run_cpus_busy_cpu_s %.1f\nforeign_busy_cpus_avg %.2f\n", db / 100, (db / 100 - us) / w;
    printf "schedstat_run_delay_s(last sample, summed over threads) %.1f\n", rd / 1e9 }' > "$o.metrics.txt"
  echo "$o: $(tail -1 "$o.err") | $(tr '\n' ' ' < "$o.metrics.txt")"
}
for s in $steps; do
  case $s in
    gen7) run gen7-route route "$wt/TMP/gen7" --series v2-series.txt --wait 30 --per 400 --kc 4000 --rows gen7-route-rows.jsonl ;;
    gen7-w300) run gen7-route-w300 route "$wt/TMP/gen7" --series v2-series.txt --wait 300 --per 400 --kc 4000 ;;
    gen7-apply) run gen7-route-apply route-apply "$wt/TMP/gen7" --series v2-series.txt --wait 30 --per 200 --rows gen7-route-apply-rows.jsonl ;;
    c5f-apply) cp -n /common/dev/rustred/TMP/w0/census/receipt-v4/c5f/series.txt c5f-series.txt; run c5f-route-apply route-apply "$wt/TMP/ctl/c5f" --series c5f-series.txt --wait 1 --per 300 --rows c5f-route-apply-rows.jsonl ;;
    gen7-hits) run gen7-route-hits route-hits "$wt/TMP/gen7" --rows-in "${ROWS_DIR:-..}/gen7-route-rows.jsonl" --rows gen7-route-hits-rows.jsonl --pps-adm 4000 --pps-rows gen7-route-hits-pps.jsonl ;;
    c5f-hits) run c5f-route-hits route-hits "$wt/TMP/ctl/c5f" --rows-in "${ROWS_DIR:-..}/c5f-route-rows.jsonl" --rows c5f-route-hits-rows.jsonl --pps-adm 2000 --pps-rows c5f-route-hits-pps.jsonl ;;
    gen7-sat) run gen7-route-saturation route-saturation "$wt/TMP/gen7" --draws 4000 ;;
    gen7-sat-seed2) run gen7-route-saturation-seed2 route-saturation "$wt/TMP/gen7" --draws 4000 --seed 2 ;;
    gen6) run gen6-route route "$wt/TMP/gen6" --series v2-series.txt --wait 30 --per 200 --kc 2000 --rows gen6-route-rows.jsonl ;;
    gen3) run gen3-route route "$wt/TMP/gen3" --series v2-series.txt --wait 30 --per 200 --kc 2000 --rows gen3-route-rows.jsonl ;;
    c5f) cp -n /common/dev/rustred/TMP/w0/census/receipt-v4/c5f/series.txt c5f-series.txt; run c5f-route route "$wt/TMP/ctl/c5f" --series c5f-series.txt --wait 1 --per 300 --kc 2000 --rows c5f-route-rows.jsonl ;;
    *) echo "unknown step $s" >&2; exit 2 ;;
  esac
done
