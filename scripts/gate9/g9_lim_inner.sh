. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/scripts/lib.sh
in_ns_guard6; shm_guard
for v in 0 1 86400 86401 1000000; do R=$S/runs/lim/v$v; mkdir -p $R
  EXTRA_RES=",\"unproven_end_limit_s\":$v" P_CRASH=0 P_MEM=0 P_NORULE=0 make_spec $R/spec.json $R/c $FC 5 2
  $RC run --spec $R/spec.json --dir $R/c --plain > $R/out 2>&1; echo "unproven_end_limit_s=$v rc=$? $(grep -o 'unproven_end_limit_s[^"]*' $R/out | head -1 | cut -c1-120)"; done
