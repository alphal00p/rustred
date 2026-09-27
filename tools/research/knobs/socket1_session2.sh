#!/usr/bin/env bash
# socket1_session.sh with a hard session budget: each plan line gets
# --timeout-seconds min(own timeout, SESSION_SECONDS - elapsed - 120) and is
# skipped when less than MIN_RUN seconds remain, so the lock is held <= 60 min.
# Usage: socket1_session2.sh <session-name> <plan-file>
set -uo pipefail
NAME=${1:?name}; PLAN=${2:?plan}
SESSION_SECONDS=${SESSION_SECONDS:-3540}
MIN_RUN=${MIN_RUN:-240}
LOG=/common/dev/rustred/TMP/w0/knobs/sessions/$NAME.log
mkdir -p "$(dirname "$LOG")"
RUNNER=/common/dev/rustred/.claude/worktrees/agent-ab06981cd80007c75/tools/research/knobs/knob_run.py
exec >>"$LOG" 2>&1
echo "$(date -u +%FT%TZ) waiting for socket1 lock"
exec 9>/common/dev/rustred/TMP/locks/socket1.lock
if ! flock -w 14400 9; then echo "lock timeout"; exit 3; fi
start=$(date +%s)
echo "$(date -u +%FT%TZ) socket1 lock acquired"
while IFS= read -r line; do
  [[ -z "$line" || "$line" == \#* ]] && continue
  elapsed=$(( $(date +%s) - start ))
  remaining=$(( SESSION_SECONDS - elapsed - 120 ))
  if (( remaining < MIN_RUN )); then echo "$(date -u +%FT%TZ) budget exhausted, skipping: $line"; continue; fi
  own=$(sed -n 's/.*--timeout-seconds \([0-9]*\).*/\1/p' <<<"$line")
  t=$remaining; [[ -n "$own" && "$own" -lt "$t" ]] && t=$own
  echo "$(date -u +%FT%TZ) run (timeout ${t}s): $line"
  eval "nix develop /common/dev/rustred --command python $RUNNER $line --timeout-seconds $t" 9>&-
  echo "$(date -u +%FT%TZ) exit $?"
done < "$PLAN"
echo "$(date -u +%FT%TZ) session done after $(( $(date +%s) - start )) s; releasing lock"
