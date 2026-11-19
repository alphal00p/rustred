#!/usr/bin/env bash
# gate8 unit8.sh TAG QUOTA CMD... : run CMD (a namespaced batch driver) as MY OWN transient systemd user unit, unpinned
# (L23): nice 10, CPUQuota QUOTA %, MemoryMax 8G, swap 0, CPUs = the allowed list (L21 sets minus the held list), ledger
# lines with >> only (L13). Blocks until the unit ends. Unit name gate8-TAG-<epoch> (printed first).
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
tag=$1; q=$2; shift 2
held=$(python3 -I -c 'import json,sys; print(len(json.load(open(sys.argv[1])).get("held",[])))' $WS/ops/FOREIGN_5L_LIFT.held 2>/dev/null || echo X)
[ "$held" = 0 ] || { echo "unit8.sh: held list not empty ($held): refuse" >&2; exit 97; }
[ -e $WS/plan/auto-campaign/implementer_ops/PAUSE ] && { echo "unit8.sh: PAUSE" >&2; exit 75; }
avail=$(awk '/MemAvailable/{print int($2/1048576)}' /proc/meminfo); [ "$avail" -ge 100 ] || { echo "unit8.sh: MemAvailable ${avail} GB < 100: refuse" >&2; exit 98; }
ALLOWED=0-167,176-191,216-257,261,263-270,272-287,320-334,336-383
unit=gate9-$tag-$(date +%s)
echo "unit $unit"
printf '%s\n' "$(date -u +%FT%TZ) (implementer:gate9): unpinned, CPUQuota $q %, nice 10, MemoryMax ${MEMMAX:-8G}, unit $unit (namespaced fake-child audit runs)" >> $WS/ops/CPU_LEDGER.md
systemd-run --user --unit=$unit --wait --pipe --collect -q -p CPUQuota=${q}% -p MemoryMax=${MEMMAX:-8G} -p MemorySwapMax=0 \
  -E PATH=$PATH -E HOME=$HOME -E CPUS=$ALLOWED -E XDG_RUNTIME_DIR=/nonexistent-gate9-xdg -- nice -n 10 taskset -c $ALLOWED "$@"
rc=$?
printf '%s\n' "$(date -u +%FT%TZ) (implementer:gate9): unit $unit ended rc $rc (unpinned)" >> $WS/ops/CPU_LEDGER.md
exit $rc
