# gate9 (a) class matrix (g9fault.so): TAG MODE [G8 assignments...] : MODE u = host-like (inside ns8u: the marked daemon is NON-DUMPABLE,
# unreadable to the capability-less supervisor, classified by the lineage rule), r = root-mapped (inside ns8r: readable).
# Session 1 runs jobs whose first launch starts a MARKED daemon (g7hostile ndaemon: double fork, setsid, chdir /), is
# SIGKILLed; session 2 runs under LD_PRELOAD g8fault.so with the given G8_* settings (in G8_MATCH the token @DP@ is the
# daemon's pid, @CDIR@ the campaign directory). Prints RESULT tag rc daemon_alive candidates failed.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/scripts/lib.sh
in_ns_guard6; shm_guard
tag=$1; mode=$2; shift 2
R=$S/runs/inj9/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
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
echo "session 1 SIGKILLed; daemon pid $DP marker $DM alive=$(alive $DP && echo yes || echo no) environ readable=$(head -c1 /proc/$DP/environ >/dev/null 2>&1 && echo yes || echo NO) ppid=$(awk '{print $4}' /proc/$DP/stat)"
sleep 0.5
envs=(LD_PRELOAD=$S/bin/g9fault.so G8_LOG=$R/fault.log)
for a in "$@"; do a=${a//@DP@/$DP}; a=${a//@CDIR@/$R/c}; envs+=("$a"); done
printf '%s\n' "${envs[@]}" > $R/inj.env
env "${envs[@]}" timeout -s KILL ${G8_TIMEOUT:-600} $RC run --spec $R/spec.json --dir $R/c --plain > $R/run2.out 2>&1; rc=$?
alive2=$(alive $DP && echo YES || echo no)
cand=$(grep -c '#' $R/fault.log 2>/dev/null); failed=$(grep -c FAILED $R/fault.log 2>/dev/null)
grep -m3 FAILED $R/fault.log 2>/dev/null | sed 's/^/  fault: /'
grep -h "recovery\|marker sweep\|lineage\|FINISHED\|unproven\|hidden\|survivor\|ERROR\|exit" $R/run2.out | sed 's/^[0-9TZ:.-]* //' | cut -c1-260 | head -12 | sed 's/^/  log: /'
flag=""; [ "$rc" = 0 ] && [ "$alive2" = YES ] && flag=" RC0_SURVIVOR"
echo "RESULT tag=$tag mode=$mode rc=$rc daemon_alive=$alive2 candidates=${cand:-0} failed=${failed:-0}$flag"
alive $DP && kill -KILL $DP
exit 0
