#!/usr/bin/env bash
# THE FIRST HOST STEP (fake children), RUNBOOK 1.7 as ONE script (lane procfix7 rounds 7-9b; gate11 M11-2 and its command,
# WS/notes/implementer.gate11.md "(H)"). It writes RUNBOOK 1.7's site.json and spec.json AS SHIPPED, runs the campaign and
# checks the pass criteria P1-P8 of RUNBOOK 1.7 (printed PASS / FAIL / n.a. / INFO; exit 0 only when none FAILs).
#
# Usage (from any clean shell: no pane or agent session has to stay open; round 9b):
#   REL=<dir holding the RELEASE rr_campaign and rr_fake_child>  bash host_step_fake1.sh --dry-run
#   REL=...  bash host_step_fake1.sh --run           THE HOST STEP (outside any namespace; the operator / coordinator)
#   REL=...  bash host_step_fake1.sh --rehearse-ns   ONLY inside a private user+pid namespace (refused elsewhere)
#   REL=...  OUT=<the step's OUT>  bash host_step_fake1.sh --grade   follow a started step by its unit's exact name, grade it
# Any other argument list prints this text and exits 2 without starting anything.
#
# THE SUPERVISOR IS A DETACHED SYSTEMD USER SERVICE (round 9b; WS/ops/LANE_RULES.md rule 5): exactly
#   systemd-run --user --wait --collect --unit=hoststep-fake1-<HHMMSS> -p Type=exec -p KillMode=mixed
#     -p TimeoutStopSec=infinity -p MemoryMax=8G -p MemorySwapMax=0 -p "UnsetEnvironment=RR_CAMPAIGN_JOB RR_CAMPAIGN_FAILPOINT"
#     -p StandardError=append:OUT/run.log -p StandardOutput=append:OUT/supervisor.stdout --working-directory=OUT
#     --setenv=PATH=<this shell's PATH> --setenv=HOME=<HOME> [--setenv=TMPDIR=<TMPDIR>] -- $REL/rr_campaign run
#     --spec OUT/spec.json --plain
#   The unit's main process IS the supervisor (its own session leader); its jobs start their own sessions and scopes as
#   before. `systemd-run --wait` (a client in its own session, output OUT/launcher.log) only waits and reports systemd's
#   exit status of the unit: the service runs on if this script, its terminal or its session ends; `--grade` then follows
#   the same unit by name and grades (P1 then uses the client's report in launcher.log if the client lived on).
# STOP: `rr_campaign control OUT/c stop` (a request; the watcher's ABORT files exactly this), or
#   `systemctl --user stop hoststep-fake1-<HHMMSS>.service` (SIGTERM to the supervisor = the same graceful stop; systemd
#   never escalates against the supervisor: TimeoutStopSec=infinity). KillMode=mixed (round 10, gate13c m13c-mixed-doc):
#   at a stop systemd sends SIGTERM to the supervisor only; once the supervisor has EXITED (a stop or a normal end) it
#   sends SIGKILL to whatever is still in the SERVICE's own cgroup: only descendants of the supervisor that never moved
#   into a job scope (its own systemctl helpers, a job's systemd-run client before its scope move); the jobs' scopes are
#   separate units and are never touched; no foreign process can be in that cgroup.
# --run      OUT=/common/build-cache/vhirschi/p_integral_dev/host-steps/fake1-<UTC> (override: OUT=...).
# --rehearse-ns  the same files except: lock_dir / ledger_file are PRIVATE COPIES under OUT (an empty cpu.N for every
#            WS/ops/locks/cpu.N; an empty ledger), the spec carries "containment":"marker_only" (no systemd scope inside a
#            namespace), and systemd-run / systemctl are the REHEARSAL FAKES beside this script (rehearsal-fake-systemd/:
#            the same service form, emulated in the namespace; refused if the real ones would be used). ITEMS / CASE_MS
#            override the workload (default: the RUNBOOK's 240 items x 60-120 s, ~1 h).
# --dry-run  prints the files and the commands, checks the preconditions; starts, signals, stops and creates nothing.
# PRE-FLIGHT (--run only; round 9): `$REL/rr_campaign scope-check --unit hoststep-scopecheck-<HHMMSS>` (ONE transient scope
#            around `true`; it signals nothing) must PASS, else nothing starts (exit 2; OUT/scope-check.txt).
#
# WHAT IT STARTS: the pre-flight's scope, then one rr_campaign supervisor as the service above, which starts its fake jobs
#   (each in its own rr-<campaign id>-* scope on the host). WHAT IT SIGNALS: nothing itself. WHAT IT STOPS: nothing; on an
#   ABORT condition (a signal_sent / scope_killed record not accounted for by this campaign, a dead or silent watcher, a
#   unit that cannot be observed for 5 min) it files `rr_campaign control OUT/c stop` (a request), never a kill.
#   Read-only: WS/ops/locks (the supervisor takes flocks there on --run), the held files, the pause flags.
set -u
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
usage() { sed -n '2,44p' "$0" | sed 's/^# \{0,1\}//'; }
# procfix7 round 8b: EVERYTHING runs from main(), which bash parses in full before it starts; install a new version only
# as a new file (a frozen copy in a dated directory), never by editing a file that may be running.
# alive PID: the process exists and is not a zombie
alive() { local st; st=$(sed -E 's/^.*\) (.).*$/\1/' "/proc/$1/stat" 2>/dev/null) || return 1; [ -n "$st" ] && [ "$st" != Z ]; }
# procfix7 round 9b: the supervisor's service unit, by its EXACT name ($SVC): running / ended / lingering / unknown
#   ended: unloaded (--collect) or inactive / failed; lingering: deactivating with no main process (processes are left
#   in the unit's own cgroup; KillMode=mixed: systemd SIGKILLs them, round 10 m13c-mixed-doc; P8 fails on a unit that
#   stays loaded)
sup_state() {
  local s
  s=$(systemctl --user show --property=LoadState,ActiveState,MainPID -- "$SVC" 2>/dev/null) || { echo unknown; return; }
  case "$s" in
    *LoadState=not-found*) echo ended ;;
    *ActiveState=deactivating*MainPID=0*|*MainPID=0*ActiveState=deactivating*) echo lingering ;;
    *ActiveState=active*|*ActiveState=activating*|*ActiveState=deactivating*|*ActiveState=reloading*) echo running ;;
    *LoadState=loaded*) echo ended ;;
    *) echo unknown ;;
  esac
}
sup_prop() { systemctl --user show --property="$1" --value -- "$SVC" 2>/dev/null; }
main() {
[ $# -eq 1 ] || { usage; exit 2; }
MODE=$1
case "$MODE" in --dry-run|--run|--rehearse-ns|--grade) ;; *) usage; exit 2;; esac
: "${REL:?set REL to the directory holding the release rr_campaign and rr_fake_child}"
RC=$REL/rr_campaign
FC=$REL/rr_fake_child
HT=$(dirname "$(readlink -f "$0")")
say() { echo "host_step_fake1: $*"; }
identity_map() { [ "$(tr -s ' ' < /proc/self/uid_map | sed 's/^ //')" = "0 0 4294967295" ]; }
[ -x "$RC" ] && [ -x "$FC" ] || { say "missing $RC or $FC"; exit 2; }
for g in p8_units.py grade_host_step.py; do
  [ -r "$HT/$g" ] || { say "missing $HT/$g (the grading, beside this script)"; exit 2; }
done
if [ "$MODE" = --grade ]; then
  : "${OUT:?set OUT to the directory of the step to grade}"
  [ -r "$OUT/supervisor.unit" ] && [ -r "$OUT/step.mode" ] || { say "$OUT holds no started step (supervisor.unit, step.mode)"; exit 2; }
  SVC=$(cat "$OUT/supervisor.unit"); STEPMODE=$(cat "$OUT/step.mode")
  LEDGER=$(cat "$OUT/step.ledger"); LEDGER_OFF=$(cat "$OUT/step.ledger_offset")
  if [ "$STEPMODE" = --rehearse-ns ]; then
    export PATH="$HT/rehearsal-fake-systemd:$PATH" FAKE_SYSTEMD_DIR="$OUT/fake-systemd"
  fi
  CLIENT=""
  say "grading the started step in $OUT: following $SVC by its exact name"
  follow_and_grade
fi
TS=$(date -u +%Y%m%dT%H%M%SZ)
OUT=${OUT:-/common/build-cache/vhirschi/p_integral_dev/host-steps/fake1-$TS}
ITEMS=${ITEMS:-240}
CASE_MS=${CASE_MS:-[60000,120000]}
# procfix7 round 11 (gate14 item 5b): HELPERS=<0-8> (default 0: the step as before): every rule job of the fake campaign
# FORKS that many honest helpers inside its own scope (same session and group, PR_SET_PDEATHSIG; half end on their own
# while the job runs, half are killed by the job itself at its end): the end-of-job proof must hold for forking jobs
# (P5 strict, unchanged grader)
HELPERS=${HELPERS:-0}
case "$HELPERS" in [0-8]) ;; *) echo "HELPERS must be 0-8" >&2; exit 2 ;; esac

# ---------------------------------------------------------------- preconditions
NFP=$(grep -a -c RR_CAMPAIGN_FAILPOINT "$RC" || true)
[ "$NFP" = 0 ] || { say "$RC names RR_CAMPAIGN_FAILPOINT $NFP time(s): not a release build"; exit 2; }
say "release check: RR_CAMPAIGN_FAILPOINT in $RC: 0"
env | grep -q '^RR_CAMPAIGN_JOB=' && { say "this shell carries RR_CAMPAIGN_JOB: start from a clean shell"; exit 2; }
case "$MODE" in
  --run)
    identity_map || { say "inside a user namespace: --run is the HOST step (use --rehearse-ns)"; exit 2; } ;;
  --rehearse-ns)
    if identity_map || [ "$(cat /proc/1/comm)" = systemd ]; then
      say "--rehearse-ns outside a private user+pid namespace: refused"; exit 2
    fi
    # inside a namespace the REAL systemd-run would reach the HOST's user manager (/run/user/<uid>): only the fakes
    export PATH="$HT/rehearsal-fake-systemd:$PATH" FAKE_SYSTEMD_DIR="$OUT/fake-systemd" ;;
esac
FAKES=no
systemd-run --version 2>/dev/null | grep -q 'REHEARSAL FAKE' && systemctl --version 2>/dev/null | grep -q 'REHEARSAL FAKE' && FAKES=yes
case "$MODE:$FAKES" in
  --rehearse-ns:no) say "the rehearsal fakes of systemd-run / systemctl are not the ones on PATH: refused"; exit 2 ;;
  --run:yes|--dry-run:yes) say "the systemd-run / systemctl on PATH are the REHEARSAL FAKES: refused"; exit 2 ;;
esac
for f in "$WS/ops/PAUSE" "$WS/plan/auto-campaign/implementer_ops/PAUSE"; do
  if [ -e "$f" ]; then
    say "pause flag $f exists: the campaign would launch nothing until it is gone"
    [ "$MODE" = --dry-run ] || exit 2
  fi
done

if [ "$MODE" = --rehearse-ns ]; then
  LOCKS=$OUT/locks; LEDGER=$OUT/CPU_LEDGER.md; CONT=',"containment":"marker_only"'
else
  LOCKS=$WS/ops/locks; LEDGER=$WS/ops/CPU_LEDGER.md; CONT=''
fi
SITE=$(cat <<J
{"schema":"rustred.campaign.site-policy.v1","tag":"rr-campaign",
 "lock_dir":"$LOCKS","lock_name":"cpu.{n}",
 "ledger_file":"$LEDGER","ledger_scan_lines":1500,"candidates":null,
 "exclude":["168-175","288-319","128-162,164-166","192-215,258-260,262,271,335","342,362,363,378"],
 "held_files":[{"path":"$WS/ops/FOREIGN_5L_LIFT.held","key":"held"},
               {"path":"$WS/ops/FOREIGN_5L_LIFT.json","key":"all_cpus_listed"},
               {"path":"$WS/ops/OUR_PINS.json","key":"entries[].cpus"}],
 "pause_files":["$WS/ops/PAUSE","$WS/plan/auto-campaign/implementer_ops/PAUSE"],
 "idle_min":0.85,"max_cores":8,"respect_foreign_pinning":true,"ignore_own_tag_claims":true}
J
)
SPEC=$(cat <<J
{"schema":"rustred.campaign.spec.v1","campaign":"host-fake-1","kind":"test",
 "dirs":{"campaign_dir":"$OUT/c"},
 "programs":{"rr_fake_child":{"path":"$FC"}},
 "resources":{"policy":"site","site_policy":"$OUT/site.json","max_jobs":8,"mem_total":"8G","host_floor":"50G",
              "disk_floor":"5G","grace_seconds":20,"sampler_ms":500,"heartbeat_timeout_s":60,"start_timeout_s":120,
              "pause_files":["$WS/ops/PAUSE","$WS/plan/auto-campaign/implementer_ops/PAUSE"]$CONT},
 "progress":{"cadence_seconds":2},
 "synthetic":{"items":$ITEMS,"nodes":3,"rungs":1,"seed":1,"case_ms":$CASE_MS,"p_norule":0.25,"p_mem":0.05,
              "p_crash":0.0,"deps":0.3,"mem_cap_mb":48,"honest_helpers":$HELPERS}}
J
)
HMS=$(date -u +%H%M%S)
UNIT=hoststep-fake1-$HMS
SVC=$UNIT.service
PFUNIT=hoststep-scopecheck-$HMS
SVC_CMD=(systemd-run --user --wait --collect "--unit=$UNIT" "--description=host step fake1 supervisor ($OUT)"
         -p Type=exec -p KillMode=mixed -p TimeoutStopSec=infinity -p MemoryMax=8G -p MemorySwapMax=0
         -p "UnsetEnvironment=RR_CAMPAIGN_JOB RR_CAMPAIGN_FAILPOINT"
         -p "StandardError=append:$OUT/run.log" -p "StandardOutput=append:$OUT/supervisor.stdout"
         "--working-directory=$OUT" "--setenv=PATH=$PATH" "--setenv=HOME=$HOME" ${TMPDIR:+"--setenv=TMPDIR=$TMPDIR"}
         -- "$RC" run --spec "$OUT/spec.json" --plain)
if [ "$MODE" = --dry-run ]; then
  say "DRY-RUN: nothing is started, signalled, stopped or created. OUT would be $OUT"
  echo "--- pre-flight (--run): $RC scope-check --unit $PFUNIT > $OUT/scope-check.txt  (must exit 0)"
  echo "--- $OUT/site.json"; echo "$SITE"; echo "--- $OUT/spec.json"; echo "$SPEC"
  echo "--- the supervisor's service (--run; the client in its own session, output >> $OUT/launcher.log):"
  printf 'setsid'; printf ' %q' "${SVC_CMD[@]}"; echo
  echo "--- follow:  systemctl --user status $SVC     (exact name; the unit is unloaded when it ends: --collect)"
  echo "--- stop:    $RC control $OUT/c stop     or     systemctl --user stop $SVC"
  echo "--- grade a step whose launcher ended:  REL=$REL OUT=$OUT bash $0 --grade"
  exit 0
fi

# ---------------------------------------------------------------- run
[ -e "$OUT" ] && { say "$OUT exists: choose a fresh OUT"; exit 2; }
mkdir -p "$OUT" || exit 2
if [ "$MODE" = --run ]; then
  # procfix7 round 9 (gate13 G13-M1): the real-systemd pre-flight; nothing starts unless it passes
  if "$RC" scope-check --unit "$PFUNIT" > "$OUT/scope-check.txt" 2>&1; then
    say "pre-flight scope-check PASS ($OUT/scope-check.txt)"
  else
    say "pre-flight scope-check FAILED: nothing was started; read $OUT/scope-check.txt"
    exit 2
  fi
  echo "$PFUNIT.scope" > "$OUT/preflight.unit"
else
  say "pre-flight scope-check: n.a. inside a namespace (no systemd)"
fi
if [ "$(systemctl --user show --property=LoadState --value -- "$SVC" 2>/dev/null)" != not-found ]; then
  say "$SVC is loaded already (or the user manager cannot be asked): nothing started"; exit 2
fi
if [ "$MODE" = --rehearse-ns ]; then
  mkdir -p "$LOCKS"
  for f in "$WS"/ops/locks/cpu.*; do : > "$LOCKS/$(basename "$f")"; done
  : > "$LEDGER"
fi
echo "$SITE" > "$OUT/site.json"; echo "$SPEC" > "$OUT/spec.json"
LEDGER_OFF=$(stat -c %s "$LEDGER")
echo "$MODE" > "$OUT/step.mode"; echo "$LEDGER" > "$OUT/step.ledger"; echo "$LEDGER_OFF" > "$OUT/step.ledger_offset"
echo "$SVC" > "$OUT/supervisor.unit"
[ "$MODE" = --run ] && systemctl --user list-units --all --no-legend --plain 2>/dev/null | awk '{print $1}' | sort > "$OUT/units.before"
: > "$OUT/units.seen"
say "OUT=$OUT mode=$MODE items=$ITEMS case_ms=$CASE_MS start $(date -u +%FT%TZ): the supervisor is the service $SVC"
printf '%q ' setsid "${SVC_CMD[@]}" > "$OUT/supervisor.command"; echo >> "$OUT/supervisor.command"
setsid "${SVC_CMD[@]}" >> "$OUT/launcher.log" 2>&1 < /dev/null &
CLIENT=$!
# the unit appears (Type=exec: once the supervisor was exec'd) or the client ends (the start failed)
for _ in $(seq 600); do
  st=$(sup_state)
  [ "$st" = running ] && break
  alive "$CLIENT" || break
  sleep 0.2
done
{ echo "started $(date -u +%FT%TZ)"; systemctl --user show --property=Id,InvocationID,MainPID,ControlGroup,ExecMainStartTimestamp -- "$SVC" 2>&1; } > "$OUT/supervisor.unit.start"
say "service $SVC: $(tr '\n' ' ' < "$OUT/supervisor.unit.start")"
follow_and_grade
}

# follow the supervisor's unit by its exact name ($SVC), supervise the watcher, then P1-P8 (all modes but --dry-run)
follow_and_grade() {
# watcher (LISTS only): the campaign id, the peak number of its scopes, the names of its units (units.seen, for P8),
# the ABORT condition; the heartbeat is written ATOMICALLY (a reader never sees a truncated, empty file)
beat() { date +%s > "$OUT/watcher.alive.tmp" && mv -f "$OUT/watcher.alive.tmp" "$OUT/watcher.alive"; }
beat
( ID=""; peak=0
  while ws=$(sup_state); [ "$ws" != ended ] && [ "$ws" != lingering ]; do
    beat
    bad=""
    [ -z "$ID" ] && ID=$(sed -n 's/.*INFO campaign id \([^ ]*\) .*/\1/p' "$OUT/run.log" 2>/dev/null | head -1)
    if [ -n "$ID" ] && [ "$(cat "$OUT/step.mode")" = --run ]; then
      n=$(systemctl --user list-units --no-legend --plain "rr-$ID-*" 2>/dev/null | grep -c . || true)
      [ "$n" -gt "$peak" ] && { peak=$n; echo "$peak" > "$OUT/scopes.peak"; }
      systemctl --user list-units --all --no-legend --plain "rr-$ID-*" 2>/dev/null | awk '{print $1}' >> "$OUT/units.seen"
    fi
    if [ -n "$ID" ] && ls "$OUT"/c/ledger/segment-*.jsonl >/dev/null 2>&1; then
      # procfix7 round 9 (gate13 m13-p5-lenient): the SAME check as P5 (grade_host_step.signal_problems)
      bad=$(python3 -I -B "$HT/grade_host_step.py" --abort-check "$OUT/c" "$ID" 2>&1)
      if [ -n "$bad" ]; then
        echo "ABORT CONDITION $(date -u +%FT%TZ): $bad" | tee -a "$OUT/abort.txt"
        "$RC" control "$OUT/c" stop --reason "host step ABORT condition: $bad" >/dev/null 2>&1 || true
        break
      fi
    fi
    sleep 10
  done ) &
WATCH=$!
# the watcher is supervised: dead, or silent for more than 120 s while the supervisor runs = ABORT; a unit that cannot
# be observed for 5 min (60 polls) = ABORT too (a stop request; the step then waits for the unit's end)
unknown=0; unobs=0
while :; do
  st=$(sup_state)
  case "$st" in
    ended|lingering) break ;;
    unknown) unobs=$((unobs + 1)) ;;
    *) unobs=0 ;;
  esac
  hb=$(cat "$OUT/watcher.alive" 2>/dev/null || true)
  silent=no
  case "$hb" in
    ''|*[!0-9]*) unknown=$((unknown + 1)); [ "$unknown" -gt 24 ] && silent=yes ;;
    *) unknown=0; [ $(( $(date +%s) - hb )) -gt 120 ] && silent=yes ;;
  esac
  why=""
  { ! alive "$WATCH" || [ "$silent" = yes ]; } && why="the watcher died or was silent for more than 120 s"
  [ "$unobs" -gt 60 ] && why="the supervisor's unit $SVC could not be observed for 5 min"
  if [ -n "$why" ]; then
    echo "ABORT CONDITION $(date -u +%FT%TZ): $why" | tee -a "$OUT/abort.txt"
    "$RC" control "$OUT/c" stop --reason "host step ABORT condition: $why" >/dev/null 2>&1 || true
    break
  fi
  sleep 5
done
# after an ABORT: wait for the unit's end (or, if it cannot be observed, for the client's)
while :; do
  st=$(sup_state)
  case "$st" in ended|lingering) break ;; esac
  [ "$st" = unknown ] && [ -n "$CLIENT" ] && ! alive "$CLIENT" && break
  sleep 5
done
wait "$WATCH" 2>/dev/null
# EXIT: systemd's exit status of the unit: the systemd-run --wait client's return code (it propagates the service's),
# or, for a lingering unit, its ExecMainCode / ExecMainStatus; --grade without the client: the client's report in
# launcher.log if it lived on; else unknown (255)
EXIT=255; HOW="unknown (no systemd report)"
if [ "$st" = lingering ]; then
  c=$(sup_prop ExecMainCode); s=$(sup_prop ExecMainStatus)
  case "$c" in 1) EXIT=$s ;; 2|3) EXIT=$((128 + s)) ;; esac
  HOW="the lingering unit's ExecMainCode $c ExecMainStatus $s"
elif [ -n "$CLIENT" ]; then
  for _ in $(seq 300); do alive "$CLIENT" || break; sleep 0.2; done
  if ! alive "$CLIENT"; then
    wait "$CLIENT"; rc=$?
    # the client's code is systemd's report only if the client saw the end (its terse end lines are in launcher.log)
    if grep -q '^Main processes terminated with: ' "$OUT/launcher.log" 2>/dev/null; then
      EXIT=$rc; HOW="systemd-run --wait"
    else
      HOW="unknown: the systemd-run --wait client ended (rc $rc) without reporting the unit's end"
    fi
  fi
else
  # procfix7 round 10 (gate13c m13c-grade-race): the client (if it lived on) prints its end lines right after the unit
  # ended: wait for them (bounded, 30 s) before reading
  for _ in $(seq 150); do grep -q '^Main processes terminated with: ' "$OUT/launcher.log" 2>/dev/null && break; sleep 0.2; done
  l=$(grep -m1 '^Main processes terminated with: code=exited, status=' "$OUT/launcher.log" 2>/dev/null)
  if [ -n "$l" ]; then EXIT=$(echo "$l" | sed -E 's/.*status=([0-9]+).*/\1/'); HOW="launcher.log (the client lived on)"; fi
fi
# procfix7 round 10 (gate13c m13c-p1-clean-signal): systemd-run --wait returns 0 also when the supervisor DIED of SIGTERM /
# SIGHUP / SIGINT / SIGPIPE (clean signals for a service; possible only before its handlers are installed): systemd's
# report then says code=killed: that is 128 + the signal, never 0
k=$(grep -m1 -oE '^Main processes terminated with: code=(killed|dumped), status=[0-9]+' "$OUT/launcher.log" 2>/dev/null | sed -E 's/.*status=//')
if [ -n "$k" ]; then EXIT=$((128 + k)); HOW="$HOW; systemd reports a death by signal $k"; fi
echo "exit $EXIT" | tee -a "$OUT/run.log"
{ echo "ended $(date -u +%FT%TZ): state $st, exit $EXIT from $HOW"; systemctl --user show --property=LoadState,ActiveState,SubState,Result,MainPID -- "$SVC" 2>&1; } > "$OUT/supervisor.unit.end"
ID=$(sed -n 's/.*INFO campaign id \([^ ]*\) .*/\1/p' "$OUT/run.log" | head -1)
say "campaign id $ID, service $SVC ended ($st), exit $EXIT ($HOW), end $(date -u +%FT%TZ)"

# ---------------------------------------------------------------- P1-P8
STEPMODE=$(cat "$OUT/step.mode")
"$RC" verify "$OUT/c" > "$OUT/verify.txt" 2>&1; VERIFY=$?
# A12 (lists only). procfix7 round 10 (host step 2: thousands of "Permission denied" lines from bash itself, whose input
# redirection failed before the 2>/dev/null applied): only processes whose REAL uid is ours are candidates (another
# user's process is never ours: the ownership rule) and only their environ is read; the real uid comes from status
# (readable for every process, also for our own non-dumpable ones, whose /proc entry is root's); an environ of OUR process
# that cannot be read while the process still exists is LISTED as UNREADABLE in a12.txt, never silenced
MARKED=0; UNREADABLE=0; ME=$(id -u)
for p in /proc/[0-9]*; do
  ruid=""
  { while read -r k v _; do [ "$k" = Uid: ] && { ruid=$v; break; }; done < "$p/status"; } 2>/dev/null
  [ "$ruid" = "$ME" ] || continue
  if e=$( { tr '\0' '\n' < "$p/environ"; } 2>/dev/null ); then
    if [[ $'\n'"$e" == *$'\n'"RR_CAMPAIGN_JOB=$ID:"* ]]; then
      MARKED=$((MARKED + 1))
      echo "MARKED ${p#/proc/} $( { tr '\0' ' ' < "$p/cmdline"; } 2>/dev/null | cut -c1-120)" >> "$OUT/a12.txt"
    fi
  elif [ -d "$p" ]; then
    UNREADABLE=$((UNREADABLE + 1))
    echo "UNREADABLE ${p#/proc/} (own real uid; environ not readable) $( { tr '\0' ' ' < "$p/cmdline"; } 2>/dev/null | cut -c1-120)" >> "$OUT/a12.txt"
  fi
done
say "A12: MARKED $MARKED; own processes with an unreadable environ $UNREADABLE (listed in $OUT/a12.txt; RUNBOOK A12: none may descend from this campaign)"
if [ "$STEPMODE" = --run ]; then
  # the step's own units end by themselves (--collect; the pre-flight's scope); give systemd a moment, never a signal
  for _ in $(seq 60); do
    [ "$(sup_prop LoadState)" = not-found ] && break
    sleep 0.5
  done
  SCOPES_AFTER=$(systemctl --user list-units --all --no-legend --plain "rr-$ID-*" 2>/dev/null | grep -c . || true)
  systemctl --user list-units --all --no-legend --plain 2>/dev/null | awk '{print $1}' | sort > "$OUT/units.after"
else
  SCOPES_AFTER=0; : > "$OUT/units.before"; : > "$OUT/units.after"
fi
ps -o pid=,stat= -u "$(id -un)" | awk '$2 ~ /^T/ {print $1}' > "$OUT/stopped.txt"
python3 -I -B "$HT/grade_host_step.py" "$OUT" "$ID" "$EXIT" "$VERIFY" "$MARKED" "$SCOPES_AFTER" "$STEPMODE" "$LEDGER" "$LEDGER_OFF" "$WS"
P=$?
say "criteria written to $OUT/pass.txt; result $([ $P = 0 ] && echo PASS || echo FAIL)"
exit $P
}
main "$@"
# shellcheck disable=SC2317 # never reached (main exits); keeps bash from reading anything appended later
exit $?
