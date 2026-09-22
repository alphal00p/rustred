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
