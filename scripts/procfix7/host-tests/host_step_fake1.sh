#!/usr/bin/env bash
# THE FIRST HOST STEP (fake children), RUNBOOK 1.7 as ONE script (lane procfix7 round 7; gate11 M11-2 and its command,
# WS/notes/implementer.gate11.md "(H)"). It writes RUNBOOK 1.7's site.json and spec.json AS SHIPPED, runs the campaign and
# checks the pass criteria P1-P8 of RUNBOOK 1.7 (printed PASS / FAIL / n.a.; exit 0 only when every applicable one passes).
#
# Usage (from a clean shell; a zellij pane for --run):
#   REL=<dir holding the RELEASE rr_campaign and rr_fake_child>  bash host_step_fake1.sh --dry-run
#   REL=...  bash host_step_fake1.sh --run           THE HOST STEP (outside any namespace; the operator / coordinator)
#   REL=...  bash host_step_fake1.sh --rehearse-ns   ONLY inside a private user+pid namespace (refused elsewhere)
# Any other argument list prints this text and exits 2 without starting anything.
#
# --run      OUT=/common/build-cache/vhirschi/p_integral_dev/host-steps/fake1-<UTC> (override: OUT=...); the campaign
#            runs as `systemd-run --user --scope --quiet --unit=hoststep-fake1-<HHMMSS> -p MemoryMax=8G -p MemorySwapMax=0
#            -- $REL/rr_campaign run --spec $OUT/spec.json --plain 2>>$OUT/run.log`.
# --rehearse-ns  the same files except: lock_dir / ledger_file are PRIVATE COPIES under OUT (an empty cpu.N for every
#            WS/ops/locks/cpu.N; an empty ledger: a rehearsal never takes the host's CPU flocks nor writes the shared CPU
#            ledger), the spec carries "containment":"marker_only" (no systemd scope inside a namespace) and there is no
#            systemd-run. ITEMS / CASE_MS override the workload (default: the RUNBOOK's 240 items x 60-120 s, ~1 h).
# --dry-run  prints the files and the command, checks the preconditions; starts, signals, stops and creates nothing.
# PRE-FLIGHT (--run only; procfix7 round 9, gate13 G13-M1): `$REL/rr_campaign scope-check --unit hoststep-scopecheck-<HHMMSS>`
#            (ONE transient scope around `true` with a job's properties; it signals nothing) must PASS, else nothing starts
#            (exit 2; OUT/scope-check.txt): the evidence a job that ends before its first check is recorded from works
#            with THIS host's systemd (the kernel's record of the cgroup at the exit, systemd-run's acknowledgment form).
#
# WHAT IT STARTS: one rr_campaign supervisor (on the host inside the hoststep-fake1-* scope), which starts its fake jobs
#   (each in its own rr-<campaign id>-* scope on the host). WHAT IT SIGNALS: nothing itself (the supervisor's guarded
#   signals only). WHAT IT STOPS: nothing; on an ABORT condition (a signal_sent / scope_killed record naming something not
#   of this campaign) it asks the campaign itself to stop (`rr_campaign control OUT/c stop`, a request file), never a kill.
#   Read-only: WS/ops/locks (the supervisor takes flocks there on --run), the held files, the pause flags.
set -u
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
usage() { sed -n '2,33p' "$0" | sed 's/^# \{0,1\}//'; }
# procfix7 round 8b (host step 1 ended without a verdict: this file was edited while a run was executing it and bash
# read the edited text): EVERYTHING runs from main(), which bash parses in full before it starts; install a new version
# only as a new file (a frozen copy in a dated directory), never by editing a file that may be running.
# alive PID: the process exists and is not a zombie
alive() { local st; st=$(sed -E 's/^.*\) (.).*$/\1/' "/proc/$1/stat" 2>/dev/null) || return 1; [ -n "$st" ] && [ "$st" != Z ]; }
main() {
[ $# -eq 1 ] || { usage; exit 2; }
MODE=$1
case "$MODE" in --dry-run|--run|--rehearse-ns) ;; *) usage; exit 2;; esac
: "${REL:?set REL to the directory holding the release rr_campaign and rr_fake_child}"
RC=$REL/rr_campaign
FC=$REL/rr_fake_child
HT=$(dirname "$(readlink -f "$0")")
TS=$(date -u +%Y%m%dT%H%M%SZ)
OUT=${OUT:-/common/build-cache/vhirschi/p_integral_dev/host-steps/fake1-$TS}
ITEMS=${ITEMS:-240}
CASE_MS=${CASE_MS:-[60000,120000]}
say() { echo "host_step_fake1: $*"; }
identity_map() { [ "$(tr -s ' ' < /proc/self/uid_map | sed 's/^ //')" = "0 0 4294967295" ]; }

# ---------------------------------------------------------------- preconditions
[ -x "$RC" ] && [ -x "$FC" ] || { say "missing $RC or $FC"; exit 2; }
for g in p8_units.py grade_host_step.py; do
  [ -r "$HT/$g" ] || { say "missing $HT/$g (the grading, beside this script)"; exit 2; }
done
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
    fi ;;
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
              "p_crash":0.0,"deps":0.3,"mem_cap_mb":48}}
J
)
UNIT=hoststep-fake1-$(date -u +%H%M%S)
if [ "$MODE" = --run ]; then
  CMD=(systemd-run --user --scope --quiet "--unit=$UNIT" -p MemoryMax=8G -p MemorySwapMax=0 -- "$RC" run --spec "$OUT/spec.json" --plain)
else
  CMD=("$RC" run --spec "$OUT/spec.json" --plain)
fi
PFUNIT=hoststep-scopecheck-$(date -u +%H%M%S)
if [ "$MODE" = --dry-run ]; then
  say "DRY-RUN: nothing is started, signalled, stopped or created. OUT would be $OUT"
  echo "--- pre-flight (--run): $RC scope-check --unit $PFUNIT > $OUT/scope-check.txt  (must exit 0)"
  echo "--- $OUT/site.json"; echo "$SITE"; echo "--- $OUT/spec.json"; echo "$SPEC"
  echo "--- command (--run):"; echo "systemd-run --user --scope --quiet --unit=$UNIT -p MemoryMax=8G -p MemorySwapMax=0 -- $RC run --spec $OUT/spec.json --plain 2>>$OUT/run.log"
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
else
  say "pre-flight scope-check: n.a. inside a namespace (no systemd)"
fi
if [ "$MODE" = --rehearse-ns ]; then
  mkdir -p "$LOCKS"
  for f in "$WS"/ops/locks/cpu.*; do : > "$LOCKS/$(basename "$f")"; done
  : > "$LEDGER"
fi
echo "$SITE" > "$OUT/site.json"; echo "$SPEC" > "$OUT/spec.json"
LEDGER_OFF=$(stat -c %s "$LEDGER")
[ "$MODE" = --run ] && systemctl --user list-units --all --no-legend --plain 2>/dev/null | awk '{print $1}' | sort > "$OUT/units.before"
: > "$OUT/units.seen"
say "OUT=$OUT mode=$MODE items=$ITEMS case_ms=$CASE_MS start $(date -u +%FT%TZ)"
"${CMD[@]}" 2>>"$OUT/run.log" &
SUP=$!
# watcher (LISTS only): the campaign id, the peak number of its scopes, the names of its units (units.seen, for P8),
# the ABORT condition
# the heartbeat is written ATOMICALLY (a reader never sees a truncated, empty file: the frozen 20261117T1030Z version
# read one and took it for 0, a false ABORT in its rehearsal)
beat() { date +%s > "$OUT/watcher.alive.tmp" && mv -f "$OUT/watcher.alive.tmp" "$OUT/watcher.alive"; }
beat
( ID=""; peak=0
  while alive "$SUP"; do
    beat
    bad=""
    [ -z "$ID" ] && ID=$(sed -n 's/.*INFO campaign id \([^ ]*\) .*/\1/p' "$OUT/run.log" | head -1)
    if [ -n "$ID" ] && [ "$MODE" = --run ]; then
      n=$(systemctl --user list-units --no-legend --plain "rr-$ID-*" 2>/dev/null | grep -c . || true)
      [ "$n" -gt "$peak" ] && { peak=$n; echo "$peak" > "$OUT/scopes.peak"; }
      systemctl --user list-units --all --no-legend --plain "rr-$ID-*" 2>/dev/null | awk '{print $1}' >> "$OUT/units.seen"
    fi
    if [ -n "$ID" ] && ls "$OUT"/c/ledger/segment-*.jsonl >/dev/null 2>&1; then
      # procfix7 round 9 (gate13 m13-p5-lenient): the SAME check as P5 (grade_host_step.signal_problems): a signal is
      # accounted for only by this campaign's own records (a group signal: that job's leader pid; a process signal:
      # its start time and this campaign's marker of that job; a scope kill: a scope unit this campaign launched)
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
# procfix7 round 8b: the watcher is supervised: dead, or silent for more than 120 s while the campaign runs = ABORT
unknown=0
while alive "$SUP"; do
  hb=$(cat "$OUT/watcher.alive" 2>/dev/null || true)
  silent=no
  case "$hb" in
    ''|*[!0-9]*) unknown=$((unknown + 1)); [ "$unknown" -gt 24 ] && silent=yes ;;
    *) unknown=0; [ $(( $(date +%s) - hb )) -gt 120 ] && silent=yes ;;
  esac
  if ! alive "$WATCH" || [ "$silent" = yes ]; then
    echo "ABORT CONDITION $(date -u +%FT%TZ): the watcher died or was silent for more than 120 s" | tee -a "$OUT/abort.txt"
    "$RC" control "$OUT/c" stop --reason "host step ABORT condition: the watcher died" >/dev/null 2>&1 || true
    break
  fi
  sleep 5
done
wait "$SUP"; EXIT=$?
wait "$WATCH" 2>/dev/null
echo "exit $EXIT" | tee -a "$OUT/run.log"
ID=$(sed -n 's/.*INFO campaign id \([^ ]*\) .*/\1/p' "$OUT/run.log" | head -1)
say "campaign id $ID, exit $EXIT, end $(date -u +%FT%TZ)"

# ---------------------------------------------------------------- P1-P8
"$RC" verify "$OUT/c" > "$OUT/verify.txt" 2>&1; VERIFY=$?
# A12 (lists only)
MARKED=0
for p in /proc/[0-9]*; do
  e=$(tr '\0' '\n' < "$p/environ" 2>/dev/null) || continue
  echo "$e" | grep -q "^RR_CAMPAIGN_JOB=$ID:" && { MARKED=$((MARKED + 1)); echo "MARKED ${p#/proc/} $(tr '\0' ' ' < "$p/cmdline" | cut -c1-120)" >> "$OUT/a12.txt"; }
done
if [ "$MODE" = --run ]; then
  SCOPES_AFTER=$(systemctl --user list-units --all --no-legend --plain "rr-$ID-*" 2>/dev/null | grep -c . || true)
  systemctl --user list-units --all --no-legend --plain 2>/dev/null | awk '{print $1}' | sort > "$OUT/units.after"
else
  SCOPES_AFTER=0; : > "$OUT/units.before"; : > "$OUT/units.after"
fi
ps -o pid=,stat= -u "$(id -un)" | awk '$2 ~ /^T/ {print $1}' > "$OUT/stopped.txt"
python3 -I -B "$HT/grade_host_step.py" "$OUT" "$ID" "$EXIT" "$VERIFY" "$MARKED" "$SCOPES_AFTER" "$MODE" "$LEDGER" "$LEDGER_OFF" "$WS"
P=$?
say "criteria written to $OUT/pass.txt; result $([ $P = 0 ] && echo PASS || echo FAIL)"
exit $P
}
main "$@"
# shellcheck disable=SC2317 # never reached (main exits); keeps bash from reading anything appended later
exit $?
