# m4 (gate6 g): a child that dies for ITS OWN reasons in a way the m4 rule attributes to the disk. INSIDE ns(). TAG MODE
#  exit153 : the job program exits 153 (= 128 + SIGXFSZ) every time (no signal, disk roomy, ulimit -f unlimited)
#  selfxfsz: the job program sends ITSELF SIGXFSZ every time; the supervisor runs under a large FINITE ulimit -f
#            (1 GB: never reached) -> m4 "own file-size limit" drain; the same command is repeated up to 8 times
#  segv    : control: the job program kills itself with SIGSEGV (disk roomy): must be a crash (row 23), not row 15b
# Only items of node N0 are hostile (the wrapper checks the label); N1 items run normally.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6/scripts/lib.sh
in_ns_guard6
tag=$1; mode=$2
R=$S/runs/m4/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
case $mode in exit153) act='exit 153';; selfxfsz) act='kill -XFSZ $$';; segv) act='kill -SEGV $$';; esac
cat > $R/wrap.sh <<W
#!$BASH_BIN
if [ "\$1" = job ]; then echo "RAN \$RR_CAMPAIGN_JOB \$(date +%s.%N)" >> $R/ran.log; case "\$RR_CAMPAIGN_JOB" in *:N0.*) $act;; esac; fi
exec $FC "\$@"
W
chmod +x $R/wrap.sh
P_CRASH=0 P_MEM=0 CASE_MS='[200,400]' MAXJOBS=2 make_spec $R/spec.json $R/c $R/wrap.sh 4 4
[ $mode = selfxfsz ] && ulimit -f 2000000
runs=1; [ $mode = selfxfsz ] && runs=8
for k in $(seq 1 $runs); do
  t0=$(date +%s); timeout -s KILL 900 $RC run --spec $R/spec.json --dir $R/c --plain > $R/run$k.out 2>&1; rc=$?
  echo "run $k: rc=$rc after $(( $(date +%s)-t0 ))s; $(grep -h 'FINISHED\|OPERATOR\|DISK FULL\|exit 75\|attention' $R/run$k.out | tail -1 | cut -c1-200)"
  [ $rc = 75 ] || break
done
echo "program starts: total $(wc -l < $R/ran.log), N0 (hostile) $(grep -c ':N0\.' $R/ran.log)"
grep -o ':N0\.[^ ]*' $R/ran.log | sed 's/:a[0-9]*$//' | sort | uniq -c | sed 's/^/  attempts per hostile job: /'
grep -h "a resource end (row 15b)\|crash\|row 23\|child_resource_failure\|crashed_twice\|file-size limit" $R/run*.out | sed 's/^[0-9TZ:-]* //' | cut -c1-170 | sort | uniq -c | sort -rn | head -8 | sed 's/^/  log: /'
$RC result $R/c > $R/result.txt 2>&1
python3 -I -c 'import json,sys; d=json.load(open(sys.argv[1])); print("  result: accepted", len(d.get("accepted") or {}), "attention", d.get("attention") if "attention" in d else [k for k in d.keys()])' $R/result.txt 2>&1 | cut -c1-300
$RC verify $R/c > $R/verify.out 2>&1; echo "verify rc=$?"
