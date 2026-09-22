# Shared symbolic-domain trials — September 22, 2026

## Outcome

The six previously studied five-loop R10/R11 source/child boxes now match
automatically using unchanged saved rules: **40 selected pieces, zero gaps,
zero unresolved guards**. Ten exact inactive-coordinate faces replace the
previous manually split input. Positive tails and actual child ranks are kept.
This is local applicability, not complete five-loop closure.

The new selected-rule RHS visitor and shared symbolic worklist also run on real
saved programs. Their two bounded trials stop on operational allowances, with
explicit routing frontiers and no recorded local gaps or RHS-validity problems.
Neither trial finishes its first domain. The full-census local matching attempts
likewise stop within their first owner. **Stage (a)/(b) remains incomplete.**
No IBPs were regenerated and no numerical masters or five-loop Vakint features
were added in this milestone.

## Implementation and validation

- Optional bounded-inactive refinement retries the same undecided predicate on
  exact singleton faces. It never samples an unbounded positive direction or
  clips intermediate rank to the saved entry bound. Full split allowances are
  reserved before traversal; an insufficient optional budget leaves the
  original unresolved piece.
- `visit_owner_applied_successors` internally runs the ordered matcher on the
  same immutable owner programs. Native fixed specialization, original-term
  child validity and descent precede equal-shift coalescing. Conditional
  coefficients retain conservative successor requests. Edges remain provisional
  when later terms or an operational interruption leave local obligations.
- Exact translated box/rank images fix finite sign-changing coordinates when
  required. Existing geometry/descent and Symbolica algebra services are reused;
  no new polynomial or reconstruction kernel is introduced.
- `owner-domain-match --follow-successors`, the Rust application API and the
  Python steering wrapper share containing owner/box/rank domains in a single
  prepared snapshot. Pending inclusion prevents duplicate scheduling but does
  not count as completed reduction. Routing is still an explicit frontier in
  this initial worklist; its output is not a resumable queue or closing artifact.
- TTY progress overwrites a domain dashboard; non-TTY heartbeats remain compact.
  `--max-guard-univariate-degree` exposes an existing native algebra allowance,
  not a change to input numerator rank.

Release validation: **2,607 core tests passed**, zero failed, 32 pre-existing
ignored diagnostics; **307 application/integration tests passed**, zero failed;
**18 Python steering tests passed**. The focused core domain gate passes all
41 tests. Separate agents performed implementation, mathematical and runtime
reviews. One draft test-constructor compile error was corrected before the
successful gate. An interrupted preliminary application build is not counted
as a successful run.

## Measured local applicability

All rows are release CLI runs steered by Python, with one compute worker and
inner pools set to one. Timings exclude compilation and include no source
generation. Full-census rows use all 67 external owner queries at R10, with
unbounded positive powers; **none finishes the first query**. The three-owner
query row uses the unchanged six narrower R10/R11 inputs. Preparation includes
saved-program loading and the current shared preparer's route-map verification.

| Input / policy | Classified queries | Retained prefix | Matching | Preparation | Whole command | Peak RSS |
|---|---:|---|---:|---:|---:|---:|
| Full census, previous defaults | 0/67 | 1,687 selected pieces, 47 terminals, 7 unresolved | 0.604 s | 109.608 s | 113.90 s | 5,805,008 KiB |
| Six queries, automatic refinement 64 | 6/6 | 40 selected pieces; no unresolved/gap | 1.275 s | 3.325 s | 4.77 s | 511,532 KiB |
| Full census, degree 64 / refinement 4,096 | 0/67 | 11,922 selected pieces, 73 terminals | 18.728 s | 118.321 s | 140.58 s | 5,805,172 KiB |
| Full census, larger aggregate allowances | 0/67 | 11,974 selected pieces, 73 terminals | 9.018 s | 123.529 s | 136.14 s | 5,809,228 KiB |

The census stopping reasons, in order, are:

1. Native guard univariate degree **17 > 16**.
2. Predicate visits **1,000,001 > 1,000,000** after allowing degree 64.
3. Native separable-factor work estimate **339,738,624 > 64,000,000** after
   increasing aggregate matcher allowances. This last run admits 534 refinement
   faces over 114 splits and performs 1,011,821 predicate visits.

The last two retained prefixes contain no unresolved or gap pieces. That says
nothing about the unvisited remainder. The initial seven unresolved boxes have
fixed positive powers and only two unfixed inactive coordinates: their combined
263 integer points are finite under R10, not uncovered positive rays.

Independent comparison confirms that automatic refinement's first source has
exactly the previous unsplit zero-numerator piece plus the 27 manually generated
slice pieces; the other five query classifications are unchanged. The table is
not a controlled speedup comparison: policies, stopping points and input sizes
differ, and these are single shared-host observations.

## Measured successor traversal

Both trials load the same three saved owners with no routing maps. They use
CPU41, a 32 GiB address-space ceiling, 2,000 domain slots, 200,000 aggregate
events and two million containment comparisons, without an elapsed deadline.
The separate full-census match uses CPU40. No compilation overlaps these trials.

| Input | Successors | Scheduled / completed domains | Inclusion reuse | Routing frontiers | Traversal | Whole | Peak RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| Original six boxes | 15,485 | 1,695 / 0 | 1,256 | 12,539 | 1.685 s | 5.55 s | 512,660 KiB |
| Three full R11 owner orthants | 84,754 | 556 / 0 | 29,313 | 54,888 | 4.335 s | 8.70 s | 813,752 KiB |

The first trial stops at two million containment checks. Its first source
remains incomplete and the other five inputs have not been inspected. The
broader trial stops at the native applied **100,000 RHS-cell allowance**;
that counter includes ordinary one-cell RHS work, not just sign crossings.
The broadened input is a different, strictly larger workload, not a matched
timing comparison. It demonstrates reduced inclusion pressure but not closure.

All recorded frontiers require routing. The broader trial retains 378
conditional successors and rank-12/rank-13 frontier boxes; these are not clipped
to entry R10 or input R11. Conditional requests are over-covers, not proof that
every point is reached. Both trials report zero RHS-validity/local-gap problems
in their inspected prefixes only. Every incomplete command exits 4 and preserves
`family_closure_claim=false`.

## Next implementation decisions

1. Integrate shared routing at domain level. For admitted transport maps, the
   active denominator rows are a unit bijection and inactive rows are affine
   rational polynomials. A numerator of degree at most R therefore maps to
   degree at most R; resulting supports lie inside the mapped root. Full-root
   and strict-subsupport rank-R requests can safely over-cover endpoints without
   materializing the native numerator expansion. Preserve the actual incoming
   rank and existing Apply/Route phase and support descent. An over-cover gap
   must not be labelled an actually reached missing rule.
2. Address native guard work refusal using bounded-coordinate specialization
   where appropriate, rather than repeatedly raising expensive factorization
   budgets. Retain the original resource failure if no bounded refinement is
   possible, and report the active predicate/domain for profiling. This is
   ordered application/work discovery, not a new independent certification lane.
3. Make real campaign RHS allowances steerable and aggregate repeated routing
   requests. The exploratory orthant report is about 89 MB despite only 21
   distinct destination supports; its compact event stream is about 6 KB.
   The next shared routing queue should consume those requests directly.
4. Profile before adding speculative algebra optimizations. A conservative
   coefficient-classification lane could skip proving uniform nonvanishing,
   while keeping native exact zero checks, cancellation and child validity.
   It would need predecessor provenance or a targeted rerun to disambiguate
   conditional frontiers later. No such optimization is implemented here.
5. Retry the shared parametric campaign after the stable slice, within the
   50-core/500-GB envelope. Do not replace it with a finite positive-dot sweep.
   Minimization, numerical masters and five-loop Vakint remain sequentially
   deferred until the R10 solve succeeds.

Reproducible local evidence is under `TMP/shared-symbolic-release.tMMfyA/`,
`TMP/three-owner-symbolic-walk.HbnkP1/` and
`TMP/three-owner-orthant-walk.yO9nbq/`. The frozen CLI is
`79cd3eee96f7a7ddbdde3967199d5730c292730330470a47d77f7b5be9488251`.
Raw temporary evidence and reference-only material are not shipped.
