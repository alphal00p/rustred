# gate9 (a) live-run class matrix: ONE session under LD_PRELOAD g9fault.so; its first job starts a MARKED daemon (g7hostile
# ndaemon: double fork, setsid, chdir /, non-dumpable); the run must kill it, or end non-zero. TAG MODE [G8 assignments...]
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/scripts/lib.sh
in_ns_guard6; shm_guard
tag=$1; mode=$2; shift 2
R=$S/runs/live9/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
case $mode in u) hostlike || { echo "NOT HOST-LIKE: abort"; exit 99; };; r) hostlike && { echo "HOST-LIKE in r mode: abort"; exit 99; };; esac
cat > $R/wrap.sh <<W
#!$BASH_BIN
if [ "\$1" = job ] && [ ! -e $R/daemon.pids ]; then $S/bin/g7hostile ndaemon $R/daemon.pids; fi
exec $FC "\$@"
W
chmod 755 $R/wrap.sh
P_CRASH=0 P_MEM=0 P_NORULE=0 CASE_MS="${G8_CASE_MS:-[1500,2500]}" MAXJOBS=${G8_MAXJOBS:-2} make_spec $R/spec.json $R/c $R/wrap.sh 5 ${G8_ITEMS:-4}
envs=(LD_PRELOAD=$S/bin/g9fault.so G8_LOG=$R/fault.log)
for a in "$@"; do a=${a//@CDIR@/$R/c}; envs+=("$a"); done
env "${envs[@]}" timeout -s KILL ${G8_TIMEOUT:-600} $RC run --spec $R/spec.json --dir $R/c --plain > $R/run1.out 2>&1; rc=$?
DP=""; [ -s $R/daemon.pids ] && read DP DM < $R/daemon.pids
alive2=no; [ -n "$DP" ] && alive $DP && alive2=YES
failed=$(grep -c FAILED $R/fault.log 2>/dev/null)
flag=""; [ "$rc" = 0 ] && [ "$alive2" = YES ] && flag=" RC0_SURVIVOR"
echo "RESULT tag=$tag mode=$mode rc=$rc daemon_alive=$alive2 daemon=${DP:-none} failed=${failed:-0}$flag"
[ -n "$DP" ] && alive $DP && kill -KILL $DP
exit 0
