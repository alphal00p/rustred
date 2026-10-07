# A second campaign phase: master reduction

The saved-owner launcher can optionally continue from scoped IBP dependency
closure into a separate **Master reduction** phase. All algebra, relation
validation, checkpoint contents and symbolic artifacts are implemented in
RustRed. Python only launches and monitors the native stages.

These are different milestones. A drained phase-one queue is only admission
to the native cold-closure check. A completed phase-two search means that its
configured finite source search finished, not that the surviving integrals
are independent or form a globally minimal master basis. No numerical master
values are computed in this phase.

## Start on an existing completed campaign

Use the current launcher, not an older Python snapshot inside the campaign.
The independently frozen phase-two executable lets an existing campaign keep
its original phase-one binary, CP6 checkpoint and inputs unchanged:

```bash
nix develop --command python -B examples/python/production_saved_owner_campaign.py \
  --campaign-directory /absolute/path/to/campaign \
  --resume --master-reduction \
  --master-reduction-executable /absolute/path/to/new/rustred \
  --start
```

Without `--start` this prepares/displays the launch plan but does not run either
stage or enable the persisted phase-two policy. With `--start`, the opt-in and
optional separate binary are saved under `master-reduction/`. The original
`bin/` and frozen steering snapshot are not replaced. Subsequent ordinary
`--resume --start` invocations of the current launcher retain the opt-in.

For a fresh campaign, add the same master options to the ordinary preparation
and launch command, without `--resume`. Phase one still uses its existing
supervisor and safety policy. Once it drains, the native second stage verifies
the captured scope before attempting terminal relations.

The default `--master-seed-depth 0` searches ordinary IBPs at the finite seed
set. A later explicit increase, for example `--master-seed-depth 1`, creates
a new phase-two search scope and passes the prior completed artifact to Rust
for validated reuse of its exact rows. Depth cannot silently decrease. The
seed depth is **not** the input numerator-rank cap.

The current finite exact elimination uses one incremental Symbolica reducer.
The worker budget parallelizes cold scope reinspection, not that row stream;
the dashboard reports measured CPU usage rather than implying all reserved
workers are busy throughout post-processing.

## Stop, resume and enlarge the input scope

Press Ctrl+C once and wait for the owned native process to save and exit.
During phase two the launcher prints its separate checkpoint directory and
the command for resumption:

```bash
nix develop --command python -B examples/python/production_saved_owner_campaign.py \
  --campaign-directory /absolute/path/to/campaign --resume --start
```

If the original CP6 manifest, required-query amendment chain and source
documents are unchanged, this returns directly to **Master reduction**. Its
completed native work is checkpointed. Cooperative Ctrl+C finishes the current
atomic row before saving; an abrupt failure may require repeating work since
the last durable checkpoint. The phase-one worklist is not rerun just to resume
phase two.

Rank or power-difference extensions change the requested scope. Use the
current `extend_rank_campaign.py`: when phase two is enabled, it deliberately
uses the current phase-aware launcher rather than an older frozen Python
snapshot. Phase one discharges the newly added obligations first. Phase two
then gets a new scope directory, leaving the earlier artifact intact. Native
Rust may reuse exact rows from the prior completed artifact only after
authenticating its family and rule-program binding. This is reuse of proven
relations, not a claim that the old scope already covered the new one.

The phase dispatcher and native processes retain the configured CPU affinity,
worker budget, checkpoint interval, RAM ceiling and cooperative RAM stop.
There is no additional solve-time deadline. A hard memory emergency can still
require killing the owned process; only its previously published durable
checkpoint is then resumable.

## Monitoring and artifact inspection

TTY output is an overwriting colored table. It reports source and normalized
terminal counts, remaining basis size, relation work, sparse-system counters,
observed/reserved cores, RAM and checkpoint status. Unknown native counters
remain unknown. Non-TTY output is throttled structured JSON carrying the same
normalized telemetry, with no terminal escape sequences.

The producer (`campaign_telemetry.py`) is separate from the terminal consumer
(`campaign_dashboard.py`). Phase-two runs save `events.jsonl`, `telemetry.jsonl`
and `status.json`. A separate read-only observer can follow stage changes:

```bash
nix develop --command python -B examples/python/campaign_monitor.py \
  /absolute/path/to/campaign
```

Ctrl+C on that observer only closes the observer; Ctrl+C on the launching
command requests the owned calculation to save and stop.

The finished native command prints the symbolic artifact directory. Inspect
it without loading a giant table of individual rules:

```bash
/absolute/path/to/new/rustred master-inspect \
  --artifact /absolute/path/to/master-artifact --format table
```

`--format auto` selects a terminal table on a TTY and JSON otherwise;
`--format json` is suitable for scripts. The summary distinguishes installed
rule records, rules observed in conservative symbolic covers, current-scope
terminal keys and retained keys from earlier stages. The remaining basis is
allowed to be nonminimal. It is not a count of proved independent masters,
and the artifact contains symbolic relations rather than numerical values.

The package preserves original native programs, routing and exact terminal
substitutions for subsequent Vakint integration. The saved-campaign routed
engine currently records symbolic dependency traces, not general coefficient
back-substitution, so this package is not yet a turnkey numerical Vakint
reducer. This finite pass uses ordinary IBPs around the terminal keys and
keeps every unresolved auxiliary column; it does not drop uncovered terms.

## Implementation checks

The steering tests cover unchanged-scope resumption, explicit opt-in,
checkpoint/amendment binding, rank and power-difference widening, monotone
search deepening, frozen phase-two executable isolation, process ownership,
Ctrl+C/save/resume, and structured non-TTY output. Native mathematical and
checkpoint replay checks are separate from these Python protocol tests.

The phase-two table was visually reviewed at 80×24 and 150×32. The evidence
under `TMP/master-dashboard-20261007/` is explicitly synthetic telemetry
rendered through an ANSI terminal emulator, not a solver benchmark or evidence
of master reduction. It verifies alignment, colors and narrow-terminal
information priority without launching a production campaign.

## Measured saved-input controls (October 7, 2026)

Using the optimized release CLI, finite seed depth0, four pinned CPUs for
cold reinspection and one incremental exact reducer:

| Input | Raw terminals | Normalized | After finite IBPs | Wall time |
|---|---:|---:|---:|---|
| Combined four-loop58-query control | 28 | 20 | 19 | 6.563s |
| Five-loop scalar R=0,D≤9 | 196 | 196 | 196 | 78.872s to interruption +26.664s resumed |

The five-loop test sends a real SIGINT, saves695 completed ordinary source
rows and resumes to4,900 without repeating the initial cold inventory. This
validates continuation, not minimality: depth0 finds no further relation among
these196 candidates. The four-loop control finds one additional relation.
Compilation is excluded; initialization, cold inspection and package writing
are included. These single shared-host runs are not performance-comparison
claims. All output is scratch-local and the source checkpoint files remain
byte-identical. Evidence is under `TMP/master-reduction-20261007/`.
