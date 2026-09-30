# Fresh five-loop rules, then the optimized parallel campaign

This is the delivery path requested on 2026-09-30: **generate improved A1 owner
programs first**, admit every owner, and only then start the Epoch walk. A1 means
terms-first, then coefficient-monomial-count source visitation, with active
sectors first. It does not change the mathematical integral comparator, which
remains the native legacy order.

## Current recovery: fresh walk using the completed rules

The September30 user trial successfully generated and admitted all67 owners
(9,966 rules), then its Epoch walk stopped after403.143s with
`P1: 2387377 anchors: UnionUndecided`. This was an internal reuse-validation
failure, not a memory stop or failure to generate the rules. The finite-point
reuse planner can accept a union whose separate region-decomposition proof
exceeds its work budget; the original release incorrectly made that inconclusive
recheck fatal. A conservative Epoch-only preflight/fallback repair has passed
independent source review and1,292 native application/CLI tests. It declines
reuse and inspects the whole obligation when the persisted union proof cannot
be obtained within its budget; it does not weaken coverage checks. The combined
four-loop control has passed independent full reinspection (all58 required
queries,32 roots and17,957 inspections). The bounded full-input five-loop
control also passed: it exercised four conservative fallbacks, saved cleanly,
and its structural cold reader found no violations across165,305 accepted G2
unions. That deliberately stopped campaign remains incomplete; the cold check
did not re-inspect native records or execute a resume. The optimized replacement
completed successfully and was atomically published at00:44 October1
(Europe/Zurich). The user's waiting launcher then started the fresh campaign.
Additional optimized controls were explicitly deferred until after launch;
they were not a release gate and are not claimed passed here.

The delivered CLI is
`/common/dev/rustred/TMP/postlaunch-20260930/epoch-union-repair-optimized-bin/rustred`,
SHA256 `23d836d7b05fac6b007cc1adea84f04548d5bf0e0a94a28ca877f98942da8045`.
Its source is `d55cfb1c`, built with the optimized campaign profile (opt-level3,
fat LTO, one codegen unit). Guarded compilation took3,706.452s; that is build
time, not solver time. The immutable sibling `FROZEN_BUILD.json` records the
successful build and deferred post-build checks.

**Do not relaunch the old affected executable.** Retain the
failed campaign, generated bundles and completed-sector receipts. Its CP6 walk
checkpoint is marked poisoned and is not resumable; do not delete that marker.
Recovery uses a fresh walk with the saved generated inputs and the repaired
executable, without regenerating rules. The historical release commands and
measurements below remain evidence, not an instruction to retry the old binary.

In `codex_astra`, with the Symbolica license already in the environment:

```sh
cd /common/dev/rustred/TMP/releases/20260930-epoch-union-repair
nix develop --command python -B \
  /common/dev/rustred/TMP/postlaunch-20260930/epoch-union-repair-delivery/launch.py
```

This machine-local release launcher waits for the completed optimized binary,
then invokes the public `production_saved_owner_campaign.py` preparation and
start commands. It copies/verifies the existing inputs, preserves their query
order and all183 queries, and starts a fresh campaign at
`/common/dev/rustred/campaigns/five-loop-a1-epoch-repaired-20260930`.
This launch has now occurred. The old campaign remains untouched. Do not invoke
the initial command again: a pre-existing
destination is not permission to overwrite campaign state.

The resource settings remain32 workers on CPUs64–95,600GB requested ceiling,
150GB host headroom and hourly checkpoints. The actual RAM ceiling can be lower
after shared-host admission. At this launch the preview admitted approximately
473GB hard/450GB soft, rather than the full600GB, to preserve host headroom.
The supervisor's live admission is authoritative. Once this new campaign has saved a clean checkpoint,
its subsequent user-initiated resume is:

```sh
python -B examples/python/production_saved_owner_campaign.py \
  --campaign-directory /common/dev/rustred/campaigns/five-loop-a1-epoch-repaired-20260930 \
  --resume --start
```

That resume command applies only to the new campaign after a successful save,
not to the poisoned original. The initial launcher waiting message does not
mean traversal has begun.

## Readiness and what the command will do

The recipe and Python pipeline have passed independent review and a native
four-loop end-to-end test. The optimized CLI SHA-256 is
`38e0e7637a228aeccb13902915378d70f25736323916976819d2145ea19b1d89`;
the matching limits-aware inspector is
`925a778b3393016bedee38f85e9db31b7a82a6aadb8e3c568c93ed740e1b4187`.
The frozen Python checkout is commit `b21e4522` at
`/common/dev/rustred/TMP/releases/20260930-a1-epoch`. Its actual Nix environment
and full five-loop metadata preparation have been checked without launching
native work. The final optimized controls below are complete. This is a ready
fresh-campaign trial, not a demonstrated universal Epoch speed advantage.

At initial release no five-loop A1 payload set existed. The user trial has now
generated and admitted it. For a fresh pipeline, metadata preparation deliberately
uses `UNGENERATED/...` owner paths: there is no fallback to old owner payloads,
and walking cannot start until selected generations and native admission succeed.

The stages are:

1. Validate the frozen family, original selected-owner manifest, complete query
   roles and generation recipe; prepare a new, disjoint directory.
2. Generate the selected owner jobs in four parent groups, sequentially sharing
   one user-specified worker/CPU budget. Every individual sector uses the existing
   generic Rust solver. Full native zero-sector preparation is retained.
3. Strictly stage all new saved owners with the existing v4 stager. Missing
   jobs, mismatched source policies, wrong parents or incomplete reports stop
   the pipeline. All routing and query scope are preserved.
4. Run the native inspector on the complete selected set, checking actual
   family, selected sectors and mathematical order, using the manifest's load
   allowances. This is admission, not a closure proof.
5. Freeze the executable and walking options with the existing production
   launcher, then hand control to its dashboard, RAM protection, periodic
   checkpointing and resume machinery.

There is no production runtime deadline. The thirty-minute rule applies to
development pilots, not this user-launched solve. RAM limits are configurable;
750 GB is not rejected by an arbitrary 500 GB maximum. Shared-host admission
may lower the effective limit to preserve the requested host headroom.

## Measured release trade-offs

The final comparisons use the same optimized executable, saved rules, CPU
placement and **16-worker budget** in each arm. Times include native launch
through process-group drain plus independent full cold reinspection; they
exclude compilation and saved-rule generation equally.

| Control | Ready native + cold | Epoch native + cold | Interpretation |
|---|---:|---:|---|
| Combined four-loop, two runs per engine, median | 15.494 s | 16.293 s | Epoch 5.16% slower wall; 34.14% less native CPU; 33.04% fewer domains |
| Finite five-loop, one pair | 319.826 s | 311.466 s | Epoch 2.61% faster total, but native walk command 3.35% slower; advantage comes from cold checking |

All six runs passed independent full native reinspection. The five-loop control
covers one 1,324-point starting query with unrestricted reachable descendants,
not all 116 production queries, and uses historical saved owners rather than
new A1 owners. Its Ready/Epoch inspection counts are 760,609/754,320; both have
zero uncovered obligations or frontiers. Epoch uses about 11.5% less native CPU
in that pair. This is modest work/CPU evidence, not a repeated five-loop speed
qualification. The separate Ready Python diagnostic timed out; it is neither
a PASS nor included as completed work in the timing comparison. Native cold-All
is the independent acceptance authority for both engines.

A1 source selection remains the stronger measured optimization: earlier matched
four-loop pairs reduced complete generation/walk/check time by 17.45% and 18.73%,
and domain work by roughly 40–44%. The selected launch combines that strategy
with the tested Epoch implementation to obtain full-scale evidence. Ready is
still a competitive alternative; no claim of 20+ busy cores has been established.

At the user's request the production command now reserves **32 physical cores
and 600 GB RAM**, not the 16 cores used in those timings. This changes the
reservation, not the evidence. There is no measured 32-core speedup yet.

### Which parallel mechanisms the selected recipe enables

The current recipe explicitly selects31 inspection workers, one coordinator,
and **zero merge-preparation helpers** (`--epoch-preparation-workers 0`). Zero
means serial P2 preparation, not automatic allocation. The implemented helper
pool can parallelize immutable source resolution, owner/phase buckets and
bounded reverse lookup, but it is not enabled by this recipe. Ordered folding,
validation and mutable publication still impose serial work.

Initial owner/routing verification separately uses the32-worker budget; its
activity must not be presented as sustained traversal scaling. Historical W16
four-loop helper comparisons did not establish a reliable whole-run gain.
The larger full-input repair pilot shows P2 is substantial, motivating a matched
runtime-only h0/h2 follow-up. That comparison is prepared, not yet run; retaining
h0 in the recovery instructions preserves the original tested configuration,
not a claim that it is optimal for all67 owners at W32.

## Exact frozen scope

The new selection retains all **67 owners, 8,246 labelled routing records,
116 required queries and 67 auxiliary queries**. Query geometry and order are
unchanged, and descendants are not clipped to the initial numerator bound.
The source selection is the immutable input to the existing LC2 campaign;
its checkpoint and running process are never modified.

| Parent | Root in original family axes | Selected jobs |
|---|---|---:|
| 30527 | `111011100111111` | 17 |
| 30699 | `111011111101011` | 18 |
| 31740 | `111101111111100` | 11 |
| 32745 | `111111111101001` | 21 |

Parent 31740 includes owner `011101110111000`, native sector 3822. Its old
standalone row had no parent; its saved original root establishes this explicit
assignment. The recipe records it rather than guessing or omitting the owner.
New native selected-sector ordinals replace historical ordinals.

The production generation policy is numerical depth 0, numerator rank 10,
`SearchFinite`, factorized sparse exact arithmetic, and the existing case
allowances 8192 / 100000 / 1024 / 4096. These are solver policies, not evidence
of closure. The final walk discharges the requested domains and their actual
reachable dependencies. Completion would mean **scoped closure**, not an
unrestricted five-loop family theorem or a minimal-master claim.

The selected-sector subset changes which independent generation jobs run;
it does not shrink the frozen routing or physical query request. Nothing in
the pipeline implementation dispatches on a topology name or loop count.
The five-loop data live in
`examples/input/five_loop_qcd_feynman_d9d10/selected_generation.json`.

## Important affordability and interruption limits

The earlier standalone 3822 A1 probe was censored after 1,350.62 seconds. It had
no completed sector shard or final bundle, with a late sampled RSS of about
1.46 GB and essentially one busy core. Its checkpoint contained preparation
metadata only. This was incomplete pilot evidence, not the eventual generation
result. The subsequent user trial completed all four parent groups: 164.378,
560.941, 2,563.821 and 102.287 seconds respectively (native guard wall times).
Their sum is 3,391.427 seconds, excluding inter-group orchestration and admission;
it is not an end-to-end campaign timing. All 67 owners were admitted, with
9,966 rules and 939 finite residual cases. No regeneration is needed for the
reuse-validation repair. Full-scope walking completion remains unknown.

Generation checkpoints preserve completed sectors. They do **not** preserve
the active sector's GPLU computation. During generation, Ctrl-C or a RAM stop
terminates and drains the owned process group; a restart repeats unfinished
sectors while reusing completed ones. The wrapper does not call this a
cooperative in-sector checkpoint, and it never loops automatically after a RAM
or algebra failure.

Native bundle assembly still has a 1 GiB per-bundle limit, distinct from the
configurable checkpoint disk allowance and RAM ceiling. Exceeding that bound
is an explicit blocker, not permission to omit owners or replace fresh rules.
All 67 historical selected payloads total about 1.28 GiB: this is why the
inspector must honor aggregate allowances from `selection.load_limits`, rather
than use its former small-study aggregate default. An inspector refusal is
fatal to the pipeline; it is never replaced by a corner-only load smoke.
The command below allows 4 GiB of generation-checkpoint storage **per parent**;
that is not a global campaign disk limit or a memory limit.

## Preparation and launch

Use a **frozen release checkout** for Python and the explicit frozen binaries,
so later edits in the development checkout cannot alter a long-running pipeline
or its resume command. The paths below are the tested release on this machine;
keep them for future resumes. Do not point the production command at a changing
Cargo target executable. No recompilation is needed for this frozen release:

```sh
export RUSTRED_RELEASE_CHECKOUT=/common/dev/rustred/TMP/releases/20260930-a1-epoch
export RUSTRED_FROZEN_CLI=/common/dev/rustred/TMP/aster-integration-20260930-resumed/optimized-escrow-bin/rustred
export RUSTRED_FROZEN_INSPECTOR=/common/dev/rustred/TMP/aster-integration-20260930-resumed/optimized-inspector-limits-bin/inspect_candidate_orders
export RUSTRED_A1_CAMPAIGN=/common/dev/rustred/campaigns/five-loop-a1-epoch-20260930
cd "$RUSTRED_RELEASE_CHECKOUT"
nix develop
```

The current Symbolica license must be present in the environment; do not add it
to the recipe or command receipts. In the `codex_astra` tab of Zellij session
`rustred`, prepare the pipeline:

```sh
python -B tools/research/runtime_order/pipeline.py \
  --directory "$RUSTRED_A1_CAMPAIGN" \
  --recipe examples/input/five_loop_qcd_feynman_d9d10/selected_generation.json \
  --selection /common/dev/rustred/campaigns/five-loop-qcd-feynman-d9d10-lc2/inputs/selection.json \
  --executable "$RUSTRED_FROZEN_CLI" \
  --inspector "$RUSTRED_FROZEN_INSPECTOR" \
  --workers 32 --cpus 64-95 \
  --max-memory-bytes 600000000000 \
  --host-memory-reserve-bytes 150000000000 \
  --checkpoint-max-bytes 4294967296
```

This command does **not** start a native process. It records the complete recipe,
binary identities, resource policy and exact per-parent commands. It validates
the existing production walking policy before any expensive solve. The example
uses 32 distinct physical cores separate from LC2's 128–227 reservation and a
600 GB requested memory ceiling. The guard rechecks availability at every start.
To retain the full 600 GB ceiling with the explicit 150 GB host reserve, arrange
at least 750 GB of available host memory before launching; otherwise the effective
ceiling is reduced. The 5% guard margin requests save/stop near 570 GB when the
full ceiling is admitted. There is no claim that all 32 cores will remain busy
inside one difficult sector. The walk reserves 31 inspectors plus a coordinator;
generation shares the same total worker budget rather than creating extra pools.

Review `pipeline.json`, `shared/selection.json`, `shared/queries.json` and
`commands/parent-*.json` in the new directory, then launch the entire sequence:

```sh
python -B tools/research/runtime_order/pipeline.py \
  --directory "$RUSTRED_A1_CAMPAIGN" --resume --start
```

Here `--resume` means reuse the prepared pipeline metadata; it does **not** imply
that any native work was already completed. The first invocation generates all
owners. Native `--resume` is passed only when that parent's current checkpoint
manifest exists. On interrupted final assembly, the same bound output/report
paths may be reassembled; unrelated files are never targets.

Each parent runs at the full configured budget, one parent at a time. Within it,
selected sectors share the Rust executor. This avoids four hidden worker pools.
The generated `--progress` output goes to the phase's `stderr`, with its latest
line and completed-sector count surfaced alongside elapsed time, observed cores
and RSS. Once the walk starts, the existing coloured campaign dashboard takes
over. Epoch is configured with base window 76, cut 16, 1,024 extra completed-result
slots, a 256 MiB returned-result admission threshold, snapshot lookup, FIFO
dispatch and serial merge preparation. The total logical window is 1,100.
The result-byte threshold is not an RSS cap: already-running jobs can overshoot
it, so the independent process RAM guard remains necessary.

The recipe retains G2 union reuse, stop-on-frontier and no automatic rescue for
this initial launch. A frontier therefore produces an inspectable checkpoint,
not hidden query modifications. No starting query is removed.

## Stop, resume, inspect and rollback

During generation, press Ctrl-C and wait for the owned process group to drain.
The message gives the pipeline directory and restart command. After diagnosing
the cause, use the same command shown above. Completed parent outputs are reused
only with matching receipts; partially staged directories are preserved as
evidence and a new atomic staging attempt is used.

RAM policy can be overridden without changing mathematical inputs or worker
allocation, for example:

```sh
python -B tools/research/runtime_order/pipeline.py \
  --directory "$RUSTRED_A1_CAMPAIGN" --resume --start \
  --max-memory-bytes 750000000000 --ram-guard-margin-percent 5
```

That limit still requires sufficient current host headroom. The override is
per invocation; it does not rewrite the original recipe. Workers, CPUs, source,
selection and generation policies remain frozen.

During the walk, Ctrl-C instead requests the native durable checkpoint. Wait for
the saved-checkpoint message. The same pipeline resume command skips completed
generation/admission and invokes the frozen production launcher with `--resume`.
All Epoch options, including extra slots and byte allowance, are replayed from
frozen steering. A raw native command that omits those options defaults to zero
extra slots and is **not** an equivalent resume command.

For a read-only snapshot, obtain the current walking run directory from:

```sh
python -c 'import json,os; from pathlib import Path; p=Path(os.environ["RUSTRED_A1_CAMPAIGN"]); print(json.loads((p/"active-run.json").read_text())["run_directory"])'
python -B examples/python/campaign_monitor.py /the/printed/run/directory --once
```

Before `active-run.json` exists, the pipeline is still generating/admitting.
Inspect `attempts/*/{request.json,resources.jsonl,result.json,stderr}` instead.
The walk also supplies the existing `telemetry.jsonl` stream for
`examples/python/plot_campaign_rates.py`; use the actual run's filename, not a
generation resource stream. Closure estimates are conservative and do not imply
eventual termination.

Rollback is simply to leave this new directory paused and keep using the
unchanged LC2 tab, checkpoint and frozen executable. Do not load an old scalar-v4
Epoch checkpoint into the new scalar-v5 implementation, copy old walker state
into this campaign, or use the CP5 binary-upgrade convenience path for CP6.
There is no rollback that turns newly generated A1 rules into an old checkpoint's
saved rule identity.

## Acceptance and remaining checks

The actual fresh four-loop pipeline passed on 2026-09-30: 4+12 newly generated
sectors, 523 rules and 28 finite residuals, all 16 owners admitted, all 508
routing records preserved, and every one of the 58 required queries verified
through 32 roots. Native cold-All reinspection checked all 17,957 inspections
with zero violations, errors, uncovered obligations or frontiers. The measured
generation/admission/walk phase took 18.142 s; its separate cold check took
10.145 s. These phase times are not a comparative speed claim. No old sector
payload was substituted. The 23-test installed Python API compatibility check
also passed against the new CLI; this used the existing selected-profile wheel,
not a newly built wheel or a new PyO3 owner-walk method.

Evidence is retained under
`TMP/aster-integration-20260930-resumed/order-pilot-plans/pipeline-acceptance/`
in `ACCEPTANCE_RESULT.json`, `RESULTS.md` and the raw per-phase receipts.
All owned processes drained after the test; LC2 was untouched. The dashboard's
closure counter was still a conservative stale lower bound at short-run exit;
the separate cold check, not that display or exit code 4, established closure.

Metadata tests cover scope/role preservation, stale payload-provenance removal,
missing-parent refusal, failed generation, checkpoint resume, atomic staging
recovery, fixed identity/resource options and guard failure reporting. These use
synthetic payload bytes and do not establish native correctness.

Metadata preparation is not a substitute for that native test. After the
five-loop walk eventually drains,
run the independent raw-checkpoint verifier, using the actual final run:

```sh
export RUSTRED_A1_RUN="$(python -c 'import json,os; from pathlib import Path; p=Path(os.environ["RUSTRED_A1_CAMPAIGN"]); print(json.loads((p/"active-run.json").read_text())["run_directory"])')"
env RAYON_NUM_THREADS=1 OMP_NUM_THREADS=1 OMP_THREAD_LIMIT=1 \
  OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 BLIS_NUM_THREADS=1 \
  SYMBOLICA_HIDE_BANNER=1 \
  taskset -c 64-95 "$RUSTRED_FROZEN_CLI" walk-verify-closure \
  --command "$RUSTRED_A1_RUN/request.json" \
  --checkpoint "$RUSTRED_A1_CAMPAIGN/checkpoints/main" \
  --no-result --require-closure --reinspect all \
  --certification-scope all-roots --reference-levers off --threads 32 \
  --output "$RUSTRED_A1_RUN/cold-all.json"
```

Run this only after the owned walking processes have exited. The native command
and immutable query bytes come from `request.json`, not a handwritten command
with omitted options. Check `verdict: PASS`, zero violations/frontiers, all
native records reinspected and the complete query/root census. This command
tests all retained roots, including helpers; it is stronger than checking only
the 116 required query IDs. A different explicit certification scope must not
be silently substituted. It has no built-in production RAM supervisor, so
reserve appropriate memory/affinity when launching this separate final check.
CP6's checkpoint-only exit 4 is not by itself a PASS.

No all-five-loop scoped closure, finite runtime bound, 200-core utilization,
minimal terminal set or Vakint numerical result is claimed by this delivery.
The separate old-saved-owner + Epoch path remains possible via
`production_saved_owner_campaign.py --prepare-from ...`; it is an explicit
fallback **without A1 regeneration**, not the user's chosen primary pipeline.
