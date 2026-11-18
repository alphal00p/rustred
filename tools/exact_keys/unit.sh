#!/usr/bin/env bash
# Start one DETACHED service unit that runs a command inside a private user+pid namespace, with the workspace environment
# sourced FIRST and then a private TMPDIR (lane rules L22 / LANE_RULES 5), pinned by taskset to the allowed CPU list minus the
# held list (read at launch), unpinned otherwise (L23). Generic: every name, cap and path is an argument.
#
# usage: unit.sh --unit NAME --mem GB --cpu-quota PCT --run-dir DIR --lane LANE [--pause-file F --pause-account A]
#                [--need-gb N] -- <command and args>
# Gates (refuse, rc 75): the pause flag naming this account, DISK_LOW when --writes-gb >= 1, host MemAvailable - need < floor.
set -u
WS=${WS:-/shared/vhirschi/t3code/vakint_dev/p_integral_dev}
unit= mem= quota= run= lane= pause= acct= need=0 floor=75 writes=0
while [ $# -gt 0 ]; do
  case $1 in
    --unit) unit=$2; shift 2;; --mem) mem=$2; shift 2;; --cpu-quota) quota=$2; shift 2;; --run-dir) run=$2; shift 2;;
    --lane) lane=$2; shift 2;; --pause-file) pause=$2; shift 2;; --pause-account) acct=$2; shift 2;;
    --need-gb) need=$2; shift 2;; --floor-gb) floor=$2; shift 2;; --writes-gb) writes=$2; shift 2;;
    --) shift; break;; *) echo "unit.sh: unknown $1" >&2; exit 2;;
  esac
done
[ -n "$unit" ] && [ -n "$mem" ] && [ -n "$quota" ] && [ -n "$run" ] && [ -n "$lane" ] && [ $# -gt 0 ] || { echo "unit.sh: missing argument" >&2; exit 2; }
if [ -n "$pause" ] && [ -e "$pause" ]; then
  if [ -z "$acct" ] || grep -q -- "$acct" "$pause"; then echo "unit.sh: pause flag $pause names this account" >&2; exit 75; fi
fi
if [ "$writes" -ge 1 ] && [ -e "$WS/ops/DISK_LOW" ]; then echo "unit.sh: DISK_LOW" >&2; exit 75; fi
avail=$(awk '/MemAvailable/ {print int($2 / 1048576)}' /proc/meminfo)
if [ $((avail - need)) -lt "$floor" ]; then echo "unit.sh: available $avail GB - need $need GB < floor $floor GB" >&2; exit 75; fi
base="0-167,176-191,216-257,261,263-270,272-287,320-334,336-383"
held=$(python3 -I -c 'import json,sys; print(",".join(str(x) for x in json.load(open(sys.argv[1])).get("held", [])))' "$WS/ops/FOREIGN_5L_LIFT.held" 2>/dev/null || true)
cpus=$(python3 -I -c '
import sys
def ex(s): return {c for p in s.split(",") if p for c in (range(int(p.split("-")[0]), int(p.split("-")[-1]) + 1))}
a = sorted(ex(sys.argv[1]) - ex(sys.argv[2]))
out, i = [], 0
while i < len(a):
    j = i
    while j + 1 < len(a) and a[j + 1] == a[j] + 1: j += 1
    out.append(f"{a[i]}-{a[j]}" if j > i else f"{a[i]}"); i = j + 1
print(",".join(out))' "$base" "$held")
mkdir -p "$run/tmp" "$run/log"
inner=$run/log/$unit.inner.sh
{
  echo '#!/usr/bin/env bash'
  echo 'set -u'
  echo "source $WS/ops/env.sh >/dev/null 2>&1"
  echo "export TMPDIR=$run/tmp"
  echo 'export PATH=/run/current-system/sw/bin:$PATH'
  echo "echo \"\$(date -u +%FT%TZ) unit $unit start (namespace pid \$\$, TMPDIR \$TMPDIR)\""
  printf 'exec taskset -c %s nice -n 10' "$cpus"; printf ' %q' "$@"; echo
} > "$inner"
chmod +x "$inner"
systemd-run --user --unit="$unit" -p MemoryMax="${mem}G" -p MemorySwapMax=0 -p CPUQuota="${quota}%" -p OOMPolicy=continue -p TimeoutStopSec=${stop_s:-30} \
  -p Nice=10 --property=StandardOutput=append:"$run/log/$unit.out" --property=StandardError=append:"$run/log/$unit.err" \
  /run/current-system/sw/bin/unshare -Urpf --mount-proc -- /run/current-system/sw/bin/bash "$inner" || exit $?
printf '%s (implementer:%s): + UNPINNED CPUQuota %s%% nice 10 -> unit %s (MemoryMax %sG, taskset %s). Released at its end.\n' \
  "$(date -u +%FT%TZ)" "$lane" "$quota" "$unit" "$mem" "$cpus" >> "$WS/ops/CPU_LEDGER.md"
echo "started $unit (MemoryMax ${mem}G, CPUQuota ${quota}%, cpus $cpus)"
