#!/usr/bin/env bash
# gate17 (copy of gate16 reh16.sh = procfix7 reh_r9b.sh, paths changed only): rehearsal of host_step_fake1.sh --rehearse-ns (the supervisor as a "service" of the in-namespace
# rehearsal fakes of systemd-run / systemctl) as the child of pid 1 of a PRIVATE user+pid+mount namespace (rule L10;
# --kill-child), private /dev/shm and TMPDIR (L22: after env.sh), fake children only; taskset to the L21 base set minus the
# held list. Run as a detached service unit (LANE_RULES rule 5), MemoryMax 8G, CPUQuota 800 %, nice 10.
# Args: TAG ITEMS CASE_MS [CASE]; env HTDIR (the script's directory), RELDIR (the release binaries).
# CASE: plain | stop:<s> (after s seconds the DOCUMENTED stop `systemctl --user stop -- <unit>` through the fake: SIGTERM
#       to the supervisor's exact pid) | killlauncher:<s> (after s seconds SIGTERM to the LAUNCHER script's exact pid, our
#       own child; the "service" runs on; then `host_step_fake1.sh --grade` follows it by name and grades).
set -u
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate17
TAG=$1; ITEMS_=$2; CASE_=$3; KIND=${4:-plain}
: "${HTDIR:?}" "${RELDIR:?}"
# shellcheck disable=SC1091
source "$WS/ops/env.sh" >/dev/null 2>&1
export TMPDIR="$S/tmp/reh17-$TAG"; mkdir -p "$TMPDIR"
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
mkdir -p "$S/host-rehearsal"
printf '%s\n' "$(date -u +%FT%TZ) (implementer:gate17): unpinned, CPUQuota 200 %, nice 10 (host-step rehearsal r9b-$TAG in a private namespace, fake children)" >> "$WS/ops/CPU_LEDGER.md"
echo "reh_r9b $TAG start $(date -u +%FT%TZ) items $ITEMS_ case_ms $CASE_ kind $KIND script $HTDIR rel $RELDIR"
export REL="$RELDIR" OUT="$S/host-rehearsal/r17-$TAG" ITEMS="$ITEMS_" CASE_MS="$CASE_" HTDIR KIND
# shellcheck disable=SC2016 # expanded by the inner bash
taskset -c "$A" unshare -Urpf --mount-proc --kill-child -- bash -c '
  mount -t tmpfs -o size=32m,mode=1777 tmpfs /dev/shm || exit 99
  SCRIPT=$HTDIR/host_step_fake1.sh
  case "$KIND" in
    plain) bash "$SCRIPT" --rehearse-ns; exit $? ;;
    stop:*)
      bash "$SCRIPT" --rehearse-ns & L=$!
      sleep "${KIND#stop:}"
      echo "reh_r9b: the documented stop: systemctl --user stop -- $(cat "$OUT/supervisor.unit") $(date -u +%FT%TZ)"
      PATH="$HTDIR/rehearsal-fake-systemd:$PATH" FAKE_SYSTEMD_DIR="$OUT/fake-systemd" systemctl --user stop -- "$(cat "$OUT/supervisor.unit")"
      wait "$L"; exit $? ;;
    killlauncher:*)
      bash "$SCRIPT" --rehearse-ns & L=$!
      sleep "${KIND#killlauncher:}"
      echo "reh_r9b: SIGTERM to the launcher script (pid $L, our own child) $(date -u +%FT%TZ)"
      kill -TERM "$L"; wait "$L"; echo "reh_r9b: launcher ended rc $?"
      bash "$SCRIPT" --grade; exit $? ;;
  esac'
rc=$?
printf '%s\n' "$(date -u +%FT%TZ) (implementer:gate17): RELEASED (host-step rehearsal r9b-$TAG ended rc $rc)" >> "$WS/ops/CPU_LEDGER.md"
echo "reh_r9b $TAG end $(date -u +%FT%TZ) rc=$rc"
exit $rc
