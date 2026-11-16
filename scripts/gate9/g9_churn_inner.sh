# gate9 (a) zombie-leader thread churn: TAG MODE(crash|live) LIFE_US [nozombie]: a job starts g9churn (marked, setsid'd,
# leader a ZOMBIE, one short-lived thread at a time). crash: session 1 SIGKILLed, session 2 must kill it or end non-zero.
# live: one session must kill it (its hunt / sweeps) or end non-zero. Prints RESULT tag rc churn_alive.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/scripts/lib.sh
in_ns_guard6; shm_guard
tag=$1; mode=$2; life=$3; nz=${4:-}
R=$S/runs/churn/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
cat > $R/wrap.sh <<W
#!$BASH_BIN
if [ "\$1" = job ] && [ ! -e $R/churn.pid ]; then $S/bin/g9churn $R/churn.pid $life $nz; fi
exec $FC "\$@"
W
chmod 755 $R/wrap.sh
P_CRASH=0 P_MEM=0 P_NORULE=0 CASE_MS='[1500,2500]' MAXJOBS=2 make_spec $R/spec.json $R/c $R/wrap.sh 5 4
st() { awk '{print $3}' /proc/$1/stat 2>/dev/null; }
if [ $mode = crash ]; then
  $RC run --spec $R/spec.json --dir $R/c --plain > $R/run1.out 2>&1 & SUP=$!
  for i in $(seq 1 300); do [ -s $R/churn.pid ] && break; sleep 0.1; done
  sleep 0.5; kill -KILL $SUP; wait $SUP 2>/dev/null
  CP=$(cat $R/churn.pid); echo "session 1 SIGKILLed; churner $CP state $(st $CP) threads $(ls /proc/$CP/task 2>/dev/null | wc -l) marker $(tr '\0' '\n' < /proc/$CP/task/$(ls /proc/$CP/task | tail -1)/environ 2>/dev/null | grep -c RR_CAMPAIGN_JOB)"
  timeout -s KILL 900 $RC run --spec $R/spec.json --dir $R/c --plain > $R/run2.out 2>&1; rc=$?; out=$R/run2.out
else
  timeout -s KILL 900 $RC run --spec $R/spec.json --dir $R/c --plain > $R/run1.out 2>&1; rc=$?; out=$R/run1.out
  CP=$(cat $R/churn.pid 2>/dev/null)
fi
alive2=no; [ -n "$CP" ] && [ -d /proc/$CP ] && [ "$(ls /proc/$CP/task 2>/dev/null | wc -l)" -gt 1 ] && alive2=YES
grep -h "marker sweep\|orphan killed\|not proven\|FINISHED\|unproven\|Attention\|leftover" $out | sed 's/^[0-9TZ:.-]* //' | cut -c1-200 | head -8 | sed 's/^/  log: /'
flag=""; [ "$rc" = 0 ] && [ "$alive2" = YES ] && flag=" RC0_SURVIVOR"
echo "RESULT tag=$tag mode=$mode life=$life $nz rc=$rc churn_alive=$alive2 (pid $CP state $(st $CP) threads $(ls /proc/$CP/task 2>/dev/null | wc -l))$flag"
exit 0
