#!/usr/bin/env bash
# Build/test against the devmain trial tree via cargo --config patch overrides (vendor/symbolica untouched).
# usage: build_devmain.sh <label> <cargo subcommand and args...>
S=/tmp/claude-1125/-common-dev-rustred/7dfabea8-fff6-436f-854b-2fed20c422f3/scratchpad/symlane/sym-merge
WT=/common/dev/rustred/.claude/worktrees/fable51-symbolica
label=$1; shift
sub=$1; shift
TD=$WT/TMP/target-devmain
exec $WT/tools/research/symbolica_lane/build_locked.sh "$label" "$sub" --target-dir "$TD" \
  --config "patch.crates-io.symbolica.path=\"$S\"" \
  --config "patch.crates-io.numerica.path=\"$S/lib/numerica\"" \
  --config "patch.crates-io.graphica.path=\"$S/lib/graphica\"" "$@"
