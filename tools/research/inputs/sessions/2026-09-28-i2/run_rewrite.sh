#!/usr/bin/env bash
# i2 lane: route-witness rewrite, one frame per owner (i2b), plus a reproduction of the W0.6 per-route build.
set -eu
cd /common/dev/rustred/TMP/w1/i2
WT=/common/dev/rustred/.claude/worktrees/fable51-inputs
IN=/common/dev/rustred/TMP/retired-campaigns-20260925.UtI4ay/five-loop-saved/inputs
TRAFFIC=/common/dev/rustred/TMP/w0/inputs/route-traffic-v2g7.tsv
cp $WT/target/release/examples/route_witness_rewrite ./route_witness_rewrite && sha256sum ./route_witness_rewrite > tool.sha256
sha256sum $IN/selection.json $WT/examples/input/tide_five_loop_manifest.json $TRAFFIC > inputs.sha256
TIME=/run/current-system/sw/bin/time
# (a) reproduction of the W0.6 build (--frame route): must equal selection-i2.json d9760837...
$TIME -v nice -n 5 taskset -c 124 ./route_witness_rewrite $IN/selection.json $WT/examples/input/tide_five_loop_manifest.json \
  $TRAFFIC repro-route/selection.json repro-route/report.tsv repro-route/summary.json --frame route > repro-route.stdout 2> repro-route.stderr
# (b) the rebuild: one frame per owner
$TIME -v nice -n 5 taskset -c 125 ./route_witness_rewrite $IN/selection.json $WT/examples/input/tide_five_loop_manifest.json \
  $TRAFFIC selection-i2b.json report-i2b.tsv summary-i2b.json --frame owner > rewrite-i2b.stdout 2> rewrite-i2b.stderr
sha256sum selection-i2b.json repro-route/selection.json /common/dev/rustred/TMP/w0/inputs/i2/selection-i2.json > selections.sha256
