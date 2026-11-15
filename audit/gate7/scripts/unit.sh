#!/usr/bin/env bash
# gate7 unit.sh TAG QUOTA CMD... : run CMD (a namespaced batch driver) as MY OWN transient systemd user unit, unpinned (L23):
# nice 10, CPUQuota QUOTA %, MemoryMax 8G, swap 0, CPUs = allowed list (L21 sets; held list must be empty, else refuse),
# taskset inside (AllowedCPUs has no effect in user units here). Ledger lines with >> only (L13). Blocks until the unit ends.
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
tag=$1; q=$2; shift 2
held=$(python3 -I -c 'import json,sys; print(len(json.load(open(sys.argv[1])).get("held",[])))' $WS/ops/FOREIGN_5L_LIFT.held 2>/dev/null || echo X)
[ "$held" = 0 ] || { echo "unit.sh: held list not empty ($held): refuse" >&2; exit 97; }
[ -e $WS/plan/auto-campaign/implementer_ops/PAUSE ] && { echo "unit.sh: PAUSE" >&2; exit 75; }
ALLOWED=0-167,176-191,216-257,261,263-270,272-287,320-334,336-383
unit=gate7-$tag-$(date +%s)
printf '%s\n' "$(date -u +%FT%TZ) (implementer:gate7): unpinned, CPUQuota $q %, nice 10, MemoryMax 8G, unit $unit (namespaced fake-child audit runs)" >> $WS/ops/CPU_LEDGER.md
systemd-run --user --unit=$unit --wait --pipe --collect -q -p CPUQuota=${q}% -p MemoryMax=8G -p MemorySwapMax=0 \
  -E PATH=$PATH -E HOME=$HOME -E CPUS=$ALLOWED -E XDG_RUNTIME_DIR=/nonexistent-gate7-xdg -- nice -n 10 taskset -c $ALLOWED "$@"
rc=$?
printf '%s\n' "$(date -u +%FT%TZ) (implementer:gate7): unit $unit ended rc $rc (unpinned)" >> $WS/ops/CPU_LEDGER.md
exit $rc
