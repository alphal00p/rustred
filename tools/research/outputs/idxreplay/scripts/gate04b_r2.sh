#!/usr/bin/env bash
# Gate 0.4(b) fix-round cells [E] (verifier round 1): per-class exponents and cost-sample noise.
# Original grid: gate04b.sh -> gate04b.{txt,jsonl} (unchanged).
set -u
cd /common/dev/rustred/TMP/w0/intel/replay
P=/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
S=/common/dev/rustred/.claude/worktrees/fable51-py/tools/research/idxreplay/project.py
IN=streams-g7-v4.jsonl
OUT=gate04b-r2.jsonl
# SoA-pattern thinning CPU exponents (t_thin.md): miss 0.46, reverse 0.50. The thinning hit proxies are not usable
# for a hit exponent (found fraction 0.31 / 0.57 / 1.00 at 25 / 50 / 100% live), so hits are bracketed:
# a= below is the HIT exponent only: 0 (no growth), 0.46 (as misses), 0.69 (C-5F dynamic hit exponent, today's layout).
OWN=miss_scans=0.46,reverse=0.50
AH=0.0,0.46,0.69
rm -f $OUT
{
echo "# $(date -u +%FT%TZ) project.py sha256 $(sha256sum $S | cut -c1-16); input $IN sha256 $(sha256sum $IN | cut -c1-16)"
echo "# own-exponent cells: miss/reverse at SoA-pattern thinning CPU exponents (0.46/0.50); a= is the hit exponent. Read only the SoA-pattern rows: the other layouts own thinning exponents differ (SoA-id miss 0.65, reverse 0.72; L0 0.55, 0.62)"
$P $S $IN --alpha $AH --class-alpha $OWN --tag t1-own --json $OUT
$P $S $IN --alpha $AH --class-alpha $OWN --thread-sweep static-g7-threads.jsonl --threads 48 --tag t48-own --json $OUT
$P $S $IN --alpha $AH --class-alpha $OWN --thread-sweep static-g7-threads.jsonl --threads 90 --tag t90-own --json $OUT
$P $S $IN --alpha $AH --class-alpha $OWN --thread-sweep static-g7-threads.jsonl --threads 90 --native-scale 3.75 --tag t90-native3.75-own --json $OUT
echo "# cost-sample noise: the v3 streams run (streams-g7.jsonl, same trace, different layer-cost sample), uniform alpha grid"
$P $S streams-g7.jsonl --alpha 0.44,0.65,0.69,0.73,0.82 --native-ms 5.239 --tag t1-v3cost-native5.239 --json $OUT
$P $S streams-g7.jsonl --alpha 0.44,0.65,0.69,0.73,0.82 --native-ms 5.28 --tag t1-v3cost-native5.28 --json $OUT
} > gate04b-r2.txt 2>&1
echo rc=$?
