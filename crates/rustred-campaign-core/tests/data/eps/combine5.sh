#!/usr/bin/env bash
# merged view of the two shards of run5 (for summarize.py)
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/epsframe
C=$S/campaign/run5_boxB_v3_addendum_all; mkdir -p $C
cat $S/campaign/run5_boxB_v3_addendum_s0/nodes.jsonl $S/campaign/run5_boxB_v3_addendum_s1/nodes.jsonl > $C/nodes.jsonl
cat $S/campaign/run5_boxB_v3_addendum_s0/results.jsonl $S/campaign/run5_boxB_v3_addendum_s1/results.jsonl > $C/results.jsonl 2>/dev/null
cat $S/campaign/run5_boxB_v3_addendum_s0/final.jsonl $S/campaign/run5_boxB_v3_addendum_s1/final.jsonl 2>/dev/null | grep '^{' > $C/final.jsonl
cat $S/campaign/run5_boxB_v3_addendum_s*/proof_retries.jsonl > $C/proof_retries.jsonl 2>/dev/null
python3 -I $S/py/summarize.py $C > $C/SUMMARY.txt
