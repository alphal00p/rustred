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
python -B examples/python/saved_campaign.py refine --campaign /path/to/campaign \
  --seed-depth 0 --saved-rule-assistance
python -B examples/python/saved_campaign.py refine --campaign /path/to/campaign \
  --seed-depth 0 --containing-sector-depth 1
python -B examples/python/saved_campaign.py refine --campaign /path/to/campaign \
  --seed-depth 0 --circuit-symmetry-assistance
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

`refine --containing-sector-depth 1` adds ordinary IBP source points by setting
one nonpositive index of each raw terminal directly to `+1`; depth `2` also
includes every pair of such promotions. This is a different finite source
selection from the signed-L1 `--seed-depth` neighborhood: a negative index can
jump several powers. All other indices stay unchanged. These extra sources
are not declared terminals, and their higher-sector columns remain in the
exact elimination. Ordinary source guards and seed/resource limits still
apply, with no mass or factorization assumption about the promoted slot.

Containing-sector depth defaults to `0` (disabled), is nonnegative and cannot
exceed the native family's arity. It is remembered only as a preference for
later explicit refinement and cannot decrease; inherited deeper work is
retained and reported at its effective depth. A changed depth selects its own
checkpoint. The low-level spelling is `--master-containing-sector-depth N`,
requiring `--refine-masters`; ordinary run, extension and publication never
activate this search. It can be combined with saved-rule assistance but does
not imply a smaller basis or a minimality proof.

`refine --circuit-symmetry-assistance` independently enables finite exact
equations from the native verified unit-circuit transformations. The supported
geometry is unshifted unit-mass vacuum integrals with full-rank `L+1` active
supports and unit primitive circuits. The native transport retains positive
dots and expands finite numerator powers, keeping every resulting auxiliary
term. Unsupported geometries are skipped; a terminal declaration is not an
equation. This option neither increases the ordinary seed depth nor enables
saved candidate equations. It can be combined with the other explicit search
options and does not establish a minimal-master or numerical-value claim.

`refine --saved-rule-assistance` opts into additional exact equations from the
portable package's applicable saved rules and verified weighted routes. The
ordinary IBP seed budget is unchanged. Both sides of each equation remain in
the sparse system, including unresolved integrals outside the starting rank
or power-difference bounds. A terminal declaration supplies no equation.
The saved rules retain their candidate authority: exact specialization does
not independently replay their source derivations. The resulting count is a
finite symbolic spanning list, with generic-parameter conditions; it is not a
claim of independent or numerical masters.

The default remains ordinary refinement. Once explicitly chosen, the mode is
remembered for subsequent explicit `refine` resumes. Use
`refine --no-saved-rule-assistance --no-circuit-symmetry-assistance` for an
ordinary-source comparison; each flag independently disables its provider.
Changing either mode
selects a distinct refinement checkpoint and starts a fresh finite search
when the source artifact used the other mode; it does not rerun the campaign
solver or modify the source package. A paused ordinary cursor is never resumed
as assisted work. The low-level launcher spells these options
`--master-saved-rule-assistance` and `--no-master-saved-rule-assistance`, both
requiring `--refine-masters`. Ordinary run, publication and extension never
start refinement merely because this preference was saved.

The circuit option follows the same explicit-only, independently remembered
boolean policy. Its low-level flags are `--master-circuit-symmetry-assistance`
and `--no-master-circuit-symmetry-assistance`. The four provider combinations
have distinct checkpoint bindings. Circuit-only refinement does not inherit
saved candidate-rule authority; combining providers preserves that authority
distinction. Already completed containing-sector and signed-L1 depths are
retained when a provider-mode change starts a new finite search.

Current experimental limitation: publishing a larger scope cannot yet import
an assisted refinement's native state. It reports an explicit error rather
than silently dropping those relations. The original publication and assisted
result remain intact; ordinary-refinement scope reuse is unchanged. Use
isolated native artifact refinement for comparisons until assisted
rebuild-only publication is available.

The current finite exact elimination uses one incremental Symbolica reducer.
The worker budget parallelizes cold scope reinspection and saved-route
verification. The dashboard reports measured CPU usage rather than implying all reserved
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
# Optional saved-rule assistance, in a separate output directory:
target/release/rustred walk-master-reduce --artifact /path/to/published-package \
  --directory /path/to/assisted-refinement --seed-depth 0 --saved-rule-assistance
# Optional verified circuit equations, independently of saved candidate rules:
target/release/rustred walk-master-reduce --artifact /path/to/published-package \
  --directory /path/to/circuit-refinement --seed-depth 0 --circuit-symmetry-assistance
```

Legacy ordinary native checkpoints remain readable. Assisted checkpoints use
the native version that also stores the fixed provider binding, pending
equations and their progress. Resume requires the same provider flags:
`--saved-rule-assistance` and/or `--circuit-symmetry-assistance` when selected.
The package supplies owners and
routes directly for saved-rule assistance, without a CP6 reload or scope
reinspection; circuit-only preparation does not require saved-rule dispatch.

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
Ctrl+C/save/resume, assistance-mode isolation and structured non-TTY output. Native mathematical and
checkpoint replay checks are separate from these Python protocol tests.

Publication and refinement tables were visually reviewed at 80×24 and 150×32.
The evidence (`publish-80.png`, `refine-150.png` and both alternate sizes)
under `TMP/master-dashboard-20261007/` is explicitly synthetic telemetry
rendered through an ANSI terminal emulator, not a solver benchmark or evidence
of master reduction. It verifies alignment, colors and narrow-terminal
information priority without launching a production campaign.

## Measured saved-input controls (October 7, 2026)

The public wrapper was also exercised with the merged optimized binary on a
small, finite four-loop bottom-sector control, pinned to one core. These are
workflow checks, not timings for the complete four-loop family:

| Workflow step | Wall time | Observed behavior |
|---|---:|---|
| Publish a copied completed R≤2,D≤5 checkpoint | 5.208s | 1 terminal, **0** new IBP rows |
| Explicitly refine that artifact | 1.171s | 16 source rows, still 1 terminal |
| Repeat ordinary run on the same scope | 0.651s | kept refined artifact; no solve/refine rerun |
| Fresh R≤1,D≤4 solve and publication | 6.288s | published automatically |
| Widen to R≤2,D≤5 and solve/publish | 5.771s | 3→11 domain records; 16 previous rows reused, **0** new rows |

Inspection took 0.035–0.048s. Between preparing the extension and finishing
its solve, inspection explicitly reported the previous published scope as
different from the new requested scope. Afterward it reported current
R≤2,D≤5 coverage. Original and copied checkpoint/input/amendment hashes in the
completed-copy gate remained unchanged. Evidence and commands are under
`TMP/saved-campaign-publication-control-20261007/` (`results.json`,
`final-validation.json`, `widen-final-validation.json`). These end-to-end times
include Python launch, input checks, cold inventory where needed, and
publication; compilation is excluded.

Separate native controls used the merged optimized CLI, finite seed depth0,
four pinned CPUs for cold reinspection and one incremental exact reducer:

| Input | Raw terminals | Normalized | After finite IBPs | Publication | Explicit refinement |
|---|---:|---:|---:|---:|---:|
| Combined four-loop58-query control | 28 | 20 | 19 | 5.900s | 0.330s |
| Five-loop scalar R=0,D≤9 | 196 | 196 | 196 | 75.235s | 1.974s to interruption +26.230s resumed |

Both publications generated **zero** ordinary source rows. The five-loop test
sends a real SIGINT, saves662 completed source rows and resumes from662 to4,900
without any cold inventory events during refinement. It retains196 candidates:
depth0 does not establish minimality. Four-loop refinement finds one additional
terminal-only relation. Copied packages cold-load/refine independently of CP6
in0.254s (four loops) and2.127s (five loops), preserving every count. Inspection
uses metadata only; the native subprocess/polling harness measured about0.026s.
The published source packages and source campaign checkpoint/input hashes stayed
unchanged. An existing unpublished four-loop checkpoint also passes read-only
`artifact-inspect` fallback with explicit `inventory_only_not_published` status.

Compilation is excluded; per-operation initialization and package writing are
included. These single shared-host controls are not performance-comparison
claims. Evidence and exact commands are under
`TMP/saved-lifecycle-20261007/release-controls/summary.json`; earlier controls
remain under `TMP/master-reduction-20261007/`. That first merged test build has SHA256
`c2f9bcee5cc2a5ca120478b41d998538d606b18b1008e20d830bdc063e723d82`.

After incorporating upstream capacity-search commit `4d9b0e40`, the delivered
`08e4a4df` source was rebuilt and both native controls repeated successfully
(`TMP/saved-lifecycle-20261007/final-controls/summary.json`). Four-loop publication
and refinement took5.904s and0.330s. Five-loop publication took75.779s; SIGINT
saved652 rows after1.970s and resumed from652 to4,900 in26.134s. Counts, portable
cold imports, zero-source publication and unchanged source hashes all pass.
The delivered optimized executable SHA256 is
`9e6c0e9f4c6aa9b51062c213a168fac5ae4c1d2a50454eeab2e06f4c337fb327`.
