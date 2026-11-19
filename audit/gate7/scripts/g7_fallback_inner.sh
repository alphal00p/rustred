# gate7 (c) m3-a: is a stale tmpfs fallback intent log of ANOTHER campaign directory merged? INSIDE ns() (root map: tmpfs mount).
# Campaign A writes an intent to its fallback (primary log full: intent_primary_enospc:1) and aborts after its first send
# (signal_after_send:1): A's fallback file stays. Campaign B is a NEW directory; A's fallback key directory is renamed to
# B's <dev>-<ino> (= what a reused inode number gives). B's first start reconciles "earlier sessions" from the fallback.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate7/scripts/lib.sh
in_ns_guard6
tag=$1
R=$S/runs/fallback/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R/xdg
mount -t tmpfs -o size=4m,mode=0700 tmpfs $R/xdg || { echo "tmpfs mount failed"; exit 5; }
export XDG_RUNTIME_DIR=$R/xdg
CAMP=campa P_CRASH=0 P_MEM=0 P_NORULE=0 CASE_MS='[600000,600000]' make_spec $R/a.json $R/a $FC 9 1
RR_CAMPAIGN_FAILPOINT=intent_primary_enospc:1,signal_after_send:1 $RC run --spec $R/a.json --dir $R/a --plain > $R/a1.out 2>&1 & SUP=$!
for i in $(seq 1 600); do grep -qs '"ev":"started"' $R/a/ledger/*.jsonl && break; sleep 0.1; done
sleep 1; kill -INT $SUP; wait $SUP; echo "campaign A rc=$? (134 = abort after the first send)"
IDA=$(campid $R/a); echo "A id $IDA"
find $R/xdg -type f | while read f; do echo "  fallback file $f: $(grep -c '"kind":"intent"' $f) intent(s)"; done
KA=$(ls $R/xdg/rr-campaign-intents-0/); echo "A key dir $KA (A dir: $(stat -c %d-%i $R/a))"
CAMP=campb P_CRASH=0 P_MEM=0 P_NORULE=0 CASE_MS='[300,600]' make_spec $R/b.json $R/b $FC 10 2
mkdir -p $R/b; KB=$(stat -c %d-%i $R/b)
mv $R/xdg/rr-campaign-intents-0/$KA $R/xdg/rr-campaign-intents-0/$KB; echo "renamed A's key dir to B's key $KB (simulated inode reuse)"
timeout -s KILL 600 $RC run --spec $R/b.json --dir $R/b --plain > $R/b1.out 2>&1; echo "campaign B rc=$?"
IDB=$(campid $R/b); echo "B id $IDB"
grep -h "intent" $R/b1.out | sed 's/^[0-9TZ:.-]* //' | cut -c1-260 | head -6 | sed 's/^/  B log: /'
echo "B ledger signal_sent records: $(grep -h -c '"signal_sent"' $R/b/ledger/*.jsonl | paste -sd+ | bc)"
grep -h '"signal_sent"' $R/b/ledger/*.jsonl | grep -o '"pid":[0-9]*\|"marker":"[^"]*"\|recovered at restart[^"]*' | paste -sd' ' | cut -c1-400 | sed 's/^/  B signal_sent: /'
echo "B ledger records naming A's id: $(grep -h -c "$IDA" $R/b/ledger/*.jsonl | paste -sd+ | bc)"
grep -h "signal_intents_reconciled" $R/b/ledger/*.jsonl | grep -o '"text":"[^"]\{0,200\}' | sed 's/^/  B note: /'
echo "fallback files left: $(find $R/xdg -type f | wc -l)"
$RC verify $R/b > $R/verify.out 2>&1; echo "B verify rc=$?"
umount $R/xdg
