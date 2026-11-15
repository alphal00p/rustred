# (e) gate7 (from gate6 e_inner): EXTERNAL work_dir (claim + atomic run-dir create at every start / launch); half of the
# interruptions EARLY (50-600 ms: start, claim, recovery). Random interruptions at random points under a CPU quota (the caller's scope), SYNTHETIC source only.
# TAG SEED STRACE(0|1) : jobs spawn marked hostile daemons (signals at every job end); interruptions INT, INTx2, TERM,
# HUP, KILL, control stop at 0.3-6.3 s; with STRACE=1 every session runs under strace and order_check.py checks that
# every signal syscall was behind a durable intent and every delivered signal is in the ledger.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate7/scripts/lib.sh
in_ns_guard6
tag=$1; seed=$2; st=${3:-0}; RANDOM=$seed
R=$S/runs/e7/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
make_wrapper $R/wrap.sh daemon chainroot
CASE_MS='[400,2500]' make_spec $R/ref.json $R/ref $R/wrap.sh $seed 16
CASE_MS='[400,2500]' make_spec $R/int.json $R/int $R/wrap.sh $seed 16
for x in ref int; do python3 -I - $R/$x.json $R/w-$x <<'P'
import json,sys
p,w=sys.argv[1],sys.argv[2]; s=json.load(open(p)); s["dirs"]["work_dir"]=w; json.dump(s,open(p,"w"))
P
done
STR="strace -I 1 -f -ttt -q --decode-fds=path,pidfd -s 1000000 -e trace=write,fdatasync,fsync,pidfd_send_signal,pidfd_open,kill,tgkill,tkill,execve,clone,clone3,fork,vfork -e signal=none"
t0=$(date +%s); $RC run --spec $R/ref.json --dir $R/ref --plain > $R/ref.out 2>&1; refrc=$?; echo "reference rc=$refrc in $(( $(date +%s)-t0 ))s"
i=0; rc=1; fin() { [ "$1" = 0 ] || [ "$1" = 10 ] || [ "$1" = 11 ]; }
while ! fin $rc && [ $i -lt 40 ]; do
  i=$((i+1))
  if [ $st = 1 ]; then $STR -o $R/strace.$i $RC run --spec $R/int.json --dir $R/int --plain > $R/int.$i.out 2>&1 & STP=$!
    SUP=""; for k in $(seq 1 400); do SUP=$(grep -m1 'execve(.*rr_campaign", \[.*"run".*) = 0' $R/strace.$i 2>/dev/null | awk '{print $1}'); [ -n "$SUP" ] && break; sleep 0.02; done
  else $RC run --spec $R/int.json --dir $R/int --plain > $R/int.$i.out 2>&1 & SUP=$!; STP=$SUP; fi
  if [ $((RANDOM % 2)) = 0 ]; then ms=$(( (RANDOM % 550) + 50 )); else ms=$(( (RANDOM % 6000) + 300 )); fi; sleep $(awk "BEGIN{print $ms/1000}")
  how=$(( RANDOM % 6 ))
  if [ -n "$SUP" ] && alive $SUP; then
    t1=$(date +%s.%N)
    case $how in
      0) kill -INT $SUP; w=INT;; 1) kill -INT $SUP; sleep 0.3; alive $SUP && kill -INT $SUP; w=INTx2;;
      2) kill -TERM $SUP; w=TERM;; 3) kill -KILL $SUP; w=KILL;; 4) kill -HUP $SUP; w=HUP;;
      5) $RC control $R/int stop > /dev/null 2>&1; w=ctlstop;;
    esac
    for k in $(seq 1 600); do alive $SUP || break; sleep 0.1; done
    if [ $st = 1 ]; then sleep 0.3; kill -TERM $STP 2>/dev/null; wait $STP; rc=$(grep -h "^$SUP .*+++ exited with" $R/strace.$i | tail -1 | grep -o '[0-9]* +++' | cut -d' ' -f1); rc=${rc:-137}; else wait $STP; rc=$?; fi
    echo "it $i: $w at ${ms}ms -> rc=$rc after $(awk "BEGIN{printf \"%.1f\", $(date +%s.%N)-$t1}")s marked_left=$(marked "RR_CAMPAIGN_JOB=$(campid $R/int):" | wc -w)"
  else
    if [ $st = 1 ]; then for k in $(seq 1 3000); do alive $SUP || break; sleep 0.1; done; sleep 0.3; kill -TERM $STP 2>/dev/null; wait $STP; rc=$(grep -h "^$SUP .*+++ exited with" $R/strace.$i | tail -1 | grep -o '[0-9]* +++' | cut -d' ' -f1); else wait $STP; rc=$?; fi; echo "it $i: finished before the interruption rc=$rc"
  fi
done
$RC verify $R/int > /dev/null 2>&1; echo "verify=$? final rc=$rc reference rc=$refrc $( [ "$rc" = "$refrc" ] && echo RC_SAME || echo RC_DIFFERS) iterations=$i"
$RC result $R/ref > $R/ref.result; $RC result $R/int > $R/int.result
echo "$(python3 -I $S/scripts/rescmp.py $R/ref.result $R/int.result)"
echo "work dir owner: $(cat $R/w-int/RR_CAMPAIGN_WORK_OWNER) campaign id: $(campid $R/int); creating-dirs left: $(find $R/w-int -maxdepth 2 -name '.*creating*' | wc -l); unrecorded moved aside: $(find $R/w-int -maxdepth 2 -name '*.unrecorded-*' | wc -l); refused starts: $(grep -l 'belongs to campaign' $R/int.*.out 2>/dev/null | wc -l)"
echo "MARKED_LEFT=$(marked "RR_CAMPAIGN_JOB=$(campid $R/int):" | wc -w) HOSTILE_ANY_LEFT=$(for p in /proc/[0-9]*; do grep -q hostile $p/cmdline 2>/dev/null && echo x; done | wc -l)"
if [ $st = 1 ]; then python3 -I $S/scripts/order_check.py $R/strace.* -- $R/int | head -8; xz -T1 -q $R/strace.*; fi
