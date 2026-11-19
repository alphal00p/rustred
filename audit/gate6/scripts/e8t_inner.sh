# E8T (gate6 c): whole CDIR on a 600k tmpfs (ledger disk full -> 75); verify WHILE full (print its output), then copy the
# directory to roomy storage and verify / resume there (the tmpfs cannot be remounted inside the userns). INSIDE ns().
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6/scripts/lib.sh
in_ns_guard6
R=$S/runs/e8t; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R/m
CASE_MS='[300,900]' P_CRASH=0 make_spec $R/ref.json $R/ref $FC 41 12
$RC run --spec $R/ref.json --dir $R/ref --plain > $R/ref.out 2>&1; echo "reference rc=$?"; $RC result $R/ref > $R/ref.result
mount -t tmpfs -o size=600k tmpfs $R/m || { echo "mount failed"; exit 2; }
DISK_FLOOR=1K CASE_MS='[300,900]' P_CRASH=0 make_spec $R/s.json $R/m/c $FC 41 12
timeout -s KILL 240 $RC run --spec $R/s.json --dir $R/m/c --plain > $R/run1.out 2>&1; echo "run1 rc=$? $(grep -h -o 'ERROR.*' $R/run1.out | tail -1 | cut -c1-140)"
$RC verify $R/m/c > $R/verify_full.out 2>&1; echo "verify while full rc=$?"; head -8 $R/verify_full.out | cut -c1-220
cp -a $R/m/c $R/copy_c; umount $R/m
$RC verify $R/copy_c > $R/verify_copy.out 2>&1; echo "verify of the byte copy on roomy storage rc=$?"; head -5 $R/verify_copy.out | cut -c1-220
timeout -s KILL 240 $RC run --spec $R/s.json --dir $R/copy_c --plain --adopt-moved-campaign > $R/run2.out 2>&1; echo "resume of the copy (adopt) rc=$?"
$RC result $R/copy_c > $R/copy.result 2>&1; echo "$(python3 -I $S/scripts/rescmp.py $R/ref.result $R/copy.result); verify $($RC verify $R/copy_c > /dev/null 2>&1; echo $?)"
