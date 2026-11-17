#!/usr/bin/env bash
# lane arrivals: T162 END GAME v2: (1) T165 done once its top-up parts A and B (3 shards) are done; (2) once T163, T165, T167 are done:
# per-parent key files (arrivals-bin --fast), the full T162 ARRIVALS list (merge_bins: keys union, depth min, parents counted once),
# fixpoint check, manifest; (3) T162's own EXACT CLOSURE from the delivered key file under its shipped service: closure.keys.bin
# (covered / terminal / UNCOVERED, depth) + uncovered list + closure paths of 12 sample uncovered keys. Units for heavy steps.
set -u
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals; P=$S/physchain; R=$P/run2; F=$P/t162final
mkdir -p $F
log() { echo "$(date -u +%FT%TZ) $*" >> $R/chain.log; }
cpus() { python3 -I -c '
import json,sys
held=set(json.load(open(sys.argv[1])).get("held",[]))
allowed=[]
for part in "0-167,176-191,216-257,261,263-270,272-287,320-334,336-383".split(","):
    a,_,b=part.partition("-"); allowed+=range(int(a),int(b or a)+1)
print(",".join(str(c) for c in allowed if c not in held))' "$WS/ops/FOREIGN_5L_LIFT.held"; }
unit() { local n=$1 th=$2 mem=$3 need=$4 what=$5; shift 6
  local gate=${GATE_OVERRIDE:-75}
  while :; do av=$(awk '/^MemAvailable:/{print int($2/1048576)}' /proc/meminfo); [ $((av - need)) -ge $gate ] && break; sleep 60; done
  printf '%s (implementer:arrivals): unpinned, CPUQuota %s00 %%, nice 10, MemoryMax %sG (unit %s: %s)\n' "$(date -u +%FT%TZ)" "$th" "$mem" "$n" "$what" >> $WS/ops/CPU_LEDGER.md
  systemd-run --user --wait --collect --unit=$n -p CPUQuota=${th}00% -p AllowedCPUs="$(cpus)" -p MemoryMax=${mem}G -p MemorySwapMax=0 -p Nice=10 -- "$@" > $F/$n.unit.txt 2>&1; }
until [ -e $R/T165.A/done ] && [ -e $R/T165.B/done ]; do sleep 60; done
if [ ! -e $R/T165/done ]; then
  mkdir -p $R/T165
  echo '{"parts": ["A", "B"], "B_shards": ["B0", "B1", "B2"], "merge": "into the T162 delivery: keys union, depth min (merge_bins); proof of top-up = from scratch: physchain/topup_test/T203,T208; shards: toy test shards_merged_equal_the_unsharded_walk"}' > $R/T165/topup.json
  touch $R/T165/done; log "T165 done (top-up parts A + B (3 shards))"
  cp $R/plan.jsonl.with-T165 $R/plan.jsonl
fi
until [ -e $R/T163/done ] && [ -e $R/T165/done ] && [ -e $R/T167/done ]; do sleep 60; done
log "T162: every parent done"
unit impl-arrivals-deliver-T162p5 4 16 6 "per-parent T162 key files of the last parents (arrivals-bin --fast)" -- python3 -I $P/deliver_partial_fast.py $R T162 T162i || { log "T162 partial FAILED"; exit 2; }
unit impl-arrivals-deliver-T162 1 8 6 "T162 ARRIVALS fixpoint list (merge of the per-parent key files)" -- python3 -I $P/merge_keys_wrap.py $S/deliver/T162.partial T162 $S/physcone/cones.json $S/deliver || { log "T162 delivery FAILED"; exit 2; }
python3 -I $P/fixpoint_check.py $R T162 T162i > $F/fixpoint.out
python3 -I $P/manifest.py > $F/manifest.out
log "T162 ARRIVALS delivered (S/deliver/T162.keys.bin; fixpoint check $(cat $F/fixpoint.out))"
touch $F/arrivals.done
TK=$(python3 -I -c 'import json,sys; print(" ".join("--trace-key \x27" + json.dumps(k, separators=(",", ":")) + "\x27" for k in json.load(open(sys.argv[1]))))' $P/t162_sample_uncovered.json)
cat > $F/closure.sh <<EOS
#!/usr/bin/env bash
source $WS/ops/env.sh
export TMPDIR=$F/tmp; mkdir -p \$TMPDIR $F/index $F/out
A=$S/deliver/T162.keys.bin
[ -s $F/rows.done ] || { /run/current-system/sw/bin/time -v $S/bin/rr_arrivals.a36 closure-rows --spec $S/phys/specs_rel/T162/spec.json --arrivals \$A --out $F/rows.jsonl --summary $F/rows.summary.json --threads 32 --index-dir $F/index > $F/rows.out 2> $F/rows.err && date -u +%FT%TZ > $F/rows.done; } || exit 2
/run/current-system/sw/bin/time -v $S/bin/rr_arrivals.a38 compute --spec $S/phys/specs_rel/T162/spec.json --rows $F/rows.jsonl --out-dir $F/out --parent T162 --edge-producer "T162 closure list" \
  --exact none --depth --arrivals \$A --closure-only --closure-chunk 2000000 --closure-out $S/deliver/T162.closure.keys.bin $TK --threads 32 --cache-entries 4000000 --index-dir $F/index > $F/closure.json 2> $F/closure.err || exit 2
EOS
chmod +x $F/closure.sh
log "T162 closure list starts"
GATE_OVERRIDE=60 unit impl-arrivals-t162closure 32 40 20 "T162 exact closure list from the delivered arrivals (covered / UNCOVERED, depth, sample chains)" -- unshare -Urpf --mount-proc -- bash $F/closure.sh || { log "T162 closure list FAILED"; exit 2; }
log "T162 closure list written (S/deliver/T162.closure.keys.bin)"
touch $F/closure.done
