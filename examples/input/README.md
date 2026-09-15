# External four-loop vacuum families

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
