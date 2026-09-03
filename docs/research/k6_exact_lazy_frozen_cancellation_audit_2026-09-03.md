# ELC1 exact-lazy frozen cancellation: adversarial design audit — 2026-09-03

## Scope and verdict

This is an adversarial review of
[`k6_exact_lazy_frozen_cancellation_design_2026-09-03.md`](k6_exact_lazy_frozen_cancellation_design_2026-09-03.md)
against the current exact Janet/Ore implementation, the ELC0 modular circuit,
and the measured K6 failure envelope. It deliberately treats the design as an
external proposal rather than relying on its author's intent.

**Conditional GO for one bounded, in-memory, frozen-epoch cancellation. NO-GO
for a lazy Janet epoch, completion loop, or artifact authority.** The core
one-sided support theorem is sound, and using a rational DAG with already-monic
exact divisors is the smallest useful experiment. The proposal nevertheless
needs four corrections before implementation can be called authoritative:

1. certificates must be issued from a consumed, complete, query-bound batch;
2. the session must retain the actual sealed ordinary-source chronology, not
   merely its opaque owner and source count;
3. probe scheduling and budget accounting must remain deterministic under any
   later parallel execution; and
4. cold lowering must be designed as a separately capped streaming phase,
   because a complete materialization cache can recreate the K6 blow-up that
   the lazy hot path avoids.

Passing one cancellation proves only that exact-lazy coefficient orchestration
can reproduce one exact normal-form step. It does not prove basis insertion,
monic normalization of new rows, synchronous autoreduction, epoch mutation,
queue exhaustion, exceptional-domain closure, or artifact publication.

## The support theorem and its exact boundary

Let a coefficient root denote an element of the characteristic-zero rational
function field represented by exact leaves, signed index translations,
addition, multiplication, and negation. At a prime and point where every
translated exact-leaf denominator is nonzero, finite-field evaluation is a
homomorphism from the corresponding localization into the prime field. Thus:

```text
valid image(root) != 0  =>  exact(root) != 0.
```

One valid nonzero image is sufficient. More probes increase the chance of
finding that witness but add no logical strength. Conversely, one or any
finite number of zero images proves nothing about exact zero.

This theorem remains exact only if a certificate binds all of:

- the live DAG owner and root incarnation;
- the complete signed Ore translation action and indexed context;
- the exact ordered query batch and corresponding image position;
- the prime, canonical residue point, and schedule ordinal;
- successful evaluation of every translated exact-leaf denominator; and
- the fact that no later query, guard, singular inverse, or resource failure
  invalidated the consumed probe.

ELC1 should expose no inverse node. That avoids the additional theorem
obligation that an inverse operand itself have exact nonzero evidence and that
its numerator condition enter localization. The existing raw circuit does
contain `CoeffNode::Inv`, so the restricted `ExactLazyArena` façade is a
mandatory authority boundary, not mere API tidiness.

Exact support then has a simple complete meaning:

- every retained term has either an authenticated exact-ingress nonzero proof,
  one valid nonzero modular certificate, or an exact Symbolica nonzero result;
- every absent nonsyntactic candidate has an exact Symbolica zero proof; and
- no unresolved root can be present in a committed row or influence Janet
  selection.

## Prioritized design findings

### P0 — batch evaluation is present, batch certificate issuance is not

`ModularProbe::try_evaluate_batch` already consumes a probe and releases an
all-or-nothing `ModularEvaluationBatch`. The batch owns its exact query vector,
images, DAG owner, probe identity, and census. However,
`CertifiedNonzero::try_replay` currently constructs a fresh probe and cache for
one root. Calling it for every sparse coefficient would discard the principal
ELC1 performance benefit.

Add one consuming issuer, not a constructor from residues:

```rust,ignore
fn try_issue_support_certificates(
    batch: ModularEvaluationBatch,
    expected_owner: &ExactLazyOwner,
    guard_count: usize,
) -> Result<CertifiedSupportBatch, ExactLazyError>;
```

The evaluated query layout should be canonical and tagged, for example
`[all_guard_roots, all_changed_coefficient_roots]`. The issuer must first
verify the full query vector and owner, then reject the entire outcome if any
guard image is zero. Only after that check may it consume nonzero coefficient
images into root-bound certificates. A guard image of zero is a rejected
point, not an exact-zero guard and not a partially successful coefficient
batch.

The current probe error path returns only `ModularGuideError`; attempted work
is then lost with the consumed lane. ELC1 needs a census-only
`RejectedProbeReport { error, census }`. It must never carry images.

### P0 — exact-zero fallback is complete only if it is all-or-nothing

All valid sample images can be zero for a nonzero rational function. Every
remaining root therefore requires exact Symbolica materialization. If exact
materialization completes, exact `is_zero` is decisive. If it hits a cap,
classification fails and the surrounding transaction must roll back; it must
not choose a conservative support or silently retain a possible zero term.

The current exact materializer handles one root per attempt and creates a new
memo table for that attempt. Before the real K=3 gate, add a multi-root
materializer with:

- one canonical ordered root batch;
- one bounded iterative postorder cache shared across that batch;
- no output release until every requested root succeeds;
- root-bound owned zero/nonzero results; and
- a cumulative caller-owned budget that retains failed attempted work.

Scalar materialization is acceptable for an initial synthetic differential,
but not evidence about K6 scaling.

### P0 — source-module identity is not source chronology

`OreActionIdentity` binds an opaque `CompletedIbpSourceRows` owner and source
count. That proves that ordinals live in the right module, but it does not give
the lowering interpreter the original source relations. Likewise,
`OreConsequence::try_validate` validates row/provenance/action/guards and the
payload census, but it does not regenerate the row from those sources.

`ExactLazySession` must therefore borrow or own an opaque
`OreSourceReplayView` over the actual `CompletedIbpSourceRows`, and construction
must call `ordering.owns_completed_source_module(...)`. The replay view needs
read-only ordinal access to the sealed relations and must reuse the existing
ordinary-source chart lifting, rather than reimplementing IBPs.

Lowering must return an `AuthenticatedLoweredConsequence` only after:

1. materializing the complete physical row, derivation, and guard lineage;
2. reconstructing every source-module coefficient in canonical order;
3. regenerating the physical row from the sealed source chronology; and
4. comparing every shift and exact coefficient.

A restricted parts constructor that checks only structural consistency would
not close this proof gap.

### P0 — cold lowering can recreate the original K6 memory failure

The proposal correctly removes eager provenance expansion from the hot path,
but its current lowering sketch materializes a complete physical batch,
expands the complete provenance map, and then materializes that complete batch.
A single shared exact cache may retain every large intermediate at once.

The measured rejected K6 consequence contained 1,826,367 numerator-plus-
denominator terms: 692,772 across 187 physical coefficients and 1,133,595
across 615 provenance coefficients. The largest coefficient had 21,569 terms.
The early-stop process already reached about 433 MiB; the deeper exact prefix
reached 1,770,948 KiB RSS. Lazy scheduling is not a production improvement if
the exact boundary reconstructs the same peak.

Split cold work into two policies:

- **support fallback:** a small all-or-nothing unresolved-root batch with a
  shared cache; and
- **final lowering:** deterministic streaming/chunked materialization with a
  separate cap, explicit cache eviction or reference-counted liveness, and
  independent peak-RSS telemetry.

Physical roots, provenance expansion, guard descriptors, and exact source
replay should be processed in canonical chunks and discarded as soon as the
next authority boundary no longer needs them. If current artifact schemas
ultimately require the fully expanded coefficient set simultaneously, that is
a later format/ownership problem; ELC1 must not conceal it behind a cold call.

### P1 — the guard lineage node is ambiguous about translation

The proposed `LeftAxpy` guard node stores both `translated_source` and
`physical_delta`. That permits an implementation to translate twice or not at
all. Store the unmodified source lineage and the delta:

```rust,ignore
LeftAxpy {
    accumulator: GuardLineageRef,
    source: GuardLineageRef,
    physical_delta: PhysicalDeltaId,
    multiplier_denominator: ExactGuardDescriptor,
}
```

Lowering then preserves accumulator guards, translates source guards exactly
once, and appends the exact denominator condition of the multiplier. This is
the existing rational AXPY policy. Modular guard values authorize probe-point
selection only and never become retained guards.

### P1 — provenance needs one shared transaction and a bounded interpreter

The whole-consequence derivation

```text
P_new = P_accumulator + multiplier * E^delta P_source
```

is exact and is the right first response to the measured provenance dominance.
It remains authoritative only when the physical AXPY, derivation node, and
guard node commit atomically. No path may retain a classified physical row
whose derivation or guard append failed.

The derivation interpreter must be iterative and cap nodes, edges, source
terms, accumulated shifts, coefficient roots, and output entries. Shared
subderivations should be memoized only within the configured memory envelope.
Deep or branching histories can otherwise turn delayed provenance into an
exponential expansion at lowering.

### P1 — monic frozen divisors suffice for normal form, not for completion

The current exact normal form checks that each chosen Janet divisor has exact
leader coefficient one and then uses multiplier `-target_coefficient`. No
division is needed. The subject and normal-form remainder need not be monic.
Accordingly the rational DAG is sufficient for ELC1 and should not grow an
inverse API.

Basis admission is different: `build_division_epoch` monic-normalizes retained
nonzero rows, applying the same scale to row and provenance and adding the
necessary exact localization condition. Autoreduction can also replace a row
whose leader changed. ELC1 has no lazy equivalent for this operation.

Therefore an ELC1 result must not implement `Into<OreConsequence>`,
`Into<JanetDivisionEpoch>`, or any basis-insertion trait except through cold
authenticated lowering. A future lazy monic operation must consume an exact
nonzero leader proof, introduce a guarded inverse, scale physical and complete
provenance together, and reclassify exact support.

### P1 — epoch mutation must preserve the current synchronous semantics

Current exact autoreduction computes every row against one immutable division
epoch, collects all replacements, and only then builds the next division-only
revision. Completion geometry and the prolongation queue are sealed only at
the autoreduction fixed point. Hidden revisions retain the last observable
sealed predecessor.

One ELC1 cursor borrowing a frozen `JanetDivisionEpoch` is compatible with
this. It does not justify mutating that epoch. A later lazy epoch must preserve:

- one immutable divisor view for a whole synchronous pass;
- exclusion of the row's own ordinal;
- all replacements committed together or none;
- a new coefficient-free divisor index for each changed revision;
- queue, leading ideal, complement, and pure-power coverage only at seal; and
- sealed-predecessor lineage distinct from arena generations.

Coefficient certificates should bind arena roots and action, while each
selection witness/scratch buffer separately binds the exact Janet epoch.

### P1 — deterministic parallel probes need wave semantics

The serial rule in the design is deterministic: process schedule ordinals in
order and query only roots unresolved after earlier ordinals. That exact rule
cannot simply be parallelized, because workers do not know which roots earlier
workers will resolve.

ELC1 should remain serial. A later parallel implementation must choose one of:

1. every concurrent probe evaluates the same complete canonical root batch,
   then outcomes are committed by increasing ordinal; or
2. deterministic wavefronts, where every probe in a wave evaluates the same
   wave-start unresolved set and the next wave starts only after ordered merge.

First-finished results must never choose a certificate. Shared atomic budget
races must not determine success. Allocate deterministic per-probe envelopes,
then merge their censuses by ordinal. Exact fallback roots, derivation output,
guards, and traces must all be canonically sorted.

### P1 — compaction invalidates certificates unless they are replayed

ELC1 should remain append-only and uncompacted. For a future compacting arena,
the live set includes physical coefficient roots, imported provenance roots,
derivation multipliers, guard descriptors, zero/nonzero proofs, and any active
transaction roots.

An old-to-new node map is insufficient evidence: a certificate names an old
owner/generation/root circuit. Compaction must create a new generation and
replay every retained modular certificate against its remapped root before
committing the generation. Any failure retains the old generation. Exact
materialization proofs likewise need an authenticated rebind or fresh exact
check. No rollback may cross the oldest certified live root.

Compaction also does not shorten the semantic depth of a long linear AXPY
history; iterative evaluation remains mandatory.

### P2 — exact ingress and limits need session-wide ownership

The ELC0 arena stores its own immutable modular limits, but higher proofs do not
bind the Ore action or an ELC1 limits contract. `ExactLazyOwner` must bind the
action, context, source replay view, arena generation, arity, and immutable
limits. Every imported exact coefficient is validated once at this boundary.

The cursor must borrow one cumulative `ExactLazyWorkBudget`. Failed probes,
fallbacks, and rolled-back arena suffixes remain charged. Live storage can be
rolled back, but cumulative nodes, translations, probe work, exact work, and
arena churn cannot. Limits should fail before reservation/native work wherever
the requested size is knowable.

## Reduction-selection invariants

The design correctly selects the greatest **reducible** exact-support term,
not necessarily the row leader. The current exact normal form iterates all
terms, queries the immutable Janet divisor index, selects the maximum reducible
Ore key, and rejects a selected key that does not strictly decrease from the
previous step. A larger irreducible term may remain throughout.

ELC1 should reuse the exact coefficient-free index and preserve the lowest
divisor-ordinal rule and logical flat-scan visit accounting. Before mutation it
must check the selected divisor's action, birth ordinal, leading shift, and
exact unit leader. The translated divisor tail must remain strictly below the
target under the same admissible Ore ordering. After AXPY, the target root must
be the structural DAG zero; neither a sampled zero nor an exact-fallback zero is
acceptable for the intended monic cancellation.

Strict descent is a cursor property between successive selected reducible
targets. It is not an assertion that the overall row leader decreases after
each step.

## Minimal corrected implementation decomposition

### ELC1a — theorem-bearing circuit seams

Implement before row logic:

1. a restricted non-inverting exact-lazy arena and committed rollback floor;
2. an exact-lazy owner binding action, source replay view, context, generation,
   arity, and immutable limits;
3. consumed batch certificate issuance from a complete tagged query list;
4. rejected-probe reports carrying census and no images; and
5. an all-or-nothing multi-root exact materializer returning owned exact
   zero/nonzero proofs.

Gate: wrong owner/action/context/limits/query order, stale roots, guard zero,
pole, later-query failure, and resource stop issue no certificate and commit no
support.

### ELC1b — augmented exact identity

Add classified/unclassified row type states, whole-consequence source
derivations, and unambiguous exact guard lineages. Import only from validated
`OreConsequence` values tied to the retained `CompletedIbpSourceRows` replay
view. Physical, provenance, and guards share one transaction.

Gate: imported and one-AXPY consequences regenerate exactly from source rows;
tampered source ordinal/order/action/translation/multiplier/guard fails.

### ELC1c — one frozen cancellation

Borrow the exact division epoch and its scratch/index. Select independently
from classified support, enforce greatest-reducible choice and strict previous-
target descent, build one monic AXPY, require structural target cancellation,
batch-classify changed roots, and commit atomically.

Gate: exact selected target/divisor/visit census and the lowered result match
one ordinary rational normal-form step. A reducible nonleader case must pass.

### ELC1d — cold authenticated lowering

Implement small-batch fallback separately from chunked final lowering. Expand
derivations iteratively, materialize guards with Symbolica, canonicalize through
`LocalizationWitness`, replay sealed sources, and only then return an
authenticated exact consequence for differential testing.

Gate: rational equality, projective whole-vector equality after cross-
multiplication, complete source replay, and exact guard equality all pass under
one-below resource tests.

### ELC1e — full frozen normal form, still no completion

Only after one-step gates pass, iterate the cursor to irreducibility against the
same frozen epoch. Run generated 1L and all four 2L sunset ordinary sources,
including active and inactive axes. Then run all four K=3 ordinary sources.

Gate: exact target/divisor transcript, support, remainder, provenance, guards,
and source replay match the rational authority with zero unresolved terms.

Do not implement lazy basis insertion, autoreduction revisions, queue rebuild,
or completion until all ELC1e gates pass and measurements justify them.

## Required adversarial tests

### Support and certificates

- a nonzero exact root that samples zero at every scheduled point falls back
  exactly and remains present;
- a nonsyntactic exact zero is removed only by Symbolica fallback;
- one early nonzero image followed by a guard zero or later-query failure
  releases no certificate;
- duplicate, reordered, truncated, foreign, or stale query batches fail;
- the lowest valid probe ordinal wins despite reversed completion timing;
- fallback exhaustion rolls back support but retains attempted-work charges;
- no exact-lazy code path can construct or import a raw inverse node.

### Ore, source replay, and guards

- a reducible nonleader cancels while a larger irreducible term survives;
- equal/increasing successive selected reducible targets fail transactionally;
- active and inactive coordinates translate coefficient, provenance, and guard
  descriptors with the correct opposite signs;
- a collision that exists only in provenance is replayed and cancelled exactly;
- a multiplier denominator absent from imported guards appears after lowering;
- source chronology reordering, owner substitution, or ordinal tampering fails;
- physical AXPY success followed by derivation/guard allocation failure leaves
  the old consequence intact.

### Monic and epoch boundaries

- a unit-leader frozen divisor requires no inverse and structurally cancels the
  target;
- a nonunit divisor is rejected by ELC1 rather than silently normalized;
- subject and irreducible remainder may remain nonmonic;
- stale divisor scratch or a foreign epoch cannot drive a cancellation;
- a test-only multi-step cursor resets its descent witness per new subject;
- no ELC1 value can enter a Janet basis without authenticated exact lowering.

### Lowering and memory

- scalar and batch exact fallback agree on shared-subexpression DAGs;
- chunk sizes 1, a small fixed block, and the configured maximum give identical
  exact output and canonical order;
- streamed source replay matches eager replay on small fixtures;
- a deep-above-4,096 circuit uses iterative traversal;
- provenance fan-out and collision fixtures exercise cache eviction and caps;
- peak live cache, final output payload, wall time, and RSS are reported
  separately from the lazy hot path.

## GO/NO-GO gates

### GO: ELC1 one-cancellation implementation

Proceed only with the ELC1a–ELC1d corrections above. The first authority gate
is zero unresolved committed roots, structural target cancellation, complete
source replay, exact guards, and exact rational/projective differential.

### GO: full frozen normal form

Proceed from one step to ELC1e only when all synthetic, 1L, and 2L tests pass,
batch probes reuse their cache, exact fallback is bounded, failures are atomic,
and repeated runs have identical transcripts and censuses.

### GO after a separately reviewed ELC2: first K6 prefix measurement

ELC1 alone cannot run this gate because it owns neither basis insertion nor a
completion epoch. After ELC1e passes, a separate lazy-completion design must
first preserve guarded monic admission and synchronous epoch semantics.

First reproduce trajectory, not merely a lower wall time:

- natural orbit 4 through basis 88, revision 118, 4,097 attempted
  prolongations, and 44,168 normal-form steps; and
- selected order `[5, 3, 4, 2, 0, 1]` through basis 100, revision 141, 5,232
  attempts, and 139,945 normal-form steps.

For the natural prefix require no more than 885,474 KiB RSS and 794.26 seconds,
exact fallback below 25% of classified roots, and later compaction below 20% of
wall time. Compare the selected-order run against its 2,489.84-second and
909,372-KiB exact baseline. Report logical divisor visits separately.

Final lowering has its own gate: it must finish exact source replay within its
configured cap and report a peak materially below the old expanded rational
path. A fast lazy prefix followed by an equal or larger cold blow-up is a
NO-GO for completion integration, though the lane may remain useful as a
scout.

### NO-GO: completion or publication

Stop before completion integration if any of the following holds:

- a sampled zero changes support or any unresolved root commits;
- certificate construction accepts a scalar residue or partial batch;
- source chronology is absent, or provenance is only structurally validated;
- modular probe values become retained application guards;
- exact-lazy code can create an unguarded inverse;
- parallel completion order or shared-budget races alter proof selection;
- compaction remaps a certificate without replay against the new generation;
- monic normalization or synchronous autoreduction semantics are skipped; or
- final exact lowering recreates the established coefficient/RSS blow-up.

Even a fully successful ELC1e run proves only frozen normal-form viability.
Lazy Janet completion requires a separate reviewed design for guarded monic
basis admission, synchronous autoreduction over immutable division epochs,
sealed queue/geometry reconstruction, and eventual exact artifact lowering.
