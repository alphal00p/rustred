# Unmeasured runtime integral-order study inputs

These three version-1 JSON descriptors are **unmeasured input recipes**, not
recommended production defaults or engine-specialized strategies. They apply to
the denominator coordinates in `../four_loop_common_basis.toml`. Use the same
compiled engine with `family-candidates --integral-order FILE`; regenerate all
selected owners for each descriptor. Do not apply a new order to saved old rules
or combine this flag with `--permutation`.

Every comparison first prioritizes support count and support ordering. Within a
fixed support all three programs are **E-primary**: the first degree row is
unweighted total excess
`E = sum(active: n_i-1) + sum(inactive: -n_i)`. Additional rows break its ties.
Thus E-primary does not mean E precedes support changes. Native descriptor
admission validates order admissibility; exact source replay and eventual scoped
closure remain separate validation gates.

## Coordinate meaning and structural motivation

Each denominator is `q_i²-1`; axes below are zero-based, matching the JSON vectors.

| Axis | Denominator | Momentum `q_i` | Momentum terms `r` | `r(r+1)/2` |
|---:|---|---|---:|---:|
| 0 | D1 | k1 | 1 | 1 |
| 1 | D2 | k2 | 1 | 1 |
| 2 | D3 | k3 | 1 | 1 |
| 3 | D4 | k4 | 1 | 1 |
| 4 | D5 | k1-k4 | 2 | 3 |
| 5 | D6 | k2-k4 | 2 | 3 |
| 6 | D7 | k3-k4 | 2 | 3 |
| 7 | D8 | k1-k2 | 2 | 3 |
| 8 | D9 | k1-k3 | 2 | 3 |
| 9 | D10 | k1-k2-k3 | 3 | 6 |

The two roots are `1111111110` and `0111111111`, whose intersection is
`0111111110`. Axis 0 is private to the first root; axis 9 to the second. The
existing frozen routing census also maps the first root's single pinches at
axes **0, 3 and 4** to the shared owner `0111111110`. This is concrete input
structure, not a topology-name branch in the solver.

- **`default_program.json`** equals the public `rustred.integral_order(10)`
  builder. Its comparator reproduces the uncut SpIRed reference priorities:
  natural support order, total excess, numerator excess, then descending
  active/inactive coordinate ties. The explicit descriptor has its own persisted
  identity, so comparison with the legacy default concerns semantic equivalence,
  not byte-identical output.
- **`shared_interface.json`** gives the two private axes support weight 3 and
  all others 1. Among equal-line supports this favors retaining fewer private
  axes. Support ties inspect 0 and 9 first, then the additional shared-owner
  pinch axes 3 and 4. Within fixed support and E, a second row penalizes positive
  excess on private axes; numerator excess follows. Final coordinate ties remain
  natural. The hypothesis is less fragmented work around a reusable common
  subtopology. The falsifier is greater end-to-end cost, pending pressure or
  branching from overemphasizing a difficult shared subsector.
- **`routing_density.json`** retains reference support ordering. After E it
  compares weighted excess with coefficients `r(r+1)/2` for both signs, then
  numerator excess. Final ties inspect larger-density axes first, preserving
  descending direction. The weights count scalar-product terms before collection
  when squaring the explicit `q_i`; they are a representation-dependent proxy for
  coefficient fill, not a graph invariant or predicted runtime. At fixed E,
  smaller weighted excess is simpler; final descending ties do not globally
  penalize each high-density coordinate independently.

## Running and comparing

From the repository root, a first-root invocation includes:

```sh
rustred family-candidates \
  --input examples/input/four_loop_combined/four_loop_common_basis.toml \
  --input-format toml --nonpositive-indices 9 \
  --integral-order examples/input/four_loop_combined/order_programs/shared_interface.json \
  --output TMP/ORDER_STUDY/root1022.rrbin
```

This abbreviated command illustrates descriptor selection, not the frozen
performance protocol. The full acceptance commands, matched discovery recipe,
checkpoint/staging workflow and resource controls are documented in
`tools/research/runtime_order/README.md`. The second root uses inactive axis 0.
Keep the previous terms-first/coefficient-monomials discovery winner fixed across
these arms; changing mathematical order and discovery order simultaneously would
obscure the cause. Compare generation plus complete scoped walking and independent
cold verification, not only sparse elimination or CPU utilization. Preserve all
16 selected owners, 508 route witnesses and the same 58 required query bytes.

The JSON files can also be loaded by the public Python API:

```python
from pathlib import Path
import rustred

source = Path("examples/input/four_loop_combined/four_loop_common_basis.toml").read_text()
order = Path("examples/input/four_loop_combined/order_programs/routing_density.json").read_text()
result = rustred.family_candidates(source, nonpositive_indices=[9], integral_order=order)
```

No result here claims these descriptors outperform the previous discovery winner
or close the full five-loop request. Promote a candidate only after native
admission and reproduced matched measurements.
