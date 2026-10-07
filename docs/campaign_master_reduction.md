# Saved campaigns: solve, inspect, and optionally refine

A successful saved-owner campaign now publishes a portable symbolic artifact
automatically, without generating extra master-reduction relation rows.
Master refinement happens only when explicitly requested. All algebra, cold
verification, binary payloads and checkpoints live in RustRed; Python is a
thin launcher and dashboard.

The common commands perform actual work unless `--dry-run` is supplied:

```bash
python -B examples/python/saved_campaign.py run --campaign /path/to/campaign
python -B examples/python/saved_campaign.py inspect --campaign /path/to/campaign
python -B examples/python/saved_campaign.py extend --campaign /path/to/campaign \
  --rank 1 --max-power-difference 10
python -B examples/python/saved_campaign.py refine --campaign /path/to/campaign \
  --seed-depth 0
```

Inputs must already be staged. `run` automatically resumes an existing
checkpoint. Extra run options go to the production launcher, for example
`--workers 32` for fresh steering or a RAM override for this invocation.
The wrapper defaults to the current `target/release/rustred` for publication,
inspection and refinement. `--executable /path/to/rustred` selects another
current build; the existing phase-one binary stays frozen.

Solve, publication and refinement are distinct milestones. A drained queue
is admission to a native cold coverage check. Publication packages the
checked scope, native programs and terminal aliases as `published_unrefined`,
without generating ordinary-IBP source rows. Optional refinement searches a
finite relation set and ends as `completed_nonminimal`: this is neither
master independence nor a globally minimal basis. No numerical values are
computed.

## Start on an existing completed campaign

Use the current wrapper, not an older frozen Python snapshot:

```bash
python -B examples/python/saved_campaign.py publish \
  --campaign /absolute/path/to/campaign --executable /absolute/path/to/new/rustred
```

This performs missing publication for an already completed scope, without
rerunning its matching solve. `publish` refuses an incomplete scope rather
than starting a solve. `refine` requires a published artifact for the current
scope and likewise never falls back to solving. The publisher/refiner is
frozen separately under `master-reduction/`; original `bin/`, CP6 and inputs
remain unchanged. Explicitly choosing a replacement build preserves old
binaries and checkpoints; native Rust validates resumed state.

The low-level launcher retains `--publish-only` and explicit
`--refine-masters` (`--masters-only`/`--master-reduction` aliases).
`--master-reduction-executable` selects its independent native binary.
Historical persisted `enabled` policies no longer authorize automatic
refinement: ordinary run/extend always publishes only.

Explicit refinement's default `--seed-depth 0` searches ordinary IBPs at the
finite seed set; it is real work, not a skip setting. A later explicit
increase, for example `--seed-depth 1`, creates
a new phase-two search scope and passes the prior completed artifact to Rust
for validated reuse of its exact rows. Depth cannot silently decrease. The
seed depth is **not** the input numerator-rank cap.

The current finite exact elimination uses one incremental Symbolica reducer.
The worker budget parallelizes cold scope reinspection, not that row stream;
the dashboard reports measured CPU usage rather than implying all reserved
workers are busy throughout post-processing.

## Stop, resume and enlarge the input scope

Press Ctrl+C once and wait for the owned native process to save and exit.
During publication or refinement the launcher prints its separate checkpoint
directory and an operation-specific resume command:

```bash
python -B examples/python/saved_campaign.py run --campaign /absolute/path/to/campaign
# To resume a manually requested refinement instead:
python -B examples/python/saved_campaign.py refine --campaign /absolute/path/to/campaign
```

If the original CP6 manifest, amendment chain and source documents are
unchanged, the selected operation resumes its own checkpoint. An ordinary
`run` never silently resumes a paused manual refinement. Cooperative Ctrl+C finishes the current
atomic row before saving; an abrupt failure may require repeating work since
the last durable checkpoint. The phase-one worklist is not rerun just to resume
phase two.

Rank or power-difference extensions change the requested scope. The current
`extend` helper always uses publisher-aware steering rather than an older
frozen Python snapshot. Phase one discharges new obligations, then publication
gets a new scope directory. Refinement does not run automatically. Native
Rust may reuse exact rows from a prior completed artifact only after
authenticating its family and rule-program binding. This is reuse of proven
relations, not a claim that the old scope already covered the new one. An
earlier paused refinement remains saved but is not automatically imported by
an extension: do not count its partial rows as reused. The original domain
ledger remains reusable independently.

Every completed scope has an inspectable artifact. The atomic relative
`artifacts/latest.json` pointer selects the latest completed package; earlier
packages stay available. A failed/paused refinement does not replace it.
An unchanged default run preserves an already refined artifact. Extending R/D
does not make the old package cover the new scope; inspection labels that
distinction. `--max-power-difference` bounds D=A−R (A is the sum of positive
propagator powers); omission preserves the current cap, and
`--unbounded-power-difference` removes only the extra stage cap, not original
per-query restrictions.

The phase dispatcher and native processes retain the configured CPU affinity,
worker budget, checkpoint interval, RAM ceiling and cooperative RAM stop.
There is no additional solve-time deadline. A hard memory emergency can still
require killing the owned process; only its previously published durable
checkpoint is then resumable.

## Monitoring and artifact inspection

TTY output is an overwriting colored table. Publication shows scoped coverage
and packaging, with refinement explicitly not requested. Manual refinement
reports source and normalized terminal counts, remaining basis size, relation work, sparse-system counters,
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
python -B examples/python/saved_campaign.py inspect --campaign /absolute/path/to/campaign
/absolute/path/to/new/rustred artifact-inspect \
  --artifact /absolute/path/to/published-artifact --format table
```

`--format auto` selects a terminal table on a TTY and JSON otherwise;
`--format json` is suitable for scripts. The summary distinguishes installed
rule records, rules observed in conservative symbolic covers, current-scope
terminal keys and retained keys from earlier stages. The remaining basis is
allowed to be nonminimal. It is not a count of proved independent masters,
and the artifact contains symbolic relations rather than numerical values.
The complete package directory is portable; campaign restart checkpoints are
separate. Native-only manual refinement of a copied package is also possible:

```bash
target/release/rustred walk-master-reduce --artifact /path/to/published-package \
  --directory /path/to/new-refinement --seed-depth 0
```

The package preserves original native programs, routing and exact terminal
substitutions for subsequent Vakint integration. The saved-campaign routed
engine currently records symbolic dependency traces, not general coefficient
back-substitution, so this package is not yet a turnkey numerical Vakint
reducer. This finite pass uses ordinary IBPs around the terminal keys and
keeps every unresolved auxiliary column; it does not drop uncovered terms.

## Implementation checks

The steering tests cover default publication, explicit-only refinement,
checkpoint/amendment binding, rank and power-difference widening, monotone
search deepening, frozen phase-two executable isolation, process ownership,
Ctrl+C/save/resume, and structured non-TTY output. Native mathematical and
checkpoint replay checks are separate from these Python protocol tests.

Publication and refinement tables were visually reviewed at 80×24 and 150×32.
The evidence (`publish-80.png`, `refine-150.png` and both alternate sizes)
under `TMP/master-dashboard-20261007/` is explicitly synthetic telemetry
rendered through an ANSI terminal emulator, not a solver benchmark or evidence
of master reduction. It verifies alignment, colors and narrow-terminal
information priority without launching a production campaign.

## Measured saved-input controls (October 7, 2026)

These earlier explicit-refinement controls predate the publication/refinement
UX split. Using the optimized release CLI, finite seed depth0, four pinned CPUs for
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
