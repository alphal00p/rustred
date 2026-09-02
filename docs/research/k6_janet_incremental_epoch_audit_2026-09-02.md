# K6 Janet incremental-epoch and copy-on-write audit — 2026-09-02

Status: pre-copy-on-write baseline audit plus implemented first ownership
slice; not a closure result. This note audits the exact Janet/Ore completion
control flow after introduction of an indexed Janet-divisor lookup. It
identified avoidable epoch work and a correctness-preserving route to
structural sharing. The borrowed scan and copy-on-first-cancellation portion is
now implemented; division/completion epoch separation and zero-proof reuse
remain follow-up work. It does **not** claim that
any K6 Janet queue has exhausted, that the K6 complement is finite, that a
closing artifact exists, or that any rule is ready for publication.

The measured release baseline remains
[K6 Janet/Ore bounded release study](k6_janet_ore_release_study_2026-09-02.md).
The authority boundary remains the one in
[Janet/Ore proposal integration seam](janet_ore_integration_seam_2026.md): an
involutive result is proposal-only until ordinary-source regeneration, exact
replay, guard/descent checking, exact owner admission, artifact sealing, and
cold-load validation all succeed.

## Executive finding

The completion loop at the measured baseline preserved a sound immutable-epoch model, but paid
for that model by reconstructing much more than mathematical correctness
requires. Every autoreduction pass first copies every exact row, provenance
witness, and guard payload through a zero-shift, unit-coefficient Ore AXPY.
Every changed pass then rebuilds the complete basis metadata. Every accepted
remainder discards the old obligation chronology, so zero reductions may be
performed again in later epochs.

The safe architecture is not an in-place mutable basis. It is a persistent
basis with:

1. shared, sealed exact row payloads carrying stable row identities and payload
   revisions;
2. epoch-local Janet masks and divisor metadata separated from completion-only
   geometry and obligations;
3. a borrowed normal-form ingress that materializes a row only at its first
   real cancellation; and
4. a bounded zero-proof cache whose reuse is checked against the exact divisor
   rows and multiplicative variables used by the old reduction sequence.

This keeps frozen-pass determinism, stale-epoch rejection, exact provenance,
localization guards, and cumulative resource accounting intact.

## Pre-copy-on-write control flow and measured implication

The relevant paths at the audited release baseline were:

- `completion/involutive/completion_loop.rs`: every autoreduction pass calls
  `try_copy_basis_consequences`, reduces all copied rows against the frozen
  epoch, and builds a replacement epoch whenever any row changes or vanishes;
- `completion/involutive/normal_form.rs`: the copy is implemented by
  `OreConsequence::try_copy_sealed`;
- `completion/involutive/ore/arithmetic.rs`: that operation is a zero plus
  unit, zero-shift left AXPY, rather than an ownership-only copy; and
- `completion/involutive/janet.rs`: every successor reconstructs normalized
  rows, leader ranking, all Janet masks, the divisor index, every
  nonmultiplicative prolongation, pure-power coverage, the minimal ordinary
  leading ideal, and its complement.

Let one autoreduction pass contain `B` rows, `S` total Ore-row terms, `P` total
provenance terms, and `G` retained guard polynomials. Before testing whether a
single row is reducible, the copy path:

- charges and performs `2 * (S + P)` coefficient transformations;
- translates `G` guards at the zero shift;
- allocates new row and provenance arrays;
- sorts and canonicalizes those arrays; and
- validates the resulting exact payload again.

If no row changes, all of that copied payload is discarded. If one row
changes, unchanged rows nevertheless arrive at the successor as newly
materialized consequences. The replacement boundary validates the full batch
again before the epoch builder scans leaders and monic invariants again.

The release K6 matrix makes the repeated factor concrete. In this control
flow, a final basis revision `r` implies exactly `r + 1` complete epoch builds
and `r + 1` started autoreduction passes: there is the initial build/pass and
one successor build/pass for every insertion or changed replacement. The
post-monic K6 stops at revisions 86 through 115 therefore performed 87 through
116 complete builds and full-basis copy passes. The pre-monic orbit-zero stop
at revision 117 performed 118. The final bases contained 59 through 91 rows
from nine initial rows.

The exact total copied payload is `sum_epoch B_epoch`; the current census does
not retain that sum. From the final basis sizes alone there were at least 50
through 82 insertions, and more if autoreduction dropped any rows. Thus each
orbit had at least 51 through 83 stable completion epochs, in addition to any
intermediate replacement epochs. This does not by itself identify the dominant
wall-time fraction—the exact coefficient swell remains independently
measured—but it proves a large multiplicative source of avoidable work.

## Implemented first slice

RustRed now scans each immutable row by reference, retains the existing
`Arc<OreConsequence>` when no Janet divisor is available, and materializes an
owned row only after the first exact cancellation has been selected. The
selection, indexed-query scratch, and historical logical-visit charge are
handed directly to the owned reduction loop, so they are not repeated. Frozen
pass semantics, exclusions, provenance, localization, and deterministic
successor sorting are unchanged, and explicit shared/materialized admission
counters cover the new ownership path. Intermediate successors still rebuild
completion-only geometry; the division/completion split below is the next
structural optimization.

There is a second structural waste: an epoch built only to serve another
autoreduction pass still constructs its prolongation queue, ordinary leading
ideal, complement, and pure-power coverage. None of those completion-only
objects is read before the next replacement if that pass changes a row.

## Required work versus rebuild work

When one exact row or leading monomial changes, correctness requires:

- exact sealing and monic normalization of each new or changed row only;
- preservation of that row's complete source provenance and localization;
- Janet-mask updates for prefix classes affected by inserted, removed, or
  changed leaders;
- a matching update or rebuild of the Janet-divisor index;
- autoreduction of rows whose terms may be divided by a new or changed Janet
  cone, to the extent required by the retained autoreduced-basis invariant;
- creation of obligations for a new row and for existing variables that have
  newly become nonmultiplicative;
- removal of obligations that are no longer nonmultiplicative or whose source
  row disappeared;
- invalidation of cached reductions that used changed row payloads or lost
  multiplicative directions; and
- deterministic reranking of still-pending obligations when the blind-domain
  priority changes.

Correctness does not require:

- identity-AXPY of an unchanged exact row;
- repeated authentication of an already sealed immutable row;
- a full leader scan and sort when an ordered row set can be updated
  transactionally;
- rebuilding completion geometry during an intermediate autoreduction pass;
- rebuilding the ordinary complement from the full orthant when the leading
  ideal is unchanged or only expands by a known antichain delta;
- redoing a zero normal form whose exact involutive reduction certificate is
  still legal in the current division; or
- repeatedly merging guards already present in the accumulated localization
  witness.

Full mask recomputation is a reasonable first implementation even though it is
not mathematically necessary. For a fixed old leader, insertion affects Janet
multiplicativity only in the prefix class shared with the inserted leader for
each ordered variable. Under insertion alone an old bit may change from
multiplicative to nonmultiplicative, but not in the opposite direction.
Removal or leader replacement can change bits in either direction. A later
persistent prefix index can restrict work to those classes; it should not be a
prerequisite for eliminating exact payload copies.

## Correct zero-proof reuse

A zero algebraic identity remains true after the basis grows, but that fact is
not sufficient to reuse a Janet reduction. Janet completion needs an
*involutively legal* representation in the current division. Adding a leader
can shrink an existing row's Janet cone by making one of its variables
nonmultiplicative. An old cancellation that shifted that divisor in the lost
direction is no longer a valid Janet cancellation.

Consequently, caching only an obligation key such as `(source row, variable)`
would be unsound. A reusable zero proof needs at least:

```text
ObligationKey = (source_row_id, source_payload_revision, variable)

ZeroProofDependency = (
    divisor_row_id,
    divisor_payload_revision,
    positive_support(operator_shift),
)
```

For every reduction step, `positive_support(operator_shift)` is the set of
coordinates whose operator exponent is nonzero. Reuse is allowed only when:

1. the source row and payload revision still exist;
2. the source variable remains a current nonmultiplicative obligation;
3. every recorded divisor row and payload revision still exists;
4. every coordinate in each recorded positive support remains multiplicative
   for that divisor in the current Janet mask; and
5. the proof belongs to the same opaque Ore action, coefficient localization,
   source module, and linear completion lineage.

New divisors do not invalidate a reduction sequence satisfying those checks.
Changed ordinals do not matter, which is why proof identity must never use a
basis ordinal. Ordinals are epoch-local and can change after canonical sorting.

The cached record must preserve the zero remainder's localization witness. A
proof reused without its required nonzero conditions would silently broaden
the coefficient domain. Retaining the full provenance and reduction trace is
useful for diagnostics and replay, but it has an aggregate memory cost. The
cache therefore needs explicit bounds on:

- proof count;
- dependency steps;
- operator-support coordinate cells;
- guard references, terms, exponent cells, and logical retained bytes; and
- any retained provenance or diagnostic transcript.

A smaller first implementation may attach a global division-shape generation
to every proof and invalidate the complete cache whenever any old row payload
or mask changes. It is correct but may discard much reusable work. The
dependency-aware form above is the target design.

Proof reuse changes telemetry, not the algebraic acceptance boundary.
`attempted_prolongations` should count actual normal-form attempts. Add separate
counts for cache hits, misses, invalidations by cause, and retained proofs. A
cache hit performs no new algebra and must not be charged again to the
cumulative normal-form ledger.

## Minimal ownership and API slice

### 1. Seal and share exact rows

Introduce an immutable row object conceptually equivalent to:

```rust
struct SealedJanetRow {
    id: JanetRowId,
    payload_revision: u64,
    leading_shift: ForwardShift,
    leading_key: ShiftComplexityKey,
    coefficient_census: CoefficientPayloadCensus,
    consequence: OreConsequence,
}
```

Epoch elements hold an `Arc<SealedJanetRow>`, their current ordinal, and their
epoch-local multiplicative mask. New or changed rows are authenticated,
monic-normalized, and assigned cached leader metadata once. Unchanged rows
retain the same Arc. Row IDs are stable within one completion lineage;
structural equality and deterministic output must not depend on pointer
addresses or process-global allocation order.

The internal hot path should have a consuming successor operation so it can
move old Arc handles after a frozen pass. The existing borrowing successor may
remain for tests or external immutable forks. Epoch identities and public
prolongations remain revision-bound, so old prolongations continue to fail the
existing stale-epoch check.

### 2. Separate division from completion views

Split the current all-in-one epoch into two layers:

```text
JanetDivisionEpoch
  ordered shared rows
  epoch-local masks
  Janet divisor index
  aggregate row/coefficient census

JanetCompletionView
  stable division epoch
  nonmultiplicative obligations
  leading ideal and exact complement
  pure-power coverage
```

Autoreduction needs only the division layer. Build the completion view after
autoreduction reaches a fixed point. If a pass changes only coefficients or
tails while ordered leaders and masks remain identical, share the division
shape and its divisor index. The index scratch should be tagged by a division
shape identity rather than a payload epoch identity in that case. Public
prolongations remain tagged by the payload epoch because their source row may
have changed.

### 3. Add a borrowed normal-form ingress

Replace unconditional `try_copy_basis_consequences` with an internal normal
form returning one of:

```text
Unchanged(shared sealed row)
Changed(new exact consequence)
Zero(exact proof and localization)
```

Run the indexed reducibility selection against the borrowed sealed payload.
If there is no reducible term, return its existing Arc with no coefficient
translation, allocation, or validation. At the first real cancellation,
materialize only that row. Prefer a borrowed AXPY constructor that writes the
new canonical row and provenance directly; copying by a unit, zero-shift AXPY
does avoidable Symbolica arithmetic even for a row that genuinely changes.

The pass remains synchronous: all reductions observe the same frozen division
epoch. Record unchanged row slots while computing outcomes, then consume the
old epoch and move their shared payloads into the replacement. This preserves
the present deterministic semantics and avoids introducing order-dependent
in-place autoreduction.

Exclusion must be expressed by stable row identity at the normal-form API.
The indexed lookup must omit that row and still return the canonical alternate
divisor when one exists. Translating a stable exclusion back to the current
ordinal is allowed inside one frozen epoch.

### 4. Retain only valid completed obligations

Keep a completion-local, resource-bounded resolved-obligation ledger beside
the immutable epoch. On each stable epoch:

1. construct or update the current canonical obligation descriptors;
2. validate cached proofs against current row revisions and masks;
3. requeue invalid proofs and remove vanished obligations;
4. rank every still-pending obligation using the current blind schedule; and
5. skip only the proofs that pass all dependency checks above.

Filtering a valid cached zero from the current ranking does not change which
remaining nonzero obligation is first. Stop at the first new nonzero remainder
as today; batching several nonzero remainders would change scheduling semantics
and is not part of this minimal slice.

### 5. Preserve transactional accounting

One `InvolutiveWorkBudget` remains shared by initialization, autoreduction, and
completion. Delta construction must preflight all new row, mask, index, proof,
guard, and geometry counts before publishing the next state. Failure leaves the
predecessor and proof ledger unchanged.

Maintain aggregate coefficient censuses by checked subtraction of removed or
revised rows and checked addition of new rows. In debug and test builds,
compare the delta result against a full recomputation. Retained-byte limits are
logical current-state limits: a shared row counts once in each current basis,
regardless of how many Arc handles temporarily reference it. Add telemetry for
shared rows, materialized rows, copied coefficient/provenance terms, index
shape reuse, and proof-cache outcomes.

## Incremental leading geometry

The ordinary leading ideal cannot shrink under a valid completion successor:

- an accepted nonzero prolongation remainder either retains the uncancelled
  prolongation target, which is an ordinary multiple of its source leader, or
  obtains a lower leader after cancellation and may expand the current leading
  ideal; and
- an autoreduced or dropped old head was cancelled by another retained
  divisor, so its old orthant remains covered.

Make this an executable invariant: every predecessor minimal generator must be
covered by the successor leading ideal. If a new head is already covered, share
the old leading ideal, complement, and pure-power coverage. Otherwise update
the minimal antichain and subtract only the new generator orthants from the old
complement. This is especially relevant for pure Janet monomial completion,
where a new head is often an ordinary multiple of an existing one and changes
the Janet cones without changing the ordinary complement at all.

Blind-domain entries can be shared when the complement is unchanged. Current
pending obligations still need ranking because their set can change. If the
complement expands contrary to the monotonicity invariant, return an internal
error rather than silently applying a delta algorithm outside its proof.

Incremental prefix-class masks and persistent divisor postings can follow
after the shared-row slice is measured. Recomputing their lightweight metadata
from cached leaders is initially safer than mixing several algorithmic changes
into the first equivalence test.

## Duplicate-head abort risk

Initial ingress now deterministically eliminates equal leading heads before
constructing Janet masks. The replacement path does not apply the same rule.
Two distinct rows reduced synchronously against one frozen epoch can acquire
the same nonzero new leading shift. The current epoch builder then returns
`DuplicateLeadingShift`, aborting an otherwise valid completion calculation.

This is a completeness/robustness risk, not evidence of an infinite Janet ray.
Extract the exact equal-head reduction into a shared sealed-batch operation and
apply it to changed collision classes before masks or the divisor index are
built. A merged row should receive a fresh row identity; cached obligations and
proof dependencies for all contributing payload revisions must be invalidated.
Its provenance and localization are combined by the existing exact Ore
arithmetic, under the same cumulative work budget.

## Termination assessment

No nontermination flaw was established by this audit. The normal-form engine
checks that every selected reducible target strictly decreases under the
frozen admissible ordering, and Janet division is Noetherian. The observed K6
stops remain resource stops before queue exhaustion, dominated by coefficient
swell and, in the pre-index measurements, divisor scanning.

The present restart policy can nevertheless prevent *practical* termination:
`max_completion_iterations` counts repeated zero obligations as fresh work.
A finite mathematical completion can therefore exhaust its bounded work
envelope long before it visits all distinct current obligations.

Add defensive progress checks:

- every successor leading ideal covers every predecessor generator;
- the leader of an accepted nonzero remainder was irreducible in its source
  epoch at normal-form exit;
- a canonical `(sealed row payloads, leaders, masks)` state may not recur
  without a genuinely new retained proof state; and
- every cached proof accepted as current passes its complete dependency and
  localization checks.

These checks diagnose an implementation cycle. They do not replace the actual
closure gate, which remains exhaustion of all mandatory obligations followed
by exact complement, guard-branch, and publication validation.

## Regression and measurement plan

The implementation gate should include:

1. **Differential algebra.** Retain a test-only full-rebuild oracle while the
   COW engine is introduced. Exhaust small monomial and polynomial fixtures,
   input permutations, sector masks, and variable orders. Compare canonical
   basis consequences, masks, obligations, leading ideals, complements,
   localization, and provenance.
2. **Zero-copy stable pass.** An already autoreduced epoch must materialize zero
   row payloads, perform no unit-copy coefficient transformations, and return
   the identical row Arcs.
3. **Localized mutation.** A fixture with one changed row must share every
   unaffected payload Arc. Rows that genuinely change retain exact source
   replay and guards.
4. **Proof reuse.** Place two zero obligations before a nonzero obligation that
   causes an insertion. Prove the valid zeros are attempted once and then
   reused, with the same final exact basis as the uncached oracle.
5. **Proof invalidation.** Independently exercise a lost multiplicative bit, a
   changed divisor payload, a dropped divisor, a changed source payload, and a
   vanished obligation. Only proofs whose dependencies remain valid may be
   reused.
6. **Ordinal independence and exclusion.** Reorder basis ordinals while
   preserving row identities; cached proofs remain correctly bound. Own-row
   exclusion remains effective and indexed lookup can select an alternate
   valid divisor.
7. **Mask and geometry oracle.** Compare incremental results with the existing
   full Janet-mask definition and full leading-ideal/complement reconstruction
   for exhaustive small leader sets, including additions, removals, and head
   replacements.
8. **Equal-head replacement.** Force synchronous remainders to collide and
   compare the shared reducer with deterministic initial preprocessing.
9. **Resource failures.** Tight row-delta, proof-cache, dependency, guard,
   index, and geometry limits fail before mutation; the old epoch and ledger
   remain usable. Overflow paths return typed errors.
10. **Stale authority.** Prolongations, index scratch, cached proofs, and row
    identities from foreign actions, lineages, localizations, source modules,
    or revisions are rejected.
11. **K3/K6 trajectory.** Compare the exact structural trajectory with the
    full-rebuild engine under the same order and caps. Report actual normal
    forms, cache hits/invalidations, materialized/shared rows, index operations,
    exact coefficient operations, wall time, and RSS. Changed telemetry must
    be explained by removed work rather than presented as algebraic divergence.

Only after those gates pass should persistent prefix masks, incremental bitmap
postings, dependency-directed autoreduction, or sequential/work-list
autoreduction be considered. Those can offer further gains, but they are not
needed to remove the clearest whole-payload copies and unused epoch products.

## Non-claim

This audit proposes a correctness-preserving reduction in repeated work. It
does not establish that the resulting engine will overcome K6 exact
coefficient swell, exhaust any of the six K6 Janet queues, produce a finite
complement, cover exceptional guard branches, or generate a closing K6
artifact. Those outcomes require new measured release runs and the unchanged
exact publication gates.
