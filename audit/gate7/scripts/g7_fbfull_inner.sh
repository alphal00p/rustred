# gate7 (c) m3-a: the tmpfs of the fallback is too small for its pre-allocation (size=128k) while the campaign
# directory's intent log is full (intent_primary_enospc:1). INSIDE ns() (root map: tmpfs mount). Expected: start log
# 'no tmpfs fallback', the signals NOT sent and listed, exit 75; then the same command without the failpoint ends normally.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate7/scripts/lib.sh
in_ns_guard6
tag=$1
R=$S/runs/fbfull/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R/xdg
mount -t tmpfs -o size=128k,mode=0700 tmpfs $R/xdg || exit 5
export XDG_RUNTIME_DIR=$R/xdg
P_CRASH=0 P_MEM=0 P_NORULE=0 CASE_MS='[600000,600000]' make_spec $R/spec.json $R/c $FC 9 2
RR_CAMPAIGN_FAILPOINT=intent_primary_enospc:1 $RC run --spec $R/spec.json --dir $R/c --plain > $R/run1.out 2>&1 & SUP=$!
for i in $(seq 1 600); do [ "$(grep -hs -c '"ev":"started"' $R/c/ledger/*.jsonl | paste -sd+ | python3 -c 'import sys; print(sum(int(x) for x in sys.stdin.read().split("+") if x.strip()))')" -ge 2 ] && break; sleep 0.1; done
sleep 1; kill -INT $SUP; wait $SUP; echo "run 1 rc=$?"
grep -h "fallback\|NOT sent\|not sent" $R/run1.out | sed 's/^[0-9TZ:.-]* //' | cut -c1-220 | sort | uniq -c | head -6 | sed 's/^/  log: /'
ID=$(campid $R/c); echo "marked after run 1: $(marked "RR_CAMPAIGN_JOB=$ID:" | wc -w); tmpfs files: $(find $R/xdg -type f | wc -l)"
timeout -s KILL 300 $RC run --spec $R/spec.json --dir $R/c --plain > $R/run2.out 2>&1 & SUP=$!
sleep 8; $RC control $R/c stop >/dev/null 2>&1; wait $SUP; echo "run 2 (no failpoint, then control stop) rc=$?; marked after: $(marked "RR_CAMPAIGN_JOB=$ID:" | wc -w)"
$RC verify $R/c > $R/v.out 2>&1; echo "verify rc=$?"
umount $R/xdg
