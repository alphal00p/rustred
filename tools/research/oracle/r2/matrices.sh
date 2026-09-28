#!/usr/bin/env bash
# Round-2 mutation matrices (exact classes, closure effects, gate on every row).
# usage: matrices.sh BIN [fg|c5f|all]
set -u
BIN=$1; WHICH=${2:-all}
W=/common/dev/rustred/.claude/worktrees/fable51-oracle
O=/common/dev/rustred/TMP/w0/oracle
R=$O/runs
M=$O/r2/mutations
mkdir -p $M
PY=/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
export RAYON_NUM_THREADS=1 OMP_NUM_THREADS=1 OMP_THREAD_LIMIT=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 BLIS_NUM_THREADS=1 SYMBOLICA_HIDE_BANNER=1
if [ "$WHICH" = fg ] || [ "$WHICH" = all ]; then
  # FG: both oracles (Python audit rows on result.json copies + Rust rows); frontier = unrestricted-helper FG fixture.
  start=$(date +%s)
  taskset -c 0-15,256-271 nice -n 5 $PY $W/examples/python/oracle_mutation_matrix.py --run $R/c4l-ordered/fg \
    --frontier-run $R/frontier-fixture/fg --frontier-expect-closed 60/124 --rustred $BIN --threads 8 --jobs 4 \
    --output $M/matrix-fg.json > $M/matrix-fg.log 2>&1
  echo "fg exit=$? wall=$(( $(date +%s) - start ))s"
fi
if [ "$WHICH" = c5f ] || [ "$WHICH" = all ]; then
  # C-5F: Rust rows only; its frontier rows run on the FG frontier fixture (C-5F has no frontiers).
  start=$(date +%s)
  taskset -c 0-15,256-271 nice -n 5 $PY $W/examples/python/oracle_mutation_matrix.py --run $R/c5f-ordered/five-finite \
    --frontier-run $R/frontier-fixture/fg --frontier-expect-closed 60/124 --rustred $BIN --threads 8 --jobs 4 \
    --skip-python --output $M/matrix-c5f.json > $M/matrix-c5f.log 2>&1
  echo "c5f exit=$? wall=$(( $(date +%s) - start ))s"
fi
