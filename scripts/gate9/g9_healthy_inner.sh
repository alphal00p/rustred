# gate9 (i): one HEALTHY fake campaign (no hostile process, p_crash 0, p_norule 0, p_mem 0): SEED WHICH(ref|run)
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/scripts/lib.sh
in_ns_guard6; shm_guard
seed=$1; which=$2; R=$S/runs/healthy${HTAG:-}/s$seed; mkdir -p $R
P_CRASH=0 P_MEM=0 P_NORULE=0 CASE_MS=${HCASE:-'[300,1500]'} MAXJOBS=4 make_spec $R/$which.json $R/$which $FC $seed ${HITEMS:-12}
t0=$(date +%s); $RC run --spec $R/$which.json --dir $R/$which --plain > $R/$which.out 2>&1; rc=$?
$RC result $R/$which > $R/$which.result 2>/dev/null; v=$($RC verify $R/$which > /dev/null 2>&1; echo $?)
echo "HEALTHY seed=$seed which=$which rc=$rc secs=$(( $(date +%s)-t0 )) verify=$v $(grep -c 'end still unproven\|NOT PROVEN\|not proven' $R/$which.out) unproven-lines"
