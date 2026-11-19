# gate11 (H) in a private namespace: (1) the start refusal without a scope (spec without resources.containment): exit code,
# what exists in CDIR afterwards, nothing launched; (2) the letter (a) host script's NAMESPACE SELF-TEST exactly as shipped;
# (3) the same script with only its self-test spec patched to "containment":"marker_only" (how far it goes without scopes).
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate11/scripts/lib.sh
in_ns_guard6; shm_guard
R=$S/runs/H/$1; [ -e $R ] && { echo "exists $R"; exit 3; }; mkdir -p $R
CONTAIN="" P_CRASH=0 P_MEM=0 P_NORULE=0 make_spec $R/noscope.json $R/c0 $FC 1 4
$RC run --spec $R/noscope.json --dir $R/c0 --plain > $R/noscope.out 2>&1; echo "H1 no containment key in a namespace: rc=$? ; $(grep -o 'no job scope[^;]*' $R/noscope.out | head -1 | cut -c1-150)"
echo "H1 files in CDIR: $(cd $R/c0 2>/dev/null && find . -type f | sort | tr '\n' ' ')"; echo "H1 jobs launched: $(ls $R/c0/jobs 2>/dev/null | wc -l)"
for v in copy patched; do
  OUT=$R/lettera-$v BIN=$S/bin/debug LETTER_A_NS_SELFTEST=1 timeout -s KILL 1500 bash $S/scripts/letter_a_$v.sh --run > $R/lettera-$v.txt 2>&1; rc=$?
  echo "H2 letter_a self-test ($v): rc=$rc; $(grep -c 'CHECK FAILED' $R/lettera-$v.txt) CHECK FAILED; supervisor run1: $(grep -o 'no job scope[^;(]*\|containment: marker only' $R/lettera-$v/run1.out 2>/dev/null | head -1)"
  grep 'CHECK FAILED\|ALL CHECKS\|SOME CHECK\|restart exit\|escapees alive\|decoy' $R/lettera-$v.txt | cut -c1-200 | head -12 | sed 's/^/   /'
done
