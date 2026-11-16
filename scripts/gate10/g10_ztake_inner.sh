# gate10 (i): lock take-over with NZ unreaped zombies in the namespace: TAG NZ. Session 1 SIGKILLed (stale LOCK); session 2
# runs under g10zinit NZ (NZ zombies of a subreaper in its ancestry... and in /proc): the time from its start to the
# take-over log line and to its end. Prints RESULT.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate10/scripts/lib.sh
in_ns_guard6; shm_guard
tag=$1; nz=$2
R=$S/runs/ztake/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
P_CRASH=0 P_MEM=0 P_NORULE=0 CASE_MS='[300,600]' MAXJOBS=2 make_spec $R/spec.json $R/c $FC 5 4
$RC run --spec $R/spec.json --dir $R/c --plain > $R/run1.out 2>&1 & SUP=$!
for i in $(seq 1 200); do grep -q 'containment' $R/run1.out 2>/dev/null && break; sleep 0.05; done
sleep 0.5; kill -KILL $SUP; wait $SUP 2>/dev/null
t0=$(date +%s.%N)
timeout -s KILL 1200 $S/bin/g10zinit $nz $RC run --spec $R/spec.json --dir $R/c --plain > $R/run2.out 2>&1; rc=$?
t1=$(date +%s.%N)
tk=$(grep -m1 'taken over' $R/run2.out | cut -c1-20)
first=$(head -1 $R/run2.out | cut -c1-20)
echo "RESULT tag=$tag zombies=$nz rc=$rc total_s=$(awk "BEGIN{printf \"%.1f\", $t1-$t0}") takeover_line=[$(grep -m1 'taken over' $R/run2.out | sed 's/^[0-9TZ:.-]* //' | cut -c1-90)] first=$first takeover_at=$tk"
exit 0
