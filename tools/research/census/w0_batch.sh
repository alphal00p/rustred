#!/usr/bin/env bash
# W0.7 census batch (receipt run). Read-only on every input; outputs go to $OUT.
# usage: w0_batch.sh CENSUS_BIN OUT [RUSTRED_BIN]
#        W0_ONLY=q3 w0_batch.sh - OUT RUSTRED_BIN   (only the Q3 step, e.g. into an existing receipt)
# Inputs (block clones, never written): gen3/gen6/gen7 clones of the v2
# checkpoint under the lane worktree TMP, the four-loop controls of
# TMP/fable51-controls/census-4l-4a17f9c7, the C-5F control
# TMP/fable51-controls/wave2-profile/w50-new-ready and the drained hot-owner
# pilot result.json. With RUSTRED_BIN (a build of this branch), the Q3
# guard/coefficient factor census (owner-domain-scan --factor-census, plain
# and with numerators) runs on the v2 selection manifest (read-only).
# Run it pinned, e.g. nice -n 19 taskset -c 16-31,272-287 w0_batch.sh ...
# A BIN.build.txt next to a binary (build commit and tree state) is copied
# into the receipt.
set -euo pipefail
only=${W0_ONLY:-}
bin=$1; [[ $only == q3 ]] || bin=$(realpath "$1")
out=$2; rbin=${3:+$(realpath "$3")}
wt=/common/dev/rustred/.claude/worktrees/fable51-compact
ctl=/common/dev/rustred/TMP/fable51-controls
pilot=/common/dev/rustred/TMP/qcd-feynman-d9d10-pilot-hot-owner/matrix-32fdec/hot-owner-physics-ordered/run/result.json
v2=/common/dev/rustred/campaigns/five-loop-qcd-feynman-d9d10-v2
v2run=$v2/runs/20260926T151353.794886Z
py=/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
# v2's CPU-hot owner (64% of gen-7 Apply seconds) and the pilot's / C-5F's
# CPU-hot owner (38% / 32% of their PPS draws; the pilot was seeded at $hot).
hot=011101110111000
hot2=000011001001011
mkdir -p "$out"/{gen3,gen6,gen7,four-loop,c5f,pilot,q3}
cd "$out"
series() { # heartbeat series "elapsed committed_domains" of a run's events.jsonl
  grep '"event":"heartbeat"' "$1" | /run/current-system/sw/bin/jq -r \
    'select(.progress.committed_domains != null) | "\(.elapsed_seconds) \(.progress.committed_domains)"'
}
run() { local o=$1; shift; local t0=$SECONDS; "$bin" "$@" > "$o.json" 2> "$o.err"; echo "$o: $(tail -1 "$o.err") (wall $((SECONDS - t0)) s)"; }
census_steps() {
[[ -s v2-series.txt ]] || series "$v2run/events.jsonl" > v2-series.txt
sha256sum "$bin" > census-bin.sha256
[[ -f $bin.build.txt ]] && cp "$bin.build.txt" census-build.txt
{ echo "worktree HEAD $(git -C "$wt" rev-parse HEAD)"; git -C "$wt" status --porcelain --untracked-files=no; } > census-git.txt
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
# Sensitivity of 'fully covered' to sampling: exact up to 2e7 points, 2e5 samples above.
run gen7/cover-w30-cap2e7 cover "$wt/TMP/gen7" --k 6000 --series v2-series.txt --wait 30 --hot $hot --cap 2e7 --samples 200000 --rows gen7/cover-w30-cap2e7-rows.jsonl
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
run c5f/cover-hot2 cover "$c5/checkpoint" --k 4000 --series c5f/series.txt --wait 1 --hot $hot2 --rows c5f/cover-hot2-rows.jsonl
# Drained hot-owner pilot (Ordered, legacy CP4: result.json) and its overlap with gen 7.
[[ -s pilot/series.txt ]] || series "$(dirname "$pilot")/events.jsonl" > pilot/series.txt
run pilot/cost cost "$pilot"
run pilot/pilot-cover cover "$pilot" --k 4000 --series pilot/series.txt --wait 30 --hot $hot --rows pilot/pilot-cover-rows.jsonl
run pilot/pilot-cover-hot2 cover "$pilot" --k 4000 --series pilot/series.txt --wait 30 --hot $hot2 --rows pilot/pilot-cover-hot2-rows.jsonl
run pilot/v2g7-vs-pilot pilot "$wt/TMP/gen7" --pilot "$pilot" --k 4000 --hot $hot
run pilot/v2g7-vs-pilot-hot2 pilot "$wt/TMP/gen7" --pilot "$pilot" --k 4000 --hot $hot2
}
[[ $only == q3 ]] || census_steps
# Q3 guard/coefficient factor census on the v2 selection (read-only).
if [[ -n $rbin ]]; then
  { echo "worktree HEAD $(git -C "$wt" rev-parse HEAD)"; git -C "$wt" status --porcelain --untracked-files=no; } > q3/git.txt
  sha256sum "$rbin" > q3/rustred-bin.sha256
  [[ -f $rbin.build.txt ]] && cp "$rbin.build.txt" q3/rustred-build.txt
  for mode in plain numerators; do
    extra=(); [[ $mode == numerators ]] && extra=(--factor-census-numerators)
    t0=$SECONDS
    # rustred refuses to start unless every inner pool is 1 (cli/routed.rs preflight_inner_pools);
    # the census then factors serially.
    env RAYON_NUM_THREADS=1 OMP_NUM_THREADS=1 OMP_THREAD_LIMIT=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 \
      BLIS_NUM_THREADS=1 SYMBOLICA_HIDE_BANNER=1 /run/current-system/sw/bin/time -f '%e s %M KB' -o q3/$mode.time \
      "$rbin" owner-domain-scan --manifest "$v2/inputs/selection.json" --owner-base "$v2/inputs" \
      --unbounded-rank --output q3/v2-factor-census-$mode.json --events q3/v2-factor-census-$mode-events.jsonl \
      --factor-census "${extra[@]}" --no-progress > q3/$mode.stdout 2> q3/$mode.stderr
    echo "q3/$mode: $(tail -1 q3/$mode.time) (wall $((SECONDS - t0)) s)"
    "$py" "$wt/tools/research/census/q3_summary.py" q3/v2-factor-census-$mode.json gen7/cost.json > q3/summary-$mode.md
  done
fi
