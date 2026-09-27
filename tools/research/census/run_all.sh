#!/usr/bin/env bash
# Run every census subcommand on one CP5 checkpoint directory.
# usage: run_all.sh CKPT_DIR OUT_PREFIX [extra cover options...]
# Env: CENSUS_BIN (default: census on PATH), SERIES (heartbeat series file
# "elapsed committed" for the dispatch threshold), HOT (hot owner mask).
set -euo pipefail
ckpt=$1; out=$2; shift 2
bin=${CENSUS_BIN:-census}
hot=${HOT:-011101110111000}
series=()
if [[ -n "${SERIES:-}" ]]; then series=(--series "$SERIES"); fi
run() { local name=$1; shift; "$bin" "$@" > "$out-$name.json" 2> "$out-$name.err"; tail -1 "$out-$name.err"; }
run stats stats "$ckpt"
run cost cost "$ckpt"
run compose compose "$ckpt"
run potential potential "$ckpt"
run cover cover "$ckpt" --hot "$hot" --rows "$out-cover-rows.jsonl" "${series[@]}" "$@"
run saturation saturation "$ckpt"
