# Boundary-preserving search for reusable IBP rules

October 4, 2026. Research followed by an independently reviewed, opt-in
research-tool implementation: 102 focused unit tests pass. The registered
four-loop pilot, primitive-weight correction, and two-template follow-up all
completed with no accepted rule,
as recorded below. Neither this note nor
a search result authorizes installation, order changes, enlarged terminals, or
closure/performance claims. The rejected five-loop whole-column bank remains parked.

## Recommendation and the actual gap

Search for **vanishing on the activation faces of a collected endpoint
coefficient**, rather than requiring that endpoint to disappear identically.
Use a bounded polynomial ansatz in the relevant index variables and exact
Symbolica coefficient splitting. This is a small search-layer extension, not
a replacement CAS, a complete syzygy algorithm, or a weakened proof gate.

The current native checker already supports the desired mathematical behavior:
in [original_producer.rs](/common/dev/rustred/crates/rustred-core/src/foundry/artifact/source_port/original_producer.rs:368),
each sign cell first undergoes exact coefficient-vanishing checks, with original
guards retained; only surviving terms are checked for activation outside the
owner root and strict descent. Search should construct coefficients that pass
this existing test. Routing availability elsewhere in the campaign does not
override this local-root contract or the finite-face integral order.

The completed six-free-index owner-3 candidate recovered an exact identity but
failed this root test. The subsequent sufficient-only filter forbade all 952
root-unsafe columns of the complete specialized 250-row bank; together with
cofinal constraints it forbade 1,023 of 1,071 columns. All three modular samples
missed. That is evidence against that constrained bank, not against
boundary-divisible coefficients. See the
[completed diagnostic](/common/dev/rustred/TMP/rule-optimizer-20261003/incidence-study/owner3-rule146-orthant-root-boundary-v1/result.json)
and [geometry inventory](/common/dev/rustred/TMP/rule-optimizer-20261003/incidence-study/owner3-rule146-orthant-root-boundary-geometry-v1.json).
No further version of that bank is proposed here.

## The condition, including its failure modes

Write an authenticated combination of translated ordinary IBPs as

```text
0 = Σ_s C_s(n) I(n+s),       C_s(n) = Σ_r W_r(n) A_{r,s}(n).
```

All contributions to the same physical endpoint are collected before imposing
constraints. For an inactive physical index `n_j <= 0`, a positive shift
`s_j = s > 0` activates that denominator precisely at
`n_j = 0,-1,...,-s+1`. On a chart containing these faces, with denominators
defined there, a polynomial endpoint numerator vanishes on every such face if
it is divisible by

```text
φ_{j,s}(n_j) = n_j (n_j+1) ... (n_j+s-1).
```

This is an exact polynomial restriction, not interpolation at a few full
integer points: the other free indices and dimension remain symbolic. For
several relevant axes require each corresponding face restriction. Their
distinct coordinate factors are relatively prime, so polynomial divisibility
by their product is sufficient; this does not assert that the multivariate
ideals are comaximal. Bounds or guards can remove faces from a particular
admitted domain, but cannot be silently changed to make a candidate pass.

**Worked shift-two example.** Suppose the *normalized* RHS coefficient is
`b(n)=n(n+1)/(d-2)` multiplying `I(...,n+2,...)`, on `n<=0` with `d!=2`.
At `n=0,-1` it is zero; at `n=-2` it is `2/(d-2)` and the child index is zero;
for `n<-2` the child remains inactive. Unlike a whole-column ban, this keeps
a useful numerator-lowering term in the interior. This algebraic example does
not assert that it comes from any particular IBP or descends in the full order.

Two traps prevent using numerator factors alone as a certificate:

- If the raw target pivot is `C_0=n(n+1)` and the endpoint is also
  `C_s=n(n+1)`, isolating the target gives coefficient `-1`, not boundary
  vanishing. The rule cannot be used at pivot zeros. Multiplying an entire
  identity by `φ` creates no new boundary-safe rule.
- If a retained source or weight denominator is `n`, the expression
  `n(n+1)/n` is still excluded at `n=0` under its original proof conditions.
  Clearing denominators and then cancelling them does not authorize that
  face. Even a simplified zero or removable pole must retain the original
  denominator witness. The same applies to poles in other free indices.

Thus test both the raw source circuit and the normalized `-C_s/C_0`, including
all source/weight poles, pivot exceptions, and native finite-face descent.
An explicit unchanged fallback may own excluded cases; a candidate that only
works away from every problematic face is not evidence of improved boundary
coverage. Global domain preservation is an obligation of the combined program.

The distinction also has a limit: an endpoint that is nonlower on a
Zariski-dense cofinal region must still have identically zero rational
coefficient there. Boundary divisibility does not rescue cofinally forbidden
terms. It only avoids overconstraining terms whose obstruction is confined to
particular faces.

## Relation to the literature and work already tried

**Logarithmic and syzygy-constrained IBPs.** Smith–Zeng impose
`V(D_j)=h_j D_j` on protected denominators using a polynomial-module kernel.
Their sector convention fixes positive powers; remaining indices are
symbolic. Böhm et al. describe the additional module intersection enforcing
propagator divisibility, separately from their complete logarithmic generators
for the Gram determinant. These are principled ways to avoid raised protected
propagator powers; neither result implies a complete cost-optimal recurrence
program for our charts.
[Smith–Zeng, §§2.2–2.3](https://arxiv.org/html/2507.11140v2#S2.SS3);
[Böhm et al., §4.1, equations 4.8–4.10](https://arxiv.org/html/1712.09737v2#S4.SS1).

The distinction here is operational, not a claim of a new algebraic theory.
Those source constraints are polynomial in denominator coordinates `D` and
typically protect active propagators uniformly. The proposed constraints are
in index variables `n`, after translations and collection, and permit positive
inactive-index shifts away from activation faces. Multiple sources may cancel
only on the face. Polynomial `D`-multiplication becomes shifts, so the two
formalisms can overlap in the resulting IBP module; we claim neither disjoint
spans nor a stronger general algorithm.

The prior [protected-source study](/common/dev/rustred/docs/research/profile_guided_protected_sources_2026-10-03.md)
already tested degree-zero/one denominator-polynomial modules, selective
protection, and narrow tangencies. Its all-active degree-one ten-dimensional
kernel had zero full image; selective protection produced nonzero images but
no target under the nominated constraints. Those negatives must not be
rebranded as untried syzygy research. Likewise, ordinary unshifted IBP columns
already carry the raising factor `n_j`: rediscovering that fact is not a new
rule. The new test is whether bounded **index-polynomial weights on a
translated, collected block** produce a useful normalized rule not admitted
by the stronger global-zero constraint.

LoopIn explicitly splits constant/index-linear components before elimination
over the kinematic field. This supports the coefficient-splitting technique,
not a generic module-kernel claim. Our earlier 16-row four-loop component
preconditioner was structurally equivalent to the incumbent on the measured
bank; merely repeating that preconditioner is not this experiment.
[LoopIn, §3.1](https://arxiv.org/html/2602.19909v1#S3.SS1);
[local result](/common/dev/rustred/docs/research/global_degree_order_literature_2026-10-01.md:897).

**Relative cohomology.** Caron-Huot–Pokraka §2.3 retain bulk, boundary, and
intersection data: the differential includes restrictions onto boundary
strata. This motivates explicit boundary obligations instead of deleting
unwanted terms. Their boundaries are integration-coordinate divisors, not our
discrete index hyperplanes. Their formalism does not prove the index-factor
condition above or supply an existing RustRed implementation.
[Primary paper, §2.3](https://arxiv.org/html/2104.06898#S2.SS3).

**Generating-function interpretation.** Equation 6 of the generating-function
paper converts differential operators to index recurrences with factorial
products. Equations 4–6 also normalize numerator coefficients by factorials
and use a zero convention outside the formal series lattice. Subsector
contributions remain explicit in equations 5 and 8–11. This explains why
boundary-adapted factorial polynomial bases are natural, but the formal zero
convention is not permission to set physical pinched or reactivated integrals
to zero. The global ordering and completeness machinery are separate.
[Primary paper, §§II–III](https://arxiv.org/html/2509.21769v2#S2).
The repository already proposed
[generating-function border completion](/common/dev/rustred/docs/research/parametric_ibp_literature_2026.md:454);
this note recommends no new generating-function engine.

## A whole weighted block, not independent column pruning

For a target expression `T(n)=Σ_s t_s(n) I(n+s)`, seek an exact relation
`P(n) T(n) + Σ_r W_r(n) A_r(n) = R(n)` with an admissible endpoint block `R`.
Boundary restrictions act on each **collected coefficient of R**, not on every
source separately. `P` must be normalized and guarded just like a scalar
target pivot. Equal graph coordinates with different domain obligations do not
become interchangeable; distinct integral endpoints cannot be cancelled by
adding their coefficients unless an additional exact relation proves it.

There is precedent for optimizing combinations rather than reducing every
integral independently: Laporta's §§2.6–2.7 exhibit additional subsystem
identities that reduce the complete higher-dot combination `W_2` in equation
17. This is not a theorem that any graph diamond cancels.
[Primary paper](https://arxiv.org/html/hep-ph/0102033v1#S2.SS7).

A future stronger variant could require the boundary restriction of `R` to
belong to a **known, proof-bearing boundary relation module**, rather than
vanish endpoint by endpoint. That needs exact boundary circuits, their uncut
ordinary-source lift, and intersection/guard ownership. It is not licensed by
cut identities alone and is outside the pilot below. The typed incumbent
replay bridge supplies scoped original-source observations modulo authenticated
zeros, not unrestricted source authority. Later composition still needs full
original-image reconstruction and unchanged producer acceptance; that producer
does not currently reuse those zero proofs. Inspector inventories or guessed
recentering cannot substitute for this work.

## Symbolica audit: enough primitives, no assumed module solver

The local build uses vendor Symbolica **3.0.0**, commit
`ef0db494533c87adb40356c996241680dc5a7bff`, pinned through
[Cargo.toml](/common/dev/rustred/Cargo.toml:31) and
[Cargo.lock](/common/dev/rustred/Cargo.lock:1561). Independent read-only review
and direct source inspection found:

- Native polynomial coefficient extraction and grouped splitting:
  `coefficient`, `to_univariate_polynomial_list`, and
  `to_multivariate_polynomial_list` in
  [polynomial.rs](/common/dev/rustred/vendor/symbolica/src/poly/polynomial.rs:3528).
- Exact restriction/division via `replace`, `replace_with_poly`, `rem`,
  `quot_rem`, and `try_div`, subject to coefficient-domain, variable-map, and
  ordering preconditions. Rational `from_num_den(...,do_gcd=false)` assumes
  already coprime inputs; it is not a safe normalization shortcut.
- Exact field elimination through the existing
  [SparseRowReducer](/common/dev/rustred/vendor/symbolica/lib/numerica/src/tensors/sparse.rs:1592)
  and dense matrix methods. Polynomial ideal Gröbner bases exist, but no public
  generic polynomial-module syzygy/Schreyer service was found in the reviewed
  pinned Rust interfaces. Field elimination is not such a service.

A separate public-documentation check on October 4 found the official Rust
link serving **3.0.1**. Its polynomial API documents the same substitution,
coefficient-splitting, and remainder operations; the official guide describes
polynomial ideal Gröbner bases. Neither reviewed page documents a generic
module-syzygy interface. This is a qualified API finding, not proof that none
exists anywhere, and does not authorize upgrading the pinned dependency.
[Current Rust polynomial API](https://docs.rs/symbolica/3.0.1/symbolica/poly/polynomial/struct.MultivariatePolynomial.html);
[official polynomial guide](https://symbolica.io/docs/polynomials.html).

Use authenticated, budgeted RustRed wrappers instead of raw substitution as a
proof boundary. In particular,
[`specialize_fixed_indices`](/common/dev/rustred/crates/rustred-core/src/algebra/indexed/specialization.rs:272)
returns the coefficient **and pre-cancellation denominator witness**;
`specialize_fixed_polynomial` restricts retained guards. The existing
[`base_coefficient_system`](/common/dev/rustred/crates/rustred-core/src/algebra/indexed/base_coefficients.rs:275)
already wraps native coefficient splitting with validation and limits, but is
`pub(crate)`, not an exposed research API.

Crucially, solve with a bounded ansatz
`W_r = Σ_{|α|<=p} x_{r,α} n_B^α` over `K=Q(d,other indices)` while retaining
boundary variables `n_B` as polynomial variables. Split all polynomial
components, then solve for the `x` over `K`. Over the unrestricted field
`K(n_B)`, every nonzero boundary factor is invertible: an equation
`C=φ Q` with unrestricted rational `Q` imposes no divisibility constraint.
The bounded ansatz is implementable with existing algebra primitives, but
required the small budgeted constraint-assembly interface implemented below;
it was not available merely by changing the prior projector JSON. No
handwritten CAS or coefficient display parsing is needed.

## One prospective exact experiment

Use the established four-loop family and owner `1111111100`, with chart
`n=[2,1,1,1,1,1,1,1,n9,n10]`, `n9,n10<=0`. It contains the already documented
required point `[2,1,1,1,1,1,1,1,-2,0]` but leaves **both** numerator indices
unbounded. Freeze the archived family, owning root, integral order, source
semantics, and baseline fallback/terminal inventory before running anything.
This fresh capability test is not a retry or narrowing of the five-loop bank.

1. Materialize the 16 ordinary four-loop directions at exactly three declared
   centers `{0,+e9,+e10}`: at most 48 original rows. Specialize only the eight
   fixed active indices. Preserve all translated coefficients and guards.
2. Use only weights `a_r(d)+b_r(d)n9+c_r(d)n10`: at most 144 unknowns over
   `Q(d)`. No adaptive rows, degree growth, extra centers, or numerical
   interpolation. Positive endpoint shifts can reach two, so both one- and
   two-face restrictions are exercised if those columns survive natively.
3. Compare two exact constraint systems on that identical ansatz: a control
   globally zeroing all positive-inactive-shift endpoint coefficients, and a
   test restricting them only on their activation faces. Protecting both
   inactive axes is an explicitly stronger sector-preserving hypothesis than
   protecting only axes excluded by the owning root. Keep the same native
   cofinal nonlower constraints in both arms.
4. Extract a nonzero target coefficient, replay the complete original-source
   product, normalize, and run the unchanged native full-chart proof including
   all finite sign cells. Retain baseline ownership of every pole/pivot-zero
   exception. Reject a purported boundary success if normalization removes its
   needed zeros or all activation faces are merely excluded. Include exact
   polynomial unit tests for the shift-two example and both counterexamples
   above; these tests are not substitutes for IBP replay.

Prospective cap: one serialized exact diagnostic, CPU69 with its existing
supervisor slot, 16 GiB process limit plus 150 GB host reserve, 300 seconds
inclusive; at most 144 unknowns, 8,192 assembled constraints and one million
nonzeros. Exceeding a cap is censored, not permission to resize. Source/context
pins, coefficient-domain/map validation, and independent preflight precede any
future authorization. The original research phase made no source changes;
the subsequent authorized implementation and its tests are recorded below.

The discriminating result is a fully replayed, normalized two-free-index rule
admitted by the face constraints but excluded by the global-zero control,
with actual face coverage. A null kernel, zero target, full-image zero,
normalization failure, native order refusal, or equivalent incumbent rule
does not deliver a useful result for this **degree-one, 48-row pilot**. An
exhausted exact constraint reduction can report no target in that bounded
ansatz. Rejection of its first target-bearing dependency rejects only the
selected candidate: another kernel combination could pass. Neither outcome
refutes the general module idea. A hit establishes feasibility only. It must later earn its place
on the unchanged complete 58-query four-loop comparison through fewer distinct
full-context endpoints or domain fragments, not a kernel count or local term
count. Keep five-loop held-outs 24/31 untouched.

## Relation to the other bold directions

| Direction | Potential leverage | Principal limitation |
| --- | --- | --- |
| Boundary-aware index-polynomial search | A fixed-support rule can cover several unbounded indices in the existing family. | Bounded ansatz is incomplete; pivots, guards and finite-face order can erase the apparent gain. |
| Finite-rank numerator-basis transport | Changes the representation producing repeated rank/power redistribution. | A shift such as `q²-1` to `q²` has finite binomial transport at bounded integer rank, not a fixed-length rule for arbitrary symbolic rank; current same-family boundaries remain. |
| Regional compiler | Optimizes an exact weighted endpoint block using incumbent proof circuits and shared-graph cost. | Composition alone only removes intermediates in an already shared DAG; profitable endpoint or fragmentation reduction must be demonstrated. |

These are complementary. Boundary constraints can be an elimination backend
for a regional compiler, not a substitute for its cost objective. They have a
smaller representation footprint than a new numerator-family interface, but
less evidence of a large performance opportunity. The present 17-diamond
example has only a common one-free-index ray and does not establish broad
composition savings. Prioritize the typed replay/source-audit work already in
progress; this note supplies a bounded follow-on test, not a competing build
or an assertion of whole-program improvement.

## Bounded implementation slice and verification

The implemented research-tool slice is a generic, opt-in constraint compiler, not a
polynomial-module solver. Its input declares polynomial index axes, protected
inactive axes, a finite monomial weight list, and explicit row/constraint/work
limits. It preserves the ordinary `Span` and original source bindings. Native
Symbolica restriction and coefficient grouping convert the collected endpoint
face conditions into linear equations over the family's base-parameter field
(`Q(d)` for the proposed pilot); native sparse elimination supplies weights.
The first version refuses source denominators containing integral indices,
rather than introducing a denominator-clearing construction with extra domain
obligations. All original assumptions and final pivot/weight poles remain live.

Full original-image replay, normalization, and the unchanged native chart proof
remain mandatory. A normalized endpoint must still vanish on each activation
face: cancellation of a boundary factor against the target pivot, or exclusion
of an entire required face by a retained guard, is a refusal. The mode-off path
is unchanged. Independent source review preceded compilation. The optimized
85-test suite passed, including ten new boundary tests covering normalized
shift-two faces, cross-source cancellation, degree-one weights, pole/pivot
refusals, foreign maps/contexts, and tight resource caps. Compilation links
the deliberately frozen `0257b9e2` core/app libraries; concurrent preferred
program changes are excluded. One failed compile used a private accessor;
the independently reviewed correction uses the equivalent public parameter
count. Both receipts remain preserved. The discriminating falsifier remains the same finite-bank,
finite-degree control: a normalized face-preserving rule must exist where the
identical global-column-zero ansatz does not; neither a kernel nor a shorter
local identity is a performance result.

The implementation selects the first target-bearing dependency in declared
source/monomial order, then normalizes and checks its complete relation. A
candidate refusal does not exhaust all combinations of the kernel. Polynomial
multiples can add raw kernel vectors without yielding a different normalized
rule, so kernel dimension is not the novelty criterion.

The [paired input plan](/common/dev/rustred/TMP/rule-optimizer-20261003/incidence-study/four-loop-boundary-plan-v2.json)
binds the saved A1 four-loop owner, all 48 source rows, the unchanged chart/order,
and both exact exporter commands. Source grouping and each face/guard
specialization are explicitly charged before retained work; the outer process
guard remains necessary. The tiny mode-off compatibility check produced
byte-identical old/new/unit-test candidate bundles. Its strict report comparison
initially stopped because the older executable lacked the later diagnostic
`exact_projection_rows`; independently checking that new value was 1, equal to
`finite_bank_rows`, left all other report fields equal except timing. The failed
wrapper receipt is preserved; no native preflight rerun occurred.

### Completed registered four-loop result

The [paired run](/common/dev/rustred/TMP/rule-optimizer-20261003/incidence-study/four-loop-boundary-v1/result.json)
finished in 1.748 seconds inclusive, both process groups cleanly drained. Both
arms used the same 48 ordinary source rows, 144 polynomial-weight unknowns,
219 physical endpoint columns, 38 mandatory cofinal exclusions, and the full
two-free-index chart. Native constraint assembly enumerated 182 activation
faces. The two exact outcomes were:

- Whole-column control: 598 constraints; visited all 144 ansatz rows and returned
  `NO_TARGET_IN_BOUNDARY_POLYNOMIAL_ANSATZ`.
- Activation-face search: 388 constraints; the selected target-bearing
  dependency was refused because a retained pole/pivot/source condition
  excluded an entire activation face. It reached neither full-chart proof nor
  export.

This demonstrates the intended safety refusal, not a useful new recurrence.
The report does not identify which retained condition caused it; no coefficient
display parsing or guessed factor attribution is made. The control is a
bounded-ansatz no-target result; the face result rejects only the first selected
candidate, not every kernel combination. No candidate artifact, closure, or
performance improvement resulted. The registered bank/degree is stopped here:
no automatic source growth, chart narrowing, condition removal, or retry.

### Registered follow-on: primitive original weights, not image division

The failed receipt did not retain its pivot or source weights. A subsequent
code audit narrows the possible cause: there were no incoming source guards,
and all guards before target normalization were base-parameter-only. The first
possible index-dependent guard is therefore the target-normalization pivot.
This is a code deduction, not an observed factor/face or evidence that the
original weight circuit has a removable common factor.

The next authorized, default-off research option is
`boundary_polynomial.primitive_original_weights`. It accepts only a validated
ordinary source frame with unique `(RowId, offset)` bindings and an injective
unit/permuted selection. Summing monomial contributions per such row coalesces
the actual original weights. Native polynomial GCD of their authenticated
numerators nominates a common factor; index-independent denominators make this
sufficient up to base-field units. Native exact division and an independent
`w_i = g v_i` multiplication check are required for **every** nonzero weight.
The quotients must remain polynomial in the declared index axes and inside the
exact declared monomial set, not merely below a total-degree limit.

The pinned Symbolica APIs are `MultivariatePolynomial::gcd` and `try_div`
(`poly/gcd.rs:3642`, `poly/polynomial.rs:5513`); RustRed's own bounded sum
implementation uses this combination with output-map authentication. The
[current public API](https://docs.rs/symbolica/latest/symbolica/poly/polynomial/struct.MultivariatePolynomial.html#method.try_div)
also documents exact quotient refusal and variable-map behavior. The code uses
the pinned library, not a new CAS. Term-pair and cumulative-operation admission
does not bound native GCD scratch space or internal work; the outer process
guard remains mandatory.

Division constructs a new original-source proposal **before** adding any
target-normalization pivot. It removes no existing source/caller condition or
pre-cancellation pole, introduces no `g != 0` workaround, and never divides
only the endpoint image. The full image, forbidden columns, new pivot, face
conditions and native chart/export proof are rebuilt. A genuine source pole,
non-common weight factor, ansatz escape, or remaining singular pivot must still
refuse. Bounded observational diagnostics identify a proposed divisor and its
per-weight checks, and the actual refusing face/guard origin; displays are not
algebraic authority.

Independent source review approved this slice, including a cumulative-budget
regression covering already retained reducer payload. All 93 optimized unit
tests passed, including eight new primitive-weight cases. The new executable's
default-off toy report matched the previous executable exactly except timing,
and old/new/unit-test exported bytes were identical. Both input parsers passed.

The separately audited, unchanged 48-row/degree-one
[off/on correction](/common/dev/rustred/TMP/rule-optimizer-20261003/incidence-study/four-loop-boundary-primitive-v1/result.json)
then finished in 1.743 seconds inclusive, after 2.381 seconds for the compatibility
preflight. Both arms selected the first target-bearing dependency at ansatz-row
prefix 56, using three nonzero original-source weights. Native GCD found the
common factor displayed as `n8` (zero-based axis 8, physical index 9); all three
native exact divisions, independent multiplication checks, and quotient-ansatz
checks passed. No existing condition was removed.

Dividing every original weight by `g` scales every endpoint and the target by
the same factor. Consequently `(C_s/g)/(C_0/g) = C_s/C_0`: the generic normalized
rational identity is unchanged. Rebuilding the divided original circuit can
avoid introducing an artificial target-pivot condition, but cannot improve its
normalized endpoint coefficients. The later face test can therefore expose a
nonzero endpoint that was already present but hidden by the first refusal.

With the option off, the diagnostic identifies guard 42 as the original target
normalization pivot, vanishing on axis 8 at zero. With it on, the divided original
circuit was replayed and renormalized, but the native restriction test found a
nonzero normalized endpoint coefficient at that same face, for physical shift
`[-1,0,0,0,0,0,0,0,2,0]`. Thus removable source-circuit scaling really existed,
but removing it did **not** produce a boundary-valid rule. Neither arm reached
full-chart proof or export; both owned process groups drained cleanly.

Axis 8 is deliberately protected by this pilot even though the saved parent
root allows it. The result rejects the stronger sector-preserving proposal,
not an observed outside-root or native finite-face-order proof. The first
candidate's refusal does not exhaust the kernel. This registered correction is
stopped: no bank growth, degree increase, chart change, guard deletion, or further
automatic attempt. No performance or closure claim follows.

### Research-only next criterion: target and endpoint valuations together

Raw endpoint zeros alone are insufficient when the raw target also vanishes.
For a required face `ell = n_j+k = 0`, let `nu_ell` denote the exact polynomial
factor multiplicity, after honoring every original pole/condition. In this
polynomial-over-base-field setting, a normalized coefficient vanishes
generically along that face only if
`nu_ell(C_s) >= nu_ell(C_0)+1`; the zero endpoint satisfies this automatically.
This must hold on every relevant activation face. It is not a sampling test,
does not discharge guard intersections, and does not prove native descent.
It also does not allow dividing a circuit unless its original weights admit
the verified division above.

The smallest conservative future search change would constrain the target
coefficient to a **nonzero base-parameter-only pivot**, while keeping the exact
endpoint face equations. Native coefficient grouping could move every
nonconstant target monomial into the linear constraint block and leave only
its constant coefficient as an eligible pivot. Then target face valuation is
zero, so raw endpoint face zeros survive normalization, subject to the same
source poles and final native proof. This is a sufficient filter, not a general
solution: it excludes potentially valid index-dependent pivots and might have
no target in the fixed bank/degree. General simultaneous valuation search is
more involved because target valuation is unknown; guessing valuations or
sweeping kernel combinations would be additional, separately bounded work.

At that checkpoint the base-only criterion was retained as research backlog.
The follow-up below refines it to a prescribed target shape. No implementation,
rerun, widened chart, new source bank, or performance promise is authorized by
this discussion.

### Follow-up research: prescribe a usable target, not an artificial zero

This subsection is read-only research after the completed negative. The
primitive target was displayed as `(d-3-n8-n9)/(d-3)`. Its index dependence is
**not** the remaining obstruction: the numerator's coefficient of `d` is 1,
so no integer-index specialization makes it the zero polynomial in generic
`d`. The nonzero endpoint on axis 8 was the actual refusal. Thus making the
target base-only is sufficient but unnecessarily restrictive. This observation
does not evaluate at a numerical dimension, discard `d-3` or other poles, or
turn the diagnostic display into source authority.

**What the literature establishes.** Smirnov's sector-oriented `s`-reduction
separately checks sector preservation and a nonzero leading coefficient before
division; the `s`-form definition requires that leading coefficient to remain
nonzero at translated sector corners. Usability of the pivot is therefore a
construction obligation, not a consequence of finding an IBP kernel. This is
a useful precedent, not an instruction to replace our fixed integral order or
implement a new Gröbner engine.
[Smirnov, §§3–4, especially reduction steps 8–10 and condition (ii)](https://arxiv.org/html/hep-ph/0602078v1#S4).
Schabinger's explicit example shows that distinct bounded-degree kernel vectors
can be polynomial multiples of an existing syzygy; they need not give new
normalized relations.
[Schabinger, §2, equations 14–16](https://arxiv.org/html/1111.4220v2#S2).

**Smallest proposed experiment: target-template feasibility.** If separately
authorized, retain the exact 48 rows, 144 degree-one weight coefficients over
`Q(d)`, full two-index chart, 388 endpoint/cofinal equations, and every existing
proof gate. Compare just two prospectively declared raw targets: `P=1`
(base-only control) and `P=d-3-n8-n9` (the weaker index-dependent test).
For each, solve `C_0=P` together with those same homogeneous endpoint equations,
using native exact field elimination. Equivalently require `C_0=lambda(d) P`
with nonzero `lambda`, then rescale all weights over the base field. This
rescaling preserves the declared index-monomial ansatz; any introduced base
denominator remains a retained condition. The template is an input hypothesis
motivated by the negative, not a parsed certificate from its display.

An illustrative identity, **not a physical IBP**, makes the distinction clear:
`0=n[(d-3-n-m)I(n,m)-I(n+1,m)]`. Its raw endpoint vanishes at `n=0`, but the
normalized successor coefficient is `1/(d-3-n-m)`, which does not. Dividing a
common factor from the original source weights cannot change that ratio.
Target-template search instead seeks a genuinely different source combination:
target proportional to `P=d-3-n-m` while the collected unwanted endpoint still
has its required factor `n`. The current negative combination is not a solution
to those joint equations.

This is one fixed paired feasibility test, not a sweep of target shapes or
kernel vectors. The six previously observed target coefficient slots suggest
only a small linear extension; admit the exact union with template monomials
before allocating. No source/degree growth is needed. A generic input-declared
target-template constraint is the missing small search API; existing Symbolica
arithmetic and original-source replay suffice, with no new CAS or guard
classifier. A future preregistration should retain the previous 300-second,
16-GiB envelope, without assuming the prior millisecond algebra time predicts
new exact elimination costs.

An inconsistent exact system refutes only that template in this finite ansatz.
A first solution that fails retained guards or native proof rejects only that
solution, not all solutions with the template. Success requires a reconstructed
ordinary circuit, all normalized face tests and the unchanged producer/export
proof; neither feasibility nor a kernel dimension is a performance result.
The test cannot simply reuse the divided negative weights: their exposed
endpoint fails the very equations being imposed. Protecting axis 8 remains
stronger than the saved root. No experiment is authorized here.

**Larger alternative: a bounded target-valuation stratification.** This is our
inference, not a theorem supplied by the cited IBP algorithms. Fix a finite
vector `r_ell` of target multiplicities on the required coordinate faces.
Native coefficient extraction in powers of `ell=n_j+k` can impose
`C_0 mod ell^r_ell = 0` and `C_s mod ell^(r_ell+1) = 0` for each endpoint
requiring that face. These are linear conditions for a *fixed* stratum. Require
the order-`r_ell` target coefficient to be nonzero as a polynomial, otherwise
the selected relation belongs to a different stratum. Those are open
conditions, not more homogeneous zero equations. In particular the `r=0`
stratum allows general index-dependent targets and excludes the false
whole-face pivot zero without demanding a base-only target.

For a finite-dimensional exact solution space, each such identically-zero
target restriction defines a linear subspace. Over infinite `Q(d)`, finitely
many proper subspaces cannot cover the whole solution space. This can guide a
bounded exact combination construction, but proves only generic face
nonvanishing: exceptional subfaces, poles, provenance and descent still need
the native checker. It is not permission to sample integer points or choose
unboundedly many combinations. If raw coefficient degree is bounded by `D`
and there are `q` distinct face hyperplanes, a naive valuation enumeration has
up to `(D+1)^q` strata, with additional coefficient fill-in and intersection
work. The [ordinary builder](/common/dev/rustred/crates/rustred-core/src/identity/generator/ordinary.rs:68)
has index-linear coefficients and shifts `0`, `e_j`, or `e_j-e_k`.
Together with degree-one weights this gives the structural upper bound `D=2`;
the three fixed centers permit at most four distinct
activation hyperplanes, hence up to 81 naive strata, not 182 independent
face choices. This is a structural bound, not a measured factor census, and a
complexity warning, not a proposed 81-run campaign.

Positive target valuation is especially delicate. Even when the rational
endpoint quotient is regular, the raw identity has zero pivot on that face.
Our unchanged producer must still refuse it there unless a new, authenticated
ordinary-source lift supplies the divided identity, or separate face and
intersection circuits/fallbacks cover those cases. Endpoint division alone
does not provide that lift. A larger regional compiler could use these strata
to build an exact bulk/face program, but would need bounded case coverage and
full endpoint/fragmentation accounting. This is more scope than the template
test, and remains secondary to the preferred whole-program experiment.

**Why saturation is not the missing acceptance gate.** The recent critical
syzygy paper distinguishes the quotient `J:B` from saturation `J:B^mu` and
explicitly removes the `B=0` component in the latter geometry. Its connection
to critical surface terms has additional hypotheses and concerns Baikov
variables/maximal cuts, not these integer-index faces.
[Critical Points and Syzygies, §§3.2–3.4, equations 28–44](https://arxiv.org/html/2509.17681#S3.SS2).
For an endpoint module `M`, membership in `M:ell^infinity` means some
`ell^k R` belongs to `M`, not necessarily `R` itself. For example,
`M=<ell e_0>` saturates to `<e_0>`; the first relation supplies no value of
`e_0` at `ell=0`. A saturation computation may diagnose removable components
or nominate a lift, but cannot certify extension onto the divisor it inverted.
Full IBP shift operators also do not commute with index coefficients, so a
commutative ideal computation is not automatically an Ore-module certificate.
The safe finite-bank formulation remains an original-image module intersected
with endpoint face/jet conditions, followed by explicit target usability and
source membership. It is not another whole-column ban or a rebranding of the
already-tested denominator-coordinate protected-source module.

**Available native services and the precise gap.** Rechecking the pinned
Symbolica 3.0.0 sources and current public 3.0.1 polynomial documentation found
native grouping, exact restriction, GCD/quotient, and field elimination; the
official guide also documents polynomial-ideal Gröbner bases. No generic
module-saturation/Schreyer service was found in those reviewed interfaces.
Ideal Gröbner availability alone does not supply original-row lifting or a
budgeted module solver. Defer module saturation and multi-stratum search until
that authority/complexity design earns a separate bounded proposal; the chosen
next experiment is the fixed two-template test above.
[Public polynomial API](https://docs.rs/symbolica/3.0.1/symbolica/poly/polynomial/struct.MultivariatePolynomial.html);
[official Gröbner documentation](https://symbolica.io/docs/polynomials.html#groebner-basis).

RustRed already splits guards by base-parameter monomials and recognizes a
nonzero constant coefficient as an empty integer exceptional locus:
[`base_coefficient_system`](/common/dev/rustred/crates/rustred-core/src/algebra/indexed/base_coefficients.rs:275),
[`has_nonzero_constant_equation`](/common/dev/rustred/crates/rustred-core/src/algebra/indexed/base_coefficients.rs:206).
These are internal services already used through native producer/matcher
paths, not a new public classifier to duplicate. Merely nonzero on a generic
face is weaker than this uniform integer-index certificate. All claims concern
the declared independent generic parameters; physical specializations and
pre-cancellation denominator witnesses remain mandatory. Finally, because the
unknown weights lie in `Q(d)`, extracting a coefficient of `d` is not a
`Q(d)`-linear operation on those unknowns. Use a fixed template for a linear
search, or apply the existing coefficient witness **after** exact solving;
do not smuggle a new bounded ansatz in `d` into the comparison.

### Authorized target-template implementation checkpoint

The next opt-in slice declares `boundary_polynomial.target_template` as sparse
integer-monomial numerator/denominator data with an exact base-parameter name
map and full index arity. It uses only native constructors and arithmetic;
there is no coefficient-string parser. The denominator must be base-only and
its raw polynomial is retained before any cancellation. For canonical nonzero
template coefficient `P_p`, the compiler adds `C_m P_p-C_p P_m=0` over the
complete target/template monomial union and nominates only nonzero `C_p`.
There are still 144 original source-weight unknowns, no hidden scalar unknown.
Full original replay must verify nonzero base-field proportionality again
**after** any primitive-weight division. All existing normalization, face,
source, root, descent and export gates are unchanged; absence of the field
retains the old path. Independent source review passed, followed by all 102
optimized tests (nine new cases, none ignored or filtered). The source and tests
are isolated in
[`boundary/target_template.rs`](/common/dev/rustred/tools/research/rule_optimizer/symbolic_projector/boundary/target_template.rs)
and its test module. The cached-library test build took 82.446 seconds and the
guarded suite 4.358 seconds (0.93-second test body); the
[receipts](/common/dev/rustred/TMP/rule-optimizer-20261003/candidates/boundary-template-build-v1/run_tests/result.json)
retain exact source/library pins. The final executable linked in 82.863 seconds;
all three owned phases drained, and the build slot was released. This is
capability verification, not a physical reduction or performance claim.

The fresh native API audit also found a prior generic membership bug:
Symbolica grouping keys contain base coordinates before index coordinates,
but the primitive quotient test omitted that offset. The corrected test uses
`base_count+axis`, with multi-index regressions for a forbidden quotient and a
valid last-axis quotient. The previous 93-test receipts remain genuine tests
that passed, not proof that this missed case was covered. In the actual prior
degree-one pilot, a common linear `n8` factor leaves index-constant quotients;
the observed nonzero endpoint and negative conclusion are unchanged. This is
a generic sparse-ansatz enforcement correction, not new source authority.

### Completed target-template result: both fixed shapes miss

Independent input review preceded the sole
[two-template run](/common/dev/rustred/TMP/rule-optimizer-20261003/incidence-study/four-loop-boundary-template-v1/result.json).
Both `P=1` and `P=d-3-n8-n9` returned
`NO_TARGET_IN_BOUNDARY_POLYNOMIAL_ANSATZ`, without a selected target proposal or
exported artifact. Unlike the earlier first-candidate face refusals, these are
completed exact no-target results for the two constrained finite ansatzes.
The unchanged inputs retained 48 ordinary rows, 144 weight unknowns, 219
endpoint columns, 38 mandatory cofinal exclusions and 182 endpoint-face
instances. Each six-monomial target/template union added five proportionality
constraints, giving 393 total constraints and one eligible target coefficient.
No extra scalar unknown or kernel-candidate sweep was introduced.

The pair completed in 1.701 seconds inclusive, after a 2.392-second
[compatibility/parser preflight](/common/dev/rustred/TMP/rule-optimizer-20261003/incidence-study/four-loop-boundary-template-preflight-v1/result.json).
The unaffected mode-off toy reports matched exactly except timing, and old,
new and unit-fixture artifacts were byte-identical. This is not a claim that
the corrected primitive sparse-membership bug preserves every former result.
Both native arms exited cleanly and all owned groups drained; CPU69/70 were
released. The fixed 48-row/144-weight search is now parked: no automatic source
growth, degree increase, new target templates, face weakening or valuation
sweep. These negatives do not refute other pivots or polynomial modules, and
establish no closure, termination or performance improvement.

As a separate concrete motivation, native guarded application of the sole
extra leaf of the 465-rule/59-terminal source-order variant selected baseline
owner-3 rule 514 and emitted 101 nonzero successors (18 same-support, 83 strict
subsupport), with 29 zero terms among its 130 saved RHS terms. Two preserved
orchestration attempts stopped before traversal/cold verification and charged
212.000 seconds. The separately authorized corrected
[v3 run](/common/dev/rustred/TMP/rule-optimizer-20261003/discovery-owner3-extra-terminal-closure-v3/summary.json)
then passed the 101-child physical RHS join and cold All/Off verification on
3,056 domains (2,656 native processed), without new terminals, under the tight
`A8/R1/D7` singleton request. Its walk took 0.844 seconds after 99.924 seconds
of preparation; all three execution receipts charge 422.497 seconds. This is sealed
dependency coverage, not termination or coefficient back-substitution:
549 abstract nodes lie on cycles. The formerly separate candidate-to-baseline
handoff is now implemented generically and pushed as `014b351b`. Its completed
same-binary five-loop comparison passed cold All/Off on all four roots, but
increased scheduled domains from 19,109 to 19,279 (+0.890%): no performance
benefit or promotion. See the
[current paired result](/common/dev/rustred/CODEX_PROGRESS.md:41).
No candidate terminal was deleted or treated as free.
