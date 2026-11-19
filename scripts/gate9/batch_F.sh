#!/usr/bin/env bash
# gate8 (F): gate6 decoy families (reg_e1: foreign decoys, copied dir, two supervisors, pid reuse, read-only verbs;
# reg_e1b: cross-signals between two campaigns) and gate7 lineage decoys (live, resume, pidreuse, nested, otherns) on a19d5d0b
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9; cd $S
echo "--- reg_e1 (TAG g8a)"; TAG=g9a timeout -s KILL 1800 scripts/ns8r.sh scripts/g8F_reg_e1_inner.sh
echo "--- reg_e1b"; timeout -s KILL 1800 scripts/ns8r.sh scripts/g8F_reg_e1b_inner.sh
for c in live resume pidreuse nested; do echo "--- lindecoy $c"; timeout -s KILL 1800 scripts/ns8r.sh scripts/g8F_g7_lindecoy_inner.sh ld-$c $c; done; echo "--- lindecoy otherns (kill-child)"; timeout -s KILL 1800 scripts/ns8r.sh scripts/g8F_lindecoy2_inner.sh ld2-otherns otherns
echo "=== done $(date -u +%FT%TZ)"
