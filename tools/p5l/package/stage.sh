#!/usr/bin/env bash
# stage.sh NAME STAGES|tops:T1,T2,.. [THREADS] : resumable assembly of one staged 1-5 package (vk5l WP6).
#   1. imports of every node the tops need (imports.py, small first, 6 at a time, one flocked CPU each);
#   2. the assembly (p5l_library assemble, job6.py: THREADS flocked CPUs, systemd scope MemoryMax $WP6_ASM_MEM (72G),
#      guarded_run floor 50 GB) with the census CHECKPOINT + JOURNAL (RUSTRED_P_ASSEMBLE_CENSUS=<pkg>/census.ckpt.rrbin),
#      the census SEED from FULL2 (RUSTRED_P_ASSEMBLE_CENSUS_SEED, check mode `seeded`), gzip payloads, the content
#      revision vk5l-<NAME>-r1, the descent certificate ON; relaunched up to 8 times after a stop (journal resume);
#   3. lazy unverified load (nothing touched) of the written package: time + peak RSS;
#   4. the release layout (release.py manifest --link) and its totals.
# State: $PKG/STATE (one line per finished step), logs in $PKG/. Re-running the script resumes after the last step done.
# Honours ops/PAUSE between steps (writes a handoff line and exits 0).
set -uo pipefail
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
T=$(cd "$(dirname "$0")" && pwd)
D=/common/build-cache/vhirschi/p_integral_dev/vk5l-wp6
NAME=$1; SPEC=$2; THREADS=${3:-8}
PLAN=$D/plan
PKG=$D/pkg-1-5/$NAME
BIN=${WP6_BIN:-$WS/build/vk5l-wp6/release}
SEED=${WP6_SEED:-$WS/SCRATCHPAD/p5l-library-2/pkg/FULL2/lib}
MEM=${WP6_ASM_MEM:-72G}
mkdir -p "$PKG"
log() { echo "$(date -u +%FT%TZ) $*" | tee -a "$PKG/driver.log"; }
done_step() { grep -qx "$1" "$PKG/STATE" 2>/dev/null; }
mark() { echo "$1" >> "$PKG/STATE"; log "step $1 done"; }
paused() { if [ -e "$WS/ops/PAUSE" ]; then log "PAUSE present: stopping before $1 (resume: rerun $0 $NAME $SPEC $THREADS)"; exit 0; fi; }

# The stage spec: "1,2,3" (stages of plan/tops.tsv) or "tops:A,B,C" (a trial set).
if [[ "$SPEC" == tops:* ]]; then
  TOPS=${SPEC#tops:}
  python3 -I - "$PLAN" "$NAME" "$TOPS" <<'EOF'
import json, sys
plan, name, tops = sys.argv[1], sys.argv[2], sys.argv[3].split(',')
s = json.load(open(plan + '/selection.json'))
for tp in s['tops']:
    tp['stage'] = 77 if tp['top'] in tops else 99
json.dump(s, open(plan + f'/selection.{name}.json', 'w'))
EOF
  SELPLAN=$PLAN/$NAME; mkdir -p "$SELPLAN"; cp "$PLAN/selection.$NAME.json" "$SELPLAN/selection.json"; STAGES=77
else
  SELPLAN=$PLAN; STAGES=$SPEC
fi
TAG=s$(echo "$STAGES" | tr -d ,)

paused imports
if ! done_step imports; then
  log "imports for stages $STAGES"
  WP6_RUSTRED=$BIN/rustred python3 -I "$T/imports.py" "$SELPLAN" "$STAGES" --parallel 6 >> "$PKG/imports.log" 2>&1 \
    || { log "imports failed (see $PKG/imports.log)"; exit 1; }
  cp "$SELPLAN/imports.$TAG.tsv" "$PKG/imports.tsv"; cp "$SELPLAN/families.$TAG.json" "$PKG/families.json"
  mark imports
fi

paused assemble
if ! done_step assemble; then
  for try in 1 2 3 4 5 6 7 8; do
    paused assemble
    [ -e "$PKG/lib/library.json" ] && break
    rm -rf "$PKG/lib.partial" "$PKG/run-asm-$try"
    log "assembly try $try ($THREADS threads, cap $MEM)"
    # Thread re-pinning watcher (multi-NUMA CPU sets are not balanced by the kernel).
    (sleep 60; python3 -I "$T/repin.py" "$PKG/run-asm-$try" "$PKG/lib" 300) &
    python3 -I "$T/job6.py" --tag "asm-$NAME-$try" --ncpu "$THREADS" --mem "$MEM" --max-seconds 172800 \
      --out "$PKG/run-asm-$try" --wait 14400 -- bash -c "source $WS/ops/env.sh; \
      export RUSTRED_P_ASSEMBLE_CENSUS=$PKG/census.ckpt.rrbin RUSTRED_P_CENSUS_PROGRESS=1 \
             RUSTRED_P_ASSEMBLE_CENSUS_SEED=$SEED RUSTRED_P_ASSEMBLE_CENSUS_CHECK=seeded \
             RUSTRED_P_PAYLOAD_ENCODING=gzip RUSTRED_P_PACKAGE_REVISION=vk5l-$NAME-r1; \
      /run/current-system/sw/bin/time -v $BIN/examples/p5l_library assemble $PKG/families.json 5 \
             \$VAKINT_TEST_P_LIBRARY_1_4 $PKG/imports.tsv $PKG/lib $THREADS"
    rc=$(cat "$PKG/run-asm-$try/rc" 2>/dev/null || echo 99)
    log "assembly try $try rc $rc"
    if [ "$rc" = 0 ] && [ -e "$PKG/lib/library.json" ]; then break; fi
    grep -q "ASSEMBLY FAILED" "$PKG/run-asm-$try/guard/stderr.log" 2>/dev/null && { log "assembly FAILED (engine refusal): not retried"; exit 1; }
  done
  [ -e "$PKG/lib/library.json" ] || { log "assembly not done after retries"; exit 1; }
  touch "$PKG/lib/ASSEMBLED.ok"
  mark assemble
fi

paused load
if ! done_step load; then
  python3 -I "$T/job6.py" --tag "load-$NAME" --ncpu 1 --mem 16G --max-seconds 3600 --out "$PKG/run-load" -- \
    bash -c "source $WS/ops/env.sh; /run/current-system/sw/bin/time -v $BIN/examples/p5l_library load $PKG/lib 0"
  [ "$(cat "$PKG/run-load/rc")" = 0 ] && mark load
fi

# Optional acceptance cell (WP6_CELL_TOP=<5L top>): p5l_library check <pkg> <top> 0 0 0 0 0 = the d0_r0 cell of that top
# reduced through the package (A) and its top sector against a RustRed Laporta modulo subsectors at 3 d (B).
paused cell
if [ -n "${WP6_CELL_TOP:-}" ] && ! done_step cell; then
  python3 -I "$T/job6.py" --tag "cell-$NAME" --ncpu 1 --mem ${WP6_CELL_MEM:-32G} --max-seconds 7200 --out "$PKG/run-cell" -- \
    bash -c "source $WS/ops/env.sh; /run/current-system/sw/bin/time -v $BIN/examples/p5l_library check $PKG/lib $WP6_CELL_TOP 0 0 0 0 0"
  [ "$(cat "$PKG/run-cell/rc")" = 0 ] && mark cell
fi

paused release
if ! done_step release; then
  python3 -I "$T/release.py" manifest "$PKG/lib" "$PKG/release" --selection "$SELPLAN/selection.json" --link \
    > "$PKG/release.totals.json" 2>> "$PKG/driver.log" && mark release
fi
log "driver done: $(cat "$PKG/STATE" | tr '\n' ' ')"
