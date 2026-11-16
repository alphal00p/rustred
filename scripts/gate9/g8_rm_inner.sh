# gate8 (b) m-rm: TAG N : N times: two interrupted starts (TERM at 0.7 / 0.9 s), a third start, then the campaign
# directory is removed after DELAY ms (python shutil.rmtree, errors recorded, the removal continues past them like
# remove_dir_all's error stops it: mode 'stop' = stop at the first error as Rust's remove_dir_all does); the supervisor's
# stderr is kept OUTSIDE the removed tree. Reports exit codes; for codes other than 70 keeps what remained.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/scripts/lib.sh
in_ns_guard6; shm_guard
tag=$1; N=${2:-20}; DELAY=${DELAY:-400}
R=$S/runs/rm8/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
for i in $(seq 1 $N); do
  D=$R/c$i; P_CRASH=0.1 CASE_MS='[300,1500]' make_spec $R/s$i.json $D $FC $((40+i)) 12
  for d in 0.7 0.9; do $RC run --spec $R/s$i.json --dir $D --plain > $R/pre$i.err 2>&1 & p=$!; sleep $d; alive $p && kill -TERM $p; wait $p; done
  $RC run --spec $R/s$i.json --dir $D --plain > $R/run$i.err 2>&1 & p=$!
  sleep $(awk "BEGIN{print $DELAY/1000}")
  python3 -I - "$D" > $R/rm$i.txt 2>&1 <<'P'
import os, sys, shutil
d = sys.argv[1]
assert d.startswith("/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/runs/rm8/")
errs = []
def onerr(func, path, exc):
    errs.append(f"{func.__name__} {path}: {exc[1]}")
    raise exc[1]
try:
    shutil.rmtree(d, onerror=onerr)
    print("removed completely")
except Exception as e:
    print("removal STOPPED at the first error:", errs[-1] if errs else e)
P
  wait $p; rc=$?
  left=$( [ -d $D ] && find $D -maxdepth 2 | sed "s#$D##" | tr '\n' ' ' | cut -c1-300 || echo none)
  echo "run $i: rc=$rc removal: $(cat $R/rm$i.txt | tr '\n' ' ' | cut -c1-200) left: $left"
  [ $rc != 70 ] && grep -h 'ERROR\|WARN\|FINISHED\|exit\|attention' $R/run$i.err | sed 's/^[0-9TZ:.-]* //' | cut -c1-220 | tail -6 | sed 's/^/    log: /'
done
