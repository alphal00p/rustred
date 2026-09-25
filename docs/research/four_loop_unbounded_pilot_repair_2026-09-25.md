# Four-loop unrestricted pilot followed by bounded repair

## Result

The user requested a lightweight, **four-loop-only** test of using an
unrestricted pass's output to target the subsequent bounded coverage work.
An input-only prototype improves FG's directly measured complete workflow from
17.434 s to 12.377 s, including the fresh pilot and selection: about **29% less
wall time**. It still closes all 124 required roots and their descendants.
BMW, H and X also close with the mixed helpers, but their observed savings do
not pay for the pilot. This is a useful selective strategy, not a demonstrated
universal replacement for the previous setup.

There are **no Rust engine modifications, rebuilds, new IBPs, or new algebraic
primitives**. The experiment uses Python steering and existing native JSON
inputs/results. It does not alter production defaults, the five-loop input, or
the user's running five-loop campaign; no five-loop experiment was performed.

## What is actually reused

The prototype operates at owner-domain granularity:

1. Run all saved owners with unrestricted helpers, retaining the original
   physical requests. This pilot can finish with unresolved dispatch conditions.
2. Read its completed dependency graph's per-domain `descendant_closed` flags.
   Select only fully unrestricted Apply orthants that are **recursively closed**,
   not merely locally clear. Validate their zero lower bounds, unlimited upper
   bounds, unlimited rank, unconstrained power predicates, and local success.
3. For those owners, replace the baseline helper with an unrestricted helper.
   Everywhere else retain the previously successful bounded helper unchanged.
4. Keep every original `A<=19, R<=12, A-R>=7` physical query verbatim and in the
   same order. Run the native walker afresh and require complete descendant
   coverage with no frontiers or failures.

The pilot supplies **geometry-selection information**, not new rules or trusted
proofs. Stage B revalidates even the selected unrestricted helpers. No partial
checkpoint, certificate, or cached dispatch result is imported. This avoids an
engine/schema change and makes a wrong heuristic selection fail the ordinary
native gates rather than silently promoting authority.

This is conservative whole-owner repair, not a minimal traversal restricted to
the precise exceptional frontier faces. Selecting only closed owners is also
more conservative than retaining all locally successful owners. For example,
FG's pilot has 111 locally clear owners but only 60 recursively closed owners;
the prototype selects the latter. The other 64 keep their baseline R12 helpers.

| Family | Unrestricted helpers selected | Baseline helpers retained |
| --- | ---: | ---: |
| FG | 60 | 64 |
| BMW | 46 | 88 |
| H | 90 | 224 |
| X | 10 | 318 |

BMW retains the baseline's selective positive-power restrictions on unselected
helpers; a selected recursively closed owner may safely be *proposed* with both
A and R unlimited because the fresh second pass independently checks it.
Every proposal contains its previous helper and original physical request.
No descendant is clipped back to the entry bounds.

## Setup and timing boundaries

The baseline, sources, binary and worker settings are those of the
[three-way helper-bound comparison](four_loop_helper_bounds_2026-09-25.md):
release executable SHA-256
`32fdec098a57dd0c51aef71c01c260fb5cf7d0954b0992db08bb0f955aed358a`,
six workers, Ordered/H256, unchanged rules and owner order, no joint pruning
or subdivision, CPU affinity 64–69 / 72–77 / 80–85 for FG / BMW / H-X.
No elapsed/work cutoff is introduced. The existing memory/headroom guard does
not trigger. This remains a shared-host experiment without filesystem cache
flushing or statistical confidence claims.

First, standalone repair passes use the previous unrestricted output to plan
new helpers. Their accounting charges the pilot cost, even though that output
was already available. These segment sums are useful for rejecting strategies
that cannot recover the pilot cost; they are not fresh end-to-end timings.

Because FG showed a possible net gain, a second harness runs a fresh comparison
in baseline / pilot-repair / pilot-repair / baseline order. A single timer starts
before the first native subprocess and ends after the final native subprocess
exits. It includes the actual unrestricted pilot, reading its report, a fresh
selection interpreter and input write, both native loads, checkpoint/result and
package output, intervening checks, and shutdown. Post-run independent streaming
audits and compilation are excluded. Each repeat has a new directory and
fresh native processes, with no resumed state or prior selected-mask file.

## Directly timed FG workflow

| Measurement | Baseline | Fresh pilot + selection + repair |
| --- | ---: | ---: |
| Median whole workflow wall time | 17.434 s | 12.377 s |
| Median waited child CPU time | 43.671 s | 23.040 s |
| Maximum single-process peak RSS | 1.173 GB | 0.360 GB |
| Final-pass discovered domains | 98,643 | 16,093 |
| Final-pass native inspections | 98,627 | 16,093 |
| Final-pass dependency edges | 539,441 | 72,261 |
| Final recursively closed roots | 124/124 | 124/124 |
| Remaining work / frontiers / errors | 0 / 0 / 0 | 0 / 0 / 0 |

Both repeats reproduce the final graph counts and native application counters.
The repaired workflow additionally performs the 124 pilot inspections; the
16,093 count is explicitly **stage B only**, while time/CPU/RSS include both
stages. Pilot-repair whole times are 12.415 and 12.338 s. Selection, including
its Python interpreter, is approximately 0.05 s per fresh workflow.

The two standalone preliminary repair runs likewise close 16,093 domains,
with about 3.7 s traversal and 7.5 s whole-command time, but those stage-B-only
times must not be substituted for the complete workflow in a speed claim.

## Other families: pilot cost does not pay back

The table below reports complete successful baseline times and **reconstructed
sums** of separately measured pilot + selection + repair stages. BMW and H
use two repetitions; X uses one repair/baseline pair and the corresponding
first pilot. Selection processing is charged, but these segment sums exclude
its interpreter startup and tiny between-stage glue costs. Including those
would not turn the observed losses into wins. No fresh integrated pipeline
speedup is claimed for these three families.

| Family | Baseline whole (s) | Charged pilot + selection + repair (s) | Baseline / repair domains | All roots closed |
| --- | ---: | ---: | ---: | ---: |
| BMW | 38.210 | 45.205 | 157,999 / 154,924 | 134/134 |
| H | 18.478 | 25.642 | 24,346 / 9,033 | 314/314 |
| X, one repair pair | 39.715 | 59.007 | 46,865 / 44,526 | 328/328 |

These domain counts again exclude the extra initial pilot. BMW's repaired
traversal is 32.081 s versus 31.399 s baseline: fewer domains alone are not a
speedup. H's repaired traversal improves from 8.905 to 7.109 s, but its pilot
adds about 4.260 s traversal plus another load/output cycle. X's small reduction
from 29.358 to 28.803 s traversal cannot repay its pilot either. All required
descendants, including those outside the physical input envelope, remain.

## Interpretation and audit

The test supports the user's idea **for at least one measured family**, with
no engine development: global closure information identifies regions where
unrestricted helpers absorb many otherwise fragmented descendants. FG loses
about 84% of its final-pass domain records. The benefit depends on which costly
regions the pilot resolves, not simply the fraction of owners selected.

The current prototype does not establish that broader selection, true cached
inspection import, or exact frontier-only repair would be better. Those would
need their own dependency/guard accounting and performance tests. No such
implementation, five-loop pilot, or production restart is part of this task.
In particular, do not infer a five-loop speedup or termination guarantee: these
four-loop controls have no basis-changing Route jobs.

Independent agents audit both the methodology and results: exact selected-owner
sets, untouched original queries and ordering, helper-only widening, unchanged
saved inputs, native inspection/alias/partial-anchor accounting, root and total
dependency closure, and timing boundaries. Stage B succeeds everywhere with
zero errors/frontiers and every original request represented. The unrestricted
pilot's incomplete status is never promoted to a family-closure claim.

Workspace evidence:

- `TMP/unbounded-pilot-repair.RZjOT8/`: generic `prepare.py`, `pipeline.py`,
  FG input receipts, standalone trials, and fresh ABBA workflow measurements.
- `TMP/bmw-unbounded-repair.MgkNQ0/`: BMW selected input, two repair trials and
  `comparison.json` with explicit charged-stage accounting.
- `TMP/unbounded-repair-hx.Bq6ucr/`: H/X inputs, repair trials, refreshed H
  baseline, `results.json`, and scope/measurement report.

Example fresh four-loop FG reproduction, using the existing workspace inputs,
environment and license:

```sh
python TMP/unbounded-pilot-repair.RZjOT8/pipeline.py \
  --mode pilot-repair \
  --baseline-config TMP/bounded-helper-control.SsjeHf/fg-baseline.config.json \
  --pilot-config TMP/bounded-helper-control.SsjeHf/fg-unbounded.config.json \
  --helper-prefix rank12-anchor- \
  --directory TMP/unbounded-pilot-repair.RZjOT8/new-fg-workflow
```

The output directory must be fresh. `--mode baseline` uses the same harness
without the pilot or selection. These are local experimental steering scripts,
not a new production CLI option or a changed default.
