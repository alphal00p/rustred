#!/usr/bin/env bash
# differential.sh <label> <harness-bin> [families...] : digest re-inspection of every natively inspected ID of the
# W0.3 control walks (fixtures + real-walk taps of TMP/w0/harness/diff-A, binary A = vendored Symbolica 953e26e2)
# with <harness-bin>; the receipt's differential section compares ordered/multiset/successor digests, counts, stats.
set -uo pipefail
label=$1; BIN=$2; shift 2
fams=${*:-fg bmw h x five-finite}
D=/common/dev/rustred/TMP/symbolica-main/symeval-dev
H=/common/dev/rustred/.claude/worktrees/fable51-symeval/tools/research/harness
A=/common/dev/rustred/TMP/w0/harness/diff-A
for fam in $fams; do
  tap=$A/$fam/walk/tap.jsonl
  out=$D/runs/diff-$label/$fam
  mkdir -p $(dirname $out)
  env BIN=$BIN FIXTURE=$A/$fam/fixture.json OUT=$out CPUS=40-55,296-311 THREADS=24 SINK=digest TAP=$tap PERF=none NICE=5 \
    $H/run_harness.sh > /dev/null 2>&1
  /nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python -c "
import json,sys
r=json.load(open('$out/receipt.json'));d=r.get('differential') or {}
print('$label $fam passed=%s identical=%s mismatched=%s not_in_tap=%s harness_natives=%s tap_completed=%s error=%s'%(r.get('passed'),d.get('identical'),d.get('mismatched'),d.get('not_in_tap'),d.get('harness_natives'),d.get('tap_completed_keys'),r.get('error')))
" || echo "$label $fam: no receipt (exit $(cat $out/exit_code 2>/dev/null))"
done
