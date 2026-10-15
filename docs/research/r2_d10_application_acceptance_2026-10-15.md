# Finite R2/D10 native Vakint application controls

Date: 2026-10-15. Implementation lane: `/root/soft_ct_nested_audit`.
Independent source reviewer: `/root/intermediate_capture_independent_audit`.

## Scope and package

This is application of an already completed saved publication, not new IBP
generation, a numerical-master calculation, or a new closure certificate.
Neither the running campaign nor the earlier R0 application package was changed.

The source is the immutable publication at
`campaigns/five-loop-minimal-r0-d10-incremental/master-reduction/scopes/14b5bd1ebf34d50913b4a6bd49a5180e476fa8126f074b3bcdc854364a2d2029/artifact.json`,
SHA256 `92f21a83246d2f66b3743b9d0f1894a91d6929a80e217d1e24bcab58ce89eab7`.
Its publication-phase checkpoint generation is **3**, while its source campaign
checkpoint generation is **8**. Those are distinct counters.

The export retains 348 recorded queries (294 nonempty and 54 empty), checks all
899 source terminal substitutions, and retains 869 terminal keys. Its reported
R2/D10 maxima describe the recorded union, not every integer input below those
maxima. Its assurance remains `trusted_saved_scope`; neither unrestricted family
closure nor master independence is asserted.

The new, untracked portable package is
`/shared/codex-2/rustred/TMP/r2-vakint-acceptance-20261015/application-r2-d10`.
It contains 77 files totaling 245,885,881 bytes. The manifest SHA256 is
`a5231e323a4ffba7e26c785f471c75f2965094032a8ffaeba66cdccc6bfc24b5`.
All file hashes are in the adjacent `package-files.json`.
Native export took 300.061 s; guarded wall time was 303.079 s, with sampled
peak process-tree RSS 14.750 GB. No traversal ledger is packaged for evaluation.

## Exact controls

Inputs use native FeynKit graph construction, explicit loop-momentum-edge
selection, DOT round trips, D-dimensional kinematics, and Vakint's existing
`from_diagram` ingress. The fast preflight proves the complete denominator and
numerator transport through native all-row/unit-Jacobian witnesses and exact
RustRed scalar lowering. It passed in 0.08 s.

In the common family, let `P_i=q_i²−1`. The five independent factor momenta are
`r=(q5,q8,q9,q12,q15)=(k5,k1−k5,k2−k3,k3−k5,k3−k4)`.
Writing `J_a=∫(r²−1)^−a` and `K_a=J_(a−1)+J_a`:

| Control | Physical numerator / powers | Exact comparison | Current result |
|---|---|---|---|
| A | Six existing expanded positive-line cancellations, including dotted/multiple lines and nonunit mass | Independently pinched native DOT, identical exact values, guards and families; warm repeat | All six passed |
| B | `r1²−r2²`, powers `(2,3,1,2,3)` | Two separately reduced lowered pieces and `(J1 J3−J2²) J1 J2 J3` | Passed |
| C | `[r1·(r2−r4)]²`, powers `(2,3,1,3,3)` | Independent tensor-factor result `2 K2 K3 J1 J3²/d` | Passed |
| D | Connected six-line banana, `k1²−k3²`, powers `(1,1,1,1,1,5)` | Two separately admitted irreducible-numerator pieces | Passed |

B and C are important admission-boundary controls: their bare denominator
keys have A11 and A12 and are **not admitted**. Their complete physical
numerators lower to respectively two and ten admitted keys. The runtime tests
check both facts against the actual prepared package. No starting cap is
widened and no admission check is bypassed.

C genuinely reaches negative-index degree two: for its base `b`, the endpoints
are `b−e_i−e_j`, `i≤j`, for zero-based slots `{0,2,7,11}`. They have D10 and are
contained in recorded query `rank2-d10-00044-7e8b667af4f4c09466639413`.
The independent tensor reference includes `d != 0`; equality is exact on the
intersection of retained conditions. We do not assert that two independently
derived conservative guard lists must be identical. Same-input first/warm
values, guards and family identities are compared identically.

The hard A control has an expanded degree-ten momentum numerator, but it
cancels to one R0/D9 key. That does **not** establish arbitrary irreducible
scalar-rank-five coverage. The B and C momentum degrees are two and four,
respectively; both have open tensor rank zero.

All tests select symbolic RustRed mode with an invalid FORM path. Independent
factor references use the existing shipped one-loop rules, not numerical
master values or FORM. These finite tests do not complete the full R6 goal,
all recorded-domain witnesses, or the complete 19-class factorized census.

## Timings and caches

Optimized native builds; compilation is excluded. Each test process loads its
own package. CPU affinity is 24–27 for B/C/D; A retains 20–23. A was running
concurrently, and the shared host was not isolated. The figures are measured
acceptance timings, not a paired whole-campaign speedup claim.

| Control | Load/init | First application | Warm repeat | Guarded whole test | Peak tree RSS |
|---|---:|---:|---:|---:|---:|
| A six cancellations | 75.994 s | Not separately timed | Not separately timed | 1597.940 s | 2.817 GB |
| B homogeneous factor | 75.979 s | 11.042 s | 0.375 s | 110.526 s | 2.797 GB |
| C cross-factor | 76.618 s | 13.108 s | 0.384 s | 92.464 s | 2.794 GB |
| D connected irreducible | 78.342 s | 70.225 s | 0.865 s | 255.064 s | 3.044 GB |

Load/init includes native Vakint construction and the loaded five-loop package
and catalogue. The B and D whole-test boundaries also include their two independent
piece reductions. The C whole test includes exact independent-factor checks.

B retained 12 coefficient-cache entries and 63 coefficient terms (2,982
accounted bytes), with nine rule applications and five coefficient hits across
the full test. Its witness memo reported one hit and three misses. C retained
91 coefficient-cache entries and 983 coefficient terms (60,392 accounted
bytes), with 80 rule applications and 98 coefficient hits; its witness memo
reported one hit and one miss. These accounting bytes are not process RSS.

D completed 12,565 rule applications with 208,368 coefficient-cache hits over
its complete first/warm/pieces test. It retained 26,148 cached integrals and
4,811,564 coefficient terms (236,951,956 accounted bytes). Its witness memo
recorded one hit and three misses. This connected result is nonzero and its
independent-piece parity is exact; it is not a scalar-only or zero-integral
shortcut. These are cumulative counters across the whole test, not an isolated
cold-vs-warm counter subtraction.

A's per-case combined first/pinched/warm comparison times, after its 75.994 s
load, are below. They are **not** separate first/warm measurements.

| Cancellation control | Combined comparison time |
|---|---:|
| Single positive line | 12.756 s |
| Dotted positive line | 3.546 s |
| Two positive lines | 1.399 s |
| Mixed dotted lines | 749.090 s |
| Single line, nonunit mass | 12.599 s |
| Mixed lines, nonunit mass | 740.619 s |

The mixed-dotted 749.090 s boundary includes
a parent-authorized five-second, low-frequency read-only profile of the owned
test process; no profiling correction was subtracted.

That sample contained 247 `cycles:u` samples with no lost samples. Inclusive
stacks included Symbolica `to_polynomial_in_vars_impl` (73.55%), RustRed
`lower_weighted_in_request` (60.72%), and Vakint
`rustred_matched_input_key` (45.42%) inside topology-witness acceptance. These
percentages overlap and describe only that five-second window. They identify
repeated scalar lowering during witness admission as an actual active cost;
they do not measure the whole test or justify attributing every slow second to
one function. Existing coefficient/witness caches do not eliminate first-use
work on a new numerator.

## Reproducibility and remaining gates

Evidence is under `TMP/r2-vakint-acceptance-20261015/`:

- `binding-before.json`, `binding-retry-before.json`, `binding-after.json`:
  exact source, dependency, tool, publication and executable bindings.
- `fast-result.json`, `export-result.json`, `B-homogeneous-factor-result.json`,
  `C-cross-factor-result.json`, `A-cancellations-result.json`,
  `D-connected-result.json`, and each phase's stdout/stderr/resource receipt.
- `A-cancellations/short-mixed.perf.data`, `perf-self.txt`,
  `perf-inclusive.txt`, and `profile.json` for the bounded sample.
- `BC-independent-audit.json` and `final-independent-audit.json` for the
  separately performed source/binding/runtime reviews.
- `frozen-vakint-unit-tests`, SHA256
  `92225fdbdb17af3f3873ba417ea7b86677d83d02c84f502a093326dfa9946fb5`.

The optimized Vk stack uses RustRed `78cbc5c1`, Symbolica `400a53c4` and
FeynKit `024f96d6`, on Vk base `7b817a99` plus the audited test-only diff.
The separate current export helper is SHA256
`ecb3c5bb4d56e8083548db7445d7cb3140555dac2a4144aa7fc0a1d32c564407`.
Its source is the frozen `d74cfad2` finite-census export build; this does not
silently change the application test's RustRed dependency.

The first incorrect-CWD build was stopped after 108.287 s. A subsequent build
exposed only a reused example-fixture import problem inside libtests; it is
retained as a failed receipt (463.646 s). The corrected optimized test build
completed in 636.958 s. No assertion or algebra was weakened to fix the import.
The independent reviewer approved both the source and that setup correction.

All four external tests (nine physical controls) and the fast structural test
completed successfully. No pilot was censored and all owned process groups
drained. Each runtime pilot had a 29-minute cooperative deadline before cleanup,
a 32 GB RAM allowance and 150 GB host reserve; A completed in 26 minutes
38 seconds. These controls do not replace the unfinished full R6 acceptance
inventory or a proof of universal QCD coverage.

The independent source and runtime audits passed the fast/export/A/B/C/D gates.
The final audit rechecked 80 source/input hashes, all 77 package files, the frozen
executable, actual nonvacuous test execution and clean process termination.
The five test/fixture/documentation files were committed and pushed to
`alphal00p/gammaloop:vakint_rustred` as
`7d2331ef7fbedaa9641897d01f3954eb049f9662`, with both author and committer
`ValentinHirschi <valentin.hirschi@gmail.com>`. The checkout was clean after the
push. Rustfmt checks, Git whitespace checks and standalone Typst compilation
passed; the generated PDF and application package remain under ignored `TMP/`.
No dependency, engine, artifact or production-campaign change was included.
