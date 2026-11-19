# gate8 (c): TAG VARIANT inside ns8r (root map, private /dev/shm; uid 0 in the ns, so the fallback base is
# /dev/shm/rr-campaign-intents-0):
#  x1 base pre-created mode 0777 | x2 base is a symlink to a directory we own | x3 key dir pre-created mode 0755
#  x4 a STALE log of THIS campaign from an UNRECORDED session (header names this campaign id) with an intent line
#  x5 the supervisor under ulimit -f 64 (KiB): expect 75 (drain), then the same command with no limit == reference
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/scripts/lib.sh
in_ns_guard6; shm_guard
tag=$1; v=$2
R=$S/runs/c8/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
B=/dev/shm/rr-campaign-intents-$(id -u)
[ $v = x2 ] && { mkdir -p $R/target; chmod 700 $R/target; ln -s $R/target $B; echo "base is a symlink: $(stat -c %F $B)"; }
P_CRASH=0 CASE_MS='[400,1200]' make_spec $R/spec.json $R/c $FC 21 8
P_CRASH=0 CASE_MS='[400,1200]' make_spec $R/ref.json $R/ref $FC 21 8
$RC run --spec $R/ref.json --dir $R/ref --plain > $R/ref.out 2>&1; echo "reference rc=$?"; $RC result $R/ref > $R/ref.result
mkdir -p $R/c
case $v in
  x1) mkdir -m 0777 $B; chmod 0777 $B;;
  x3) mkdir -m 0700 $B; k=$(stat -c '%d-%i' $R/c); mkdir -m 0755 $B/$k; chmod 0755 $B/$k;;
esac
if [ $v = x4 ]; then
  $RC run --spec $R/spec.json --dir $R/c --plain > $R/run0.out 2>&1; echo "first run rc=$?"
  ID=$(campid $R/c); k=$(stat -c '%d-%i' $R/c); mkdir -p -m 0700 $B; mkdir -p -m 0700 $B/$k
  printf '%s\n' "{\"schema\":\"rustred.campaign.signal-intent.v1\",\"kind\":\"header\",\"id\":\"deadbeef.4242.12345\",\"campaign\":\"$ID\"}" "{\"schema\":\"rustred.campaign.signal-intent.v1\",\"kind\":\"intent\",\"id\":\"i-stale-1\",\"pid\":4242,\"start_ticks\":12345,\"signal\":9,\"target\":\"process\",\"marker\":\"$ID:N0.i0000.L0-t0:a1\",\"proof\":\"marker\",\"by\":\"stale\"}" > $B/$k/deadbeef.4242.12345.jsonl
  chmod 600 $B/$k/deadbeef.4242.12345.jsonl
  $RC control $R/c retry N0.i0000.L0 > /dev/null 2>&1
fi
if [ $v = x5 ]; then ( ulimit -f 64; exec $RC run --spec $R/spec.json --dir $R/c --plain > $R/run1.out 2>&1 ); rc=$?; echo "under ulimit -f 64: rc=$rc"; grep -h 'ERROR\|limit\|75\|DISK' $R/run1.out | sed 's/^[0-9TZ:.-]* //' | cut -c1-200 | tail -3 | sed 's/^/  log: /'; fi
$RC run --spec $R/spec.json --dir $R/c --plain > $R/run2.out 2>&1; rc=$?
echo "run rc=$rc"
grep -h 'fallback\|foreign' $R/run2.out | sed 's/^[0-9TZ:.-]* //' | cut -c1-240 | head -4 | sed 's/^/  log: /'
echo "  ledger: signal_intents_foreign notes $(cat $R/c/ledger/*.jsonl | grep -c 'signal_intents_foreign'); signal_sent naming pid 4242: $(grep -h '"signal_sent"' $R/c/ledger/*.jsonl | grep -c '"pid":4242,'); stale file kept: $( [ -e "$B/${k:-x}/deadbeef.4242.12345.jsonl" ] && echo yes || echo no)"
$RC result $R/c > $R/c.result; echo "  $(python3 -I $S/scripts/rescmp.py $R/ref.result $R/c.result); verify $($RC verify $R/c > /dev/null 2>&1; echo $?)"
echo "RESULT tag=$tag variant=$v rc=$rc"
