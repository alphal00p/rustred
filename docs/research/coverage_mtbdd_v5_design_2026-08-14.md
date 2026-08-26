# Coverage V5: reduced MTBDD over authenticated factor atoms

> **Post-design scaling note (2026-08-25).** The authenticated residual-path
> cursor now avoids flattening all terminal paths, but a genuine all-inactive
> all-36 `L=6`, `K=21` diagnostic still constructs 49 atoms and 268,427 rooted
> nodes before traversal. V5 remains the compact-case oracle and an optional
> repeated-query classifier. The six-loop primary path will instead search
> one target frontier directly from a shared authenticated normalized-coverage
> owner, under separate formula-search limits; this does not invalidate the
> V5 representation or replay contract described below. The focused source is
> [`full_six_loop_k21_source_finds_first_residual_with_bounded_cursor_memory`](../../src/parametric_sector_residual_path.rs);
> licensed parallel Nextest run `d1b3d6f2-70fe-4da2-ba36-9a671f48080a`
> passed the complete cursor slice 6/6 with 997 tests skipped, and the ignored
> stress took 132.985 seconds. The tested source SHA-256 was
> `acbaff2560b78135b90ba347cfcc16f3d74ea1fe0329e4cf7a02022fb96514ce`.
> The command was `SYMBOLICA_LICENSE='<process-local license>'
> SYMBOLICA_HIDE_BANNER=1 nix develop -c cargo nextest run --lib -j 4
> --run-ignored all --no-fail-fast -E 'test(parametric_sector_residual_path)'`.

Status: **revised staged design; production Coverage V5 is not implemented**.

The generic decision-DAG core at
[`src/coverage_decision_dag.rs`](../../src/coverage_decision_dag.rs) is now
declared crate-private from `src/lib.rs`.  The frozen hash in section 14.1
passed 23/23 focused tests standalone with four test threads and under licensed
parallel nextest.  The current source contains direct iterative
ITE/apply, dead-priority-suffix truncation,
manager-branded live references, rooted reachable export/rebuild, operation-wide
memo/work accounting, and first-error transactional poisoning.  Its final
SHA-256 is
`86fef90ed96d5c57de8775411150db59eab0d682d9acf9889d3618650a9e3025`.
The final independent resource/replay audit found no remaining P0/P1 core
issue.  Any later core edit invalidates this freeze and requires both focused
commands to be rerun.  These focused results do not constitute outer V5
acceptance.

The frozen core is now connected to a private stage-1 compiler through an
authenticated backend-neutral formula IR.  Generated `WhenBad` attempts are
replayed against their family/context/sector scope, their ordinary polynomial
loci are authenticated by the Symbolica-backed coefficient context, and
canonical factor-zero disjunctions compile to a product-free MTBDD.  Dedicated
formula-normalization and MTBDD resource matrices have exact/one-below tests,
including bounded canonical sorting, atom staging/sort/dedup, typed-root
allocation/traversal, and aggregate generated-source/condition censuses.  A
private owning stage-1 certificate now retains the shared generated row span,
ordered attempts, normalized formula payload, rooted MTBDD, limits, and stats;
replay reauthenticates and rebuilds that complete typed payload.  This is not
yet an accepted Coverage V5 artifact: the deterministic length/count-delimited
source-identity encoding, the explicit-V4 adapter over the same normalized IR,
the complete owner-level mapping of nested frozen-core limits, provider
integration, and outer transactional resource matrix remain staged.

Original design date: 2026-08-14.  Last revised: 2026-08-25.

This note specifies the next representation for RustRed's global parametric
sector coverage.  It records the concrete failure which motivated the change,
the semantics which must not change, the proposed proof objects and cumulative
resource limits, and the full migration required by discovery, fixed-point,
depth-growth, provider, and residual-region consumers.  Except for the
lib-wired generic core just described, it is not a claim that Coverage V5, its
queue integration, or its Boolean-cover integration already exists.

This is one scaling submilestone inside RustRed's unchanged full scope.  It
does not narrow the requirement to reproduce LiteRed's generic generated
parametric reductions and Vakint's tensor/rule-application semantics in pure
Rust with Symbolica.

The change is entirely within RustRed's pure-Rust/Symbolica architecture.
Mathematica, LiteRed, FORM, and Vakint remain read-only specifications or
validation oracles; none is a runtime dependency.

## 1. Observed failure and what it proves

The exact boundary was observed in the focused ignored regression
[`sunset_011_depth_two_coverage_uses_factored_product_without_native_expansion`](../../tests/sunset_depth_two_product_support.rs#L44).
That test directly reconstructs the generic depth-two candidate layers for
residual sector `011` and invokes global V4 coverage.  It failed with the typed
symbolic-sector resource error

```text
resource: "symbolic sector case splits"
requested: 65537
limit: 65536
```

The full end-to-end
[`automatic_depth_two_growth_closes_connected_sunset_j211`](../../tests/generated_family_depth_growth.rs#L368)
stress regression has **not** yet been rerun after that focused observation.
The 65,537 evidence must therefore be attributed only to the focused ignored
test; it is not evidence that the complete depth-growth reduction reached the
same boundary or now succeeds.

The configured defaults responsible for that exact boundary are
[`max_splits = 65_536`](../../src/symbolic_sector_cases.rs#L68) and
[`max_live_cases = 65_537`](../../src/symbolic_sector_cases.rs#L69).  A binary
split increases the number of live leaves by one, so raising only the split
limit would merely expose the adjacent live-case limit.

This observation proves only the following:

1. 65,536 binary split records had already been admitted in that coverage
   construction;
2. the compiler attempted one more split; and
3. the failure was a deterministic, fail-closed resource refusal, not a wrong
   algebraic result.

It does **not** yet measure the eventual number of splits, the eventual MTBDD
node count, the number of reachable residual conjunctions, or the final
runtime after the representation change.  Those quantities remain acceptance
measurements, not assumptions in this design.

The failure appeared after the product-locus fallback had become exact and
non-materializing.  A factor product whose conservative expanded support is
above the materialization cutoff is routed to
`CandidateProductZeroRouting::Factored` in
[`route_candidate_product_zero_loci`](../../src/parametric_sector_coverage.rs#L2357).
The current implementation then puts every factor-zero atom back into the
candidate formula, and the global overlay emits ordinary binary polynomial
splits for those atoms.  This is algebraically sound, but sequential
composition can repeatedly refine every still-open factor branch.

## 2. Versioned V5 invariant: ordered `noRules`-inspired update

Within this versioned V5 replay policy, RustRed preserves the following
LiteRed2-inspired priority semantics; this ordering is not a global
compatibility constraint on future solver kernels.
Candidates are processed in persisted order.  For candidate `i`:

- every region already covered by an earlier candidate remains frozen;
- candidate `i` is selected on the part of the still-open region where its
  authenticated bad-domain formula is false;
- the part where its bad-domain formula is true remains open for later
  candidates; and
- exhausting the supplied candidates yields `Uncovered` or `Unsupported`,
  never an inferred master integral.

This is the behavior documented and implemented by the current ordered
composition loop in
[`compose_global_partition`](../../src/parametric_sector_coverage.rs#L1085).
For bad-domain functions `bad_i`, the terminal-valued function is equivalently

```text
cover(i, open) = ITE(not bad_i,
                     DescendingRule(i),
                     cover(i + 1, open))
```

with the appropriate final residual terminal.  Coverage V5 changes the
representation and construction algorithm for this function.  It does not
change this function.

## 3. Why the smaller interventions are insufficient

### 3.1 Raising the split and live-leaf defaults

A paired, bounded raise of `max_splits` and `max_live_cases` would preserve
mathematical semantics.  Arc sharing also makes such a raise safer than it was
when every descendant path deep-cloned each polynomial.  It is useful as a
diagnostic experiment.

It is not the production scaling fix:

- it retains one node for every binary split produced by the same expansion;
- it retains one explicit final path for every leaf;
- later candidates can multiply the number of open factor paths; and
- downstream code still scans, clones, or replays the explicit partition.

The independent logical predicate-reference cap remains valuable.  In the
current binary-tree representation, `max_total_leaf_predicates = 4_000_000`
is an external-path-length bound, not a polynomial-payload bound.  It would
eventually stop any cap-raise experiment.  It must not be raised merely to
move the same failure.

### 3.2 Arc sharing

The current Arc change is correct and must remain.  Each split's polynomial is
shared by its transcript record and all descendant leaf predicates; replay
also clones the Arc rather than the polynomial.  The exact term and
canonical-display byte censuses therefore charge one immutable payload per
split, while `max_total_leaf_predicates` separately charges path references.

Arc sharing reduces payload memory.  It cannot merge two logically identical
decision suffixes, and it cannot reduce `splits.len()`.  Moreover, the coverage
overlay currently calls `polynomial.clone()` before each new split in
[`overlay_candidate_bad_formula`](../../src/parametric_sector_coverage.rs#L2069),
so equal structural loci split on different branches are still distinct
per-split polynomial allocations.  Existing term/byte limits account for
those allocations honestly.

### 3.3 A persistent path tree or DAG alone

The existing split transcript is already a compact parent/child decision
tree.  Replacing flattened leaf predicate arrays with persistent linked paths
would reduce repeated Arc-reference storage, but the 65,537th logical split
would still exist.  It would therefore still reach the split limit and still
leave downstream work proportional to the number of explicit regions.

A useful global representation must merge identical **decision functions and
terminal-valued suffixes**, not only share path prefixes.  This is why V5 uses
a reduced multi-terminal binary decision diagram (MTBDD).

### 3.4 Why a compound factor-product atom is not the baseline

V5 does not need a formal `p1*...*pk = 0` atom to avoid product
materialization.  Its baseline compiles the product-zero condition directly as
the ordered Boolean OR of the already authenticated ordinary factor atoms

```text
(p1 = 0) OR ... OR (pk = 0).
```

This construction invokes no Symbolica multiplication and retains no expanded
product.  In a reduced ordered DAG it costs at most one newly retained branch
per factor before cross-candidate reduction and suffix sharing.  More
importantly, every Boolean assignment is an assignment to base factor facts;
there is no independent compound variable that can disagree with one of its
factors.

A derived compound test may later be A/B-tested as an optional, separately
versioned representation optimization.  It is not part of the V5 baseline,
must never be interpreted as a free propositional variable, and cannot become
the default unless expansion back to the base-factor OR is proved equivalent
under the criteria in section 4.  An ordinary tree plus a compound atom would
still duplicate other conditions and priority suffixes; the reduced MTBDD is
the scaling mechanism in either representation.

## 4. Algebra, Boolean semantics, and equivalence domain

All concrete loci remain authenticated Symbolica polynomials in

```text
K[n] = Q(theta_1, ..., theta_s)[n_1, ..., n_N].
```

This is an integral domain.  For nonzero factors `p_i`, the baseline product
formula is

```text
product_bad([p_1, ..., p_k])
    = factor_zero(p_1) OR ... OR factor_zero(p_k).
```

The authenticated decomposition witness and its canonical factor list must:

- contain only authenticated structural-locus ordinals;
- discard nonzero elements of the base field `K`, which are units in `K[n]`;
- reject an identically zero or malformed retained factor;
- be sorted and duplicate-free;
- collapse an empty list to Boolean false; and
- collapse a singleton list to its ordinary polynomial-zero atom.

Equivalence is defined first over **pre-compression formula assignments**.  Let
`P` be the canonical structural loci which occur in the normalized candidate
formulas before a backend performs product compression, and let
`z: P -> {false, true}` assign each such ordinary locus its zero status.  V5
evaluates those literals directly.  A concrete product locus introduced by the
V4 backend is not another independent input: for its authenticated
decomposition `[p_1, ..., p_k]`, its value is derived as
`OR_i z(p_i)`.  The same rule applies to an optional compound test.  Inputs
which assign either derived predicate independently, or inconsistently with
its factors, are outside the semantic domain and must never be used for
differential testing, reduction, or coverage claims.

Other ordinary pre-compression loci are independent in the small Boolean
oracle, even if further algebraic implications happen to hold between their
polynomials.  Coordinate and exact-divisibility restrictions are represented
by independently replayable empty-region proofs as described in section 7;
concrete-point checks separately exercise the actual polynomial algebra.

Cross-schema comparison uses a representation-neutral semantic disposition:
the exact descending candidate ordinal, the exact ordered unsupported list,
`Uncovered`, or the `ProvedEmpty` category with an independently validated
proof.  V4 empty reasons contain flattened predicate ordinals, while V5 reasons
use stable atom/factor locators, and two atom orders may select different valid
first contradictions.  Therefore raw proof-reason bytes are compared exactly
within one schema's replay, but are not required to be identical between V4
and V5.

Equivalence is also checked over concrete points.  For every in-sector integer
index point at which checked Symbolica specialization succeeds, derive `z(p)`
from the authenticated polynomial specialization and require V4, baseline V5,
and any optional compound experiment to return the same semantic disposition.
The pre-compression assignment check tests Boolean construction independently
of Symbolica; the concrete-point check tests the polynomial evaluator, every
materialized-product witness, and sector binding.  Neither check permits an
independently chosen V4 product-locus or `AnyFactorZero` bit.

### 4.1 One authenticated, backend-neutral formula IR

Backend selection happens only after one normalized candidate formula has been
built from the authenticated `GeneratedWhenBadCompilation`.  The immutable IR
retains, for every candidate:

- each clause and its schema-defined order;
- every literal's structural-locus ordinal and exact `EqualZero`/`NonZero`
  polarity;
- the canonical, strictly increasing list of atomic `EqualZero` factors before
  product compression, including exact source provenance; and
- the candidate/source ordinal and authentication identity from which it was
  derived.

Clause order is authenticated-source order with exact duplicate clauses
removed at their first occurrence; canonical factor lists are sorted and
deduplicated separately.  Another ordering policy requires another schema.
The current private `CandidateBadFormula` construction inside
`src/parametric_sector_coverage.rs` must be refactored behind this shared seam;
building separate semantic formulas independently for V4 and V5 is not an
acceptable differential test.

The explicit V4 backend may apply its existing bounded, witnessed concrete
product compression to the IR's atomic factor list.  Baseline V5 always
compiles that original list as ordinary factor-zero OR and must not call the
V4 product multiplication or recover provenance by factoring a product later.
A concrete polynomial which was already an authenticated literal before
backend selection remains an ordinary atom; this exception does not permit V5
to reuse a product introduced only by V4 compression.

## 5. Proposed V5 data model

The new objects should live in a coverage-specific module, for example
`src/parametric_sector_mtbdd.rs`.  The polynomial-only
`SymbolicSectorCasePartitionCertificate` remains the V1 proof used by local
`WhenBad` certificates and by legacy Coverage V4.

```rust
pub enum ParametricSectorCoverageAtom {
    PolynomialZero {
        structural_locus_ordinal: usize,
    },
}

pub enum ParametricSectorMtbddRef {
    Node(usize),
    Terminal(usize),
}

pub struct ParametricSectorMtbddNode {
    ordinal: usize,
    atom_ordinal: usize,
    when_false: ParametricSectorMtbddRef,
    when_true: ParametricSectorMtbddRef,
}

pub struct ParametricSectorMtbddTerminal {
    ordinal: usize,
    payload: ParametricSectorMtbddTerminalPayload,
}

pub enum ParametricSectorMtbddTerminalPayload {
    BooleanFalse,
    BooleanTrue,
    Disposition(ParametricSectorLeafDispositionV5),
}

pub struct ParametricSectorCoverageMtbdd {
    order_schema: &'static str,
    root: ParametricSectorMtbddRef,
    boolean_false: ParametricSectorMtbddRef,
    boolean_true: ParametricSectorMtbddRef,
    atoms: Box<[ParametricSectorCoverageAtom]>,
    nodes: Box<[ParametricSectorMtbddNode]>,
    terminals: Box<[ParametricSectorMtbddTerminal]>,
    source_identity: Arc<str>,
}
```

The node branch names are deliberately neutral.  In baseline V5 every node is
an ordinary `PolynomialZero`: true means equal zero and false means nonzero.
Candidate product formulas retain canonical factor-ordinal lists in their
authenticated source/formula records, but those lists do not introduce a
second kind of Boolean variable.

An optional compound experiment requires a distinct atom/order schema and a
versioned payload such as `DerivedAnyFactorZero { factor_set_ordinal }`.  Its
replay validator must expand it to the baseline OR for equivalence and its
evaluator must derive its truth from the base factor evaluations.  It is not
included in `ParametricSectorCoverageAtom` above because it is not the default
V5 certificate vocabulary.

Boolean endpoints are construction terminals in the same generic core arena.
They are persisted explicitly and must be distinct terminals with the exact
`BooleanFalse` and `BooleanTrue` payloads above.  The production rooted view
contains exactly the final coverage root plus those two endpoints; candidate
formula roots are transient reconstruction values, not additional production
roots.  Replay regenerates every candidate formula from its authenticated
source record, composes the final root, and performs reachable-only export
from that root and the endpoints.  Test-only differential artifacts may retain
role-tagged candidate-formula roots under a separate schema, but production V5
must not keep semantically dead candidate suffixes reachable merely for
diagnostics.

The outer V5 validator separately traverses the final coverage root and
requires every terminal reachable from that root to be a `Disposition`.
`BooleanFalse` and `BooleanTrue` are auxiliary construction endpoints retained
through their explicit endpoint references; neither may be reachable from the
final coverage root.  This typed-root invariant is additional to generic-core
reachability and is checked during construction, export, replay, concrete
classification, and tamper tests.

Disposition terminal payloads retain the existing semantic outcomes:

- `DescendingRule { candidate_ordinal }`;
- `ProvedEmptyLocus { reason }`;
- `Uncovered`; and
- `Unsupported { candidate_ordinals }`.

Empty-locus reasons must refer to stable atom/factor locators, not to ordinals
inside a flattened leaf predicate slice.  The full proof reason is part of the
terminal payload used for terminal interning.  Two empty terminals are merged
only when their complete retained reasons are equal.

A descending ordinal must identify a certified attempt.  Whenever the
fallback is reachable, its unsupported list is exactly the duplicate-free
list of all authenticated unsupported attempts in persisted attempt order;
unsupported attempts never acquire descending terminals.  Replay rejects
out-of-range ordinals, source-kind substitutions, reordered or partial
unsupported lists, and invalid proof locators.

## 6. Canonical MTBDD construction

### 6.1 Stable atom order

Coverage V5 defines and persists an atom-order schema.  It must not depend on
hash-map iteration or allocation addresses.  The baseline policy orders
ordinary polynomial-zero atoms by authenticated structural-locus ordinal.
Polynomials proved to differ by a nonzero element of `K` define the same
structural zero locus and must already share one ordinal; literal polynomial
equality is not required.  A bounded, failed, or unavailable associate proof
fails typed or retains distinct loci according to the existing exact policy;
it may never merge them speculatively.

Another measured policy can be introduced only under another order-schema.
The optional compound experiment has its own order-schema version; it may not
silently insert derived atoms into the baseline order.

### 6.2 Reduction and interning

Terminals are interned by their complete payload.  Nonterminal nodes are
interned by

```text
(atom_ordinal, when_false, when_true).
```

If both children are equal, construction returns the child directly.  The
unique table may be a deterministic ordered map or a non-iterated hash lookup
with exact collision buckets.  Persistent IDs, traversal, export order, and
source bytes must never depend on hash-table iteration.  Hash collisions are
resolved by exact triple comparison and both lookups and comparisons are
metered.  The current generic core uses an explicit stable FNV-1a hash for node
buckets and filters persisted objects in deterministic arena order.  Stored
ordinals use a documented topological order, and the certificate rejects
cycles, duplicate triples, duplicate terminals, and unreachable garbage.

### 6.3 Candidate composition

Each authenticated normalized candidate bad formula is compiled into a Boolean
decision diagram over V5 atoms.  For the ordinary variable
`x_l := [structural_locus_l = 0]`, `CandidateBadAtom::EqualZero` compiles to
`x_l` and `CandidateBadAtom::NonZero` compiles to `NOT x_l`.  An atomic clause
is that literal, a two-atom clause is the conjunction of its two literals, and
the complete bad formula is the OR of its clauses.  In particular, the real
symbolic leak gate is compiled exactly as

```text
(boundary = 0) AND (numerator_gate != 0)
    = x_boundary AND NOT x_numerator_gate.
```

An empty clause list is Boolean false, so its candidate is applicable on the
complete still-open domain.  Polarity is retained in the authenticated
formula record and replayed; it must never be inferred from a product-routing
shortcut.

A factored product-zero source is compiled by a
deterministic OR fold over its canonical factor-locus ordinals.  For the direct
node-building route, start at Boolean false and visit the strictly increasing
factor ordinals in reverse, constructing `branch(locus, suffix, true)`; generic
apply may produce the same reduced function but not change the persisted atom
order.  This route does not call Symbolica multiplication, retain an expanded
product, or charge a fictional native-product term count.  Concrete polynomial
literals authenticated before backend selection remain valid.  A concrete
product created only by V4 compression is derived oracle state and may not be
adopted as a baseline V5 atom.

The compiler then folds candidates in exact priority order with
terminal-valued `ITE`, preserving the ordered `noRules` semantics from section
2.  Unsupported candidate ordinals contribute only to the final residual
terminal; they never create a descending terminal.  Construction must
short-circuit semantically dead work: direct binary apply must not build an
unused truth-table row, and priority composition must truncate the suffix
after a constant-false bad formula.  On success, the persisted rooted view may
contain no node or terminal unreachable from the final root or the two
explicit Boolean endpoints.

## 7. Coordinate and divisibility implications

The current global case state contains exact polynomial decisions, fixed
coordinates, excluded coordinates, and a bounded divisibility cache.  V5
retains those proof rules in a canonical path-constraint state.

For a concrete polynomial atom, the existing implications remain valid:

- if `p | q`, then `p=0` implies `q=0`; and
- if `p | q`, then `q!=0` implies `p!=0`.

The standalone baseline OR chain therefore propagates only ordinary facts.  Its
false result traverses every factor as nonzero; a true result traverses one
concrete factor-zero edge after the preceding factor-nonzero edges, with later
factors left as don't-cares.  Global composition may reduce a factor decision
only when both branches already have the same complete terminal-valued suffix.
Coordinate recognition and divisibility witnesses are emitted only for
concrete factor decisions actually retained on the path.

If the optional derived-compound experiment is enabled, its false branch means
every factor is nonzero and its true branch is the positive clause that at
least one factor is zero.  Known factor facts must simplify that clause; empty
clauses contradict and singleton clauses force their last factor zero.  A
compound true branch alone may not manufacture a coordinate assignment.  This
logic is an experiment-only obligation, not baseline V5 state.  General ideal
or integer-lattice contradiction pruning remains outside this milestone.

Constraint propagation is an exact restriction optimization.  It is not used
as an unrecorded equivalence rule for MTBDD node interning.

Restriction is a separate bounded pass over the propositional decision
function.  Its memo key is at least
`(decision_ref, canonical_constraint_state_identity)`: a shared node reached
under two different prefix constraints cannot reuse a result keyed only by the
node.  Every replacement by `ProvedEmptyLocus` retains an independently
replayable coordinate or exact-divisibility witness.  Constraint-state
construction, interning, comparisons, memo entries, implication checks, and
proof references have explicit cumulative limits and exact/one-below tests.
The differential corpus includes orthant violations, conflicting fixed values,
equality/nonzero contradictions, and both valid divisibility implications.

## 8. Exact source identity and replay

Coverage V5 adds a bounded, exact structural source identity.  It is a
length/count-delimited encoding, never `Debug` output and never a
probabilistic digest.  It includes at least:

- V5 schema and atom-order schema;
- family, `K(n)` context, and sector identities;
- every retained V5 construction, replay, work, and representation limit;
- the canonical structural polynomial table;
- the complete normalized candidate-formula IR: clause order, every literal's
  locus and polarity, canonical atomic factor lists, source provenance, and
  backend routing decisions;
- every canonical candidate-formula factor list and V4-derived decomposition
  witness used by a differential artifact;
- the atom table;
- complete terminal payloads and proof reasons;
- every node tuple and ordinal;
- the final root reference and the two distinct Boolean endpoint references;
- the identities of the retained generated-source/candidate attempts from
  which the function is rebuilt.

Process-local manager brands and collection allocation capacities are
diagnostic state, not certificate state, and are excluded from the source
identity.  Persisted logical/work statistics are encoded explicitly; a
`CoverageDecisionDagCapacityStats` value is never hashed or compared as replay
identity because a failed transactional attempt may safely retain capacity.

The source identity is a compact downstream binding, not a proof by itself.
Replay must:

1. validate schema, family, context, sector, and coherent limits;
2. authenticate every concrete polynomial;
3. recensus structural loci, formula factor lists, factor references, atoms,
   nodes, terminals, edges, proof reasons, and identity bytes;
4. validate factor canonicality and independently rebuild every baseline
   factor OR;
5. validate every reference range, topological edge, strict atom-order edge,
   reduction rule, unique-table property, final root, distinct correctly typed
   Boolean endpoints, and complete rooted reachability;
6. regenerate and replay the underlying IBP/LI candidate attempts;
7. independently rebuild the complete rooted MTBDD with the persisted order
   policy, reject unreachable arena entries, and, for an experimental compound
   payload, prove pre-compression-assignment equivalence to its expanded
   baseline OR; and
8. compare the complete same-schema payload, persisted logical/work
   statistics, and source identity exactly.

Replay may share immutable Symbolica polynomial allocations by Arc.  It may
not trust stored nodes merely because their source-identity text matches.

## 9. Resource model

V5 must not repurpose the meaning of V4 split/leaf limits.  It adds explicit
checked limits and exact statistics for at least:

- retained candidate-formula factor lists and aggregate factor references;
- optional derived-compound factor sets and references, under separate counters;
- coverage atoms;
- retained MTBDD nodes, terminals, and edges;
- terminal payload references and empty-reason references;
- unique-table lookups/comparisons;
- `apply`/`ITE` calls and memo-table entries;
- formula-to-MTBDD work;
- canonical constraint states and, for the optional experiment, positive
  factor clauses;
- coordinate/divisibility propagation steps;
- concrete classification node visits and factor evaluations;
- source-identity bytes;
- residual traversal node visits, stack depth, yielded paths, and retained
  path decisions; and
- queue/Boolean-cover factor expansions.

All count arithmetic is checked.  Large tables are reserved only after their
individual and aggregate limits pass.  Allocation failures are typed where
Rust's stable collection API permits fallible reservation.

This preflight rule includes rooted export: terminal, node, edge, atom-ordinal,
and remap requirements are charged before the corresponding reserve, clone,
push, or insertion.  The pending generic-core freeze is not accepted against
this requirement until the export path and its exact/one-below regressions
verify that ordering.

Core budgets are cumulative over the complete public compilation/replay
operation.  In particular, ITE/apply calls, memo insertions, Boolean validation
visits, work-stack pushes, operation steps, unique-table lookups/comparisons,
and priority-candidate work accumulate across every internal ITE and every
candidate.  Starting a fresh local memo table may bound peak memory, but it
must not reset the cumulative counter.  Retained unique-table and graph
censuses are likewise aggregate.  Separate explicit limits may bound peak
simultaneous cache entries and cumulative cache insertions; one field may not
ambiguously serve both meanings.

Production V5 construction places Boolean endpoint and disposition-terminal
creation, every normalized formula compilation, priority composition, and
rooted export inside one `CoverageDecisionDag::with_operation` scope.  The
single-operation convenience methods are not sequenced as the production
compiler, because doing so would reset operation-local limits between phases.
Replay regeneration likewise uses one complete operation scope.

On typed failure the retained logical graph, interning membership, retained
census, committed work statistics, and any previously emitted rooted bytes are
unchanged; a deterministic retry produces the same IDs and rooted view.
Allocator capacities may grow and are reported only as diagnostics.  They are
neither required to roll back nor included in certificate equality.

The outer V5 error type preserves the generic core's typed resource name,
requested count, and limit, and adds the exact compilation/replay stage.  It
must not collapse a core, Symbolica, authentication, or allocation failure into
an unstructured string.

Useful V5 statistics distinguish:

- distinct nodes and terminals;
- reduction hits (`low == high`);
- unique-table reuse;
- `apply` memo hits;
- ordinary factor-OR nodes and factor references;
- optional derived-compound nodes and factor references, reported separately;
- proved-impossible branches;
- residual paths actually traversed; and
- concrete factor evaluations avoided by caching.

The number of root-to-terminal paths is deliberately not called a retained
leaf count.  It may be exponentially larger than the retained MTBDD.  A
bounded dynamic-programming path census may be requested for diagnostics, but
V5 construction must not flatten those paths.

## 10. Concrete classification

The current concrete lookup scans every explicit case and every predicate.
V5 instead traverses from the MTBDD root:

1. evaluate a concrete polynomial-zero atom by checked specialization;
2. cache every structural-locus zero/nonzero result for the duration of the
   query; and
3. follow exactly one edge until one terminal is reached.

An optional derived-compound node may short-circuit over its factors, but its
result is derived from the same cache and its experimental stats are separate.

This returns the same first-applicable candidate semantics without enumerating
unrelated regions.

### 10.1 Provider application remains independently checked

Global coverage is a routing certificate, not permission to skip the selected
candidate's local proof.  `ParametricSectorRuleProvider` currently re-evaluates
both the global coverage disposition and the selected candidate-local
`WhenBad` certificate at the concrete query point before applying the retained
parametric rule.  V5 must preserve that sequence.  A global
`DescendingRule { candidate_ordinal }` identifies which authenticated attempt
to try; the provider then checks its local `WhenBad` classification and invokes
the rule's ordinary concrete applicability check.  An inapplicable result is
delegated or continued according to the existing provider policy, never
upgraded to a reduction because the MTBDD selected it.

The same rule applies to `GeneratedSectorConditionalRuleProvider`: V5 may
replace its global residual-region locator, but each installed conditional rule
is still applied through the existing local applicability API and an
`Inapplicable` result continues to the next rule.  Local
`SymbolicSectorCasePartitionCertificate` and `SymbolicSectorCaseId` values
inside `WhenBad` proofs remain V1-local objects; they are not replaced by a V5
global region ID.

## 11. Lazy residual conjunctions

Downstream conditional derivation sometimes needs one exact residual
conjunction, not only a concrete lookup. V5 now provides a bounded
deterministic DFS cursor over root-to-terminal paths in
[`src/parametric_sector_residual_path.rs`](../../src/parametric_sector_residual_path.rs).

```rust
pub struct ParametricSectorResidualPathDecision {
    node_ordinal: usize,
    atom_ordinal: usize,
    structural_locus_ordinal: usize,
    polarity: NonZeroOrEqualZero,
}

pub struct ParametricSectorResidualPathCertificate {
    source: Arc<ParametricSectorMtbddCoverageCertificate>,
    request: AnyResidualOrOneKind,
    decisions: Box<[ParametricSectorResidualPathDecision]>,
    terminal_ordinal: usize,
    limits_and_stats: BoundedTraversalEvidence,
}
```

The current V1 identity is deliberately process-local: exact `Arc` allocation
identity binds the path to its source, and replay reauthenticates the complete
source before reproducing the requested yield ordinal. A later durable owner
still needs count-delimited source identity bytes.

The cursor stores only O(depth) frames and the current decision stack. It
does not mark a shared node globally visited: reaching the same node through
two different prefixes describes two different disjoint regions.  Branch
order is fixed by the order schema.  Skipped atom levels are don't-care
variables and do not appear in the conjunction.

A path certificate replays by walking its edge list from the root and checking
the terminal.  Two yielded paths are disjoint at their first different branch,
and the complete DFS covers the MTBDD root.  Enumeration remains explicitly
bounded; an MTBDD can still have exponentially many paths.  Resource
exhaustion yields a typed incomplete traversal rather than silently dropping
residual regions.

## 12. Live-leaf queue V3

The present queue first collects every residual case, then invokes an
extractor which replays and clones the complete source partition for each
case.  V3 removes that flattening boundary.

- `GeneratedSectorLiveLeafQueueCertificateV3` retains one shared Arc to the V5
  discovery/coverage certificate.
- A work item is bound by a residual-path certificate, not by
  `SymbolicSectorCaseId`.
- The queue consumes the final-root DFS cursor incrementally rather than first
  constructing `Vec<all sources>`.  It yields work items only for `Uncovered`
  and `Unsupported` terminals.  `DescendingRule`, `ProvedEmptyLocus`, and both
  construction Boolean endpoints never become work items; an unsupported work
  item retains the exact ordered candidate list from its terminal.
- Coverage replay occurs once before the queue batch.
- Per-path extraction validates the path and retains only its concrete factor
  facts, coordinate witnesses, unresolved non-product formulas, and source
  identity.  Positive factor clauses occur only in the optional compound
  experiment.
- Work-item locators use stable `(path_position, factor_position)`-style
  references rather than ordinals in a flattened predicate slice.

For baseline path extraction, concrete equality/nonzero atoms go through
existing coordinate recognition.  A product OR's true paths contain a
specific factor-zero decision; its false path contains every factor-nonzero
decision.  In the optional compound experiment, true remains one unresolved
positive factor clause unless propagation forces a factor, false supplies all
factor-nonzero facts, and true alone is never a coordinate assignment.

The current V2 queue remains the replay path for Coverage V4 artifacts.

## 13. Product Boolean cover V2

The product Boolean cover is the correct layer at which to expand one selected
residual product condition.  V2 binds to the residual path and shared V5
source identity instead of comparing a cloned full partition.

Its root CNF receives baseline concrete `p=0` decisions as singleton positive
clauses and concrete `p!=0` decisions as nonzero facts.  This is already enough
for ordinary factor-OR paths.  Only the optional compound experiment adds the
mapping `DerivedAnyFactorZero(F)=true` to one positive `F` clause and false to
all factor-nonzero facts.

The existing clause canonicalization, subsumption, propagation, coordinate
contradiction checks, and bounded DPLL construction can then be reused.  V2
adds exact counters for baseline factor references/lookups and, separately,
optional derived-compound predicates and expansions.

The lazy path is intentional.  It prevents global root-to-terminal path
flattening while preserving the exact base-factor decisions needed by the
particular residual affine/tensor computation.  Optional compound expansion,
if retained after A/B measurement, is delayed to this layer.

## 14. Schema and migration plan

The representation change is not compatible with silently reusing the V4
schema.

- add `PARAMETRIC_SECTOR_COVERAGE_V5_SCHEMA` for the baseline ordinary-factor
  MTBDD;
- add a versioned normalized candidate-bad-formula schema shared by the V4
  oracle and V5 compiler, without changing persisted V4 artifacts in place;
- give any derived-compound A/B experiment a distinct coverage/atom-order
  schema rather than interpreting it as baseline V5;
- retain the V4 explicit-partition compiler and replay implementation as a
  selectable oracle backend;
- add `GENERATED_SECTOR_DISCOVERY_V6_SCHEMA` with a versioned coverage payload;
- add queue schema V3 for MTBDD residual paths;
- add product Boolean-cover schema V2 for path/factor-formula sources; and
- version every owning or locator-bearing certificate listed below rather than
  changing the meaning of an existing field in place.

`GeneratedSectorDiscoveryCertificate` currently stores one concrete
`ParametricSectorCoverageCertificate`, and discovery V5 already denotes the
authenticated-accepted-composition replay strategy.  Coverage V5 therefore
requires discovery **V6**, not reuse of discovery V5.  Its payload should be an
explicit enum along these lines:

```rust
pub enum GeneratedSectorDiscoveryCoveragePayloadV6 {
    ExplicitPartitionV4(ParametricSectorCoverageCertificate),
    ReducedMtbddV5(ParametricSectorCoverageMtbddCertificate),
}
```

V6 stats must be representation-aware: V4 leaf/split counts remain V4 fields;
V5 node/terminal/edge and traversed-residual-path counts are not mislabeled as
leaves.  Replay dispatches by the enum tag, then compares the complete tagged
payload.  Older discovery V1--V5 artifacts retain their present replay paths.

V4-to-V5 migration is deterministic recompilation from the retained
family/context/generated attempts.  It is not a structural cast from the old
leaf list.  The backend-neutral pre-compression formula IR is regenerated once.
V4's materialized/factored routing is authenticated only for the explicit V4
artifact; V5 applies its own fixed ordinary-factor OR policy under V5 limits and
atom order.

The main compiler should emit V5 only after differential and replay tests pass.
No compatibility accessor may implement `partition()` or `cases()` for V5 by
eagerly flattening the MTBDD.  Callers must migrate to concrete traversal or
lazy residual paths.

### 14.1 Explicit V4 backend and oracle fixture

During development, coverage compilation must expose an internal test-only or
explicitly configured backend choice, for example
`ExplicitPartitionV4 | ReducedMtbddV5`.  The V4 oracle and V5 subject receive
the same authenticated `GeneratedWhenBadCompilation` attempts and the same
single normalized pre-compression formula IR in persisted order.  The fixture
retains attempt identities, V4 terminal dispositions, product-decomposition
witnesses, and the pre-compression/concrete-point query set.  Its V4 Boolean
adapter derives every materialized product-locus bit as the OR of its witnessed
factor bits and never accepts that bit as an independent input.  It compares
the schema-neutral semantic disposition defined in section 4; same-schema
payload/reason bytes remain exact replay data.  It does not compare V4 case
ordinals with V5 path ordinals and does not substitute masters.

The focused boundary evidence is associated with this licensed, parallel,
ignored-test invocation:

```bash
cd /shared/localunitaritythree/LiteRed
SYMBOLICA_LICENSE='your-license' \
SYMBOLICA_HIDE_BANNER=1 \
cargo nextest run -j4 --run-ignored ignored-only \
  --success-output immediate \
  -E 'test(sunset_011_depth_two_coverage_uses_factored_product_without_native_expansion)'
```

The frozen generic-core snapshot
`86fef90ed96d5c57de8775411150db59eab0d682d9acf9889d3618650a9e3025`
passed 23/23 standalone tests with four threads after formatting:

```bash
cd /shared/localunitaritythree/LiteRed
/nix/store/9pb4ikjdw4gp766ayxl6gg3b7hqm6ds4-rust-stable-with-components-2026-07-09/bin/rustc \
  --edition=2024 --test \
  -C linker=/nix/store/hb2bs5fg5wkm04x565737qd5nh2hy5nk-gcc-wrapper-15.2.0/bin/cc \
  src/coverage_decision_dag.rs \
  -o /tmp/rustred_coverage_decision_dag_tests
/tmp/rustred_coverage_decision_dag_tests --test-threads=4
```

The same frozen hash passed the licensed crate-focused run with 23 tests passed
and 430 skipped (nextest run ID
`ea230d23-cd9d-422c-86c4-04c90508ac98`):

```bash
cd /shared/localunitaritythree/LiteRed
env \
  SYMBOLICA_LICENSE='your-license' \
  SYMBOLICA_HIDE_BANNER=1 \
  CARGO_INCREMENTAL=0 \
  cargo nextest run -j4 --lib -E 'test(coverage_decision_dag)'
```

This final evidence was captured at `2026-08-20T09:31:24Z` on
`x86_64-unknown-linux-gnu`, NixOS 26.11, Linux 6.18.37, with rustc/cargo 1.97.0,
cargo-nextest 0.9.140, and an Intel Xeon W-2135.  `cargo tree -e features -i
symbolica` reported Symbolica 2.2.0 with `gmp` and
`tracing_max_level_info`; no `no_gmp` feature is present.  These are focused
generic-core and crate-compilation results, not Coverage V5 integration.

Every subsequent failure, differential result, and performance measurement
must capture the literal command, relevant environment variables, working
directory, UTC timestamp, Rust/nextest versions, platform/CPU, exit status,
and hashes before drawing a conclusion. The 2026-08-20 planning snapshot was:

```text
tests/sunset_depth_two_product_support.rs  221266e702038c1e08f2d7fbc5f808daabb2d48547912f9def9a01e42ed819e0
src/parametric_sector_coverage.rs         6adf95d98b58564248dfc7b15a0af3b2aaf49347c98ba54ea1966ca946c748b9
src/symbolic_sector_cases.rs              6b46a3d4894ea975f59f0bf4f07141a477e60f9f8782817dd26dfe1699682a1f
src/coverage_decision_dag.rs              86fef90ed96d5c57de8775411150db59eab0d682d9acf9889d3618650a9e3025
src/parametric_sector_mtbdd.rs            NOT_YET_CREATED
src/lib.rs                                7251ec35f8daa2690eb8cf40cbbe6250e4799a359dfb49eacd8e286de906c629
Cargo.toml                                c9099f5e4074e20a2dbec0aa252ca9a44849bdb3bd2cdc42b3dbb2aac3073bea
Cargo.lock                                0e93aa13657c89c6dbd4b084ea47e847007722271f3af267e73f3904039b3e36
flake.nix                                 6d976fb2037f31b73f9c9cd74c64aa188ee7480283a53a7b7f1456b4071848d3
flake.lock                                01d38f66c85385251cedc49d5fbebf52da6d46b937038b771619291c5177caa4
```

Those hashes describe that historical snapshot; a future rerun must recapture
them.  The original focused probe did not retain a complete UTC/tool-version/
platform manifest, so the boundary is valid failure evidence but not a
performance baseline. At that snapshot the complete depth-growth stress had
not been rerun, and no V5 comparison or performance number had been measured.

The manifest must also retain `cargo tree -e features -i symbolica` (or an
equivalent `cargo metadata` feature projection).  `Cargo.lock` alone does not
prove selected features. At that snapshot `Cargo.toml` disabled Symbolica
default features and selected `gmp` plus `tracing_max_level_info`; no RustRed
test or production build may select `no_gmp`.  When the custom Nix flake is
used, both flake hashes and the resolved `rustc`, `cargo`, `cargo-nextest`, C
compiler, and `m4` paths/versions are part of the invocation record.

## 15. File-level integration points

### 15.1 Owning certificate and provider versions

The migration is larger than the coverage and queue files because certificates
embed discovery/queue material by value and providers preflight V4 leaf
censuses.  The complete owning-layer inventory is:

- MTBDD types, builder, evaluator, and replay validator in
  `src/parametric_sector_mtbdd.rs`, the implemented cursor/path certificates
  in `src/parametric_sector_residual_path.rs`, repaired/wired core support from
  `src/coverage_decision_dag.rs`, and crate-private wiring in `src/lib.rs`;
- the versioned backend-neutral candidate bad-formula IR and one-time
  authenticated normalization seam, replacing the current private,
  backend-entangled `CandidateBadFormula` construction without weakening V4;
- V5 schema, limits, certificate representation, candidate composition,
  concrete lookup, and product routing in
  [`src/parametric_sector_coverage.rs`](../../src/parametric_sector_coverage.rs);
- discovery V6's tagged coverage payload, replay, and representation-aware
  stats in `src/generated_sector_discovery.rs`;
- queue V3 source/work-item/cursor integration in
  [`src/generated_sector_live_leaf_queue.rs`](../../src/generated_sector_live_leaf_queue.rs);
- `GeneratedFamilyRuleSystemCertificate` V2 and its sector outcome payloads in
  `src/generated_family_rule_system.rs`;
- `GeneratedFamilyFixedPointCertificate` V2, residual work/history payloads,
  replay/equality logic, and `GeneratedFamilyFixedPointProvider` V2 in
  `src/generated_family_fixed_point.rs` and
  `src/generated_family_fixed_point_provider.rs`;
- `GeneratedFamilyDepthGrowthCertificate` V2, attempt/final-status material,
  residual summaries, replay/equality logic, and
  `GeneratedFamilyDepthGrowthProvider` V2 in
  `src/generated_family_depth_growth.rs`;
- coverage/queue payload enums and cumulative preflights in
  `src/generated_provider_stack.rs`, `ParametricSectorRuleProvider` V2 in
  `src/parametric_sector_provider.rs`, and
  `GeneratedSectorConditionalRuleProvider` V2 in
  `src/generated_sector_conditional_provider.rs`;
- `GeneratedFamilyRuleSystemProvider` V3's representation-aware preflight and
  replay in `src/generated_family_rule_provider.rs` (the existing provider
  schema is already V2);
- a path-based coordinate extraction proof beside, rather than silently
  changing, [`src/coordinate_equality_loci.rs`](../../src/coordinate_equality_loci.rs);
- Boolean-cover V2 source binding and root CNF construction in
  [`src/product_locus_boolean_cover.rs`](../../src/product_locus_boolean_cover.rs);
- dedicated V5 integration tests plus the existing family depth-growth
  regression.

Each V2 name above is a design requirement, not an existing constant.  If an
implementation uses a tagged payload under a differently named new schema, it
must still preserve old replay and may not silently reinterpret V1 fields.

### 15.2 Global region-ID migration inventory

`SymbolicSectorCaseId` is an ordinal in an explicit partition and cannot name a
V5 residual region.  Introduce a versioned global region locator whose V4 arm
binds `(coverage_identity, case_id)` and whose V5 arm binds
`(coverage_identity, residual_path_certificate)`.  A bare terminal ordinal or
DFS yield position is insufficient because shared nodes can be reached under
different prefixes.

Every global `source_case`, classification case, or predicate-position field
must be audited and versioned in these consumers:

- `src/parametric_sector_coverage.rs`;
- `src/generated_sector_live_leaf_queue.rs`;
- `src/coordinate_equality_loci.rs`;
- `src/generated_residual_affine_case_inventory.rs`;
- `src/generated_sector_conditional_provider.rs`;
- `src/generated_family_fixed_point.rs`;
- `src/affine_parametric_ordering.rs`;
- `src/generated_cylindrical_residual_start.rs`;
- `src/generated_residual_affine_branch_bound_relation.rs`;
- `src/generated_residual_affine_pivot_target_matching.rs`;
- `src/product_locus_boolean_cover.rs`;
- `src/residual_affine_branch_guard_composition.rs`;
- `src/residual_unit_affine_index_map.rs`;
- `src/affine_locus_bound_relation.rs`;
- `src/generated_residual_affine_when_bad_compilation.rs`;
- `src/generated_residual_affine_when_bad_descent.rs`;
- the V4-global `GuardOrigin` variants retained by `src/guards.rs`; and
- the corresponding origin construction, transport, and replay in
  `src/parametric_coefficient.rs`, including a V2 successor for
  `RESIDUAL_UNIT_AFFINE_COMPOSITION_V1_SCHEMA`.

Their locator-bearing V1 certificate schemas require V2 successors where
applicable: coordinate equality, residual case inventory, affine ordering,
cylindrical residual start, branch-bound relation, pivot-target matching,
residual product Boolean cover, branch-guard composition, unit affine index
map, affine-locus-bound relation, and generated residual affine `WhenBad`.
Certificates which retain those objects transitively must also be versioned and
replayed through the tagged locator: `src/residual_affine_branch_system.rs`,
`src/generated_residual_affine_branch_reelimination.rs`,
`src/affine_prepare_points.rs`, `src/affine_prepare_point_schedule.rs`, and
`src/generated_cylindrical_row_system.rs`.  This transitive list is part of the
migration; changing only queue `source_case` fields is incomplete.

The manifest layer is part of that transitive migration, not a string-format
detail.  `PARAMETRIC_RELATION_MANIFEST_V2_SCHEMA` in
`src/parametric_relation.rs` serializes every `GuardOrigin`, so it requires a
V3 successor plus a legacy dispatcher.  `PARAMETRIC_SOURCE_MANIFEST_V1_SCHEMA`
in `src/parametric_elimination.rs` nests those relation manifests and requires
an explicit V2 successor/legacy decision; its consumers, including
`src/persistent_parametric_elimination.rs` and generated residual branch
re-elimination, must replay the tagged manifest rather than reinterpret V1
bytes.

Some of those modules also consume candidate-local partitions.  Those local
uses do not migrate.  `src/symbolic_sector_cases.rs`, `src/when_bad.rs`,
`src/generated_when_bad.rs`, genuinely candidate-local fields in
`src/parametric_coefficient.rs` and `src/guards.rs`, and each retained
candidate's local applicability proof continue to use
`SymbolicSectorCasePartitionCertificate` and `SymbolicSectorCaseId`.
However, `GuardOrigin` variants in `src/guards.rs` also retain V4-global
`source_case` ordinals emitted from residual maps in
`src/parametric_coefficient.rs`.  Those global-origin variants must gain a
versioned V4/V5 region locator, and every schema which retains them directly
or transitively must be versioned and replayed.  Dual-use modules must expose
distinct local-case and global-region types; they may not change either
meaning through an alias.

`src/symbolic_sector_cases.rs` remains the generic polynomial-only local
partition proof and legacy V4 building block.  V5 must not weaken its replay
or resource semantics.

## 16. Staged implementation

1. **Freeze attributable evidence and the V4 oracle.** Preserve the focused
   65,537 error with command/environment/hashes, create small explicit-V4
   oracle fixtures, and leave the legacy defaults unchanged.  Rerun the full
   stress separately; do not conflate it with the focused evidence.
2. **Completed: finish and freeze the generic core.** The lib-wired hash
   `86fef90e...e3025` passes 23/23 standalone and licensed parallel-nextest
   tests.  It includes direct iterative apply, dead-suffix truncation, rooted
   export/rebuild, manager brands, first-error poisoning, early export/replay
   preflights, and the core exact/one-below resource matrix.  Any later edit
   reopens this gate.
3. **In progress: normalize once, then build baseline ordinary factor OR.** A
   private authenticated backend-neutral formula IR, bounded canonical
   normalizer, product-free MTBDD compiler, and private owning/rebuilding
   stage-1 certificate now exist.  Focused tests cover source/factor
   canonicality, typed-root malformations, priority cutoffs,
   absorption/tautology cases, authenticated sunset replay, aggregate
   generated-source censuses, and exact/one-below normalization/MTBDD
   resources.  The remaining part of this stage is to encode and meter the
   exact structural source identity, make the explicit V4 oracle consume the
   same normalized IR, and map every nested frozen-DAG limit through the owner.
   No Symbolica product is multiplied or retained by the MTBDD route.
4. **Coverage V5 differential compiler.** Compile the same authenticated
   candidate attempts into explicit V4 and baseline V5 on small fixtures;
   compare every pre-compression assignment and retained concrete-point corpus.
5. **V5 replay and concrete/provider application.** Make independent rooted
   reconstruction and traversal pass, then preserve the candidate-local
   `WhenBad` and concrete rule-applicability rechecks before any reduction.
6. **Discovery V6 and owning-certificate migration.** Add the tagged payload,
   then version family rule-system, fixed-point, depth-growth, provider-stack,
   and provider ownership/replay without flattening V5.
7. **Completed traversal checkpoint.** The bounded residual cursor and
   process-local path certificates retain one shared source allocation and
   pass compact replay/resource tests plus the ignored all-36 `K=21` stress.
   This checkpoint does not avoid global MTBDD construction.
8. **Direct normalized-formula frontier and global region IDs.** Extract the
   normalized source owner, search one authenticated frontier without building
   a global MTBDD, then add Queue V3 and migrate every global `source_case`
   consumer from section 15 while preserving local case IDs.
9. **Boolean-cover V2.** Consume baseline concrete factor facts and retain exact
   DPLL proof data.  Add optional positive-clause handling only if the compound
   experiment is built.
10. **Measure before any default switch.** On fixed hashes and commands, record
   V4/V5 terminal equivalence, V5 nodes/terminals/edges, cumulative memo and
   validation work, residual paths traversed, peak memory, and wall time for
   small fixtures, the focused sector-011 test, and the full depth-growth
   stress.  A separately versioned compound representation may be A/B-tested
   here against the ordinary-factor baseline.
11. **Switch default discovery only after acceptance.** Emit baseline V5 only
    after differential, local-applicability, replay/tamper, cumulative-resource,
    provider/fixed-point/depth-growth, focused sunset, and full stress suites
    are green with captured evidence.  Keep V4 replay and the explicit V4
    oracle backend.

## 17. Acceptance tests

### 17.1 Boolean and priority equivalence

- Exhaustive pre-compression formula assignments agree between explicit V4 and
  baseline V5 through the schema-neutral semantic disposition from section 4,
  rather than raw proof bytes or representation-specific region IDs.  Every V4
  materialized-product predicate is derived from its witnessed factor OR; an
  independently assigned product bit is rejected by the test harness.
- Sampled and boundary integer lattice points agree using checked Symbolica
  specialization; specialization failures agree as typed failures.
- Reversing two overlapping candidates changes the selected ordinal exactly
  as ordered `noRules` semantics requires and does not change their union.
- Empty search remains `Uncovered`; unsupported search remains explicitly
  `Unsupported`.
- Direct apply is checked for all 16 binary truth tables over independent,
  equal, correlated, and shared-subgraph inputs with multi-terminal outputs.
- Exhaustive literal tests compile `EqualZero` as `x`, `NonZero` as `NOT x`,
  and the production leak clause `(boundary=0 && numerator_gate!=0)` as
  `x_boundary && NOT x_numerator_gate`; swapping either polarity must fail the
  V4 differential oracle and concrete-point corpus.
- Empty, atomic, conjunction, and mixed-OR candidate formulas agree with V4;
  factored-product OR tests supplement rather than replace the numerator-gate
  conjunction tests.
- Materialized-product and factored V4 routes are both exercised against the
  same normalized IR.  At concrete points, direct specialization of a
  materialized product agrees with the OR derived from its authenticated
  factors.
- Orthant violations, conflicting fixed values, equality/nonzero conflicts,
  and both exact-divisibility implications agree in terminal category, while
  each V4/V5 empty witness replays independently under its own locator schema.
- `left=false` with the `left && !right` truth table returns the existing false
  terminal without constructing `!right`, and an earlier constant-false bad
  formula prevents construction of every lower-priority continuation.
- Deep ITE/apply and priority chains prove iterative stack safety, not merely
  iterative evaluation.

### 17.2 MTBDD canonicality and replay

- Equal inputs produce byte-identical source identities and payloads.
- `low == high` reductions and unique-table reuse are exercised.
- Node, terminal, atom, factor, root, edge-order, cycle, reachability, stats,
  limit, and identity tampering all fail typed replay.
- Clause order, literal polarity, normalized-factor provenance, backend route,
  certified/unsupported attempt kind, descending ordinal, and unsupported-list
  tampering all fail typed replay.
- Foreign-manager roots and Boolean terminal pairs cannot silently alias a
  same-ordinal object in another manager.
- The final coverage root reaches only disposition terminals.  Making either
  Boolean endpoint reachable from it fails construction/replay even though the
  same endpoints remain required auxiliary roots in the persisted view.
- A successful rooted export contains no unreachable node; replay rejects an
  appended canonical-but-unreachable node.
- Failure after at least one successful append rolls back terminal/node vectors
  and both interning tables; retry produces the same IDs and rooted bytes.  A
  test that fails before the first append is not sufficient rollback evidence.
- A `with_operation` closure which catches that append-then-fail method error
  and returns `Ok` still returns the first typed error, rolls back, and retries
  to the same IDs.  Allocator capacity is separately audited and need not
  shrink.

### 17.3 Exact and one-below resource matrix

For every resource, construct a fixture whose exact audited requirement is
`N`.  Limit `N` must succeed with exact stats; limit `N-1` must return the
expected resource name with `requested=N`, leave logical graph contents,
interning membership, retained census, committed work stats, and any persisted
certificate bytes identical to the checkpoint, and succeed deterministically
when retried at `N`.  Diagnostic allocator capacity may increase and is neither
certificate identity nor rollback state.  Every logical limit is checked before
the corresponding allocation or retained insertion.  The focused generic-core
suite is not full acceptance until this entire core-and-outer matrix is green.
The matrix covers at least:

- terminals, terminal-index entries, nodes, unique-table entries, and retained
  child edges;
- rooted reachable-export nodes/edges and atom ordinals;
- current work-stack entries and peak simultaneous memo entries;
- cumulative stack pushes, operation steps, ITE/apply calls, ITE memo
  insertions, memo hits, Boolean validation visits, unique lookups/comparisons,
  and priority candidates across the complete public operation;
- structural loci, candidate-formula factor lists, aggregate factor
  references, ordinary factor-OR construction work, terminal payload
  references, proof-reason references, and source-identity bytes;
- replay regeneration work, candidate attempts, and formula-to-MTBDD work;
- canonical constraint states, `(node, constraint-state)` restriction memo
  entries/comparisons, coordinate/divisibility implications, and empty-proof
  references;
- concrete classification node visits, distinct factor evaluations, and cache
  entries; and
- residual traversal visits/depth/yields/decisions, queue retention, and
  Boolean-cover factor/clause/literal/DPLL/state-byte work.

Overflow cases for every aggregate multiplication/addition fail before
allocation.  Nested ITEs and successive priority candidates specifically prove
that cumulative counters do not reset merely because an internal cache is
recreated.  Outer coverage/provider error conversion preserves the exact core
resource name, requested count, configured limit, and operation stage.

### 17.4 Baseline factor OR and optional compound experiment

- Units, duplicates, empty lists, singletons, malformed ordinals, and zero
  factors have their specified typed behavior before OR construction.
- A canonical factor list contains exactly the normalized atomic `EqualZero`
  clauses selected before backend compression.  A `NonZero` literal, a literal
  taken from a conjunction, missing provenance, or a V4-only product ordinal is
  rejected as a factor-source substitution.
- The baseline OR agrees with “any factor is zero” on every base-factor
  assignment and concrete point, retains no expanded product, and performs no
  Symbolica multiplication.
- The explicit V4 materialized-product route agrees with that same OR by
  deriving its product-locus truth from the retained decomposition witness;
  direct product specialization is checked separately at concrete points.
- The standalone formula's false OR path records every factor nonzero; each
  true path records one concrete zero factor after its preceding nonzero
  factors, without inventing decisions for skipped suffix factors.  Any factor
  omitted after terminal-valued composition is justified by equal complete
  child suffixes.
- If a derived-compound representation is implemented, it uses a distinct
  schema and is A/B-equivalent to expanded baseline V5 over all base-factor
  assignments and concrete points.  Inconsistent independently assigned
  compound bits are rejected as outside the semantic domain, not counted as
  counterexamples or accepted as coverage.
- Optional compound/factor contradictions are proved empty when supported or
  retained conservatively; they are never incorrectly covered.

### 17.5 Lazy paths, queue V3, and region migration

- DFS paths are pairwise disjoint and cover the root on finite fixtures.
- Shared nodes are revisited under different prefixes correctly.
- Extracting one residual conjunction uses O(depth) traversal state and does
  not allocate all root-to-terminal paths.
- Traversal/path/factor limits fail at exact boundaries without exposing a
  partial work item.
- Queue replay authenticates the shared coverage once and every retained path
  independently.
- Queue V3 emits exactly the `Uncovered` and `Unsupported` final-root paths,
  preserves each unsupported ordinal list, and emits no work item for a
  descending, proved-empty, or Boolean terminal.
- V4 case IDs and V5 path locators cannot be substituted across schemas or
  source identities; every consumer in section 15 passes replay/tamper tests.
- Candidate-local `SymbolicSectorCaseId` proofs remain unchanged and replay
  independently of the migrated global region locator.
- Optional compound true paths do not invent coordinate assignments; compound
  false paths contribute exact factor-nonzero facts.

### 17.6 Product Boolean cover V2

- Baseline path factor-zero/nonzero decisions produce the exact singleton
  clauses and facts expected by direct evaluation.
- In the optional compound experiment, true creates exactly one `k`-literal
  positive clause and false creates exactly `k` factor-nonzero facts before
  canonical deduplication.
- Exact/one-below factor, clause, literal, DPLL, state-byte, and replay budgets
  remain transactional.
- Concrete Boolean terminals agree with direct factor evaluation.

### 17.7 Discovery, fixed-point, depth-growth, and providers

- Discovery V6 replays both tagged V4 and V5 payloads and rejects tag/payload,
  representation-stat, limit, and source-identity substitutions.
- Family rule-system V2, fixed-point V2, and depth-growth V2 reproduce V4
  behavior on oracle fixtures and process V5 residual paths without flattening.
- Provider-stack preflight uses representation-aware cumulative budgets.
- At every concrete query, a globally selected descending candidate must still
  pass its local `WhenBad` classification and concrete parametric-rule
  applicability check.  A deliberately inapplicable selected rule delegates or
  continues and never returns a reduction.
- Conditional providers likewise continue after local `Inapplicable`, with
  exact query/attempt stats under both V4 and V5 global payloads.

### 17.8 End-to-end milestone regression

- The focused ignored sector-011 test first reproduces the recorded V4 boundary
  on captured hashes, then baseline V5 completes without raising the legacy
  65,536 split default and reports its own cumulative limits/stats.
- The full depth-two connected sunset test is rerun separately and completes;
  its result is not inferred from the focused fixture.
- `J(2,1,1)` reduces completely to the same `J(1,1,1)` coefficient
  `(d-3)/(3*m2)` expected by the current test.
- The certificate proves candidates were freshly generated and selected
  parametrically; no topology-, sector-, or loop-count-specific recurrence,
  atom order, branch, or production limit is hardcoded.  Sector `011` and
  `J(2,1,1)` occur only as concrete validation/oracle fixtures.
- Every invocation records the section 14 command/environment/hash manifest.
  Tests use GMP-enabled Symbolica, never `no_gmp`, and run in parallel through
  `cargo nextest run -j4` except for an explicitly documented focused ignored
  selection that still retains `-j4`.
- The manifest includes the Symbolica feature projection and, when used, the
  custom Nix flake/toolchain hashes and resolved compiler, linker, `m4`, and
  nextest versions.

## 18. Non-goals and honesty boundary

Coverage V5 does not by itself complete LiteRed parity.  It does not add a
general Gröbner/saturation integer-lattice emptiness prover, masters,
symmetries, persistence serialization, tensor reduction, or multi-loop Vakint
validation.  It supplies a scalable exact representation for the already
generated candidate-domain composition and a faithful residual interface for
the next layers.

Those are non-goals only of this Coverage V5 submilestone.  They remain part of
RustRed's overall required LiteRed/Vakint-equivalent scope, including generic
generated parametric IBP reduction, persistence, tensor numerator reduction,
rule application, and the staged one- through five-loop validation program.

As of this revision, the generic decision-DAG core is lib-wired and frozen at
`86fef90e...e3025`; its 23-test focused suite passes standalone and under
licensed parallel nextest, and the independent resource/replay audit reports
no remaining P0/P1 core issue.  The private stage-1 integration now includes
the normalized formula IR, authenticated/bounded normalization from generated
attempts, a product-free rooted MTBDD compiler, and a private owning certificate
whose replay rebuilds the complete typed stage-1 payload.  Licensed GMP
Symbolica run `eae4bd40-50cb-4b33-8aab-c009e8f62361` passed 31/31 focused tests
with `nextest -j4`; this includes exact/one-below formula resources, all eight
aggregate generated-source limits at the owning boundary, bounded MTBDD atom
and typed-root work, exhaustive small Boolean assignments, and concrete sunset
V4 point comparisons.  After correcting two independent accounting-boundary
oracles, the complete licensed-GMP command
`cargo nextest run -j4 --no-fail-fast --lib` passed 484/484 tests in 91.465 s
on 2026-08-20.  That full-library checkpoint establishes compatibility with
the current RustRed library; it does not promote the private stage-1 payload to
production V5 acceptance.  The explicit-V4 same-IR adapter/differential,
deterministic exact source-identity bytes, complete owner-level nested-core
resource mapping, discovery V6, owning consumer versions, global region
migration, queue V3, and Boolean-cover V2 remain staged work.  The optional
compound representation is only a proposed A/B experiment.

Two distinct large-case facts are now measured. The legacy focused ignored
test requested split 65,537 under a 65,536 limit. The new ignored all-36
`L=6`, `K=21` source retains 49 atoms, 268,427 rooted nodes, and 18 terminals;
its cursor reaches the first Unsupported terminal in 43 decisions. Licensed
Nextest run `d1b3d6f2-70fe-4da2-ba36-9a671f48080a` passed the complete six-test
cursor slice; the stress took 132.985 seconds. The full depth-growth stress has
not been rerun, and no phase-separated construction/replay peak-memory
profile, provider result, arity-21 Ready result, or completed reduction has
been measured. Any future claim must cite an executed
command/environment/hash manifest and its observed statistics.
