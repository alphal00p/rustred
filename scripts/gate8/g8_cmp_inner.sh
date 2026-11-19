. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate8/scripts/lib.sh
in_ns_guard6
R=$1; $RC result $R/ref > $R/ref.result2 2>&1; $RC result $R/int > $R/int.result2 2>&1; $RC verify $R/int > /dev/null 2>&1; v=$?
echo "$(basename $R): verify=$v $(python3 -I $S/scripts/rescmp.py $R/ref.result2 $R/int.result2) marked_left=$(marked "RR_CAMPAIGN_JOB=$(campid $R/int):" | wc -w)"
