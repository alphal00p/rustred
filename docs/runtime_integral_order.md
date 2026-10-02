# Runtime mathematical integral orders

Status (2026-10-01): native core/application, CLI, installed Python API and
cold-load/checkpoint tests pass; the feature is merged into `main`. Optimized
matched pilots qualify a source-visitation strategy with40–44% less four-loop
domain work. A later R-primary mathematical order reduces that strategy's work
a further4.05%, with4.70–9.47% lower traversal time in two counterbalanced short
pairs, but increases generation, file size and source replay cost. See the
[original controls](research/final_order_s5_pilots_2026-09-30.md) and
[current rule-order study](research/rule_selection_portfolio_2026-10-01.md).
This is not a full five-loop benefit or campaign-switch recommendation.
The subsequent fourteen-sector five-loop compatibility control also passes
source replay and cold reinspection, but has essentially unchanged domain work
(1,647→1,648) and higher generation/replay cost. Its small starting query is
not the full production scope.
An additional real-routing R≤13 control regenerates three lower-owner payloads
under the alternate order in the full67-owner pool: domains fall1.29%, but
inspections and traversal are essentially unchanged. This also does not justify
a production switch.
Complete native diagnostic views subsequently find no changed formulas in
those three owners, only order metadata. The small work delta is therefore
not evidence of improved generated rules.

Two independent controls select generated rules:

- `discovery_strategy` visits finite source rows and sectors in a chosen order.
  It does not alter which integral is simpler.
- `integral_order` defines the mathematical order used for preconditioning,
  modular column pivots, exact lifting, descent and source replay. The complete
  descriptor survives binary candidate output and generation checkpoints.

The order is fixed for each generated owner. Runtime steering can select new
weights and priorities without recompiling RustRed, but must regenerate the
affected owner rules. A walking flag cannot change the meaning of saved rules.

An individual descriptor's well-foundedness is not, by itself, a proof of global
termination after mixing owner orders and routing between supports. The current
domain walker validates local descent and cold verification checks reachable
obligation coverage; neither supplies that stronger route-global theorem. See
the mixed-order caveat in the current rule-order study before interpreting a
successful mixed-owner pilot as a complete concrete reduction guarantee.

## Admissible descriptor

All comparisons are simpler-first. The comparator applies, in order:

1. Optional `pre_support_degree_rows`, compared lexicographically using physical
   powers: `n*active[axis]` when `n > 0`, `(-n)*inactive[axis]` otherwise.
   The default empty prefix preserves the historical support-first comparison.
2. Number of positive propagator powers.
3. Sum of `support_weights` for positive coordinates.
4. Support bits in `support_priority` order, with `false < true`.
5. `degree_rows`, compared lexicographically. An active index `n > 0` contributes
   `(n-1)*active[axis]`; an inactive index contributes `(-n)*inactive[axis]`.
6. Coordinate excess ties in `coordinate_priority` order, using the declared
   sign-group ordering and ascending/descending directions.

Both priority vectors list axes in comparison order, not ranks by slot.
Every vector has the family arity. Weights are unsigned integers. Every axis
of each sign must have a positive coefficient in at least one prefix or suffix row, and
no row may be identically zero. Native validation checks vector dimensions,
permutations, resource bounds and weighted accumulator bounds before search.

There are finitely many supports. Nonnegative weighted degree rows are
well-founded, and their joint level sets are finite when every axis is covered.
Thus reversed final coordinate ties do not introduce an infinite descent.
Symbolic/shift comparison cancels common coordinates only within a proved
support; applicability, guard poles and support-changing branches still require
their existing exact proof gates. This is not an arbitrary executable comparator.

Cuts and removed-cut configurations are currently rejected for this programmed
order. Legacy cut search remains a separate supported solver configuration.
The optional total-excess-envelope certificate additionally requires an empty
pre-support prefix and a first degree row that is a positive multiple of
*unweighted* total excess. Its induction assumes sector-first ordering; fixed-
support equivalence of absolute degree and excess does not establish that.
Likewise the specialized unconditional sector-monotone pinch witness rejects
pre-support orders. General source-port descent instead checks physical degrees
on exact sign-refined cells, failing closed if the comparison is unresolved. A lawful
weighted order can increase unweighted excess; it is not relabelled E-primary.

## Python and CLI

```python
from pathlib import Path
import rustred

# The default descriptor reproduces the uncut SpIRed priorities. The explicit
# program still has its own persisted descriptor identity.
order = rustred.integral_order(3)

# Fully runtime weighted alternative; native Rust validates admissibility.
weighted = rustred.integral_order(
    3,
    support_weights=[1, 2, 4],
    support_priority=[2, 0, 1],
    degree_rows=[{"active": [4, 2, 1], "inactive": [1, 2, 4]}],
    coordinate_priority=[1, 2, 0],
)
source = Path("family.toml").read_text()
candidates = rustred.family_candidates(source, integral_order=weighted,
    discovery_strategy=rustred.discovery_strategy(rows="coefficient-monomials"))
Path("order.json").write_text(weighted)
```

Use the same descriptor with one existing binary:

```sh
rustred family-candidates --input family.toml --output candidates.rrbin \
  --integral-order order.json
```

`--integral-order` and legacy `--permutation` are mutually exclusive. The Python
helper only builds data and performs basic shape checks; the Rust compiler is
the mathematical admission boundary. Manually supplied version-1 JSON permits
the same fields and rejects unknown fields. Existing source-discovery JSON is
separate and can be combined with a mathematical order.

The tested R-primary experiment is expressible using that same public helper:

```python
def rank_primary_order(arity):
    return rustred.integral_order(arity, degree_rows=[
        {"active": [0] * arity, "inactive": [1] * arity},
        {"active": [1] * arity, "inactive": [0] * arity},
    ])
```

It compares numerator rank before positive-index excess within a fixed support;
the earlier support priorities and default coordinate ties are unchanged. This
is a data-only research choice, not a new default or a bound on rank across
pinches/routing. Regenerate rules with it; do not relabel an existing artifact.
The helper reproduces the exact K10 and K15 study descriptors. Both pass their
registered generation/replay/walk gates: the combined four-loop control and a
limited fourteen-sector five-loop control, respectively. The latter supplies
compatibility evidence, not a five-loop performance improvement. The generic
CLI and `stage.py` accept this order; the generation-first selected-owner
`pipeline.py` still accepts only the legacy order. Do not bypass its reserved
arguments or substitute old payloads to work around that steering limitation.

An experimental, genuinely global absolute-degree order is now expressible as:

```python
global_degree = rustred.integral_order(arity, pre_support_degree_rows=[
    {"active": [1] * arity, "inactive": [1] * arity},
])
```

This compares `F=sum(abs(n_i))` **before** support, unlike the older post-support
excess `E=F-number_of_positive_indices`. A pinch from `(1,1)` to `(0,3)` is
no longer descending. Conversely a transition from `(-3,1)` to `(1,1)` can
descend despite activating a denominator. This is an opt-in research order,
not a demonstrated campaign speedup or a family-closure claim. Existing
empty-prefix programs retain their exact binary identity; nonempty prefixes
use the versioned v2 compiled-order encoding. JSON retains descriptor version1
with this optional explicit field. Source rules must be regenerated, not relabelled.
The source solver's existing activation-boundary restrictions and concrete
routed evaluator's literal-subsector restriction still apply: a mathematically
descending reactivation can remain unsupported. It must not be discarded as a
zero or counted as covered. The experimental comparator does not claim to have
removed these independent source/application limitations.
The separate certified `ClosingArtifact` installation path currently still
requires sector-first `RuleCell` witnesses and rejects these prefix programs,
even for a tadpole. Candidate generation, source-identity/guard replay and
the general local descent checks are separate from that certificate. Do not
interpret a successfully loaded candidate bundle as a certified closing artifact.
The first mixed-owner four-loop comparison retained all 58 queries and terminal
keys, reducing domains by 3.64% but not traversal time. See the
[measurement and literature note](research/global_degree_order_literature_2026-10-01.md).

## Rust and compilation boundaries

`rustred::order::{OrderDescriptor, CompiledOrder, DegreeRow, CoordinateGroups,
Direction, Limits}` is backed by the small Symbolica-independent `rustred-order`
crate. `CompiledOrder::compile` validates data once; `from_builder` evaluates a
Rust closure once and retains only its descriptor. Comparison is allocation-free
and borrows shared immutable data, rather than invoking callbacks or cloning CAS
expressions in elimination loops.

Set `SectorConfig::integral_order` for the solver, or
`FamilyCandidatesRequest::integral_order` for application generation. Use
`OrderingPolicy::try_programmed` at generic concrete/descent boundaries.
`SectorSolution::order` retains the actual source-search order, so replay must
not reconstruct it solely from a coordinate permutation.

Data changes require no Rust build. New order primitives require rebuilding the
small order crate and affected dependants. New Rust caller-side builder closures
compile the caller, not another monomorphization of the full elimination engine.
Core/application code changes still rebuild those crates; module separation is
not a claim of automatic crate-level reuse.

## Opt-in exact-rule portfolio

Implementation status (2026-10-01): the original portfolio passed independent
review, focused native tests, six installed Python/CLI/checkpoint tests and
1,320 application tests (with explicit external/scale exclusions and a 16-core
test allocation). These cover recipe binding and K3 worker-count equality with
source replay. The later total-positive-shift score passes 19 focused core,
nine application and two fresh installed Python/CLI tests. Baseline,
branch-first, rank-first and total-positive candidates pass full cold
reinspection and original-source/guard replay on the combined four-loop
control, but none demonstrates a useful improvement over A1. These
tested alternatives are not recommended for production; see
`docs/research/rule_selection_portfolio_2026-10-01.md` for measured results.
Build provenance and exact binary/test boundaries are recorded in
`CODEX_PROGRESS.md`; the original full application run predates the later
focused score tests. No subsequent untested experiment inherits that full-suite
pass merely by sharing the same checkout.

An optional `rule_selection` inside the discovery descriptor compares a baseline
rule against at most two alternate source-visitation plans. All trials use the
same mathematical integral order, preconditioned source basis, exact backend
and probe settings. Only a rule whose exact descent, guards and exceptional
geometry pass admission can compete. The winner's exceptional children enter
the existing queue once, in their original order. Equal scores retain the
baseline or earlier trial.

For example, this data-only Python recipe asks for alternatives only when the
baseline has exceptional children or at least eight RHS terms:

```python
selection = rustred.rule_portfolio(
    alternatives=["input-order", "coefficient-monomials"],
    quality=["exceptional-cases", "max-numerator-shift-excursion",
             "guard-predicates", "rhs-terms", "coefficient-monomials"],
    max_depth=1,
    max_rows=256,
    max_exact_trace_rows=64,
    max_exact_trace_terms=4096,
    trigger={"kind": "any-at-least", "thresholds": [
        {"feature": "exceptional-cases", "minimum": 1},
        {"feature": "rhs-terms", "minimum": 8},
    ]},
)
strategy = rustred.discovery_strategy(rows="terms", rule_selection=selection)
Path("discovery.json").write_text(strategy)
# Pass discovery_strategy=strategy to rustred.family_candidates, or use
# --discovery-strategy discovery.json with the existing family-candidates CLI.
```

These illustrative limits are not a measured optimal setting. The Rust API
exposes the same control as `SectorConfig::rule_selection` with
`RuleSelectionPolicy::BoundedPortfolio`. Alternatives may use existing generic
row features or validated materialized row-order plans. Neither interface
changes the mathematical integral comparator during a solve.

Quality keys also include total numerator-shift excursion, maximum and total
positive-shift excursion, affine exceptional cases, guard branches, source rows
and search rows. `total-positive-shift-excursion` sums active-axis positive
displacements over all RHS terms; `max-positive-shift-excursion` takes the
largest such per-term sum. Both ignore inactive axes. Symbolic coordinates use
target-relative offsets; fixed coordinates use differences of positive degrees,
not the fixed value as a displacement. For example, raising one active power in
four RHS terms has maximum one but total four. This is an opt-in preference,
not a demonstrated campaign improvement or a change to existing recipes.
Keys are lexicographic; ascending is the default, while an explicit
`descending` flag reverses a key. Shift-excursion scores are structural proxies,
not bounds on physical numerator rank, and exceptional-case counts are not
counts of disjoint geometric regions. No score provides closure authority.

The optional-trial caps limit search depth, generated rows and the exact source
trace before lifting. They do not interrupt an individual Symbolica algebra
operation or cap wall time. Budget exhaustion and recognized unsupported
optional geometry discard that alternative; malformed input, exact replay or
descent failures remain errors. A failed baseline is not rescued by this
portfolio. Fully fixed numerical cases keep their existing solver path.

Omitting the portfolio preserves the version-1 first-valid discovery descriptor.
Enabling it uses version2 and binds all alternatives, limits, quality priorities
and triggers into generation checkpoint identity. A changed recipe cannot
silently resume that generation. Candidate/owner binary rule encodings are
unchanged. The generation report's `rule_selection` section and distinct
trial-finished progress events account for attempted and rejected work; resumed
sectors contribute no invented historical timing. Worker-summed durations are
not whole-campaign wall time, and exact-materialization time is included in
search time rather than added twice.

## Experimental application policy for checked alternatives

Rule dispatch is separate from both discovery visitation and mathematical
integral order. `rustred::solver::RuleDispatchPolicy::Partition` remains the
default. The opt-in `AfterBaselinePartitionWholePiece` first lets ordinary rules
partition a requested domain, then tries the marked alternative on each whole
selected piece. It introduces no new split or refinement. If the alternative's
case, exclusions or original denominators cannot be proved applicable throughout
that piece, selection stays with the baseline. It cannot replace a gap, terminal,
zero or unresolved outcome, or override an earlier immutable batch. Actual
application still checks source conditions, every RHS obligation and descent.

The Rust application API
`encode_checked_priority_owner_with_policy::<N>(..., policy)` exports an
original-source-checked alternative while preserving the old rules and finite
terminals. The proof remains attached to the returned `CheckedPriorityOwnerExport`;
its candidate bytes alone are not a closing artifact. Normal loading, inspection,
concrete application and cold domain reinspection retain the dispatch policy.
Unsupported equation-only re-export and certification reject marked programs
rather than silently removing their semantics.

This experimental mechanism passes the focused native core and candidate-bundle
regressions. It has no demonstrated whole-campaign speedup yet. The
[current research control](research/global_degree_order_literature_2026-10-01.md#selective-application-preserve-the-baseline-partition)
keeps all original queries and measures complete traversal plus cold verification.

## Persistence, routing and proof boundaries

Ordinary generated candidate structure is version2, generation checkpoints
version4. Marked dispatch alternatives use candidate structure version3 with a
sparse rule-policy table; outputs without that table retain exact version2 bytes.
Changing a persisted policy changes the owner payload and invalidates reuse of
a checkpoint bound to the old payload.
A narrow read-only reader for existing version1 candidate structures derives
their exact legacy permutation order; it cannot introduce programmed metadata
or silently migrate a campaign. Unknown/inconsistent versions are rejected
before native Symbolica payload import.

For the concrete routed evaluator, different independently valid owners may use
different orders: ordinary same-support applications remain within one owner,
while owner rerouting after entry requires strict support-count decrease.
The domain walker also permits some support swaps/reactivations, so its
coverage checks alone do not establish that same global termination argument.
Reusing an already proved rule in
a permuted coordinate frame is different: transport its full descriptor with
`CompiledOrder::transport` and prove comparison commutation. Equal names are
not an equivariance proof. Generation, metadata loading, modular evidence and
order-law tests alone never establish family closure.
