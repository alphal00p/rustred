# Exact-lazy persistent Janet completion design — 2026-09-03

## Evidence boundary

This note records the reviewed design for the first persistent exact-lazy
Janet/Ore completion experiment after ELC1. It is an implementation contract,
not a K6 closure result. The implemented foundation owns authenticated exact
ingress, exact-support classification, complete normal forms, guarded monic
normalization, persistent immutable lazy Janet epochs, and a cold
Symbolica-backed replay boundary. It does not yet own production basis
admission, lazy prolongation, autoreduction, queue exhaustion, a finite
terminal complement, or artifact publication.

The existing exact completion loop remains the semantic oracle. In particular,
the lazy path must preserve its one cumulative work ledger, immutable epoch
revisions, synchronous autoreduction, complete Janet obligation queue,
deterministic ordering, and exact leading-ideal complement. The optimization
is a coefficient representation change; it is not permission to weaken a
proof boundary.

## Implemented foundation and audit status

The ELC1/early-ELC2 foundation is now implemented and independently audited.
Its focused serial gates pass `14/14` cold-lowering tests and `72/72`
exact-lazy tests under the current Symbolica runtime license; the complete
involutive subsystem passes `196/196`. Three independent audit passes found no
remaining P0/P1 soundness or authority defect and approved the next persistent
completion slice.

The scalar cold boundary is intentionally not yet a K6 publication boundary.
It currently expands one complete physical/provenance/guard root envelope and
retains the shared exact-materialization cache and all outputs until the batch
finishes. This is bounded and exact, but can recreate the measured K6 peak-RSS
cliff. Final K6 publication therefore additionally requires deterministic
chunked/streamed materialization, safe cache eviction based on root liveness,
selective source lifting, and peak-memory telemetry under one campaign-owned
cumulative lowering budget.

The first ELC2 geometry slice is also implemented: exact and persistent lazy
epochs share a coefficient-free Janet geometry interface for masks, indexed
division, selection, prolongations, the leading ideal, exact complement, and
pure-power witnesses. Every immutable snapshot has an opaque identity in
addition to its deterministic revision depth, so sibling successors cannot
accept one another's scratch, selections, or prolongations.

The follow-on ELC1e and guarded-normalization slices are now implemented. A
normal-form cursor owns the cumulative support and involutive-work ledgers for
its complete lifetime, reselects the greatest reducible supported term after
every committed cancellation, enforces strict descent, and cannot be finalized
before irreducibility. The generated one-loop source and all four generated
`K=3` ordinary sources agree with the exact path for active and inactive Ore
actions. Guarded monic normalization has one proof-bound route to inversion:
the actual live leader is selected internally, its inverse and structural-one
replacement are sealed to the owner, row identity, shift, and coefficient
roots, and a typed numerator-nonzero guard is joined to the complete historic
domain.

A translated multi-step `K=3` audit exposed, and the current implementation
resolved, a fail-closed cold-boundary mismatch. For generated source ordinal
one, all-active action, and outer shift `[1,1,0]`, exact replay retains three
syntactic guards including the redundant product `(n0+1)(n1+1)`, while lazy
lineage retains the three canonical factor guards `n0+1`, `n1+1`, and `n1+2`.
The boundary now proves the required one-way implication between principal
open domains using bounded Symbolica square-free factorization, polynomial
GCD, and exact division, while retaining the authenticated lazy domain as the
published authority. Adversarial missing-factor and unrelated-factor cases
still fail closed.

An independent resource audit then found that GCD and exact-quotient
coefficients can be taller than either expanded input, so a naive input-height
native-operation envelope was not conservative. The boundary now applies a
checked mixed-radix Kronecker-to-univariate Mignotte factor-height bound before
entering Symbolica for square-free factorization, GCD, or exact quotient.
Concrete taller-GCD and taller-quotient regressions, including a one-below
preflight rejection, cover the corrected contract.

The same audit identified two authority requirements that are easy to miss at
the structural epoch seam. First, root liveness is not itself proof that a
wrapper crossed its transaction boundary when hash-consing reused only old
roots. Every consequence now carries a private per-transaction commit receipt,
published only after the complete arena floor and census commit; aborted and
failed-commit wrappers remain permanently inadmissible. Cold lowering and
normal-form ingress explicitly check this receipt. Second, a finalized normal
form must retain its exact epoch, action, excluded-divisor mode, and cumulative
campaign-ledger identity. A self-excluding autoreduction remainder must never
be substitutable for a full normal form, and neither result becomes a basis
row merely because its roots are live and its leader is monic. The persistent
epoch API stays structural until an opaque, mode-correct admission joins these
proofs.

Persistent lazy division and complete epochs are also implemented. They share
committed coefficient payloads through `Arc`, rebuild only coefficient-free
Janet geometry, carry opaque sibling-distinguishing epoch identity, and bind
all selector/index work to one non-cloneable campaign ledger. Per-transaction
commit receipts prevent an aborted all-preinterned wrapper from crossing the
epoch boundary. Raw addition/replacement constructors remain test-only;
production successors stay closed until mode-correct normal-form admission
tokens are implemented.

## Architectural decision

Do not force lazy rows through `JanetDivisionEpoch`: its elements deliberately
own exact `OreConsequence` values. Add a coefficient-lazy sibling epoch whose
elements own `Arc<ExactLazyConsequence>`, while extracting the coefficient-free
geometry used by both paths into one implementation.

The intended internal modules are:

```text
exact_lazy/
  epoch.rs
  normalization.rs
  normal_form.rs
  autoreduction.rs
  completion.rs
  telemetry.rs
```

The core epoch shape is:

```text
ExactLazyJanetElement
  canonical ordinal
  exact supported leader and ordering key
  Janet multiplicative mask
  Arc<ExactLazyConsequence>

ExactLazyJanetDivisionEpoch
  opaque instance/revision identity and sealed predecessor
  exact-lazy owner and Ore action
  arity, canonical elements, shared Janet divisor index

ExactLazyJanetEpoch
  division epoch
  complete nonmultiplicative prolongation queue
  exact leading ideal and uncovered partition
  pure-power coverage
```

Janet mask construction, divisor selection, prolongation geometry,
pure-power witnesses, blind-domain priority, and complement construction must
accept a narrow coefficient-free geometry view. They must not learn about
lazy coefficient nodes or duplicate their exact-path implementations. Initial
exact rows are imported once and retained as shared lazy values; all successor
epochs are coefficient-lazy.

## Guarded monic normalization

This is the first mandatory ELC2 authority boundary. Given an exactly supported
nonzero remainder

```text
r = a E^u + tail,
```

the epoch may install

```text
a^-1 r = E^u + a^-1 tail
```

only on the domain where the numerator of `a` is nonzero. Existing rational
definedness guards retain its denominator condition. The transaction API must
therefore expose a specialized proof-consuming operation, never generic lazy
division:

```text
try_actual_leader_inverse(classified_row, ordering)
    -> GuardedLeaderInverse
```

The operation independently locates the live leader, consumes or checks its
exact nonzero proof, creates the inverse with the existing Symbolica-backed
rational DAG primitive, binds a private seal to owner/row/shift/root/inverse,
and creates `NumeratorOf(a)` guard lineage. Only that seal may replace the
leader by structural one while multiplying the tail and source derivation by
the inverse. The complete output support is classified again and the whole
coefficient/provenance/guard mutation commits atomically.

No general transaction `inv` or `div` API is permitted. Nor may a structural
simplification of `a * inv(a)` erase the `a != 0` exceptional domain. A
separate projective fallback,

```text
sigma_delta(a) subject - b E^delta divisor,
```

is sound without inverse nodes, but it retains nonmonic rows and is expected
to grow substantially larger circuits. It is a separately typed experimental
lane, not an implicit fallback.

The vendored Symbolica API already supplies canonical rational-polynomial
inversion, exact division, numerator/denominator extraction, polynomial GCD
cancellation, and exact normalization. RustRed must use those primitives.
RustRed owns only the Ore action, Janet geometry, proof consumption, guards,
provenance, and resource admission; those capabilities are not available as
Symbolica primitives.

## Immutable autoreduction

Every autoreduction pass observes one immutable division epoch:

1. reduce each row against that same epoch while excluding itself;
2. share an unchanged row by cloning its `Arc`;
3. retain an exact empty-support witness for a dropped row;
4. guard-normalize every changed nonzero remainder;
5. resolve equal heads deterministically before successor construction; and
6. publish no successor until the complete pass succeeds.

Repeat until a pass changes nothing. Only the stable epoch may construct its
complement and Janet queue. No row, mask, divisor index, or queue is mutated in
place.

Synchronous remainders can acquire equal leading shifts. Rejecting such a pass
as `DuplicateLeadingShift` would make the completion algorithm partial.
ELC2 must extract the exact initial equal-head reduction or implement its lazy
equivalent as one sealed deterministic batch. Equal-head handling must itself
preserve exact support, guards, provenance, and cumulative accounting.

## Completion loop and closure authority

The lazy loop mirrors the exact loop:

1. seal the stable epoch and its exact complement geometry;
2. construct and rank every mandatory nonmultiplicative prolongation;
3. prolong one row and reduce it completely against the immutable epoch;
4. retain a zero witness, or normalize and insert the first nonzero remainder;
5. invalidate the complete old queue and all epoch-local witnesses;
6. synchronously autoreduce and rebuild an immutable epoch; and
7. repeat until the current complete queue is exhausted.

Diagnostic blind-domain truncation may change priority only; it may not omit a
Janet obligation. A queue-exhaustion seal is bound to the exact epoch, owner,
action, ordering, and complete set of final zero witnesses.

Queue exhaustion is not family closure. The final uncovered partition must
independently be finite, its cardinality must fit the configured bound, and
every point must be enumerated as an explicit terminal candidate. An empty
queue with an infinite complement returns a typed
`QueueExhaustedNonFinite` report containing the missing axes/unbounded boxes.
It can never publish an artifact or reinterpret an infinite ray as a numerical
master.

For a finite complement, extract the existing bounded lattice enumerator from
the owner-cover compiler rather than creating another lattice implementation.
The terminal enumeration and queue-exhaustion seal must join the same immutable
epoch identity before cold publication can begin.

## Campaign-owned cumulative resources

Production exposes a one-shot campaign object which owns all mutable ledgers:

```text
ExactLazyCompletionCampaign
  ExactLazySession
  ExactLazySupportBudget
  InvolutiveWorkBudget
  ExactLazyCompletionBudget
  optional read-only progress observer
```

Limits are supplied once. Callers cannot replace support or work budgets
between cancellation steps, normal forms, epochs, or retries. Failed
transactions roll live arenas back but never refund coefficient-DAG churn,
classification attempts, modular probes, exact fallbacks, selector/index
work, traces, queue work, or epoch work.

Telemetry records both live and cumulative arena sizes; basis and queue peaks;
classifications and exact-fallback fractions; divisor/index work; normal-form
steps; autoreduction sharing/materialization; complement geometry; and cold
replay work. Observers see committed snapshots only. TTY rendering and wall
clock timing stay in the application layer, so non-TTY logs remain stable and
machine-readable.

The first implementation keeps epoch mutation serial. Only immutable modular
probe evaluations may be parallelized. The lowest scheduled successful probe,
not the fastest worker, supplies proof authority, and worker counts 1, 2, and
4 must produce identical rows, guards, terminal sets, transcripts, and logical
censuses.

## Cold publication boundary

After exact queue exhaustion and finite-complement certification:

- cold-lower every final basis row;
- cold-lower and replay every retained final zero-obligation witness;
- verify exact support, monicity, leader identity, source provenance, and
  canonical localization guards;
- rebuild the exact Janet geometry and compare it with the lazy epoch; and
- construct publication objects only from authenticated exact consequences.

The scalar ELC1 lowering operation is sufficient for the first small
correctness fixtures. K6 publication requires a batched cold boundary sharing
one ordinary-source lift, provenance memo, Symbolica materialization cache,
and cumulative lowering budget. A fast hot completion that recreates the old
RSS or coefficient explosion while lowering is a failed K6 optimization, not
a successful result.

## Implementation sequence

1. Finish and independently audit ELC1 cold lowering.
2. Extract shared coefficient-free Janet geometry and prove the exact path's
   K3 trajectories and censuses unchanged.
3. Implement proof-bound leader inversion and guarded monic normalization.
4. Implement immutable lazy division and complete epochs with `Arc` sharing.
5. Implement a full frozen normal form with structurally bound cumulative
   budgets.
6. Implement lazy prolongation.
7. Implement synchronous autoreduction and equal-head collision reduction.
8. Implement the one-shot completion campaign and immutable queue rebuild.
9. Add queue-exhaustion and finite-complement certificate types.
10. Add batched cold lowering and exact publication reconstruction.
11. Differential-test synthetic, one-loop, and all four K3 ordinary sources
    against the exact path.
12. Run bounded release K6 prefixes under natural and selected orderings,
    compare trajectory/RSS/fallback fractions with the frozen baselines, then
    attempt complete K6 only after the prefix gates pass.

## Adversarial acceptance gates

- Foreign or stale owner, action, epoch, ordering, scratch, schedule, and
  budget values fail before mutation.
- An aborted or uncommitted row cannot enter an epoch; committed shared rows
  remain live after later aborts.
- A leader inverse cannot be minted for an arbitrary coefficient, stale proof,
  or nonleader root.
- Normalizing `a=(n-1)/(n+2)` retains `n+2 != 0`, adds `n-1 != 0`, and never
  makes the rule applicable at `n=1`.
- One-below limits fail atomically and retain attempted-work charges.
- Lazy selected targets/divisors, cold remainder, provenance, and guards equal
  the exact-path result on bounded fixtures.
- Unchanged autoreduction rows preserve pointer identity; equal heads are
  resolved canonically.
- Old queue entries and zero witnesses fail after any successor epoch.
- Sampled zero coefficients always reach exact fallback before support can
  change.
- Queue exhaustion with an infinite complement cannot become a closure or
  artifact claim.
- Corrupt cold physical rows, derivations, guards, terminal enumerations, or
  zero witnesses fail exact replay.

Only a zero-uncovered, queue-exhausted, cold-replayed, reloadable artifact may
be called a closing K6 artifact.
