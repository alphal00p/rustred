# Ordering review for the first parallel delivery

The initial source review by `stage_a_release` has now been followed by actual
optimized, input-driven experiments by `bounded_ordering_pilots`. Root reviewed
the scope and interpretation. The original review is retained at
`TMP/codex-stage-a.2RU3AX/algebraic-input-order-review.md`; measured evidence is
under `TMP/codex-ordering-study.7nspU4/`.

## Recommendation

Retain the existing saved owner programs and helper-first query order for this
delivery. There is evidence that algebraic ordering changes both expression
size and exceptional geometry, but no completed, validated alternate five-loop
library qualifies a replacement. Test adaptive pending-job dispatch separately;
it cannot change the algebraic pivots in already generated programs.

## New optimized experiments — 2026-09-29

All experiments used the same campaign-profile executable from `e1bdb9e7`,
SHA256 `b5bd346cd9bfa925a4324031660cb3b2993e23c11f1e53765cb6758b87d9d95c`.
Changing inputs, coordinate order or dispatch did not require recompilation.

### Algebraic coordinate priority

The public generator was run on the fixed common-four-loop sub-root
`0111100110`, with nonpositive indices `0,5,6,9`, sparse backend and search
depth two. Natural and reversed coordinate priorities both completed exact
certification and cold loading. They cover the same 64 support sectors
(19 nonzero and 45 zero) and exactly the same 19 terminal keys. Neither arm
clips rank or adds terminals. This is a sub-root experiment, not proof about
the full five-loop library.

| Measurement | Natural | Reversed |
| --- | ---: | ---: |
| Solver core, seconds | 1.610 | 4.580 |
| Generation + certification + cold process wall, seconds | 29.614 | 42.618 |
| Generated rules | 1,746 | 1,315 |
| Unique coefficients | 3,754 | 11,350 |
| Candidate bytes | 1,821,243 | 7,365,398 |
| Certified artifact bytes | 36,226,562 | 40,383,636 |

One pair favors natural order: fewer reversed-order rules did **not** mean
less algebra or faster completion. It establishes neither an optimum nor a
general five-loop speedup. All six generation/certification/cold commands
completed without timeouts. `COORDINATE_RESULTS.md` and
`coordinate-results.json` retain exact commands, phase times, CPU, RSS and scope
comparisons. No production owner program was replaced.

### Initial query ordering and adaptive dispatch

The combined four-loop control retains all 58 required rows and 16 owner
programs. Reordering the same rows to put broad helpers first was unfavorable:
the unchanged-order run completed with 51,139 saved domains in 11.514 native
seconds; the helper-first experiment had already committed approximately
497,493 domains after 47 seconds. A cooperative save/stop was therefore
requested. It stopped after 168.296 native seconds with 1,670,241 admitted
domains and 10,783 pending. That arm is **censored**, not a completed timing
or a claim of failed mathematical closure. Its checkpoint is retained and the
second planned helper-first arm was not run. The existing five-loop query
layout is already helper-first; this result does not license rearranging it.

On a separate complete combined-control pair, rolling FIFO took 26.877 seconds
including independent cold verification, versus 30.068 seconds for adaptive
dispatch. Both passed the same complete closure check. This single pair shows
no adaptive benefit and leaves adaptive dispatch opt-in. It does not prove
FIFO best for every workload. Exact receipts: `COMBINED_RESULTS.md` and
`combined-results.json` in the same evidence directory.

## What is currently selected

- All 67 selected owner programs derive from the four natural-priority parent
  runs in `TMP/tide-r10-four-parent-batch.yGOAw6/`, plus the separately exported
  native3822 owner under the 31740 manifest. The manifests/protocol and that
  export's receipt specify natural coordinate priority. This is provenance
  evidence, not a fresh binary-record order census.
- The exact LC2 query file has 183 rows. Each owner's `owner-anchor-*` helper
  appears before its required rows. These names describe existing layout only:
  the explicit immutable role declaration, not a name substring, establishes
  the fresh campaign's 116 required queries and 67 auxiliary helpers.
- The reduction order compares aggregate sector/corner-distance/degree data
  before coordinate tie-breaks. Reversing coordinate ties alone cannot prevent
  a rule from exchanging decreased dot excess for increased numerator rank.
  See [rank-scoped owner reuse](rank_scoped_owner_reuse.md).

## Existing evidence and its limits

| Probe | Observed result | What it does not establish |
| --- | --- | --- |
| Four-loop H reverse priority | An early rejected sector improved from 60/63 replayed rules and seven uncovered boxes to 68/70 and zero boxes; a later point still failed. | No durable alternate complete library. |
| Four-loop FG reverse priority | Natural search 106 ms versus reverse 193 ms; the exceptional locus changed from nonlinear/infinite to one rational/integer point. Both unsupported. | Neither faster completion nor a successful physical-root campaign. |
| Five-loop reverse/half-rotation | Both reached their historical 900-second cap without an artifact. | Censored runs are not completed timing ratios. |
| Fixed integral order, 25 source rows permuted | RHS sizes 781/653/725; single-run wall 38.02/38.32/28.23 s. All retained the same exceptional equation list and failed on the same conic. | One lower timing does not remove the obstruction or demonstrate repeatable campaign benefit. |
| Four-loop helper-first layout | Existing FG rows preserved; initial native admissions 248 to 124 in the recorded combined ordering direction. | No unused helper-first gain in the current five-loop layout, which already does this. |

Sources: [four-loop ordering probe](../four_loop_ordering_probe.md),
[FG exceptional geometry](four_loop_fg_exceptional_geometry.md),
[TIDE exceptional geometry](tide_exceptional_geometry.md),
[combined four-loop campaign](four_loop_combined_run_2026-09-27.md), and local
receipts `TMP/root29751-ordering-portfolio.JEqDKU/RESULTS.md`,
`TMP/tide-source-order-pilots.BEebXm/RESULTS.md`,
`TMP/four-loop-helper-order.Iy9nQt/RESULTS.md`.

## A narrowly justified later experiment

Once the core parallel delivery is validated, a small algebraic prescreen could
compare identity versus half-rotation of the same 25 source rows on the existing
isolated case195 harness, keeping integral/coefficient order and target fixed.
Use two matched pairs, a fixed worker, exact replay and identical guards and
descendant scope. Charge preparation/serialization as well as search time.

Reject the candidate if it fails exact replay, merely moves the obstruction,
changes scope, or lacks repeatable end-to-end benefit. Even a successful
prescreen would require regenerated, cold-loaded affected owner artifacts and
matched closure controls before changing campaign inputs. No such experiment
or regeneration was performed or silently incorporated in this delivery.

Query-array order is a different lever: it changes admission/representatives
and the checkpoint binding but does not alter algebraic pivots. Adaptive
dispatch is different again: it changes which existing pending obligation runs
next, and must preserve fairness and replayable decisions. None confers closure
authority or licenses dropping a difficult obligation.

## Runtime controls and rebuild boundary

The public `family-candidates` and `family-close` commands already accept
`--input` and `--permutation N,N,...`, a zero-based coordinate-priority
permutation. `family-candidates` also selects its exact backend at runtime.
Different families, coordinate permutations and supported backends therefore
use the same compiled engine. Campaign worker budgets, query ordering and
FIFO/adaptive pending-job dispatch likewise do not need a Rust rebuild.

A different algebraic order does require generating and validating the affected
owner programs, rather than relabelling an existing artifact or mutating its
checkpoint binding. This is computation on new input, not compilation. A general
arbitrary source-row/pivot-policy control is not asserted here. The isolated
historical source-row harness is not evidence of such a public API. The upcoming
matched campaign matrix deliberately fixes the saved programs and executable
while varying the declared campaign policies; no generation is charged as a
hidden prerequisite of those comparisons.
