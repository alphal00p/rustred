# Compatible stable upgrade, 2026-09-29

Status: optimized build and representative native upgrade/cold gates pass.
Independent final source, binary, receipt and delivery review passed. This
documentation milestone is ready for the stable fast-forward push. No live
campaign was changed or signalled.

The stable native source is `986d046ea9e50eccaa3c1da5ef6bc1af320ac8d0`
(tree `72f07e975c908c2f9624696474b5f4b402232860`), based on stable `e56b8cdb`.
Its two normal cherry-picks are `0dd30be7` (projected endpoint buffer reuse,
original reviewed slice `cab33228`) and `986d046e` (lean coordinator telemetry,
original reviewed slice `1a052c68`). Existing G2/rescue support is retained.
No private Epoch slice is included. Symbolica remains clean upstream
`ef0db494533c87adb40356c996241680dc5a7bff`, without local patches.

The new code reuses projected geometry storage and captures scalar worker
telemetry before JSON serialization. The compatible production upgrade keeps
LC2's frozen G2 mode **Off**, so the earlier 24–28% G2 work saving does not
describe this upgrade. No speedup has been measured for these two small
changes. No roles, query ordering, scheduling options or input bindings are
migrated.

## Build and validation

The independent stable clone is
`/common/dev/rustred/TMP/codex-stage-a.2RU3AX/repo`. Its private Cargo cache was
copied from the idle previous campaign cache with ordinary
`cp -a --reflink=auto`; the source and copied files have different inodes.
Cargo performed normal source/dependency validation and rebuilds. No cache
fingerprints or source mtimes were rewritten.

The exact build is:

```sh
cd /common/dev/rustred/TMP/codex-stage-a.2RU3AX/repo
env TMPDIR=/common/dev/rustred/TMP CARGO_INCREMENTAL=0 \
  nix develop --command cargo build --profile campaign --locked --offline \
  -j8 -p rustred-app --bin rustred
```

The actual build ran inside the existing guard with CPUs 0–15, shared
`heavy.lock` and `build-0.lock`, 250 GiB minimum initial headroom and a
150 GiB ongoing floor. The campaign profile has fat LTO and one codegen unit;
there is no mimalloc feature or target-CPU override. Compilation time is not
solver time. It exited zero without a stop reason after 3,566.486 seconds
(59 minutes 26 seconds); minimum observed available memory was
534,749,913,088 bytes. This exceeded the 30-minute Stage A target, as allowed
for correctness/build time. Stage B continued independently. Receipt:
`TMP/codex-stage-a.2RU3AX/campaign-build/`.

The new executable is:

```text
/common/dev/rustred/TMP/codex-stage-a.2RU3AX/candidate-bin/rustred-986d046e
SHA-256 0995f0fda2637eb4bf0bdc5b46249aba9c6f4ba513196143a2b07d3aaa739c21
110864632 bytes; frozen mode 0555
```

Its build-source identity is the native commit above, not any later
documentation-only commit. It is a newly built artifact, not the earlier
frozen `d12` campaign binary. Binary/source manifests are
`TMP/codex-stage-a.2RU3AX/{binary-freeze,source-freeze}.json`.

Verified:

- The complete core source and all changed telemetry implementation/regression
  files match fully tested `29e30a79` exactly. Those earlier receipts include
  2,845 passed core tests and all five named lean telemetry regressions.
  The earlier full app test executable also contains private Epoch code, so
  its full-suite total is not presented as an exact Stage A application run.
- The 13 existing Python executable-upgrade tests pass on this stable clone:
  `TMP/codex-stage-a.2RU3AX/python-upgrade-tests/`. These cover refusal without
  mutation, live-process/checkpoint locks, immutable options, interrupted
  upgrades, bounded probes and historical-binary rollback. They use fake
  native probes and do not themselves prove native resume.
- At 13:34:51 UTC, read-only verification matched all 67 LC2 owner files,
  selection and the exact 183-query / 121,424-byte document against its
  existing input receipt. The coherent manifest was CP5 generation 5,
  walk semantics 1. Frozen steering SHA-256 remained
  `f3330f58296bc4b70f0ed7b51d9ec60b0ccc3a54282775fc89ebbb4c62b78819`.
  Receipt: `TMP/codex-stage-a.2RU3AX/lc2-identity-before-build.json`.
- The read-only candidate check at 14:35:28 UTC again matched those exact
  LC2 input and steering hashes. The new binary's probe matches the observed
  LC2 manifest: `RUSTRED-WALK-CP5`, schema 5, walk semantics 1. No production
  write, lock acquisition, signal or solver launch was performed. Receipt:
  `TMP/codex-stage-a.2RU3AX/lc2-identity-candidate.json`.
- Actual old LC2 executable pause, candidate upgrade/resume, and fresh-process
  full cold reinspection pass for both Ordered/W6 and Ready/W6, G2 Off, all
  248 unchanged FG control queries. Each old process exited 4 after a durable
  work checkpoint and fully drained; each candidate exited 0, with zero
  unresolved domains/frontiers. Input bytes and frozen options were unchanged,
  and the steering argv changed only at the executable path. Both public
  upgrade probes and historical-binary rollback probes pass.

| Representative policy | Domains closed | Native records cold-reinspected | Roots independently verified | Whole drill wall time |
|---|---:|---:|---:|---:|
| Ordered/W6 | 98,909 | 98,869 / 98,869 | 248 / 248 | 42.018 s |
| Ready/W6 | 98,881 | 98,841 / 98,841 | 248 / 248 | 39.443 s |

Both cold verifiers and their paired Python audits report no violations.
These are correctness drills, including setup, pause, probes and verification,
not matched performance measurements. Ready may discover a different valid
domain set; no cross-policy identity or speedup is claimed. Receipts:
`TMP/codex-stage-a.2RU3AX/native-upgrade-smoke-{ordered-v3,ready}/`.

Two earlier Ordered attempts remain explicitly `INCOMPLETE`: the first test
controller incorrectly expected an Off-mode G2 report (Off intentionally
omits it), and the second direct verifier launch lacked required thread caps.
The native pause/resume legs completed in both; neither is counted as a
complete gate. Their logs remain in `native-upgrade-smoke-ordered/` and
`native-upgrade-smoke-ordered-v2/`. The corrected controller is frozen by hash
in each passing receipt and uses the standard six thread caps, owned-process
drain checks, heavy/build locks and 250/150 GiB host admission/run floors.

LC2's observed committed manifest references about 60.7 GB. These executions
are representative controls, not a full-size LC2 replay, all-topology campaign
test or production-speedup measurement. Each retains an independent coherent
paused checkpoint copy after the old writer drains. Rollback was checked by
the public executable-history probe; actual old-binary execution after the
candidate's new checkpoint was not performed. The pre-upgrade copies remain
available for an actual rollback drill if subsequently needed.

## Owner commands after Stage A is announced ready

Only the owner pauses or resumes production. In the current LC2 terminal,
press Ctrl-C once, then wait for the durable saved pause and exit 4 before
the upgrade. Keep LC2's existing inputs, checkpoints and frozen original
binary. Use the stable clone's launcher while the root checkout develops
`fable_5_1_parallel`.

The path below is the tested frozen Stage A artifact. Wait for the Stage A
ready announcement after independent review and push, then run the read-only
upgrade plan first:

```sh
cd /common/dev/rustred/TMP/codex-stage-a.2RU3AX/repo
env TMPDIR=/common/dev/rustred/TMP nix develop --command python \
  examples/python/production_saved_owner_campaign.py \
  --campaign-directory /common/dev/rustred/campaigns/five-loop-qcd-feynman-d9d10-lc2 \
  --resume --upgrade-executable \
  /common/dev/rustred/TMP/codex-stage-a.2RU3AX/candidate-bin/rustred-986d046e \
  --json
```

The same command with `--start` replacing `--json` performs the compatible
upgrade and resume. Omit workers, CPUs, query/owner options and G2 options:
the launcher retains the frozen values. A later ordinary resume is:

```sh
env TMPDIR=/common/dev/rustred/TMP nix develop --command python \
  examples/python/production_saved_owner_campaign.py \
  --campaign-directory /common/dev/rustred/campaigns/five-loop-qcd-feynman-d9d10-lc2 \
  --resume --start
```

To roll back, pause and wait for exit 4 again, then use the same upgrade
command with `--upgrade-executable` pointing to the original retained binary:

```text
/common/dev/rustred/campaigns/five-loop-qcd-feynman-d9d10-lc2/bin/rustred-fd9b9ac96a50513d8119a43938313e8a729110d6a3815b0f949cec4acb1d9d60
```

The executable history records the original binary's matching checkpoint
semantics, including when that original executable lacks the new probe.
Rollback still requires a paused campaign and all native request bindings to
match. No production action was performed during this delivery.
