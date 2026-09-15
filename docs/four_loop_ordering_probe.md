# Four-loop ordering probe

This note records bounded ordering experiments on the externally supplied
`examples/input/four_loop_h.toml` family. The ordering is a solver input, not a
family or topology selector. The example driver now passes the same ordering
permutation to the source-port audit, so replay and publication use one
coherent ordering authority.

All runs below used the release `spired-generate-four-loop-h` example, six
workers, unit mass, and the current Symbolica-backed exact replay. None of the
runs produced a durable artifact; publication remains fail-closed.

| coordinate permutation | result |
| --- | --- |
| natural (implicit) | 60/63 rules replayed and descended; 7 uncovered boxes; three affine target/exceptional diagnostics |
| reverse `9,8,7,6,5,4,3,2,1,0` | 68/70 replayed and descended; zero uncovered boxes; two affine target/exceptional diagnostics |
| rotate `1,2,3,4,5,6,7,8,9,0` | 55/60 replayed and descended; zero uncovered boxes; five affine diagnostics |
| even/odd `0,2,4,6,8,1,3,5,7,9` | solver stopped earlier on an unsupported nonlinear exceptional intersection |

The reverse ordering is therefore a useful candidate for the eventual bounded
portfolio, but it is not a closure result. The remaining two affine rules are
lower-dimensional ownership strata. They cannot be represented honestly as
rectangular boxes: adding their bounding boxes would over-cover points where the
affine equality is false. A complete four-loop artifact still needs the generic
affine application-domain and exact mixed-stratum coverage path described in
the four-loop closure plan. No H-specific rule is hard-coded here.

