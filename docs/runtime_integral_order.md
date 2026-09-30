# Runtime mathematical integral orders

Status (2026-09-30): native core/application, CLI, installed Python API and
cold-load/checkpoint tests pass on `fable_5_1_parallel`. Optimized matched pilots
qualify a source-visitation strategy with40–44% less four-loop domain work;
the tested non-default mathematical comparators have not beaten that strategy
overall. See [the measured controls](research/final_order_s5_pilots_2026-09-30.md).
Do not treat API flexibility alone as a campaign-switch recommendation.

Two independent controls select generated rules:

- `discovery_strategy` visits finite source rows and sectors in a chosen order.
  It does not alter which integral is simpler.
- `integral_order` defines the mathematical order used for preconditioning,
  modular column pivots, exact lifting, descent and source replay. The complete
  descriptor survives binary candidate output and generation checkpoints.

The order is fixed for each generated owner. Runtime steering can select new
weights and priorities without recompiling RustRed, but must regenerate the
affected owner rules. A walking flag cannot change the meaning of saved rules.

## Admissible descriptor

All comparisons are simpler-first. The comparator applies, in order:

1. Number of positive propagator powers (mandatory). A strict pinch is simpler.
2. Sum of `support_weights` for positive coordinates.
3. Support bits in `support_priority` order, with `false < true`.
4. `degree_rows`, compared lexicographically. An active index `n > 0` contributes
   `(n-1)*active[axis]`; an inactive index contributes `(-n)*inactive[axis]`.
5. Coordinate excess ties in `coordinate_priority` order, using the declared
   sign-group ordering and ascending/descending directions.

Both priority vectors list axes in comparison order, not ranks by slot.
Every vector has the family arity. Weights are unsigned integers. Every axis
of each sign must have a positive coefficient in at least one degree row, and
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
The optional total-excess-envelope certificate additionally requires a first
degree row that is a positive multiple of *unweighted* total excess. A lawful
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

## Persistence, routing and proof boundaries

Generated candidate structure is version2, generation checkpoints version4.
A narrow read-only reader for existing version1 candidate structures derives
their exact legacy permutation order; it cannot introduce programmed metadata
or silently migrate a campaign. Unknown/inconsistent versions are rejected
before native Symbolica payload import.

Different independently valid routed owners may use different orders: ordinary
same-support applications remain within one owner, while owner rerouting after
entry requires strict support-count decrease. Reusing an already proved rule in
a permuted coordinate frame is different: transport its full descriptor with
`CompiledOrder::transport` and prove comparison commutation. Equal names are
not an equivariance proof. Generation, metadata loading, modular evidence and
order-law tests alone never establish family closure.
