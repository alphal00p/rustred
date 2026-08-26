# Replayable persistent sector-elimination database

Status: historical, non-normative architecture plus an isolated
replay-reference slice, 2026-08-13. The governing design has since moved to
typed current-lineage transactions and permits different generic solver and
pivot policies after exact semantic validation.
This note is a source-level design for replacing repeated cumulative
elimination rebuilds in RustRed. The fast incremental kernel and fixed-point
integration are not implemented yet.

## Scope

The target is LiteRed's in-memory `SolvejSector` database protocol—`init`,
`clean`, `submit`, and `solve`—implemented with Rust and Symbolica rational
polynomials. The database is topology- and loop-count-independent. A concrete
topology may appear only in tests.

This layer does not generate IBPs, decide `WhenBad`, discover residual cases,
infer masters, or apply lower-sector reduction rules. The production target
incrementally eliminates an authenticated, ordered stream of generated `K(n)`
equations and exposes pivot events to the residual scheduler. The landed
reference slice instead rebuilds each prefix and trusts its caller for source
authentication. Those surrounding proof boundaries remain owned by
`GeneratedSymbolicRowSpanCertificate`,
`GeneratedWhenBadCompiler`, `ParametricSectorCoverageCertificate`, and the
family fixed-point compiler.

## Exact LiteRed lifetime and protocol

The Mathematica implementation is precise about the database lifetime:

- `SolvejSector` selects the ordinary or Fermat implementations of the four
  operations at `vendor/LiteRed2/Source/LiteRed2026.m:2377-2381` and calls
  `init` once at line 2428.
- The outer loop selects a contiguous residual-case group at lines 2430-2438.
  It then calls `clean[dbase,vars]` at line 2439. The ordinary implementation
  replaces the pivot-rule list with `{0->0}` and clears pending equations at
  lines 2614-2615.
- Therefore LiteRed does **not** union one elimination basis across unrelated
  residual-case groups. The database persists only inside the active group.
  A Rust per-sector store may retain several case-database certificates for
  replay, but their algebraic pivot spans must remain isolated.
- Inside one group, `startp`, `pointsdone`, and `badconditions` are initialized
  at lines 2446-2449. The depth loop generates equations only for points not
  already in `pointsdone` at lines 2471-2476.
- `submit` replaces the pending equation batch; it does not clear committed
  pivots (`submiteqs`, lines 2628-2629). Thus the pivot database grows across
  exact-diamond depth layers and also survives a change of the first remaining
  `startp` within that case group (lines 2496-2516).
- `solveeqs` walks the pending equations in source order (lines 2648-2659).
  Every equation is passed to `Solvej` and may add a pivot. It stops only when
  the newest pivot's left side matches the active selector. The already
  consumed prefix is committed and the unconsumed suffix remains pending for
  the next `solve` call.
- If `WhenBad` cannot compile an emitted rule, LiteRed adds that left side to
  `except` (lines 2501-2505) and continues solving. The pivot is **not**
  removed from the database; it may simplify later equations.

This gives three distinct persistent objects:

1. the family-wide generated IBP/LI row span;
2. a case-group-local triangular elimination database, persistent across
   submissions within that group; and
3. the sector-wide list of accepted guarded rules, persistent across groups.

Only object 2 is the subject of this design.

## What `Solvej` means algebraically

With the default `SubstituteAlways -> False`, LiteRed performs forward
triangular elimination (`LiteRed2026.m:2164-2195`):

1. collect one equation as a sparse linear combination of integrals;
2. choose its most complex surviving integral;
3. if that integral is already a database pivot, substitute the older rule
   and continue;
4. normalize the first nonzero, previously unpivoted leading coefficient;
5. prepend the new rule to the database.

It does not retroactively rewrite older rules when a new pivot is committed.
Consequently a reduced row-echelon form is neither required nor equivalent to
the default candidate-priority transcript. Recursive rule application later
performs the effective back-substitution.

RustRed's `ParametricElimination::build` has the same essential forward
contract (`src/parametric_elimination.rs:336-497`). For each source row it:

- reduces by earlier normalized pivots in pivot ordinal order;
- discards an exact zero row;
- chooses the hardest remaining `IndexShift` under the fixed ordering;
- divides by its Symbolica `RationalPolynomial<IntegerRing, u16>` coefficient,
  retaining the divisor's nonzero guard and origins; and
- records a replay trace containing the source-row ordinal, prior-pivot
  reductions, and divisor.

The current adaptive search nevertheless rebuilds that full cumulative
elimination at every depth (`src/adaptive_rules.rs:278-423`) and independently
for every residual anchor (`src/generated_family_fixed_point.rs:1469-1500`).
The mathematics is reusable; the mutable incremental state is missing.

## Incremental kernel and equivalence theorem

For one exact residual-case group, fix:

- family and `K(n)` context fingerprints;
- sector and integral ordering;
- a case-group identity and active-symbol signature;
- the shared generated row-span identity;
- arithmetic and resource limits; and
- an ordered source-row stream split into submitted batches.

The live state contains normalized pivots `P = [p0, ..., pk)`. Appending one
source row `r` performs:

```text
for p in P, in pivot ordinal order:
    c := coefficient(r, pivot_shift(p))
    if c != 0:
        r := r - c*p
        record (pivot_ordinal(p), c)
if r == 0:
    record Dependent
else:
    s := hardest shift in support(r)
    d := coefficient(r, s)
    p_new := r/d, with exact divisor guard provenance
    record Pivot { shift=s, divisor=d, reductions, unit_relation=p_new }
    append p_new to P
```

This is equivalent to rebuilding `ParametricElimination` over the complete
ordered prefix. The proof is induction on source-row ordinal. The induction
hypothesis makes every prior normalized pivot and trace identical. Both
algorithms therefore apply identical reductions to the next row, obtain the
same exact Symbolica rational polynomial coefficients, make the same zero
decision, and choose the same hardest support element under one total order.
Normalization and guard propagation are deterministic, so the new pivot and
trace are identical. Future columns do not affect an earlier pivot because
they are absent from that earlier row.

This theorem requires ordered concatenation, not set union. The implementation
must not silently deduplicate equal equations: LiteRed's `pointsdone` prevents
duplicate point generation, while a duplicate submitted row is still a real
consumed event that reduces to zero and consumes ledger budget.

## Proposed Rust types

The algebraic kernel should be selector-neutral. Candidate filtering belongs
to the scheduler because a `WhenBad`-unsupported candidate remains an active
pivot.

```rust
struct PersistentParametricEliminationDb {
    scope: EliminationCaseScope,
    pivots: Vec<ParametricPivotEquation>,
    pending: VecDeque<PendingSourceRow>,
    consumed_rows: usize,
    batches: Vec<SubmittedBatch>,
    ledger: PersistentEliminationLedger,
}

struct EliminationCaseScope {
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    row_span_manifest: Arc<str>,
    sector: SectorMask,
    ordering: ParametricEliminationOrdering,
    case_group_id: SymbolicSectorCaseGroupId,
    active_index_positions: Box<[usize]>,
}

enum SourceRowRecipe {
    Generated {
        basis_ordinal: usize,
        translation: IndexShift,
    },
    VerifiedWholeRowSymmetryTransport {
        basis_ordinal: usize,
        symmetry_ordinal: usize,
        translation: IndexShift,
    },
}

enum ConsumedRowOutcome {
    Dependent,
    Pivot { pivot_ordinal: usize },
}
```

The public protocol mirrors LiteRed without importing its global-state model:

- `new(scope, shared_row_span, limits)` corresponds to `init`;
- `reset_case_group(new_scope)` finalizes the old case certificate and creates
  an empty algebraic database, corresponding to `clean`;
- `submit(batch_locator, recipes)` installs the next pending queue and rejects
  a second submit while a suffix remains. LiteRed's raw assignment would
  overwrite such a suffix, but its valid scheduler traces either consume it or
  cross an explicit `clean` boundary. The Rust rejection makes accidental
  loss impossible while remaining equivalent on valid traces;
- `consume_next()` commits exactly one row and returns its dependent/pivot
  event;
- `solve_until(selector)` is a convenience loop that stops after the first
  newly committed pivot accepted by the caller's selector, retaining the
  pending suffix; and
- `checkpoint()` freezes an immutable replay certificate.

The selector receives a borrowed pivot event. It may return `Emit`, `Skip`, or
`Excluded`. All three leave the pivot committed. Authentication and `WhenBad`
compilation occur after `Emit`; failure must update the scheduler's exclusion
set, not mutate this database.

## Source authentication and storage

The live database must not accept arbitrary `ParametricRelation` values at the
production boundary. A submitted row is regenerated from a compact
`SourceRowRecipe` against the shared
`GeneratedSymbolicRowSpanCertificate`. This preserves the current generated
source boundary:

- the basis ordinal resolves to a canonical IBP/LI row or an already verified
  complete-identity symmetry transport;
- `translated` is replayed exactly in the authenticated `K(n)` context;
- the recipe records the exact lattice shift and resulting row identity; and
- family/context/row-span manifests are checked before any arithmetic.

The certificate should retain recipes and algebraic traces, not a clone of
every translated source row. Replay regenerates one source row at a time,
checks its row manifest, applies the recorded prior-pivot reductions and
divisor, and discards it. Only normalized pivot relations need persistent
materialization. The family-wide row span remains one shared `Arc`.

For an initial bounded slice, accepting rows from a trusted caller is safe only
as a low-level, explicitly prevalidated API; it must not be wired directly
into a production provider. Family/context/arity and manifests are structural
validation, not generated-source authentication. Its certificate must still
bind the complete ordered source manifest and replay all rows.

## Candidate priority and batch transcript

Candidate priority is not merely `sort(pivots)`. It is the event order induced
by:

1. the ordered `preparepoints` layer;
2. generated relation order within each point;
3. sequential reduction/commit of each equation;
4. the current case selector; and
5. the exact set of left sides excluded from emission.

Each `SubmittedBatch` therefore records:

- its residual material locator, anchor/start-point locator, and local depth;
- the ordered point recipes and generated-row ranges;
- the pending cursor before and after every `solve_until` call;
- every consumed-row outcome;
- every pivot offered to the selector and the selector disposition; and
- the pivot ordinal finally handed to `GeneratedWhenBadCompiler`, if any.

Replay must reconstruct the pending suffix. Otherwise a certificate could
silently reorder equations following a successful candidate and change all
later pivot priorities.

## Back-substitution boundary

The historical persistent-database proposal preserved LiteRed2's triangular
rule order as its deterministic replay policy. That is not a global RustRed
compatibility requirement. Under this proposal there are two sound consumers:

- demand reduction recursively applies an emitted rule and then reduces its
  right-hand-side integrals through the provider stack; or
- an optional, separately certified export pass traverses pivots in reverse
  dependency order and back-substitutes only strictly simpler pivots.

The export pass must not mutate the discovery transcript. It should retain a
trace for every substitution and prove acyclicity from the integral ordering.
Uncovered lower-sector integrals remain columns. They are never promoted to
masters by elimination failure.

## Symbolica coefficient requirements

All arithmetic remains in RustRed's authenticated Symbolica wrappers:

- `ParametricCoefficientContext` fixes the exact base-plus-index variable map;
- coefficients are `RationalPolynomial<IntegerRing, u16>`;
- addition, multiplication, division, and normalization use the existing
  checked exact-algebra methods and `ExactAlgebraLimits`;
- pivot division retains numerator nonzero conditions and every `GuardOrigin`;
- zero is structural exact zero after checked normalization, never a numeric
  sample; and
- replay compares complete relation and guard provenance, not expressions
  printed as strings.

Using raw Symbolica operators in the database would bypass variable-map,
degree, monomial, coefficient-bit, guard-cardinality, and normalization limits
already enforced by `ParametricElimination`.

## Resource ledger

Per-call limits are insufficient for a persistent object because many small
submissions can evade them. The database needs monotone case-local and
sector-aggregate ledgers. Every allocation is preflighted before translation,
clone, normalization, or retention.

At minimum charge:

- submitted batches and points;
- enumerated source recipes and translation components;
- consumed source rows, input terms, guards, and guard origins;
- retained pivots, terms, guards, and origins;
- reduction attempts and sparse coefficient updates;
- maximum transient row width and bytes;
- pending rows and pending source bytes;
- pivot trace reductions and trace bytes;
- source-manifest and certificate bytes;
- candidate offers/emissions; and
- replay work independently from construction work.

The sector store additionally charges the sum across finalized case-group
databases. `clean` releases live transient memory but does not erase cumulative
work or retained certificate bytes. Integer counters use checked arithmetic;
resource exhaustion returns a typed unresolved interruption before mutation.

## Interaction with residual fixed point

The current fixed-point compiler creates a fresh
`AdaptiveParametricRuleProvider` for each residual anchor and rebuilds every
cumulative stencil. Integration should happen in two stages:

1. Within one authenticated residual case group, replace the adaptive
   cumulative rebuild with one database. Each newly discovered exact-diamond
   layer is submitted once, and the scheduler consumes pivot events until it
   finds an applicable candidate or exhausts the layer.
2. Let the residual scheduler own a `SectorEliminationStore` containing
   isolated case-group databases plus the global accepted-rule composition.
   Reusing a prior database is allowed only when the exact case-group identity,
   active symbolic coordinates, ordering, source row span, quotient snapshot,
   and submitted-prefix transcript all match.

Do not union pivots across distinct residual cells merely because they belong
to the same sector. That would differ from LiteRed's `clean` boundary and can
move a division guard or a case-specialized zero assumption into another
locus.

Likewise, solved proper-subsector feedback is a separate quotient layer.
`SolvejSector` itself only adds `SR` and `ZerojRule` for fully numeric points
at lines 2475-2476; it does not import already solved proper-subsector
`jRules` into this database. RustRed may add a proof-bearing lower-sector
normalizer as an enhancement, but its snapshot fingerprint must become part of
`EliminationCaseScope` and every substitution must be replayed.

## Replay checks

An immutable case-database certificate is accepted only if replay proves:

1. exact family, context, sector, ordering, case group, active-symbol, shared
   row-span, quotient-snapshot, configuration, and limit binding;
2. exact batch and source-recipe order;
3. exact pending cursors and no overwritten unconsumed suffix;
4. every dependent-row result;
5. every pivot shift, prior-pivot factor, divisor, normalized relation, and
   guard origin;
6. every selector offer/disposition and emitted pivot ordinal;
7. construction and replay ledgers; and
8. equality with a full-prefix `ParametricElimination::build` reference in
   validation tests.

The certificate reports interrupted/resource-limited explicitly. It cannot
turn an unfinished pending batch, a failed authentication, or exhausted
resource budget into `Uncovered` or `Master`.

## Smallest safe landing slice

The smallest non-topology-specific slice is now landed as
`src/persistent_parametric_elimination.rs`: an isolated, replayable append-only
reference database built on the existing `ParametricElimination`:

1. bind a context and fixed ordering;
2. accept ordered caller-prevalidated row batches, with the production
   row-span recipe boundary explicitly deferred;
3. reject `submit` while an unconsumed suffix exists;
4. retain batch boundaries and a cumulative ordered source manifest;
5. rebuild `ParametricElimination` over the prefix at each checkpoint;
6. expose only newly created pivot ordinals in source/event order; and
7. replay every checkpoint and compare it to an independent full-prefix build.

This reference slice is intentionally not the performance implementation. It
locks down `init/submit/solve/clean`, batch-prefix, candidate-priority, and
replay semantics without touching the fixed-point compiler. A subsequent
internal refactor can extract an incremental builder from
`ParametricElimination`; black-box tests must require byte-for-byte-equivalent
manifests, pivots, traces, guards, and candidate events between the reference
rebuild and the incremental kernel.

The first concrete validation family should be the one-loop massive tadpole:
split translated generated rows into at least two depth batches, assert that
the database prefix after each submission equals a full rebuild, consume only
through the first selector match, resume the pending suffix, and verify that
`clean` starts an algebraically empty second case scope. A two-loop sunset test
then checks that multiple same-sector depth batches produce the same ordered
pivot/candidate transcript as the present cumulative adaptive search. Expected
reduction coefficients belong only in assertions.

## Reference-slice self-audit and validation

The prefix invariant was checked against the implementation of
`ParametricElimination`, not assumed from its API. `validate_source` constructs
the column inventory by sorting each shift with the injective
`IntegralComplexityKey` and an `IndexShift` tie-breaker
(`src/parametric_elimination.rs:825-933`). Appending a row can insert new
columns and change the numeric ranks of older columns, but it cannot change the
pairwise order of any two columns already present. A previously processed row
contains no future column. Reducing it can introduce only columns from an
earlier pivot, hence only earlier-prefix columns. `hardest_shift` chooses the
maximum rank within that support (`src/parametric_elimination.rs:1102-1125`),
so every earlier pivot choice remains unchanged. Exact row reduction,
normalization, row identifiers, divisor guards, and guard origins are then
deterministic. Resource limits can reject a rebuild but do not select a
different algebraic result.

The reference database enforces this theorem at runtime: every newly rebuilt
prefix must retain all previous pivot shifts, traces, unit relations, and
complete guard provenance byte-for-byte, with at most one new pivot for the
one newly consumed row. A mismatch poisons the database.

Pending-suffix behavior was also checked against the raw LiteRed loop. A
successful `solveeqs` call consumes through the selected pivot and stores the
unconsumed suffix with `Drop[eqs,i]`. Repeated calls continue that suffix. A
call that finds no matching pivot has nevertheless processed the complete
batch before the next depth submission overwrites it. If no cases remain,
`clean` may discard a suffix. RustRed represents these valid traces by
`solve_until`, full consumption before another `submit`, and a certificate
that may explicitly end with pending rows. It rejects an accidental submit
over a live suffix; this is a safe strengthening of the raw assignment, not a
change to valid scheduler behavior.

`tests/persistent_parametric_elimination.rs` uses generated one-loop massive
tadpole rows for fifteen parallel black-box checks. They cover ordered
duplicate/dependent events, selector suffixes, exact full-prefix agreement,
clean boundaries, replay, fixed and cumulative input bounds, bounded V1
manifests, replay-clone proxies, zero-row prefix work, poisoning, and live plus
certificate-replay manifest coexistence. Four module-private corruption tests
independently alter row-manifest metadata, retained statistics, batch ranges,
and the consumed cursor and require typed rejection before replay cloning.

The focused command

```text
cargo nextest run -j4 --test persistent_parametric_elimination --no-fail-fast
```

passes 15/15. The private replay-audit filter passes 4/4, and `cargo check
--tests` passes; output is limited to pre-existing Symbolica SIMD,
tensor-constructor, and unrelated dead-code warnings.

### Resource-accounting hardening

The reference database now accounts for the non-arithmetic work and retained
payload that the first slice originally left implicit:

- `max_retained_batch_label_bytes` is a cumulative label-retention bound, not
  merely a per-label bound.  The matching statistic is preflighted before a
  batch or row is committed.
- `ParametricRelation::stable_manifest_with_limit` uses the same canonical
  encoder as `stable_manifest`, but streams row identities, shifts, and guard
  origins through bounded writers. Before converting a retained Symbolica
  rational polynomial to an `Atom`, its full expression is streamed through a
  fallible byte counter; the resulting Atom's binary payload is then checked as
  a second proxy. Symbolica's V1 canonical atom printer itself recursively
  materializes and sorts `String`s and is not exposed as a fallible writer, so
  the final canonical temporary remains unavoidable and is checked against the
  exact output budget immediately after creation. The binary-Atom proxy is not
  claimed to upper-bound that canonical String. A successful bounded encoding
  remains byte-for-byte identical to the unbounded compatibility API, and
  writer failures preserve count-overflow versus ordinary limit errors.
  Elimination's aggregate source-manifest encoder also calls this bounded API.
  Submission uses the remaining aggregate manifest budget for each row,
  records its exact byte length, and rejects the complete batch without
  mutation if any row fails.
- Every rebuild is charged by its full source-prefix row count, term count,
  and exact source-manifest bytes.  These cumulative prefix-width ledgers are
  deliberately independent of sparse reductions and updates: a sequence of
  zero/dependent rows still consumes `1 + 2 + ... + N` row work and cannot
  hide quadratic reference-rebuild cost behind zero arithmetic statistics.
  Manifest bytes are charged twice because `ParametricElimination::build`
  materializes the manifest once during construction and once during its
  mandatory replay.
- Certificate replay treats all stored counters as decoder metadata. Before
  any deep clone it recomputes row scope, coefficient/polynomial validity,
  guard structure and origins, sparse integral slots, distinct columns,
  anchor-plus-shift keys, exact row-manifest lengths, labels, and contiguous
  covering batch ranges. It compares the recomputed totals with persisted
  statistics, then audits every batch-local clone bound. Separate
  coexisting-source-clone bounds charge the original certificate plus the
  complete prospective replay database. Worst-case doubled retained rows,
  sparse integral slots, and row-manifest bytes are preflighted at submission,
  so a completed certificate cannot retain a batch or aggregate database that
  its configured replay is unable to clone. These are deliberately named
  source-clone proxies: they do not claim to bound coefficient monomials,
  exponent arrays, or simultaneously live elimination pivots, which remain
  governed by the nested exact-algebra/elimination limits.
- A separate peak limit charges simultaneously live elimination source
  manifests. It includes an outer certified elimination during replay, the
  prior live prefix, both new build/replay aggregate manifests, and the largest
  bounded row-manifest temporary. This closes the otherwise hidden coexistence
  peak while retaining exact V1 bytes.
- Fixed elimination inputs are staged before submission commits: cumulative
  sparse terms, guards, guard origins, the distinct column union, anchor-plus-
  shift arithmetic, and complexity-key construction must all fit. This avoids
  accepting a batch that is guaranteed to poison on its next consume while
  leaving dependency-sensitive pivot/retained-pivot limits at consume time.

All cumulative counters use checked arithmetic. Mutating protocol paths use a
fallible pending-cursor query; the original infallible observer saturates only
for API compatibility. Any consume-time accounting, elimination,
post-commit-selector invariant, or replay failure interrupts the mutable
database; it cannot be retried as an algebraic miss. Boundary tests exercise an exact limit and one
byte/unit below it and assert that failed submission or consumption leaves the
committed batches, events, cursors, and statistics unchanged.  In particular,
two structurally valid zero rows are rejected by the cumulative prefix-row
budget on the second rebuild, proving that the bound does not depend on a
nonzero pivot or sparse update.

The remaining trust boundary is intentional and important: the reference
method is named `submit_prevalidated_rows`. It checks family, context, arity,
ordered manifests, and exact elimination replay, but it does **not** own a
`GeneratedSymbolicRowSpanCertificate` or prove that the rows are generated
IBP/LI translations. Fixed-point integration is forbidden until submitted
rows are regenerated from retained row-span recipes and that provenance is
part of the certificate. The reference slice also does not yet encode a
symbolic residual case-group identity or selector-decision transcript; those
belong to the production store/scheduler boundary.

## Non-goals and failure semantics

- No hardcoded recurrence, loop-count dispatch, topology-name branch, or
  expected master count enters the database.
- No FORM or Mathematica execution is required.
- A database certificate is not a `WhenBad` certificate and cannot establish
  a candidate's global applicability.
- Failure to emit a selected pivot is not evidence of an empty residual locus
  or a master integral.
- A resource limit, unsupported guard, malformed source recipe, replay
  mismatch, or selector exhaustion remains a typed nonterminal result.
