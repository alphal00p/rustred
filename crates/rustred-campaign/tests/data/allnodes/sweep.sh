#!/usr/bin/env bash
# Lane allnodes: the RESUMABLE integer-index sweep: nodes in the order of <order file> (smallest table first, boxed nodes of
# other lanes last), at most $PAR at once, each its own unit + pid namespace + TMPDIR + index dir (launch_node.sh / node.sh).
# A finished node (nodes/<node>/final.json) is never recomputed; a failing node does not stop the batch.
# Usage: sweep.sh <order file: one node per line> <run dir W>   env: SPECS BIN PROOF COEF BUDGET MAXR THREADS KONLY PASS PAR
#        NODE_QUOTA NODE_MEM_GB
set -u
export SWEEP_DIR=$(cd "$(dirname "$0")" && pwd)
export W=$2
mkdir -p "$W/nodes"
echo "$(date -u +%FT%TZ) sweep start pass=$PASS budget=$BUDGET par=$PAR quota/node=$NODE_QUOTA mem/node=${NODE_MEM_GB}G" >> "$W/sweep.log"
xargs -a "$1" -P "$PAR" -I{} bash "$SWEEP_DIR/launch_node.sh" {}
echo "$(date -u +%FT%TZ) SWEEP_DONE" >> "$W/sweep.log"
