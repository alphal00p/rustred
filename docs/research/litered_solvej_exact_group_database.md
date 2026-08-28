# LiteRed `Solvej` semantics and the exact RustRed group database

Status: LiteRed2 source analysis and RustRed design rationale.

Scope: the generic, topology-independent equation database used while deriving
parametric reduction rules. This document records relevant LiteRed2 behavior,
the exact affine-coordinate bridge already present in RustRed, and the
transactional/replay design RustRed currently chooses as a versioned policy
for a safe and efficient Rust implementation. LiteRed2's source organization,
mutation order, pivot
order, and incidental behavior are not compatibility requirements. This note
does **not** specify a vacuum topology, a loop-count-specific recurrence
catalogue, or a FORM translation.

The primary LiteRed source is
`vendor/LiteRed2/Source/LiteRed2026.m`.  Vakint is used only as an application
and validation oracle; its authored FORM recurrences are not RustRed input.

## 1. Source observations and current RustRed choices

1. LiteRed's default `Solvej[eq, db]` is a **top-reduction** algorithm.  It
   repeatedly inspects the hardest surviving integral.  If that integral is
   already reducible by the database, it substitutes that term and repeats.  If
   it is not, it immediately makes that integral the new pivot.  It does not
   first reduce the row by every older pivot.
2. The stored right-hand side is therefore only top-reduced.  It can contain an
   easier integral for which a database rule already exists.
3. One database is shared by all targets in one affine-translation group and is
   cleared between groups.  Pivots survive target changes, depth changes, and
   rejected `WhenBad` candidates inside the group.
4. RustRed currently admits generated rows as authenticated **raw source
   rows**, in source order. A per-case pre-eliminated pivot list would not be
   equivalent to this chosen replay contract, although future algorithms may
   replace the contract if they prove the same accepted mathematics and better
   performance.
5. A candidate becomes a public reduction rule only after exact recentering and
   `WhenBad` compilation.  Algebraic pivot discovery and public-rule coverage
   are separate state transitions.
6. Failure to publish a rule is not evidence for a master integral.  Neither a
   dependent row, exhausted batch, bounded search, unsupported condition, nor a
   rejected candidate may be converted into a master claim.

## 2. Exact LiteRed `Solvej` behavior

`Solvej` is documented as solving for the most complex integral, and its default
is `SubstituteAlways -> False`
(`vendor/LiteRed2/Source/LiteRed2026.m:2121-2143`).  The standalone form
collects terms, removes zero leading coefficients, selects the hardest integral,
and divides by its coefficient
(`vendor/LiteRed2/Source/LiteRed2026.m:2146-2161`).

The database form implements the following algorithm
(`vendor/LiteRed2/Source/LiteRed2026.m:2164-2195`):

```text
solve_top_reduced(raw_equation, db):
    row := collect_and_simplify(raw_equation)

    loop:
        if row has no integral terms:
            return Dependent if its free term is zero, otherwise Inconsistent

        p := hardest integral with a nonzero simplified coefficient
        c := coefficient(row, p)

        if a database rule matches p:
            # Substitute this hardest term only, then recollect.
            row := collect_and_simplify(row - c*p + c*db[p])
            continue

        # The first unknown hardest term becomes the pivot immediately.
        rhs := -(row - c*p) / c
        unit := p - rhs == 0
        assert every integral on rhs is strictly easier than p
        prepend db with p -> rhs
        return NewPivot(unit)
```

In RustRed the database lookup is an exact lookup in the retained common
physical-key frame.  Guarded normalization must retain all incoming coefficient
guards and the nonzero condition introduced by division by `c`.

The top-reduced distinction is observable.  Suppose `A < B`, the database
already contains `A -> a`, and the new row is `B + c A == 0`.  Default LiteRed
stores `B -> -c A`; it does not store `B -> -c a`.  The optional
`SubstituteAlways -> True` branch does exhaustive substitution and also rewrites
older rules, but `SolvejSector` uses the default branch
(`vendor/LiteRed2/Source/LiteRed2026.m:2176-2183`).

Termination of the lookup loop follows from the persisted strict integral
order: every stored pivot right-hand side is below its left-hand side, so each
successful lookup strictly lowers the row's maximum integral.  RustRed must
verify this invariant when a unit pivot is admitted rather than assuming it.

### Ordering authority

LiteRed permits an automatic or custom `jsOrder` matrix
(`vendor/LiteRed2/Source/LiteRed2026.m:1444-1508`), derives integral complexity
from it (`vendor/LiteRed2/Source/LiteRed2026.m:1511-1564`), and uses
`HighjIndex` to select the hardest integral
(`vendor/LiteRed2/Source/LiteRed2026.m:1678-1686`).  Collection preserves the
integral/coefficient separation used by this selection
(`vendor/LiteRed2/Source/LiteRed2026.m:4171-4202`).

RustRed's current exact physical key stores its ordering policy and complete
formal complexity tuple; its `Ord` implementation compares those fields before
the physical shift (`src/solver/exact_session/physical_key.rs`).  A production
database must accept keys only through its retained frame/solve plan, as the
source comments require.  The current
public policy is `RustRedUnshiftedV1`
(`src/sectors.rs:716-743`).  Custom LiteRed-compatible `jsOrder` remains a
future compatibility requirement; a database must never silently change its
comparator after construction.

## 3. `SolvejSector` group scheduling

### 3.1 Forming and ordering groups

LiteRed instantiates each requested logical case as an integral.  It gathers two
cases when the difference of their integral-index tuples consists entirely of
integers, i.e. when they differ only by an integer translation in the same
affine geometry.  Cases inside a group are sorted from harder to easier; groups
are sorted by number of fixed parameters, number of numeric indices, and the
ordered positions of symbolic indices
(`vendor/LiteRed2/Source/LiteRed2026.m:2416-2425`).  The selected group is then
reversed, giving the actual target order from easier to harder
(`vendor/LiteRed2/Source/LiteRed2026.m:2430-2438`).

RustRed's inventory represents one group by ambient arity, case ordinals, an
anchor case, free positions, a compact affine matrix, and exact anchor offsets
(`src/generated_affine_residual_case_inventory.rs:708-716`).  Geometry equality
requires the same arity, free positions, and compact linear coefficients
(`src/generated_affine_residual_case_inventory.rs:4127-4142`).  The first case
creates the group anchor
(`src/generated_affine_residual_case_inventory.rs:4230-4249`), and every case
offset is calculated exactly as its constant vector minus the anchor constant
vector
(`src/generated_affine_residual_case_inventory.rs:4417-4458`).

The immutable solve plan at `src/solver/exact_session/plan.rs` already
materializes easier-to-harder order.  It constructs a
physical key for every group case and stable-sorts by that key, with inventory
position as the tie breaker.

### 3.2 Database lifetime

LiteRed initializes the database before the outer group loop, but calls `clean`
once at the start of each selected group
(`vendor/LiteRed2/Source/LiteRed2026.m:2428-2439`).  Consequently:

- pivots persist across all targets, depths, and submitted batches in one group;
- a pivot rejected for public use by `WhenBad` still remains algebraically
  available to reduce later rows;
- no pivot crosses an affine-group boundary;
- accepted conditional rules survive outside the group and their exceptional
  loci are re-enumerated by the outer residual scheduler.

The elementary database helpers confirm the intended lifetime.  `cleandb`
clears pivots and pending equations, `submiteqs` replaces the pending batch, and
`solveeqs` scans rows in source order
(`vendor/LiteRed2/Source/LiteRed2026.m:2602-2629` and
`vendor/LiteRed2/Source/LiteRed2026.m:2648-2659`).  During that scan, every
consumed row may add a pivot.  The first newly/latest pivot matching the current
selector is returned; the consumed prefix is committed and the unconsumed
suffix remains pending.  Pivots that do not match the selector remain useful.

### 3.3 Depth and point order

At each depth, LiteRed generates rows in deterministic point order and then in
the recurrence-source order, submits the flattened batch, and drains matching
pivots before increasing depth
(`vendor/LiteRed2/Source/LiteRed2026.m:2470-2517`).

`diamond[l,d]` is an exact Manhattan shell `|q|_1 = d`, not a ball
(`vendor/LiteRed2/Source/LiteRed2026.m:6094-6097`).  Previously visited points
are removed, so successive shells are cumulative without duplicate row
generation.

For fully numeric cases, LiteRed unions and deduplicates the shell around **all**
remaining starts and globally sorts the physical points
(`vendor/LiteRed2/Source/LiteRed2026.m:2682-2695`). Under the current versioned
replay policy, RustRed merges numeric case schedules by common physical point
and source ordinal; merely concatenating complete per-case schedules changes
that policy's row priority.

For symbolic cases, LiteRed uses only the first unresolved start and keeps fixed
numeric coordinates in the sector
(`vendor/LiteRed2/Source/LiteRed2026.m:2698-2711`).  When removal of a target
changes that first start, depth resets to zero
(`vendor/LiteRed2/Source/LiteRed2026.m:2496-2516`).

The existing per-case reelimination component is a useful authenticated row
source, but explicitly does not implement same-group ownership, adaptive depth,
target matching, or `WhenBad`
(`src/generated_affine_residual_case_reelimination.rs:1-17`).  Its scheduled
rows are compiled in depth/point/source order
(`src/generated_affine_residual_case_reelimination.rs:896-988`).  The future
group database must ingest the authenticated raw row and its witness, not the
component's current pre-eliminated pivot output.

## 4. Candidate selection, recentering, and `WhenBad`

LiteRed's inner loop asks the database for a new/latest pivot matching any
remaining target, recenters it, selects the concrete case it covers, and invokes
`WhenBad`
(`vendor/LiteRed2/Source/LiteRed2026.m:2467-2490`).

Let one affine group be written as

```text
case u:       J(A n_F + b_u + q)
anchor:       b_0
case offset:  o_u = b_u - b_0
physical key: r = o_u + q
common form:  J(A n_F + b_0 + r)
```

These are exact arbitrary-precision integer coordinates.  RustRed's physical
frame implements `local -> physical` as `o_u + q` without `i64` arithmetic and
the inverse as `r - o_u` (`src/solver/exact_session/physical_key.rs`).

If top reduction produces a unit pivot at physical shift `r`, let `r_F` denote
the components of `r` at the group's free positions.  Recenter by

```text
coefficient variables:  n_F -> n_F - r_F
target offset:           t = r - A r_F
RHS physical shifts:     s -> s - r
```

The pivot belongs to the first unresolved solve-plan target whose offset is
`o_u = t`. After recentering, its left-hand side is the target's unshifted
parametric integral and its pivot key is zero. RustRed's live authority-free
recenter kernel implements the target-offset, coefficient-translation, and
centered-key formulas, and the exact session applies them only to the sealed
top-reduced unit row. The earlier raw exact-relation/i64 differential adapter
has been retired because it ran before that authority boundary; Git retains it
as historical design evidence.

### 4.1 Exact LiteRed `WhenBad`

LiteRed collects all RHS integral/coefficient pairs and computes two kinds of
bad locus
(`vendor/LiteRed2/Source/LiteRed2026.m:2565-2569`):

1. **Denominator degeneration.**  It factors coefficient denominators.  For
   each factor it finds the index locus on which that factor is identically zero
   as a polynomial in the independent external parameters (all parameter
   coefficients vanish).
2. **Inactive-sector leak.**  For a coordinate inactive in the target sector,
   an RHS integral is outside the reduction domain when that coordinate becomes
   at least one.  The branch counts as bad only when the corresponding
   coefficient numerator does not vanish there.  An unconditional leak yields
   literal `True`.

The result is simplified over the integer sector domain.  If simplification
introduces noninteger powers, LiteRed conservatively returns literal `True`.
Moving an active coordinate below one is allowed: that is a valid lower-sector
term, not a leak.

RustRed's compiler also proves uniform strict descent before classifying a rule
(`src/when_bad.rs:1360-1390`), retains prior guards and coefficient-denominator
conditions (`src/when_bad.rs:1392-1452`), and partitions boundary numerator-zero
from numerator-nonzero branches (`src/when_bad.rs:1454-1494`).  Those are
intentional Rust safety refinements, not behavior to attribute to the
Mathematica implementation.

### 4.2 Target-state transitions

Let `C` be the current case domain and let compilation partition it into
applicable leaves and exceptional leaves.  `B` below is LiteRed's aggregate bad
condition.

| Result | Public rule | Residual work | Current target | Algebraic pivot |
|---|---|---|---|---|
| `B = False` / certified with full applicable coverage | publish on `C` | none | `Consumed` | keep |
| `B != True` / certified mixed leaves | publish on `C && !B` | enqueue `C && B` | `Consumed` in this inventory epoch | keep |
| `B = True` / identically bad | none | keep `C` unresolved | `Unresolved` | keep |
| Rust `Unsupported` | none | keep the uncovered domain unresolved | `Unresolved` | keep |
| operational or resource error | none | unchanged | unchanged | no partial commit |

LiteRed performs the accepted and rejected branches at
`vendor/LiteRed2/Source/LiteRed2026.m:2492-2505`.  On acceptance it records the
rule, removes the case, and adds the bad locus to the next outer residual pass.
On literal `True`, it records a bad-rule diagnostic and adds the pivot LHS to
`except`; this prevents the same latest database pivot from being yielded again
while retaining it for algebraic reduction.  RustRed can represent the new
pivot as a typed one-shot event, so it need not reproduce the `except` pattern
hack.

In RustRed, `Unsupported` explicitly means uncovered/requeue, not master
(`src/when_bad.rs:694-716` and `src/when_bad.rs:1153-1166`).  The older affine
coverage layer already follows the rule that only a certified result consumes a
target; unsupported and identically bad outcomes remain available
(`src/generated_residual_affine_group_effective_coverage.rs:631-706`).

## 5. Production group database design

### 5.1 Immutable owner identity

One database owner must bind:

- the exact `Arc`-owned inventory and authority;
- family and coefficient-context fingerprints;
- group ordinal, anchor case, exact physical frame, and immutable solve plan;
- the persisted integral-ordering policy;
- source/reelimination authorities used to authenticate raw rows;
- arithmetic, coordinate, event-log, and retained-memory limits.

Allocation identity is part of authorization even where mathematical values are
equal.  Keys constructed by another frame, another inventory epoch, or another
policy are rejected.

### 5.2 Mutable state

The database state should contain:

```text
targets[solve_ordinal] = Unresolved | Consumed

pending_batch:
    authenticated raw-row recipes/witness locators
    next unconsumed row cursor

pivots:
    exact physical pivot key -> sealed guarded unit equation

events (append only):
    batch submission
    actual hardest-pivot substitutions and factors
    dependent rows
    new pivots and guarded divisors
    unmatched candidate pivots
    accepted/rejected target candidates
    target transitions

published:
    certified public rule leaves
    exact residual leaves for the outer scheduler

control:
    inventory epoch, status, counters, limits, interruption marker
```

No public rule is stored in the algebraic pivot map, and no conditional public
rule is used to top-reduce later raw rows.  The map contains exact unit equations
valid under their retained algebraic guards; publication is a separate
domain-coverage product.

### 5.3 Row transaction

For each pending raw row:

```text
stage_row(recipe, witness):
    authenticate family, context, source, case, point, depth, and source ordinal
    row := regenerate the exact raw Symbolica relation
    map every local key q from case u to physical key o_u + q

    reductions := []
    loop:
        collect and exactly simplify coefficients
        if row == 0:
            return StagedDependent(reductions)

        p := maximum physical key in row
        c := coefficient(row, p)

        if pivots contains p:
            row := row - c * pivots[p]
            append (p, c) to reductions
            continue

        inverse := checked_guarded_divide(1, c)
        unit := inverse * row
        verify coefficient(unit, p) == 1
        verify every other key in unit is strictly below p
        return StagedNewPivot(p, c, unit, reductions)
```

This is the Rust form of default `Solvej`.  The stored equation can be represented
as `J(p) + sum_q a_q J(q) + a_0 == 0`, with public RHS
`-sum_q a_q J(q) - a_0`.

After a new pivot is staged:

1. If its recentered offset does not identify the first eligible unresolved
   target selected by the solve plan, commit the algebraic pivot, event trace,
   and row cursor, then continue scanning.
2. If it matches, stage exact recentering and `WhenBad` compilation before
   mutating target or publication state.
3. On certified applicable coverage, atomically commit the pivot, trace, cursor,
   public rule leaves, residual leaves, and `Consumed` transition.
4. On identically bad or unsupported coverage, atomically commit the algebraic
   pivot, trace, cursor, and rejected-candidate event, but leave the target
   unresolved.
5. On panic, allocation failure, resource limit, authentication error, or other
   operational failure, commit none of that row's mutations.  Either leave it
   retryable at the same cursor or mark the database fail-stop interrupted.

A dependent row commits only its dependent event and cursor advance.  A new
batch may replace the pending batch only after the old suffix is exhausted or
the group is complete; otherwise submission must fail rather than silently lose
rows.

The current persistent elimination component supplies useful cursor, replay,
and fail-stop patterns
(`src/persistent_parametric_elimination.rs:1-16`,
`src/persistent_parametric_elimination.rs:740-1053`), but its algebraic reducer
does not implement this policy's `Solvej` kernel.

### 5.4 Replay contract

Before publishing results, replay must:

1. Reauthenticate the exact parent allocations, group, frame, plan, family,
   context, limits, and source manifests.
2. Regenerate every consumed raw row from its retained recipe/witness in
   submitted order.
3. Recompute local-to-physical mapping `o_u + q`.
4. Replay the recorded sequence of **actual hardest-term** lookups, checking each
   pivot key and exact factor.
5. Recompute zero/dependent or new-leader status; for a new pivot, repeat guarded
   division, unit-coefficient verification, and strict-order verification.
6. Recompute the target offset `r - A r_F`, verify selection of the first
   eligible unresolved solve-plan target, and repeat recentering.
7. Recompile `WhenBad`, compare applicable/residual leaf manifests, and replay
   the target transition.
8. Verify batch cursors, all append-only events, published-rule manifests, and
   residual manifests exactly.

Replay must not substitute a mathematically equal but independently allocated
inventory/frame, must not replay a full chronological pivot sweep in place of
the recorded hardest-term sequence, and must not infer missing events from final
state alone.

## 6. Difference from current `ParametricElimination`

The existing generic component is explicitly a guarded, replayable sparse
eliminator over parametric relations
(`src/parametric_elimination.rs:1-13`).  It is valuable infrastructure, but its
row semantics differ from LiteRed:

| Concern | LiteRed default `Solvej` | Current `ParametricElimination` | Current versioned group DB |
|---|---|---|---|
| Reduction choice | inspect current hardest term; substitute only if that term is known | iterate through all committed pivots in chronological order | exact hardest-key lookup loop |
| Stop point | first unknown hardest term | after the complete pivot sweep | first unknown hardest term |
| Stored RHS | top-reduced only | fully reduced against earlier pivots encountered | top-reduced only |
| Prior-rule rewriting | no | pivots are composed into later rows | no |
| Lifetime | one affine target group | one fixed submitted row set | mutable batches across a group |
| Selector | first new/latest pivot matching unresolved target | no `SolvejSector` target selector | typed candidate event plus solve plan |
| `WhenBad` | gates public rule, not algebraic pivot retention | outside component | transactional publication transition |

Current construction calls `reduce_by_pivots` before choosing the leader
(`src/parametric_elimination.rs:732-851`), and `reduce_by_pivots` walks every
prior pivot (`src/parametric_elimination.rs:1754-1822`).  Only afterward does
`hardest_shift` select the pivot (`src/parametric_elimination.rs:1945-1965`).
Reusing that function unchanged would turn the example `B + c A` into
`B -> -c a`, alter guard propagation, and potentially narrow direct coverage.

The current implementation reuses exact coefficient arithmetic, guarded
division, sparse relation updates, resource accounting, and certificate
encoding while implementing the top-reduction control loop and its trace as a
separate kernel.

## 7. What is never a master-integral proof

The following outcomes are explicitly **non-master** outcomes:

- a zero or dependent generated row;
- no currently available rows;
- exhaustion of a pending batch or source schedule;
- a new pivot that does not match the current target selector;
- `WhenBad = True`, `IdenticallyBad`, or `Unsupported`;
- depth, time, memory, integer-bit, or other resource exhaustion;
- an unresolved target after a bounded search;
- symmetry canonicalization or a quotient representative by itself;
- failure of an optional backend or simplifier.

The per-case scheduler already documents `NoAvailableRows` as unresolved work,
not master evidence
(`src/generated_affine_residual_case_reelimination.rs:10-17`).

LiteRed temporarily reports remaining starts as `misFound` after search-depth
exhaustion (`vendor/LiteRed2/Source/LiteRed2026.m:2519-2520`), but in a complete
run it finally recomputes masters as the integer-sector complement of all
published rule domains
(`vendor/LiteRed2/Source/LiteRed2026.m:2541-2547`).  The `NMIs` early exits are
user-supplied expected-master-count heuristics
(`vendor/LiteRed2/Source/LiteRed2026.m:2258-2259` and
`vendor/LiteRed2/Source/LiteRed2026.m:2453-2473`), not mathematical proofs.

RustRed correctness mode must therefore return `Unresolved` for incomplete
coverage.  It may declare a master only from an explicit master policy or
certificate, or from a completed outer fixed-point search whose certified rule
domains have a proven integer-domain complement.  Any LiteRed-style `NMIs`
shortcut must be opt-in and labeled heuristic.

## 8. Vakint boundary: application oracle, not derivation source

Vakint's `integrateduv.frm` demonstrates the downstream semantics RustRed must
eventually reproduce in pure Rust/Symbolica:

- one-loop numerator lowering and recurrence application
  (`vendor/gammaloop/crates/vakint/form_src/alphaloop/integrateduv.frm:17-29`);
- two-loop topology mapping, scalar-product numerator lowering, zero/symmetry
  canonicalization, guarded hardcoded recurrences, and separate masters
  (`vendor/gammaloop/crates/vakint/form_src/alphaloop/integrateduv.frm:31-152`);
- three-loop topology mappings and tensor/scalar numerator lowering
  (`vendor/gammaloop/crates/vakint/form_src/alphaloop/integrateduv.frm:155-240`);
- three-loop zero/symmetry rules and the beginning of its hardcoded recurrence
  catalogue
  (`vendor/gammaloop/crates/vakint/form_src/alphaloop/integrateduv.frm:250-320`);
- fixed-point guarded rule application and merging of generated raising shifts
  (`vendor/gammaloop/crates/vakint/form_src/alphaloop/integrateduv.frm:1101-1116`);
- explicit checking for unreduced leftovers
  (`vendor/gammaloop/crates/vakint/form_src/alphaloop/integrateduv.frm:1118-1125`);
- reduction from higher to lower loop count
  (`vendor/gammaloop/crates/vakint/form_src/alphaloop/integrateduv.frm:1129-1139`);
- master substitution as a separate final phase
  (`vendor/gammaloop/crates/vakint/form_src/alphaloop/integrateduv.frm:1162-1218`).

The RustRed application layer should lower tensor/scalar products to shifted
integrals, canonicalize zeros and symmetries, choose an applicable generated
guarded rule, instantiate it as Symbolica expressions, and repeat to a fixed
point.  Master-value substitution remains separate.  Vakint's authored FORM
recurrences are validation oracles only: RustRed must neither execute FORM nor
copy a topology-specific recurrence catalogue into the generic derivation
engine.

## 9. Rust safety additions (not claims about LiteRed)

The following are deliberate **RustRed additions**:

- arbitrary-precision Symbolica/GMP coordinates and checked coordinate
  preflights instead of machine-integer arithmetic;
- exact `Arc` allocation identity and authority checks for inventory, frame,
  plan, source, family, and coefficient context;
- typed target, row, pivot, rejection, interruption, and replay states;
- fallible allocations plus explicit arithmetic, term, guard, integer-bit,
  retained-byte, and event-count limits;
- stage/commit atomicity and fail-stop interruption instead of partially mutated
  lists;
- append-only provenance and deterministic replay certificates;
- explicit uniform-descent proof before public rule admission;
- a sharper boundary split that distinguishes coefficient-numerator zero from
  nonzero branches;
- a typed one-shot new-pivot event, eliminating LiteRed's `except` workaround;
- conservative `Unsupported -> Unresolved`, never `Unsupported -> Master`.

LiteRed itself uses mutable definitions/lists and disk reservation markers; it
does not provide these transaction, resource-envelope, allocation-identity, or
replay guarantees. RustRed deliberately strengthens those properties. The
row order and pivot policy described above are a versioned RustRed design and
its current replay contract, not a requirement to reproduce LiteRed2 internals; they
may be replaced by a demonstrably equivalent or stronger generic algorithm.
The accepted recentering identities and coverage semantics remain mathematical
requirements.

## 10. Implementation acceptance checklist

A production implementation is ready to replace the current per-case
elimination path only when tests establish all of the following:

1. The `A -> a`, `A < B`, `B + c A == 0` regression stores `B -> -c A` and its
   replay records no lookup of `A` before choosing `B`.
2. A row whose hardest term is known performs that one lookup, recollects, and
   repeats until it is dependent or reaches the first unknown hardest term.
3. Two cases in one affine group reuse pivots; two distinct groups cannot share
   keys or pivots.
4. Numeric multi-start shells are globally merged/deduplicated; symbolic
   scheduling follows only the first unresolved start and resets depth when it
   changes.
5. Unmatched and rejected pivots remain available internally without consuming
   a target or being yielded twice as the same new-pivot event.
6. Exact recentering satisfies `r = o_u + q`, `t = r - A r_F`, coefficient
   substitution `n_F -> n_F - r_F`, and RHS centering `s -> s - r` for large GMP
   offsets as well as small integers.
7. Certified, mixed, identically bad, and unsupported `WhenBad` outcomes follow
   the transition table exactly.
8. Injected panic/allocation/resource failures leave no partial row, cursor,
   pivot, target, rule, or residual mutation.
9. Replay rejects reordered rows, altered reduction factors, a full-pivot-sweep
   trace, foreign but value-equal parent allocations, and altered coverage
   leaves.
10. Every exhaustion/rejection path returns unresolved work and never silently
    creates a master integral.

Concrete one-, two-, and three-loop vacuum families are appropriate fixtures
and Vakint is an appropriate reduction-output oracle.  They validate this
generic protocol; they must not introduce topology- or loop-count-specific
branches into it.
