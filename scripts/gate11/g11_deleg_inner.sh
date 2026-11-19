# gate11 (a)(4) emulation: inside a private namespace whose unit is DELEGATED (Delegate=yes: the supervisor's own cgroup
# carries user.delegate and is owned by its euid) the supervisor uses per-job TRACKING LEAVES (cgroup.procs at the spawn,
# cgroup.kill + the recursive populated flag at the end): the cgroup primitives of the host's scope path, exercised here.
# TAG MODE(r) KIND: escapee (job leaves an UNMARKED setsid'd sleep: only the cgroup can find it), zchain (UNMARKED fast fork
# chain 8 s), daemon (MARKED setsid'd daemon). Live session; prints DELEG tag kind rc tracking escapee_alive leaves_left.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate11/scripts/lib.sh
in_ns_guard6; shm_guard
tag=$1; kind=$2; R=$S/runs/deleg/$tag; [ -e $R ] && { echo "exists $R"; exit 3; }; mkdir -p $R
case $kind in
  escapee) start="setsid env -u RR_CAMPAIGN_JOB sh -c 'sleep 600 </dev/null >/dev/null 2>&1 & echo \$! > $R/esc.pid'" ;;
  zchain)  start="env -i $S/bin/g11zchain 8 0" ;;
  daemon)  start="$S/bin/g7hostile ndaemon $R/esc.pids; read d m < $R/esc.pids; echo \$d > $R/esc.pid" ;;
esac
{ echo "#!$BASH_BIN"; echo "if [ \"\$1\" = job ] && mkdir $R/lock 2>/dev/null; then $start; fi"; echo "exec $FC \"\$@\""; } > $R/wrap.sh
chmod 755 $R/wrap.sh
own=$(cut -d: -f3 /proc/self/cgroup); echo "own cgroup $own delegated=$(getfattr -n user.delegate --only-values /sys/fs/cgroup$own 2>/dev/null)"
EXTRA_RES=',"unproven_end_limit_s":60' P_CRASH=0 P_MEM=0 P_NORULE=0 CASE_MS='[1500,2500]' MAXJOBS=2 make_spec $R/spec.json $R/c $R/wrap.sh 4 4
$RC run --spec $R/spec.json --dir $R/c --plain > $R/run.out 2>&1; rc=$?
e=$(cat $R/esc.pid 2>/dev/null); ea=no; [ -n "$e" ] && alive $e && ea=YES
tr=$(grep -m1 -o 'tracking[^;]*' $R/c/log/supervisor.log | cut -c1-120)
leaves=$(ls -d /sys/fs/cgroup$own/job-* 2>/dev/null | wc -l)
echo "DELEG tag=$tag kind=$kind rc=$rc escapee=${e:-none} escapee_alive=$ea leaves_left=$leaves containment=\"$(grep -h -o 'containment [a-z_]*:[^ ]*\|containment marker_only\|containment cgroup:[^ ]*' $R/c/log/supervisor.log | sort | uniq -c | tr '\n' ' ')\" kill_records=$(cat $R/c/ledger/*.jsonl | grep -c 'job cgroup') $(grep -h 'FINISHED\|not proven\|descendant' $R/run.out | tail -1 | cut -c1-140)"
[ -n "$e" ] && alive $e && kill -KILL $e
exit 0
