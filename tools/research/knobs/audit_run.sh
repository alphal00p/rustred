#!/usr/bin/env bash
# Audit knob run directories (streaming audit of result.json) on the
# lane CPUs (AUDIT_CPUS, default 44-51,300-307). Usage: audit_run.sh <run-dir> [more run dirs...]
set -uo pipefail
WT=/common/dev/rustred/.claude/worktrees/agent-ab06981cd80007c75
for run in "$@"; do
  [[ -f "$run/argv.json" ]] || nix develop /common/dev/rustred --command python -c "import json,sys; json.dump(json.load(open(sys.argv[1]))['argv'], open(sys.argv[2],'w'), indent=1)" "$run/command.json" "$run/argv.json"
  nice -n 5 taskset -c "${AUDIT_CPUS:-44-51,300-307}" nix develop /common/dev/rustred --command python "$WT/examples/python/audit_owner_domain_walk.py" "$run" --command "$run/argv.json" > "$run/audit.stdout" 2>&1
  echo "$run exit=$? violations=$(nix develop /common/dev/rustred --command python -c "import json,sys; print(len(json.load(open(sys.argv[1]))['violations']))" "$run/audit.json" 2>/dev/null)"
done
