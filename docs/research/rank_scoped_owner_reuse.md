# Reusing sector programs through verified momentum maps

Status: the bounded **concrete integral-key transporter is implemented** and
passes independent review and the release core gate. A trace-only saved-owner
dispatcher and selective loader are now implemented and pass independent
review and full release runtime gates. Coefficient back-substitution
through routed owners and whole-family coverage are not delivered capabilities.
This work avoids repeated sector search; it is not terminal minimization or a
closure proof.

## Trace-first implementation boundary

The new Rust-library surface deliberately separates three responsibilities:

- `CandidateOwnerContext` shares one immutable family, coefficient context,
  source conditions, zero certificates, input rank scope and request limits.
- `CandidateOwnerPrograms` prepares the selected saved sector programs without
  changing their original roots, rule orderings, guards or explicit terminals.
- `RoutedCandidateReducer::trace_targets` traverses exact local successors
  using already-verified `CandidateOwnerRoute` maps. It reports missing owners
  separately from missing rules in an installed owner.

The application crate's `load_generated_candidate_owners` takes borrowed bytes
and explicit masks for immutable, single-sector candidate bundles. It admits
all structural metadata and aggregate byte/collection limits before importing
native Symbolica state. The selected owners must share the family and the exact
saved solver policy; roots and orderings may differ. Zero-sector analysis uses
the deduplicated union of the actual saved-root downsets with an explicit work
limit. It does not scan all possible masks regardless of those roots. The
complete-checkpoint loader retains its separate locking/all-shards semantics.

One request shares rule-application, pending-state, node and expansion budgets
across all owners. Routing's conservative native-expansion operation/endpoint
bounds are charged before the expansion. Bounds on encoded input bytes are not
claims about peak decoded memory. An unsupported map, invalid support change,
failed guard or exhausted resource limit cannot create a terminal.

Traversal states distinguish `Route(key)` and `Apply(owner, key)`. A literal
owner applies directly; otherwise an exact map produces a finite combination
in its target owner's coordinates. Same-support IBP successors stay with that
owner and must descend in its saved ordering. Only strict support drops may
route again. This makes routing a one-way phase between successive reductions
in active-line count; a same-count change to another support is rejected.

The public rank bound is checked at input admission only. All internally
required successors remain visible, including above-rank intermediates. The
dispatcher reuses the existing exact first-applicable-rule evaluator; it does
not add a CAS implementation, search for rules or rematch graphs. A trace
coalesces each local expression, but does not back-substitute coefficients or
globally cancel a frontier. Even an empty finite trace is not a proof for
arbitrary positive denominator powers.

The first real-data pilot reuses twelve saved SearchFinite
programs (78,139,434 encoded bytes) for the same three class-29550 targets
previously traced with only one owner. Offline witness matrices are composed
with native Symbolica matrix operations and then verified again against the
actual loaded family. Its runtime measurements are separate from the release
test gates; no outcome is assumed from the metadata alone.

The source-consistent release core gate passes **2,474 tests, zero failures,
32 existing ignored**, including 17 new owner/dispatcher tests. It takes
133.48 s wall /132.46 CPU-s, with 159,904 KiB peak RSS. All **251 application
and integration tests** pass (174 unit plus 77 integration): 63.54 s wall,
62.34 CPU-s, 194,384 KiB peak RSS. The focused selected-owner, declared-affine
and complete-checkpoint tests also pass. Compilation is excluded from those
test timings, and these are regression tests, not solver benchmarks.
Independent reviews cover the implementation and the actual release receipts.
Evidence: `TMP/routed-owner-core-gate.zu7s29/` and
`TMP/selected-owner-app-gate.VafMK4/`.

## Actual five-loop reuse pilots

The first launch cold-loads the twelve programs and verifies all 37 maps but
then rejects the external CSV's old `trace,` command prefix. An input-only
correction removes precisely that prefix, preserving the same three integral
keys and the frozen executable; the failed setup receipt is retained.

The corrected run loads twelve owners in 3.251 s and verifies their 37 maps
in 0.393 s. Its rank-one target traces 12,253 distinct keys with 1,304 rule
applications and 361 transports in 0.269 s. It reaches 111 declared terminals
and 10,477 missing-owner keys across 125 supports, with no missing-rule groups.
Observed maximum rank is two, below the generation scope R10. These new
obligations are pinches exposed beyond the initial route set, not 10,477 proven
missing IBPs. The next rank-ten target reaches the one-million operational-node
allowance without a complete report; the third target is not attempted.
Whole process: 35.37 s wall /35.11 CPU-s /934,456 KiB peak RSS, status 1.

An expanded offline selection reuses one saved program for each of the 66
represented classes, retaining the exact original primary owner for comparison.
The 463,527,220 encoded bytes are larger than the unconstrained minimum-owner
sum because that primary is deliberately fixed. There are 8,215 covered-label
route records: 66 identities and 8,149 nonidentity maps. All 31 labels belonging
to still-missing class 30231 are explicitly omitted. This selection is neither
a terminal-minimality step nor a family-closure assertion.

Three independent processes cold-load that same selection successfully in
8.17–8.50 s and verify **all 8,149 composed maps** in 87.46–88.10 s. They then
hit the existing four-million *conservative cumulative endpoint estimate*,
not an observed four-million-node frontier:

| Input | Whole wall time | CPU seconds | Peak RSS, KiB | First rejected endpoint charge |
|---|---:|---:|---:|---:|
| rank one | 127.08 s | 126.19 | 3,035,980 | 4,000,073 |
| rank ten, one numerator axis | 117.41 s | 116.56 | 3,070,864 | 4,070,370 |
| rank ten, two numerator axes | 121.64 s | 120.76 | 3,073,916 | 4,174,774 |

All exit with status 1 and no complete trace report. These measurements include
each process's own loading and route preparation; they are not shared-cache
timings or completed reductions. The next explicitly budgeted diagnostic raises
aggregate work allowances while retaining time, per-call and memory limits.
No source search, back-substitution, master evaluation or regeneration occurs.
Evidence: `TMP/routed-owner-input.1hzrSj/` and
`TMP/routed-all-saved-owners.Hw5g0s/`.

### Larger work allowances: one complete finite trace

The same 66-owner selection, with unchanged rules and inputs, is retested using
explicitly larger aggregate transport allowances. The rank-one input
`[-1,2,1,0,1,1,0,0,0,1,1,1,1,1,1]` now completes:

| Quantity | Measured result |
|---|---:|
| Cold import / verify 8,149 maps | 8.203 / 87.114 s |
| Dependency trace | 51.184 s |
| Distinct keys / phase nodes | 541,889 / 544,228 |
| Rule applications / transport calls | 219,003 / 210,751 |
| Declared terminals / uncovered keys | 321 / **0** |
| Maximum encountered numerator rank / dot excess | 5 / 10 |
| Whole process wall / CPU | 148.33 / 147.16 s |
| Peak RSS | 3,103,304 KiB |

This is finite reachability under the candidate formulas, not coefficient
back-substitution, source-identity certification or arbitrary-dot family
closure. In particular, the input's rank one is not an internal cutoff.
The conservative cumulative endpoint charge is 8,557,491, explaining the
earlier four-million allowance failure; it is not an emitted-term count.

The two rank-ten inputs still stop at the explicit 64-million cumulative
endpoint allowance, requesting 64,223,654 and 64,044,620 respectively. Whole
times are 282.43 and 280.52 s, with roughly 3.12 GiB peak RSS each. Neither
returns a completed trace/frontier. The first includes a 20-second attached
profile and is therefore instrumented. That local 952-sample, zero-loss window
attributes about 74% inclusive samples to transport and 72% to native numerator
expansion; inclusive shares overlap and are not whole-process timings.
Symbolica's native rational coefficient domain is being evaluated to remove
unnecessary rational-function wrapper work from this already-constant domain.
No speedup is claimed until matched measurements exist.

The first focused release gate for this optimization passes fourteen of fifteen
expansion tests but correctly rejects the large-rational differential fixture:
its prospective output-storage estimate is too small. Symbolica's native
integer clone can reuse a larger cached GMP allocation, so source capacity is
not an upper bound on clone capacity. The runtime fails closed; it does not
return an incorrect polynomial. The failed receipt is preserved, and a native
output-allocation correction must pass the unchanged assertion before release.
Existing parallel campaigns and the measurements below use the earlier tested
implementation, not this unaccepted optimization.
Evidence: `TMP/affine-interval-native-q-core.SSsyiW/`.

The corrected boundary copy uses Symbolica's public native raw ownership APIs,
avoiding the larger pooled allocation without changing arithmetic or relaxing
the assertion. Its fresh focused release gate passes all 16 native-expansion
tests, including the original failure and an explicit warmed-cache regression.
All ten one-parameter geometry tests pass too. The subsequent full core suite
was initially interrupted; the runtime-only continuation now passes **2,492
core tests, zero failures, 32 existing ignored diagnostics**, followed by
**251 application tests, zero failures**. Source and frozen-binary checks pass.
Evidence: `TMP/affine-q-core-runtime-resume.QLBUYL/` and
`TMP/affine-interval-native-q-app-runtime.VycAew/`. This validates the corrected
implementation, not its speed on a rank-ten campaign.

The completed standalone native-3822 owner then permits a 67-program selection:
1,280,854,595 encoded bytes, all 8,246 census labels and 8,179 nonidentity maps.
The original 66 owners and routes are unchanged. Fresh bounded checks use the
existing explicit ingress fields (1 GiB per file, 2 GiB aggregate input), not
a new schema or relaxed mathematical admission. No rule regeneration occurs.
All 67 programs cold-load and all 8,179 composed nonidentity maps verify.
The rank-one control then reproduces exactly the earlier 541,889 keys,
219,003 applications, 321 terminals and zero frontier: 54.968 s trace time,
160.34 s whole-process wall, 159.15 CPU-s and 5,815,792 KiB peak RSS. Loading
the newly included large program increases setup/storage; this is a functional
control, not a claimed speed comparison. The two rank-ten runs subsequently
reach their 1,800-second deadlines without a complete trace report:
1,800.61 / 1,800.60 s wall, 1,786.65 / 1,786.59 CPU-s, and
5,974,040 / 5,887,956 KiB peak RSS respectively, both status 124. Neither
completed coverage nor a missing-rule frontier can be inferred from these
deadline-censored runs. They used the previous tested arithmetic, not the
pending native-Q specialization.
An additional 67-owner corner sweep completes too, but **every corner is
already a declared terminal**: 67 keys, zero rule applications and zero
frontier. Its 105.92 s whole-process time is dominated by loading and map
preparation. This is a catalogue/admission check, not new nontrivial reduction
coverage. Evidence: `TMP/routed-owner-corners.lw3Zhu/`.

A stronger generic sweep raises each owner's first active power to two and
sets its first inactive power to minus one. Before its shared 1,800-second
deadline, **11 of 67 targets finish**: ten nontrivial traces and one terminal
identity, all with zero frontier. Their summed per-target counts are
7,480,370 keys and 2,954,033 rule applications (not a deduplicated union).
Maximum encountered rank is six. One completed target alone visits 4,490,126
keys and takes 1,149.372 s. Target 11 is unfinished and targets 12–66 are
unattempted, not failed mathematical coverage. This motivates concurrent target
scheduling with one shared immutable program library, so an expensive early
target does not hide the remaining classes. Evidence:
`TMP/routed-owner-rank1-dot1.SGSe5o/`.
Evidence: `TMP/routed-work-limits.dGzynx/`,
`TMP/routed-application-profile.sK4PVQ/`,
`TMP/routed-all67-owners.WkZDFn/` and
`TMP/routed-full-census.GqXBUQ/`.

### Follow-up optimization boundary

For a fixed admitted route, numerator expansion is
`P(D) = product_i L_i(D)^max(-n_i,0)`. Positive input powers determine only
the mapped base key `b`; each expanded monomial `c_e D^e` gives `c_e I(b-e)`.
This permits a future bounded cache keyed by the exact route and full negative
degree vector, storing exponent offsets rather than concrete integral keys.
It cannot be keyed just by total rank, masks or graph class. Every hit must
still check the current base, overflow, support changes and request limits;
cached coefficients and outputs must fit a shared live-memory allowance.
Under unchanged conservative budget semantics, hits still incur the existing
prospective work charges. Such a cache can save CPU but would not itself cure
a cumulative work-cap failure.

This is an independently reviewed design observation, **not an implemented
cache or measured speedup**. Current traces do not record degree signatures,
so they establish no hit rate. First measure the native-Q specialization;
only then decide whether signature measurement and memoization are worthwhile.
Local design audit:
`TMP/selective-owner-dispatch-design.ooqQyX/NUMERATOR_PATTERN_MEMO_AUDIT.md`.

## Implemented transport seam

`sector::symmetry::integral_transport::compile` binds an exact `VerifiedMap`
to its source/target families and active roots, returning a shared immutable
`Prepared` object. `Prepared::transport` returns exact typed target integral
keys and coefficients after bounded native Symbolica numerator expansion.
It does not search for IBPs or apply sector programs.

Admission checks family fingerprints, coefficient contexts, equal integration
dimensions, zero analytic power shifts, unit Jacobian, no unresolved conditions,
a unit active-line bijection and rational-constant affine numerator images.
The independent audit caught the need to check dimensions explicitly: an
algebraically valid denominator substitution alone does not authorize replacing
an integral in dimension `d` by one in `d+2`. It also prompted cumulative
coefficient-memory admission before selected numerator rows are cloned.

The existing native numerator-expansion module now lives in
`family::numerator_expansion`, with its former artifact callers using that
shared implementation. No polynomial engine or rational reconstruction code
was introduced. Positive powers are transported without expansion; finite
negative powers use native polynomial powers/products/coalescing. Bounds and
unsupported maps fail explicitly without returning partial combinations.

The source-consistent release core gate passes **2,430 tests, zero failures,
32 existing ignored**, including 13 new transporter tests, all eight existing
native-expansion tests and all eight old matcher-transport tests. The full
suite takes 132.70 s wall / 131.66 CPU-s with 188,652 KiB peak RSS; compilation
is separate. Focused tests cover inverse cancellation, constants, multiple
numerator factors, pinches, arbitrary positive powers, admission failures,
overflow/budgets and deterministic shared-owner use. This verifies the local
transport service, not application through a routed program library.

An input-driven five-loop integration pilot subsequently checks one witnessed
map into each of the 67 census classes. All **607 concrete transports pass**:
corners, raised positive powers, every inactive coordinate at rank one, and
one rank-10 numerator per map. The 540 lower-rank/corner cases also compose
with independently verified native inverse maps and exactly recover their
original integral after coefficient coalescing; the 67 rank-10 cases check
forward transport, exact repeat, rank/dot bounds and support containment.
The pilot emits 220,195 terms in total, at most 8,008 in one expansion. Whole
optimized single-worker process time is **13.60 s wall / 13.45 CPU-s**, with
15,360 KiB peak RSS, including input reading, family construction, map
verification and repeated expansion. Compilation is outside that boundary.
This exercises 67 selected maps, not all 8,246 labelled routes or any IBP
application. Evidence: `TMP/integral-transport-real-routes.DuIDt1/`.

## Motivation and existing evidence

The frozen five-loop input census has 8,246 distinct labelled sectors not
proved zero by its screening, but only 67 graph classes. Existing offline witnesses route every labelled
sector to a representative by an exactly verified unimodular loop-momentum
map. The recorded native verification, independently audited, checks 57,890
active-line identities.
These witnesses are not generally permutations of the complete denominator
basis: inactive numerator coordinates can transform into affine combinations.

At the non-atomic September 21, 11:12 UTC checkpoint snapshot, 5,404 labelled
sectors are saved, representing 65 classes. Only 29 literal published
representative masks are saved; other classes have differently labelled
owners. Choosing a saved owner per class could avoid much remaining duplicate
search. It does not establish coverage of arbitrary positive powers or of
all recursive descendants. The separate finite-retention campaigns must not
be silently merged into the broad-search campaign's single-policy artifact.

A later 13:46 UTC inventory reaches 5,916 saved labels and 66 represented
classes. The newly saved class-29550 owner cold-loads with 487 rules and
56 terminals. Three finite traces (one rank-one and two rank-ten inputs) all
have missing successors, but every missing key has strictly lower support.
Across those traces there are 38 missing supports: 17 have an already saved
program in exactly those coordinates, and all 38 route to 11 represented
graph classes. This is direct motivation for selective program loading and
the cycle-free dispatcher below. Their trace-only implementation is described
above; the availability of those programs does not prove that their recursive
application will close. No same-support or above-rank missing key appears in these three
checks; this observation is not a uniform bound on arbitrary input powers.
Evidence: `TMP/class29550-frontier.mInPtQ/`.

## Route concrete integral keys, not every symbolic rule

Keep each owner's program, guards and ordering in its original coordinates.
Transform requested integral keys into exact finite combinations in those
coordinates, then use the existing guarded rule application. This avoids
copying and re-expressing every symbolic rule under every routing.

For the first implementation, admit only verified maps with:

- authenticated source/target families, equal integration dimensions and no
  unresolved conditions (native-proved nonzero constants are harmless);
- unit Jacobian and zero analytic power shifts;
- a unit-coefficient bijection of source active denominators onto the owner's
  active denominators;
- rational-constant affine images for the remaining denominator coordinates.

Compose source-to-representative and owner-to-representative witnesses with
Symbolica matrix arithmetic, then reverify the composed map with the existing
`sector::symmetry::verify`. A stored verification boolean is not a substitute
for an executable verified map. Unsupported maps remain explicit errors.

## Numerators, pinches and rank

Let the source's positive powers be arbitrary integers `a_i`, and its inactive
powers be `-b_j`, with `b_j >= 0` and `sum b_j <= R`. Map positive powers by
the active bijection, giving a nonnegative base vector `a'`. Expand only the
finite numerator product

```text
product_j (c_j + sum_k M_jk D'_k)^b_j.
```

Symbolica already provides the powers, multiplication and coalescing. Each
resulting monomial has exponent vector `e >= 0` with `sum e <= R`, and its
integral key is `a' - e`. Thus its numerator rank is at most R, its positive
dot excess cannot increase, and its active support cannot grow. Propagator
cancellation can produce lower sectors and must be preserved.

For an algebraic illustration, suppose an admitted map has active images
`D0=D'0`, `D1=D'1` and numerator image `D2=D'2+D'0-D'1+1`. Then

```text
I_source(a,b,-1)
  = I_owner(a,b,-1) + I_owner(a-1,b,0)
    - I_owner(a,b-1,0) + I_owner(a,b,0).
```

At `a=1` or `b=1`, some terms are pinched. This is an illustration of an
already-admitted affine denominator map, not a claim that arbitrary affine
matrices are realizable loop-momentum transformations.

This proof concerns entry transport only. IBP descendants can exceed the
entry rank R; they must remain reachable and may not be clipped or rejected
by reapplying the public entry check at every owner transition.

The actual uncut ordering does not itself guarantee rank nonincrease. Within
one support it first compares total corner distance `D+R`, where D is positive
dot excess. For example, `(3,1,-10)` has D=2, R=10, whereas `(1,1,-11)` has
D=0, R=11 and strictly descends. This is an ordering counterexample, not a
claim that a saved IBP contains that edge. Repeated hypothetical shifts
`(-2,0,-1)` could spend arbitrary input dots to grow numerator rank while every
individual path still terminates. Rank-10 input admission cannot justify
discarding those children.

A cheap sufficient check, to investigate on actual saved programs, is that
every same-support RHS shift has nonnegative sum over inactive source axes:
its rank change is exactly minus that sum. Combined with strict support
containment, this would bound rank growth to the finitely many pinch events.
For constant shift vector delta and active set A, a conservative pinch bound is

```text
B(delta,A) = sum_(i inactive) max(-delta_i,0)
           + sum_(i active)   max(-1-delta_i,0).
```

If every same-support term passes the check, a finite library with maximum B
and p initial active lines has the sufficient envelope `R_entry+p*B`.
No such universal property has been checked for the current saved programs.
A failing conservative check would mean "not proved by this criterion", not
proof of an actual unbounded dependency or permission to invent terminals.

## A sufficient cycle-free application schedule

1. Select one immutable owner per class and route a non-owner entry to it once.
2. Apply same-support IBPs in that owner's original exact ordering.
3. Route to another owner only after the active-line count strictly decreases.
4. Apply the same rule recursively to every induced pinch.

The well-founded measure is active-line count, then a one-way routing phase,
then the owner's exact descending integral order. Finite numerator expansion
does not itself introduce an infinite branch.

Enforce active-support containment. The current generic integral order can
permit a same-count move to a different labelled support; the proposed narrow
scheduler must reject that transition rather than silently invoking routing.
A more general transition policy would require another termination argument.
No decoded-program census has yet established that every saved program meets
this additional admission condition.

## Reuse existing components

- `VerifiedMap` already owns exact denominator images, conditions and Jacobian.
- The native numerator-expansion helper has moved mechanically to the shared
  family layer. Its powers/products/coalescing remain Symbolica-owned; do not
  implement a second polynomial engine.
- The existing candidate reducer owns exact guards, coefficient specialization,
  strict descent, zero handling and memoization. Reuse its internal one-step
  operation and successor checks rather than recursively calling its public
  entry API with reset budgets.
- The complete-checkpoint loader must retain its completeness requirement.
  A separately named selective owner-library loader can admit chosen shards
  against their original manifests, sources, orderings, rank and policy.

Use shared immutable family/map/program owners. One aggregate request budget
must cover routing, expansion, application, memoization and the missing
dependency frontier. Missing owners, unsupported maps, exhausted budgets and
uncovered children remain distinct incomplete results.

## Risks and acceptance

Sparse expansion can still be large: the unrestricted count of monomials of
degree at most R in 15 variables is 3,268,760 at R10 and 3,247,943,160 at R20.
Actual verified maps may be much sparser; these counts are bounds, not measured
workloads. Keep existing native-expansion resource checks and explicit limits.

Test identity/permutation agreement, non-involutive map composition, multiple
numerator factors, cancellation, pinches, arbitrary positive powers, rejected
foreign/unsupported maps, same-count routing cycles, above-R internal children,
shared budgets and deterministic results. Old shards must remain unchanged.

Even a successful finite dependency trace establishes only those requested
reductions. Full rank-bounded family closure additionally needs parametric
coverage of the positive-power directions and their complete successor set.
Neither 67 saved owners nor master-count agreement proves this.

Local design evidence: `TMP/full-routing-owner-audit.84kF2V/RECOMMENDATION.md`
and `TMP/verified-key-transporter.3AmZKz/API_DESIGN.md`. No reference-only source
code or unpublished PDF content is included in this design. The implementation
gate and independent audit are recorded in `TMP/integral-transport-gate.sknvDG/`.
