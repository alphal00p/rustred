# E2 crash seed: SEED MODES... ; env FP (failpoint site:n or empty), BUILD
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6/scripts/lib.sh
in_ns_guard6
seed=$1; shift; modes="$*"; tag=$(echo $modes | tr ' ' '+')
R=$S/runs/reg_e2y/s$seed; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
RANDOM=$seed
make_wrapper $R/wrap.sh $modes
CASE_MS='[500,3000]' make_spec $R/spec.json $R/c $R/wrap.sh $seed 10
ncrash=$(( (seed % 3) + 1 ))
echo "seed=$seed modes=[$modes] crashes=$ncrash"
for k in $(seq 1 $ncrash); do
  if [ $((seed % 2)) = 0 ]; then
    sites=(before_settle_commit after_settle_commit after_launch_record after_spawn progress_write)
    fp="${sites[$((RANDOM % 5))]}:$(( (RANDOM % 6) + 1 ))"
    RR_CAMPAIGN_FAILPOINT=$fp timeout -s KILL 300 $RC run --spec $R/spec.json --dir $R/c --plain > $R/run$k.out 2>&1; rc=$?
    echo "crash $k: failpoint $fp rc=$rc"
  else
    t=$(( (RANDOM % 9000) + 500 ))
    $RC run --spec $R/spec.json --dir $R/c --plain > $R/run$k.out 2>&1 & SUP=$!
    sleep $(awk "BEGIN{print $t/1000}")
    if alive $SUP; then kill -KILL $SUP; fi; wait $SUP; rc=$?
    echo "crash $k: SIGKILL at ${t}ms rc=$rc"
  fi
  ID=$(campid $R/c)
  echo "  marked after crash $k: $(marked "RR_CAMPAIGN_JOB=$ID:" | wc -w)"
done
# final run(s) to completion, bounded; an exit 8 / 71 is reported, never silently retried
t0=$(date +%s)
timeout -s KILL 900 $RC run --spec $R/spec.json --dir $R/c --plain > $R/final.out 2>&1; frc=$?
echo "final rc=$frc after $(( $(date +%s)-t0 ))s"
ID=$(campid $R/c)
sleep 1
ML=$(marked "RR_CAMPAIGN_JOB=$ID:"); echo "MARKED_LEFT=$(echo $ML | wc -w) $ML"
for p in $ML; do echo "  $p $(cat /proc/$p/comm) $(grep -o 'RR_CAMPAIGN_JOB=[^ ]*' <(tr '\0' ' ' </proc/$p/environ))"; done
UL=0; for p in /proc/[0-9]*; do p=${p#/proc/}; [ $p = 1 ] || [ $p = $$ ] && continue; grep -q hostile /proc/$p/cmdline 2>/dev/null && UL=$((UL+1)); done; echo "HOSTILE_ANY_LEFT=$UL"
$RC verify $R/c > $R/verify.out 2>&1; echo "verify rc=$?"
$RC result $R/c > $R/result.txt 2>&1; echo "result rc=$?"
grep -h 'FINISHED\|ATTENTION\|exit 8\|NOT PROVEN\|unproven' $R/final.out | tail -3
# gate5 diagnostic: what are the hostile processes left, and do they carry THIS campaign's marker at some instant?
for p in /proc/[0-9]*; do p=${p#/proc/}; [ $p = 1 ] || [ $p = $$ ] && continue; grep -q hostile /proc/$p/cmdline 2>/dev/null || continue
  hits=0; for i in $(seq 1 40); do tr '\0' '\n' < /proc/$p/environ 2>/dev/null | grep -q "^RR_CAMPAIGN_JOB=$ID:" && hits=$((hits+1)); sleep 0.025; done
  echo "  LEFT $p $(tr '\0' ' ' < /proc/$p/cmdline | sed 's#.*/hostile#hostile#') sid=$(awk '{print $6}' /proc/$p/stat) marked_in_samples=$hits/40 state=$(awk "/^State:/{print \$2}" /proc/$p/status) start=$(starttime $p) ledger_STOP=$(grep -h "\"signal_sent\"" $R/c/ledger/*.jsonl | grep -c "\"pid\":$p,\"start_ticks\":$(starttime $p),\"signal\":19") ledger_KILL=$(grep -h "\"signal_sent\"" $R/c/ledger/*.jsonl | grep -c "\"pid\":$p,\"start_ticks\":$(starttime $p),\"signal\":9")"
done
