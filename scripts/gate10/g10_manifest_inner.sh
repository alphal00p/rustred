# gate10 (g) G9-B1 variants: TAG MODE(r|u) VARIANT. A crashed session 1 left a MARKED daemon; the ledger's MANIFEST.json is
# made: dangling (dangling symlink), dir (a directory), zero (zero length), torn (cut in half), gone (removed; the ledger
# directory keeps its segments), eacces (ledger dir mode 000; u mode only), unreadable (manifest mode 000; u only), foreign
# (a FRESH campaign whose ledger/ holds only a foreign file). Session 2 must refuse typed (never 0), touch nothing, signal
# nothing; restored, session 3 recovers (kills the daemon, exit 0). Prints RESULT.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate10/scripts/lib.sh
in_ns_guard6; shm_guard
tag=$1; mode=$2; v=$3
R=$S/runs/mf10/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
case $mode in u) hostlike || { echo "NOT HOST-LIKE: abort"; exit 99; };; r) hostlike && { echo "HOST-LIKE in r mode: abort"; exit 99; };; esac
cat > $R/wrap.sh <<W
#!$BASH_BIN
if [ "\$1" = job ] && [ ! -e $R/daemon.pids ]; then $S/bin/g7hostile ndaemon $R/daemon.pids; fi
exec $FC "\$@"
W
chmod 755 $R/wrap.sh
P_CRASH=0 P_MEM=0 P_NORULE=0 CASE_MS='[1500,2500]' MAXJOBS=2 make_spec $R/spec.json $R/c $R/wrap.sh 5 4
L=$R/c/ledger; M=$L/MANIFEST.json; DP=""
if [ $v = foreign ]; then
  mkdir -p $L; echo "not ours" > $L/foreign.txt
else
  $RC run --spec $R/spec.json --dir $R/c --plain > $R/run1.out 2>&1 & SUP=$!
  for i in $(seq 1 300); do [ -s $R/daemon.pids ] && break; sleep 0.1; done
  sleep 0.5; kill -KILL $SUP; wait $SUP 2>/dev/null
  read DP DM < $R/daemon.pids
  cp -a $M $R/manifest.save
fi
case $v in
  dangling) rm -- $M; ln -s $R/nonexistent-target $M;;
  dir) rm -- $M; mkdir $M;;
  zero) : > $M;;
  torn) head -c $(( $(stat -c %s $R/manifest.save) / 2 )) $R/manifest.save > $M;;
  gone) rm -- $M;;
  eacces) chmod 000 $L;;
  unreadable) chmod 000 $M;;
  foreign) ;;
esac
sum() { (cd $L 2>/dev/null && for f in $(ls -A 2>/dev/null); do [ -f "$f" ] && [ ! -L "$f" ] && echo "$f $(sha256sum < $f | cut -c1-16)"; done) 2>/dev/null; }
[ $v = eacces ] && b4=$(chmod 700 $L; sum; chmod 000 $L) || b4=$(sum)
timeout -s KILL 300 $RC run --spec $R/spec.json --dir $R/c --plain > $R/run2.out 2>&1; rc=$?
[ $v = eacces ] && af=$(chmod 700 $L; sum; chmod 000 $L) || af=$(sum)
same=TOUCHED; [ "$b4" = "$af" ] && same=untouched
al=n/a; [ -n "$DP" ] && { alive $DP && al=YES || al=no; }
msg=$(grep -h 'rr_campaign:\|ERROR' $R/run2.out | tail -1 | cut -c1-200)
# restore and recover
case $v in
  dangling) rm -- $M; cp -a $R/manifest.save $M;;
  dir) rmdir $M; cp -a $R/manifest.save $M;;
  zero|torn|gone) cp -a $R/manifest.save $M;;
  eacces) chmod 700 $L;;
  unreadable) chmod 644 $M;;
  foreign) rm -- $L/foreign.txt;;
esac
timeout -s KILL 300 $RC run --spec $R/spec.json --dir $R/c --plain > $R/run3.out 2>&1; rc3=$?
al3=n/a; [ -n "$DP" ] && { alive $DP && al3=YES || al3=no; }
flag=""; [ "$rc" = 0 ] && flag=" RC0_UNDER_DAMAGE"; [ "$same" = TOUCHED ] && flag="$flag LEDGER_TOUCHED"
echo "RESULT tag=$tag mode=$mode variant=$v rc=$rc ledger=$same daemon_alive=$al | restored: rc=$rc3 daemon_alive=$al3$flag | $msg"
[ -n "$DP" ] && alive $DP && kill -KILL $DP
exit 0
