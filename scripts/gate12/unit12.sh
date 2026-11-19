#!/usr/bin/env bash
# gate12 unit12.sh TAG QUOTA CMD... : run CMD as MY OWN transient systemd user unit (unpinned, L23): nice 10, CPUQuota QUOTA %,
# MemoryMax ${MEMMAX:-4G} (<= 8G), swap 0, AllowedCPUs = the L21 base set minus the held list (refuses when held is not
# empty), ledger lines with >> only (L13). Refuses unless MemAvailable - MemoryMax >= 75 GB and fewer than 2 gate12-* units
# run. Blocks until the unit ends. Unit name gate12-TAG-<epoch> (printed first).
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
tag=$1; q=$2; shift 2
mm=${MEMMAX:-4G}; mmg=${mm%G}
[ "$mmg" -le 8 ] || { echo "unit12.sh: cap > 8G refused" >&2; exit 95; }
held=$(python3 -I -c 'import json,sys; print(len(json.load(open(sys.argv[1])).get("held",[])))' $WS/ops/FOREIGN_5L_LIFT.held 2>/dev/null || echo X)
[ "$held" = 0 ] || { echo "unit12.sh: held list not empty ($held): refuse" >&2; exit 97; }
[ -e $WS/plan/auto-campaign/implementer_ops/PAUSE ] && { echo "unit12.sh: PAUSE" >&2; exit 75; }
avail=$(awk '/MemAvailable/{print int($2/1048576)}' /proc/meminfo); [ $((avail - mmg)) -ge 75 ] || { echo "unit12.sh: MemAvailable ${avail} GB - cap ${mmg} < 75: refuse" >&2; exit 98; }
running=$(systemctl --user list-units --no-legend --state=running 'gate12-*' 2>/dev/null | wc -l)
[ "$running" -lt 2 ] || { echo "unit12.sh: $running gate12 units running: refuse (max 2)" >&2; exit 96; }
ALLOWED=0-167,176-191,216-257,261,263-270,272-287,320-334,336-383
unit=gate12-$tag-$(date +%s)
echo "unit $unit"
printf '%s\n' "$(date -u +%FT%TZ) (implementer:gate12): unpinned, CPUQuota $q %, nice 10, MemoryMax $mm, unit $unit (namespaced fake-child audit runs)" >> $WS/ops/CPU_LEDGER.md
systemd-run --user --unit=$unit --wait --pipe --collect -q -p CPUQuota=${q}% -p MemoryMax=$mm -p MemorySwapMax=0 -p AllowedCPUs=$ALLOWED ${EXTRA_PROPS:-} \
  -E PATH=$PATH -E HOME=$HOME -E CPUS=$ALLOWED -E XDG_RUNTIME_DIR=/nonexistent-gate12-xdg -- nice -n 10 taskset -c $ALLOWED "$@"
rc=$?
printf '%s\n' "$(date -u +%FT%TZ) (implementer:gate12): unit $unit ended rc $rc (unpinned)" >> $WS/ops/CPU_LEDGER.md
exit $rc
