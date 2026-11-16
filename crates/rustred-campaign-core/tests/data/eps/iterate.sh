#!/usr/bin/env bash
# one iteration: check (with the facts so far) -> new fact requests -> rr_proof per chart (byte offsets) -> append facts
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/epsframe
B=/shared/vhirschi/t3code/vakint_dev/p_integral_dev/bin/frozen/20261114T2332Z-engine-line7a
EPSFRAME_REQ_DIR=$S/req TMPDIR=$S/tmp systemd-run --user --scope -q -p MemoryMax=4G -p MemorySwapMax=0 -- \
  $S/eps_frame_bin2 --ignored --exact eps_check_write_fact_requests --nocapture --test-threads 1 2>&1 | grep requested
for t in T012_a0-td-e T041_min T047_min; do
  charts=$(python3 -I $S/py/split_req.py $S/req/$t.table.jsonl.fact_requests.jsonl $S/req/$t.rr)
  : > $S/req/$t.new_facts.jsonl
  for c in $charts; do
    ( ulimit -v 4000000; unshare -Urpf --mount-proc -- $B/rr_proof instantiate --requests $S/req/$t.rr.$c --out $S/req/$t.facts.$c ) > /dev/null
    cat $S/req/$t.facts.$c >> $S/req/$t.new_facts.jsonl
  done
  sort -u $S/req/$t.new_facts.jsonl $S/req/$t.table.jsonl.check_facts.jsonl > $S/req/$t.merged.jsonl 2>/dev/null || sort -u $S/req/$t.new_facts.jsonl > $S/req/$t.merged.jsonl
  mv $S/req/$t.merged.jsonl $S/req/$t.table.jsonl.check_facts.jsonl
  echo "$t facts $(wc -l < $S/req/$t.table.jsonl.check_facts.jsonl)"
done
