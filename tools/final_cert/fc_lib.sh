#!/usr/bin/env bash
# fc_lib.sh -- generic engine of the SINGLE FINAL CERTIFICATION of one node table (lane parametric, 2026-11-17).
#
# Sourced by a node launcher (final_cert_<NODE>.sh) that sets the cfg_* configuration and calls `fc_main "$@"`; executed
# directly (`bash fc_lib.sh --orchestrate RUN` / `--step RUN STEP`) by the units, which read the run's frozen run.env.
# No node knowledge here (L8): nodes, cones, terminals, keys, evidence columns are configuration.
#
# Every step runs as a DETACHED systemd user SERVICE unit `parametric-final-<node>-<step>` (never a scope); its payload runs
# inside a private user+pid namespace (`unshare -Urpf --mount-proc`), sources WS/ops/env.sh FIRST and then exports a private
# TMPDIR of the run (L22), and is pinned with taskset to the allowed CPU list (reserved sets and FOREIGN_5L_LIFT.held removed).
# The orchestrator unit `parametric-final-<node>-orch` runs the steps in WAVES (the steps of a wave in parallel, each with
# `systemd-run --wait`), gates every wave on the PAUSE flag of this account, DISK_LOW and host memory (75 GB must stay
# available after the wave's expected peak), appends one CPU_LEDGER line per unit, skips steps that have a .done marker
# (RESUME = run the launcher again with the same arguments), and STOPS the node at the first failed step: no second
# certificate, no patch of the table.
set -u -o pipefail
FC_WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
FC_KIT=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
FC_BASE_CPUS=0-167,176-191,216-257,261,263-270,272-287,320-334,336-383
FC_FLOOR_GB=75
FC_PAUSE=$FC_WS/plan/auto-campaign/implementer_ops/PAUSE
FC_ACCOUNT=${CLAUDE_CONFIG_DIR:-/home/vhirschi/.claude_account_9}
FC_STEPS="prep cert term b1 exact imp_plain imp_shared live fm one deliver"
FC_WAVES="prep|cert term b1 exact imp_plain|imp_shared live fm|one|deliver"

fc_log() { echo "$(date -u +%FT%TZ) $*"; }
fc_die() { echo "FINAL-CERT REFUSED: $*" >&2; exit 2; }

# ------------------------------------------------------------------------------------------------- helpers (launch side)
fc_sha() { sha256sum "$1" | cut -d' ' -f1; }
fc_allowed_cpus() {   # base list minus the CPUs held for the foreign 5L lift (read at launch)
  python3 -I - "$FC_BASE_CPUS" "$FC_WS/ops/FOREIGN_5L_LIFT.held" <<'EOF'
import json, sys
def expand(s):
    out = set()
    for part in s.split(','):
        a, _, b = part.partition('-'); out.update(range(int(a), int(b or a) + 1))
    return out
cpus = expand(sys.argv[1])
try:
    held = json.load(open(sys.argv[2])).get('held', [])
except (OSError, ValueError):
    held = []
for h in held:
    cpus -= expand(str(h)) if isinstance(h, str) else {int(h)}
s = sorted(cpus); runs = []; i = 0
while i < len(s):
    j = i
    while j + 1 < len(s) and s[j + 1] == s[j] + 1: j += 1
    runs.append(str(s[i]) if i == j else '%d-%d' % (s[i], s[j])); i = j + 1
print(','.join(runs))
EOF
}
fc_avail_gb() { awk '/MemAvailable/{print int($2/1048576)}' /proc/meminfo; }
fc_pause_mine() {   # only a flag that names THIS account stops the run (implementer, 2026-11-17)
  [ -e "$FC_PAUSE" ] && grep -q "$FC_ACCOUNT" "$FC_PAUSE"
}

# step resources: "cap_gb peak_gb cpus writes_gb expected_wall" from cfg_res_<step> (configuration), with defaults
fc_res() { local v="cfg_res_$1"; echo "${!v:-8 4 1 0 10m}"; }

# ------------------------------------------------------------------------------------------------------------ the plan
fc_step_doc() {
  case $1 in
    prep) echo "sha256 of every candidate file and input; domain copies (rhs -> []); structural scan (rule indices 0..n-1, residuals = terminal set, end records, origin.dispatch_position); sources lists (domain / full) with the bounded recipes placed before the first rule of dispatch position >= ${cfg_recipe_before_dp:-n/a}; per-line offset index";;
    cert) echo "region certificate b2_face_by_face (harness, engine model $cfg_engine_model, G5 reduced, strict loci of the records), cones $cfg_cones; PASS iff on every cone of [$cfg_exact_cones]: certificate_status complete, uncovered elements = terminals exactly, 0 undecided, checker ok, no flagged piece${cfg_orthant_undecided:+; orthant: uncovered = terminals and <= $cfg_orthant_undecided undecided (decided by argument)}";;
    term) echo "terminal rule on the FULL sources (no record contains a terminal, 0 coverage stubs; sealed recipes listed apart)";;
    b1) echo "strict descent b1_descent_on_tables on the FULL table file(s), zero proofs $cfg_zeros; PASS iff does_not_descend_listed = 0 and record_errors = 0";;
    exact) echo "exactness binding: target_rhs_sha256 of every record vs helper evidence (${cfg_exact_config##*/}) + side files ($cfg_side_dirs); lists changed records, per-key and NOT proven records, the watch list";;
    imp_plain) echo "strict native import --stream-payload (rustred ${cfg_rustred}) + --check, sets: $cfg_import_sets (plain)";;
    imp_shared) echo "strict native import --stream-payload --coef-form shared + --check, sets: $cfg_import_sets (shared)";;
    live) echo "per-record first-match liveness in $cfg_region of the changed and the not-whole-domain records (record_liveness, rule_subset = earlier records on the face, capped to the region; resumable shards, $cfg_live_shards in parallel)";;
    fm) echo "engine drill ONCE: rr_node first-match (engine binary) vs the core dispatcher on $cfg_fm_n arrival keys (deterministic sample) + terminals + acceptance keys; PASS iff 0 mismatches and open = terminals";;
    one) echo "acceptance keys: FIRST-MATCH + ONE-LEVEL reduces (key and level-1 children, rr_node reduce with masters = terminals + the rule's children) on the drill table (final + sealed recipe relations at the recipe position); no full reduction";;
    deliver) echo "delivery directory $cfg_deliver_dir: table links, recipes, certificate, all verdicts, summary, VERIFIER_SPEC.md, MANIFEST.json";;
  esac
}

fc_plan() {
  local run=$1 w s
  echo "PLAN for $cfg_node (label ${FC_LABEL:-FINAL}); run dir $run"
  echo "  candidate: $FC_CAND_FILES"
  echo "  sha256   : $FC_CAND_SHAS"
  local wi=0
  IFS='|' read -ra W <<< "$FC_WAVES"
  for w in "${W[@]}"; do
    wi=$((wi + 1)); echo "  wave $wi:"
    for s in $w; do
      fc_enabled "$s" || { echo "    - $s: SKIPPED (not configured for this node)"; continue; }
      read -r cap peak cpu wgb wall <<< "$(fc_res "$s")"
      echo "    - unit $FC_UNIT-$s  MemoryMax ${cap}G (expected peak ${peak} GB), CPUQuota $((cpu * 100))%, writes ~${wgb} GB, expected wall ${wall}"
      echo "      $(fc_step_doc "$s")"
    done
  done
}

fc_enabled() {
  case $1 in
    one) [ -n "${cfg_accept_keys:-}" ] ;;
    *) return 0 ;;
  esac
}

# -------------------------------------------------------------------------------------------------- input verification
fc_check_inputs() {   # every "path=sha" of cfg_inputs, every tool; prints a table; returns the number of problems
  local bad=0 e p want got
  for e in "${cfg_inputs[@]}"; do
    p=${e%%=*}; want=${e#*=}
    if [ ! -e "$p" ]; then echo "  MISSING  $p"; bad=$((bad + 1)); continue; fi
    got=$(fc_sha "$p")
    if [ "$want" = - ]; then echo "  sha256   $got  $p (recorded)";
    elif [ "$got" = "$want" ]; then echo "  ok       $got  $p";
    else echo "  MISMATCH $p: want $want got $got"; bad=$((bad + 1)); fi
  done
  for p in python3 systemd-run systemctl unshare taskset prlimit /run/current-system/sw/bin/time; do
    command -v "$p" >/dev/null 2>&1 || { echo "  MISSING tool $p"; bad=$((bad + 1)); }
  done
  return $bad
}

# --------------------------------------------------------------------------------------------------------- launcher
fc_main() {
  local dry=0 status=0 label="" run="" cands=() shas=() a
  while [ $# -gt 0 ]; do
    case $1 in
      --candidate) cands+=("$2"); shift 2 ;;
      --sha256) shas+=("$2"); shift 2 ;;
      --dry) dry=1; shift ;;
      --status) status=1; shift ;;
      --label) label=$2; shift 2 ;;
      --run-dir) run=$2; shift 2 ;;
      *) fc_die "unknown argument $1 (usage: --candidate PATH --sha256 HEX [--candidate PATH --sha256 HEX ...] [--dry] [--label L] [--run-dir D] [--status])" ;;
    esac
  done
  [ ${#cands[@]} -ge 1 ] || fc_die "no --candidate"
  [ ${#cands[@]} -eq ${#shas[@]} ] || fc_die "one --sha256 per --candidate"
  local i
  for i in "${!cands[@]}"; do
    [ -f "${cands[$i]}" ] || fc_die "candidate ${cands[$i]} missing"
    a=$(fc_sha "${cands[$i]}")
    [ "$a" = "${shas[$i]}" ] || fc_die "candidate ${cands[$i]}: sha256 $a != given ${shas[$i]}"
    echo "candidate ok: ${shas[$i]}  ${cands[$i]}"
  done
  FC_CAND_FILES=$(IFS=:; echo "${cands[*]}"); FC_CAND_SHAS=$(IFS=:; echo "${shas[*]}")
  FC_LABEL=${label:-FINAL}
  local unode=$cfg_node; [ "$FC_LABEL" != FINAL ] && unode="$cfg_node-$(echo "$FC_LABEL" | tr 'A-Z' 'a-z')"
  FC_UNIT=parametric-final-$unode
  run=${run:-$cfg_scratch/runs/$cfg_node-$FC_LABEL-${shas[0]:0:8}}
  FC_RUN=$run
  if [ $status = 1 ]; then fc_status "$run"; return 0; fi
  echo "inputs:"
  fc_check_inputs; local nbad=$?
  echo "host: available $(fc_avail_gb) GB (floor $FC_FLOOR_GB after the wave's peak); DISK_LOW $([ -e "$FC_WS/ops/DISK_LOW" ] && echo PRESENT || echo absent); PAUSE for this account: $(fc_pause_mine && echo PRESENT || echo absent)"
  echo "allowed CPUs: $(fc_allowed_cpus)"
  for s in $FC_STEPS; do systemctl --user is-active -q "$FC_UNIT-$s" && echo "  UNIT ACTIVE: $FC_UNIT-$s"; done
  systemctl --user is-active -q "$FC_UNIT-orch" && { echo "  orchestrator $FC_UNIT-orch is ACTIVE"; [ $dry = 1 ] || fc_die "already running"; }
  fc_plan "$run"
  [ $nbad -eq 0 ] || fc_die "$nbad input problem(s)"
  if [ $dry = 1 ]; then echo "DRY: nothing started."; return 0; fi
  fc_pause_mine && fc_die "PAUSE flag for this account"
  mkdir -p "$run"/{steps,log,tmp}
  if [ -f "$run/run.env" ]; then
    grep -q "FC_CAND_SHAS=$FC_CAND_SHAS\$\|FC_CAND_SHAS=\"$FC_CAND_SHAS\"\|FC_CAND_SHAS='$FC_CAND_SHAS'" "$run/run.env" || fc_die "$run/run.env is for another candidate"
    echo "RESUME in $run (done: $(ls "$run/steps" | grep -c '\.done$'))"
  else
    { declare -p $(compgen -v cfg_); declare -p FC_CAND_FILES FC_CAND_SHAS FC_LABEL FC_UNIT FC_RUN; echo "FC_KIT_FROZEN=$FC_KIT"; } > "$run/run.env"
    echo "$FC_LABEL" > "$run/LABEL"
  fi
  rm -f "$run/steps/"*.fail
  systemctl --user reset-failed "$FC_UNIT-orch" 2>/dev/null   # this run's own orchestrator unit, failed state of a stopped run
  systemd-run --user --unit="$FC_UNIT-orch" -p MemoryMax=1G -p MemorySwapMax=0 -p Nice=10 \
    --setenv=PATH="$PATH" --setenv=HOME="$HOME" --setenv=XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}" \
    --setenv=CLAUDE_CONFIG_DIR="$FC_ACCOUNT" \
    -p StandardOutput=append:"$run/log/orchestrator.log" -p StandardError=append:"$run/log/orchestrator.log" \
    -- bash "$FC_KIT/fc_lib.sh" --orchestrate "$run" || fc_die "could not start the orchestrator"
  echo "started $FC_UNIT-orch; log $run/log/orchestrator.log; wait: systemctl --user is-active $FC_UNIT-orch"
}

fc_status() {
  local run=$1 s
  for s in $FC_STEPS; do
    if [ -e "$run/steps/$s.done" ]; then echo "  $s: done"; elif [ -e "$run/steps/$s.fail" ]; then echo "  $s: FAILED";
    elif systemctl --user is-active -q "$FC_UNIT-$s"; then echo "  $s: running"; else echo "  $s: -"; fi
  done
  [ -f "$run/summary.json" ] && echo "summary: $run/summary.json"
}

# ------------------------------------------------------------------------------------------------------ orchestrator
fc_orchestrate() {
  local run=$1 w s wi=0
  # shellcheck disable=SC1090
  source "$run/run.env"
  fc_log "orchestrator start $FC_UNIT ($FC_LABEL) pid $$"
  IFS='|' read -ra W <<< "$FC_WAVES"
  for w in "${W[@]}"; do
    wi=$((wi + 1))
    local todo=() peak=0 wgb=0
    for s in $w; do
      [ -e "$run/steps/$s.done" ] && continue
      fc_enabled "$s" || { echo '{"skipped": "not configured for this node"}' > "$run/steps/$s.verdict.json"; touch "$run/steps/$s.done"; continue; }
      todo+=("$s"); read -r _c p _u g _w <<< "$(fc_res "$s")"; peak=$((peak + p)); wgb=$((wgb + g))
    done
    [ ${#todo[@]} -eq 0 ] && continue
    # gates: PAUSE of this account, DISK_LOW for writes above 1 GB, memory floor after the wave's expected peak
    if fc_pause_mine; then fc_log "STOP before wave $wi: PAUSE flag names this account; resume = run the launcher again"; exit 75; fi
    if [ "$wgb" -ge 1 ] && [ -e "$FC_WS/ops/DISK_LOW" ]; then fc_log "STOP before wave $wi: DISK_LOW and the wave writes ~$wgb GB"; exit 75; fi
    local waited=0
    while [ $(( $(fc_avail_gb) - peak )) -lt $FC_FLOOR_GB ]; do
      [ $waited -ge 7200 ] && { fc_log "STOP before wave $wi: memory floor not met for 2 h (available $(fc_avail_gb) GB, wave peak $peak GB)"; exit 75; }
      [ $((waited % 600)) -eq 0 ] && fc_log "wave $wi waits: available $(fc_avail_gb) GB, expected peak $peak GB, floor $FC_FLOOR_GB"
      sleep 60; waited=$((waited + 60))
    done
    fc_log "wave $wi: ${todo[*]} (expected peak $peak GB, available $(fc_avail_gb) GB)"
    local pids=() cpus; cpus=$(fc_allowed_cpus)
    for s in "${todo[@]}"; do fc_unit "$run" "$s" "$cpus" & pids+=($!); done
    for p in "${pids[@]}"; do wait "$p"; done
    for s in "${todo[@]}"; do
      if [ -e "$run/steps/$s.done" ]; then fc_log "step $s: PASS"; else fc_log "step $s: FAIL (see $run/steps/$s.verdict.json, $run/log/$s.err)"; touch "$run/steps/$s.fail"; fi
    done
    for s in "${todo[@]}"; do
      [ -e "$run/steps/$s.fail" ] && { fc_log "NODE STOPPED at wave $wi (failed: $(cd "$run/steps" && ls *.fail | tr '\n' ' ')): no second certificate, no patch"; fc_final_summary "$run" STOPPED; exit 1; }
    done
  done
  fc_final_summary "$run" COMPLETE
  fc_log "orchestrator end: all steps PASS"
}

fc_unit() {   # one step as its own service unit, waited for; the payload writes steps/<s>.rc and the verdict
  local run=$1 s=$2 cpus=$3 cap peak cpu wgb wall u
  read -r cap peak cpu wgb wall <<< "$(fc_res "$s")"
  u=$FC_UNIT-$s
  if systemctl --user is-active -q "$u"; then   # resumed orchestrator: the step unit survived; wait for it
    fc_log "step $s: unit $u still active, waiting"
    while systemctl --user is-active -q "$u"; do sleep 30; done
  else
    rm -f "$run/steps/$s.rc"
    printf '%s (implementer:parametric): unit %s unpinned (taskset allowed list), CPUQuota %d %%, nice 10, MemoryMax %sG (final certification %s)\n' \
      "$(date -u +%FT%TZ)" "$u" $((cpu * 100)) "$cap" "$FC_LABEL" >> "$FC_WS/ops/CPU_LEDGER.md"
    systemd-run --user --wait --collect --quiet --unit="$u" -p MemoryMax="${cap}G" -p MemorySwapMax=0 -p CPUQuota=$((cpu * 100))% -p Nice=10 \
      --setenv=PATH="$PATH" --setenv=HOME="$HOME" --setenv=FC_CPUS="$cpus" \
      -p StandardOutput=append:"$run/log/$s.out" -p StandardError=append:"$run/log/$s.err" \
      -- unshare -Urpf --mount-proc -- bash "$FC_KIT_FROZEN/fc_lib.sh" --step "$run" "$s"
  fi
  [ "$(cat "$run/steps/$s.rc" 2>/dev/null)" = 0 ] && touch "$run/steps/$s.done"
  return 0
}

fc_final_summary() {
  local run=$1 state=$2
  python3 -I "$FC_KIT_FROZEN/fc_tool.py" summary --run "$run" --state "$state" >> "$run/log/orchestrator.log" 2>&1 || true
}

# ------------------------------------------------------------------------------------------------------ step payloads
# run INSIDE the unit, inside the private pid namespace; env.sh first, then the private TMPDIR (L22)
fc_step() {
  local run=$1 s=$2 rc=0
  # shellcheck disable=SC1090
  source "$run/run.env"
  # shellcheck disable=SC1091
  source "$FC_WS/ops/env.sh" >/dev/null 2>&1
  export TMPDIR=$run/tmp/$s; mkdir -p "$TMPDIR"
  local T="python3 -I $FC_KIT_FROZEN/fc_tool.py" TS="taskset -c ${FC_CPUS:-$FC_BASE_CPUS}" H=$cfg_harness N=$cfg_node
  local PR=$run/prep D
  echo "$(date -u +%FT%TZ) step $s start (pid namespace pid $$, TMPDIR $TMPDIR, uid $(id -u))"
  export PARAMETRIC_INDEX_DIR=$run/idx PARAMETRIC_FALLTHROUGH=reduced PARAMETRIC_ENGINE=$cfg_engine_model
  mkdir -p "$run/idx"
  case $s in
    prep)
      D=$PR; mkdir -p "$D"
      { local i=0 f; IFS=: read -ra F <<< "$FC_CAND_FILES"; IFS=: read -ra S <<< "$FC_CAND_SHAS"
        for f in "${F[@]}"; do [ "$(fc_sha "$f")" = "${S[$i]}" ] || { echo "candidate $f sha256 changed"; rc=3; }; i=$((i + 1)); done
        fc_check_inputs > "$D/inputs.check.txt" || rc=3; } || rc=3
      [ $rc = 0 ] && { $TS $T prep --node "$N" --label "$FC_LABEL" --tables "$FC_CAND_FILES" --terminals "$cfg_terminals" \
          --recipes "${cfg_recipes:-}" --recipe-before-dp "${cfg_recipe_before_dp:--1}" --out "$D" > "$D/prep.json" || rc=$?; }
      [ $rc = 0 ] && [ "$(echo "$FC_CAND_FILES" | tr ':' '\n' | wc -l)" -gt 1 ] && { $TS $T concat --tables "$FC_CAND_FILES" --out "$D/$N.ship.concat.jsonl" || rc=$?; }
      ;;
    cert)
      D=$run/cert; mkdir -p "$D"
      PARAMETRIC_SPEC=$PR/$N.domain.sources.json PARAMETRIC_TERMINALS=$cfg_terminals PARAMETRIC_CONES="$cfg_cones" PARAMETRIC_MEMO=20000 \
      PARAMETRIC_OUT=$D/$N PARAMETRIC_PIECE_SLOTS= PARAMETRIC_PIECE_DEPTH=0 PARAMETRIC_PIECE_RSS_GB=$cfg_cert_rss_gb \
      PARAMETRIC_PIECE_UNITS=100000000000 PARAMETRIC_PIECE_WALL=14400 PARAMETRIC_RESIDUAL_MAX=20000000 PARAMETRIC_MAX_BLOCKED_CLASSES=5000000 \
      PARAMETRIC_ELEMENT_PROBE=1 PARAMETRIC_PROBE_CONTAIN=1 PARAMETRIC_PROBE_CONE=$cfg_region \
        $TS "$H" --ignored --exact b2_face_by_face --nocapture > "$D/harness.log" 2>&1 || rc=$?
      [ $rc = 0 ] && { $T certcheck --summary "$D/$N.summary.json" --terminals "$cfg_terminals" --cones "$cfg_cones" --exact-cones "$cfg_exact_cones" \
          ${cfg_orthant_undecided:+--orthant-undecided "$cfg_orthant_undecided"} --out "$D/cert.verdict.json" || rc=$?; }
      ;;
    term)
      D=$run/term; mkdir -p "$D"
      PARAMETRIC_SPEC=$PR/$N.full.sources.json PARAMETRIC_TERMINALS=$cfg_terminals PARAMETRIC_OUT=$D/$N.terminal_check.json \
        $TS "$H" --ignored --exact terminal_check --nocapture > "$D/harness.log" 2>&1 || rc=$?
      [ $rc = 0 ] && { $T termcheck --file "$D/$N.terminal_check.json" --out "$D/term.verdict.json" || rc=$?; }
      ;;
    b1)
      D=$run/b1; mkdir -p "$D"
      local zs="" z
      for z in ${cfg_zero_summaries//:/ }; do zs="$zs${zs:+;}$z@$(fc_sha "$z")"; done
      PARAMETRIC_TABLES=$FC_CAND_FILES PARAMETRIC_ZEROS=$cfg_zeros PARAMETRIC_ZERO_SUMMARIES="$zs" PARAMETRIC_THREADS=2 PARAMETRIC_OUT=$D/$N.b1.jsonl \
        $TS "$H" --ignored --exact b1_descent_on_tables --nocapture > "$D/harness.log" 2>&1 || rc=$?
      [ $rc = 0 ] && { $T b1check --summary "$D/$N.b1.jsonl.summary.json" --out "$D/b1.verdict.json" || rc=$?; }
      ;;
    exact)
      D=$run/exact; mkdir -p "$D"
      $TS $T exact --tables "$FC_CAND_FILES" --config "$cfg_exact_config" --positions "$PR/$N.positions.jsonl" --side-dirs "${cfg_side_dirs:-}" \
        --cone-bound "${cfg_region#simplex:}" ${cfg_reference_index:+--reference-index "$cfg_reference_index"} --out "$D/$N.exact" > "$D/exact.log" 2>&1 || rc=$?
      ;;
    imp_plain|imp_shared)
      local form=${s#imp_} set file sd args
      for set in $cfg_import_sets; do
        [ "${set#*:}" = "$form" ] || continue
        file=${set%%:*}
        if [ "$file" = concat ]; then file=$PR/$N.ship.concat.jsonl; else file=$(echo "$FC_CAND_FILES" | cut -d: -f$((file + 1))); fi
        sd=$run/imp/${set%%:*}-$form; mkdir -p "$sd/out"
        args=(p-import-table --table "$file" --graph "$cfg_graph" --ext "$cfg_ext" --label "$N" --lp "$cfg_lp" --out "$sd/out"
              --census-family "$cfg_family" --no-verify --force --stream-payload)
        [ "$form" = shared ] && args+=(--coef-form shared)
        [ -n "${cfg_recipes:-}" ] && [ "${set%%:*}" = 0 ] && args+=(--recipes "$PR/$N.recipes.jsonl")
        ( cd "$sd" && $TS /run/current-system/sw/bin/time -v "$cfg_rustred" "${args[@]}" > import.stdout 2> import.stderr; echo $? > import.rc )
        if [ "$(cat "$sd/import.rc")" = 0 ]; then
          ( cd "$sd" && RUSTRED_P_CHECK_THREADS=4 $TS /run/current-system/sw/bin/time -v "$cfg_rustred" p-import-table --check "$sd/out/$N.json" > check.stdout 2> check.stderr; echo $? > check.rc )
        fi
        $T impcheck --dir "$sd" --terminals "$cfg_terminals" > "$sd/impcheck.log" 2>&1 || rc=3
      done
      ;;
    live)
      D=$run/live; mkdir -p "$D"
      local sel=$run/exact/$N.exact.live_positions.json k
      python3 -I -c 'import json,sys; p=json.load(open(sys.argv[1])); m=int(sys.argv[4]); p=p[:m] if m>0 else p; json.dump(p, open(sys.argv[3]+"/selected.json","w")); k=int(sys.argv[2]); [json.dump(p[i::k], open("%s/shard.%d.json"%(sys.argv[3],i),"w")) for i in range(k)]' \
        "$sel" "$cfg_live_shards" "$D" "${cfg_live_max:-0}"; sel=$D/selected.json
      local lp=()
      for ((k = 0; k < cfg_live_shards; k++)); do
        PARAMETRIC_SPEC=$PR/$N.domain.sources.json PARAMETRIC_LIVENESS_CONE=$cfg_region PARAMETRIC_LIVENESS_POSITIONS=$D/shard.$k.json PARAMETRIC_LIVENESS_RESUME=1 \
        PARAMETRIC_LIVENESS_UNITS=$cfg_live_units PARAMETRIC_LIVENESS_WALL=$cfg_live_wall PARAMETRIC_LIVENESS_ENUM=20000 PARAMETRIC_OUT=$D/live.$k.jsonl \
          $TS "$H" --ignored --exact record_liveness --nocapture > "$D/shard.$k.log" 2>&1 & lp+=($!)
      done
      for k in "${lp[@]}"; do wait "$k" || rc=$?; done
      $T livecompile --positions "$PR/$N.positions.jsonl" --shards "$D/live.*.jsonl" --selected "$sel" --region "$cfg_region" --out "$D/$N.liveness" > "$D/compile.log" 2>&1 || rc=$?
      ;;
    fm)
      D=$run/fm; mkdir -p "$D"
      local tbl=$FC_CAND_FILES; [ -f "$PR/$N.ship.concat.jsonl" ] && tbl=$PR/$N.ship.concat.jsonl
      $T fmkeys --arrivals "${cfg_arrivals:-}" --n "$cfg_fm_n" --terminals "$cfg_terminals" --explicit "${cfg_accept_keys:-}" --out "$D/keys" > "$D/fmkeys.log" 2>&1 || rc=$?
      $TS /run/current-system/sw/bin/time -v "$cfg_rr_node" first-match --table "$tbl" --keys "$D/keys.plain.jsonl" > "$D/engine.jsonl" 2> "$D/engine.err" || rc=$?
      PARAMETRIC_SPEC=$PR/$N.domain.sources.json PARAMETRIC_KEYS=$D/keys.plain.jsonl PARAMETRIC_OUT=$D/core.jsonl \
        $TS "$H" --ignored --exact drill_core_first_match --nocapture > "$D/core.log" 2>&1 || rc=$?
      [ $rc = 0 ] && { $T fmcompare --keys "$D/keys.jsonl" --core "$D/core.jsonl" --engine "$D/engine.jsonl" --engine-err "$D/engine.err" \
          --positions "$PR/$N.positions.jsonl" --scan "$PR/$N.scan.json" --terminals "$cfg_terminals" --out "$D/fm.verdict.json" || rc=$?; }
      ;;
    one)
      D=$run/one; mkdir -p "$D"
      local ks=$D/accept.plain.jsonl
      python3 -I -c 'import json,sys; [print(json.dumps(json.loads(l)["key"])) for l in open(sys.argv[1]) if l.strip()]' "$cfg_accept_keys" > "$ks"
      PARAMETRIC_SPEC=$PR/$N.domain.sources.json PARAMETRIC_KEYS=$ks PARAMETRIC_OUT=$D/core0.jsonl $TS "$H" --ignored --exact drill_core_first_match --nocapture > "$D/core0.log" 2>&1 || rc=$?
      $T onejobs --table "${FC_CAND_FILES%%:*}" --index "$PR/$N.0.index.jsonl" --scan "$PR/$N.scan.json" --keys "$cfg_accept_keys" --core "$D/core0.jsonl" \
        --positions "$PR/$N.positions.jsonl" --terminals "$cfg_terminals" --cap1 "$cfg_one_cap1" --out "$D/jobs0.jsonl" > "$D/jobs0.log" 2>&1 || rc=$?
      PARAMETRIC_SPEC=$PR/$N.domain.sources.json PARAMETRIC_KEYS=$D/jobs0.jsonl.need.jsonl PARAMETRIC_OUT=$D/core1.jsonl $TS "$H" --ignored --exact drill_core_first_match --nocapture > "$D/core1.log" 2>&1 || rc=$?
      $T onejobs --table "${FC_CAND_FILES%%:*}" --index "$PR/$N.0.index.jsonl" --scan "$PR/$N.scan.json" --keys "$cfg_accept_keys" --core "$D/core0.jsonl:$D/core1.jsonl" \
        --positions "$PR/$N.positions.jsonl" --terminals "$cfg_terminals" --cap1 "$cfg_one_cap1" --out "$D/jobs.jsonl" > "$D/jobs.log" 2>&1 || rc=$?
      local dtab=${FC_CAND_FILES%%:*}; [ -f "$PR/$N.ship.concat.jsonl" ] && dtab=$PR/$N.ship.concat.jsonl
      if [ -n "${cfg_drill_points:-}" ]; then
        dtab=$D/$N.drill.table.jsonl
        $T drilltable --table "${FC_CAND_FILES%%:*}" --points "$cfg_drill_points" --position "$(python3 -I -c 'import json,sys; print(json.load(open(sys.argv[1]))["recipe_position"])' "$PR/$N.scan.json")" \
          --out "$dtab" > "$D/drilltable.log" 2>&1 || rc=$?
      fi
      [ $rc = 0 ] && { $TS $T onerun --jobs "$D/jobs.jsonl" --table "$dtab" --rr-node "$cfg_rr_node" --terminals "$cfg_terminals" --parallel "$cfg_one_parallel" \
          --wall 1800 --as-gb 40 --work "$D/work" --out "$D/onelevel.results.jsonl" > "$D/onerun.log" 2>&1 || rc=$?; }
      ;;
    deliver)
      $T deliver --run "$run" --out "$cfg_deliver_dir" > "$run/log/deliver.tool.log" 2>&1 || rc=$?
      ;;
    *) echo "unknown step $s"; rc=2 ;;
  esac
  echo "$(date -u +%FT%TZ) step $s end rc $rc"
  echo "$rc" > "$run/steps/$s.rc"
  return 0
}

fc_internal() {
  case ${1:-} in
    --orchestrate) fc_orchestrate "$2" ;;
    --step) fc_step "$2" "$3" ;;
    *) echo "fc_lib.sh: sourced by final_cert_<NODE>.sh; internal modes --orchestrate RUN | --step RUN STEP" >&2; exit 2 ;;
  esac
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then fc_internal "$@"; fi
