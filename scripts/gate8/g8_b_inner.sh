# (b) gate6: plain run stopped by SIGHUP / SIGTERM / SIGINT (as a lost terminal would), resume == reference. INSIDE ns().
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate8/scripts/lib.sh
in_ns_guard6; shm_guard
R=$S/runs/b_sig_$1; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
make_wrapper $R/wrap.sh daemon
CASE_MS='[1500,3000]' P_CRASH=0 make_spec $R/ref.json $R/ref $R/wrap.sh 31 10
$RC run --spec $R/ref.json --dir $R/ref --plain > $R/ref.out 2>&1; echo "reference rc=$?"; $RC result $R/ref > $R/ref.result 2>&1
for sig in HUP TERM INT; do
  D=$R/run-$sig; CASE_MS='[1500,3000]' P_CRASH=0 make_spec $R/$sig.json $D $R/wrap.sh 31 10
  $RC run --spec $R/$sig.json --dir $D --plain > $D.out 2>&1 & SUP=$!; sleep 3; t1=$(date +%s.%N); kill -$sig $SUP; wait $SUP; rc=$?
  echo "SIG$sig at 3 s -> rc=$rc after $(awk "BEGIN{printf \"%.1f\", $(date +%s.%N)-$t1}") s; marked after: $(marked "RR_CAMPAIGN_JOB=$(campid $D):" | wc -w)"
  $RC run --spec $R/$sig.json --dir $D --plain > $D.resume.out 2>&1; echo "  resume rc=$?"
  $RC result $D > $D.result 2>&1; echo "  $(python3 -I $S/scripts/rescmp.py $R/ref.result $D.result); verify $($RC verify $D >/dev/null 2>&1; echo $?)"
done
