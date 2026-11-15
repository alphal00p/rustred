# m3 (gate6 g): intent log on a FULL file system. INSIDE ns() (own mount ns). TAG MODE  (MODE: prealloc | noprealloc)
# CDIR/log is a 1 MiB private tmpfs; the 64 KiB reserve is disabled (a directory takes its name), so after the fill the
# ONLY space for intents is the 256 KiB preallocated past the end of signal-intents.jsonl (prealloc), or none at all
# (noprealloc: failpoint no_intent_prealloc = the preallocation failed at start). 3 jobs of 60 s each spawn 2 marked
# daemons. The fill: one file grown by a filler loop that keeps taking any freed space. Then SIGINT. Strace order check:
# every signal syscall behind a durable intent (write + fdatasync before it); exit code; marked left; ERROR lines; then
# the fill file is removed and the same command recovers.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6/scripts/lib.sh
in_ns_guard6
tag=$1; mode=$2
R=$S/runs/m3/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R/c/log
mount -t tmpfs -o size=1m,mode=0700 tmpfs $R/c/log || { echo "mount failed"; exit 2; }
mkdir $R/c/log/signal-intents.reserve
make_wrapper $R/wrap.sh daemon daemon
CASE_MS='[60000,60000]' P_CRASH=0 P_MEM=0 P_NORULE=0 MAXJOBS=3 GRACE=2 make_spec $R/spec.json $R/c $R/wrap.sh 3 3
FPV=""; [ $mode = noprealloc ] && FPV=no_intent_prealloc:1
env ${FPV:+RR_CAMPAIGN_FAILPOINT=$FPV} strace -I 1 -f -ttt -q --decode-fds=path,pidfd -s 1000000 -e trace=write,fdatasync,fsync,fallocate,pidfd_send_signal,pidfd_open,kill,tgkill,tkill,execve,clone,clone3,fork,vfork -e signal=none \
  -o $R/strace.1 $RC run --spec $R/spec.json --dir $R/c --plain > $R/run1.out 2>&1 & STP=$!
SUPP=""; for i in $(seq 1 400); do SUPP=$(grep -m1 'execve(.*rr_campaign", \[.*"run".*) = 0' $R/strace.1 2>/dev/null | awk '{print $1}'); [ -n "$SUPP" ] && break; sleep 0.02; done
for i in $(seq 1 150); do n=$(cat $R/c/ledger/*.jsonl 2>/dev/null | grep -c '"ev":"started"'); [ "${n:-0}" -ge 3 ] && break; sleep 0.2; done
sleep 2; ID=$(campid $R/c)
echo "supervisor $SUPP; started=$n; marked: $(marked "RR_CAMPAIGN_JOB=$ID:" | wc -w); intent log size $(stat -c %s $R/c/log/signal-intents.jsonl) bytes, allocated $(( $(stat -c %b $R/c/log/signal-intents.jsonl) * 512 )) bytes"
grep -h 'fallocate(' $R/strace.1 | head -2 | cut -c1-160 | sed 's/^/  /'
dd if=/dev/zero of=$R/c/log/fill bs=4k 2>/dev/null
( while :; do dd if=/dev/zero of=$R/c/log/fill bs=4k count=16 oflag=append conv=notrunc 2>/dev/null; sleep 0.005; done ) & FILL=$!
sleep 0.5; echo "df log: $(df -k $R/c/log | tail -1 | awk '{print $4}') KB free"
T0=$(date +%s.%N); kill -INT $SUPP; echo "SIGINT sent"
for t in $(seq 1 100); do sleep 2; { [ -d /proc/$SUPP ] && ! grep -q '^State:[[:space:]]*[ZX]' /proc/$SUPP/status; } || break; done
if [ -d /proc/$SUPP ] && ! grep -q '^State:[[:space:]]*[ZX]' /proc/$SUPP/status; then echo "STUCK: SIGKILL exact pid $SUPP"; kill -KILL $SUPP; fi
sleep 0.5; kill -TERM $STP 2>/dev/null; wait $STP 2>/dev/null
echo "run1 end after $(awk "BEGIN{printf \"%.1f\", $(date +%s.%N)-$T0}") s: $(grep -h "^$SUPP .*+++ \(exited\|killed\)" $R/strace.1 | tail -1 | cut -d' ' -f3-)"
grep -h 'NOT sent\|not durable\|DISK FULL\|signals_not_sent\|abandoned\|FINISHED\|stopped' $R/run1.out | cut -c1-220 | head -8 | sed 's/^/  log: /'
echo "marked after run1: $(marked "RR_CAMPAIGN_JOB=$ID:" | wc -w); intents: $(grep -c '"kind":"intent"' $R/c/log/signal-intents.jsonl)"
kill -KILL $FILL 2>/dev/null; wait $FILL 2>/dev/null
cp $R/c/log/signal-intents.jsonl $R/intents.run1.jsonl 2>/dev/null
python3 -I $S/scripts/order_check.py $R/strace.1 -- $R/c | head -6
rmfile6 $R/c/log/fill
echo "space freed: $(df -k $R/c/log | tail -1 | awk '{print $4}') KB free"
timeout -s KILL 600 $RC run --spec $R/spec.json --dir $R/c --plain > $R/run2.out 2>&1; echo "run2 rc=$?"
echo "MARKED_LEFT=$(marked "RR_CAMPAIGN_JOB=$ID:" | wc -w)"
$RC verify $R/c > $R/verify.out 2>&1; echo "verify rc=$?"
echo "POSSIBLY SENT records: $(grep -h '"signal_sent"' $R/c/ledger/*.jsonl | grep -c 'POSSIBLY SENT')"
cp $R/c/log/supervisor.log $R/supervisor.log 2>/dev/null
xz -T1 -q $R/strace.1 2>/dev/null
umount $R/c/log
