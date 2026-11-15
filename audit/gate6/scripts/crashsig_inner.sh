# CRASHSIG (gate6 a/e): crash the supervisor AT a signal (failpoint signal_after_intent:K / signal_after_send:K) with
# hostile marked daemons + flipexec among the children; both sessions under strace; order check; restart must leave 0
# marked, reconcile every intent (delivered-not-recorded / POSSIBLY SENT), verify 0. INSIDE ns(). TAG SITE K
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6/scripts/lib.sh
in_ns_guard6
tag=$1; site=$2; K=$3
R=$S/runs/crashsig/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
make_wrapper $R/wrap.sh daemon flipexec chainroot
CASE_MS='[400,1500]' P_CRASH=0 make_spec $R/spec.json $R/c $R/wrap.sh 13 8
STR="strace -I 1 -f -ttt -q --decode-fds=path,pidfd -s 1000000 -e trace=write,fdatasync,fsync,pidfd_send_signal,pidfd_open,kill,tgkill,tkill,execve,clone,clone3,fork,vfork -e signal=none"
for s in 1 2; do
  if [ $s = 1 ]; then env RR_CAMPAIGN_FAILPOINT=$site:$K $STR -o $R/strace.$s $RC run --spec $R/spec.json --dir $R/c --plain > $R/run$s.out 2>&1 & STP=$!
  else $STR -o $R/strace.$s $RC run --spec $R/spec.json --dir $R/c --plain > $R/run$s.out 2>&1 & STP=$!; fi
  SUP=""; for i in $(seq 1 400); do SUP=$(grep -m1 'execve(.*rr_campaign", \[.*"run".*) = 0' $R/strace.$s 2>/dev/null | awk '{print $1}'); [ -n "$SUP" ] && break; sleep 0.02; done
  for i in $(seq 1 3000); do alive $SUP || break; sleep 0.1; done
  sleep 0.3; kill -TERM $STP 2>/dev/null; wait $STP 2>/dev/null
  echo "session $s ($([ $s = 1 ] && echo "failpoint $site:$K" || echo restart)): $(grep -h "^$SUP .*+++ \(exited\|killed\)" $R/strace.$s | tail -1 | sed 's/.*+++ \(.*\) +++/\1/'); marked after: $(marked "RR_CAMPAIGN_JOB=$(campid $R/c):" | wc -w)"
done
python3 -I $S/scripts/order_check.py $R/strace.1 $R/strace.2 -- $R/c | head -8
echo "reconciled: delivered-not-recorded $(grep -h '"signal_sent"' $R/c/ledger/*.jsonl | grep -c 'not recorded before'), POSSIBLY SENT $(grep -h '"signal_sent"' $R/c/ledger/*.jsonl | grep -c 'POSSIBLY SENT'), stop-hold kills $(grep -h killed_by_stop_hold $R/c/ledger/*.jsonl | wc -l)"
echo "HOSTILE_ANY_LEFT=$(for p in /proc/[0-9]*; do grep -q hostile $p/cmdline 2>/dev/null && echo x; done | wc -l) STOPPED_LEFT=$(for p in /proc/[0-9]*; do grep -q '^State:[[:space:]]*T' $p/status 2>/dev/null && echo x; done | wc -l)"
$RC verify $R/c > $R/verify.out 2>&1; echo "verify rc=$?"
xz -T1 -q $R/strace.* 2>/dev/null
