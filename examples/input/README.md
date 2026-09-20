# External vacuum families

[`three_loop_k6.toml`](three_loop_k6.toml) supplies the complete three-loop
family to the generic CLI and Python examples, without authored rules or
search hints. It has six physical denominator coordinates and requests an
unrestricted sector census. The identifier-safe metadata name is shared with
the Rust-library example; migrating existing Vakint artifacts requires the
matching private loader binding as well as new bytes.

## Four-loop parents

These TOML files are ordinary user input to RustRed's topology-independent
parser and solver. Their descriptive names are not engine dispatch keys and
do not select precomputed identities. They record the four parent momentum
routings registered by Vakint's topology matcher: H, X, BMW, and FG.

Every coordinate has the form `Di = qi^2 - 1`. The common squared mass is
literally one, not a free parameter; only the dimension `d` is inferred as a
scalar parameter. For a reduction of target `a` onto master `b`, dimensional
homogeneity restores the coefficient factor
`(mass_squared)^(sum(b) - sum(a))`.

There are ten independent vacuum scalar products at four loops. The
physical graph may have fewer than ten propagators, so auxiliary ISP
coordinates complete its basis. They are not extra physical graph edges:
their starting target powers are zero, and numerator powers can make them
negative. The generic parametric IBP sources still have all ten index slots.

## Coordinate order

Slots `D1` through the last physical slot retain Vakint's propagator order,
with its `k(i)` written as `ki`. Momentum signs below are part of the input;
an overall sign of a single `qi` does not change its denominator.

| Slot | [H](four_loop_h.toml) | [X](four_loop_x.toml) | [BMW](four_loop_bmw.toml) | [FG](four_loop_fg.toml) |
| --- | --- | --- | --- | --- |
| D1 | k1 | k1 | k1 | k1 |
| D2 | k2 | k2 | k2 | k2 |
| D3 | k3 | k3 | k3 | k3 |
| D4 | k4 | k4 | k4 | k1-k3 |
| D5 | k1-k3 | k1-k3 | k1-k2 | k4 |
| D6 | k2-k3 | k2-k3 | k3-k4 | k2-k3 |
| D7 | k3-k1+k4 | k3-k1+k4 | k2+k3-k1 | k1-k3+k4 |
| D8 | k3-k2+k4 | k3-k2+k4 | k3-k4-k1 | k1-k2 |
| D9 | k3+k4 | k3-k1-k2+k4 | k1-k3 (ISP) | k2-k4 (ISP) |
| D10 | k1-k2 (ISP) | k3+k4 (ISP) | k2-k4 (ISP) | k3-k4 (ISP) |
| Parent sector | `1111111110` | `1111111110` | `1111111100` | `1111111100` |

The input targets have unit powers in every physical slot and zero in the
ISP slots. They specify sample integrals, not a promise of sector or family
closure. A parent-family artifact must also cover its required contractions
and numerator sectors; a successful source derivation alone proves neither.

## Run and verify

With a release CLI already built and `SYMBOLICA_LICENSE` set, run from the
repository root:

```console
rustred derive --input examples/input/four_loop_x.toml \
  --input-format toml --relations ordinary --n-cores 1 \
  --output TMP/four_loop_x_sources.toml
```

Replace `x` with `h`, `bmw`, or `fg` to study the other inputs. Expected
output is `status = "ok"`, ten denominator coordinates, inferred parameters
`["d"]`, and sixteen ordinary IBP sources. `derive` authenticates the
complete denominator basis using Symbolica's exact matrix inverse and
determinant before generating those sources. The recorded nonzero
determinants for these inputs are H `-64`, X `-64`, BMW `-64`, and FG `64`
(in RustRed's scalar-product coordinate order).

For an explicitly bounded solver diagnostic rather than source derivation,
choose a sector, for example:

```console
rustred family-solve --input examples/input/four_loop_x.toml \
  --input-format toml --sectors 0000000001 --n-cores 1
```

This selects one auxiliary-coordinate sector only, not the physical X
parent; it is a quick input-path smoke test. `family-solve` currently reports
diagnostic rule counts, not a publishable closing artifact. Full physical
parent and contraction studies must be requested explicitly and assessed
by exact artifact validation, not these smoke-test counts.

## Request complete closure of a physical parent

The auxiliary coordinates can be explicitly restricted to nonpositive powers
without bounding numerator rank. Use `--nonpositive-indices 9` for H/X and
`--nonpositive-indices 8,9` for BMW/FG. These are zero-based input indices, not
permuted ordering positions. For example:

```console
rustred family-close --input examples/input/four_loop_fg.toml \
  --input-format toml --nonpositive-indices 8,9 --n-cores 2 \
  --progress --output TMP/four_loop_fg_physical.rr
```

The equivalent Python request is
`rustred.family_close(source, input_format="toml", nonpositive_indices=[8, 9])`.
The Rust request uses `FamilyCloseRequest::nonpositive_indices`.

Larger families may need explicit publication-resource allowances. For example,
add `--max-domain-bound-endpoint-cells 65536` and
`--max-predicate-consistency-work 67108864` to `family-close`, and pass those
same settings to `campaign inspect` and `campaign reduce` when loading the
result. These are finite caller-selected storage/work budgets, not changes to
the identities, coverage domain, or proof requirements; their sufficiency for
a particular full family must be established by an actual successful run.
Defaults remain 8,192 endpoint cells and 4,194,304 consistency-work units.

Python accepts the same keyword names, `max_domain_bound_endpoint_cells` and
`max_predicate_consistency_work`, on `family_close`,
`inspect_closing_artifact`, and `reduce_with_closing_artifact`. In Rust, set
`FamilyCloseRequest.publication_limits`; cold requests carry independent
`load_limits`. A successful artifact never supplies or raises its own loading
budget. Increasing a resource allowance cannot make an uncovered region valid.

This includes every physical parent contraction and arbitrary negative ISP
powers. The sample target does **not** infer this domain: omitting the option
requests all ten-coordinate sectors, including positive ISP powers. Exact
replay, descent, coverage and cold-load validation remain mandatory. The
artifact binds its scope and refuses out-of-scope reductions.

These commands are closure attempts, not shipped four-loop artifacts. See the
[current measurements](../../docs/four_loop_parent_closure_probe.md); search
completion alone does not establish publication or four-loop Vakint parity.

## Five-loop connected graph case studies

[`five_loop_cube.toml`](five_loop_cube.toml) and
[`five_loop_mobius8.toml`](five_loop_mobius8.toml) are two distinct, connected
cubic vacuum graphs with eight vertices, twelve physical edges, and
`L = E - V + 1 = 5` loops. The cube consists of the squares `0-1-2-3-0` and
`4-5-6-7-4` joined by four corresponding spokes. The Möbius ladder is the
cycle `0-1-2-3-4-5-6-7-0` plus `0-4,1-5,2-6,3-7`. These are concrete graph
case studies, not claims of independent master integrals or coverage of all
five-loop topologies. Their names never select engine algorithms or rules.

Each input records its directed edge list in denominator order. The first
seven edges form a spanning tree; the remaining five carry `k1` through
`k5`. Tree-edge momenta solve the reduced incidence system exactly, with
outgoing minus incoming momentum zero at all eight vertices. Every
denominator is `qi^2 - 1`; the three additional coordinates complete the
15-dimensional scalar-product basis but are not physical graph edges.

| Input | Physical root | Zero-based nonpositive slots | Physical/full basis rank | Full determinant¹ | Graph automorphism order |
| --- | --- | --- | --- | --- | --- |
| Cube | `111111111111000` | `12,13,14` | `12 / 15` | `1024` | `48` |
| Möbius-8 | `111111111111000` | `12,13,14` | `12 / 15` | `-1024` | `16` |

¹ Rows are `D1..D15`, and columns are `ki·kj` in lexicographic order
`i <= j`, with cross-term coefficient `2 qi qj`. Exact ranks, incidence
solutions, and determinants were checked with Symbolica's rational matrix
API. Symbolica's graph API checked connectivity, cubic valence, canonical
automorphism orders, and that the two graphs are not isomorphic. The public
`derive` command independently accepts both full bases and generates 25
ordinary IBP sources per family.

For example, source preparation alone is:

```console
target/release/rustred derive --input examples/input/five_loop_cube.toml \
  --input-format toml --relations ordinary --n-cores 1 \
  --output TMP/five-loop-cube-sources.toml
```

Matching `.input` files provide the same families in compact Symbolica syntax
without explicit parameter declarations. Their derived families and sources
were checked identical to their TOML counterparts. A physical-family solve
must explicitly restrict slots `12,13,14` to nonpositive powers; target
zeros and descriptive metadata do not impose the solver's domain. The domain
then includes the physical parent and its contractions, with arbitrary ISP
numerators. Solving only `111111111111000` is a parent-sector diagnostic,
not completion of that whole physical family.

The older `five_loop_complete_scalar_product` and
`five_loop_chain_with_isps` inputs remain algebraic parent-sector controls.
Their all-positive targets are not substitutes for these twelve-edge
physical graph workloads. No five-loop closing artifact or successful full
family solve is claimed by the new input definitions.

Candidate generation can now be steered without recompilation through the
CLI or Python. For example, from the repository root with `TMP/` present:

```console
target/release/rustred family-candidates \
  --input examples/input/five_loop_cube.toml \
  --nonpositive-indices 12,13,14 --n-cores 6 \
  --exact-backend semi-numerical --progress \
  --output TMP/cube.rrcandidate --report-output TMP/cube.report.toml
```

The corresponding Python call is:

```python
from pathlib import Path
import rustred

result = rustred.family_candidates(
    Path("examples/input/five_loop_cube.toml").read_text(),
    nonpositive_indices=[12, 13, 14], n_cores=6,
    exact_backend="semi-numerical",
)
Path("TMP/cube.rrcandidate").write_bytes(result.bundle)
print(result.to_toml())
```

Use `exact_backend="sparse"` / `--exact-backend sparse` for the default exact
materializer. Both choices retain the same source search and bounded numerical
corner search. Semi-numerical symbolic materialization uses Symbolica's
reconstruction API with finite degree/probe/prime limits, and reports failure
rather than silently falling back. Candidate output is **not certified**.
These five-loop commands are resource-intensive attempts, not promises of
completion. In particular the default candidate-output budgets still apply;
large outputs can require an explicitly enlarged Rust-library policy.
Reports separate preparation, solver and encoding times and identify the
backend. CLI progress uses an overwriting field on a terminal; `--progress`
opts into plain stderr when redirected, leaving the data stream unchanged.

### A smaller connected five-loop input

[`five_loop_banana.toml`](five_loop_banana.toml) instead has two vertices joined
by six massive lines: the five independent momenta `k1` through `k5`, and their
sum. It is a connected five-loop graph, not a product chosen as the parent.
Nine auxiliary pair-squares complete the fifteen scalar-product coordinates;
the missing `k4·k5` is recovered from the total square. This is ordinary input
data, with no special solver path or authored rule.

Its physical root is `111111000000000`. Explicitly restrict zero-based slots
`6,7,8,9,10,11,12,13,14` to nonpositive powers. Any five of the six physical
momenta form a unimodular basis, so the six one-line pinches factor into five
tadpoles. Supports with at most four physical lines leave a scaleless loop
direction. Thus native preparation is expected to find seven nonzero sectors
and 57 zero sectors in this physical downset. This sector count is not a master
count or a guarantee of fast rule generation: the parent still has nine
independent numerator coordinates.

For a candidate-generation attempt using the unchanged generic engine:

```console
target/release/rustred family-candidates \
  --input examples/input/five_loop_banana.toml \
  --nonpositive-indices 6,7,8,9,10,11,12,13,14 --n-cores 1 \
  --exact-backend sparse-factorized --numerical-depth 0 --progress \
  --checkpoint-dir TMP/banana-sectors \
  --output TMP/banana.rrcandidate --report-output TMP/banana.report.toml
```

Use an empty checkpoint directory for a fresh run. As with the other five-loop
inputs, impose an explicit process time/memory budget when experimenting. A
successful candidate save would still not certify closure or cover other
five-loop topologies. See the [measured baselines](../../docs/research/five_loop_candidate_baselines.md)
for actual outcomes rather than inferring them from the smaller sector census.

### TIDE's complete five-loop reference census

[`tide_five_loop.toml`](tide_five_loop.toml) supplies the fifteen ordered
auxiliary momenta of Luthe's five-loop thesis. Its four twelve-line parents
and all 67 published topology representatives are recorded as offline input
data in [`tide_five_loop_manifest.json`](tide_five_loop_manifest.json).
Neither file installs topology-specific strategies or relations in RustRed.

| Reference parent ID | Positive slots, in input order | Zero-based nonpositive slots |
| --- | --- | --- |
| 32745 | `111111111101001` | `10,12,13` |
| 31740 | `111101111111100` | `4,13,14` |
| 30699 | `111011111101011` | `3,10,12` |
| 30527 | `111011100111111` | `3,7,8` |

The reference IDs use a big-endian bit convention. In particular, slot 14 is
`k1+k2-k4`, so this is not the basis of all pairwise momentum differences.
As with the other examples, denominators are `qi^2-1`; numerical comparisons
with the thesis's Euclidean `qi^2+1` convention require an explicit measure
and sign conversion.

Source preparation is a short reproducible check:

```console
target/release/rustred derive --input examples/input/tide_five_loop.toml \
  --input-format toml --relations ordinary --n-cores 1 \
  --output TMP/tide-five-loop-sources.toml
```

Expected results are fifteen denominator coordinates, inferred parameter `d`,
and 25 ordinary IBP sources. CLI and Python derivation agree, and Symbolica's
exact basis determinant is `-1024`. These checks do not solve any sector.
The manifest distinguishes the 48 nonfactorized and 19 factorized classes.
All 67 representatives now have independently checked simultaneous integer
momentum-routing [witnesses](tide_five_loop_routing_witnesses.json) to parent
contractions, with determinant `±1`.
This checks the input cover, not any IBP result. See the [research and reproduction plan](../../docs/research/tide_five_loop_census.md)
for the exact census, published scope, current checks, and progressive solve
sequence. No completed five-loop family is claimed.
