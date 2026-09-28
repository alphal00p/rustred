#!/usr/bin/env bash
# Round-3 mutation matrices (exact classes, closure effects, gate and applicability on every row).
# The C-5F matrix is split in two invocations (each < 1 h): the round-2 rows, then the new rows.
# usage: matrices.sh BIN [fg|c5f-old|c5f-new|all]
set -u
BIN=$1; WHICH=${2:-all}
W=/common/dev/rustred/.claude/worktrees/fable51-oracle
O=/common/dev/rustred/TMP/w0/oracle
R=$O/runs
M=$O/r3/mutations
mkdir -p $M
PY=/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
export TMPDIR=/common/dev/rustred/TMP
export RAYON_NUM_THREADS=1 OMP_NUM_THREADS=1 OMP_THREAD_LIMIT=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 BLIS_NUM_THREADS=1 SYMBOLICA_HIDE_BANNER=1
NEW="self-anchored-partial partial-as-anchor partial-anchor-cycle non-initial-anchor shrunk-residual route-partial dropped-routed-edge routed-false-hit miscounted-route-events"
OLD="dropped-edge injected-false-hit retargeted-alias retargeted-anchor seal-with-error hidden-error miscounted-events miscounted-successors remapped-query foreign-request foreign-owners mismatched-result alias-chain-detour dropped-frontier-record hidden-frontier seal-with-frontier"
run() {  # label args...
  local label=$1; shift
  local start=$(date +%s)
  taskset -c 0-15,256-271 nice -n 5 $PY $W/examples/python/oracle_mutation_matrix.py "$@" --rustred $BIN \
    --threads 8 --jobs 4 --output $M/matrix-$label.json > $M/matrix-$label.log 2>&1
  echo "$label exit=$? wall=$(( $(date +%s) - start ))s"
}
case $WHICH in fg|all)
  # FG: both oracles, every row (FG has no Route natives: the routed rows are not applicable).
  run fg --run $R/c4l-ordered/fg --frontier-run $R/frontier-fixture/fg --frontier-expect-closed 60/124 ;;
esac
case $WHICH in c5f-old|all)
  run c5f-old --run $R/c5f-ordered/five-finite --frontier-run $R/frontier-fixture/fg --frontier-expect-closed 60/124 \
    --skip-python --only $OLD ;;
esac
case $WHICH in c5f-new|all)
  run c5f-new --run $R/c5f-ordered/five-finite --skip-python --skip-partial-rows --only $NEW ;;
esac
