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
  --output /tmp/four_loop_x_sources.toml
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
  --progress --output /tmp/four_loop_fg_physical.rr
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
