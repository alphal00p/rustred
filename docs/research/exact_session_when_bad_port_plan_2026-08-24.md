# Exact-session `WhenBad` and publication port plan

Status: authoritative implementation plan; Phase A event-ledger foundation and
the scoped independent-cylinder Ready geometry/descent/hazard checkpoint are
implemented; updated 2026-08-25.

This document specifies the next topology-neutral RustRed seam after
`GeneratedAffineResidualGroupExactSessionRecenterOutcome`. It joins the
current exact session transaction to LiteRed-style `WhenBad`, target
disposition, exceptional residual work, and sealed rule publication.

The implementation governed by this plan is pure Rust using GMP-enabled
Symbolica. It must not use FORM, Mathematica at runtime, Symbolica's `no_gmp`
feature, loop-count-specific code, topology-specific recurrences, or
hard-coded reduction rules. Loop counts and concrete vacuum or scattering
topologies are validation inputs only; they never select a core algorithm.

The preceding database/recenter ownership contract remains
`docs/research/exact_group_solve_transaction.md`. This plan is narrower and
normative for the missing `WhenBad` and publication seam.

## 1. Current implementation checkpoint

The current-lineage components below are implemented and tested:

- `src/generated_affine_residual_group_exact_database.rs` stages and commits
  LiteRed-style hardest-only exact rows with consume-once transition identity.
- `src/generated_affine_residual_group_exact_targets.rs` owns the persisted
  target order, current target premises, typed equality-refinement targets,
  immutable target dispositions, and exact successor preparation. Its
  `prepare_successor(..., Option<GeneratedAffineResidualGroupRetainedReadyExactTarget>)`
  can either preserve every target or consume exactly one authenticated Ready
  target.
- `src/generated_affine_residual_group_exact_session.rs` is the sole owner of
  the database and exact target-state allocation. It seals them into a staged
  transaction, exposes an authenticated joint view, and returns the owning
  `GeneratedAffineResidualGroupExactSessionRecenterOutcome::{NoTarget,
  RequiresAffineEqualityRefinement, Ready}`.
- `src/generated_affine_residual_group_exact_recenter_kernel.rs` supplies the
  authority-free, arbitrary-precision recentering arithmetic through
  `ExactCenteredShift`, `ExactRecenteredTerm`, and `ExactRecenteredRow`.
- Current Ready outcomes retain the exact recentered row, the retained Ready
  target, and the original consume-once transaction. Target premises and
  translated row guards remain separate. No outcome publishes a rule or
  consumes a target.
- The current lineage also implements typed `commit_no_target` and
  `commit_and_suspend_affine_equality_refinement` transitions. NoTarget
  consumes the running session and returns the sole continuation owner only
  after commit. Equality consumes the running session into a sealed
  `GeneratedAffineResidualGroupExactSessionSuspendedForRefinedEpoch` that
  retains the committed session, successor-bound unresolved equality target,
  and exact private terminal event while exposing no resume, staging, or
  session-extraction method.
- Staged database rows retain an opaque production or synthetic source recipe
  and shared exact dependent-reduction or new-pivot evidence. The private
  transaction path prepares an owning database commit that can be aborted on
  recoverable outer-preflight failure. After all event and target replacements
  are admitted, an allocation-free checked fail-stop boundary precedes the
  infallible database/owner commit tail. Equality successor state and rebound
  target are minted as one capability-gated pair over the same exact
  allocation.
- The session owns a private append-only event ledger for every consumed
  source. Current events cover `Dependent`, `NoTarget`, and mandatory
  affine-equality refinement, share the database's exact source/evidence
  allocations, and retain authenticated versions, disposition, target data,
  exact offsets where applicable, and cumulative resource statistics.
- `replay()` creates a fresh shadow session and restages every opaque source
  recipe chronologically. It reruns hardest-only reduction, recentering, and
  target matching; compares exact shared evidence and dispositions; and checks
  terminal database, target, event, and resource state. Equality-suspension
  replay additionally authenticates the exact terminal event.
- `src/generated_affine_residual_group_ready_publication.rs` authenticates the
  sealed Ready/session/target geometry without extracting the transaction. For
  an independent cylinder it locates the unique unit zero-shift pivot, builds
  source and RHS keys from the exact selected anchor, proves strict physical-
  key descent, and retains finite inactive-orthant hazard intervals as
  Symbolica `Integer` data. Its `ReadyForConditions` result is an unpublished,
  target-preserving typestate. General compact affine target maps currently
  return an operational `Pending` result and remain part of the active phase.

Focused tests cover exact Ready translation, NoTarget beyond `i64`, a 4,096-bit
coordinate, post-top-reduction leader selection, transaction return after
stale/foreign/resource failure, equality return before translation, atomic
dependent commit, sealed sibling-successor pairing, exact/one-below resource
boundaries, and the private production-source recipe's Arc lifetime and replay.

The typed transition slice passed licensed, GMP-enabled four-way `cargo-nextest`
runs `f8fefe69-c966-48eb-ada8-9bac85f24158` (sealed equality, 1/1),
`e06c10c3-2f18-48ca-8eec-51a229972d82` (source surface plus compositional
production recipe, 2/2), and `4e1ef6e4-749e-4f2e-8f00-869059b61f20`
(remaining exact database/session/targets/recenter-kernel regression, 44/44).
`cargo check --tests -j 4`, `cargo fmt --all -- --check`, and
`git diff --check` also passed.

The event-ledger slice adds focused tests for private
event/source/evidence `Arc` identity and lifetime, retained production-source
replay after staging owners drop, cumulative and one-below event/replay owner
limits, chronological fresh-shadow re-execution of dependent and pivot
transitions, exact NoTarget/equality offsets, and sealed suspension replay.
The production-source resource test additionally drops every external source
pipeline handle and proves that the admitted recipe owns the physical row,
re-elimination, bound outcomes, elimination, and source-local
authority/premises/ordering/schedule graph. A separate anchor fixture proves
that only exact frame-authority pointer identity suppresses that authority's
otherwise unique allocation.

The frozen event-ledger milestone passed licensed, GMP-enabled four-way
`cargo-nextest` runs `34021f1d-7458-4700-9ec9-4155cd338c39` (all 16 exact
session tests, including the sealed equality replay, 16/16),
`75cec2fb-6846-4a1b-8df5-029b1331e717` (exact database, physical-row, and
recenter-kernel tests, 42/42), and `ff9310fa-fb3b-42e2-b79f-531fc93708ad`
(the complete retained source-parent graph gate, 62/62). `cargo check
--all-targets -j 4`, `cargo fmt --all -- --check`, and `git diff --check`
also passed. Neither FORM nor Symbolica's `no_gmp` feature was used.

Production-source coverage is deliberately compositional: genuine physical-row
ingress proves retained Arc identity, lifetime, and row replay, while the full
equality transition test uses its sealed synthetic test adapter. The current
physical-row fixture skips equality-premise source cases, so an end-to-end
production equality row remains a future refined-epoch gate.

The scoped exact Ready checkpoint passed independent licensed-GMP Nextest
gates with explicit `--lib -j4`: run
`a06d5558-e404-4048-a2e9-5407277a95d6` passed all 11 tests in the independent
Ready/publication validation module, and
`f74b89eb-1e59-4628-91d7-82af1f11b893` passed the two internal Ready units plus
the physical-key comparison witness. The latter shares the authoritative
comparison implementation used by `Ord`; successful descent transcripts keep
only the first decisive component rather than retaining every RHS key. A fast
test-only `L=6`, `K=21` family validates all 36 generic ordinary-IBP rows and
stable regeneration. A genuine all-inactive arity-21 Ready probe is currently
blocked earlier by eager Boolean-cover split 65,537 exceeding the 65,536 cap;
the cap was not raised.  A later complete-MTBDD experiment avoided that
explicit partition but retained 49 atoms and 268,427 rooted nodes before its
cursor could return the first residual.  The high-loop path therefore requires
a shared authenticated normalized-source owner and bounded direct
normalized-formula frontier search.  The MTBDD remains an optional compact-case
backend under a separately measured construction budget, not the primary
arity-21 entry path.  This supersedes this plan's earlier "lazy
MTBDD/sector-DAG" entry wording without invalidating the V5 representation or
its replay contract.

Not yet implemented are the Ready-native condition transcript and relative
partition, the terminal current-lineage exact `WhenBad` compiler,
target-consuming rule publication, exceptional residual orchestration, or
replayable current-lineage rule/residual handles. The current event ledger is
a complete transcript only for its implemented non-publishing dispositions;
its schema and replay must be extended with the future `WhenBad`, publication,
and residual manifests. The mature `GeneratedResidualAffine...`
implementation is an oracle, not production authority for these missing
pieces. Full LiteRed parity, arbitrary one-loop pentagon reduction, and the
high-throughput two- through six-loop vacuum milestones therefore remain
pending.

The former `src/exact.rs` blocker is complete: exact scalar and matrix algebra
now crosses Symbolica's public GMP `Rational` and `Matrix<Q>` APIs. Continued
Phase B/C work must keep applying the same Symbolica-first rule. See the
[`Symbolica exact-linear-algebra API inventory`](symbolica_exact_linear_algebra_api_inventory.md)
and the
[`Symbolica-first algebra migration audit`](symbolica_first_algebra_migration_audit_2026-08-24.md).
RustRed must not build a second CAS or matrix layer.

## 2. Normative LiteRed semantics

### 2.1 Three distinct state layers

LiteRed keeps three semantically separate layers. RustRed must preserve this
separation even though it makes their successful transition atomic:

1. **Algebra database.** `Solvej` repeatedly reduces only the current hardest
   known integral. It normalizes and installs the first unknown hardest pivot
   before target matching or `WhenBad`
   (`vendor/LiteRed2/Source/LiteRed2026.m:2164-2195`). A candidate later
   rejected by `WhenBad` remains an algebraic pivot and may reduce subsequent
   source rows.
2. **Ordered target state.** `SolvejSector` scans the persisted `cases` order
   and selects at most the first matching case after recentering
   (`:2430-2435`, `:2484-2486`). A certified rule consumes the selected coarse
   target. A rejected candidate does not. RustRed's equality-refinement
   typestate is a safe extension: it must defer the group into a refined epoch,
   not skip the first target.
3. **Published coverage and residual work.** Applicable guarded rules and bad
   subdomains are not the algebra database. A mixed accepted rule removes its
   coarse case, publishes the good subdomain, and places the exceptional
   subdomain into later work (`:2488-2500`, `:2519-2523`).

Append-only row/candidate events belong to the third owner layer as replay and
diagnostic authority. They do not substitute for either database pivots or
target dispositions.

### 2.2 Recentering and first-target order

For the coordinates symbolic in the current affine group, LiteRed applies

```text
n_i -> 2 n_i - r_i
```

where `r` is the returned pivot coordinate. Fixed coordinates are unchanged.
The current exact kernel expresses the same operation in local geometry as

```text
t = r - A r_F
delta_F = -r_F
q = s - r
```

and must remain the only production arithmetic path. After recentering, scan
unresolved targets strictly by persisted solve ordinal. Never use hash/set
iteration, equal-offset tie breaking, or a later Ready target when the first
matching target requires equality refinement.

### 2.3 Exact LiteRed bad formula

Let the recentered right-hand side, after equal integrals have been collected
and exactly-zero coefficients removed, be

```text
R(n) = sum_t b_t(n, lambda) J(a_t(n)).
```

Here `n` are integer indices, `lambda` are algebraically independent
parameters, `c_i in {0,1}` is the target-sector corner, and
`Z = { i | c_i = 0 }` is the set of inactive coordinates.
`CollectjList` performs the coefficient collection and cancellation
(`vendor/LiteRed2/Source/LiteRed2026.m:4132-4134`, `:4171-4202`).

For every factor `f` appearing in a coefficient denominator, LiteRed asks
whether the factor is identically zero as a polynomial in the independent
parameters:

```text
D_f(n) = AND_alpha [ coefficient_{lambda^alpha}(f(n, lambda)) = 0 ]
D(n)   = OR_f D_f(n).
```

With no parameters, LiteRed inserts a dummy parameter so the same operation
is defined. It does not automatically split every physical parameter locus
such as `d - 4 = 0`; the test is polynomial identity in the declared
parameters (`vendor/LiteRed2/Source/LiteRed2026.m:2565-2567`).
For example, `n+d` is never identically zero as a polynomial in parameter `d`
because its `d` coefficient is one; treating `n+d=0` as one pointwise
`Q(d)[n]` predicate would be an unsound over-partition. The Symbolica-native
implementation must project the denominator into its parameter-coefficient
vector (public `RationalPolynomial::to_polynomial(base_variables, true)` after
the authenticated exponent/variable conversion) and form one conjunction of
zero predicates for those index-polynomial coefficients. Factoring is useful
for smaller clauses but is not required for correctness in the integral
domain.

Run this projection only on final Ready coefficients after exact integral-term
collection and recentering, so cancelled terms cannot leave spurious bad
clauses. For a future nonidentity compact-affine pullback, use the public
Symbolica-backed `ResidualAffineCoefficientComposition::Available`: its
normalized `value` supplies the leak numerator, while its separately retained
pre-normalization `mapped_denominator` supplies the substitution-domain
condition. `ZeroMappedDenominator` is an all-domain terminal
`IdenticallyBad` candidate, not `Unsupported` or an operational failure. Never
project an earlier raw coefficient or reapply Ready's diagnostic translation.

For every RHS term, the raw inactive-coordinate leak locus is

```text
L_t_raw(n) = OR_{i in Z} [ a_{t,i}(n) >= 1 ]
L_t(n)     = SmartReduce(L_t_raw(n)).
```

Only inactive coordinates are leaks. An active coordinate reaching zero is a
valid pinch into a subsector. If any `L_t` is literal `True`, the whole
candidate is bad. Otherwise LiteRed decomposes `L_t` into alternatives `x`
and retains `x` when

```text
Expand(Numerator[b_t] /. ToRules[x]) =!= 0.
```

The leak part is the disjunction of retained alternatives:

```text
N(n) = OR_{retained (t,x)} x.
B(n) = SmartReduce(D(n) OR N(n)).
```

If reduction introduces any noninteger power, normally a radical, LiteRed
fails closed and replaces `B` with `True`
(`vendor/LiteRed2/Source/LiteRed2026.m:2565-2569`). `SmartReduce` performs
integer reduction under the sector orthant and conservatively returns `True`
for unrecognized residual structure (`:2573-2581`).

The published and exceptional domains of a target with premise `C` are

```text
applicable  = C AND NOT B
exceptional = C AND B.
```

LiteRed marks all of an alternative `x` bad when the coefficient numerator is
not identically zero after `ToRules[x]`; it does not generally emit
`x AND numerator != 0`. RustRed may use its existing explicit numerator-gate
partition to certify a strictly larger good locus, but that is a
Symbolica-native strengthening. It must be represented and replayed as a
proof, never assumed as literal LiteRed behavior.

### 2.4 Successful and failed candidate semantics

If `B =!= True`, LiteRed publishes the guarded rule, records `C AND B`, and
deletes the selected target case (`:2492-2500`). Thus a mixed partition still
consumes the original coarse target; its exceptional leaves are separate work.

If `B === True`, LiteRed publishes no rule, leaves the target case unresolved,
and excludes the exact candidate from being returned again. It does not remove
the already installed pivot from the algebra database (`:2501-2505`). A
consume-once committed pivot/event supplies RustRed's equivalent exclusion.

No outcome in this seam infers a master integral.

## 3. Current authority boundary

The only admissible production input to exact `WhenBad` is the owning current
Ready typestate from
`GeneratedAffineResidualGroupExactSessionRecenterOutcome::Ready`. It binds:

- the exact session/database allocation and staged transition identity;
- database epoch, group, state version, source cursor, and pivot identity;
- the retained post-top-reduction source/replay recipe;
- the current target-state allocation and first persisted unresolved Ready
  target;
- the target locator, affine geometry, and current target-premises certificate;
- the exact recentered terms, exact centered shifts, and translated row guards.

The concrete premise authorities are
`GeneratedAffineResidualCasePremisesCertificate` and
`GeneratedAffineResidualCaseEqualityRefinementCertificate`
(`src/generated_affine_residual_case_premises.rs:385-535`). The retained
target capabilities are
`GeneratedAffineResidualGroupRetainedReadyExactTarget` and
`GeneratedAffineResidualGroupRetainedEqualityRefinementExactTarget`
(`src/generated_affine_residual_group_exact_targets.rs:1753-1855`).

Private constructors and non-`Clone` ownership are part of the proof. A public
constructor from a relation, target ordinal, locator, manifest, physical key,
or matching visible values is forbidden. Hashes and fingerprints are replay
evidence, not allocation authority.

The exact compiler consumes Ready and returns a terminal object that continues
to own it. Dropping Ready or any later terminal commits nothing. Every
preparation error returns the same owning typestate unchanged.

Mechanically, preparation borrows `&Ready` inside `catch_unwind` and builds a
separate admitted prepared value. Only after `Ok(prepared)` may code destructure
or move Ready. Commit preparation similarly borrows `&terminal` and moves the
terminal owner only after every successor and replacement is ready. This
borrow-then-move order is required; early destructuring would make exact retry
ownership impossible even if the public error type claimed otherwise.

## 4. Explicitly forbidden old-authority reuse

The following older `GeneratedResidualAffine...` objects must not be adapted,
wrapped, or reconstructed by copying ordinals:

- `GeneratedResidualAffineWhenBadBinding`,
  `AuthenticatedGeneratedResidualAffineWhenBadInput`,
  `GeneratedResidualAffineWhenBadCertificate`, and
  `GeneratedResidualAffineWhenBadCompilation`
  (`src/generated_residual_affine_when_bad_compilation.rs:255-326`,
  `:1151-1227`, `:5391-6200`, `:6803-7197`);
- `WhenBadCertificate`, `WhenBadCandidateBinding`, `BoundaryHazardRange`, and
  the old `IndexShift` descent proof in `src/when_bad.rs`;
- old descent and pullback-table certificates in
  `src/generated_residual_affine_when_bad_descent.rs` and
  `src/generated_residual_affine_when_bad_pullback_gate.rs`;
- `GeneratedResidualAffineSequentialTargetState`, old group effective
  coverage, and its rule/residual locators
  (`src/generated_residual_affine_group_effective_coverage.rs:321-706`);
- old sector effective-coverage owner and residual-queue authority slots;
- `ConditionalConcreteAuthority::GeneratedAffine`, old conditional-rule
  specialization, and old generated provider publication.

Those types authenticate the old matcher/`Arc<ParametricRelation>` graph and
carry `IndexShift`, `i64` boundary coordinates, or old owner locators. They
cannot prove ownership of the new session transaction. They remain semantic,
differential, and resource-accounting oracles only.

## 5. Source-neutral kernels to reuse

Reuse is restricted to mathematics with no old authority payload:

1. The truth-routing idea in `src/direct_bad_formula.rs`, after generalizing
   `DirectBadFormulaClause` from its current one- or two-atom shape to a
   resource-bounded slice in a shared atom arena. A coefficient-denominator
   identity clause can contain arbitrarily many parameter-coefficient zero
   atoms, so the existing fixed-width clause is not a correct reusable core.
2. The relative target-domain partition core and
   `AffineWhenBadRelativePartitionCertificate` from
   `src/generated_residual_affine_when_bad.rs:382-518,863-1638`. A new exact
   outer owner must authenticate its problem and retain the certificate.
3. Canonical Symbolica polynomial normalization, associate detection, and
   deterministic deduplication algorithms from
   `src/generated_residual_affine_condition_accumulator.rs`. Its existing
   input/certificate and `Option<&IndexShift>` provenance are not reusable.
   For the new high-loop path, project each unique locus once to `K[n]`, call
   public Symbolica `MultivariatePolynomial<Field>::make_monic`, and cache that
   canonical representative. A hash bucket is lookup only; exact Symbolica
   polynomial equality remains the proof. This replaces quadratic pairwise
   associate scans with one checked normalization per unique locus while the
   old cross-product helper remains a differential oracle. The wrapper must
   preflight growth, catch panic, authenticate maps/bounds, and return an
   operational resource failure rather than implementing fallback algebra.
   Canonicalize/authenticate Symbolica rational coefficients before hashing
   because `Integer` representation variants can be value-equal but
   representation-distinct; preserve source insertion order for the transcript
   and use exact monic polynomial equality to confirm every bucket hit.
4. Compact affine polynomial-composition algorithms used by
   `src/generated_residual_affine_when_bad_pullback_gate.rs`. Its old Ready
   binding, event table, boundary values, and certificate are not reusable.

The old `finite_boundary_hazard_range` and
`prove_uniform_same_sector_descent` (`src/when_bad.rs:3326-3654`) are
algorithms to audit/port to `Integer`, not functions or proof types to call.
Only its `InactiveSectorActivation` interval belongs to parametric `WhenBad`.
The old helper's `ConcreteIndexOverflow` intervals at `i64::MIN/MAX` are
artifacts of a concrete representation, not mathematical target bad loci. If
a bounded concrete application representation still needs such checks, put
them in the sealed application layer and never feed them into rule derivation,
target disposition, or the parametric bad formula.

## 6. Implementation phases and APIs

### Phase A: validated typed non-publishing commits and event ledger —
implemented foundation

The typed NoTarget commit, consuming equality suspension, private append-only
chronological event collection, cumulative resource accounting, and
fresh-shadow replay described in the checkpoint are the validated foundation
of this phase. The current private event disposition implements `Dependent`,
`NoTarget`, and `RequiresAffineEqualityRefinement` only. The target extended
schema is

```rust,ignore
enum GeneratedAffineResidualGroupExactRunDisposition {
    Running,
    RequiresAffineEqualityRefinement,
}

enum GeneratedAffineResidualGroupExactSessionEvent {
    Dependent { /* sealed source transition */ },
    NoTarget { /* sealed committed pivot */ },
    RequiresAffineEqualityRefinement {
        /* target locator + retained refinement certificate */
    },
    WhenBadIdenticallyBad { /* future sealed candidate */ },
    WhenBadUnsupported { /* future typed representation reason */ },
    Certified { /* future publication locator/counts */ },
}
```

Names may follow existing conventions, but fields and constructors remain
private. Add consuming APIs whose failure retains the precise input type:

```rust,ignore
fn commit_no_target(
    self,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    outcome: GeneratedAffineResidualGroupExactSessionRecenterNoTarget,
) -> Result<
    GeneratedAffineResidualGroupExactSessionCommittedNoTarget,
    GeneratedAffineResidualGroupExactSessionCommitNoTargetFailure,
>;

fn commit_and_suspend_affine_equality_refinement(
    self,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    outcome:
        GeneratedAffineResidualGroupExactSessionRecenterRequiresAffineEqualityRefinement,
) -> Result<
    GeneratedAffineResidualGroupExactSessionSuspendedForRefinedEpoch,
    GeneratedAffineResidualGroupExactSessionSuspendRefinedEpochFailure,
>;
```

All concrete types remain crate-private.

`NoTarget` commits the pivot/cursor and advances target-state version with
`prepare_successor(..., None)`. It consumes no target and emits no rule. The
running session is consumed across this boundary: preflight failure returns
the original `(session, outcome)`, while success returns a typed committed
owner whose `into_session` is the only continuation seam. Thus even an
unreachable post-database invariant failure cannot leave a callable session.

Equality also commits the pivot/cursor without target consumption, but records
the refinement obligation and changes the run disposition so no further row
can be staged in this group. A fresh quotient/refined target epoch must start
with a fresh database; generic-field pivots must not be imported blindly into
the equality quotient. Equality is neither `IdenticallyBad`, a normal
rejection, nor a solved target.

The existing unconsumed kernel and database now use owning preparation plus a
preallocated event/target replacement and infallible final tail. Preserve that
seam and extend it to prepared rule and residual replacements for later
`WhenBad` outcomes. The suspended equality owner is itself the immediate
refinement authority; its chronological event is already recorded/replayed
without making the committed session resumable.

### Phase B: exact target geometry, descent, and hazards

Add topology-neutral current-lineage modules, for example:

```text
generated_affine_residual_group_exact_when_bad_descent.rs
generated_affine_residual_group_exact_when_bad_conditions.rs
generated_affine_residual_group_exact_when_bad_pullback.rs
generated_affine_residual_group_exact_when_bad.rs
```

The exact target-geometry view is borrow-only and constructed solely from the
retained Ready target/session. It exposes target constants, corner, free
positions, compact affine map, premises, and ordering inputs without exposing
the transaction or allowing target selection.

Port signed same-sector descent to exact physical-key comparison. Add an exact
physical-frame helper that combines the target anchor offset with an
`ExactCenteredShift` and yields a
`GeneratedAffineResidualGroupPhysicalKey`/exact target-local key. Never call
`ExactCenteredShift::try_to_index_shift`. The exact lattice shift and ordering
foundation already exist in
`src/generated_affine_residual_group_physical_key.rs:384-605`; the new helper
must preserve that ordering rather than duplicate it.

Boundary endpoints, boundary values, interval counts, activation coordinates,
target constants, key aggregates, and descent witnesses use
`symbolica::domains::integer::Integer`. No generated value may round-trip
through `i64`, `i128`, `usize`, or `IndexShift`. A `usize` conversion is
permitted only after proving a nonnegative materialization count fits both
`usize` and its configured limit.

This exact boundary compiler enumerates only genuine inactive-coordinate
activation values. Active-coordinate pinches are valid subsector terms, and
`i64` concrete-overflow boundaries are application-representation checks, not
`WhenBad` hazards.

Represent every inactive activation interval lazily with exact `Integer`
fields. For an inactive target coordinate and centered RHS shift `q_i > 0`,
the interval is `first = 1 - q_i`, `last = 0`, `count = q_i`. An inactive
constant coordinate that is activated is an unconditional bad witness and
must be rejected before numerator gates. A separate bounded materializer may
expand a range only after comparing its exact count with the configured limit
and proving that count fits `usize`; a `2^4096` interval must remain one lazy
range rather than attempting `2^4096` allocations.

Add an exact-local physical-key path beside the legacy `IndexShift` input:
prospectively census `anchor_offset + ExactCenteredShift`, execute canonical
GMP addition, build both RHS and target keys through the existing physical
frame, and compare with the same private field-order routine used by `Ord`.
The descent witness records the first decisive key component so replay cannot
drift from the actual database order.

For nonidentity target maps, expose the already-computed exact affine constants
from `RecenterReady::target_offset` through a narrow borrowed view and combine
them with the authenticated free positions and compact matrix in
`ResidualAffineCompactMapView`. `execute_target_offset` has already computed
`p-A p_F`; condition compilation must not recompute those constants from the
physical anchor. Independent cylinders skip this composition because their
Ready coefficients and row guards are already centered.

### Phase C: owning exact `WhenBad` compiler

Introduce a non-`Clone` current-lineage terminal:

```rust,ignore
enum GeneratedAffineResidualGroupExactWhenBadCompilation {
    Certified(GeneratedAffineResidualGroupExactWhenBadCertified),
    IdenticallyBad(GeneratedAffineResidualGroupExactWhenBadIdenticallyBad),
    Unsupported(GeneratedAffineResidualGroupExactWhenBadUnsupported),
}
```

Every variant owns the complete RecenterReady typestate. The compiler runs
under `catch_unwind`; on authentication, resource, allocation, arithmetic, or
panic failure it returns the exact Ready owner. Terminal `Unsupported` is
reserved for a deterministic, replayable representation limitation after
valid input authentication. It must not conceal a transient or resource
failure.

A successfully reproduced LiteRed fail-closed radical/noninteger-power result
has `B=True` and is therefore `IdenticallyBad`. `Unsupported` applies only
when RustRed can authenticate and replay a specific representation limitation
but cannot construct a sound `B` partition; it is not an alternate spelling
for the literal fail-closed formula.

The Certified certificate retains current target premises, exact row guards,
canonical coefficient-denominator conditions, exact descent witnesses,
boundary/numerator pullbacks, structural loci, and the relative partition
certificate. An all-exceptional partition is classified `IdenticallyBad`, not
Certified with zero applicable leaves.

`IdenticallyBad` retains the authenticated literal-true or all-exceptional
proof, complete condition/descent/pullback transcript, and full resource
census. `Unsupported` retains its deterministic typed reason, the
authenticated partial transcript up to the unsupported seam, and its complete
resource census. Neither may be a reason-only enum that cannot replay why the
authenticated candidate was rejected.

### Phase D: atomic target disposition and publication

Add sealed owner collections for:

- chronological committed events;
- current-lineage guarded rule records;
- exceptional-domain and exceptional-leak residual records;
- equality-refinement records.

Add consuming terminal commit APIs. A Certified commit prepares the target
successor with `Some(retained_ready_target)`, one event, all applicable rule
leaves, and all exceptional residual leaves. `IdenticallyBad` and
`Unsupported` use `None`, keep the target unresolved, and publish no rule.

Published handles name a committed exact-session event/record allocation and
offer only sealed replay/application views. They do not expose a raw
`ParametricRelation` or become authoritative merely from a serialized
locator. Each committed guarded-rule record retains or moves the complete exact
recentered RHS coefficients and shifts, or an equally complete authenticated
source recipe from which replay reconstructs those exact values. Partition
locators and counts alone are insufficient to apply a reduction. Provider
integration happens only after this owner exists.

### Phase E: replay and provider integration

Replay must independently reconstruct and verify:

1. authenticated exact physical source row and hardest-only staging;
2. exact session/database/target predecessor binding;
3. persisted first unresolved target selection;
4. exact recentering and separation of premises from row guards;
5. condition, descent, pullback, direct-formula, and partition transcripts;
6. the committed transition identity and target disposition;
7. published applicable and exceptional locators/counts.

Durable manifests are untrusted inputs. Loading a manifest must rerun replay
under the current family/context/session authority before minting an in-memory
sealed handle. Only then may the conditional-rule/provider layer specialize
or apply it.

## 7. Condition provenance and ordering

The exact condition accumulator receives sources in this stable order:

1. authenticated target premises, in their persisted certificate order;
2. exact recentered row guards, in row order;
3. coefficient denominator conditions, pivot first via the retained
   `pivot_term_ordinal`, then RHS centered-term/descent order and deterministic
   factor order;
4. boundary and numerator-gate loci, retaining term/coordinate/event
   provenance.

Canonical associate detection and deduplication may merge equal predicates,
but the certificate records every contributing source. Equality predicates
must not be laundered into nonzero premises. Target premises define the target
domain and remain distinct from candidate-specific guard failures until the
partition compiler. Candidate guard complements become exceptional work.

Locus interning is separate from direct-formula clause membership. A projected
denominator contributes one arbitrary-width bad clause
`c_0=0 AND ... AND c_k=0`, with denominator-term and parameter-monomial
provenance on every interned locus. It does not contribute `k+1` independent
candidate-required guards: that would turn LiteRed's conjunction into an
incorrect disjunction. Row guards retain their own one-atom clause semantics.

An identically-zero pivot/coefficient denominator makes the candidate
`IdenticallyBad`. Unknown canonicalization or unsupported symbolic structure
fails closed to typed `Unsupported` only when it is a deterministic
representation result; an exception or exhausted limit is operational.

## 8. Transition matrix

| Typed outcome | Algebra database and cursor | Target state | Events/residuals | Published rule |
|---|---|---|---|---|
| `Dependent` | Advance source; no new pivot | Dispositions unchanged; allocation, binding, and version advance to the prepared successor | Dependent event | None |
| `NoTarget` | Commit pivot; advance source | All unresolved/consumed dispositions preserved | NoTarget event | None |
| `RequiresAffineEqualityRefinement` | Commit pivot; advance source, then stop group | Selected target remains unsolved; refined epoch required | Mandatory refinement event | None |
| Certified, `B=False` | Commit pivot; advance source | Consume exactly selected Ready target | Certified event; no exceptional leaf | Full applicable rule |
| Certified, mixed `B` | Commit pivot; advance source | Consume exactly selected Ready target | Certified event plus every exceptional child | Every applicable leaf |
| `IdenticallyBad` | Commit pivot; advance source | Selected target remains unresolved | Rejected-candidate event; no duplicate residual | None |
| `Unsupported` | Commit pivot; advance source | Selected target remains unresolved | Typed unsupported event/requeue provenance only; no duplicate exceptional residual leaf | None |
| Authentication, stale/foreign token, allocation, arithmetic, limit, or panic failure | Commit nothing | Unchanged | None | None |

A rejected pivot is committed once and is not offered to a second target. A
later source row is reduced by it and may solve the same unresolved target.
The persisted first matching target remains final for the current candidate.

## 9. Atomic owner transition

Before any mutation, prepare and admit all of:

- the staged database successor/replacement owned by the transaction;
- target-state successor, using `Some(Ready)` only for Certified;
- run-disposition successor;
- complete event-vector replacement;
- complete guarded-rule-vector replacement;
- complete residual/refinement-vector replacement;
- aggregate statistics and every new retained/peak capacity.

Only after all preparations succeed may the move-only commit tail:

1. commit the staged database row;
2. swap in the prebuilt target state;
3. swap in prebuilt events, rules, residuals, run disposition, and statistics;
4. drop predecessor owners.

The tail performs no allocation, Symbolica operation, GMP arithmetic,
formatting, hashing, replay, or other fallible work. It must be panic-free in
release behavior. A preflight error returns the exact owning terminal. A
post-database invariant failure is a fatal internal-consistency error and must
be made unreachable through complete preflight; it cannot honestly return a
retry token after possible mutation.

## 10. Resource contract

Every phase is admitted before allocation or expensive native work. Resource
statistics and limits must cover:

- already-live staged transaction/database ownership, using
  `max(staged_live_prospective_retained_bytes,
  staged_live_observed_retained_bytes)`;
- current target-state and catalog combined ownership;
- the exact recentered-row owner already retained by Ready;
- exact target constants, centered shifts, physical keys, descent comparisons,
  boundary endpoints/counts/values, and their GMP magnitude bits;
- Symbolica polynomial normalization, factor inspection, composition, and
  native temporary bytes;
- each retained child-certificate prefix plus current child scratch;
- complete WhenBad owner retained bytes;
- target predecessor/successor/catalog overlap;
- event/rule/residual/refinement replacement capacities and control blocks;
- owner-wide combined-live and native-scratch peaks.

The implemented Ready geometry/descent/hazard checkpoint currently reports an
incremental subphase census and explicitly excludes the pre-existing Ready
graph. That local limit is useful for exact retry tests but is not the complete
Phase C owner-wide contract above. The owning `WhenBad` compiler must project
this child from a remaining aggregate budget and combine its prospective and
observed retained/native-work census with the authenticated staged transaction,
target state, Ready owner, and later condition/partition children before any
publication can be admitted.

Project every child compiler from the remaining aggregate budget. Never reset
a child to default limits. Before materializing boundary events, preflight the
complete exact cardinality, the `usize` conversion, value bit envelopes,
vector capacity, retained bytes, and native temporary work. Check prospective
minimum capacity before allocation and observed capacity afterward.

Do not double-charge shared `Arc` payload graphs, but do charge every new
control block, pointer slot, vector capacity, and uniquely owned transcript.
The target-state predecessor + successor + shared-catalog peak already has a
model in `GeneratedAffineResidualGroupExactTargetState::prepare_successor`
(`src/generated_affine_residual_group_exact_targets.rs:1210-1328`).

Every reported resource dimension requires an exact-limit success test and a
one-below transactional rejection test. Failures return the same owning
typestate with database, cursor, targets, events, capacities, and statistics
unchanged.

## 11. Replay and redaction invariants

- Public/debug views expose locators, counts, predicate classes, and resource
  census only. Private shifts, boundary values, raw conditions, source
  expressions, and transaction identities remain redacted unless an internal
  replay view needs them.
- A rule, residual, or refinement handle is authorized by its retained current
  owner allocation and committed event, not by equal visible fields.
- Replay rejects cross-session, cross-database, sibling target-state,
  abandoned-transition, stale-version, reordered-condition, modified-leaf,
  or changed-resource transcript substitutions.
- Replay is deterministic under spare vector capacity, concurrency, and
  serialization round trips. Nonces remain private and non-wrapping.
- No application path accepts or returns an unsealed raw
  `ParametricRelation` as rule authority.

## 12. Required implementation gates

### Phase A gates

- NoTarget commits exactly once, advances both versions, preserves every
  target, emits no rule, and drop remains inert.
- Equality commits exactly once, retains its locator/refinement proof, emits a
  mandatory refinement event, and prevents further same-epoch staging.
- Stale, foreign, wrong-allocation, one-below, allocation, and caught-panic
  paths return the exact input outcome and mutate nothing.
- Source-surface checks prove there is no transaction extractor or untyped
  production commit bypass.

### Exact `WhenBad` gates

- Differential small-coordinate mathematical transcripts against the old
  lineage, used strictly as an oracle after erasing its authority.
- Exact shifts, target keys, descent, and boundary values beyond `i64`,
  including a `2^4096` case, with no downcast path.
- Cover zero denominator and exact parameter-identity semantics: `n+d` is never
  identity-bad, `n(n+d)` is bad exactly at `n=0`, and multi-parameter/full
  coefficient vectors, the zero-parameter dummy equivalent, clauses wider
  than two atoms, and one-below atom-arena budgets. Also cover
  active-coordinate pinch, inactive-coordinate activation, numerator
  vanishing/nonvanishing on a boundary, and multiple simultaneous hazards.
  Test LiteRed radical/noninteger-power fail-closed behavior separately as
  `IdenticallyBad`; a deterministic authenticated RustRed representation limit
  is typed `Unsupported`, while limit exhaustion or panic is operational and
  returns the owner unchanged.
- Empty collected RHS as a valid all-domain zero rule, not a malformed row or
  an inferred master.
- `CollectjList`-equivalent canonical collection: duplicate/equivalent RHS
  integrals combine before classification, including exact coefficient
  cancellation that removes a would-be denominator or leak hazard.
- Stable target-premise/row-guard/denominator ordering, associate deduplication,
  inherited truths, and no invalid converse implications.
- All-applicable, all-exceptional, and mixed applicable/exceptional partitions.
- Exact and one-below limits for every GMP, Symbolica, retained, scratch,
  cardinality, comparison, and publication resource.
- A topology-neutral arity-21, many-RHS, multiple-hazard Ready/condition stress
  row, without a topology name, loop-count dispatch, or injected recurrence
  coefficient.
- A generic six-loop family-generation gate proving `L^2=36` independently
  replayable IBP source rows per seed, followed by a session/scheduler batch
  gate that consumes 36 authenticated sources. One arity-21 row is not a
  substitute for this distinct source-batch test.

### State/publication gates

- Full transition-table test covering Dependent, NoTarget, equality,
  Certified, IdenticallyBad, Unsupported, and every operational-failure class.
- Rejected pivot remains in the database and reduces a later row that can solve
  the same target.
- Rejection does not try a second target; Certified consumes only the first
  persisted matching Ready target.
- Mixed Certified commit publishes every applicable leaf and queues every
  exceptional leaf while consuming the coarse target exactly once.
- Applicable specialization reproduces the complete exact recentered
  relation coefficient-for-coefficient and shift-for-shift. Exceptional
  specialization is refused by the rule handle and routed to the matching
  residual authority.
- Failure injection before each prepared replacement leaves database, cursor,
  targets, run disposition, events, rules, residuals, capacities, and stats
  byte-for-byte unchanged.
- Tamper, redaction, replay, serialization, concurrency, and abandoned-sibling
  authority tests.

All Rust test gates use licensed, GMP-enabled Symbolica and run in parallel.
No test or build enables `no_gmp`; no test invokes FORM. Concrete one-loop and
later vacuum/scattering topologies may validate the finished generic pipeline,
including numerator/denominator cancellation closure, but cannot replace the
topology-neutral unit, property, and replay tests above.
Artifact tests additionally compare deterministic single-worker and
multi-worker checksums while the surrounding test suite remains sharded and
parallel.

## 13. Completion criteria and integration order

Implement in this order:

1. typed NoTarget commit and sealed equality suspension — completed;
2. extend the common fully prepared tail with the minimal chronological event
   ledger and owner-wide replacement preparation — completed for Dependent,
   NoTarget, and equality-refinement dispositions;
3. replace the upstream eager all-orthant case inventory with a replayable
   shared normalized-source owner and bounded direct normalized-formula
   target-frontier search so `K=21` families can enter the exact session
   without first materializing either `2^K` cases or a complete MTBDD; retain
   the MTBDD only as a compact-case/repeated-query backend under its own
   measured construction budget;
4. exact-`Integer` geometry, descent, boundary, condition, and pullback cores —
   independent-cylinder descent/lazy hazards implemented; general compact-
   affine geometry plus conditions and pullbacks active;
5. owning current-lineage exact `WhenBad` terminal compiler;
6. atomic Certified/rejected disposition, sealed rules, and exceptional work;
7. extend the implemented chronological replay with durable rule/residual
   manifest validation, concrete application, and provider integration;
8. topology-based validation from one loop upward.

A phase is complete only when its success disposition, retry ownership,
resource envelope, replay path, source-surface seal, exact/one-below tests, and
parallel GMP test gate all pass. No phase may bridge a missing proof by
constructing an old-lineage certificate, hard-coding a topology, downcasting
generated arithmetic, invoking FORM, or treating a failed candidate as a
master integral.
