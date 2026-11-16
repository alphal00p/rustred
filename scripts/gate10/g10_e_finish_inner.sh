# gate10 (e): finish an interrupted drill campaign that hit the harness's 60-session cap: TAG ; run the same command
# (no interruption) until it ends finished, then compare with its reference
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate10/scripts/lib.sh
in_ns_guard6; shm_guard
R=$S/runs/e10/$1; k=0; rc=1
while [ $k -lt 5 ]; do k=$((k+1)); $RC run --spec $R/int.json --dir $R/int --plain > $R/fin.$k.out 2>&1; rc=$?; echo "finish session $k rc=$rc"; case $rc in 0|10|11) break;; esac; done
$RC verify $R/int > /dev/null 2>&1; v=$?
$RC result $R/int > $R/int.result
echo "FINISH tag=$1 rc=$rc verify=$v $(python3 -I $S/scripts/rescmp.py $R/ref.result $R/int.result) MARKED_LEFT=$(marked "RR_CAMPAIGN_JOB=$(campid $R/int):" | wc -w)"
