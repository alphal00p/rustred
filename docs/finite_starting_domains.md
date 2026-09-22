# Finite starting domains and fixed-target repair

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
- 4.2647 s shared traversal; 108.2325 s native cold total including owner loading
  and route verification; 112.0488 s supervisor wall time;
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

Local verification evidence is in `TMP/finite-entry-gate.2GG13m/` (untracked).
The passing app gate took 64.97 s wall / 61.52 s user CPU, and the Python gate
50.20 s wall / 48.89 s user CPU; these are test-suite times, not solver benchmarks.
An initial unlicensed app run failed nine multicore preflights; rerunning with
the supplied license passed. An initial Python comparison accidentally selected
an older debug CLI and was interrupted; the passing gate explicitly selected
the matching release CLI and freshly built extension. No solver changes were
needed to resolve either setup issue.
