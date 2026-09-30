# Fresh five-loop rules, then the optimized parallel campaign

This is the delivery path requested on 2026-09-30: **generate improved A1 owner
programs first**, admit every owner, and only then start the Epoch walk. A1 means
terms-first, then coefficient-monomial-count source visitation, with active
sectors first. It does not change the mathematical integral comparator, which
remains the native legacy order.

## Readiness and what the command will do

The recipe and Python pipeline have passed independent review and a native
four-loop end-to-end test. The optimized CLI SHA-256 is
`38e0e7637a228aeccb13902915378d70f25736323916976819d2145ea19b1d89`;
the matching limits-aware inspector is
`925a778b3393016bedee38f85e9db31b7a82a6aadb8e3c568c93ed740e1b4187`.
The final performance report and frozen Python checkout still need to be
recorded before the launch command below is recommended. Until its concrete
checkout bindings are filled in, this remains a **preparation runbook**.

No new five-loop A1 payload set exists yet. Metadata preparation deliberately
uses `UNGENERATED/...` owner paths. The pipeline has no fallback to old owner
payloads and cannot start its walk until all selected generations and actual
native admission succeed.

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

The large standalone 3822 A1 probe was censored after 1,350.62 seconds. It had
no completed sector shard or final bundle, with a late sampled RSS of about
1.46 GB and essentially one busy core. Its checkpoint contained preparation
metadata only. Therefore the total time to regenerate all 67 A1 owners is
**unknown**; the earlier lower-loop wins do not settle this cost.

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

## Preparation and launch

Use a **frozen release checkout** for Python and the explicit frozen binaries,
so later edits in the development checkout cannot alter a long-running pipeline
or its resume command. The release handoff supplies that checkout path and the
CLI/inspector SHA-256 values. Do not point the production command at a changing
Cargo target executable. The code below deliberately requires these bindings:

```sh
export RUSTRED_RELEASE_CHECKOUT=/absolute/path/to/frozen-release-checkout
export RUSTRED_FROZEN_CLI=/absolute/path/to/frozen/rustred
export RUSTRED_FROZEN_INSPECTOR=/absolute/path/to/frozen/inspect_candidate_orders
export RUSTRED_A1_CAMPAIGN=/common/dev/rustred/campaigns/five-loop-a1-epoch
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
  --workers 16 --cpus 64-79 \
  --max-memory-bytes 400000000000 \
  --host-memory-reserve-bytes 150000000000 \
  --checkpoint-max-bytes 4294967296
```

This command does **not** start a native process. It records the complete recipe,
binary identities, resource policy and exact per-parent commands. It validates
the existing production walking policy before any expensive solve. The example
uses 16 disjoint cores and a 400 GB requested memory ceiling; the release
handoff must confirm that this affinity and headroom remain available alongside
LC2. There is no claim that 16 cores will remain busy inside one difficult sector.

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
  taskset -c 64-79 "$RUSTRED_FROZEN_CLI" walk-verify-closure \
  --command "$RUSTRED_A1_RUN/request.json" \
  --checkpoint "$RUSTRED_A1_CAMPAIGN/checkpoints/main" \
  --no-result --require-closure --reinspect all \
  --certification-scope all-roots --reference-levers off --threads 16 \
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
