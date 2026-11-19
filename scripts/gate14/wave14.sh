#!/usr/bin/env bash
# gate14: waves of namespaced test runs (each test binary = pid 1 of its OWN unshare -Urpf --mount-proc --kill-child;
# private TMPDIR per run; L10 / L22). Started by the caller as a detached service unit of its own (CPUQuota, nice 10).
# Args: TAG WAVES TEST SPEC...   SPEC = <label>=<binary> (one instance of each SPEC per wave, all in parallel).
# Output: S/logs/w/<TAG>/<label>.<wave>.<k>.txt and one line per run in S/logs/w/<TAG>/summary.txt.
set -u
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate14
TAG=$1; WAVES=$2; TEST=$3; shift 3
D=$S/logs/w/$TAG; mkdir -p "$D"
# shellcheck disable=SC1091
source "$WS/ops/env.sh" >/dev/null 2>&1
unset RR_CAMPAIGN_JOB RR_CAMPAIGN_FAILPOINT
# L21/L23 allowed set minus the held list; AllowedCPUs is not delegated in user units here (LANE_RULES rule 15 note): taskset
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
EXTRA=(); [ "$TEST" = g14_marked_relay_below_a_markerless_subreaper_is_never_verified_while_it_runs ] && EXTRA=(--ignored)
printf '%s\n' "$(date -u +%FT%TZ) (implementer:gate14): unpinned, CPUQuota ${Q:-800} %, nice 10 ($TAG: namespaced test waves)" >> "$WS/ops/CPU_LEDGER.md"
echo "$TAG start $(date -u +%FT%TZ) waves $WAVES test $TEST specs $* load $(cut -d' ' -f1 /proc/loadavg)"
for w in $(seq "$WAVES"); do
  pids=(); names=(); k=0
  for spec in "$@"; do
    k=$((k + 1)); lab=${spec%%=*}; bin=${spec#*=}
    T=$S/rt/w/$TAG.$lab.$w.$k; mkdir -p "$T"
    o=$D/$lab.$w.$k.txt
    env TMPDIR="$T" taskset -c "$A" unshare -Urpf --mount-proc --kill-child -- \
      "$bin" "$TEST" --exact "${EXTRA[@]}" --nocapture --test-threads=1 > "$o" 2>&1 &
    pids+=("$!"); names+=("$lab.$w.$k")
  done
  for i in "${!pids[@]}"; do
    wait "${pids[$i]}"; rc=$?
    o=$D/${names[$i]}.txt
    res=$(grep -m1 -E 'test result:' "$o" | cut -c1-40)
    why=$(grep -m1 -E 'Verified while it runs|still running|panicked|G14 SUMMARY' "$o" | cut -c1-200)
    echo "$(date -u +%FT%TZ) ${names[$i]} rc=$rc load=$(cut -d' ' -f1 /proc/loadavg) $res | $why" >> "$D/summary.txt"
  done
done
printf '%s\n' "$(date -u +%FT%TZ) (implementer:gate14): RELEASED ($TAG ended)" >> "$WS/ops/CPU_LEDGER.md"
echo "$TAG end $(date -u +%FT%TZ)"
