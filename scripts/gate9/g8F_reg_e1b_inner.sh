# E1b: copied campaign directory + two supervisors at once, crashes on both sides.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/scripts/lib.sh
in_ns_guard6; shm_guard
R=$S/runs/reg_e1b; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
make_wrapper $R/wrap.sh daemon chainroot
CASE_MS='[4000,8000]' P_CRASH=0 make_spec $R/spec1.json $R/c1 $R/wrap.sh 12 12
$RC run --spec $R/spec1.json --dir $R/c1 --plain > $R/s1a.out 2>&1 & S1=$!
sleep 6; ID=$(campid $R/c1); echo "ID=$ID"
kill -KILL $S1; wait $S1 2>/dev/null; echo "S1 crashed"
O1=$(marked "RR_CAMPAIGN_JOB=$ID:"); echo "S1 orphans: $(echo $O1|wc -w)"
cp -a $R/c1 $R/c2
$RC run --spec $R/spec1.json --dir $R/c2 --plain > $R/s2a.out 2>&1; echo "S2 on copy w/o adopt rc=$?"; tail -1 $R/s2a.out
$RC run --spec $R/spec1.json --dir $R/c2 --plain --adopt-moved-campaign > $R/s2.out 2>&1 & S2=$!
sleep 6
ID2=$(grep -h 'marker_id' $R/c2/ledger/*.jsonl | grep -o '"path":"[^"]*"' | tail -1 | sed 's/"path":"//;s/"//'); echo "ID2=$ID2"
A1=0; for p in $O1; do alive $p && A1=$((A1+1)); done; echo "S1 orphans alive 6 s after S2 (copy) started: $A1 / $(echo $O1|wc -w)"
O2=$(marked "RR_CAMPAIGN_JOB=$ID2:"); echo "S2 marked now: $(echo $O2|wc -w)"
# crash S2 too, then restart S1 (must kill only ID orphans), then restart S2
kill -KILL $S2; wait $S2 2>/dev/null; echo "S2 crashed"
O2=$(marked "RR_CAMPAIGN_JOB=$ID2:"); echo "S2 orphans: $(echo $O2|wc -w)"
$RC run --spec $R/spec1.json --dir $R/c1 --plain > $R/s1b.out 2>&1 & S1=$!
sleep 8
A2=0; for p in $O2; do alive $p && A2=$((A2+1)); done; echo "S2 orphans alive 8 s after S1 restart: $A2 / $(echo $O2|wc -w)"
$RC run --spec $R/spec1.json --dir $R/c2 --plain --adopt-moved-campaign > $R/s2b.out 2>&1 & S2=$!
wait $S1; echo "S1 rc=$?"; wait $S2; echo "S2 rc=$?"
echo "left ID: $(marked "RR_CAMPAIGN_JOB=$ID:" | wc -w)  left ID2: $(marked "RR_CAMPAIGN_JOB=$ID2:" | wc -w)"
echo "c1 signals to pids recorded as S2 orphans:"; for p in $O2; do grep -h '"signal_sent"' $R/c1/ledger/*.jsonl | grep "\"pid\":$p," | grep -o '"marker":"[^"]*"' | sort -u; done
echo "c2 signals carrying ID marker:"; grep -h '"signal_sent"' $R/c2/ledger/*.jsonl | grep -c "\"marker\":\"$ID:"
grep -h 'recovery: marker sweep' $R/c1/log/* $R/c2/log/* 2>/dev/null
