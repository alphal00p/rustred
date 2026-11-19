#!/usr/bin/env bash
# batchA: copied-directory, m3, m4 experiments, each in its own private namespace (inns.sh)
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6
cd $S
for a in "cp_inner.sh shared1 shared 5" "cp_inner.sh internal1 internal 5" "m4_inner.sh exit153 exit153" "m4_inner.sh segv segv" "m4_inner.sh selfxfsz selfxfsz" "m3_inner.sh prealloc1 prealloc" "m3_inner.sh noprealloc1 noprealloc" "crashsig_inner.sh i1 signal_after_intent 1" "crashsig_inner.sh i3 signal_after_intent 3" "crashsig_inner.sh s2 signal_after_send 2" "crashsig_inner.sh s5 signal_after_send 5"; do
  echo "=== $a ($(date -u +%FT%TZ))"
  timeout -s KILL 1800 scripts/inns.sh scripts/$a
done
echo "=== batchA done $(date -u +%FT%TZ)"
