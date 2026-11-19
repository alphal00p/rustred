#!/usr/bin/env bash
# gate7: gate6's own reproductions (root-mapped private namespaces) re-run on procfix7 452d0207
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate7; cd $S
for a in "cp_inner.sh shared1 shared 5" "cp_inner.sh internal1 internal 5" "reg_e1_inner.sh" "reg_e1b_inner.sh" "sec_inner.sh" "menv_inner.sh" "reg_d_inner.sh" "hs_inner.sh" \
         "b_inner.sh" "m4_inner.sh exit153 exit153" "m4_inner.sh selfxfsz selfxfsz" "m4_inner.sh segv segv" "m3_inner.sh prealloc1 prealloc" "m3_inner.sh noprealloc1 noprealloc" \
         "crashsig_inner.sh i1 signal_after_intent 1" "crashsig_inner.sh s2 signal_after_send 2" "m1_inner.sh m1a unmarked" "reg_e7_inner.sh"; do
  echo "=== $a ($(date -u +%FT%TZ))"
  timeout -s KILL 2400 scripts/inns.sh scripts/$a
done
echo "=== batch_g6root done $(date -u +%FT%TZ)"
