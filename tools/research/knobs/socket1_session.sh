#!/usr/bin/env bash
# Run a list of knob_run.py invocations on socket 1 under the shared socket-1
# lock (<= 60 min per session). Usage: socket1_session.sh <session-name> <plan-file>
# Each non-empty, non-# line of the plan file is the argument list of one
# knob_run.py call (the session enforces a 55-min budget: later lines are skipped).
set -uo pipefail
NAME=${1:?name}; PLAN=${2:?plan}
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
  now=$(date +%s)
  if (( now - start > ${BUDGET:-3300} )); then echo "$(date -u +%FT%TZ) budget exhausted, skipping: $line"; continue; fi
  echo "$(date -u +%FT%TZ) run: $line"
  eval "nix develop /common/dev/rustred --command python $RUNNER $line" 9>&-
  echo "$(date -u +%FT%TZ) exit $?"
done < "$PLAN"
echo "$(date -u +%FT%TZ) session done after $(( $(date +%s) - start )) s; releasing lock"
