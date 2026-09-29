#!/usr/bin/env python
"""Provenance sidecar for a lane binary (critique PROV-1).

Copies BINARY to OUT_DIR/rustred-ops-<arm>-<sha8> (unless --no-copy) and
writes <that path>.provenance.json with: the binary sha256; the git rev and
crates/ tree the build started from and whether crates/, Cargo.toml and
Cargo.lock still equal that rev in the working tree; the Cargo.toml and
Cargo.lock blob ids; the sha256 of the [profile.campaign] section of that
rev's Cargo.toml; the cargo profile and features; the vendor/symbolica
commit, the sha256 of its working-tree diff and whether the checkout is clean
(no local Symbolica patch); rustc/cargo versions; `nm -D` malloc/free.

usage: provenance.py --binary SRC --arm ARM --profile release|campaign
                     [--features F] --rev REV [--started UTC] [--finished UTC]
                     [--out-dir DIR] [--no-copy] [--note TEXT]
Run inside `nix develop` (rustc/cargo/nm on PATH) from the worktree.
"""
import argparse
import hashlib
import json
import shutil
import subprocess
from pathlib import Path

WT = Path(__file__).resolve().parents[3]

def git(*args, cwd=WT, check=True):
    return subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, check=check)


def sha256(path=None, data=None):
    digest = hashlib.sha256()
    if data is not None:
        digest.update(data)
    else:
        with open(path, "rb") as stream:
            for block in iter(lambda: stream.read(1 << 20), b""):
                digest.update(block)
    return digest.hexdigest()


def profile_section(text):
    lines, inside = [], False
    for line in text.splitlines():
        if line.startswith("["):
            inside = line.strip() == "[profile.campaign]"
        if inside:
            lines.append(line)
    return "\n".join(lines) + "\n"


def tool(*argv):
    try:
        return subprocess.run(argv, capture_output=True, text=True, check=True).stdout.strip()
    except (OSError, subprocess.CalledProcessError) as error:
        return f"unavailable: {error}"


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--binary", required=True, type=Path)
    p.add_argument("--arm", required=True)
    p.add_argument("--profile", required=True, choices=("release", "campaign"))
    p.add_argument("--features", default="")
    p.add_argument("--rev", required=True)
    p.add_argument("--started")
    p.add_argument("--finished")
    p.add_argument("--out-dir", type=Path, default=Path("/common/dev/rustred/TMP/w1-ops/bin"))
    p.add_argument("--no-copy", action="store_true")
    p.add_argument("--note")
    args = p.parse_args()
    full = sha256(args.binary)
    target = args.binary if args.no_copy else args.out_dir / f"rustred-ops-{args.arm}-{full[:8]}"
    if not args.no_copy:
        args.out_dir.mkdir(parents=True, exist_ok=True)
        shutil.copy2(args.binary, target)
        assert sha256(target) == full
    rev = git("rev-parse", args.rev).stdout.strip()
    clean = git("diff", "--quiet", rev, "--", "crates", "Cargo.toml", "Cargo.lock", check=False).returncode == 0
    vendor = WT / "vendor/symbolica"
    vendor_diff = subprocess.run(["git", "diff"], cwd=vendor, capture_output=True, check=True).stdout
    features = [f for f in args.features.split(",") if f]
    command = (f"cargo build --profile {args.profile} --locked --offline -p rustred-app --bin rustred"
               + (f" --features {','.join(features)}" if features else ""))
    nm = [line.strip() for line in tool("nm", "-D", str(args.binary)).splitlines()
          if line.split()[-1:] in (["malloc"], ["free"])]
    sidecar = {
        "schema": "rustred.ops-binary-provenance.v1", "binary": str(target), "sha256": full,
        "source_path": str(args.binary), "arm": args.arm, "cargo_profile": args.profile,
        "features": features, "cargo_command": command,
        "git_rev": rev, "crates_tree": git("rev-parse", f"{rev}:crates").stdout.strip(),
        "cargo_toml_blob": git("rev-parse", f"{rev}:Cargo.toml").stdout.strip(),
        "cargo_lock_blob": git("rev-parse", f"{rev}:Cargo.lock").stdout.strip(),
        "profile_campaign_section_sha256": sha256(data=profile_section(
            git("show", f"{rev}:Cargo.toml").stdout).encode()),
        "working_tree_crates_cargo_equal_rev_at_sidecar_time": clean,
        "vendor_symbolica_commit": git("rev-parse", "HEAD", cwd=vendor).stdout.strip(),
        "vendor_symbolica_diff_sha256": sha256(data=vendor_diff),
        "vendor_symbolica_clean": not vendor_diff,
        "rustc": tool("rustc", "-vV").replace("\n", "; "), "cargo": tool("cargo", "-V"),
        "nm_malloc_free": nm, "build_started_utc": args.started, "build_finished_utc": args.finished,
        "note": args.note, "family_closure_claim": False,
    }
    Path(str(target) + ".provenance.json").write_text(json.dumps(sidecar, indent=1) + "\n")
    print(json.dumps({k: sidecar[k] for k in ("binary", "sha256", "git_rev", "crates_tree",
                                              "working_tree_crates_cargo_equal_rev_at_sidecar_time",
                                              "vendor_symbolica_clean")}))


if __name__ == "__main__":
    main()
