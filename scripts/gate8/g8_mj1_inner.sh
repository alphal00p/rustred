# gate8 (g) MJ-1 variants: TAG VARIANT [MAXJOBS] inside ns8r (root-mapped; the SUPERVISOR is capability-less through
# g7capless, so a non-dumpable process is unreadable to it, while this script keeps root to force pid reuse).
# The first job leaves a NON-DUMPABLE descendant below the supervisor (g7hostile ndorphan, pid file). unproven_end_limit_s 20.
#  none      : nothing done: expect exit 8 naming the pid (after ~120 s hunt + 20 s), then the operator kills it -> 0
#  during    : the hidden process exits 60 s into the hunt (before the limit): expect a normal settle, exit 0, no unproven_end
#  atlimit   : it exits right after the 'end UNPROVEN' line: expect the later proof to settle it, exit 0 (or 8 naming nothing alive?)
#  pidreuse  : it exits after the 'end UNPROVEN' line and a FOREIGN non-dumpable decoy is forked AT ITS PID by a decoy parent
#              that is older than the campaign: expect the decoy never signalled, the attempt settled, exit 0
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate8/scripts/lib.sh
in_ns_guard6; shm_guard
tag=$1; var=$2; mj=${3:-1}
R=$S/runs/mj1/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
hostlike && { echo "needs ns8r"; exit 99; }
cat > $R/wrap.sh <<W
#!$BASH_BIN
if [ "\$1" = job ] && mkdir $R/nd.lock 2>/dev/null; then $S/bin/g7hostile ndorphan $R/nd.pid; fi
exec $FC "\$@"
W
chmod 755 $R/wrap.sh
EXTRA_RES=', "unproven_end_limit_s": 20' P_CRASH=0 P_MEM=0 P_NORULE=0 MAXJOBS=$mj CASE_MS='[3000,5000]' make_spec $R/spec.json $R/c $R/wrap.sh 7 ${ITEMS:-6}
# decoy parent: started BEFORE the campaign (older than every session), forks the decoy at the requested pid on demand
( while [ ! -e $R/decoy.go ]; do sleep 0.1; done; T=$(cat $R/decoy.go)
  for try in $(seq 1 400); do echo $((T-1)) > /proc/sys/kernel/ns_last_pid; env -u RR_CAMPAIGN_JOB $S/bin/g7hostile ndself $R/decoy.pid & d=$!; sleep 0.05
    if [ "$d" = "$T" ]; then echo "decoy forked at the reused pid $T (try $try)" > $R/decoy.log; wait; exit 0; fi; kill -KILL $d; wait $d 2>/dev/null; : > $R/decoy.pid; done
  echo "pid reuse failed" > $R/decoy.log ) & DPAR=$!
sleep 1.2
$S/bin/g7capless $RC run --spec $R/spec.json --dir $R/c --plain > $R/run1.out 2>&1 & SUP=$!
for i in $(seq 1 600); do [ -s $R/nd.pid ] && break; sleep 0.1; done; ND=$(cat $R/nd.pid); NDS=$(starttime $ND)
echo "hidden pid $ND (start $NDS) ppid $(awk '{print $4}' /proc/$ND/stat) environ readable by the capless supervisor: by design no"
t0=$(date +%s)
case $var in
  during) sleep 60; kill -KILL $ND; echo "hidden process killed at +$(( $(date +%s)-t0 ))s (before the limit)";;
  atlimit|pidreuse) for i in $(seq 1 3000); do grep -q 'end UNPROVEN' $R/run1.out && break; alive $SUP || break; sleep 0.1; done
     echo "'end UNPROVEN' seen at +$(( $(date +%s)-t0 ))s: $(grep -m1 'end UNPROVEN' $R/run1.out | cut -c1-220)"
     kill -KILL $ND; while alive $ND; do sleep 0.01; done
     [ $var = pidreuse ] && { echo $ND > $R/decoy.go; for i in $(seq 1 200); do [ -s $R/decoy.log ] && break; sleep 0.1; done; cat $R/decoy.log; };;
esac
# sample running jobs while the attempt is held (max_jobs accounting)
( while alive $SUP; do echo "$(date +%s) $(ls -d $R/c/jobs/*/ 2>/dev/null | wc -l) $(for p in /proc/[0-9]*; do tr '\0' '\n' < $p/environ 2>/dev/null | grep -q "^RR_CAMPAIGN_JOB=" && tr '\0' '\n' < $p/environ 2>/dev/null | grep -m1 '^RR_CAMPAIGN_JOB=' ; done | sort -u | wc -l)" >> $R/slots.log; sleep 2; done ) &
wait $SUP; rc=$?
echo "session 1 rc=$rc after $(( $(date +%s)-t0 ))s"
grep -h 'end UNPROVEN\|unproven_end\|CPU lease\|operator decision\|FINISHED\|unverified_end_cleared\|exit' $R/run1.out | sed 's/^[0-9TZ:.-]* //' | cut -c1-230 | head -8 | sed 's/^/  log: /'
DEC=$(cat $R/decoy.pid 2>/dev/null)
if [ -n "$DEC" ]; then echo "decoy $DEC alive=$(alive $DEC && echo yes || echo NO) start $(starttime $DEC) (hidden was start $NDS); signals naming it in the ledger: $(grep -c "pid\":$DEC," $R/c/ledger/*.jsonl)"; fi
if [ $rc = 8 ]; then
  if alive $ND && [ "$(starttime $ND)" = "$NDS" ]; then grep -q "$ND" $R/run1.out && echo "  exit 8 names the hidden pid $ND: operator kills it" || echo "  EXIT 8 DOES NOT NAME $ND"; kill -KILL $ND; fi
  $S/bin/g7capless $RC run --spec $R/spec.json --dir $R/c --plain > $R/run2.out 2>&1; rc2=$?; echo "session 2 (after the operator step) rc=$rc2"
fi
echo "max concurrently marked jobs while running (from slots.log): $(awk '{print $3}' $R/slots.log | sort -n | tail -1)"
echo "RESULT tag=$tag var=$var max_jobs=$mj rc=$rc rc2=${rc2:--} hidden_alive=$(alive $ND && [ "$(starttime $ND)" = "$NDS" ] && echo YES || echo no) decoy_alive=$( [ -n "$DEC" ] && { alive $DEC && echo yes || echo NO; } || echo -)"
$RC verify $R/c > /dev/null 2>&1; echo "verify rc=$?"
[ -n "$DEC" ] && alive $DEC && kill -KILL $DEC; kill $DPAR 2>/dev/null; wait $DPAR 2>/dev/null
exit 0
