# smoke: TAG ; a small synthetic campaign to completion (works in ns and nsu)
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6/scripts/lib.sh
in_ns_guard6
tag=$1; R=$S/runs/smoke_$tag; rm -rf -- "$R"; mkdir -p $R
echo "uid=$(id -u) caps=$(grep CapEff /proc/self/status)"
P_CRASH=0 CASE_MS='[300,800]' make_spec $R/spec.json $R/c $FC 7 8
timeout -s KILL 300 $RC run --spec $R/spec.json --dir $R/c --plain > $R/run.out 2>&1; echo "run rc=$?"
ID=$(campid $R/c); echo "id=$ID marked_left=$(marked "RR_CAMPAIGN_JOB=$ID:" | wc -w)"
$RC verify $R/c > $R/verify.out 2>&1; echo "verify rc=$?"
grep -h "containment\|FINISHED\|ERROR\|WARN" $R/run.out | head -12
