# gate12 item 1 (copy of gate11 g11_drop_inner.sh + g12thr helpers g12:MODE:ARG) LISTING ATTACKS: TAG MODE HELPER [G11 assignments...]
# Runs ONLY inside a private user+pid+mount namespace (ns8r.sh root-mapped r / ns8u.sh host-like u); refuses otherwise.
# Session 1 runs jobs whose FIRST launch starts HELPER, is SIGKILLed; session 2 runs under LD_PRELOAD g11filter.so with the
# given G11_* settings (the token @DP@ in them is the MARKED helper pid, @PP@ its parent; @DPFILE@ a file holding @DP@).
#   HELPER daemon  : g7hostile ndaemon (marked, double fork, setsid, non-dumpable; re-parented to the namespace's init:
#                    REACHED by the children walk from pid 1)
#   HELPER thread  : env -i g11thr (UNMARKED parent, setsid; its NON-LEADER thread forks a MARKED sleep: NOT reached)
#   HELPER zleader : env -i g11thr zleader (unmarked SUBREAPER whose leader exits; the orphaned MARKED sleep is re-parented
#                    to its live non-leader thread: NOT reached)
# Prints RESULT tag rc marked_alive dropped-listings records LEDGER_OK|CHANGED [RC0_SURVIVOR].
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate12/scripts/lib.sh
in_ns_guard6; shm_guard
tag=$1; mode=$2; helper=$3; shift 3
R=$S/runs/drop12/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
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
  echo "fi"
  echo "exec $FC \"\$@\""
} > $R/wrap.sh
chmod 755 $R/wrap.sh
P_CRASH=0 P_MEM=0 P_NORULE=0 CASE_MS="${G8_CASE_MS:-[1500,2500]}" MAXJOBS=${G8_MAXJOBS:-2} make_spec $R/spec.json $R/c $R/wrap.sh 5 ${G8_ITEMS:-4}
$RC run --spec $R/spec.json --dir $R/c --plain > $R/run1.out 2>&1 & SUP=$!
for i in $(seq 1 300); do [ -s $R/daemon.pids ] && break; sleep 0.1; done
sleep 0.5; kill -KILL $SUP; wait $SUP 2>/dev/null
read DP PP < $R/daemon.pids
[ "$helper" = daemon ] && PP=$(awk '{print $4}' /proc/$DP/stat 2>/dev/null)
echo "$DP" > $R/drop.txt
echo "session 1 SIGKILLed; marked pid $DP (parent $PP) alive=$(alive $DP && echo yes || echo no) marker=$(tr '\0' '\n' < /proc/$DP/environ 2>/dev/null | grep -c RR_CAMPAIGN_JOB) leader-walk-from-1-reaches=$( (cat /proc/1/task/1/children; cat /proc/$PP/task/$PP/children) 2>/dev/null | tr ' ' '\n' | grep -cx "$DP")"
sleep 0.5
envs=(LD_PRELOAD=$S/bin/g11filter.so G11_LOG=$R/filter.log)
for a in "$@"; do a=${a//@DP@/$DP}; a=${a//@PP@/$PP}; a=${a//@DPFILE@/$R/drop.txt}; envs+=("$a"); done
printf '%s\n' "${envs[@]}" > $R/inj.env
python3 -I $S/scripts/ledger_lines.py $R/c > $R/before.txt
env "${envs[@]}" timeout -s KILL ${G8_TIMEOUT:-600} $RC run --spec $R/spec.json --dir $R/c --plain > $R/run2.out 2>&1; rc=$?
alive2=$(alive $DP && echo YES || echo no)
nf=$(grep -c 'dropped \[[0-9]' $R/filter.log 2>/dev/null); nl=$(grep -c . $R/filter.log 2>/dev/null)
grep -h "recovery\|marker sweep\|incomplete\|FINISHED\|unproven\|vanished\|ERROR\|exit" $R/run2.out | sed 's/^[0-9TZ:.-]* //' | cut -c1-240 | head -8 | sed 's/^/  log: /'
python3 -I $S/scripts/ledger_lines.py $R/c > $R/after.txt
nb=$(wc -l < $R/before.txt); led=LEDGER_OK
head -n $nb $R/after.txt | cmp -s - $R/before.txt || led=LEDGER_CHANGED
flag=""; [ "$rc" = 0 ] && [ "$alive2" = YES ] && flag=" RC0_SURVIVOR"
echo "RESULT tag=$tag mode=$mode helper=$helper rc=$rc marked_alive=$alive2 filtered_listings=${nf:-0}/${nl:-0} records=$nb/$(wc -l < $R/after.txt) $led$flag"
alive $DP && kill -KILL $DP
[ -n "$PP" ] && alive $PP && kill -KILL $PP
exit 0
