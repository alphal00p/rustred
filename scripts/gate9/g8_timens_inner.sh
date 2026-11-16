# gate8 (g) MJ-2 variants (inside ns8r, root map; readable marked daemon of a crashed session 1). TAG VARIANT:
#  t1  S1 in a time ns (+100000 s), S2 outside (gate7's repro)
#  t2  S1 outside, S2 in a time ns with a NEGATIVE offset (-50000 s)
#  t3  S1 and S2 in two DIFFERENT time namespaces with the SAME offset (+100000 s)
#  t4  both outside; only the job's daemon runs in its own time ns (+100000 s): the child differs
#  t5  S1 in a time ns (+100000 s), S2 outside with /proc/self/ns/time UNREADABLE (injected stat failure)
#  t6  S1 in a time ns (+100000 s) with /proc/self/ns/time UNREADABLE there (S1 records none), S2 outside
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/scripts/lib.sh
in_ns_guard6; shm_guard
tag=$1; v=$2
R=$S/runs/timens/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
D="$S/bin/g7hostile ndaemon $R/daemon.pids"; [ $v = t4 ] && D="unshare -T --boottime 100000 -- $S/bin/g7hostile ndaemon $R/daemon.pids"
cat > $R/wrap.sh <<W
#!$BASH_BIN
if [ "\$1" = job ] && [ ! -e $R/daemon.pids ]; then $D; fi
exec $FC "\$@"
W
chmod 755 $R/wrap.sh
P_CRASH=0 P_MEM=0 P_NORULE=0 CASE_MS='[2500,4000]' MAXJOBS=2 make_spec $R/spec.json $R/c $R/wrap.sh 5 6
INJ="env LD_PRELOAD=$S/bin/g8fault.so G8_MATCH=/proc/self/ns/time G8_OPS=stat G8_FROM=1 G8_LOG=$R/fault.log"
case $v in t1|t3|t5) P1="unshare -T --boottime 100000 --";; t6) P1="unshare -T --boottime 100000 -- $INJ";; *) P1="";; esac
case $v in t2) P2="unshare -T --boottime -50000 --";; t3) P2="unshare -T --boottime 100000 --";; t5) P2="$INJ";; *) P2="";; esac
$P1 $RC run --spec $R/spec.json --dir $R/c --plain > $R/run1.out 2>&1 & U=$!
for i in $(seq 1 300); do [ -s $R/daemon.pids ] && break; sleep 0.1; done
sleep 0.5; kill -KILL $U; wait $U 2>/dev/null; sleep 0.5
read DP DM < $R/daemon.pids; echo "session 1 killed; daemon $DP alive=$(alive $DP && echo yes || echo no) start(read here)=$(starttime $DP)"
grep -h -o '"time_ns":[0-9]*' $R/c/ledger/*.jsonl | head -2 | sed 's/^/  recorded: /'; grep -q '"time_ns"' $R/c/ledger/*.jsonl || echo "  recorded: no time_ns"
timeout -s KILL 600 $P2 $RC run --spec $R/spec.json --dir $R/c --plain > $R/run2.out 2>&1; rc=$?
grep -h "recovery: marker sweep\|time namespace\|lineage rule" $R/run2.out | sed 's/^[0-9TZ:.-]* //' | cut -c1-220 | sed 's/^/  log: /'
flag=""; [ "$rc" = 0 ] && alive $DP && flag=" RC0_SURVIVOR"
echo "RESULT tag=$tag variant=$v session2 rc=$rc daemon_alive=$(alive $DP && echo YES || echo no)$flag"
alive $DP && kill -KILL $DP
exit 0
