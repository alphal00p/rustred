#!/usr/bin/env bash
# fix-inputs 2026-09-28: replicate of the harness gen-7 inspected-sample re-inspection (K=8 on one CCX, 96-103,
# SMT siblings 352-359 not used by us) to check the per-class K=1 calibration used for the I1 sizing.
set -euo pipefail
export TMPDIR=/common/dev/rustred/TMP
H=/common/dev/rustred/TMP/w0/harness
T=/common/dev/rustred/.claude/worktrees/fable51-csr/tools/research/harness
I=/common/dev/rustred/.claude/worktrees/fable51-inputs/tools/research/inputs
O=/common/dev/rustred/TMP/w0/inputs/k1cal
PY=/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
echo "start $(date -u +%FT%TZ)"
env BIN=$H/bin/rustred_app-harness-B-fp FIXTURE=$H/crosscheck/gen7-inspected-10k.json OUT=$O/reinspect-k8-r2 \
  CPUS=96-103 THREADS=8 PIN=1 PERF=stat PERF_EVENTS=instructions:u,cycles:u $T/run_harness.sh
echo "end $(date -u +%FT%TZ)"
