#!/usr/bin/env bash
# withcpus.sh N MIN TAG CMD... : take up to N CPUs from the shared picker (retry until >= MIN), non-blocking flock on each
# ops/locks/cpu.<n> (held by THIS process's fds for the whole command), append-only ledger lines, run CMD with CPUS set.
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
n=$1; min=$2; tag=$3; shift 3
got=()
for try in $(seq 1 400); do
  [ -e $WS/plan/auto-campaign/implementer_ops/PAUSE ] && { echo "PAUSE flag" >&2; exit 75; }
  list=$(python3 -I $WS/ops/pick_free.py $n 2>/dev/null | head -1)
  for c in ${list//,/ }; do
    exec {fd}>"$WS/ops/locks/cpu.$c" || continue
    if flock -n $fd; then got+=($c); else exec {fd}>&-; fi
    [ ${#got[@]} -ge $n ] && break
  done
  [ ${#got[@]} -ge $min ] && break
  sleep 15
done
[ ${#got[@]} -ge $min ] || { echo "withcpus: no CPUs after retries" >&2; exit 98; }
CPUS=$(IFS=,; echo "${got[*]}")
printf '%s\n' "$(date -u +%FT%TZ) (implementer gate6): + CPUs $CPUS (flock) -> $tag (namespaced fake-child audit runs). Released at its end." >> $WS/ops/CPU_LEDGER.md
echo "withcpus: $tag on CPUs $CPUS" >&2
CPUS=$CPUS "$@"; rc=$?
printf '%s\n' "$(date -u +%FT%TZ) (implementer gate6): RELEASED $CPUS ($tag ended rc $rc)" >> $WS/ops/CPU_LEDGER.md
exit $rc
