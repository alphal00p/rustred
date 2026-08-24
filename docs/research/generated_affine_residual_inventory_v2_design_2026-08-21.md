# Source-neutral affine residual inventory V2

## Status

This is the implementation contract for the affine-inventory layer that follows
`generated_affine_residual_boolean_cover`.  It preserves every existing V1
schema and replay meaning.  Production behavior is independent of topology,
family name, graph shape, and loop count; concrete vacuum families are test
fixtures only.

The immediate source is one exact
`Arc<GeneratedAffineResidualBooleanCoverCertificate>`.  That certificate
already owns the unified initial-global or prior-effective source authority and
all sealed initial Boolean children.  The inventory must not retain a second
source authority, synthesize a V1 live queue, or copy prior predicates.

### Implementation checkpoint

The fresh integer-system, controlled guard-composition, sealed-origin, and
opaque initial-terminal prerequisites are implemented and audited.  The next
work is deliberately split into two bounded seams:

1. **B0 authority firewall.**  The unified prior-effective authority must
   project old target/guard payloads internally into lifetime-bound,
   source-neutral views.  No `GeneratedResidualAffineCaseLocator`,
   `ResidualAffineBranchGuardCompositionEntry`, raw condition/origin set, old
   source case, relative case, or partition may be returned by the V2
   authority or Boolean-cover API.  Existing V1/current-owner APIs stay
   unchanged.
2. **B1 inventory compiler.**  Compile the exact Boolean-cover `Arc` into the
   certificate below through one linear replay session.  Every dense Boolean
   record is consumed once; a failed child attempt poisons the session; no
   per-child V1 replay or fabricated queue is permitted.

The family scheduler remains downstream of both seams.

### Exceptional equality refinement

Preserving an exceptional predicate `P(n)=0` as an inherited premise is
necessary for soundness but is not sufficient for LiteRed-like recursive
closure.  Before the next elimination epoch, every `EqualZero` predicate must
be intersected with the inherited affine integer map (or used to quotient all
new coefficients); `NonZero` predicates remain guards.  LiteRed performs this
transition by rebuilding `noRules` with `SmartReduce`/`ToRules` and applying
the resulting substitutions to the next starts
(`LiteRed2026.m:2446,2488-2496,2522,2573-2578`).

A topology-neutral acceptance fixture is the identity map on `(n0,n1)`,
`P=n0+n1-3`, and the row
`P*J(n+e0)+J(n)=0`.  Off `P=0` the high pivot is valid; on `P=0`, composing
`n0=t, n1=3-t` must expose the lower pivot `J(n)=0`.  Treating `P=0` only as an
opaque guard would repeat the high-pivot failure and is not accepted as a
complete second epoch.

## Required ownership

The new certificate should have the following logical shape:

```rust
pub(crate) struct GeneratedAffineResidualCaseInventoryCertificate {
    schema: &'static str,
    source_boolean_cover: Arc<GeneratedAffineResidualBooleanCoverCertificate>,
    initial_affine_children: Vec<GeneratedAffineInitialGlobalAffineTerminal>,
    terminals: Vec<GeneratedAffineResidualInventoryTerminalRecord>,
    cases: Vec<GeneratedAffineResidualInventoryCase>,
    groups: Vec<GeneratedAffineResidualContiguousCaseGroup>,
    limits: GeneratedAffineResidualCaseInventoryLimits,
    stats: GeneratedAffineResidualCaseInventoryStats,
}
```

All owned child and certificate types are non-`Clone`; `Debug` output is
redacted.  A persisted terminal locator contains only the Boolean record
ordinal plus its source-neutral `(source work-item ordinal, local terminal
ordinal)` locator.  V1 source-case identifiers and old queue locators never
cross this seam.

## One-replay adapter

The frozen V1 branch and guard public compilers replay their source for each
child.  V2 instead needs one complete Boolean replay followed by positional
no-replay child compilation.  Add a sealed session borrowed from the complete
Boolean certificate:

```rust
pub(crate) struct GeneratedAffineResidualBooleanReplaySession<'scope> {
    certificate: &'scope GeneratedAffineResidualBooleanCoverCertificate,
    family: &'scope IntegralFamily,
    context: &'scope ParametricCoefficientContext,
}

impl GeneratedAffineResidualBooleanCoverCertificate {
    pub(crate) fn replay_session<'scope>(
        &'scope self,
        family: &'scope IntegralFamily,
        context: &'scope ParametricCoefficientContext,
    ) -> Result<GeneratedAffineResidualBooleanReplaySession<'scope>, Error>;
}
```

Session construction performs exactly one complete Boolean replay.  Its
`compile_ready_terminal(record_ordinal, limits)` operation:

1. resolves the terminal through its private binding;
2. requires `ReadyForAffineRecognition`;
3. selects the private sealed cover and V1 node internally;
4. clones the exact private cover `Arc`;
5. invokes a narrow `pub(crate)` no-replay branch adapter;
6. invokes a no-replay guard adapter when the branch has an affine map; and
7. authenticates cover and branch ownership with `Arc::ptr_eq` before sealing
   the child.

The session never returns a raw V1 cover, branch, guard, queue, extraction, or
source-case identifier.

Record consumption is linear as well.  A freely repeatable borrowed
`compile_ready_terminal` would allow the same fresh child to be compiled more
than once after the single parent replay.  Use either a monotonically consumed
record cursor or a consuming per-record authorization; arbitrary repeated
positional compilation is forbidden.

## Terminal mapping

Every Boolean record is consumed exactly once and retained in identical order.

| Boolean outcome | Inventory result |
|---|---|
| `SourceProvedEmpty` | passthrough source-empty terminal |
| `BooleanProvedEmpty` | passthrough Boolean-empty terminal |
| `ReadyForAffineRecognition` | compile one sealed initial affine child |
| `PriorUnsupported` | affine-unsupported terminal, preserving atoms and reasons by reference |
| `PriorActionable` | one actionable case from the inherited map and guards |
| `PriorExceptionalDomain` | one actionable case preserving every exceptional predicate |
| `PriorExceptionalLeak` | one actionable case preserving leak kind and every predicate |

The public V2 outcome vocabulary may coalesce prior unprocessed and unconsumed
targets as actionable.  Their distinction remains in a private binding and is
reauthenticated through the retained Boolean certificate.

An initial affine child has one of four outcomes: proved empty, unsupported,
guard contradiction, or actionable.  Only actionable terminals own cases.

## Narrow views

`authenticated_terminal_view(record_ordinal)` and
`authenticated_case_view(case_ordinal)` reauthenticate the complete chain back
to the Boolean source.  A case view may expose only:

- an affine-map borrow;
- positional guard entries;
- positional additional exceptional predicates;
- constants and free positions; and
- group and source-neutral locator metadata.

Initial and ordinary prior-actionable cases have no additional exceptional
predicates.  Exceptional cases borrow their exact predicate sequence from the
retained source.  Unsupported views expose positional atoms and reasons.

No view exposes an owning `Arc`, raw V1 certificate, source queue, old source
case, effective owner, or private relative partition.

### Guard-origin neutrality

The existing V1 branch-guard compiler cannot be called unchanged by this
layer.  Its retained `GuardOrigin::ResidualAffineBranchNonzeroGuardSubstitution`
contains the V1 source case, work-item ordinal, and ready-node ordinal, and the
origin is reachable through the public condition on a guard entry.  The V2
fresh adapter must therefore select a distinct, source-neutral composition
mode which emits only `GuardOrigin::GeneratedAffineSealedCondition`; the
opaque initial child retains the complete record/work-item/node/locus binding
needed for replay.

For the same reason, prior-actionable and prior-exceptional Boolean source
views must not return a raw `ResidualAffineBranchGuardCompositionEntry`.
They return a narrow projected entry view exposing the structural-locus
ordinal, mapped polynomial, composition statistics, and a class projection.
For base-assumption and free-index-dependent classes the projection exposes
the exact condition polynomial but never its V1 origin set.  A later sealed
rule may reconstruct only the public-safe generated-affine origin after its
owner chain has authenticated the original predicate.  No old source-case
locator crosses the V2 Boolean or inventory seam.

The projection begins at the unified source-authority boundary.  Its prior-
effective variant must not return the raw effective-residual source view,
because that view reaches the old case locator and `source_case`, while its
actionable children reach raw V1 guard entries.  Wrapping that value only
after it has crossed the authority API is insufficient: the authority and its
replay session return a narrow source-neutral prior projection directly.

## Geometry grouping

Reuse the generic V1 geometry key:

```text
(ambient arity, ordered free positions, row-major compact linear matrix)
```

Constants are not part of the geometry key.  A case stores the exact offset
`b_case - b_anchor`.  Move anchor constants into the group builder so the
geometry core can be shared without making a V2 case masquerade as a V1 case.

Cases with equal geometry but different exceptional premises may share a
geometry group.  They must continue to resolve predicates from their own
source record; an anchor's premises are never inherited by another case.

## Resource accounting

Logical GMP payload envelopes use
`Gmp(N,B) = ceil(B/8) + N*size_of::<usize>() + max(N-1,0)`.  The final term
covers the worst-case gap between rounding each retained integer's bit payload
separately and rounding the aggregate bit count once.

Before V2 inventory construction, branch and guard compilation need sealed
retained/fresh/temporary memory-envelope helpers equivalent to the Boolean
child helper.  A child census excludes the shared Boolean source graph and
includes:

- branch and guard certificates, control blocks, handles, and uniquely owned
  vector/integer/map payloads;
- comparison units, comparison bytes, and integer-bit work;
- branch fresh peak;
- branch retained plus guard fresh peak; and
- final branch-plus-guard retained bytes.

The inventory additionally accounts for logical slots for terminals, children,
cases, and groups; cloned constants, matrices, free-position arrays, and
offsets; temporary geometry; and prefix-retained overlap during conversion.
Allocator capacity and allocator rounding are never used as certificate data.

For one ready terminal, the sequential peak is bounded by:

```text
retained prefix
+ max(
    branch fresh peak,
    branch retained + guard fresh peak,
    branch/guard retained + geometry conversion peak
  )
```

Prior maps and predicates are counted as authenticated reference work and are
not charged as copied transitive payload.  All proportional scans are admitted
from positional counts before their first lookup.

Replay authenticates old retained bytes from sealed raw payloads and uses
limit-derived fresh-child envelopes before reconstruction.  With the shared
Boolean source graph excluded, inventory replay peak is:

```text
old inventory retained + fresh inventory peak
```

## Conservation

Compilation and replay enforce all of the following:

- inventory terminal count equals Boolean terminal count;
- record order and locators are identical;
- every Boolean record is visited once;
- ready count equals initial child count and branch compilation count;
- initial affine outcomes partition all ready terminals;
- guard compilation count equals the guarded initial branches;
- prior unsupported maps once to unsupported;
- every prior actionable/domain/leak record maps once to actionable;
- actionable terminal count equals case count;
- every case belongs to exactly one group;
- group order and anchors follow first source occurrence;
- every anchor offset is exact;
- empty, unsupported, and contradictory terminals own no case;
- exceptional cases resolve their original predicates in exact order; and
- all cover and branch allocations match by identity.

Any resource interruption or authentication failure returns no partial
certificate.

## Acceptance tests

Tests run in parallel and use topology-specific inputs only as fixtures.  The
minimum matrix covers:

- source-empty, Boolean-empty, affine-empty, unsupported, contradiction, and
  actionable initial outcomes;
- multiple ready DPLL terminals and stable ordering;
- prior unsupported, unprocessed, unconsumed, exceptional-domain, and
  exceptional-leak sources;
- exact positional preservation of atoms, reasons, guards, constants, free
  positions, and exceptional predicates;
- equal geometry with different constants and offsets;
- equal geometry with distinct exceptional premises without premise aliasing;
- distinct matrices or free positions producing distinct groups;
- exactly one Boolean replay and no public per-child replay;
- same source allocation acceptance and independently equal allocation
  rejection;
- corrupted record/child/case/group links;
- exact and one-below values for every count, work, retained, temporary,
  fresh, replay, and recursive-comparison limit;
- replay after external source handles are dropped;
- concurrent replay through a shared `Arc`; and
- differential initial-source output against V1 after erasing V1-only locator
  fields.

Later ordering, re-elimination, matching, `WhenBad`, and effective-sector
layers must receive adjacent V2 wrappers bound to `Arc<V2 inventory> + case
ordinal`.  Converting V2 cases back into authored or fabricated V1
certificates is forbidden.

## Implementation staging

Keep the next patches independently auditable:

1. Finish the linear fresh-integer authorization, then add only the fresh
   branch bundle, the separate V2 sealed-origin guard bundle, logical-memory
   helpers, and the opaque initial affine terminal.  No inventory is built in
   this stage.  Branch outcomes which are empty or unsupported—including an
   atom-unsupported branch with a diagnostic map—must not retain a guard-plan
   authorization.  A guarded affine branch is the sole consumer.
2. Add the one private raw-cover executor beside
   `GeneratedAffineInitialGlobalBooleanCover`, projected prior guard views, and
   the one-replay Boolean session.  Test sparse V1 node ordinals, exact Arc
   identity, zero public child replays, zero integer replays, sealed guard
   origins, and rejection of every non-Ready record before adding grouping.
3. Add the source-neutral case inventory and geometry grouping.  Then test
   terminal/case/group conservation, prior identity mapping, exceptional
   premise isolation, exact offsets, replay/tampering, four-thread replay,
   and exact/one-below limits for every positive resource field.

Parallel tests use licensed GMP-enabled Symbolica and `cargo nextest run -j4`.
Any replay instrumentation is thread-local so parallel execution does not
create false counts.  The high-risk regression checks are sparse node ordinal
versus dense terminal position, zero-guard compilation, accidental use of a
public V1 branch/guard compiler, repeated fresh authorization, legacy guard
origin leakage, allocator-capacity accounting, and recursive comparison of a
shared cover/branch/integer payload through multiple aliases.
