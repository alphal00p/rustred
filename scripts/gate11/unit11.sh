#!/usr/bin/env bash
# gate11 unit10.sh TAG QUOTA CMD... : run CMD as MY OWN transient systemd user unit (unpinned, L23): nice 10, CPUQuota QUOTA %,
# MemoryMax ${MEMMAX:-4G} (<= 8G), swap 0, CPUs = the allowed list (L21 sets; refuses when the held list is not empty),
# ledger lines with >> only (L13). RAM rule of the brief: refuses unless MemAvailable - MemoryMax >= 75 GB; at most 2 units of
# mine at once (counts running gate11-* units). Blocks until the unit ends. Unit name gate11-TAG-<epoch> (printed first).
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
tag=$1; q=$2; shift 2
mm=${MEMMAX:-4G}; mmg=${mm%G}
held=$(python3 -I -c 'import json,sys; print(len(json.load(open(sys.argv[1])).get("held",[])))' $WS/ops/FOREIGN_5L_LIFT.held 2>/dev/null || echo X)
[ "$held" = 0 ] || { echo "unit10.sh: held list not empty ($held): refuse" >&2; exit 97; }
[ -e $WS/plan/auto-campaign/implementer_ops/PAUSE ] && { echo "unit10.sh: PAUSE" >&2; exit 75; }
avail=$(awk '/MemAvailable/{print int($2/1048576)}' /proc/meminfo); [ $((avail - mmg)) -ge 75 ] || { echo "unit10.sh: MemAvailable ${avail} GB - cap ${mmg} < 75: refuse" >&2; exit 98; }
running=$(systemctl --user list-units --no-legend --state=running 'gate11-*' 2>/dev/null | wc -l)
[ "${GATE11_SLOT_OK:-0}" = 1 ] || [ "$running" -lt 2 ] || { echo "unit10.sh: $running gate11 units running: refuse (max 2)" >&2; exit 96; }
ALLOWED=0-167,176-191,216-257,261,263-270,272-287,320-334,336-383
unit=gate11-$tag-$(date +%s)
echo "unit $unit"
printf '%s\n' "$(date -u +%FT%TZ) (implementer:gate11): unpinned, CPUQuota $q %, nice 10, MemoryMax $mm, unit $unit (namespaced fake-child audit runs)" >> $WS/ops/CPU_LEDGER.md
systemd-run --user --unit=$unit --wait --pipe --collect -q -p CPUQuota=${q}% -p MemoryMax=$mm -p MemorySwapMax=0 ${EXTRA_PROPS:-} \
  -E PATH=$PATH -E HOME=$HOME -E CPUS=$ALLOWED -E XDG_RUNTIME_DIR=/nonexistent-gate11-xdg -- nice -n 10 taskset -c $ALLOWED "$@"
rc=$?
printf '%s\n' "$(date -u +%FT%TZ) (implementer:gate11): unit $unit ended rc $rc (unpinned)" >> $WS/ops/CPU_LEDGER.md
exit $rc
