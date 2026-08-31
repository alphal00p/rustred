# Dual-obstruction source discovery for exact parametric-IBP completion

## Status and scope

This note specifies the next source-discovery experiment after the measured K6 degree-one owner
sweep remained `NonFinite`. It targets translated ordinary IBP sources that can remove the largest
uncovered lattice orthants without constructing every source in a larger total-degree shell.

The proposed **target-normalized inverse-incidence oracle is a RustRed-specific synthesis and
inference**. It is not a theorem claimed by any paper cited below. Its fixed-sample progress lemma
is proved here directly. The literature motivates symbolic target normalization, exceptional
strata, shift-operator ideals, modular discovery, and standard-pair geometry; it does not establish
that this algorithm closes K6 or scales to K21.

The objective remains an exact, strictly descending rewrite artifact with a finite terminal set.
The terminal set need not be minimal. Modular ranks, modular duals, bounded searches, and sampled
stability are discovery evidence only. **No modular negative result may create or promote a
terminal.**

### Implemented prerequisites (2026-08-31)

The following generic boundaries are now implemented, regression-pinned, and independently
audited:

- selected translation of only canonical `(ordinary source, signed offset)` requests;
- checked target-last modular right obstructions with `q_target = 1` and exact finite-field
  `A q = 0` replay;
- one construction-neutral sealed physical-plan core shared by rectangular chart and sparse
  selected-source shells;
- provenance-only source-instance identity, with translation radius retained solely as scheduling
  metadata;
- strict physical-plan binding at modular, exact-lift, and partition joins, plus a sealed plan
  identity and pointer rejoin across semantic and outer-extension owner-cover authority;
- complete-ordinary versus external-only source-layout provenance sealed through translated
  batches and required by the incidence index;
- a complete-source modular evaluator whose point and finite-field domain are obtainable only from
  one admitted `ModularPhysicalFrame`; and
- bounded inverse-incidence nomination using checked `alpha = u - s`, canonical deduplication, and
  existing-row exclusion.

The canonical K6 zero-offset index contains nine sources, 90 term incidences, and 31 distinct
relative shifts. Its target-unit bootstrap produces 90 unique requests. This is a structural
census, not a modular hit or closure result. Executable full-row residual pairing, stable request
accumulation, and one immutable fresh-plan/partition/query epoch are now implemented. The bounded
outer scheduler which repeats those epochs and hands a live hit to exact lift remains the next
slice.

## Measured K6 control

The sealed K6 source set has nine ordinary rows with term counts

```text
(8, 11, 11, 11, 8, 11, 11, 11, 8),
```

so one translation layer contains 90 exact terms and 31 distinct relative shifts. The current
one-sided chart frames have:

| degree | offsets | rows | entries | S4a columns | S4a modular hits | exact replays |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 7 | 63 | 630 | 157 | 2 | 2 |
| 2 | 28 | 252 | 2,520 | 488 | 22 | 22 |
| 3 | 84 | 756 | 7,560 | 1,191 | not a pinned sweep | not a pinned sweep |

Across all six full-rank sector orbits, the pinned degree-one sweep produced owner counts
`0, 3, 2, 2, 3, 7`. All 17 modular hits replayed exactly. Only five admitted owners were
individually guard-total, and the maximal uncovered free dimensions remained
`6, 5, 6, 6, 5, 5`. The combined S4a degree-one plus degree-two sweep admitted 24 owners but only
one guard-total owner; its exact complement still consists of three five-dimensional unbounded
boxes. Increasing degree produced real relations but did not solve closure.

These frames remain controls. The inverse-incidence lane must beat them in exact cover improvement,
not merely in row count or modular hit rate.

## Algebraic problem

Write an ordinary source row as

```text
G_r(n) = sum_{s in S_r} c_{r,s}(n) J(n + s).
```

A translation by `alpha` is

```text
G_(r,alpha)(n) = sum_s c_{r,s}(n + alpha) J(n + alpha + s).
```

For one requested target, partition every integral column into:

- `target`: the integral being solved;
- `forbidden`: an integral not strictly simpler, including every unowned proper-subsector term;
  and
- `allowed`: an authenticated strictly descending term, zero/factorization result, or immutable
  lower-sector owner.

This is one fixed classifier on the entire signed-shift universe for the duration of a discovery
task, not merely a classification of the columns currently materialized. A newly encountered
column is classified against the same target stratum, ordering policy, and immutable lower-sector
snapshot. Changing any of those inputs starts a new task/epoch; it must not retroactively change
the matrix used by the progress argument.

For a selected row set `R`, project away the allowed columns and write the sampled matrix as

```text
A_R = [F_R | t_R].
```

A sampled left combination isolating the target exists exactly when

```text
rank([F_R | t_R]) > rank(F_R).
```

When the ranks are equal, there is a normalized right obstruction

```text
q = (x, 1),                 F_R x + t_R = 0.
```

It annihilates every selected projected row but evaluates to one on the target. Allowed columns
are assigned zero in `q`. For arguments involving rows not yet materialized, `q` is also extended
by zero to every forbidden column outside its finite support.

### Target-zero normalization

If the original target is `J(n+t)`, substitute `m=n+t` and replace every source offset `alpha` by
`beta=alpha-t`. The same relation is then searched with target `J(m)` and source rows

```text
G_(r,beta)(m) = sum_s c_{r,s}(m + beta) J(m + beta + s).
```

Thus every discovery task may use target shift zero and arbitrary signed source offsets. This is an
algebraic reindexing, not permission to ignore domains: target stratum, source conditions, guards,
sector crossings, and ordering roles must be pulled back and checked exactly. Same-sector ordering
is translation invariant only while both compared integrals remain in that sector. In particular,
the decorated target domain and the mapping from nonnegative chart coordinates to sampled integral
indices must be translated together with the source rows; running the existing origin-based chart
sampler unchanged would sample the wrong problem. An admitted target-zero result must be transported
back to the original target coordinates before semantic and outer-extension comparison.

## Inverse-incidence oracle

Let `supp(q)` be the finite target-plus-forbidden support of the current right obstruction. A
translated row can pair nontrivially with `q` only if one of its shifts equals a supported shift:

```text
u = alpha + s,              u in supp(q), s in S_r.
```

Consequently every potentially separating translated row is nominated by the finite enumeration

```text
for u in supp(q):
    for ordinary source r:
        for source shift s in S_r:
            nominate (r, alpha = u - s)
```

After stable deduplication, evaluate the complete residual

```text
rho_(r,alpha)(q)
    = sum_s c_(r,s)(n0 + alpha) q_(alpha + s)
```

at the current valid modular sample. Cancellation is decided only after summing the complete row.
A row whose support is disjoint from `supp(q)` has residual zero by construction, so the oracle
does not omit a row capable of cutting this particular `q`.

Rows with nonzero residual are retained even when they introduce new forbidden columns or do not
immediately change the rank difference. Such rows may be jointly necessary. A row with zero
residual in one epoch is not permanently marked irrelevant; a later obstruction can give it a
nonzero residual.

### Deterministic discovery loop

For one prime and one valid sample:

1. Begin with no selected rows and a logical target column. The first obstruction is the unit
   target functional, so the first epoch nominates exactly translations touching the target.
2. Build the selected projected matrix with forbidden columns in stable integral-key order and the
   target appended last.
3. If target rank separates, pass the hit to exact circuit recovery.
4. Otherwise use Symbolica's sparse row reducer and deterministic serial back substitution. Set the
   target free variable to one and every other free variable to zero, then verify `A_R q = 0` and
   `q_target = 1` explicitly. Validate that the target-last column is nonpivot before extracting
   this vector; rank equality predicts that fact but native output is not trusted implicitly.
5. Run the inverse-incidence enumeration and add every not-yet-selected valid row with nonzero
   residual. A zero-residual evaluation may be cached only under the current obstruction
   fingerprint; it is not a permanent `seen` mark and must be reconsidered for a later `q`.
6. Exhaustively classify every column introduced by retained rows, rebuild the target-local column
   registry and projected matrix by raw integral key, and repeat. Never carry a numeric column
   ordinal across that rebuild.
7. Only after the persistent nomination queue is empty, every structurally incident row was
   evaluable at this same sample, and no unseen row has nonzero residual, report a typed
   `SampledDeclaredModuleDual`. It is scoped to the declared source set, all its signed
   translations, this target partition, and this specialization. It is modular negative evidence,
   never a terminal or exact master claim.

A resource-capped implementation may place all nominated rows in a persistent stable queue and
consume fair batches. Exhausting a cap returns `BudgetExhausted`; it must not return “no rule.”
The `|W|` bound below counts exhaustive obstruction epochs. Fair micro-batches may take arbitrarily
many queue-consumption/rebuild steps before one such epoch has retained a cutting witness; fairness
alone does not give a `|W|` micro-batch bound.

Source-condition or coefficient-denominator failure makes the sample inapplicable to that row. It
must produce a typed retry and may enqueue a separately constructed exact exceptional-domain
obligation; the modular failure itself does not certify such a branch. Skipping a singular incident
row would invalidate `SampledDeclaredModuleDual`.

### Fixed-sample progress lemma

Fix:

- one finite field and one coefficient sample at which all relevant witness rows are defined;
- one target task identified by its decorated stratum/branch, ordering policy, immutable-owner
  snapshot, and fixed exact classification of every shift as allowed or forbidden; and
- the complete set of all signed translations of the declared ordinary source rows.

Assume a finite row set `W` in that specialized translated module has a left combination whose
forbidden coefficients vanish and whose target coefficient is one; allowed coefficients may remain.
At a no-hit epoch, extend the current finite-support `q` by zero to every unmaterialized forbidden
and allowed column. It annihilates every selected row and is one on the target. If it also annihilated
every row of `W`, pairing it with the assumed left combination would give `0 = 1`, since allowed
coefficients pair with zero. Therefore at least one row of `W` has nonzero residual against `q`.
That row intersects `supp(q)`, so the inverse-incidence enumeration nominates it. A selected row
already has zero residual, hence this witness row is previously absent. If every nonzero-residual
row is retained, each no-hit epoch adds at least one previously absent row of `W`, and a hit occurs
after at most `|W|` such epochs.

The progress measure is inclusion of witness rows, not monotone affine-nullspace dimension: adding
a row can introduce a new forbidden variable. This is why immediately neutral-looking rows cannot
be discarded.

The selected witness rows must all be valid simultaneously at the fixed sample. Exact recovery
retains the conjunction of every participating source gate; rows belonging to incompatible exact
branch identities must not be merged merely because their modular coefficients can be evaluated.

The lemma is a relative completeness statement for one specialized declared module. It does not
prove that:

- a fixed deterministic sample is generic;
- a modular hit lifts over `Q(d,n)`;
- a source row remains valid on a source-condition or guard-zero branch;
- an exact circuit is strictly descending or translation-stable; or
- an exact closing artifact or a practical K21 terminal set exists.

If a finite exact generic witness exists, sufficiently generic modular states should expose it, but
only exact lift and replay turn that expectation into a rule. Independent deterministic primes and
index/parameter points are discovery retries, not voting-based proof. Their selected row identities
may be unioned; null-vector supports must never be compared as invariant objects across samples.

## Geometry-driven target scheduling

The exact leading ideal and its disjoint `LatticeBox` complement already identify the current
uncovered geometry. Canonical standard pairs may later compress or deduplicate tasks, but they are
not required for the first implementation.

Schedule uncovered boxes by:

1. decreasing free dimension;
2. decreasing number of boxes at that dimension;
3. stable lower-corner and sector/guard identity; and
4. an anisotropic translation cost that makes free-direction motion cheaper than transverse halo
   motion.

Run a target-zero discovery task at the lower corner of the highest-priority box, with deterministic
generic sample values along its free coordinates. Exact circuits with the same leader and source
stratum enter one bucket. Compile their pivot guards jointly: several individually partial rules can
make a guard-total decision DAG. A bucket is retained when it improves exact structural coverage or
discharges a named exact guard branch. Only a guard-total, strict, cold-extended semantic rule may
add a leading-ideal generator.

After every exact admission batch, recompute the exact complement and discard stale modular
priority scores. Positive-dimensional guard-zero sets are separate algebraic obligations; standard
pairs describe the monomial leading ideal, not arbitrary affine or nonlinear guard varieties.

Production scheduling is bottom-up. Proper-subsector terms are allowed only through an immutable
lower-sector owner snapshot. The empty snapshot used by a diagnostic sweep is not a production
closure policy.

## Certificate boundary

| Layer | What it certifies | What it cannot certify |
| --- | --- | --- |
| Modular rank hit | a sampled candidate support | an exact relation |
| Modular right obstruction | which translated rows can cut the current sampled no-hit | a master or terminal |
| Incident exhaustion | a finite-support dual for every signed translation of the declared sources in one target partition and sample | a generic or full-IBP-module no-rule result |
| Exact Symbolica lift and regenerated-source replay | an exact source identity on its authenticated source gate | applicability or descent |
| Exact target unit and forbidden cancellation | a rule with only authenticated allowed descendants | guard totality |
| Semantic guard DAG and strict-order proof | applicability and termination on its exact stratum | translation-stable orthant ownership |
| Cold outer extension and immutable lower witnesses | one exact owner orthant | global closure |
| Exact leading-ideal complement with no free coordinate and every point explicitly terminal | finite, possibly nonminimal closure | terminal minimality or independence |

An optional rationally reconstructed dual would become exact negative evidence only after symbolic
verification against every incident translated source and every source gate. On a
positive-dimensional complement it is a signal to change sources, ordering, or strata—not an
infinite family of terminals. Finite terminals are admitted by exact finite-complement accounting,
never by modular obstruction sampling.

Multiplying a gated rational relation by a denominator does not erase its intrinsic source gate.
Only an independently replayed ungated polynomial identity can do that. Similarly, a unit ideal
among several target-pivot polynomials covers their target-pivot branches only under any common
intrinsic source gate; the gate-zero branch remains a separate obligation.

## RustRed integration

The existing exact proof path should remain authoritative. The new work belongs before it.

### Selected source plan

This boundary is implemented. `OneSidedChartFrame::try_new` retains the rectangular
one-sided schedule, while `SelectedSourceFrame::try_new` consumes only exact records of the form:

```text
TranslatedSourceRequest {
    source_ordinal,
    signed_offset,
}
```

Provenance plus signed offset is the stable identity. Total translation degree is a scheduling
cost, not identity. The selected plan must not translate all `L^2` ordinary rows merely because one
row at an offset was nominated.

`ParametricIbpGenerator::translate_completed_source_rows` remains the rectangular API and expands
**every** completed ordinary source at every supplied offset. The implemented
`translate_selected_completed_source_rows` instead translates only the canonical deduplicated
`(source_ordinal, signed_offset)` request set through the same Symbolica-backed relation translation
and exact `TranslatedSourceProvenance`. A modular scout may later compile/cache coefficient evaluators for cheap
`(source, offset, sample)` evaluation, but every retained exact row is regenerated through that
normal identity boundary before lift and replay.

### Obstruction service

This boundary is also implemented. The projected sparse reduction in
`crates/rustred-core/src/foundry/completion/frame/modular/rank.rs` returns either a target
hit or a checked `ModularRightObstruction`. It uses Symbolica's `SparseRowReducer`, `U`, pivots, and
serial `back_substitute`; no independent CAS or generic elimination engine is needed. Target-last
projection makes the canonical normalized kernel vector a thin extraction from the RREF. Preserve
the current preflighted fill bounds, native-panic boundary, and postvalidation: Symbolica provides
the elimination primitive, not RustRed's resource or trust policy.

The obstruction numbers every canonical forbidden key first and the target last, retains an
explicit logical-to-physical map, normalizes `q_target = 1`, and replays `A q = 0` before returning.

Useful private components are:

- `OrdinarySourceIncidenceIndex`: source term shifts and source ordinals;
- `TranslatedSourceRequest`: one ordinary row and one signed offset;
- `ModularRightObstruction`: field/sample identity, ordered support, and verified normalized `q`;
- `IncidentTranslationNominations`: stable inverse-incidence nomination without proof authority;
  and
- `SelectedSourceFrame`: construction-neutral CSR and exact provenance for only retained requests.

`ModularPhysicalFrame`, `ModularHit`, `TargetColumnPartition`, exact lift, and replay now consume the
construction-neutral `PhysicalFramePlan` nested by either scheduling shell. The shared assembler
retains exact source chronology and raw columns without pretending a sparse selection is a
rectangle. Pointer and sealed-identity joins remain strict across every proof-bearing boundary.

The full-residual filter and immutable fresh-plan epoch are implemented. Nominations are sealed to
their exact ordinary-source incidence index and either a target-unit bootstrap or one exact checked
obstruction query; only the latter may enter residual pairing. The evaluator materializes every
source condition, coefficient denominator, and source term before pairing the supported columns.
Every stable augmentation is then translated anew, assembled into a pointer-distinct physical
plan, exhaustively repartitioned under the fixed stratum/order/lower-owner snapshot, and resampled
from the original integer probe inputs. Empty residual batches still authenticate family, context,
layout, and row chronology. Neither empty nor unchanged batches carry sampled-dual authority.

The active work is the bounded outer scheduler. It must bootstrap the target-unit requests, execute
fresh query epochs in deterministic probe order, union every obstruction-bound nonzero residual
request in stable raw identity order, discard all stale plan-local ordinals and witnesses before a
rebuild, and invoke exact lift and replay on an actual hit while its epoch is alive. Exhaustive
sampled-dual and resource-stop outcomes must remain typed discovery evidence with no terminal or
closure authority.

After that compatibility boundary exists and rank separates, reuse the current modular-hit support
selection, exact lift, fraction-free source replay, semantic admission, cold outer extension, and
owner-cover compiler. Raw physical integral shifts remain matrix columns; symmetry may canonicalize
whole target tasks or transport authenticated exact results, but it must not delete independent rows
inside a fixed-target solve.

### Determinism and parallelism

- Derive an ordered prime/sample schedule from the family fingerprint and task key and record every
  actual value and rejection.
- Sort rows by exact source provenance and signed offset; sort forbidden columns by raw shift and
  append target last.
- Use a canonical serial RREF for obstruction selection. Parallel RREF row permutations must not
  alter the discovery artifact.
- Keep selected-row sets and reducer state sample-local. A global request pool may union stable row
  identities discovered by several samples, but a row singular at one sample cannot be inserted
  into that sample's matrix merely because another sample admitted it.
- Evaluate independent standard-pair/sample tasks against immutable source skeletons and lower
  owners. Merge nominated requests and exact results in stable key order.
- Share support/provenance data read-only. Each prime owns only its modular values and bounded
  reducer state; no worker copies the full translated universe.

## Resource and scaling model

For a complete vacuum family at loop count `L`, `K=L(L+1)/2` and the conventional ordinary-source
count is `L^2`. Blind one-sided total-degree rows and signed-radius-three controls are:

| K | loops | sources | degree 2 | degree 3 | degree 4 | signed radius 3 |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 6 | 3 | 9 | 252 | 756 | 1,890 | 3,393 |
| 10 | 4 | 16 | 1,056 | 4,576 | 16,016 | 24,976 |
| 15 | 5 | 25 | 3,400 | 20,400 | 96,900 | 124,775 |
| 21 | 6 | 36 | 9,108 | 72,864 | 455,400 | 478,332 |

At K21, signed radii two, three, and four contain 33,300, 478,332, and 5,167,044 shifted rows.
They are controls, not default materialization plans.

Let:

- `T` be the total term occurrences in all base sources;
- `p=|supp(q)|`;
- `R` be retained translated rows;
- `C` be retained target-plus-forbidden columns; and
- `w` be average retained row width.

One oracle epoch enumerates at most `p*T` source-term incidences before request deduplication. That
is only the nomination cost. A naive implementation which then evaluates every distinct nominated
request by scanning its complete row can cost as much as `O(p*T*w)` field operations in the same
epoch; an incidence/convolution cache may reduce the measured constant but does not change the
soundness boundary. Retained input nonzeros are approximately `Z=R*w`; memory is
`O(Z + sparse elimination fill + R + C)`. Costs accumulate over all obstruction epochs and modular
states. The general structural bound

```text
T <= L^2 (K^2 + K + 1)
```

is 16,668 at K21, while measured K6 has `T=90`. Actual K10 and K15 support and obstruction density
must be measured before extrapolating to K21. A dense `q`, large new-forbidden frontier, or sparse
factorization fill can erase the incidence advantage. Nothing in the progress lemma gives an
asymptotic row, column, fill, or epoch bound better than the finite witness it assumes.

### Provisional experiment envelopes

These budgets stop a research lane; they never prove absence of a rule.

| K | retained rows per target | retained projected columns | fill multiple | memory per modular task | wall time |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 6 | 256 | 4,096 | 20x input | 2 GiB | 2 min |
| 10 | 2,048 | 16,384 | 20x | 8 GiB | 10 min |
| 15 | 8,192 | 65,536 | 20x | 32 GiB | 30 min |
| 21 | dry-run first | model-derived only | 20x | require 2x headroom | no closure launch yet |

Do not launch a production K21 solve until K10 and K15 fix a predictive model for selected rows,
candidate scans, projected columns, fill, exact coefficient size, and guard branching. A first K21
dry run should remain below roughly 32,768 selected rows—about seven percent of the
signed-radius-three control—and have twice its predicted memory available.

### Kill gates

Return a typed budget result and pause this lane when any of the following holds:

- selected rows approach ten percent of the signed-radius-three control without a stronger exact
  complement reduction than the best shell or target-directed control;
- `p*T`, projected columns, or selected rows grow superlinearly from K6 through K15;
- Symbolica fill exceeds 20 times retained input nonzeros or the admitted memory envelope;
- exact lift/replay coefficient degree, integer size, term count, or replay time dominates modular
  discovery;
- held-out primes repeatedly expose unstable supports or bad-specialization hits;
- guard branches grow faster than exact owner-cover improvement;
- two complete geometry epochs fail to reduce the maximal uncovered free dimension, its
  multiplicity, or a named exact guard obligation; or
- K15 lies outside the frozen K6/K10 scaling interval.

Passing a gate is evidence for the next benchmark only. K6 success is not a K21 complexity proof.
Benchmark the full `L^2` source set against any smaller Lie-generated source set; the latter may
reduce `T` but must not silently weaken the declared translated module.

## Mutants and regression cases

The first implementation must kill all of the following mutants.

1. **Individually neutral, jointly separating.** With columns `[f,t]`, rows `[1,1]` and `[1,2]`
   do not separate individually but do jointly.
2. **Helper chain.** With `[f1,f2,t]`, rows `[1,0,1]`, `[1,1,0]`, and `[0,1,0]` require target
   contact followed by two forbidden-only helpers.
3. **Neutral-row reconsideration.** Starting from `t+f1=0`, `f2=0` is initially neutral;
   after adding `t+f2=0`, it becomes necessary.
4. **New forbidden frontier.** Two rows sharing a newly introduced forbidden column close only as a
   pair; immediate rank-gain filtering must fail.
5. **Incidence exhaustiveness.** On a small family and signed ball, brute force and the oracle must
   agree on every nonzero residual candidate.
6. **Target normalization.** Reindex a known target-`t` circuit to target zero and require identical
   exact replay, descendant roles, and pulled-back guards.
7. **Bad-prime false hit.** A specialization dropping `rank(F)` too far may nominate a hit; exact
   lift must reject it without creating an owner.
8. **Sample-specific zero residual.** A true row invisible at one point is nominated by another
   deterministic modular state; the first state is not negative proof.
9. **Multidimensional right kernel.** Different primes or pivot orders can yield different `q`
   supports; code must not compare those supports as invariant.
10. **Physical-order target pivot.** Put the raw target key between two forbidden keys. Reusing the
    current physically sorted projection must fail the target-free precondition; the target-last
    logical projection must still return and explicitly verify the normalized obstruction.
11. **Singular source.** A source condition or denominator zero produces retry/branch, not a zero
    row or incident-exhaustion certificate.
12. **Unowned lower sector.** Mutating an unowned proper-subsector column from forbidden to allowed
    must fail exact partition validation.
13. **Partial target guard.** A rule with target pivot `n` must not cover `n=0`.
14. **Shared intrinsic gate.** Target pivots `n` and `n-1` under common source gate `n!=0` do not
    cover the gate-zero branch merely because their pivot ideal is the unit ideal.
15. **Symmetry row deletion.** Two orbit-related but independent rows are jointly needed; task
    transport is permitted, blind row-orbit deletion is not.
16. **Sampled-terminal mutation.** `SampledDeclaredModuleDual`, repeated modular no-hits, and stable
    ranks must be rejected as terminal evidence.
17. **Finite nonminimal closure.** A finite complement with several explicit terminals closes;
    deleting any one terminal produces `FiniteTerminalOwnership` failure.
18. **Determinism.** Worker counts and sample-task completion order produce byte-identical admitted
    artifacts after rows are canonicalized into the prescribed provenance/offset order. Deliberately
    permuted raw insertion order must either be canonicalized before Symbolica or produce the same
    post-canonical result; arbitrary native pivot chronology is not an artifact identity.
19. **Leave-one-owner-out.** Removing a known K6 owner must cause rediscovery of an exact-equivalent
    rule within the declared K6 envelope, a typed budget result, or an honest sampled obstruction.
    This is a sensitivity diagnostic, not a theorem that every removed lower-sector dependency is
    reproducible from the same-sector translated module.

## First implementation experiment

1. Use the canonical S4a fixture to test the oracle at target zero on the highest-priority current
   uncovered box. Begin with no rows; do not seed a degree shell.
2. Run at least three deterministic valid modular states. Add every residual-cutting request to a
   stable shared request set, while retaining per-state validity and sample-local selected matrices.
3. On the first rank hit, reuse exact support recovery and replay. Run deterministic alternative
   sample/row schedules to seek additional target pivots for guard-totality.
4. Compare retained rows, columns, nonzeros, fill, exact coefficient sizes, guards, and exact
   complement delta with degree-one, degree-two, signed-radius, and exact backward-incidence
   controls.
5. After the fixture proves the mechanics, run production sectors bottom-up with immutable lower
   owners and repeat the exact geometry calculation after every admission batch.

Acceptance is an exact drop in uncovered free dimension and ultimately zero positive-dimensional
complement, not a modular hit count. Every remaining finite point must be emitted as an explicit
typed terminal before an artifact can be called closed.

## Primary sources and the inference boundary

- LiteRed solves identities at a general point, shifts the selected leader back to `J(n)`, analyzes
  applicability, and searches neighboring points on uncovered exceptional strata
  ([arXiv:1212.2685](https://arxiv.org/abs/1212.2685),
  [arXiv:1310.1145](https://arxiv.org/abs/1310.1145)). This supports target normalization and exact
  exceptional-branch handling, not the inverse-incidence progress lemma.
- IBPs form a finitely generated left ideal in a rational double-shift algebra, and bounded
  shift-operator ansatze can be solved by function-field linear algebra
  ([arXiv:2210.05347](https://arxiv.org/abs/2210.05347)). This supports the matrix formulation, not
  a universal finite neighborhood.
- Standard pairs encode monomial-ideal complements and support effective decomposition algorithms
  ([arXiv:2005.10968](https://arxiv.org/abs/2005.10968)). They schedule structural holes; they do
  not solve general polynomial guard-zero sets.
- FiniteFlow establishes scalable finite-field sampling and reconstruction as a discovery
  technique ([arXiv:1905.08019](https://arxiv.org/abs/1905.08019)).
- Blade uses numerical reductions and ansatze to discover compact block-triangular systems
  ([arXiv:2405.14621](https://arxiv.org/abs/2405.14621)). It does not make modular support an exact
  IBP certificate.
- Generating-function symbolic reduction organizes rules and completeness through induced lattice
  shifts and uncovered lattice points ([arXiv:2605.09541](https://arxiv.org/abs/2605.09541)). Its
  high-loop scaling and complete guard treatment remain open implementation questions.
- The Lie-algebraic structure of IBP generators motivates benchmarking a smaller vacuum generating
  set against the full `L^2` rows ([arXiv:0804.3008](https://arxiv.org/abs/0804.3008)). Exact module
  equivalence or end-to-end closure must decide whether that reduction is safe.

The inverse-incidence oracle, its target-zero use inside RustRed, and the fixed-sample progress
lemma above are new design inferences of this research programme. Their correctness scope is the
declared specialized row module; only RustRed's existing exact replay and owner-cover boundaries
may authorize a shipped artifact.
