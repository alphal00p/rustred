. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/scripts/lib.sh
in_ns_guard6; shm_guard
R=$S/runs/segv/$1; mkdir -p $R; shift
P_CRASH=0 P_MEM=0 P_NORULE=0 make_spec $R/spec.json $R/c $FC 5 2
env LD_PRELOAD=$S/bin/g9fault.so G8_LOG=$R/fault.log "$@" $RC run --spec $R/spec.json --dir $R/c --plain > $R/out.txt 2>&1; echo "rc=$?"
tail -3 $R/out.txt; tail -3 $R/fault.log
env LD_PRELOAD=$S/bin/g9fault.so G8_LOG=$R/fault2.log "$@" strace -f -o $R/strace.txt $RC run --spec $R/spec.json --dir $R/c2 --plain > $R/out2.txt 2>&1; echo "strace rc=$?"; grep -n "SIGSEGV\|si_addr" $R/strace.txt | head -3; grep -B5 "SIGSEGV" $R/strace.txt | head -12
