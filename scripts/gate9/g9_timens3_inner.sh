# gate9 (g) MJ8-1 across THREE sessions: S1 in a time ns (+100000 s) with its /proc/self/ns/time unreadable (records an explicit
# unknown), its job starts marked daemon D1, S1 SIGKILLed; S2 outside: the resume must kill D1; S2's first job starts a
# second daemon D2 (wrapper re-armed), S2 SIGKILLed; S3 outside: must kill D2 (the unknown of S1 still counts). TAG
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/scripts/lib.sh
in_ns_guard6; shm_guard
tag=$1; R=$S/runs/timens3/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
cat > $R/wrap.sh <<W
#!$BASH_BIN
if [ "\$1" = job ] && [ -e $R/arm ]; then rm -f $R/arm; $S/bin/g7hostile ndaemon $R/daemon.pids; fi
exec $FC "\$@"
W
chmod 755 $R/wrap.sh
P_CRASH=0 P_MEM=0 P_NORULE=0 CASE_MS='[2500,4000]' MAXJOBS=2 make_spec $R/spec.json $R/c $R/wrap.sh 5 10
INJ="env LD_PRELOAD=$S/bin/g9fault.so G8_MATCH=/proc/self/ns/time G8_OPS=stat G8_FROM=1 G8_LOG=$R/fault.log"
crash() { local pre=$1 n=$2; touch $R/arm; $pre $RC run --spec $R/spec.json --dir $R/c --plain > $R/run$n.out 2>&1 & U=$!
  for i in $(seq 1 300); do [ "$(wc -l < $R/daemon.pids 2>/dev/null)" -ge $3 ] && break; sleep 0.1; done
  sleep 0.5; kill -KILL $U; wait $U 2>/dev/null; sleep 0.5; }
crash "unshare -T --boottime 100000 -- $INJ" 1 1
D1=$(sed -n 1p $R/daemon.pids | cut -d' ' -f1); echo "S1 killed; D1 $D1 alive=$(alive $D1 && echo yes || echo no); time_ns_unknown recorded: $(grep -c time_ns_unknown $R/c/ledger/*.jsonl)"
crash "" 2 2
D2=$(sed -n 2p $R/daemon.pids | cut -d' ' -f1); echo "S2 killed; D1 alive=$(alive $D1 && echo YES || echo no) D2 $D2 alive=$(alive $D2 && echo yes || echo no)"
grep -h "recovery: marker sweep\|time namespace" $R/run2.out | sed 's/^[0-9TZ:.-]* //' | cut -c1-200 | sed 's/^/  S2 log: /'
timeout -s KILL 600 $RC run --spec $R/spec.json --dir $R/c --plain > $R/run3.out 2>&1; rc=$?
grep -h "recovery: marker sweep\|time namespace\|lineage rule" $R/run3.out | sed 's/^[0-9TZ:.-]* //' | cut -c1-200 | sed 's/^/  S3 log: /'
flag=""; [ "$rc" = 0 ] && { alive $D1 || alive $D2; } && flag=" RC0_SURVIVOR"
echo "RESULT tag=$tag S3 rc=$rc D1_alive=$(alive $D1 && echo YES || echo no) D2_alive=$(alive $D2 && echo YES || echo no)$flag"
for p in $D1 $D2; do alive $p && kill -KILL $p; done; exit 0
