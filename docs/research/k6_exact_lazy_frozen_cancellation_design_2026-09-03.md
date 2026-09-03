# ELC1 exact-lazy augmented Ore cancellation design — 2026-09-03

## Status and decision

This note specifies the smallest authoritative **in-memory** exact-lazy
experiment after ELC0.  ELC1 imports one exact subject and one immutable exact
Janet division epoch, selects the greatest exactly supported Janet-reducible
term, performs one exact lazy left-Ore cancellation, and lowers the result for
independent comparison with the existing rational and projective paths.

ELC1 is authority for the support and algebraic identity of that one retained
lazy consequence.  It is not a Janet completion epoch, queue-exhaustion proof,
rule, checkpoint format, or artifact-publication boundary.  No conversion from
an ELC1 value to an artifact should exist.

The first implementation should use the existing **rational coefficient DAG
with monic frozen divisors**.  It should not wait for a combined projective
polynomial DAG:

- every current exact Janet divisor is already exactly monic;
- one normal-form cancellation needs only exact-leaf ingress, translation,
  negation, multiplication, and addition;
- it introduces no new inverse node;
- ELC0 already evaluates and exactly materializes these rational DAG
  operations iteratively through Symbolica; and
- `a + (-a) * sigma_delta(1)` cancels structurally, while every other
  coefficient can remain unexpanded until support classification or lowering.

The projective polynomial representation remains a measured alternative.  It
may win once basis insertion requires repeated monic normalization or if
rational leaf/materialization costs dominate, but combining it with ELC1 now
would conflate two hypotheses and delay the quickest K6 falsifier.

## Exact authority contract

An admitted ELC1 consequence must satisfy all of the following.

1. Every retained physical shift has one owned coefficient root and a proof
   that this exact rational function is nonzero.  Therefore its sorted shift
   set is exact support, not sampled support.
2. A physical term is absent only because its DAG root is structurally zero or
   because Symbolica materialized that same owned root and proved it exactly
   zero.  Any number of zero finite-field images remains inconclusive.
3. The complete source-module identity is retained by a whole-consequence
   derivation DAG.  The physical row and derivation receive the same multiplier,
   Ore translation, and addition in one transaction.
4. Localization is retained as exact guard descriptors.  A modular probe can
   reject a point at a guard zero, but a probe value is never itself an
   application guard.
5. The selected target is the greatest **reducible** exact-support term.  It
   need not be the overall row leader.  Successive committed selected targets
   strictly decrease in the frozen exact Ore order.
6. The selected target must become the structural DAG zero before support
   classification.  A sampled-zero target is never accepted as cancellation.
7. The division epoch, ordering/action owner, indexed-context fingerprint,
   source-module owner, coefficient-arena generation, limit contract, and
   support proofs all agree.
8. Every failure leaves the prior consequence and selection witness unchanged.
   Attempted work remains charged to one caller-owned cumulative budget.

The core one-sided theorem used here is exact: if evaluation of an owned
rational-function circuit is defined at a prime-field point and has nonzero
image, then that circuit is not the zero rational function.  The converse is
false and is never used.

## Data flow

```text
authenticated exact subject + frozen exact Janet division epoch
                    |
                    | exact-leaf ingress once
                    v
       exact-lazy session and restricted coefficient DAG
                    |
                    | classify complete physical support
                    v
       classified row + exact derivation + guard lineage
                    |
                    | exact Janet-index selection
                    v
       transactional monic lazy AXPY cancellation
                    |
                    | deterministic batched probes, then exact fallback
                    v
          admitted exact-lazy augmented consequence
                    |
                    | cold Symbolica lowering and source replay
                    v
      exact rational / projective differential only in ELC1
```

## Minimal module layout

Keep the new types private under the existing coefficient-circuit owner so
ELC1 can reuse ELC0 without broadly widening APIs:

```text
involutive/modular/exact_lazy/
    mod.rs
    error.rs
    limits.rs
    arena.rs
    model.rs
    support.rs
    provenance.rs
    guards.rs
    normal_form.rs
    lower.rs
    tests.rs
```

`modular::normal_form` remains proposal-only sampled guidance.  The new child
module is a separate exact layer over the field-independent ELC0 circuit.  The
top-level `modular` module documentation must be updated when ELC1 lands so it
does not falsely describe every descendant as proposal-only.

This nested placement is intentionally smaller than immediately moving ELC0
into a new shared crate module.  If ELC1 survives its K=3/K6 gates, the neutral
coefficient-circuit files can then be lifted mechanically without changing
their semantics.

## Restricted exact-lazy arena

`ExactLazyArena` owns `ModularCoefficientDag` behind a narrower interface:

```rust,ignore
struct ExactLazyArena {
    coefficient: ModularCoefficientDag,
    derivations: SourceDerivationArena,
    guard_lineages: GuardLineageArena,
    owner: ExactLazyOwner,
    limits: ExactLazyLimits,
}

impl ExactLazyArena {
    fn try_exact_leaf(... ) -> Result<LazyCoeff, ExactLazyError>;
    fn zero(&self) -> LazyCoeff;
    fn one(&self) -> LazyCoeff;
    fn try_neg(&mut self, value: &LazyCoeff, ...) -> Result<LazyCoeff, _>;
    fn try_add(&mut self, left: &LazyCoeff, right: &LazyCoeff, ...)
        -> Result<LazyCoeff, _>;
    fn try_mul(&mut self, left: &LazyCoeff, right: &LazyCoeff, ...)
        -> Result<LazyCoeff, _>;
    fn try_translate_by_operator(&mut self, value: &LazyCoeff, shift: &ForwardShift, ...)
        -> Result<LazyCoeff, _>;
    fn try_transaction(&mut self, ...) -> Result<ExactLazyTransaction<'_>, _>;
}
```

There is deliberately no ELC1 `inv` or `div` method.  The raw scout DAG's
unchecked inverse constructor must not be reachable from exact-lazy code.
Future monic normalization may add an evidence-consuming inverse whose input
is an owned exact-nonzero proof and whose numerator condition is appended to
the guard lineage, but that is outside ELC1.

`LazyCoeff` wraps an ELC0 `CoeffRef` plus the exact-lazy arena owner.  Callers
cannot construct one from node ordinals.  `ExactLazyOwner` binds the DAG
owner/generation, action identity, source-module authority, context
fingerprint, arity, and immutable limits contract.

An `ExactLazyTransaction` checkpoints all three arenas.  It is the only code
allowed to call ELC0 rollback.  Dropping or failing the transaction rolls live
storage back; committing advances the retained floor.  No later transaction
can roll back behind a root named by an admitted support proof.  ELC0's
monotone node/delta incarnation counters continue to prevent stale-slot
resurrection, while cumulative creation/work counters are never rolled back.

ELC1 is append-only between transactions and performs no compaction.
Compaction needs a new arena generation plus transactional proof rebinding and
is an ELC2/ELC3 experiment, not a prerequisite for one cancellation.

## Exact-lazy consequence and type-state support

Use separate unclassified and admitted row types:

```rust,ignore
struct LazyOreTerm {
    shift: ForwardShift,
    coefficient: LazyCoeff,
    nonzero: ExactNonzeroProof,
}

struct ClassifiedLazyOreRow {
    terms: Box<[LazyOreTerm]>, // strictly shift-sorted, exact nonzero support
}

struct PendingLazyOreTerm {
    shift: ForwardShift,
    coefficient: LazyCoeff,
    prior_proof: Option<ExactNonzeroProof>,
}

struct UnclassifiedLazyOreRow {
    terms: Box<[PendingLazyOreTerm]>,
    structural_zero_elisions: Box<[StructuralZeroProof]>,
}

struct ExactLazyConsequence {
    owner: ExactLazyOwner,
    row: ClassifiedLazyOreRow,
    derivation: SourceDerivationRef,
    guards: GuardLineageRef,
    census: ExactLazyPayloadCensus,
}
```

Only `ClassifiedLazyOreRow` exposes support iteration or a leading-term API.
Sparse AXPY produces `UnclassifiedLazyOreRow`; it cannot be queried by the
Janet scheduler.  Classification consumes it and either returns a complete
classified row or fails the surrounding transaction.

Unchanged subject terms retain their existing proof.  Newly transformed or
collision coefficients are reclassified.  The first implementation may
classify all changed roots; sound algebraic propagation through negation,
translation, and products can be added later if measurements justify the
additional proof graph.

### Nonzero and zero proofs

```rust,ignore
enum ExactNonzeroProof {
    ExactIngress(OwnedExactLeafNonzero),
    Modular(OwnedCertifiedNonzero),
    ExactFallback(OwnedExactNonzeroMaterialization),
}

enum ExactZeroProof {
    Structural(OwnedStructuralZero),
    ExactFallback(OwnedExactZeroMaterialization),
}
```

The higher-level wrappers bind the ELC0 root proof to the exact-lazy owner and
Ore action.  `CertifiedNonzero` currently binds DAG/context/root/probe; that is
necessary but not sufficient by itself for an exact-lazy row.

An exact fallback consumes `ExactMaterialization` only after checking its
owned root and exact zero/nonzero value.  There is no constructor from a bool,
residue, hash, or telemetry record.  Exact-zero proofs for removed
nonsyntactic terms remain in the committed cancellation record so the support
transition is independently auditable.

## Whole source-module provenance

Do not eagerly transform every sparse provenance coefficient on every lazy
cancellation.  The measured K6 prefix already had 615 provenance entries
versus 187 physical entries, and expanded provenance dominated coefficient
payload.

Retain a whole-consequence operation DAG instead:

```rust,ignore
enum SourceDerivationNode {
    Imported {
        terms: Box<[ImportedSourceTerm]>,
    },
    LeftAxpy {
        accumulator: SourceDerivationRef,
        multiplier: LazyCoeff,
        operator_shift: ForwardShift,
        source: SourceDerivationRef,
    },
}

struct ImportedSourceTerm {
    source_ordinal: usize,
    left_shift: ForwardShift,
    left_coefficient: LazyCoeff,
}
```

An imported node is built only from an authenticated `OreConsequence`; its
source ordinals are checked by the exact ordering/source owner.  A cancellation
adds exactly one `LeftAxpy` node in the same transaction as the physical row.
It means, without expansion,

```text
P_new = P_accumulator + multiplier * E^operator_shift P_source.
```

The lowering interpreter is iterative and bounded.  It expands a selected
derivation into a canonical sorted sparse map only at the differential or an
eventual exact boundary, applying the same coefficient translation,
multiplication, shift addition, and merge as the physical operation.  It then
materializes the resulting coefficient roots in one batch and replays the
original source chronology exactly.

This is complete source-module provenance, not a row-only checksum.  It avoids
the hot-path coefficient expansion while retaining all source ordinals, left
shifts, and exact multipliers needed for replay.

## Exact guard descriptors

Keep guard semantics conservative and exact while delaying expansion:

```rust,ignore
enum ExactGuardDescriptor {
    Polynomial(LazyCoeff),      // authenticated denominator-one polynomial
    DenominatorOf(LazyCoeff),   // exact denominator condition of a rational root
}

enum GuardLineageNode {
    Imported(Box<[ExactGuardDescriptor]>),
    LeftAxpy {
        accumulator: GuardLineageRef,
        translated_source: GuardLineageRef,
        physical_delta: PhysicalDeltaId,
        multiplier_denominator: ExactGuardDescriptor,
    },
}
```

Existing `LocalizationWitness` polynomials enter as denominator-one exact
leaves.  Translating a source lineage applies the exact signed physical Ore
translation to every descriptor.  The normal-form multiplier is the negated
target coefficient, so `DenominatorOf(multiplier)` reproduces the current
exact AXPY localization policy without materializing it in the hot path.

At lowering, Symbolica materializes descriptor roots.  `Polynomial` requires
a denominator of one; `DenominatorOf` uses the indexed context's existing
denominator-condition operation.  The resulting polynomials enter the
existing `LocalizationWitness` canonicalization/deduplication boundary.  A
unit condition disappears there.  No finite-field residue becomes a retained
guard.

For probe admissibility, a probe evaluates the exact polynomial descriptors
and rejects any zero.  Evaluating a `DenominatorOf` root already rejects a zero
exact-leaf denominator.  This is merely point selection; the descriptor
lineage remains the exact authority.

## Deterministic batched support classification

Per-root `CertifiedNonzero::try_replay` would rebuild a probe and cache for
every coefficient.  ELC1 must instead classify the whole changed sparse row
in batches.

### Probe schedule

`ExactLazyProbeSchedule::try_new` takes a semantic scope and a bounded sorted
list of `ProbeSpec { ordinal, modulus, full_integer_point }`.  Construction:

- canonicalizes each point through `ModularProbe`;
- requires strictly increasing unique ordinals;
- rejects residue-equivalent `(prime, point)` tasks;
- validates complete base-parameter plus index arity; and
- binds the schedule to the exact-lazy owner and immutable limits.

The initial ELC1 schedule is a small fixed deterministic table used by tests.
The eventual generated schedule may hash the semantic scope, but no random or
completion-order choice enters authority.

### One batch per probe, not one probe per root

Add a private ELC0 batch-certification seam:

```rust,ignore
fn try_certify_batch(
    dag: &ModularCoefficientDag,
    context: &IndexedCoefficientContext,
    guard_roots: &[CoeffRef],
    coefficient_roots: &[CoeffRef],
    spec: &ProbeSpec,
    limits: ModularGuideLimits,
) -> Result<CertifiedEvaluationBatch, RejectedProbeReport>;
```

It constructs one probe, evaluates all guards and then the complete ordered
root batch while retaining one translated-value cache, and releases no image
unless the entire batch succeeds.  `CertifiedEvaluationBatch` owns the exact
query list, DAG owner, context fingerprint, complete probe identity, images,
and census.  A consuming constructor may issue root-bound
`CertifiedNonzero` values only for nonzero images in that same batch.  It does
not accept caller-provided scalar residues and does not replay each root in a
fresh cache.

For every probe ordinal, the classifier evaluates only roots still unresolved
at the start of that ordinal, in canonical shift order.  It records the first
valid nonzero witness by **lowest schedule ordinal**, regardless of worker
completion order.  ELC1 should execute probes serially first; later parallel
execution collects complete outcomes by ordinal before committing decisions.

A singular guard, exact-leaf denominator, inverse, or resource stop rejects
that complete probe outcome.  A valid sampled zero leaves the root unresolved.
After the schedule:

- structurally zero roots already have exact zero proofs;
- roots with a valid nonzero image receive the owned modular proof; and
- every remaining root is sent to exact Symbolica materialization.

Exact fallback returns either exact zero (remove the term and retain its proof)
or exact nonzero (retain the term and proof).  A fallback resource stop aborts
the transaction; it never commits `Unresolved` support.

The exact fallback should also gain an all-or-nothing batch materializer so
one iterative postorder cache serves every unresolved root.  Scalar fallback
is acceptable only for the first synthetic bring-up, not for the K=3 gate.

## Frozen-epoch cancellation algorithm

`ExactLazyFrozenNormalFormCursor` borrows one `JanetDivisionEpoch`, owns one
admitted lazy subject, one divisor scratch buffer, and the previous selected
target key.  It also borrows the caller-owned work budget for its lifetime.

The ELC1 operation is:

1. Validate the epoch/action/context/session/limits bindings and exclusion.
2. Iterate the exact classified subject support.  Query the existing frozen
   Janet divisor index for each shift and choose the greatest reducible key,
   preserving the exact lowest-divisor-ordinal rule and logical visit census.
3. Require the selected key to be lower than the last committed selected key.
   A larger irreducible row term may remain.
4. Fetch the exact basis element at the selected ordinal.  Recheck its birth
   ordinal, leading shift, and exact monic coefficient; use its cached imported
   lazy consequence under the same session.
5. Compute `operator_shift = target - divisor_leader` with checked exact shift
   arithmetic.
6. Begin one arena transaction.  Set `multiplier = -target_coefficient` and
   build the sparse physical candidate

   ```text
   subject + multiplier * E^operator_shift divisor.
   ```

7. Require the target root to be the structural DAG zero and remove it.  A
   nonzero, sampled-zero, or merely absent target is a typed invariant failure.
8. In the same transaction, append the matching `SourceDerivationNode::LeftAxpy`
   and guard-lineage node.
9. Batch-classify every changed physical root.  Remove only exact zeros and
   reject the transaction if any root remains unresolved.
10. Validate sorted unique support, all proof ownership, exact target absence,
    payload limits, and the step trace.  Commit all arenas atomically, then set
    the previous-target witness.

The public-private API should look like:

```rust,ignore
impl<'epoch, 'budget> ExactLazyFrozenNormalFormCursor<'epoch, 'budget> {
    fn try_new(
        subject: ExactLazyConsequence,
        epoch: &'epoch JanetDivisionEpoch,
        excluded_divisor: Option<usize>,
        session: &'epoch mut ExactLazySession,
        budget: &'budget mut ExactLazyWorkBudget,
    ) -> Result<Self, ExactLazyError>;

    fn try_cancel_once(&mut self)
        -> Result<ExactLazyCancellationOutcome, ExactLazyError>;
}

enum ExactLazyCancellationOutcome {
    Irreducible,
    Reduced(ExactLazyReductionStep),
}
```

Do not accept a caller-selected target in the authoritative API.  A separate
test-only replay method may accept an expected target/divisor tuple and must
verify it against the cursor's independent exact selection.

## Exact lowering and differentials

`try_lower_for_exact_replay` is a cold, all-or-nothing boundary:

1. materialize the complete physical root batch;
2. iteratively expand the source-derivation DAG under provenance limits;
3. batch-materialize canonical provenance roots;
4. materialize and canonicalize every guard descriptor through Symbolica;
5. construct an `OreConsequence` only through a new restricted constructor
   that authenticates row, provenance, localization, action, source ordinals,
   arity, context, and complete coefficient census; and
6. independently regenerate the physical row from the original ordinary
   sources and compare every shift/coefficient exactly.

The constructor is an exact validation boundary, not an unchecked field
assembler.  It belongs next to `OreConsequence` and remains visible only
inside involutive completion.

For the projective differential, clear the same exact subject/divisor through
`PrimitiveOreConsequence`, perform its GCD-scaled pseudo-reduction, and compare
the complete materialized lazy row and provenance projectively.  Because the
exact frozen divisor is monic, the rational-DAG result should also agree
exactly with the ordinary rational AXPY.

## Cumulative limits and accounting

`ExactLazyLimits` embeds the exact ELC0 circuit, probe, exact-materializer, and
involutive divisor contracts and adds:

- maximum imported physical and provenance terms;
- live and cumulative physical row terms;
- source-derivation nodes, edges, imported terms, and shift-coordinate cells;
- guard-lineage nodes, edges, descriptors, and translated coordinate cells;
- scheduled, valid, singular, and resource-rejected probes;
- cumulative batched probe queries, evaluation steps, frame pushes, cache
  entries, leaf terms, and exponent cells;
- support-classification attempts and roots;
- exact fallback batches and roots;
- retained modular and exact zero/nonzero proofs;
- cancellation attempts, committed cancellations, normal-form steps, logical
  divisor visits, divisor-index operations, and trace bytes;
- lowering derivation visits, expanded provenance entries, materialized roots,
  output terms/exponent cells/bytes; and
- transaction live storage and cumulative arena churn.

`ExactLazyWorkBudget` binds one immutable `ExactLazyLimits` value and is held
for the cursor/session lifetime.  Attempts are charged before work.  Failed
transactions do not reduce counters.  Live DAG storage may roll back, while
ELC0's total-node/translation/leaf creation counters remain monotone.

Current rejected modular probes discard their census with the error.  ELC1
needs either `RejectedProbeReport { error, census }` or a conservative
precharge of the complete per-probe envelope.  Returning a report is preferred
because K6 viability depends on measured work as well as a formal cap.  No
partial coefficient images may accompany the report.

Use checked integer arithmetic and fallible reservations for every retained
vector/map.  Symbolica owns all rational arithmetic, exact polynomial work,
and finite-field evaluation.  RustRed owns only DAG/row orchestration,
ordering, provenance, guard descriptors, work policy, and authority binding.

## Precise gaps after ELC0

| Gap | Minimal ELC1 correction | Blocks one cancellation? |
|---|---|---:|
| The ELC0 DAG exposes raw `inv/div` inside the modular owner | Wrap it in `ExactLazyArena`, exposing no inverse in ELC1 | Yes, for a safe authority boundary |
| `CertifiedNonzero::try_replay` rebuilds a probe cache per root | Add consuming batch replay/certification over a bound ordered query list | Yes, for meaningful sparse-row scaling |
| Probe errors discard attempted-work census | Return a census-only rejected report, with no images | Yes, for exact cumulative accounting |
| Exact materialization is scalar and its memo table is per attempt | Add an all-or-nothing multi-root materializer sharing one iterative cache | No for first synthetic test; yes for K=3 |
| ELC0 certificates do not bind Ore action/session | Wrap them in owner-bound exact-lazy proof types | Yes |
| There is no exact-zero proof type | Consume root-bound exact materialization into zero/nonzero wrappers | Yes |
| General arena rollback can invalidate certified roots | Restrict checkpoint/rollback to an exact-lazy transaction and committed floor | Yes |
| `ModularOreRow` has no provenance and uses sampled support | Do not reuse it; add exact-lazy classified/unclassified augmented types | Yes |
| Exact lazy guards have no descriptor/lowering type | Add polynomial/denominator descriptors and a lineage DAG | Yes |
| `OreConsequence` has no restricted derived-parts constructor | Add a fully authenticating cold-lowering seam | Yes for exact differential/lowering |
| `DagOwner` is not a persistent compaction generation | Keep ELC1 uncompacted and in-process; add generation rebinding later | No |
| Janet epochs retain exact coefficient rows | Borrow the exact `JanetDivisionEpoch` and reuse its coefficient-free index | No; this is ideal for ELC1 |
| There is no lazy basis insertion/monic normalization | Explicitly out of ELC1 scope | No |

The existing coefficient-free `JanetMonomialView`, shared mask construction,
and indexed divisor queries are sufficient.  ELC1 must not create a parallel
Janet-divisibility implementation.

## Required tests

### Authority and classification

- One unlucky sampled zero followed by a lower-ordinal valid nonzero witness
  (under deliberately permuted worker completion) chooses the same schedule
  ordinal deterministically.
- All scheduled images zero for an exact nonzero root triggers Symbolica
  fallback and retains the term.
- A nonsyntactic exact zero is removed only after Symbolica fallback.
- A pole or zero guard rejects the whole probe batch without releasing partial
  images or changing support.
- A structurally zero target cancels without a probe; a merely sampled-zero
  target is rejected.
- Wrong DAG/session/action/context/epoch/limits/query ordering and stale roots
  fail with typed errors.
- Exact fallback exhaustion and every classification cap leave the prior
  consequence unchanged while attempted work remains charged.

### Ore, ordering, provenance, and guards

- A reducible nonleader can be cancelled while a larger irreducible term
  remains.
- A second selected target equal to or greater than the previous target is
  rejected transactionally.
- The selected divisor ordinal and logical divisor visits match the existing
  exact indexed normal form.
- Active and inactive axes apply opposite physical translations to physical
  coefficients, derivation coefficients, and guards.
- A provenance-only collision/cancellation lowers to the exact source-module
  combination.
- A multiplier denominator present only in a derived DAG root appears in the
  lowered localization witness.
- Exact source replay after the cancellation reconstructs every physical row
  coefficient.

### Rational/projective/generated differentials

- Synthetic rows compare the lowered lazy result exactly with one ordinary
  rational AXPY step.
- The projective GCD-scaled path compares equal after common-scale
  cross-multiplication of both physical row and provenance.
- Generated 1L and all four 2L sunset ordinary sources exercise real frozen
  Janet divisors, source ordinals, and active/inactive charts.
- Probe execution orders and supported worker counts produce identical exact
  support, selected witness ordinals, trace, lowered coefficients, and guards.

### Resource boundaries

Exercise one-below limits for every new row, derivation, guard, probe,
classification, fallback, trace, lowering, and transaction counter.  A failed
batch releases no image; a failed transaction releases no root, proof,
derivation, or guard lineage.  Reusing a rolled-back node or translation slot
must remain stale even when its ordinal is reused.

## Implementation order and go/no-go

1. Add the restricted exact-lazy arena/transaction and exact owner binding.
2. Add deterministic probe schedules plus batched support certification and
   batch exact fallback.
3. Import a complete exact consequence, including derivation and guards, into
   a classified exact-lazy consequence.
4. Implement independent greatest-reducible-term selection and one monic
   frozen-epoch cancellation.
5. Add cold lowering, complete source replay, and rational/projective
   differentials through generated 1L/2L inputs.
6. Run the K=3 four-source frozen-normal-form gate before integrating any lazy
   epoch or completion queue.

Proceed beyond ELC1 only if there are zero unresolved committed terms, exact
target/divisor trajectories match the current path, full source replay and
guards match, and batched classification materially avoids coefficient
expansion.  If exact fallback expands most collision roots or the coefficient
DAG/derivation lineage approaches the old rational payload, retain the lane as
a scout and measure the separate projective polynomial-DAG alternative.

For the first K6 grounding run, stop at the established exact prefix rather
than raising caps.  Report exact-fallback fraction, DAG and derivation nodes,
probe/cache work, lowering peak, wall time, and RSS.  Queue exhaustion and an
artifact remain later exact milestones; ELC1 cannot claim either.
