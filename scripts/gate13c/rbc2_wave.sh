#!/usr/bin/env bash
# gate13c item 6: rb_c2 (proc_reaudit_regressions) reproduction. Every test binary runs as pid 1 of its OWN private
# user+pid+mount namespace (rule L10; --kill-child: the namespace dies with its unshare), private TMPDIR per run (L22),
# taskset to the L21 base set minus the held list. Started by the caller as a detached service unit (LANE_RULES rule 5).
# Args: TAG WAVES SPEC...   SPEC = <label>=<binary> (one instance of each SPEC per wave, all in parallel; repeat a SPEC
# to run it twice per wave). Env TRACE=1: RR_HUNT_TRACE=1 for the binaries (debug builds print the hunt's trace).
# Output: S/logs/rbc2/<TAG>/<label>.<wave>.<k>.txt and a summary line per run in S/logs/rbc2/<TAG>/summary.txt.
set -u
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate13c
TAG=$1; WAVES=$2; shift 2
D=$S/logs/rbc2/$TAG; mkdir -p "$D"
# shellcheck disable=SC1091
source "$WS/ops/env.sh" >/dev/null 2>&1
unset RR_CAMPAIGN_JOB RR_CAMPAIGN_FAILPOINT
A=$(python3 -I -B - "$WS/ops/FOREIGN_5L_LIFT.held" <<'PY'
import json, sys
base = "0-167,176-191,216-257,261,263-270,272-287,320-334,336-383"
c = set()
for p in base.split(','):
    a, _, b = p.partition('-'); c.update(range(int(a), int(b or a) + 1))
try:
    for x in json.load(open(sys.argv[1])).get('held', []):
        if isinstance(x, int): c.discard(x)
except (FileNotFoundError, ValueError): pass
l = sorted(c); out = []; i = 0
while i < len(l):
    j = i
    while j + 1 < len(l) and l[j + 1] == l[j] + 1: j += 1
    out.append(f"{l[i]}-{l[j]}" if j > i else f"{l[i]}"); i = j + 1
print(",".join(out))
PY
)
printf '%s\n' "$(date -u +%FT%TZ) (implementer:gate13c): unpinned, CPUQuota ${RBC2_QUOTA:-800} %, nice 10 (rb_c2 reproduction $TAG, test binaries in private namespaces)" >> "$WS/ops/CPU_LEDGER.md"
echo "rbc2 $TAG start $(date -u +%FT%TZ) waves $WAVES specs $* cpus $A load $(cut -d' ' -f1 /proc/loadavg)"
for w in $(seq "$WAVES"); do
  pids=(); names=(); k=0
  for spec in "$@"; do
    k=$((k + 1)); lab=${spec%%=*}; bin=${spec#*=}
    T=$S/rt/$TAG.$lab.$w.$k; mkdir -p "$T"
    o=$D/$lab.$w.$k.txt
    if [ "${TRACE:-0}" = 1 ]; then tr=(RR_HUNT_TRACE=1); else tr=(); fi
    env TMPDIR="$T" "${tr[@]}" taskset -c "$A" unshare -Urpf --mount-proc --kill-child -- \
      "$bin" rb_c2_reforking_relay_never_verified_while_it_runs --exact --nocapture --test-threads=1 > "$o" 2>&1 &
    pids+=("$!"); names+=("$lab.$w.$k")
  done
  for i in "${!pids[@]}"; do
    wait "${pids[$i]}"; rc=$?
    o=$D/${names[$i]}.txt
    res=$(grep -m1 -E 'test result:' "$o" | cut -c1-40)
    why=$(grep -m1 -E 'Verified while it runs|still running|panicked' "$o" | cut -c1-160)
    echo "$(date -u +%FT%TZ) ${names[$i]} rc=$rc load=$(cut -d' ' -f1 /proc/loadavg) $res | $why" | tee -a "$D/summary.txt"
  done
done
printf '%s\n' "$(date -u +%FT%TZ) (implementer:gate13c): RELEASED (rb_c2 reproduction $TAG ended)" >> "$WS/ops/CPU_LEDGER.md"
echo "rbc2 $TAG end $(date -u +%FT%TZ)"
