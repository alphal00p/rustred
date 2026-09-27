#!/usr/bin/env bash
# Gate 0.4(b) projection grid [E] on the v4 real-stream replay (streams-g7-v4.jsonl).
set -u
cd /common/dev/rustred/TMP/w0/intel/replay
P=/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
S=/common/dev/rustred/.claude/worktrees/fable51-py/tools/research/idxreplay/project.py
IN=streams-g7-v4.jsonl
OUT=gate04b.jsonl
A=0.44,0.65,0.69,0.73,0.82
rm -f $OUT
{
echo "# $(date -u +%FT%TZ) project.py sha256 $(sha256sum $S | cut -c1-16); input $IN sha256 $(sha256sum $IN | cut -c1-16)"
# 1. thread factor 1 (single-thread costs), k=16, cheap 100 ns
$P $S $IN --alpha $A --tag t1 --json $OUT
# 2. per-set thread factors from the socket-1 sweep (48 and 90 threads vs 1 thread, same session)
$P $S $IN --alpha $A --thread-sweep static-g7-threads.jsonl --threads 48 --tag t48 --json $OUT
$P $S $IN --alpha $A --thread-sweep static-g7-threads.jsonl --threads 90 --tag t90 --json $OUT
# 3. scalar factors named in the handoff (SoA-pattern ~2.4, SoA-id ~2.6)
$P $S $IN --alpha $A --thread-factor 2.4 --tag tf2.4 --json $OUT
$P $S $IN --alpha $A --thread-factor 2.6 --tag tf2.6 --json $OUT
# 4. sensitivity: native CPU per native inflated by the gate-0.3 in-process factor at K=96 (3.75x), lookups at 90 threads
$P $S $IN --alpha $A --thread-sweep static-g7-threads.jsonl --threads 90 --native-scale 3.75 --tag t90-native3.75 --json $OUT
# 5. sensitivity: MRU k=64, cheap tier 300 ns
$P $S $IN --alpha $A --k 64 --tag t1-k64 --json $OUT
$P $S $IN --alpha $A --cheap-ns 300 --tag t1-cheap300 --json $OUT
$P $S $IN --alpha $A --thread-sweep static-g7-threads.jsonl --threads 90 --cheap-ns 300 --tag t90-cheap300 --json $OUT
} > gate04b.txt 2>&1
echo rc=$?
