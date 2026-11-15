# E1: foreign decoys vs ownership rule; copied directory; two supervisors; pid reuse; read-only commands.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6/scripts/lib.sh
in_ns_guard6
R=$S/runs/reg_e1${TAG:-}; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
make_wrapper $R/wrap.sh daemon chainroot
CASE_MS='[4000,8000]' P_CRASH=0 make_spec $R/spec1.json $R/c1 $R/wrap.sh 11 12
$RC run --spec $R/spec1.json --dir $R/c1 --plain > $R/s1a.out 2>&1 & S1=$!
sleep 5
ID=$(campid $R/c1); echo "ID=$ID S1=$S1"
setsid $H sleep & D1=$!
env -i RR_CAMPAIGN_JOB=g5.0000000000000000:N0.i0000.L0-t0:a1 setsid $H sleep & D2=$!
JP=$(marked "RR_CAMPAIGN_JOB=$ID" | head -1); echo "job pid for envcopy: $JP ($(tr '\0' ' ' </proc/$JP/cmdline))"
$H envcopy $JP -- $H sleep & D3=$!
env -i RR_CAMPAIGN_JOB=$ID:zz:a1 setsid $H sleep & D4=$!
env -i RR_CAMPAIGN_JOB=${ID}x:N0.i0000.L0-t0:a1 setsid $H sleep & D5=$!     # id with a suffix
$H envraw "RR_CAMPAIGN_JOB=g5.0000000000000000:x" "RR_CAMPAIGN_JOB=$ID:N0.i0000.L0-t0:a1" -- $H sleep & D6=$!  # raw env: 1st foreign, 2nd ours
env -i "RR_CAMPAIGN_JOB= $ID:N0.i0000.L0-t0:a1" setsid $H sleep & D7=$!   # leading space
$H envraw "RR_CAMPAIGN_JOB=$ID:N0.i0000.L0-t0:a1" "RR_CAMPAIGN_JOB=g5.0000000000000000:x" -- $H sleep & D8=$!  # raw env: 1st ours, 2nd foreign
sleep 1
echo "decoys D1..D8: $D1 $D2 $D3 $D4 $D5 $D6 $D7 $D8"
# read-only commands while live
for c in "status $R/c1 --once" "status $R/c1 --json" "verify $R/c1" "result $R/c1"; do $RC $c > /dev/null 2>&1; echo "ro '$c' rc=$?"; done
# copy the live directory; try to run on the copy while the original lives
cp -a $R/c1 $R/c2
sed "s#$R/c1#$R/c2#" $R/spec1.json > $R/spec2.json
$RC run --spec $R/spec2.json --dir $R/c2 --plain > $R/s2-live-try.out 2>&1; echo "copy run while original alive: rc=$?"; tail -2 $R/s2-live-try.out
# crash S1
sleep 2; kill -KILL $S1; wait $S1 2>/dev/null; echo "S1 crashed (SIGKILL)"
sleep 1
ORPH=$(marked "RR_CAMPAIGN_JOB=$ID:"); echo "S1 orphans (marked $ID): $(echo $ORPH | wc -w)"
# pid reuse: a dead recorded leader pid gets a decoy with a FOREIGN marker and one with NO marker
LEADERS=$(grep -h '"started"' $R/c1/ledger/*.jsonl | grep -o '"pid":[0-9]*' | sed 's/"pid"://' | sort -un)
REUSED=""
for P in $LEADERS; do
  if [ ! -d /proc/$P ]; then
    if [ -z "$REUSED" ]; then got=$(env -i RR_CAMPAIGN_JOB=g5.ffffffffffffffff:N0.i0000.L0-t0:a1 $H pidfill $P $H sleep); [ "$got" = "$P" ] && REUSED="$P"; 
    else got=$(env -i PATH=/bin $H pidfill $P $H sleep); [ "$got" = "$P" ] && REUSED="$REUSED $P"; break; fi
  fi
done
echo "pid reuse decoys at recorded leader pids: $REUSED"
# second supervisor on the COPY (adopt-moved) while S1 is dead and its orphans live
$RC run --spec $R/spec2.json --dir $R/c2 --plain > $R/s2.out 2>&1 & S2=$!
sleep 6
ID2=$(grep -h 'marker_id' $R/c2/ledger/*.jsonl | grep -o '"path":"[^"]*"' | tail -1 | sed 's/"path":"//;s/"//')
echo "S2 without --adopt: rc=$(wait $S2; echo $?)"; tail -2 $R/s2.out
$RC run --spec $R/spec2.json --dir $R/c2 --plain --adopt-moved-campaign > $R/s2b.out 2>&1 & S2=$!
sleep 6
ID2=$(grep -h 'marker_id' $R/c2/ledger/*.jsonl | grep -o '"path":"[^"]*"' | tail -1 | sed 's/"path":"//;s/"//')
echo "ID2=$ID2"
ORPH_AFTER_S2=$(for p in $ORPH; do alive $p && echo $p; done | wc -w); echo "S1 orphans alive after S2 start: $ORPH_AFTER_S2 (of $(echo $ORPH|wc -w))"
# restart S1 while S2 runs
$RC run --spec $R/spec1.json --dir $R/c1 --plain > $R/s1b.out 2>&1 & S1B=$!
sleep 8
S2JOBS=$(marked "RR_CAMPAIGN_JOB=$ID2:" | wc -w); echo "S2 marked processes alive while S1 restarted: $S2JOBS"
wait $S1B; echo "S1 restart rc=$?"
wait $S2; echo "S2 rc=$?"
echo "--- survivors"
for v in D1 D2 D3 D4 D5 D6 D7 D8; do p=${!v}; if alive $p; then echo "$v $p ALIVE"; else echo "$v $p GONE"; fi; done
for p in $REUSED; do if alive $p; then echo "reused $p ALIVE ($(tr '\0' ' ' < /proc/$p/environ | head -c 80))"; else echo "reused $p GONE"; fi; done
echo "marked $ID left: $(marked "RR_CAMPAIGN_JOB=$ID:" | wc -w)  marked $ID2 left: $(marked "RR_CAMPAIGN_JOB=$ID2:" | wc -w)"
echo "--- signal_sent records c1 by proof/by:"; grep -h '"signal_sent"' $R/c1/ledger/*.jsonl | grep -o '"proof":"[^"]*","by":"[^"(]*' | sort | uniq -c
echo "--- signal_sent pids c1 matching decoys:"; for v in D1 D2 D3 D4 D5 D6 D7 D8; do p=${!v}; n=$(grep -h '"signal_sent"' $R/c1/ledger/*.jsonl $R/c2/ledger/*.jsonl | grep -c "\"pid\":$p,"); echo "$v $p records=$n"; done
for p in $REUSED; do n=$(grep -h '"signal_sent"' $R/c1/ledger/*.jsonl $R/c2/ledger/*.jsonl | grep -c "\"pid\":$p,"); echo "reused $p signal records (any start)=$n"; done
grep -h 'orphan_killed\|marked_survivor\|scope' $R/c1/log/* 2>/dev/null | tail -5
