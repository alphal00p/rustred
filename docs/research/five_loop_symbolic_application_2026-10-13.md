# Five-loop saved-rule application: validation and measurements

This is a work-in-progress record, not a claim that the full Vakint acceptance
suite passes. No new IBPs or numerical masters are generated in this work.

## Scope and runtime data

The fixed source is THE_ONE generation 54, scope
`f7ee869008fccc6ed6cf2f4e3342c78d2b8607f59ac4d523dcae504595c1b0f8`.
It contains 67 owners, 9,982 rule records, 804 recorded starting domains,
939 terminal source keys and 905 retained terminal keys after finite refinement.
Of the historical requests, native geometry proves 41 empty; the application
admission union contains the other 763. The rank-11 and D=9 maxima are **not**
blanket coverage of every integral below those values. All admission tests use
the recorded, correlated domain predicates.

The local application package excludes the traversal ledger and finite-search
matrix. It uses native Symbolica coefficient data, compressed owner chunks,
exact momentum routes, admission predicates and terminal substitutions. It is
never committed or pushed. Vakint must receive an explicit artifact path; no
configured path means no five-loop RustRed processing.

Initial local export: `TMP/five-loop-vakint-20261013/application-gen54-first`.
Manifest SHA256:
`01425e1cecc71893d2e3a592a162600100326f285914926df61e43dd0c870e4b`.
Package size: 281,312,247 bytes, including 196,539,548 compressed owner bytes and
80,758,329 terminal-map bytes. This checksum identifies the manifest, not a
single-file archive. The manifest binds its constituent payloads.

Export checked substitutions for all 939 source keys. It took 317.306 s inside
the example / 321.443 s including the process supervisor, with sampled process
tree peak RSS of 36.860 GB. This is an offline packaging cost, not IBP generation.

## Witness inventory

A deterministic feasible representative, maximum-numerator-rank witness and
maximum-positive-power witness are extracted for each nonempty recorded domain.
After deduplication the inventory contains 813 concrete integrals (0.129 s).
Scalar negative-index degree R ranges from 0 to 11; positive-power sum A ranges
from 9 to 20. Momentum degree is reported separately: a homogeneous GeV^10
scalar-product numerator has five scalar products, not scalar degree R=10.

| Scalar negative-index degree | Distinct inventory cases |
|---|---:|
| 0 | 67 |
| 1 | 112 |
| 2 | 120 |
| 3 | 120 |
| 4 | 94 |
| 5 | 84 |
| Each of 6 through 11 | 36 |

The 67 degree-zero witnesses are not a claim of 67 distinct graph classes.
Graph census coverage and integral admission are separate tests. These boundary
samples are not exhaustive enumeration of every integer point in each domain.

## Matched guard-union optimization

Both runs used optimized frozen executables, the same runtime package and four
input points, CPU affinity 48–55, one observed computational core, and unchanged
Symbolica/Rayon worker limits. Compilation is excluded. Other host workloads
were active; this is one matched pair, not a confidence-interval study.

Warm calls used no new rule applications but formerly spent most time scanning
guard vectors for duplicates. A 10 s profile attributed 78.12% to polynomial
equality and 14.94% to `memcmp`. The correction uses existing native Symbolica
Hash/Eq, preserving first-seen condition order, and shares identical immutable
guard vectors after decoding. No guards are suppressed.

| Point | Output terms | Conditions | Baseline first (s) | Optimized first (s) | Baseline warm median (s) | Optimized warm median (s) |
|---|---:|---:|---:|---:|---:|---:|
| 00162 | 1 | 27,494 | 1.8434 | 0.0168 | 1.8675 | 0.0175 |
| 00163 | 23 | 27,570 | 74.7732 | 0.4098 | 73.8239 | 0.0212 |
| 00164 | 22 | 27,494 | 1.8547 | 0.0174 | 1.8659 | 0.0181 |
| 00165 | 1 | 27,494 | 1.8526 | 0.0175 | 1.8494 | 0.0181 |

Cold package preparation fell from 171.692 to 152.370 s; whole-run sampled peak
RSS fell from 18.764 to 7.736 GB. Each run independently checked exact first/warm
coefficient and guard equality. Matching output counts alone are not an oracle
comparison or an independent proof of the source rules.

Evidence directories under `TMP/five-loop-vakint-20261013/`:
`first-four-points/`, `guard-sharing-four-points/`.

## Operational-limit failures and retry

The initial full-corpus pilots used the ordinary 1 GiB / 16 million aggregate
coefficient-term cache policy. Several rank-one reductions exceeded it. These
are resource-limit errors, not missing-rule frontiers.

- Independent mode (cache cleared between inputs) was stopped after 107
  successes and 12 resource failures; unattempted cases remain untested.
- Shared mode reported 76 successes and 737 failures. After the first exhausted
  cache, most later errors were caused by the same retained full cache: they
  must not be interpreted as 737 independent mathematical failures.
- Retrying `point-00154` with an explicit 32 GiB byte budget and no separate
  aggregate-term ceiling completed successfully. The other operation budgets
  and mathematical admission scope were unchanged.

That input is `[-1,1,1,0,1,1,1,0,1,1,0,1,0,0,2]`: R=1, A=10, D=9.

| Measurement | Result |
|---|---:|
| Fresh-process package preparation | 158.880 s |
| First application after preparation | 316.898 s |
| Warm repeat | 0.3444 s |
| Output terms / conditions | 82 / 29,324 |
| Operational nodes / rule applications | 99,679 / 41,857 |
| Cached coefficient payload | 3.029 GB |
| Sampled process-tree peak RSS | 11.436 GB |
| Exact first/warm equality | passed |

Evidence: `cache-32g-retry/`; frozen executable SHA256
`7f2c2d728e536b4dfbd4d2d8463fe02f838e0d22ff684b58757ced9795684212`.
Factorized cache storage is an opt-in reuse of RustRed's existing Symbolica
factorized-polynomial arithmetic; comparative measurements are pending.

One degree-11 boundary witness (`point-00000`, A=20, D=9) completed in 0.0412 s
after 162.464 s preparation, with a 0.0188 s exact-equal warm call and one output
term. It is an easy factorized case, not an estimate for all high-rank inputs.
A subsequent 36-case degree-11 cohort was stopped diagnostically after one
success and three default work-limit errors: aggregate transport endpoints,
operational nodes, and aggregate transport operations. The run is censored
(414.623 s, 15.769 GB peak); unattempted cases remain untested. The native driver
now accepts explicit named operational limits from JSON without rebuilding.

## Vakint matching milestone

The corrected native topology suite passes 16 tests in 24.62 s in a debug build,
including native DOT round trips for all 67 registered reference classes,
disconnected products and alternative factor attachments, simultaneous
numerator/LMB routing, and existing lower-loop routing regressions. This is
matching validation, not full saved-artifact application acceptance.

A slow five-loop case was attributed to the old one-through-four-loop matcher
sweep: 130.390 s before entering native matching, versus 0.0099 s for native
input and 0.0786 s for incidence plus exact routing verification. Dispatching
validated five-loop long-form input to the native matcher first removes that
unrelated sweep. The lower-loop path stays intact. These debug measurements
diagnose control flow; they are not release-performance claims.

Independent review also required preserving the previous PySecDec eligibility
and `allow_unknown_integrals` fallback when an exact unit-Jacobian proposal
fails. The absence of a configured artifact path still disables five-loop
RustRed; it does not disable unrelated existing evaluation methods.

The actual external generation-54 package cold-loads successfully in Vakint's
Symbolica 3.0.1 environment. Debug load plus preparation took 1152.745 s, of
which 2.521 s was structural product preparation. It identifies 38 positive
product terminals, retains 867 flat terminals (268 with numerator indices),
and prepares 15 lower-family descriptions. No binary-format migration was
needed. This does not establish complete factor-product reduction parity;
numerator-bearing factorized cases still need explicit validation.

## Outstanding acceptance work

- Complete admitted inventory reductions, including higher-rank cases, with
  resource outcomes reported individually.
- Validate exact alternative-LMB and numerator-cancellation comparisons, and
  factorized lower-program composition through the now-tested package loader.
- Complete generated-graph census reconciliation and lower-loop regressions.
- Report matching, tensor, scalar application, terminal normalization and
  rendering times separately in the final Vakint acceptance table.
- Verify missing-path disable behavior even after an instance has populated
  its application cache. No numerical substitution or FORM fallback is allowed.

The exported package inherits the saved scope's authority. It does not claim
independent certification, minimality, or unrestricted five-loop closure.
