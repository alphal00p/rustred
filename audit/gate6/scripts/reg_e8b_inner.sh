# E8b (c): full disk by a filler file on a private tmpfs; free space (rm filler) and resume.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6/scripts/lib.sh
in_ns_guard6
R=$S/runs/reg_e8b; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
DISK_FLOOR=1K CASE_MS='[300,900]' P_CRASH=0 make_spec $R/ref.json $R/ref $FC 41 12
$RC run --spec $R/ref.json --dir $R/ref --plain > $R/ref.out 2>&1; echo "reference rc=$?"; $RC result $R/ref > $R/ref.result
for free in 300 700 1200; do
  M=$R/m$free; mkdir -p $M; mount -t tmpfs -o size=8m tmpfs $M
  dd if=/dev/zero of=$M/filler bs=1k count=$((8192-free)) status=none 2>/dev/null
  D=$M/c; DISK_FLOOR=1K CASE_MS='[300,900]' P_CRASH=0 make_spec $R/s$free.json $D $FC 41 12
  sed -i "s#\"campaign_dir\":\"$D\"#\"campaign_dir\":\"$D\"#" $R/s$free.json
  for k in 1 2 3; do
    timeout -s KILL 240 $RC run --spec $R/s$free.json --dir $D --plain > $R/s$free.run$k.out 2>&1; rc=$?
    echo "free=${free}k run$k rc=$rc $(grep -h -o 'ERROR.*' $R/s$free.run$k.out | tail -1 | cut -c1-110)"
    [ $rc = 0 ] && break
    rmfile6 $M/filler   # the operator frees space and runs the same command
  done
  $RC verify $D > $R/s$free.verify 2>&1; v=$?
  $RC result $D > $R/s$free.result 2>&1
  q=$(cat $D/ledger/*.jsonl | grep -c '"job_queued"'); rows=$(cat $R/s$free.run*.out | grep -o '(row [0-9a-z]*)' | sort | uniq -c | tr '\n' ' ')
  # result.json differs only by campaign_dir? compare the dispositions/accepted
  same=$(python3 -I -c 'import json,sys; a=json.load(open(sys.argv[1])); b=json.load(open(sys.argv[2])); print("yes" if (a.get("accepted"),a.get("dispositions"),a.get("terminals"))==(b.get("accepted"),b.get("dispositions"),b.get("terminals")) else "no")' $R/s$free.result $R/ref.result 2>&1)
  echo "   verify=$v job_queued=$q result_same=$same rows: $rows torn_files=$(ls $D/ledger | grep -c torn) marked_left=$(marked "RR_CAMPAIGN_JOB=$(campid $D):" | wc -w)"
  head -3 $R/s$free.verify | cut -c1-200
done
