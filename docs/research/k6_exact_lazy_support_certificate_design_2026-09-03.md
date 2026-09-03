# Exact-lazy support certificates for Janet/Ore completion — 2026-09-03

## Status and purpose

This note records a candidate follow-on to the bounded modular normal-form
lane. It is not implemented authority and it does not weaken any existing
publication gate. Its purpose is to test whether RustRed can avoid expanding
most rational-function coefficients while retaining an exact, independently
replayable proof of every leading monomial used by Janet completion.

The decisive observation is one-sided. Let `c(d,n)` be an exact rational
function represented by a circuit over authenticated Symbolica coefficients.
If every inverse encountered by the circuit is defined at a finite-field
sample and the result is nonzero, then `c` is not the zero rational function.
Reduction modulo a prime can turn a nonzero characteristic-zero function into
zero, but it cannot turn the identically zero function into a nonzero value.
Thus a valid nonzero residue is an exact nonzeroness certificate; a zero
residue is never an exact zero certificate.

## Authority boundary

The lane may use finite fields to prove only nonzeroness. It must classify a
coefficient in exactly one of these ways:

1. `KnownZero`: zero follows structurally from the exact circuit or from an
   exact Symbolica materialization and equality test;
2. `CertifiedNonzero`: an owned coefficient circuit plus a replayable valid
   finite-field sample has nonzero residue; or
3. `Unresolved`: no valid nonzero sample was found and exact materialization
   has not established zero or nonzero.

Only the first two states may enter an authoritative row support. An
`Unresolved` coefficient stops or defers the transaction. Repeated modular
zeros, cross-prime agreement, probability estimates, or a resource stop can
never create `KnownZero`, discharge an obligation, declare a master, or close
an artifact.

Each nonzero witness must bind the coefficient-DAG owner and root, exact
coefficient context, Ore action, physical translation, modulus, complete
sample point, residue, and all denominator/inverse checks. Loading or joining
a witness re-evaluates this tuple; callers cannot construct it from scalar
telemetry. The witness is evidence about one exact coefficient root, not about
another algebraically similar expression.

## Exact circuit consequence

An authoritative lazy consequence needs more than a sampled physical row. It
retains exact circuit roots for both:

- sparse Ore-row terms keyed by forward shifts; and
- sparse source-module provenance keyed by `(source ordinal, left shift)`.

Ore translation, addition, multiplication, negation, and projective or monic
normalization create exact DAG nodes. Therefore the provenance circuit is a
derivation proof even before it is expanded. Every input leaf comes from an
already authenticated ordinary source or localization polynomial. The lane
must apply the same physical sign map to row, provenance, and guards.

Physical-row support is normalized after every operation. Syntactic zeros are
removed immediately. Every remaining coefficient is evaluated over a small
deterministic schedule of independent Symbolica `Zp64` lanes. The first valid
nonzero lane seals a `CertifiedNonzero`; coefficients zero in all usable lanes
are sent to exact Symbolica materialization. This makes the resulting support
exact rather than probable.

Provenance terms need not be sampled merely to choose a leader, but they must
remain as exact circuit roots and receive bounded normalization. Before a
consequence crosses an existing exact/public boundary, its complete
provenance is materialized or replayed from the authenticated source module.

## Exact Janet completion

The completion scheduler may operate on a row only after all physical support
terms are `CertifiedNonzero`. Janet masks, divisor indices, leaders, and
nonmultiplicative obligations are then computed from exact support.

For one normal-form cancellation:

1. select the greatest exactly supported Janet-divisible term;
2. verify the divisor and operator shift under the frozen epoch;
3. update physical row, provenance, and localization circuits with the exact
   left Ore action;
4. reclassify only coefficients affected by the sparse merge; and
5. require the selected target to become `KnownZero`.

A zero remainder is authoritative only when its complete physical row is
empty after exact classification. A modularly empty sampled row is not a zero
remainder. Queue exhaustion is authoritative only when every exact Janet
obligation has been processed under these rules. The ordinary exact leading
complement, guard-stratum checks, regenerated-source replay, owner admission,
and cold artifact load remain unchanged after that point.

## Memory and lifecycle requirements

A global append-only expression arena could merely replace coefficient swell
with hundreds of millions of retained DAG nodes. A viability prototype must
therefore measure and bound:

- live and cumulative DAG nodes;
- exact leaves and translated-delta nodes;
- row and provenance roots;
- evaluation-cache entries per probe;
- exact fallbacks caused by all-zero samples; and
- bytes retained across Janet revisions and autoreduction passes.

At each immutable epoch boundary, RustRed should compact only roots reachable
from the retained basis, localization, and mandatory proof records into a
fresh hash-consed arena. The old epoch and all probe caches are dropped
transactionally. Compaction must preserve root-to-root witness identity by
replaying witnesses against the new roots; pointer identity alone cannot cross
an arena generation. If compaction costs dominate, an alternative weak-owner
arena may be studied, but unbounded append-only retention is not acceptable.

The modular work is embarrassingly parallel by independent probe. Exact
support decisions and basis mutations remain deterministic and serial at the
epoch transaction boundary. Workers share immutable circuit shape and exact
leaves; each owns only field values, caches, and bounded scratch. This avoids
forking the entire exact basis per worker.

## Relationship to projective fraction-free replay

This lane and projective arithmetic are complementary. Projective
GCD-scaled pseudo-reduction avoids inverse nodes and denominator growth, while
finite-field nonzero witnesses avoid expanding polynomial products merely to
learn support. The preferred prototype should therefore use primitive
projective circuit rows, with Symbolica `gcd`, `gcd_multiple`, and `try_div`
at explicitly measured normalization boundaries. It should not reproduce
those CAS algorithms inside RustRed.

If exact GCD materialization becomes the dominant cost, first compare three
bounded policies: normalize every cancellation, normalize only before basis
admission, and normalize only at epoch compaction. Modular GCD guesses may
nominate work but cannot authorize exact division without Symbolica
verification.

## Falsifiable implementation sequence

1. Add an internal exact materializer for the M0 coefficient DAG, memoized by
   `(node, translation)`, and differential-test it against direct Symbolica
   rational arithmetic for active and inactive shifts.
2. Add a consuming `CertifiedNonzero` constructor which independently replays
   a nonzero `Zp64` observation. Provide no scalar-residue constructor.
3. Build one exact-lazy sparse row cancellation against a frozen Janet epoch.
   Differential-test exact support, leader, and remainder against the current
   rational and new projective implementations through generated one- and
   two-loop cases.
4. Build a bounded completion prototype with mandatory epoch compaction. Run
   all independent probes and exact fallbacks under explicit work limits.
5. Ground it on the same release K6 orbit/order envelopes used by the exact
   baseline. Compare queue progress, exact fallbacks, live DAG bytes, RSS, and
   elapsed time. Do not raise the old exact limits to mask a regression.
6. Promote the representation into the production completion seam only if it
   preserves the exact trajectory on small cases and materially reduces K6
   retained payload and wall time. Otherwise retain it only as a scheduling
   scout.

## Acceptance and rejection criteria

The prototype is viable only if:

- every retained leader has at least one replayed nonzero witness or an exact
  Symbolica nonzero result;
- every removed nonsyntactic term has an exact Symbolica zero result;
- all independent worker counts produce identical exact leaders, queue
  outcomes, and final proof content;
- one- and two-loop completion and source replay are byte-for-byte stable at
  the existing exact boundary;
- K6 makes substantially more queue progress per unit RSS and wall time than
  the exact baseline; and
- no sampled miss or all-zero probe set affects closure authority.

Reject or redesign the lane if exact-zero fallbacks expand at roughly the old
rate, append-only DAG memory approaches the rational baseline, epoch
compaction dominates, or final proof materialization simply recreates the
same peak before producing useful exact owner inputs.
