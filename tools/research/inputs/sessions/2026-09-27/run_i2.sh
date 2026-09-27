#!/usr/bin/env bash
# I2: route-witness rewrite + load-time re-verification (after the example is built).
set -eu
cd /common/dev/rustred/TMP/w0/inputs/i2
WT=/common/dev/rustred/.claude/worktrees/fable51-inputs
IN=/common/dev/rustred/TMP/retired-campaigns-20260925.UtI4ay/five-loop-saved/inputs
BIN=$WT/target/release/examples/route_witness_rewrite
cp $BIN ./route_witness_rewrite && sha256sum ./route_witness_rewrite > tool.sha256
source /common/dev/rustred/TMP/w0/inputs/env1.sh
/run/current-system/sw/bin/time -v nice -n 5 taskset -c 110 ./route_witness_rewrite $IN/selection.json \
  $WT/examples/input/tide_five_loop_manifest.json /common/dev/rustred/TMP/w0/inputs/route-traffic-v2g7.tsv \
  selection-i2.json report.tsv summary.json > rewrite.stdout 2> rewrite.stderr
sha256sum selection-i2.json > selection-i2.sha256
# load-time verification: native map verification + transport compile of every route
/run/current-system/sw/bin/time -v nice -n 5 taskset -c 111 /common/dev/rustred/TMP/fable51-controls/bin/rustred-4a17f9c7 \
  owner-domain-scan --manifest selection-i2.json --owner-base $IN --max-numerator-rank 0 \
  --output load-scan.json --events load-scan.events.jsonl > load-scan.stdout 2> load-scan.stderr || echo "load scan exit $?"
