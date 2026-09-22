# Finite starting domains and fixed-target repair

## Execution resumed from the September 22 checkpoint

The user has explicitly resumed work from the tested and pushed checkpoint.
The matched maximal-index pilot and all-owner R0 control have now run, both
resource-incomplete. Their audits direct the next work toward compact correlated
domains and serial admission cost, before larger controls. The objective below
is **unfinished**, not complete or mathematically blocked. Current evidence and
resume steps are in the [bounded-routing pilot report](research/bounded_routing_pilot_2026-09-22.md).

## Objective

Generate a closing finite IBP program for a generic, finite envelope of
dimension-four renormalizable starting integrals. Do not specialize to an
enumerated QCD beta-function diagram catalogue. Five loops is the current
pressure workload, not an engine specialization.

Retain existing parametric rules as the fast path and repair only concrete
missing targets. Terminal minimization and numerical evaluation are deferred.

## Domain contract

For a scalar key `n`, distinguish:

- `A = sum(max(n_i, 0))`, total positive denominator power;
- `R = sum(max(-n_i, 0))`, scalar-product numerator degree;
- `t = count(n_i > 0)` and dot excess `A - t`.

A finite entry envelope supplies finite bounds on both A and R, optionally
correlated by dimensional power counting. This is an input-domain restriction,
never a reduction or source-generation cutoff. Entry bounds never filter
descendants; existing compact-index admission and resource failures remain
explicit rather than silently dropping a dependency.

The engine accepts explicit budgets and masks without recognizing topology
names, fixed loop counts or a specific observable. A separate physical profile
must justify its budgets from graph/vertex power counting, gauge convention,
external Taylor order and the treatment of nested renormalization forests.
In particular, a one-shot two-point Taylor expansion is not automatically a
bound on every recursively expanded GammaLoop forest term. Until the profile
is audited, explicit budgets define only the stated finite mathematical domain,
not a guarantee of all renormalizable amplitudes.

### Conditional marginal-coefficient profile

The first named profile is
`EntryPowerBudget::renormalizable_marginal_feynman(L)`. Its explicit assumptions
are ordinary dimension-four renormalizable vertices in Feynman gauge,
three-/four-valent skeletons with tree two-point insertions resummed, conventional
proper UV forests without vacuum/one-point nodes or oversubtraction, and
polynomial scalarization without loop-dependent projector denominators. All
explicit mass factors must be retained in the power count; stripping an
inverse-mass factor invalidates the grading argument. This is not automatic
authentication of an arbitrary GammaLoop model or UV prescription.
For the forest argument, maximal children must be vertex-disjoint and each
contracted forest quotient must retain a loop. Forest conventions allowing
children to share vertices are not automatically covered by this proof.

For E external legs and root superficial degree delta <= 4-E, a connected
skeleton has I <= 3L+E-3 internal lines. Contracting the proper 1PI children of
each ordinary forest node leaves a nonempty bridgeless quotient and therefore
at least one loop. Quotient loop counts telescope to L, bounding the number of
Taylor nodes by L. Since each proper nonvacuum, non-one-point node has degree
at most two, their total derivative budget obeys
`B <= 2(L-1) + delta`.

Each denominator derivative adds at most one positive power:
`A <= I+B <= 5L-1`. A child external momentum can be another loop momentum,
so a Taylor factor can add **two** global numerator degrees. Starting from
`N0 <= 2I-4L+delta` and extracting the marginal coefficient's delta true
external degrees gives `2R <= N0+2B-delta <= 6L-2`.
Finally, dimensional homogeneity with nonnegative explicit mass/coupling
degree P gives `4L+2R-2A+P=0`. Hence this conservative finite envelope is:

```
A <= 5L-1,  R <= 3L-1,  A-R >= 2L.
```

At five loops this is `A<=24, R<=14, A-R>=10`. It is not legitimate to claim
that R10 alone covers every raw nested-forest term. The correlated restriction
is nevertheless much smaller than admitting independent rank and dot caps:
an undotted twelve-line target permits R<=2, an eleven-line target R<=1,
and a ten-line target only R=0. Scalar rank is not tensor rank.

L is the number of **still-unintegrated** loops, summed across factorized
components. A lower-loop skeleton multiplied by an already integrated
counterterm needs its own profile, not the five-loop grading merely because
its perturbative order is five. A full two-point jet including its mass term
likewise needs a different grading than the marginal p-squared coefficient.
General covariant-gauge longitudinal terms and different forest policies can
be admitted with explicit larger budgets; they are not silently included in
this named profile.

### GammaLoop integration boundary

A read-only audit of the local GammaLoop checkout supports the forest premise
for its default `HedgePoset` path: additions tested independent are vertex-disjoint,
and evaluation of a disconnected union multiplies its components without an
additional Taylor operation (`uv/hedge_poset.rs`, `from_spinneys` and
`compute_4d_for_node`). Ordinary subtraction steps add cycle rank.

However, `uv/approx/local_4d.rs` keeps the full UV scaling jet, and
`uv/approx/integrated.rs` sends it to integration without first selecting only
the marginal coefficient. Thus the current marginal profile is **not** a
coverage guarantee for every GammaLoop integration call. For example, the
one-loop mass tadpole `[1]` belongs to a two-point mass term but fails the
marginal entry condition `A-R>=2`.

For a full nonvacuum two-/three-/four-point UV jet under the same structural
assumptions, the conservative input bounds instead are
`A<=5L-1`, `R<=3L`, `A-R>=2L-1`: a two-point coefficient can retain engineering
dimension two instead of zero. At five loops this gives `A<=24`, `R<=15`,
`A-R>=9`. The generic explicit-budget interface can represent these bounds;
there is no new automatic physics-authentication claim. The final campaign
must either use a documented marginal projection or include the full-jet
domain. It must not silently drop mass-counterterm inputs. Gauge/model
assumptions and graph-local correlations still require explicit admission.

### Next tightening: graph/forest profiles, not a diagram catalogue

The conservative envelope is an admission mechanism, not a claim that every
tuple can occur. The next useful restriction is a union of structural profiles,
each supplied by graph/forest power counting rather than by an observable's
enumerated diagrams. Under the assumptions above, a profile can record:

- I: original propagator count before equal-momentum denominator merging;
- N0: original numerator momentum-degree bound;
- B: sum of the applicable forest Taylor degrees;
- e: extracted degree in true external momenta.

These imply `A<=I+B` and `R<=floor((N0+2B-e)/2)`, retaining any additional
mass-grading restriction. For example, a **primitive** five-loop two-point
graph has `B<=2`, `I<=14`, `N0<=10`, `e=2`, hence `A<=16, R<=6` for its
marginal coefficient. That example does not cover graphs with subdivergences.
Recording the number V4 of quartic vertices sharpens the conservative marginal
profile to `A<=5L-1-V4, R<=3L-1-V4` under the same skeleton assumptions.
These are proposed derived bounds, not implemented automatic graph admission.

One can also retain where Taylor derivatives act. Let b_j be baseline powers
after native equal-momentum denominator merging, and let C_v contain the merged
slots containing any original line that Taylor node v may differentiate. All
indices refer to the same merged-slot coordinates. For `d_j=max(0,a_j-b_j)`, necessary
conditions include

```
d_j <= sum(delta_v for v with j in C_v)
sum(d_j for j in S) <= sum(delta_v for v with C_v intersect S nonempty).
```

Here S is a recorded group of denominator slots, not necessarily every possible
subset. A root with Taylor budget two and one child with budget two permits
four additional powers globally, but at most two on lines outside that child.
Numerator cancellation only lowers denominator powers, so it preserves these
upper bounds. Conservative incidence must include every line a derivative can
affect; an incorrect routing-based omission would make the restriction unsafe.

Keep the **union** of profiles: separately maximizing every budget discards
these correlations. Generate admitted regions directly rather than filtering
the trillion-tuple envelope. None of this requires another CAS or integer
programming implementation. Conversely, do not infer I or V4 from the final
support size t: quartic vertices, numerator pinches and equal-momentum merging
can all reduce t. Already integrated counterterm insertions require explicit
baseline powers and numerator/Taylor data as well. The marginal/full-jet and
gauge distinctions still apply.

The auxiliary-mass method also requires consistent treatment of mass terms and
counterterms; it does not justify silently deleting them. See Section 2 of
[Chetyrkin, Misiak and Münz](https://arxiv.org/abs/hep-ph/9711266). The particular
finite bounds above are our conditional power-counting derivations, not claims
that this paper supplies those bounds.

## Implementation sequence

1. Audit and document the physical envelope; expose generic finite entry-domain
   traversal with deterministic ordering and bounded-memory input generation.
2. Add opt-in fully fixed nomination to the existing feedback session. Keep all
   powers; use the existing `SearchFinite` source solver and exact replay.
3. Separate unresolved search outcomes from deliberately accepted finite
   terminals. A nonminimal terminal policy is allowed, but unfinished work,
   cancellation and errors are never silently accepted.
4. Connect bounded entry batches to shared concrete dependency tracing and
   owner feedback. Do not require coefficient back-substitution or terminal
   numerical values to establish this generation milestone.
5. Validate entry-domain exhaustion, concrete guard selection, cross-owner
   routing, strict descent, fixed-point feedback and finite explicit terminal
   output on small controls. Audit implementation and mathematical scope
   independently. Run release builds for solver experiments.
6. Run the complete five-loop census with one shared rule/dependency campaign,
   Python steering, useful progress accounting, and 50-core/500-GB limits.
   Report exhaustive completion only when every admitted input and its
   dependencies reaches an explicit terminal or zero without unresolved work.

## Scalability and reporting

Finite does not mean cheap. Report exact input counts when available, queued
and processed nodes, actual missing keys, new rules, searched residuals and
accepted terminals, maximum intermediate A/R, cache hits, worker utilization,
CPU/wall time and aggregate RSS. An iterator must not be mistaken for bounded
campaign memory if the scheduler eagerly consumes it before starting workers.

No elapsed deadline replaces monitoring. The retained objective is completion
within fifteen hours when measurements support a realistic attempt; otherwise
checkpoint, diagnose, optimize and retry. Saved rules are not regenerated by
default.

## Current implementation status

The generic finite-domain iterator and opt-in fixed-target nomination are
implemented. The licensed release application gate passes 332 library tests
and 82 integration tests; the matching release Python/CLI gate passes all
50 Python tests. CLI and Python return identical complete planning JSON for
the 67-owner census. These gates ran in the shared working tree; unrelated
pre-existing edits are not part of this milestone. The domain uses Symbolica's
public `CombinationIterator` and `Integer::binom`, not another combinatorial or
CAS kernel. Declaration, public re-export and existing call sites were checked.
Independent review caught and corrected an empty-shell traversal issue before
the milestone: correlated bands now prune impossible A layers before iteration.

The one-loop end-to-end control exhausts the starting keys `[2]`, `[3]`, `[4]`,
repairs missing rules, and reaches the explicitly searched terminal `[1]`,
which lies outside the starting envelope. Its results agree with one, two and
six trace workers. This checks the separation of entry bounds from descendants;
it is not a five-loop performance measurement.

Planning the current 67-owner census with the conservative five-loop profile
counts **2,188,260,327,648 labelled starting tuples**. This is an exact count
of the envelope, not a count of missing rules or independent integrals. It
includes many tuples that no renormalizable graph produces. Consequently,
bounded batches alone are not a practical full-campaign algorithm. Reuse
parametric rule regions to skip already covered inputs, intersect their genuine
gaps with the finite envelope, and repair only the remaining concrete targets.
Graph-local power-counting correlations may tighten admission further without
requiring an observable-specific diagram catalogue. No brute-force traversal of
the trillion-entry envelope has been launched.

### Five-loop concrete control

A release, Python-steered control selected the first admitted starting key in
each of the 67 owner classes, then traced their dependencies jointly through
the **unchanged saved rules** with 50 workers. It completed with zero missing
rules/owners and zero failed nodes:

- 67 starting keys, 347,816 distinct reachable integrals, 348,652 operational nodes;
- 130,348 rule applications, 5,797,375 shared-work deduplication hits;
- 314 existing declared terminals, with no new source search or terminal evaluation;
- 4.2647 s shared traversal; 108.2325 s application timer including owner loading,
  route verification and post-trace work; 112.0488 s supervisor wall time;
- 5.645 GB sampled aggregate peak RSS (not an exact transient-peak measurement).

This demonstrates complete dependency traversal for those 67 explicit inputs,
not coverage of every index configuration in their classes. It performs no
coefficient back-substitution and is not a new closing parametric artifact.
Most cold time is setup rather than rule traversal; retained sessions are
therefore important for the next campaign. Evidence:
`TMP/finite-entry-gate.2GG13m/shared-owner-campaign.p3kv5i96/`.

Two campaign integration requirements remain explicit: the current scheduler
eagerly admits all supplied targets, so a lazy generator alone is not bounded
campaign memory; and saved R10 entry scopes must not silently admit an R14
profile. Bounded retained-session batches, compressed coverage/gap traversal,
and compatible explicit entry scopes are required before the full run.
No finite renormalizable five-loop envelope
has yet been claimed exhaustively covered, and no complete new campaign has
been launched.

### Five-loop diagonal stress control

A second control uses 50 concrete inputs in owner `111010100100101`, on a
diagonal associated with a previously unresolved symbolic guard. In ordinary
integral indices, `n_0=n_1=t` for `3<=t<=9` and `n_11=-k` for
`1<=k<=min(10,2t-4)`; other active powers are one and inactive powers zero.
They lie inside the conditional marginal envelope and the saved R10 entry
scope. This is a diagnostic input selection, not topology-specific engine code.

The first run hit its explicit two-million operational-node diagnostic
allowance after 13.585 s of tracing. It had not observed any missing rule, but
1,384,225 nodes remained queued, so it established no completion. Its 46 failed
nodes were resource-stop fallout, not 46 missing IBPs. A second run retained
the same inputs/rules and increased the diagnostic work allowances, without
an elapsed deadline. It **completed**:

- 50 inputs; 54,695,037 distinct reachable integrals, 54,695,087 operational nodes;
- 15,512,818 rule applications; 14,535,822,697 deduplication hits;
- zero missing rules, missing owners, failed nodes or remaining queued work;
- five existing declared terminal keys; no new rules or terminal evaluations;
- 1,552.875 s shared traversal (25.881 min), 1,666.006 s application timer,
  1,695.856 s supervisor wall time;
- 50 configured workers, not a claim of 50 continuously busy cores;
- 31,294 s sampled aggregate CPU time (about 18.48 busy cores averaged across
  the sampled supervisor interval, including setup);
- 26.109 GB sampled aggregate peak RSS, well below the 450/500 GB thresholds.

The application timer includes preparation and post-trace work, but is not
the whole child-process lifetime: heartbeat evidence indicates about 102.7 s
before tracing and 10.4 s after the final scheduler snapshot; the supervisor
also includes process shutdown/reaping. None of those differences is a pure
artifact-loading benchmark. These counts refer to exact dependency tracing without coefficient
back-substitution. The trace made 1,198,295 routing calls and charged an aggregate
projected pre-coalescing endpoint bound of 29,691,375,754; this is a prospective
work counter, not the observed number of emitted endpoints. Deduplication avoids repeating node expansion,
but does not eliminate the work of generating repeated routed dependencies.
Thus this run is evidence of large concrete routing fan-out, not a new rule gap
or an estimate that the complete envelope can finish in fifteen hours. Its
finite completion is restricted to the 50 supplied roots and their descendants.
Evidence: `TMP/finite-entry-gate.2GG13m/shared-owner-campaign.k1ou4fev/`; the
resource-censored first attempt is `shared-owner-campaign.gcu38c5o/` beside it.

### Bounded positive-coordinate refinement

The opt-in `OwnerDomainRefinementAxes::FiniteAxes` policy extends existing exact
guard refinement to positive coordinates with explicit finite upper bounds.
The default remains `InactiveOnly`. CLI/Python steering exposes
`--bounded-refinement-axes finite-axes` together with a separate positive
`--max-bounded-refinement-cells-per-query` allowance. A full split is admitted
before any of its faces is published; insufficient allowance retains the
original unresolved condition or typed native refusal. Rank bounds apply only
to inactive coordinates and never manufacture a positive-coordinate bound.

This reuses Symbolica-backed guard algebra. It preserves first-rule priority,
whole excluded conjunctions, denominator/source guards and cancellation. It
is exact local dispatch classification, not a routing extension or a closure
claim. Bound preservation across owners is handled separately by the bounded
routing implementation below, not by the refinement flag alone. The focused
release gate passes 84 matching tests, including ten new finite-axis tests and
the full `u64`-width overflow boundary. The subsequent complete release core
gate passes **2,710 tests**, zero failures and 32 existing ignored diagnostics
in 156.47 s (suite time, not a solver benchmark). The licensed release
integration gate also passes 335 application-library tests and 82 integration tests, 50 public
Python API/CLI tests, ten matcher-steering tests and twelve campaign-supervisor
tests. Independent code/mathematical review found no remaining issue.

On the saved five-loop diagonal, a box with local coordinates `x_0,x_1=1..16`,
`x_11=1..10` and all others zero contains 2,560 points. For positive axes
`n=x+1`; inactive `n=-x`. The default policy leaves its coupled equality
unresolved. Finite-axis refinement resolves it into 46 selected-rule regions
using one 16-face split: 16 diagonal pieces select rule 285 and 30 off-diagonal
pieces select rule 309. All 2,560 points were independently dispatched as
singletons, with **zero partition or rule-selection mismatches**. All 50 stress
roots lie inside this box. Only 980 of its points satisfy the marginal envelope:
this rectangular diagnostic intentionally overcovers, rather than defining
the physical starting domain.

The one-owner query takes 9.574 ms of matching, 893.360 ms of preparation and
0.94 s whole-command wall time. It performs **no RHS traversal** and must not
be compared as the same workload as the 25.881-minute concrete trace. Evidence:
`TMP/finite-entry-gate.2GG13m/diagonal-match-{inactive-only,finite-axes}.json`
and `diagonal-singletons-result.json`.

A larger local control restricts all 57 formerly unresolved R10 boxes to a
conservative box overcover of `A<=24`. For each positive axis i, its new local
upper bound is `min(old_upper_i,24-t-sum(other_positive_lower))`; inactive
bounds and the R10 simplex remain unchanged. This preserves every original
point satisfying A24 but can include points violating the correlated A/R
profile. None of the 57 queries is empty. Loading the seven relevant unchanged
owners without routing, native first-rule dispatch resolves **all 57 queries**
into 1,956 selected-rule regions using 45 existing owner/batch/rule combinations,
with zero unknowns, exact gaps or invalid
source conditions. It uses 954 refinement faces in 84 splits; matching takes
2.700 s, preparation 9.534 s and the application timer 12.240 s.

This removes the local guard obstruction for these **bounded R10 queries**.
It does not cover R14/R15 inputs, all recursively reachable domains, or the
whole five-loop physical envelope. The saved candidate rule IDs were not
forced: each query used ordinary ordered dispatch. Evidence:
`TMP/finite-entry-gate.2GG13m/bounded-guard-{queries,owners,result}.json`.

### Bounded routing implementation

The audited design is now implemented. The complete release core gate passes
2,720 tests (32 existing diagnostics ignored) in 161.61 s, including the 21
focused native routing tests. Application/integration tests pass 422; all 72
Python API/CLI, matcher-steering and supervisor tests pass against the matching
release build. Existing
admitted `Prepared` maps already store an active-row unit bijection. Their
endpoints have powers `n'=B-e`, where B copies positive source powers and the
affine inactive numerator substitution gives `|e|<=R`. Thus surviving positive
axes can retain their mapped source upper bounds, with physical lower bound one
(local lower zero). For a pinched source subset P, each lost positive axis
consumes at least `local_lower_j+1` numerator degree, giving the conservative
residual rank bound

```
R_child <= R - sum(local_lower_j + 1 for j in P).
```

If that sum exceeds R, the subset is impossible; do not saturate the residual
rank to zero. Affine constants can only lower the monomial degree. Do not
permute inactive-coordinate bounds as if the numerator map were a bijection,
and do not preserve positive lower bounds after numerator cancellation.
Literal-owner routing can preserve the original box exactly. Source validity
remains a separate mandatory obligation, even for known-zero destinations.

`visit_bounded_domain_route_overcover` uses the already compiled
`Prepared.active_target` map, not another routing or CAS implementation. Bounds
are carried through initial nonliteral admission, RHS successor admission,
mapped Apply and route reentry. Full-orthant calls delegate to the same visitor
with zero lowers and unbounded uppers. Weighted subset pruning charges examined
candidates, not only emitted covers; `masks_pruned` records impossible weighted
pinches separately from emitted events. Invalid boxes fail before dispatch and
rank-empty intersections emit nothing.

Ten new native differential/boundary tests compare covers with actual
`Prepared::transport` endpoints and cover affine constants, multi-pinches, rank
zero, unbounded rank, overflow, cancellation and source guards. Application
tests exercise both initial and genuine generated-IBP successor routing, source
obligations, above-entry rank and worker-count parity. Independent mathematical
and source review confirms the bound for the currently admitted map class; it
is not a claim for arbitrary nonlinear denominator transformations or global
recursive closure. No polynomial algebra kernel was introduced. An initial new
application test supplied a multi-sector bundle to a single-owner loader; its
fixture was corrected to use the existing single-sector native solver/exporter,
without weakening the bound assertions. Gate evidence is in
`TMP/bounded-route-gate.clR7HK/`; the shared working tree also contains unrelated
scheduler edits, which are not part of the bounded-routing commit.

The first 50-worker recursive diagonal-box pilot then stopped for optimization:
74,326 of 159,344 scheduled domains completed with zero observed frontiers, but
85,017 remained queued. It spent 321.095 s in traversal and reached 15.301 GB
sampled peak RSS. Its 6.091 billion general containment comparisons saturated
serial publication throughput. Every committed record retained finite positive
bounds. This is incomplete diagnostic evidence, not a closed domain. A small
maximal-candidate lookup optimization is implemented and independently audited;
its release gate passes 428 application/integration and all 72 Python/steering
tests. The core is unchanged from the passing 2,720-test gate. The index retires
only redundant lookup entries, never pending work. The resumed matched run
now establishes a 2.88–2.92× traversal improvement for an identical completed
74,326-domain prefix. It stops at the inherited 50-million-event allowance,
with 137,684 complete and 127,080 pending. The all-67-owner binary-dot/R0 control
then stops at 500,000 scheduled domains, with 90,428 complete and 409,571 pending.
Neither reports a missing-rule frontier, and neither is closed. The all-owner
control exposes renewed serial admission pressure despite low memory use;
do not blindly widen it or the independent-axis A24 envelope. Preserve compact
correlations and improve containment lookup before the next larger attempt.
See the [three-lane audit and measurements](research/bounded_routing_pilot_2026-09-22.md#resumed-measurements-and-recommendations)
and [primary-source recommendations](research/finite_domain_campaign_literature_audit_2026-09-22.md).

### Retaining total-power correlations

Even finite coordinate bounds can be a very loose overcover. The 67-owner
rectangular A24/R10 control contains 91,254,043,244,510,861 index tuples: separate
coordinate caps allow total A as high as 156, instead of 24. Such boxes are
diagnostics, not a faithful enumeration of the physical input envelope. The
resumed pilots confirmed excessive growth. The implementation now retains
`A<=A_max` and a band `D_min<=D=A-R<=D_max` alongside each box and the existing
rank cap. These constraints survive tightening and participate in equality,
containment, reuse and reported domain provenance.

For a fixed support, active and inactive sums range over independent integer
intervals. Their aggregate feasibility and coordinate projections can be
computed in O(N) with checked integer arithmetic and explicit infinity; no
general polyhedral solver or CAS kernel is needed. Include the active offset
`A=t+sum(active_local_coordinates)`. Tightened coordinate bounds alone do not
replace the A/D predicates: e.g. two powers in 1..3 still include (3,3), which
violates A<=4.

After the existing sign-crossing coordinates are fixed, native
`applied/geometry.rs::image` already computes exact delta_R. For a fixed IBP
shift s, `delta_D=sum(s)` and `delta_A=delta_R+delta_D`, so translate the
constraints with the image. Do not reapply original entry caps to descendants.
Admitted affine routing obeys `A_child<=A_source`, `D_child>=D_source`; a
weighted pinch cost C tightens A and R upper bounds by C. Preserve D_min but
generally widen D_max, because affine constants can lower numerator degree.
Literal routing preserves the entire contract. Effective projected caps, such
as `R<=A_max-D_min`, should be used before costly candidate enumeration.

An exact decomposition into independent axis boxes is not a practical
alternative: distinct positive tuples on A=24 cannot share a contained box.
Thus support size t alone requires at least `binomial(23,t-1)` boxes, totaling
37,167,402 across the present census even at R0. This is a lower bound on exact
box representation, not a lower bound on solving the parametric problem.
The compact geometry service and its native/application integration have
independent mathematical/source review. Its 11 standalone optimized tests pass,
including exhaustive comparison across 442,368 small finite domains. Native
matching, applied successors, routing, application queue/cache identity and
diagnostics now retain the predicates. The full release core gate passes 2,748
tests (32 existing diagnostics ignored); application/integration gates pass 435
tests and Python/API/steering gates pass 72. Query, match and walk JSON schemas are
v2; see [the input contract](shared_owner_domain_matching.md). The optional
guarded-pullback path explicitly refuses constrained inputs rather than silently
discarding predicates. No original entry bounds are reapplied to descendants.
The unconstrained wrappers retain their existing behavior.

Workspace design and audit evidence is in
`TMP/bounded-routing-pilot.4kY19R/CORRELATIONS{,_AUDIT,_IMPLEMENTATION}.md`
and `POWER_DOMAIN_INTEGRATION_AUDIT_2026-09-22.md`. Release-gate evidence is in
`TMP/power-domain-gate.UpQa87/`. This implementation milestone does not establish
recursive five-loop completion, a campaign speedup, or an all-family ETA.

The first constrained local five-loop query selects 980 of the old diagonal's
2,560 points using A<=24, D>=10 and R<=10. Its 28 selected-rule regions agree
point-for-point with the saved singleton oracle: zero partition or rule-selection
mismatches and no excluded point admitted. Native matching takes 0.008475 s,
preparation 0.702373 s and application total 0.710908 s. This is first-rule
classification only, with neither RHS following nor new rule generation.
The subsequent 50-worker recursive pilot stopped cooperatively for admission
optimization after 106.956 s traversal: 49,686 completed domains, 145,431 queued,
one partial cancelled domain and zero observed frontiers. It spent 4.972 billion
containment comparisons, including 0.988 billion reverse maintenance, with
9.589 GB sampled peak RSS. Correlation fidelity alone does not remove redundant
domain representations or conservative routing fanout. Independent review
initially recommended measuring cached semantic inclusion, followed by a census
of exact full-permutation routes. The subsequent audit now measures a 4.27-fold
index-only replay gain, while the map census rules out full-permutation routing
for the observed traffic. Support-aware affine bounds instead tighten 62.23%
of projected root covers in a native diagnostic. The integrated
semantic-containment gate and recursive measurement are now complete; the
narrower support-aware routing slice has also passed its release gate and its
separate recursive pilot is now measured. These are not end-to-end speedups; details and
validation caveats are in the [parallel audit](research/finite_domain_parallel_audit_2026-09-22.md).
Neither input covers the full marginal R14 or full-jet
R15 envelope. Local receipts are under
`TMP/correlated-routing-pilots.jw6Bwo/match-probe.hQCWzw/`; the independent
verification is recorded in `LOCAL_MATCH_AUDIT.md` in the parent directory.
The recursive receipt is `shared-owner-campaign.jtsg2q0t/` in the same parent;
the [pilot report](research/bounded_routing_pilot_2026-09-22.md) gives complete
boundaries and audit findings. No durable pending-work resume or closing
artifact is claimed from the interrupted run.

### Cached semantic containment checkpoint

The queue now caches exact coordinate/A/R/D extrema for its admitted domain
vocabulary. This recognizes equivalent or containing regions missed by raw
optional-bound comparisons, without changing raw keys or cancelling pending
obligations. Six completed-descriptor replays measure a 4.27-fold median paired
index-time improvement and approximately fivefold fewer comparisons; they do
not replay every original admission or establish an end-to-end speedup.

The release core gate passes 2,758 tests (32 diagnostics ignored), and the
application/integration gates pass 443 (one replay diagnostic ignored).
Two initially failing fixture counts were corrected with explicit shared-ID
assertions: a fully active owner has actual R=0 regardless of its rank label.
All original worker-count and initial-orthant equivalence checks pass. The
same-input 980-point recursive pilot uses a frozen semantic-only binary,
independent of the subsequent support-aware native routing implementation.
Its receipts are `TMP/correlated-routing-pilots.jw6Bwo/shared-owner-campaign.r8txq1a3/`;
this implementation checkpoint does not claim that pilot or the full physical
five-loop envelope has closed. See the [audit synthesis](research/finite_domain_parallel_audit_2026-09-22.md).

That semantic-only pilot stopped cooperatively after 169.163 s traversal with
125,503 completed domains, 103,626 pending and one partial cancelled domain.
It observed no frontier, but spent 8.157 billion containment comparisons and
averaged about 2.08 busy cores in its late interval. Peak sampled RSS was
11.515 GB. This is unfinished conservative traversal, not a missing-rule
diagnosis, a completed envelope, or an end-to-end speedup versus the differently
scheduled earlier prefix.

### Support-aware affine routing checkpoint

Prepared native maps now cache which inactive source rows can contribute to
each target denominator, using Symbolica's exact coefficient zero test. For
target j, the source-derived degree bound is
`C_j=min(sum(relevant upper_i), R_max-sum(irrelevant lower_i))`.
A surviving positive local coordinate retains lower bound
`max(0, source_lower_j-C_j)`; a pinch is impossible if `C_j<=source_lower_j`.
Simultaneous pinches still consume their full combined weighted cost. Each
child uses source-derived bounds rather than the projected all-positive
sibling's bounds, and newly inactive axes reset their local lower to zero.
These are necessary support conditions, not assertions of actual reachability.

The unconstrained wrapper's behavior and artifact schemas are unchanged.
There is no numerator expansion or new CAS primitive, and no entry caps are
reapplied to descendants. The release gate passes **2,768 core tests** (32
existing diagnostics ignored), **443 application/integration tests** (one
index-replay diagnostic ignored), and all **72 Python/steering tests** against
the matching release CLI and extension. The focused routing gate passes 40
tests. Independent mathematical and source review passes; see the
[routing audit](research/affine_support_routing_audit_2026-09-22.md).

The same-input 50-worker pilot is recorded separately under
`TMP/correlated-routing-pilots.jw6Bwo/shared-owner-campaign.ujyk56ua/` with
release-gate evidence in `TMP/affine-support-gate.cv24Y0/`. Measurements include
the pre-existing scheduler/escrow working-tree changes used by both compared
pilots, not just the clean native-routing commit. No full-envelope completion
or fifteen-hour forecast follows from passing these implementation gates.

The pilot stopped cooperatively for optimization after **121.100 s traversal**:
63,913 domains complete, 113,500 pending, one partial cancelled domain and zero
observed frontiers. Preparation was 103.737 s, application total 224.836 s and
supervisor total 232.155 s; sampled peak RSS was 8.666 GB. It spent 6.887 billion
containment comparisons. The completed-result buffer again filled while the
queue grew and only 1–3 cores were typically busy late in the run. Stronger
bounds also distinguish more domains and can reduce reuse by containing boxes;
the observed prefix did not establish a traversal benefit. Stopped runs with
different admission histories do not define a campaign-speed ratio.

Independent comparison of 5,910 **identical Route source descriptors** present
in both completed prefixes does establish local pruning: emitted Route
descendants fall from 101,805 to 18,429 (81.90% fewer), with unchanged Apply/zero
counts and identical examined-mask counts. This is a per-query result, not a
full-campaign reduction factor. The index must handle the resulting more varied
domain descriptions efficiently before that local gain can improve traversal.

The next bounded step is an aggregate-extremum filter over cached native
summaries, preserving exact containment, the earliest valid retained candidate,
all pending jobs and fallible-allocation semantics. Both forward lookup and
reverse retirement must avoid unconditional scans. Measure complete admission
streams and memory/selectivity before adoption; see the
[independent index recommendation](research/domain_admission_index_next_step_2026-09-22.md).
This is not a reason to widen the physical envelope, generate new rules, or
begin terminal evaluation.

### Aggregate admission index: focused gate passed, integration pending

The queue implementation groups maximal candidates by exact
native `(A_max,R_max,D_min)` extrema, including explicit infinity and empty
cases. Both forward lookup and reverse retirement filter whole groups before
the unchanged exact containment test. Exact/orthant lookup retains priority;
the fallback still returns the minimum valid retained ID, independent of group
order. Retiring a lookup candidate never removes an exact key or pending job.
Finite-comparison-cap behavior is unchanged, and storage/counters are preflighted
before mutation. There is no new CAS primitive or production schema.

Independent source/math review passes. The actual-source optimized queue gate
passes 36 tests (one diagnostic ignored), including three complete streams of
46,080 requests with every returned ID and complete state checked against an
independent linear-semantic model. This includes repeated and rejected requests.
The frozen original production queue also returns identical IDs in twelve
saved-descriptor timing replays, six per receipt. Median paired wall-time ratios
are 2.59x on the 63,913 support-pilot descriptors and 2.09x on the 125,503
semantic-pilot descriptors, including all filter work in the timing boundary.
These sequences omit original rejected campaign proposals; they are not full
campaign replays or closure results. Filtering is not universally faster: one
shuffled synthetic stream performs slightly more full checks.

The full release application rebuild/gate and fresh Python gate are pending;
no campaign has run with the new index yet. Source and timing audit:
[aggregate index audit](research/domain_admission_index_audit_2026-09-22.md).
Local evidence: `TMP/aggregate-admission-gate.HrfYdG/`.

### Readiness beyond the index pilot

A follow-up source audit separates the remaining integration obligations:

- The complete conservative full-jet input is representable with existing
  query fields: for each census owner of support size t, use zero local lowers,
  active upper `24-t`, inactive upper 15, rank 15, and retained `A<=24,D>=9`.
  This is an input specification, not a completed campaign. Current prepared
  pilot files contain smaller controls, not these full all-owner queries.
- Wider symbolic matching is intentionally allowed by
  `walking/mod.rs`, but public concrete root admission still enforces saved
  R10 context scope in the native campaign scheduler. An above-entry overlay
  does not widen that scope; a dedicated R11 regression verifies this. A
  successful R15 diagnostic must not silently retag R10 saved bundles or be
  advertised as R15 public evaluator admission.
- Conservative frontier boxes are explicitly not reached missing-rule claims
  (`walking/inspection.rs`). Concrete witness extraction and replay from an
  admitted starting input remain necessary before fixed-target repair.
- `RoutedFeedbackSession::replace_targets` already supplies bounded retained
  batches, and exact fixed-target nomination/overlay installation exist. The
  missing connection is compact traversal to genuine witnesses and retained
  CLI/Python feedback steering, plus explicit requested-entry-domain authority;
  it is not the batch-replacement primitive itself.
- Worklist resolution remains separate from final bounded-closure reporting,
  fixed-point feedback, source-valid routing, and explicit finite terminals.
  Graph/forest incidence profiles are still proposed restrictions, not
  implemented or automatically authenticated physical input filters.

Do not treat index speedups as completion of these tasks or move to terminal
minimization/evaluation before the requested finite starting domain is handled.

Local verification evidence is in `TMP/finite-entry-gate.2GG13m/` (untracked).
The passing app gate took 64.97 s wall / 61.52 s user CPU, and the Python gate
50.20 s wall / 48.89 s user CPU; these are test-suite times, not solver benchmarks.
An initial unlicensed app run failed nine multicore preflights; rerunning with
the supplied license passed. An initial Python comparison accidentally selected
an older debug CLI and was interrupted; the passing gate explicitly selected
the matching release CLI and freshly built extension. No solver changes were
needed to resolve either setup issue.
