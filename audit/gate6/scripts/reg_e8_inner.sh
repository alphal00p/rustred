# E8 (c): low limits. Each case: run under the limit, then resume without it; compare with the reference.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6/scripts/lib.sh
in_ns_guard6
R=$S/runs/reg_e8; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
CASE_MS='[300,900]' P_CRASH=0 make_spec $R/ref.json $R/ref $FC 41 12
$RC run --spec $R/ref.json --dir $R/ref --plain > $R/ref.out 2>&1; echo "reference rc=$?"; $RC result $R/ref > $R/ref.result
refq=$(cat $R/ref/ledger/*.jsonl | grep -c '"job_queued"')
check() { # check NAME DIR
  local D=$2
  $RC run --spec $R/$1.json --dir $D --plain > $D.resume.out 2>&1; local rrc=$?
  $RC verify $D > /dev/null 2>&1; local v=$?
  $RC result $D > $D.result 2>&1
  local q=$(cat $D/ledger/*.jsonl | grep -c '"job_queued"'); local a=$(cat $D/ledger/*.jsonl | grep -c '"ev":"launch"')
  local same=$(python3 -I $S/scripts/rescmp.py $R/ref.result $D.result)
  echo "   resume rc=$rrc verify=$v job_queued=$q (ref $refq) launches=$a result_same=$same marked_left=$(marked "RR_CAMPAIGN_JOB=$(campid $D):" | wc -w)"
}
for n in 12 24 40; do
  D=$R/nofile$n; CASE_MS='[300,900]' P_CRASH=0 make_spec $R/nofile$n.json $D $FC 41 12
  ( ulimit -n $n; timeout -s KILL 240 $RC run --spec $R/nofile$n.json --dir $D --plain > $D.out 2>&1; echo "nofile=$n rc=$? $(grep -h -o 'ERROR.*' $D.out | tail -1 | cut -c1-160)" )
  check nofile$n $D
done
for n in 6 12 20; do
  D=$R/nproc$n; CASE_MS='[300,900]' P_CRASH=0 make_spec $R/nproc$n.json $D $FC 41 12
  ( ulimit -u $n; timeout -s KILL 240 $RC run --spec $R/nproc$n.json --dir $D --plain > $D.out 2>&1; echo "nproc=$n rc=$? $(grep -h -o 'ERROR.*\|panicked.*' $D.out | tail -1 | cut -c1-160)" )
  check nproc$n $D
done
for kb in 64 512; do
  D=$R/fsize$kb; CASE_MS='[300,900]' P_CRASH=0 make_spec $R/fsize$kb.json $D $FC 41 12
  ( ulimit -f $kb; timeout -s KILL 240 $RC run --spec $R/fsize$kb.json --dir $D --plain > $D.out 2>&1; echo "fsize=${kb}k rc=$? $(grep -h -o 'ERROR.*' $D.out | tail -1 | cut -c1-160)" )
  check fsize$kb $D
done
for sz in 600k 2m 6m; do
  M=$R/tmpfs$sz; mkdir -p $M; mount -t tmpfs -o size=$sz tmpfs $M || { echo "mount failed"; continue; }
  D=$M/c; DISK_FLOOR=1K CASE_MS='[300,900]' P_CRASH=0 make_spec $R/tmpfs$sz.json $D $FC 41 12
  timeout -s KILL 240 $RC run --spec $R/tmpfs$sz.json --dir $D --plain > $R/tmpfs$sz.out 2>&1; echo "tmpfs=$sz rc=$? $(grep -h -o 'ERROR.*' $R/tmpfs$sz.out | tail -1 | cut -c1-160)"
  df -k $M | tail -1 | awk '{print "   used " $3 "k of " $2 "k"}'
  mount -o remount,size=200m $M
  check tmpfs$sz $D
done
