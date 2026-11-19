# gate11 (a)(2) false-unproven check: SEED WHICH(ref|run|term) : one fake campaign (p_crash 0, p_norule 0, p_mem 0). ref: the
# plain fake child. run / term: every job's wrapper starts a loop that spawns SHORT-LIVED MARKED helpers (10-50 ms, every
# third one setsid'd) while the job leader lives; the last helpers outlive the leader by up to 50 ms and exit NORMALLY (the
# end-of-job sweep's scan sees them, they are gone before its stop: "vanished"). term: SIGTERM at a random 3-12 s, then the
# same command (resume). Prints HELPERS seed which rc(s) verify unproven-lines vanished-notes resolved-notes.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate11/scripts/lib.sh
in_ns_guard6; shm_guard
seed=$1; which=$2; R=$S/runs/helpers11/s$seed; mkdir -p $R
prog=$FC
if [ "$which" != ref ]; then
  prog=$R/wrap.sh
  cat > $prog <<W
#!$BASH_BIN
if [ "\$1" = job ]; then
  P=\$\$
  ( while [ "\$(cut -d' ' -f3 /proc/\$P/stat 2>/dev/null)" = S ] || [ "\$(cut -d' ' -f3 /proc/\$P/stat 2>/dev/null)" = R ]; do
      sleep 0.0\$((RANDOM%5+1)) &
      [ \$((RANDOM%3)) = 0 ] && setsid -f sleep 0.0\$((RANDOM%5+1))
      sleep 0.01
    done ) &
fi
exec $FC "\$@"
W
  chmod 755 $prog
fi
P_CRASH=0 P_MEM=0 P_NORULE=0 CASE_MS=${HCASE:-'[500,3000]'} MAXJOBS=4 make_spec $R/$which.json $R/$which $prog $seed ${HITEMS:-40}
t0=$(date +%s); rcs=""
if [ "$which" = term ] || [ "$which" = kill ]; then
  sig=TERM; [ "$which" = kill ] && sig=KILL
  $RC run --spec $R/$which.json --dir $R/$which --plain > $R/$which.out 2>&1 & SUP=$!
  sleep $((RANDOM%10+3)); kill -$sig $SUP; wait $SUP; rcs="$?,"
fi
$RC run --spec $R/$which.json --dir $R/$which --plain >> $R/$which.out 2>&1; rc=$?; rcs="$rcs$rc"
# kill mode: an exit 8 of the resume (a vanish of the dead session's helper: no atomic proof in a namespace) -> once more
if [ "$which" = kill ] && [ "$rc" = 8 ]; then $RC run --spec $R/$which.json --dir $R/$which --plain >> $R/$which.out 2>&1; rc=$?; rcs="$rcs,$rc"; fi
$RC result $R/$which > $R/$which.result 2>/dev/null; v=$($RC verify $R/$which > /dev/null 2>&1; echo $?)
nv=$(cat $R/$which/ledger/*.jsonl 2>/dev/null | grep -o 'sweep_vanished {' | wc -l); nr=$(cat $R/$which/ledger/*.jsonl 2>/dev/null | grep -o 'sweep_vanished_resolved {' | wc -l)
nk=$(grep -c 'orphan killed' $R/$which.out)
left=$(marked "$(campid $R/$which)" | wc -l)
echo "HELPERS seed=$seed which=$which rc=$rcs secs=$(( $(date +%s)-t0 )) verify=$v unproven_lines=$(grep -c 'not proven\|NOT PROVEN\|unproven' $R/$which.out) vanished_notes=${nv:-0} resolved_notes=${nr:-0} orphan_killed=$nk marked_left=$left"
