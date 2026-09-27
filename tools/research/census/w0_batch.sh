#!/usr/bin/env bash
# W0.7 census batch (receipt run). Read-only on every input; outputs go to $OUT.
# usage: w0_batch.sh CENSUS_BIN OUT
# Inputs (block clones, never written): gen3/gen6/gen7 clones of the v2
# checkpoint under the lane worktree TMP, the four-loop controls of
# TMP/fable51-controls/census-4l-4a17f9c7, the C-5F control
# TMP/fable51-controls/wave2-profile/w50-new-ready and the drained hot-owner
# pilot result.json.
set -euo pipefail
bin=$1; out=$2
wt=/common/dev/rustred/.claude/worktrees/fable51-compact
ctl=/common/dev/rustred/TMP/fable51-controls
pilot=/common/dev/rustred/TMP/qcd-feynman-d9d10-pilot-hot-owner/matrix-32fdec/hot-owner-physics-ordered/run/result.json
v2run=/common/dev/rustred/campaigns/five-loop-qcd-feynman-d9d10-v2/runs/20260926T151353.794886Z
hot=011101110111000
mkdir -p "$out"/{gen3,gen6,gen7,four-loop,c5f,pilot}
cd "$out"
series() { # heartbeat series "elapsed committed_records" of a run's events.jsonl
  grep '"event":"heartbeat"' "$1" | nix run nixpkgs#jq -- -r \
    'select(.progress.committed_domains != null) | "\(.elapsed_seconds) \(.progress.committed_domains)"'
}
[[ -s v2-series.txt ]] || series "$v2run/events.jsonl" > v2-series.txt
sha256sum "$bin" > census-bin.sha256
run() { local o=$1; shift; "$bin" "$@" > "$o.json" 2> "$o.err"; echo "$o: $(tail -1 "$o.err")"; }
# Five-loop v2 checkpoints.
for g in 3 6 7; do
  ck=$wt/TMP/gen$g
  run gen$g/stats stats "$ck"
  run gen$g/cost cost "$ck"
  run gen$g/compose compose "$ck"
  run gen$g/potential potential "$ck"
  run gen$g/cover-w30 cover "$ck" --k 6000 --series v2-series.txt --wait 30 --hot $hot --rows gen$g/cover-w30-rows.jsonl
done
run gen7/cover-w300 cover "$wt/TMP/gen7" --k 6000 --series v2-series.txt --wait 300 --hot $hot --rows gen7/cover-w300-rows.jsonl
for g in 3 6 7; do run gen$g/saturation saturation "$wt/TMP/gen$g" --draws 2000; done
# Drained controls: four-loop FG/BMW/H/X (4a17f9c7) and C-5F (53e672fc).
for f in fg bmw h x; do
  [[ -s four-loop/$f-series.txt ]] || series "$ctl/census-4l-4a17f9c7/$f/events.jsonl" > four-loop/$f-series.txt
  ck=$ctl/census-4l-4a17f9c7/$f/checkpoint
  run four-loop/$f-stats stats "$ck"
  run four-loop/$f-cost cost "$ck"
  run four-loop/$f-potential potential "$ck"
  run four-loop/$f-cover cover "$ck" --k 4000 --series four-loop/$f-series.txt --wait 1 --hot 0000000000 --rows four-loop/$f-cover-rows.jsonl
  run four-loop/$f-saturation saturation "$ck" --draws 1000
done
c5=$ctl/wave2-profile/w50-new-ready/five-finite
[[ -s c5f/series.txt ]] || series "$c5/events.jsonl" > c5f/series.txt
run c5f/stats stats "$c5/checkpoint"
run c5f/cost cost "$c5/checkpoint"
run c5f/potential potential "$c5/checkpoint"
run c5f/cover cover "$c5/checkpoint" --k 4000 --series c5f/series.txt --wait 1 --hot $hot --rows c5f/cover-rows.jsonl
# Drained hot-owner pilot (Ordered, legacy CP4: result.json) and its overlap with gen 7.
[[ -s pilot/series.txt ]] || series "$(dirname "$pilot")/events.jsonl" > pilot/series.txt
run pilot/pilot-cover cover "$pilot" --k 4000 --series pilot/series.txt --wait 30 --hot $hot --rows pilot/pilot-cover-rows.jsonl
run pilot/v2g7-vs-pilot pilot "$wt/TMP/gen7" --pilot "$pilot" --k 4000 --hot $hot
