# Saved-owner guard obstruction triage, 2026-09-22

The measured local matcher stopped on avoidable guard checks as well as real
diagonal-domain representation limits. It did **not** identify a missing IBP
rule. The next small change should try cheaper native base-coefficient equations
first; an exact affine sign-on-box precheck is the next useful, priority-preserving
slice. Neither requires new source generation or a general certification engine.

## Evidence and scope

Inputs are the completed local run
`TMP/shared-domain-routing-controls.RWWK6m/full67.json` and its saved final
denominator request. The run processed 67 owners, exactly classified 59,
selected 213,523 rule pieces and 893 declared-terminal pieces, and retained 163
unknown pieces plus a separate native factor-work refusal. It reported no exact
gap. Those are local ordered-dispatch results, not recursive family closure.

The bounded inspection in `TMP/sufficient-rule-guard-triage.DOvAoo/` loaded the
eight affected saved owners once (937,006,184 encoded bytes), preserved all 164
original requested cells and inspected 190 native atoms. `result.json` records
every original/fixed-specialized expression, actual box and rank. No expression
text was truncated. Whole helper time was 11.44 s wall, 8.47 s user, 2.87 s
system, peak RSS 4,272,736 KiB; native load was 9.425 s. This was not a solver
benchmark or new rule generation.

`native-analysis.json` independently reparses those complete native-printed
polynomials with Symbolica. It checks total monomial degrees, uses native Integer
arithmetic for affine box bounds, and natively splits/checks/factors the one
cheap denominator coefficient discussed below. Runtime was 0.21 s wall, 0.01 s
user, 0.19 s system, peak RSS 3,072 KiB. Both helpers exited 0; standalone
compilation times are separate. No workspace build or production edit was made.

## What the 163 unknown pieces actually contain

| First unresolved predicate | Pieces | Finding |
|---|---:|---|
| Singleton excluded conjunction | 133 | All affine: 82 sign-definite on the recorded box, 51 genuine diagonal zero loci |
| Multi-atom excluded conjunction | 14 | Five two-atom, seven three-atom, one four-atom, one five-atom; first atoms genuinely nonlinear |
| Equality | 16 | Affine, with genuine diagonal zero loci |

All 147 excluded-conjunction stops occurred at atom ordinal zero. They represent
26 distinct excluded branches; the equalities represent four distinct predicates.
The native total-degree check confirms 149 affine first obstructions, rather
than inferring affine structure merely from per-variable degree one. Nonlinear
first atoms have total degree up to five. Every axis occurring in the 149 affine
atoms is positive and unbounded in its recorded cell.

The 77 pieces for owner `010111011000001` have guards `c+n1+n3`, for
`c=-2,-1,0,...,8`, with multiplicities `2,3,4,...,12`. Every one has **n1 >= 4
and n3 >= 2**. Their exact lower bounds are therefore `4,5,6,...,14`, with the
same multiplicities. In particular, the `c=-2` claim does not incorrectly use
the generic positive corner `(1,1)`, where this guard would vanish.

Another five pieces, for owner `011011000111111`, have
`-3+n10+2*n9`, with both involved indices at least two: lower bound three.
Thus a tiny affine interval test disproves the zero locus of 82 currently
obstructing predicates. It does not yet establish every other guard of those
rules or complete their queries.

The other 51 singleton exclusions and 16 equalities have mixed-sign affine
slopes and attainable integral diagonal zeros. Examples include `n10-n11`,
`1+n10-n11`, and `5+3*n1-4*n5`. Native integer gcd divisibility and the opposing
unbounded slopes confirm integral zeros above the recorded lower bounds; the
rank constraint involves inactive axes, not these positive tails. These 67
cases are genuine limitations of a coordinate-box-only partition, not evidence
that the saved candidate lacks the required diagonal branch.

## A small AND shortcut, not a wholesale solution

An excluded branch means that **all** its atoms vanish. One atom proven nonzero
everywhere on the current cell disproves that entire conjunction, independently
of the earlier unknown atom. This preserves the same rule and its priority.
Only 14 of the 147 recorded conjunctions have another atom to inspect.

Five of those 14 already have a uniformly nonzero later affine atom:

| Inspection request ordinal | Later atom | Recorded bound | Consequence |
|---:|---|---|---|
| 3 | `-46+21*n2+4*n1` | n1 >= 4, n2 >= 3 | >= 33 |
| 4 | `-6+n2+7*n1` | n1 >= 4, n2 >= 3 | >= 25 |
| 149, 150 | `3-n2` | n2 >= 4 | <= -1 |
| 162 | `329+48*n1` | n1 >= 6 | >= 617 |

The last three can already be resolved individually by the existing univariate
lane; the matcher simply never reaches them after its first Unknown. Together
with the 82 singleton cases, these give **87 currently obstructing predicates**
that small sufficient nonvanishing checks can discharge. This is not a measured
count of newly completed queries. Other branches retain actual zero faces:
for example, later atoms `5-n2` and `n1-6` intersect their recorded boxes.

## Native cost ordering can avoid the separate factor-work refusal

The final request is owner `011101110111000`, batch 0, rule 534, original RHS
denominator 1. Its 169-term denominator involves n1 and n2, with n1 >= 6 and
n2 >= 3. Symbolica's native base-variable split and native polynomial equality
confirm that its d^6 coefficient is exactly

```
58752 * (n1 - 1) * (n2 - 1) * (n2 - 2).
```

Native factorization independently returns those three linear factors and the
constant. None of their root hyperplanes intersects the requested box. Hence
this one coefficient proves the full denominator is not identically zero as a
polynomial in the generic base parameter d. This does not assert nonvanishing
at every numerical specialization of d.

The current base-monomial storage order visits d^0 through d^6. Their term counts
are `44,38,29,24,17,11,6`, and n1/n2 degree pairs are
`(5,8),(5,7),(4,6),(4,5),(3,4),(2,3),(1,2)`.
The existing prospective work formula charges 47,775,744 for d^0, then requests
another 18,874,368 for d^1: cumulative 66,650,112 exceeds the unchanged 64M cap.
It therefore refuses before reaching the cheap witness. The same formula charges
only 576 for the d^6 coefficient. This is a static accounting calculation, not a
measured rerun of a changed matcher.

The minimal implementation is a bounded borrowed-equation ordering by generic
structural cost, with deterministic ties. Keep context/payload admission and the
native factorizer, actual work charges, replay checks and errors unchanged; do
not identify a particular dimension variable or topology. Do not eagerly turn
an expensive unused equation's cost estimate into an error before trying the
cheap equation. Selecting one equation is sufficient because its zero locus
contains the simultaneous zero locus of the full coefficient system.

## Native APIs and the next narrow affine slice

Relevant existing implementation seams:

- `algebra/indexed/base_coefficients.rs:270` already uses Symbolica's
  `to_multivariate_polynomial_list` to extract integer index-polynomial equations;
  `:347` sorts storage by base monomial, and `:713` factors in that order.
  `:1054` owns the current structural factor-work estimate.
- `owners/domains/matching/guards.rs:41` already performs bounded native fixed
  specialization and coefficient splitting before zero-locus resolution. It is
  the appropriate domain-aware nonvanishing precheck seam.
- Symbolica `poly/polynomial.rs:1198` exposes borrowed exponent rows and its
  coefficient vector is public; `:2299` provides native coefficient lookup.
  No new polynomial representation, manipulation or factorizer is needed.
- RustRed `case/intersection/native.rs:103` already recognizes affine native
  polynomials by total monomial degree. `case/affine/bounds.rs:35` already computes
  sector-sign row bounds using native Integer arithmetic. It handles fixed axes
  and sector endpoints, not the arbitrary lower/upper bounds of these lattice
  boxes, so it is a reusable pattern rather than a directly sufficient API.
- RustRed also has the broader crate-private
  `AffineApplicationDomain::equation_proved_empty_in_box` service in
  `foundry/parametric/affine/restriction.rs`. It clones a transient polynomial
  and domain carrier and invokes `box_bounds.rs`, including native singleton
  canonicalization and finite-face refinement. It is relevant existing domain
  machinery, not a missing capability; it is not an allocation-free replacement
  for the narrow borrowed coefficient-row precheck needed here.

The inspected Symbolica polynomial/equation APIs provide native coefficients and
linear equation solving, but no directly matching exact integer affine-on-
LatticeBox interval service was found. A narrow RustRed domain helper can sum
the exact finite endpoints selected by each native Integer coefficient's sign,
using `None` for genuine infinity. It may return Nonzero only when zero lies
strictly outside the interval; otherwise it falls through unchanged. Applying
this to any one base-coefficient equation is sufficient. Box overapproximation
of a rank simplex is safe for this *disproof*, not a converse feasibility claim.
Never assign an assumed numerical sign to a symbolic base parameter.

Budget the scans, scratch and native integer payload under the existing guard
policy; handle huge fixed coordinates without narrowing them into i64. Probe
later AND atoms only for uniform nonvanishing, preserving source-condition
checks, original denominators and real errors. Tests should include the actual
`-2+n1+n3` bounds and its vanishing `(1,1)` counterexample, mixed diagonals,
infinite tails, negative coefficient direction, nonlinear refusal, symbolic
base splitting, cancellation/limits and exact unchanged dispatch priority.

## Saved generation provenance and the actual remaining gap

Generated payloads do retain affine case equations, fixed faces, all excluded
AND branches, RHS formulas and source seeds: see candidate bundle
`model.rs:354` and native reconstruction in `codec.rs:549`. They do not serialize
an explicit parent/child case-tree or the discharged generation queue. The
sector solver traverses exceptional children before returning a SectorSolution
(`solver/sector.rs:389`); that type expressly does not establish recursive
successor coverage above the generation entry rank.

The affected owner of the final refusal already completed standalone R10
SearchFinite generation: 841 saved rules, 113 finite residuals and 399,788 RHS
terms (`TMP/native3822-output-retry.dX6Q42/RESULTS.md`). There are 306 saved rule
positions after rule 534; their existence is not proof they apply on this box.
Concrete evaluation can test diagonal guards at exact keys already. The current
symbolic matcher is attempting the stronger ordered rectangular partition.
Calling its Unknown a missing rule would confuse an application representation
limit with an actual source-search gap.

After the cheap checks, two bounded alternatives are worth evaluating against
remaining measured obstacles, without building a general certifier:

1. Reuse saved case/guard predicates as conditional domains for dependency
   discovery. The queue must retain those predicates through specialization,
   rank images, routing and reuse; a box with dropped predicates is only an
   overcover and cannot nominate a reached missing-rule frontier. Existing
   structural scanning retains predicates but does not itself establish actual
   selected-rule successor closure.
2. An explicitly opt-in sufficient-rule policy may try a later already-valid
   formula on the same cell. Existing public APIs do not expose a safe arbitrary
   rule application override, so this requires narrow matcher/applied plumbing,
   not forged SelectedRule pieces. Mandatory source conditions, all chosen-rule
   guards/denominators and per-original-term child/descent checks remain.
   Carry skipped-unknown/refusal provenance; exhaustion is Unresolved, never an
   ExactGap. Do not let later overlay terminals shadow an earlier unknown rule.

No genuine source-search gap was established by this inspection. Necessary next
work is efficient valid saved-rule application and predicate-preserving shared
successor traversal, with actual above-entry-rank gaps investigated if they are
encountered. Full first-priority partition certification is not a prerequisite
for this deferred-certification campaign. No closure or speedup is claimed here.

## Implemented slice and release validation

The first two shortcuts are implemented: deterministic ordering of borrowed
base-coefficient equations by structural cost, and an exact native-Integer
affine sign test on the actual box. They preserve canonical equation storage,
actual native factor/replay admission, strict input checks and rule priority.
The affine test only proves nonvanishing; otherwise the existing resolver runs.
It accounts for actual inactive rank without clipping positive powers, uses no
expression clones, and falls back if its optional scan/arithmetic allowance is
insufficient. No AND lookahead or alternate-rule selection was added.

The release gates pass **2,640 core tests**, **320 application tests**, and
**19 Python steering tests**, with zero failures; 32 existing core diagnostics
remain ignored. The 17 new focused tests cover ordering, exact endpoints,
resource fallback, fixed specialization, rank bounds and concrete-evaluator
parity. Independent implementation and mathematical review cover the changes.

The initial full core run exposed three stale test expectations. Two expected
refinement/Unknown for positive sums now proved nonzero; their original inputs
are retained in new no-refinement/concrete-parity tests, and genuine diagonals
preserve every assertion of the original refinement tests. The third pinned an
old cumulative refusal count before its semantic checks: the cheaper-first
schedule changes that count, not the intended refusal. Its exact new diagnostic
is pinned and every subsequent positive/negative guard assertion remains. The
corrected full suite passes; the initial failed receipts remain separate.

Evidence: `TMP/native-guard-shortcuts.TjYasm/`,
`TMP/native-guard-core.KREpMe/` (initial diagnostics),
`TMP/native-guard-core-corrected.NMyRsx/` (passing core gate), and
`TMP/native-guard-app.jf9p4c/` (passing application gate/frozen CLI).
The later corrections touch tests only; the application binary's production
sources are unchanged. Compilation time is not a solver measurement.

## Same-input full-67 outcome

The control finishes all 67 input queries, with the same **59 locally resolved
owners** and eight incomplete owners. The former native factor-work refusal is
gone, but local coverage is still incomplete:

| Measurement | Before | After |
|---|---:|---:|
| Selected-rule regions | 213,523 | 214,681 |
| Declared-terminal regions | 893 | 900 |
| Unresolved guard regions | 163 | 166 |
| Native-work refusals | 1 | 0 |
| Preparation | 104.41 s | 104.95 s |
| Local matching | 120.24 s | 97.52 s |
| Whole command | 229.44 s | 208.27 s |
| CPU time | 227.74 s | 206.06 s |
| Peak RSS | 6,472,756 KiB | 6,476,340 KiB |

All **82** inspected sign-definite obstructions disappear. The other **81**
old unknown records survive unchanged. Continuing past the resolved predicates
exposes **85 new original-denominator unknowns**, giving 166 rather than a net
decrease. These arise at four saved rule/term positions on two owners; they are
not evidence of missing IBPs or actual singular endpoints. The formerly refused
owner now records 16,633 selected regions and 113 terminal regions (previously
13,983 and 106), with its same seven unresolved guards and no error.

The selection, R10 domains and native allowances are unchanged. The baseline
used CPU40 and the new run CPU41; both use one native worker and 64 GiB address
space, with no elapsed deadline. The new run adds the reviewed 48/60 GiB sampled
RSS supervisor and has no overlapping owned native/build workload. The shared
host is not isolated. Matching is about 19% shorter in this single observation,
with changed internal work and more progress past a refusal—not a statistical
or completed-workload speedup claim. Neither run performs recursive RHS walks
or new IBP generation. All exact-gap and invalid-source counts remain zero.

Evidence: `TMP/native-guard-full67.F1c0pX/`, including the runner, raw results,
timings, comparison and independent reviews. The process is terminal/reaped,
with incomplete status 4 and no operator/RSS stop. The next inspection targets
the newly exposed denominator expressions. The separate applied-RHS affine
refusal and planned optional-numerator treatment are recorded in
[the shared-walk report](shared_domain_index_2026-09-22.md). None of these local
measurements establishes complete recursive R10 closure.

## Follow-up: the 85 denominator checks

Inspection of the actual saved expressions finds **no index zero on any of
these 85 recorded domains**. Symbolica factors each specialized denominator;
all 85 exact native multiplication replays agree. There are 40 distinct
specialized polynomials. Their index-dependent factors are affine and strictly
positive on the recorded boxes. For example, one set has `n1>=4, n3>=2` and
factors `(n1-1)(n1+n3+t-2)` with `t>=1`, giving lower bounds 3 and 5. Another has
`n9>=2, n10>=2` and factors `(n9-1)(2*n9+n10-3)`, with lower bounds 1 and 3.
Formal base factors in `d` remain elements of the existing rational coefficient
field; this is not a statement about every numerical specialization of `d`.

The native factor calls total 0.003408 s in the diagnostic. This is neither a
campaign timing nor a new rule computation. Evidence and independent review:
`TMP/native-denominator-guard-triage.VZMSl5/`.

The follow-up implementation lets the existing resolver ask the same sufficient
affine interval check about **borrowed factors already returned by Symbolica**.
It adds no factorization algorithm or extra factor call. One optional work
allowance spans whole equations and factors; exhaustion falls through to the
strict existing result. Native input, work, output, remaining root and replay
checks stay mandatory. A genuinely intersecting diagonal stays unresolved.
This is separate from the optional RHS-numerator fallback described in
[the worklist interface](../shared_owner_domain_matching.md#optional-coefficient-support-classification).
Neither change promotes an unknown condition to a terminal or proves family
closure merely by exhausting a worklist.

The combined follow-up release gates pass **2,661 core tests**, **323 application
tests**, and **19 Python steering tests**, with zero failures and 32 existing
ignored core diagnostics. The added tests exercise mandatory error propagation,
factor/root replay, actual rank geometry, bounded optional diagnostics and
concrete dispatch parity. Independent code and mathematical audits pass. The
full core test executable runs in 157.68 s; builds are separate from solver
measurements. Evidence: `TMP/optional-factor-core.fwKhFq/` and
`TMP/optional-guard-app.f0H224/`. The new all-class controls below use that
frozen application binary, not an in-progress build.

### Follow-up same-input local control

The run finishes all 67 queries with **60 locally resolved owners** and seven
incomplete owners. Every one of the 85 inspected original-denominator unknowns
is removed, the other **81 full unresolved records are unchanged**, and no new
unknown is added. Owner `010111011000001` now classifies exactly. The count
change is not evidence that the seven remaining owners need new rules.

| Measurement | Whole-equation affine check | Borrowed-factor check |
|---|---:|---:|
| Locally resolved owners | 59 | 60 |
| Selected-rule regions | 214,681 | 213,951 |
| Declared-terminal regions | 900 | 900 |
| Unresolved guard regions | 166 | 81 |
| Exact gaps / invalid sources / native refusals | 0 / 0 / 0 | 0 / 0 / 0 |
| Preparation | 104.95 s | 106.86 s |
| Local matching | 97.52 s | 94.09 s |
| Whole command | 208.27 s | 206.50 s |
| CPU time | 206.06 s | 204.63 s |
| Peak RSS | 6,476,340 KiB | 6,473,076 KiB |

This uses the same inputs, native allowances, CPU41, one native worker and
64 GiB address-space envelope with 48/60 GiB sampled RSS supervision. There is
no elapsed deadline or overlapping owned build/native workload. The modest
timing difference is a single shared-host observation with changed partition
work, not a statistical speedup or full solve benchmark. Status 4 reflects
unresolved geometry; no operator/resource stop or native error occurred.
Evidence: `TMP/factored-guard-full67.uqUNUY/`, terminal and reaped.

The next shared successor control initializes every R10 owner orthant at once.
Its scope therefore differs from the older six-query controls and its timing
must not be presented as their matched speedup. Both optional-numerator
handling and the factored mandatory-guard checks are active; saved rules remain
unchanged. Full recursive closure, bounded parallel symbolic scheduling and
automatic missing-rule source feedback remain unfinished.
