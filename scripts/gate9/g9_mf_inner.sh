# gate9 (a) third member? "manifest.exists() is false" on a STAT ERROR -> the supervisor CREATES a new ledger over an existing
# campaign. TAG MODE [G8 assignments...]: session 1 runs jobs whose first launch starts a MARKED daemon (g7hostile ndaemon), is
# SIGKILLed; session 2 runs under LD_PRELOAD g9fault.so (@CDIR@ = the campaign directory). Prints RESULT tag rc daemon_alive.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/scripts/lib.sh
in_ns_guard6; shm_guard
tag=$1; mode=$2; shift 2
R=$S/runs/mf/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
case $mode in u) hostlike || { echo "NOT HOST-LIKE: abort"; exit 99; };; r) hostlike && { echo "HOST-LIKE in r mode: abort"; exit 99; };; esac
cat > $R/wrap.sh <<W
#!$BASH_BIN
if [ "\$1" = job ] && [ ! -e $R/daemon.pids ]; then $S/bin/g7hostile ndaemon $R/daemon.pids; fi
exec $FC "\$@"
W
chmod 755 $R/wrap.sh
P_CRASH=0 P_MEM=0 P_NORULE=0 CASE_MS="${G8_CASE_MS:-[1500,2500]}" MAXJOBS=${G8_MAXJOBS:-2} make_spec $R/spec.json $R/c $R/wrap.sh 5 ${G8_ITEMS:-4}
$RC run --spec $R/spec.json --dir $R/c --plain > $R/run1.out 2>&1 & SUP=$!
for i in $(seq 1 300); do [ -s $R/daemon.pids ] && break; sleep 0.1; done
sleep 0.5; kill -KILL $SUP; wait $SUP 2>/dev/null
read DP DM < $R/daemon.pids
seq1=$(ls $R/c/ledger/ 2>/dev/null | tr '\n' ' ')
echo "session 1 SIGKILLed; daemon pid $DP marker $DM alive=$(alive $DP && echo yes || echo no); ledger files: $seq1"
cp -a $R/c/ledger $R/ledger.before
sleep 0.5
envs=(LD_PRELOAD=$S/bin/g9fault.so G8_LOG=$R/fault.log)
for a in "$@"; do a=${a//@DP@/$DP}; a=${a//@CDIR@/$R/c}; envs+=("$a"); done
printf '%s\n' "${envs[@]}" > $R/inj.env
env "${envs[@]}" timeout -s KILL ${G8_TIMEOUT:-600} $RC run --spec $R/spec.json --dir $R/c --plain > $R/run2.out 2>&1; rc=$?
alive2=$(alive $DP && echo YES || echo no)
grep FAILED $R/fault.log 2>/dev/null | head -5 | sed 's/^/  fault: /'
grep -h "ledger\|recovery\|marker sweep\|lineage\|FINISHED\|unproven\|ERROR\|moved aside\|not this campaign" $R/run2.out | sed 's/^[0-9TZ:.-]* //' | cut -c1-240 | head -14 | sed 's/^/  log: /'
echo "  ledger after: $(ls $R/c/ledger | tr '\n' ' ')"
echo "  records before: $(cat $R/ledger.before/segment-*.jsonl | wc -l) after: $(cat $R/c/ledger/segment-*.jsonl | wc -l); SupervisorStarted after: $(grep -c supervisor_started $R/c/ledger/segment-*.jsonl | tr '\n' ' ')"
flag=""; [ "$rc" = 0 ] && [ "$alive2" = YES ] && flag=" RC0_SURVIVOR"
echo "RESULT tag=$tag mode=$mode rc=$rc daemon_alive=$alive2$flag"
alive $DP && kill -KILL $DP
exit 0
