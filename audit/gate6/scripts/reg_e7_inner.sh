# E7 (b): terminal disappears under `run` (dashboard) and `watch`.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6/scripts/lib.sh
in_ns_guard6
R=$S/runs/reg_e7; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
P="python3 -I $S/scripts/reg_ptydrive.py"
CASE_MS='[1500,3000]' P_CRASH=0 make_spec $R/ref.json $R/ref $FC 31 10
$RC run --spec $R/ref.json --dir $R/ref --plain > $R/ref.out 2>&1; echo "reference rc=$?"; $RC result $R/ref > $R/ref.result 2>&1
for mode in ctty-close noctty-close; do
  D=$R/run-$mode; CASE_MS='[1500,3000]' P_CRASH=0 make_spec $R/$mode.json $D $FC 31 10
  echo "run dashboard, $mode at 3 s: $($P $mode 3 60 -- $RC run --spec $R/$mode.json --dir $D)"
  ML=$(marked "RR_CAMPAIGN_JOB=$(campid $D):" | wc -w); echo "  marked after: $ML"
  $RC run --spec $R/$mode.json --dir $D --plain > $D.resume.out 2>&1; echo "  resume rc=$?"
  $RC result $D > $D.result 2>&1; cmp -s <(grep -v '"campaign_dir"\|/runs/' $D.result) <(grep -v '"campaign_dir"\|/runs/' $R/ref.result) && echo "  result == reference" || { echo "  RESULT DIFFERS"; diff <(cat $R/ref.result) <(cat $D.result) | head -5; }
  W=$R/w-$mode; CASE_MS='[1500,3000]' P_CRASH=0 make_spec $R/w$mode.json $W $FC 31 10
  $RC run --spec $R/w$mode.json --dir $W --plain > $W.out 2>&1 & SUP=$!; sleep 2
  echo "watch, $mode at 3 s: $($P $mode 3 15 -- $RC watch $W)"
  alive $SUP && echo "  supervisor alive after the viewer's terminal went: yes" || echo "  supervisor alive: NO"
  wait $SUP; echo "  supervisor rc=$?"
done
