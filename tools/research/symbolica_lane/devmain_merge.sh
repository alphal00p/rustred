#!/usr/bin/env bash
# Reproduce the "devmain" trial tree: upstream Symbolica dev (445b882d) merged with upstream main (70375b9e).
# usage: nix develop --command bash tools/research/symbolica_lane/devmain_merge.sh OUTDIR (from the worktree root;
# never touches the checkout of vendor/symbolica)
set -eu
OUT=$1
git -C vendor/symbolica fetch https://github.com/symbolica-dev/symbolica '+refs/heads/*:refs/remotes/upstream/*'
git -C vendor/symbolica worktree add --detach "$OUT" 445b882d6e0dab7e97c8b302f420fe4e538d0955
cd "$OUT"
git -c user.name=ValentinHirschi -c user.email=valentin.hirschi@gmail.com merge --no-edit 70375b9e || true
# Conflicts are confined to Cargo.toml (comment line) and build.rs (worktree rebuild fix): take main's side,
# then restore dev's reconstruction example block that main's Cargo.toml lacks.
git checkout --theirs Cargo.toml build.rs
python - <<'PY'
s = open('Cargo.toml').read()
block = '[[example]]\nname = "reconstruction_joint_benchmark"\nrequired-features = ["native_code_generation"]\n\n'
if 'reconstruction_joint_benchmark' not in s:
    i = s.index('[[bench]]')
    s = s[:i] + block + s[i:]
open('Cargo.toml', 'w').write(s)
PY
git add Cargo.toml build.rs
git -c user.name=ValentinHirschi -c user.email=valentin.hirschi@gmail.com commit -q --no-edit
git log --oneline -1
