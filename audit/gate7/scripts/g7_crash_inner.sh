# gate7 (a): crash seeds with NON-DUMPABLE marked descendants. INSIDE innsu (host-like: capless supervisor; a non-dumpable
# process's environ is unreadable to it). SEED MODES... (g7: ndaemon ndchain ndorphan; gate4 hostile: daemon chainroot flipexec dropexec)
# Checks: every final run with a live marked g7hostile ends 8 (never 0 / 10); every live one is NAMED in the final message /
# note; the operator kills exactly the named pids (up to 3 rounds) and the same command then ends 0 with nothing left.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate7/scripts/lib.sh
in_ns_guard6
hostlike || { echo "NOT HOST-LIKE"; exit 99; }
seed=$1; shift; modes="$*"
R=$S/runs/crash/s$seed; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
RANDOM=$seed
{ echo "#!$BASH_BIN"; echo 'if [ "$1" = job ]; then'; echo '  export HOSTILE_MARK="$RR_CAMPAIGN_JOB"'
  for m in $modes; do case $m in
    ndaemon) echo "  $S/bin/g7hostile ndaemon $R/nd.pids";;
    ndchain) echo "  $S/bin/g7hostile ndchain 1500";;
    ndorphan) echo "  $S/bin/g7hostile ndorphan";;
    *) echo "  $H $m 25 >/dev/null 2>&1 &";;
  esac; done
  echo '  wait'; echo 'fi'; echo "exec $FC \"\$@\""; } > $R/wrap.sh
chmod 755 $R/wrap.sh
P_CRASH=0 P_MEM=0 CASE_MS='[400,2000]' MAXJOBS=2 make_spec $R/spec.json $R/c $R/wrap.sh $seed 6
ncrash=$(( (seed % 3) + 1 ))
echo "seed=$seed modes=[$modes] crashes=$ncrash"
for k in $(seq 1 $ncrash); do
  if [ $((seed % 2)) = 0 ]; then
    sites=(before_settle_commit after_settle_commit after_launch_record after_spawn progress_write)
    fp="${sites[$((RANDOM % 5))]}:$(( (RANDOM % 4) + 1 ))"
    RR_CAMPAIGN_FAILPOINT=$fp timeout -s KILL 300 $RC run --spec $R/spec.json --dir $R/c --plain > $R/run$k.out 2>&1; rc=$?
    echo "crash $k: failpoint $fp rc=$rc"
  else
    t=$(( (RANDOM % 6000) + 500 ))
    $RC run --spec $R/spec.json --dir $R/c --plain > $R/run$k.out 2>&1 & SUP=$!
    sleep $(awk "BEGIN{print $t/1000}"); if alive $SUP; then kill -KILL $SUP; fi; wait $SUP; rc=$?
    echo "crash $k: SIGKILL at ${t}ms rc=$rc"
  fi
done
g7alive() { for p in /proc/[0-9]*; do p=${p#/proc/}; [ "$(cat /proc/$p/comm 2>/dev/null)" = g7hostile ] && alive $p && echo $p; done; }
hostalive() { for p in /proc/[0-9]*; do p=${p#/proc/}; [ "$p" = 1 ] && continue; [ "$(cat /proc/$p/comm 2>/dev/null)" = hostile ] && alive $p && echo $p; done; }
age() { echo $(( $(awk '{print int($1*100)}' /proc/uptime) - $(starttime $1) )); }   # ticks (namespace boottime = host)
for round in 1 2 3; do
  $RC run --spec $R/spec.json --dir $R/c --plain > $R/final$round.out 2>&1 & SUP=$!
  for k in $(seq 1 2400); do alive $SUP || break; sleep 0.1; done
  stopped=no
  if alive $SUP; then $RC control $R/c stop > /dev/null 2>&1; stopped=yes; for k in $(seq 1 1200); do alive $SUP || break; sleep 0.1; done; alive $SUP && { kill -KILL $SUP; stopped=killed; }; fi
  wait $SUP; frc=$?; sleep 1
  G=$(g7alive | tr '\n' ' '); HD=$(hostalive | wc -l)
  unnamed=""; nstable=0
  for p in $G; do a=$(age $p); [ "$a" -gt 2000 ] || continue; nstable=$((nstable+1)); grep -q "pid $p \|\[$p\]\|\[$p,\|,$p\]\|,$p," $R/final$round.out $R/c/ledger/*.jsonl 2>/dev/null || unnamed="$unnamed $p"; done
  verdict=OK
  [ -n "$G" ] && [ "$frc" = 0 ] && verdict="BLOCKER(exit 0 with a live marked non-dumpable)"
  [ -n "$G" ] && [ "$frc" != 0 ] && [ "$frc" != 8 ] && [ "$frc" != 140 ] && verdict="DEFECT(rc $frc with a live marked non-dumpable)"
  [ -n "$unnamed" ] && verdict="$verdict; UNNAMED stable survivor(s):$unnamed"
  [ -z "$G" ] && [ "$frc" != 0 ] && verdict="note(rc $frc, none left)"
  echo "final round $round: rc=$frc operator_stop=$stopped g7hostile alive=[${G}] stable=$nstable gate4-hostile alive=$HD unproven-lines=$(grep -c 'end still unproven\|not proven\|NOT PROVEN' $R/final$round.out) -> $verdict"
  [ "$frc" = 0 ] && [ -z "$G" ] && break
  # the operator kills exactly the g7hostile processes the log names (all are this campaign's; the hopping chain is named by its current pid)
  for p in $G; do kill -KILL $p; done; sleep 0.5
done
$RC verify $R/c > $R/verify.out 2>&1; echo "verify rc=$?"
for p in $(g7alive); do kill -KILL $p; done
echo "SEED_END seed=$seed final_rc=$frc"
