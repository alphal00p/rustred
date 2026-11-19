#!/usr/bin/env bash
# gate13b item 7: the FROZEN host step WS/bin/frozen/20261117T1806Z-host-step (read-only, unchanged) in --rehearse-ns, in a
# PRIVATE user+pid+mount namespace (rule L10; --kill-child), private /dev/shm and TMPDIR (L22: after env.sh), fake
# children only, taskset to the L21 base set minus the held list. Started as a detached service unit (LANE_RULES rule 5).
# Args: TAG ITEMS CASE_MS. OUT = S/reh/TAG.
set -u
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
F=$WS/bin/frozen/20261117T1806Z-host-step
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate13b
TAG=$1; ITEMS_=$2; CASE_=$3
# shellcheck disable=SC1091
source "$WS/ops/env.sh" >/dev/null 2>&1
export TMPDIR="$S/reh/$TAG.tmp"; mkdir -p "$TMPDIR"
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
printf '%s\n' "$(date -u +%FT%TZ) (implementer:gate13b): unpinned, CPUQuota 800 %, nice 10 (frozen 1806Z host-step rehearsal $TAG in a private namespace, fake children)" >> "$WS/ops/CPU_LEDGER.md"
echo "reh13b $TAG start $(date -u +%FT%TZ) items $ITEMS_ case_ms $CASE_ TMPDIR $TMPDIR cpus $A"
export REL="$F/bin" OUT="$S/reh/$TAG" ITEMS="$ITEMS_" CASE_MS="$CASE_"
# shellcheck disable=SC2016 # $0 is expanded by the inner bash (the script path)
taskset -c "$A" unshare -Urpf --mount-proc --kill-child -- \
  bash -c 'mount -t tmpfs -o size=32m,mode=1777 tmpfs /dev/shm && exec bash "$0" --rehearse-ns' "$F/host_step_fake1.sh"
rc=$?
printf '%s\n' "$(date -u +%FT%TZ) (implementer:gate13b): RELEASED (frozen 1806Z host-step rehearsal $TAG ended rc $rc)" >> "$WS/ops/CPU_LEDGER.md"
echo "reh13b $TAG end $(date -u +%FT%TZ) rc=$rc"
exit $rc
