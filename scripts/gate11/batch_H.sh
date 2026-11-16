#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate11; cd $S
echo "##### (H) refusal + letter_a self-test"; timeout -s KILL 3000 scripts/ns8r.sh scripts/g11_H_inner.sh h1 2>&1 | grep -v '^  log'
echo "##### (H) 1.7 dress rehearsal (1/10 scale, private locks + ledger)"; timeout -s KILL 3000 scripts/ns8r.sh scripts/g11_host17_inner.sh r1 2>&1
echo "##### (e) finish eB1 (uninterrupted)"; timeout -s KILL 3000 scripts/ns8r.sh scripts/g10_e_finish_inner.sh eB1 2>&1 | grep -E "finish session|FINISH"
echo "=== done $(date -u +%FT%TZ)"
