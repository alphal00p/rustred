# gate11 (a)(1) LIVE listing attack: TAG MODE HELPER [G11 assignments...]: ONE session under LD_PRELOAD g11filter.so whose
# first job starts HELPER (daemon / thread / zleader as in g11_drop_inner.sh); the wrapper writes the MARKED pid to
# $R/drop.txt (the filter re-reads it at every listing: G11_DROP_FILE=@DPFILE@). The run must kill it or end non-zero.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate12/scripts/lib.sh
in_ns_guard6; shm_guard
tag=$1; mode=$2; helper=$3; shift 3
R=$S/runs/droplive12/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
case $mode in u) hostlike || { echo "NOT HOST-LIKE: abort"; exit 99; };; r) hostlike && { echo "HOST-LIKE in r mode: abort"; exit 99; };; esac
case $helper in
  daemon)  start="$S/bin/g7hostile ndaemon $R/daemon.pids" ;;
  thread)  start="env -i $S/bin/g11thr $R/daemon.pids \"\$RR_CAMPAIGN_JOB\" thread &" ;;
  zleader) start="env -i $S/bin/g11thr $R/daemon.pids \"\$RR_CAMPAIGN_JOB\" zleader &" ;;
  g12:*) IFS=: read -r _ gm ga <<< "$helper"; start="env -i $S/bin/g12thr $R/daemon.pids \"\$RR_CAMPAIGN_JOB\" $gm $ga &" ;;
  *) echo "helper?"; exit 2 ;;
esac
{
  echo "#!$BASH_BIN"
  echo "if [ \"\$1\" = job ] && [ ! -e $R/started ]; then"
  echo "  : > $R/started"
  echo "  $start"
  echo "  for i in \$(seq 1 50); do [ -s $R/daemon.pids ] && break; sleep 0.1; done"
  echo "  cut -d' ' -f1 $R/daemon.pids > $R/drop.txt.tmp && mv $R/drop.txt.tmp $R/drop.txt"
  echo "fi"
  echo "exec $FC \"\$@\""
} > $R/wrap.sh
chmod 755 $R/wrap.sh
P_CRASH=0 P_MEM=0 P_NORULE=0 CASE_MS="${G8_CASE_MS:-[1500,2500]}" MAXJOBS=${G8_MAXJOBS:-2} make_spec $R/spec.json $R/c $R/wrap.sh 5 ${G8_ITEMS:-4}
envs=(LD_PRELOAD=$S/bin/g11filter.so G11_LOG=$R/filter.log)
for a in "$@"; do a=${a//@DPFILE@/$R/drop.txt}; envs+=("$a"); done
env "${envs[@]}" timeout -s KILL ${G8_TIMEOUT:-600} $RC run --spec $R/spec.json --dir $R/c --plain > $R/run1.out 2>&1; rc=$?
DP=""; [ -s $R/drop.txt ] && read DP < $R/drop.txt
alive2=no; [ -n "$DP" ] && alive $DP && alive2=YES
nf=$(grep -c 'dropped \[[0-9]' $R/filter.log 2>/dev/null)
grep -h "incomplete\|FINISHED\|unproven\|vanished\|descendant\|ERROR\|exit" $R/run1.out | sed 's/^[0-9TZ:.-]* //' | cut -c1-240 | head -6 | sed 's/^/  log: /'
flag=""; [ "$rc" = 0 ] && [ "$alive2" = YES ] && flag=" RC0_SURVIVOR"
echo "RESULT tag=$tag mode=$mode helper=$helper rc=$rc marked_alive=$alive2 marked=${DP:-none} filtered_listings=${nf:-0}$flag"
[ -n "$DP" ] && alive $DP && kill -KILL $DP
exit 0
