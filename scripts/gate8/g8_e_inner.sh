# gate8 (e): TAG SEED KIND MAXJOBS : interrupted = uninterrupted, random interruptions (INT, INTx2, TERM, HUP, KILL,
# control stop) at random points of every session; the caller's unit sets the CPU quota.
#  KIND syn : synthetic, EXTERNAL work_dir, jobs spawn marked hostile daemons / re-forking chains (readable).
#  KIND unp : HOST-LIKE (inside ns8u): the first job also leaves a NON-DUMPABLE descendant below the supervisor (g7hostile
#             ndorphan): its end stays unproven -> unproven_end_limit_s (20 s) after the ~120 s end-of-job hunt -> exit 8
#             naming it; about half of the interruptions are aimed INTO that wait (after an 'end still unproven' line).
#             The operator step (kill exactly the named pid after checking the exit-8 message names it) is done in the
#             reference and in the interrupted campaign alike; then the same command finishes.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate8/scripts/lib.sh
in_ns_guard6; shm_guard
tag=$1; seed=$2; kind=$3; mj=${4:-4}; RANDOM=$seed
R=$S/runs/e8/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
[ $kind = unp ] && { hostlike || { echo "unp needs ns8u"; exit 99; }; }
for x in ref int; do
cat > $R/wrap-$x.sh <<W
#!$BASH_BIN
if [ "\$1" = job ]; then
  export HOSTILE_MARK="\$RR_CAMPAIGN_JOB"
  $H daemon >/dev/null 2>&1 &
  $H chainroot 40 >/dev/null 2>&1 &
  if [ $kind = unp ] && mkdir $R/nd-$x.lock 2>/dev/null; then $S/bin/g7hostile ndorphan $R/nd-$x.pid; fi
  wait
fi
exec $FC "\$@"
W
chmod 755 $R/wrap-$x.sh
EXTRA_RES=", \"unproven_end_limit_s\": ${UNPROVEN_LIMIT:-20}" MAXJOBS=$mj CASE_MS='[400,2500]' make_spec $R/$x.json $R/$x $R/wrap-$x.sh $seed 14
python3 -I - $R/$x.json $R/w-$x <<'P'
import json,sys
p,w=sys.argv[1],sys.argv[2]; s=json.load(open(p)); s["dirs"]["work_dir"]=w; json.dump(s,open(p,"w"))
P
done
fin() { [ "$1" = 0 ] || [ "$1" = 10 ] || [ "$1" = 11 ]; }
# operator step for an exit 8: the named pid must be the recorded non-dumpable descendant; kill exactly it
op8() { local x=$1 out=$2 p; p=$(cat $R/nd-$x.pid 2>/dev/null)
  if [ -n "$p" ] && alive $p; then
    if grep -q "pid $p\b\|\[$p\]\|\[$p,\|, $p\]" $out; then echo "  exit 8 names the hidden pid $p: killed by the operator"; kill -KILL $p; sleep 0.3
    else echo "  EXIT 8 DOES NOT NAME the hidden pid $p (alive): $(grep -h 'exit\|UNPROVEN\|not proven' $out | tail -2 | cut -c1-200)"; kill -KILL $p; fi
  else echo "  exit 8 with no live hidden pid: $(grep -h 'operator decision\|not proven\|UNPROVEN' $out | tail -1 | cut -c1-200)"; fi; }
t0=$(date +%s); k=0; refrc=1
while ! fin $refrc && [ $k -lt 6 ]; do k=$((k+1)); $RC run --spec $R/ref.json --dir $R/ref --plain > $R/ref.$k.out 2>&1; refrc=$?; echo "reference session $k rc=$refrc"; [ $refrc = 8 ] && op8 ref $R/ref.$k.out; done
echo "reference rc=$refrc in $(( $(date +%s)-t0 ))s"
i=0; rc=1; nint=0; nwait=0
while ! fin $rc && [ $i -lt 60 ]; do
  i=$((i+1)); $RC run --spec $R/int.json --dir $R/int --plain > $R/int.$i.out 2>&1 & SUP=$!
  aim=0; if [ $kind = unp ] && [ $nwait -lt 6 ] && [ -e $R/nd-int.pid ] && [ $((RANDOM % 2)) = 0 ]; then aim=1; fi
  if [ $aim = 1 ]; then
    for q in $(seq 1 3000); do grep -q 'end still unproven\|end UNPROVEN' $R/int.$i.out 2>/dev/null && break; alive $SUP || break; sleep 0.1; done
    ms=$(( RANDOM % (${UNPROVEN_LIMIT:-20} * 750) )); sleep $(awk "BEGIN{print $ms/1000}")
  elif [ $((RANDOM % 2)) = 0 ]; then ms=$(( (RANDOM % 550) + 50 )); sleep $(awk "BEGIN{print $ms/1000}")
  else ms=$(( (RANDOM % 6000) + 300 )); sleep $(awk "BEGIN{print $ms/1000}"); fi
  how=$(( RANDOM % 6 ))
  if alive $SUP && { [ $kind != unp ] || [ $nint -lt 14 ]; }; then
    inwait=$(grep -c 'end still unproven\|end UNPROVEN' $R/int.$i.out)
    t1=$(date +%s.%N)
    case $how in
      0) kill -INT $SUP; w=INT;; 1) kill -INT $SUP; sleep 0.3; alive $SUP && kill -INT $SUP; w=INTx2;;
      2) kill -TERM $SUP; w=TERM;; 3) kill -KILL $SUP; w=KILL;; 4) kill -HUP $SUP; w=HUP;;
      5) $RC control $R/int stop > /dev/null 2>&1; w=ctlstop;;
    esac
    for q in $(seq 1 1200); do alive $SUP || break; sleep 0.1; done
    wait $SUP; rc=$?; nint=$((nint+1)); [ "$inwait" -gt 0 ] && nwait=$((nwait+1))
    echo "it $i: $w at ${ms}ms$( [ $aim = 1 ] && echo ' (aimed at the unproven wait)')$( [ "$inwait" -gt 0 ] && echo ' IN_WAIT') -> rc=$rc after $(awk "BEGIN{printf \"%.1f\", $(date +%s.%N)-$t1}")s marked_left=$(marked "RR_CAMPAIGN_JOB=$(campid $R/int):" | wc -w)"
  else
    wait $SUP; rc=$?; echo "it $i: ran to its end rc=$rc"
  fi
  [ $rc = 8 ] && op8 int $R/int.$i.out
done
$RC verify $R/int > /dev/null 2>&1; v=$?
$RC result $R/ref > $R/ref.result; $RC result $R/int > $R/int.result
echo "SUMMARY tag=$tag kind=$kind max_jobs=$mj interruptions=$nint in_wait=$nwait sessions=$i final rc=$rc ref rc=$refrc verify=$v $(python3 -I $S/scripts/rescmp.py $R/ref.result $R/int.result) MARKED_LEFT=$(marked "RR_CAMPAIGN_JOB=$(campid $R/int):" | wc -w) hidden_left=$(p=$(cat $R/nd-int.pid 2>/dev/null); [ -n "$p" ] && alive $p && echo $p || echo 0)"
