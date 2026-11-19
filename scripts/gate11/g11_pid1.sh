#!/usr/bin/env bash
# gate11 (h): the SUPERVISOR AS PID 1 of its own private user+pid+mount namespace (unshare -Urpf --mount-proc; a private
# tmpfs on /dev/shm; bash execs into rr_campaign, so the supervisor IS pid 1), one campaign whose first job starts an UNMARKED
# fast fork chain (env -i g11zchain SECS LIFE_US: every hop forks its successor and exits; each exited hop is a zombie of the
# child subreaper = the supervisor). From outside (this script runs in the auditor's unit and only READS /proc of its own
# descendants) the supervisor's zombie children are sampled every 0.2 s. MODE pid1 | child (child: under ns8r.sh's bash).
# Prints PID1RESULT tag mode rc max_zombies max_children samples.
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate11
. $S/scripts/lib.sh
mode=$1; secs=$2; life=$3; tag=$4
R=$S/runs/pid1/$tag; [ -e $R ] && { echo "exists $R"; exit 3; }; mkdir -p $R
cat > $R/wrap.sh <<W
#!$BASH_BIN
if [ "\$1" = job ] && mkdir $R/chain.lock 2>/dev/null; then env -i $S/bin/g11zchain $secs $life; fi
exec $FC "\$@"
W
chmod 755 $R/wrap.sh
EXTRA_RES=',"unproven_end_limit_s":120' P_CRASH=0 P_MEM=0 P_NORULE=0 CASE_MS='[1500,2500]' MAXJOBS=2 make_spec $R/spec.json $R/c $R/wrap.sh 3 4
t=$S/tmp/p1.$$.$RANDOM; mkdir -p $t
if [ "$mode" = pid1 ]; then
  TMPDIR=$t taskset -c "$CPUS" unshare -Urpf --mount-proc --kill-child -- $BASH_BIN -c 'mount -t tmpfs -o size=32m,mode=1777 tmpfs /dev/shm && touch /dev/shm/.gate11-private && exec "$0" "$@"' $RC run --spec $R/spec.json --dir $R/c --plain > $R/run.out 2>&1 &
else
  TMPDIR=$t taskset -c "$CPUS" unshare -Urpf --mount-proc --kill-child -- $BASH_BIN -c 'mount -t tmpfs -o size=32m,mode=1777 tmpfs /dev/shm && touch /dev/shm/.gate11-private && "$0" "$@"; exit $?' $RC run --spec $R/spec.json --dir $R/c --plain > $R/run.out 2>&1 &
fi
U=$!
sup=""; maxz=0; maxc=0; n=0
while kill -0 $U 2>/dev/null; do
  if [ -z "$sup" ]; then
    k=$(cat /proc/$U/task/$U/children 2>/dev/null | awk '{print $1}')
    if [ -n "$k" ]; then
      if [ "$mode" = pid1 ]; then sup=$k; else
        for c in $(cat /proc/$k/task/*/children 2>/dev/null); do grep -q rr_campaign /proc/$c/cmdline 2>/dev/null && sup=$c; done
      fi
    fi
  fi
  if [ -n "$sup" ]; then
    kids=$(cat /proc/$sup/task/*/children 2>/dev/null)
    nc=$(echo $kids | wc -w); nz=0
    for c in $kids; do [ "$(cut -d' ' -f3 /proc/$c/stat 2>/dev/null)" = Z ] && nz=$((nz+1)); done
    [ $nz -gt $maxz ] && maxz=$nz; [ $nc -gt $maxc ] && maxc=$nc; n=$((n+1))
  fi
  sleep 0.2
done
wait $U; rc=$?
[ -n "$t" ] && case "$t" in /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate11/tmp/?*) rm -r -- "$t";; esac
nspid=$(grep -o 'supervisor pid [0-9]*' $R/c/log/supervisor.log 2>/dev/null | head -1)
echo "PID1RESULT tag=$tag mode=$mode chain=${secs}s/${life}us rc=$rc max_zombies=$maxz max_children=$maxc samples=$n reaped_note=\"$(grep -o 'reaped[^,;]*' $R/c/log/supervisor.log | tail -1)\" $(grep -h 'FINISHED\|not proven\|descendant' $R/run.out | tail -1 | cut -c1-160)"
