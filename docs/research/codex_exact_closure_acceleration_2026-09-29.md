# Exact closure acceleration: source/literature research receipt

Date: 2026-09-29. Author: `closure_acceleration_research`.
Status: **one feasible opportunity test; no engine implementation or speedup
recommendation**. Generic WQO/recurrence acceleration is deferred. Production,
pilots, checkpoints and inputs were untouched; no builds, native execution,
large evidence scans or Git operations were performed.

The target remains exact scoped closure of every one of the frozen 116 required
queries, including all reachable obligations. This is not terminal minimization
or an unrestricted five-loop theorem. Read completely: `CODEX_PROGRESS_PLAN.md`,
`CODEX_PROGRESS.md` (1,104-line snapshot), and
`docs/research/codex_next_candidates_2026-09-29.md` (409-line snapshot).

## Decision

The most concrete remaining traversal-reduction question is **a single tighter
G2 residual using coordinate and A/R bounds as well as D**. Current G2 only
trims fully covered D levels at the two ends. A fixed-D domain can have a large
already inspected coordinate-side portion and still receive no G2 loan. The
existing finite-point enumerator and independent union checker provide an exact
authority route for a one-piece residual that excludes such a portion.

There is no measured incidence or benefit for that extension. Recommend a
small, bounded shadow-planner opportunity test after the running measurement
lane releases resources; defer runtime integration until a positive result.
This is a stronger surviving question than the two other G2 extensions examined:

- **Symbolic fallback for enumeration limits: rejected for the observed Ready
  finite control.** Every relevant limit-failure counter is zero.
- **Multiple residual D bands: deferred on prior negative opportunity evidence.**
  The historical census found a one-piece D residual in 2,284/2,298 partials.

The literature does not provide a justified plug-in theorem that makes the
current campaign a terminating WSTS, a translation quotient, or a closed
inductive envelope. Those approaches need additional mathematical evidence
that is absent from the current records and cannot be inferred from zero
frontiers or positive queue throughput.

## Evidence examined and its limits

The root supplied the path to the existing 6.6KB Ready metrics; I read it
independently:
`TMP/codex-g2-pilot-prep.n7Kd5q/ready-pairs/deployment-ready-candidate-union-r1/five-finite/metrics.json`.
This is one finite control, not the 116-query production scope or a reproduced
deployment result.

| Ready Union r1 counter | Observed value |
|---|---:|
| Scheduled domains | 964,909 |
| Native inspections | 758,183 |
| G2 planning attempts (`planned_jobs`) | 194,530 |
| Full G2 covers / residual G2 records | 50,510 / 30,088 |
| Whole: no candidates | 79,587 |
| Whole: no covered D level | 34,345 |
| Whole: unbounded / over point cap / over test budget | 0 / 0 / 0 |
| Whole: unrepresentable / authority refused / empty | 0 / 0 / 0 |
| Enumerated query points | 6,912,612 |
| G2 membership tests | 28,052,730 |
| G2 planning seconds | 5.630466 |
| Traversal seconds / whole-command seconds | 79.936175 / 180.746 |

The 34,345 no-covered-level cases are a potential cohort, **not** a count of
coordinate-side opportunities. They may have no covered points at all. The
5.63 seconds are summed planner work; do not treat their ratio to traversal as
removable wall time. Existing aggregate metrics do not record all uncovered
point shapes, actual candidate sets, or per-job Ready dispatch snapshots.

Also read the small existing
`TMP/codex-integration/post_g2_profile_priority_2026-09-29.md`.
Coordinator/helper cost attribution remains the strongest evidence-backed
near-term profiling priority. The research below does not displace it.

## Primary literature and what it actually permits

1. Doyen and Raskin, *Antichains for the Automata-Based Approach to
   Model-Checking* (2009), especially section 3 and Lemma 3.3: compact antichain
   fixed points require a simulation preorder that preserves the relevant
   acceptance/transition operators. Compactness alone does not guarantee cheap
   operations. RustRed already uses exact containment to avoid duplicate
   obligations; a larger quotient requires its own simulation proof. The
   candidate below is an engineering application of exact set coverage, not a
   claim that their automata theorem proves IBP closure.
   [Primary paper](https://arxiv.org/pdf/0902.3958).
2. Finkel, *From Well Structured Transition Systems to Program Verification*
   (2020), sections 2–3: a WQO must be compatible with transitions, and effective
   procedures require further hypotheses. The author explicitly identifies
   zero tests as an obstacle to monotonicity of counter machines. That obstacle
   has a direct analogue in RustRed's index-dependent guards and poles; removing
   them to obtain a monotone abstraction is outside this task.
   [Primary paper](https://arxiv.org/pdf/2008.02929).
3. Frohn, *A Calculus for Modular Loop Acceleration* (2020), Theorems 1–3 and
   Definitions 3–4: exact acceleration of a repeated update requires guard
   implications throughout its iterations. The paper carefully distinguishes
   exact formulas from under-approximations. A closed form for `n -> n+s` alone
   does not permit skipping exceptional intermediate states, nor does
   accelerating one branch discharge all the other RHS terms of a recurrence.
   [Primary paper](https://arxiv.org/pdf/2001.01516).

These support restrictions on a proposed method. None supplies evidence of a
speedup or a valid WQO/acceleration instantiation for this workload.

## Candidate for a bounded opportunity test: one exact tightened residual

### Mechanism and source seam

In `crates/rustred-app/src/application/routed_campaign/walking/g2.rs`,
`plan_inner` (around lines 394–573) enumerates Q, obtains same-owner eligible
anchors before the snapshot, then tests complete D levels only from the top and
bottom. `Plan` (around line 110) stores just one `(lo, hi)` residual D interval;
`residual_domain` keeps Q's coordinate/rank/A bounds unchanged.

Let P be the finite lattice points of Q under the existing enumerator and
point cap. Classify points against a bounded subset of valid older anchor
scopes, using the existing membership tests and native point-containment
authority. Let U contain every point not proved covered by those scopes. Form
one residual R by intersecting Q with the following extrema of U:

- coordinate minima and maxima;
- maximum R and maximum A;
- minimum and maximum D.

These are all predicates already expressible by `Domain`. If U is empty, the
result is a full cover. Otherwise, `U subset R subset Q`, hence
`Q subset R union anchors`. Points included in R need not be uncovered: covered
points inside its bounding constraints are conservatively reinspected. This is
an exact cover certificate with a possibly nonminimal residual; it makes no
claim that R equals the exact set difference.

Only points actually omitted from R need borrowing authority. Untested or
inconclusive points must remain in U, or the attempt must fall back to the
unchanged planner/whole inspection. Never omit a point because a membership or
resource test did not finish. Retain the present whole-domain fallback on
enumeration failure. The first test should stop on any extra budget exhaustion
rather than invent new runtime fallback semantics.

There remains one native residual inspection. Successful residual plus older
anchor scopes can still lend the whole original Q under the existing
publication-order argument. Whole anchor dependencies remain intact.

### Small constructive example (not workload evidence)

Take owner `(true,false)`, local coordinates `0 <= x,y <= 4`, and exact D=1.
Physical indices are `(x+1,-y)`, so D=`x+1-y`; Q consists of the five points
`(0,0),...,(4,4)`. An earlier valid anchor contains the first two points.
The current planner cannot remove the only D level and inspects all five.
The proposed U is `(2,2),(3,3),(4,4)` and R has lower coordinates `(2,2)`,
upper `(4,4)`, retaining D=1. It contains exactly those three points. No input
point is added, no guard is inferred, and the two omitted points retain their
anchor dependency.

This demonstrates a representable opportunity not addressed by D trimming.
It does not establish that such anchors occur at actual campaign dispatches.

### Exact authority and lifecycle obligations

- Preserve owner, phase, immutable program/request identity, initial-plan
  precedence, anchor eligibility, quarantine, and the strict older-snapshot
  rule. Locally inspected anchors remain recursively open dependencies.
- Validate `R subset Q` and `Q subset R union anchors` independently. The
  existing offline `verify_closure/lattice.rs::covered_by_union` (lines 17–22,
  162–215 and 462) is an exact bounded integer-set checker for these predicates.
  Keep native `DomainPowerSummary` point-containment authority in the planner
  and this separate checker offline; do not share away that independence.
- Inspect R through the ordinary native Apply path. Its original poles,
  guards, source/child validity, descent, zero terms, cancellations and every
  successor remain obligations. Full cover only borrows already checked scope.
- Persist the exact residual domain and deterministic planning decision. The
  present G2 row/record/pin schema carries one D pair and explicitly says
  `coordinates_and_rank_unchanged`; it cannot truthfully encode this extension.
  Fresh opt-in schema, replay, rescue, verifier and mutation work is real cost.
  Do not silently reinterpret old G2 records or old point counters.
- A narrower source may specialize coefficients differently and produce a
  different successor partition. Mathematical equivalence and dependency
  coverage are the required comparison, not bit identity with the changed arm.
  Preserve exact old-mode behavior.

Relevant source: `walking/inspection.rs` (ordinary event forwarding),
`walking/execution/g2.rs` (pins, publication, eligibility, record geometry), and
core `owners/domains/applied/engine.rs` lines 476–763 (term restriction,
source conditions, descent, cancellations, exact translated A/R/D images).
No algebra primitive needs to be added for this candidate.

### Smallest positive-opportunity test and falsifiers

First construct a **bounded shadow planner** for a small sampled set of actual
post-G2 dispatches in the finite control and, only if useful, the hot control.
Capture Q, its actual snapshot, all examined candidate scopes and eligibility,
the old plan, and a bounded exact membership trace. Prefer a small fixed number
of no-covered-level and residual cases selected by immutable ID, with a strict
total-work budget. No live production tracing is proposed here. A hypothetical
planner over all final anchors is invalid: some anchors were unavailable at
dispatch. A publication-window proxy is not equivalent to that snapshot.

Require independently checked positive examples where the new R is a strict
subset of the old residual, charging all extra membership work and retained
anchor links. Report saved points and source geometry separately from estimated
native work. For a tiny positive subset, ordinary native reinspection of old
versus new residuals can establish the additional term/boundary/successor work
avoided; this is a later resource-coordinated test, not executed research.

Falsify/park if the bounded representative sample finds no meaningful stricter
residuals, if extra membership cost dominates, if success requires many
additional dependencies, or if narrower native calls create enough splitting
to cancel the reduction. Mutation tests must remove one required anchor,
shrink R past one uncovered point, alter a coordinate/rank/A bound, introduce a
late/quarantined anchor, and interrupt/resume an accepted prefix.

Complexity: P points, C examined candidates, dimension N. Worst-case membership
work remains O(P C N), with explicit existing-style caps; calculating extrema
is O(P N). Extra stored point assignments are O(P), plus the existing candidate
list. Unlike the current edge-level early exits, this can examine many more
points/candidates. Runtime benefit must be demonstrated, not inferred from
the set identity. The small shadow test is justified; engine integration is
deferred pending that evidence and independent review.

### Distinction from retained negatives

This never enlarges Q, merges pending domains, modifies an admitted ID, or
changes the frozen helper/input geometries. It therefore does not revive G1
widening/dense cells, failed bounded helpers, or the zero-opportunity pending
D-band census. It preserves one residual call, unlike fine piece certification
that previously cost 1.57x. It retains complete broad-anchor dependencies and
does not require the missing successful source-to-target transcript maps. It
does not turn the negative closed-descendant census into a positive result.

## Other mechanisms: explicit disposition

**Symbolic full-union fallback:** geometrically feasible using bounded D/axis
splits and native single-container inclusion at leaves, with exact union
verification. It could avoid enumerating a large domain, but the current Ready
control has zero `unbounded`, `over_point_cap` and `over_test_budget` failures.
Since the existing exhaustive level checker already detects finite full union
covers within those limits, there is no additional full-cover opportunity here.
Defer until another representative, unchanged-scope control shows actual
capacity failures with valid earlier cover witnesses. Avoid a general
admission-time union service or serial historical scan.

**Multiple residual D bands:** source permits only one piece by design. The
historical note `docs/research/fable51_w1_g2prod_2026-09-28.md` lines 28–40
records one exact D residual piece in 2,284/2,298 gen-7 partials. That is sparse
historical opportunity, not post-G2 evidence. More pieces also multiply native
setup, persistence and replay costs. Defer unless actual post-G2 snapshots show
a materially different distribution.

**WQO/Karp–Miller-style acceleration:** no sound instantiation established.
Finite owner masks and Dickson's lemma on integer vectors do not make all box
domains WQO under containment: distinct singleton domains form an infinite
antichain. The physically meaningful guards/poles may change at a translated
index. A simulation compatible with all transitions and bad/frontier states,
plus effective exact operations, must be proved before a new order can prune
work. No guard-dropping abstraction or omega/hull widening is acceptable here.

**Translation or recurrence induction:** defer, not a ready second candidate.
For a shift `n -> n+s`, geometric translation is simple but a safe macro must
account for every RHS exit at every intermediate index and every guard/pole
exception. Rational coefficients generally depend on indices; translated
boxes with the same shape are not interchangeable inspection results.
Current successful records omit source partitions/maps, so such a macro
cannot be authenticated by grouping those records.

A smallest legitimate reopening test would identify one actual sign-stable
same-owner recurrence, an exactly representable family of source domains,
a checked decreasing natural-number parameter, a finite base, and **all**
side exits already covered by exact recursively closed domains. Reinspect the
single symbolic family natively, retaining original conditions and masks.
Only after this certificate skips a real chain with lower total proof work
than ordinary G2 is it worth designing a runtime form. If it needs a new
parametric ray/polyhedral/SMT representation, if a shifted guard exception is
unresolved, or if side exits reproduce the same open cone, stop this avenue.

The existing core is not missing the elementary induction observation:
`foundry/artifact/source_port/total_excess.rs` lines 92–99 and 452–506 already
checks complete sector programs with inductive degree envelopes. Same-sector
SpiReD descent bounds total excess; cross-sector degree propagation is
separately checked. Its complete-census source-port artifact contract is not
the current routed owner campaign's contract. Reusing it requires an actual
checked smaller envelope, not treating that diagnostic as closure or silently
replacing A/R/D scope with a wider simplex. `scope/contract/envelope.rs`
expressly requires replay, guards, descent, coverage and RHS closure.

## Pinned and public Symbolica audit for the deferred translation test

No new CAS facility is needed merely to test a proposed translation identity.
Workspace `Cargo.toml` pins `=3.0.0` and patches to `vendor/symbolica`;
the progress receipt identifies gitlink `ef0db494` (not rechecked through Git
in this research lane). The pinned source exposes
`MultivariatePolynomial::shift_var` at `vendor/symbolica/src/poly/polynomial.rs:2893`.
RustRed's existing `algebra/indexed/translation.rs:99` public
`translate_polynomial` and line 116 sealed variant validate context/arity,
preflight growth, and execute native `shift_var` at line 286. The coefficient
translation retains numerator/denominator and checked normalization.
Current public Rustdoc also shows Symbolica 3.0.0 and the same shift operation:
[Symbolica polynomial API](https://docs.rs/symbolica/latest/symbolica/poly/polynomial/struct.MultivariatePolynomial.html#method.shift_var).

An exact identity `p(n+s)=p(n)` is a sufficient translation-invariance test for
that individual polynomial. It is not a proof of all rule-selection predicates,
of guard monotonicity on a region, or of universal recurrence closure. The API
availability removes an algebra implementation obstacle; it supplies none of
the missing workload opportunity or proof obligations. No symbolic sampling,
new finite-field kernel, quantifier eliminator or external CAS is proposed.

## Handoff

Recommend independent critique of the one-piece residual proof and its schema
cost, then register at most the bounded shadow opportunity test. Keep all
implementation deferred until real post-G2 cases survive its falsifier. Retain
the measured zero-opportunity symbolic-cap result and historical sparse
multi-band result. There is still no basis for a production termination ETA,
an automatic campaign switch, or a claim that the remaining millions of
domains can be replaced by a generic WQO/induction theorem.
