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

### What a `point-XXXXX` names

These are deterministic test-inventory IDs, not topology names, master labels,
or ranks. A row records its exact 15-coordinate integral key and every original
query/domain boundary that proposed it. Sorting the execution order by rank
does not renumber the rows. Multiple domains can share one concrete witness.

For example, `point-00435` has powers
`[1,1,0,1,1,0,1,1,1,1,0,1,0,-1,1]`. Thus ten coordinates appear as simple
denominators, coordinate14 is an inverse-denominator numerator, and four
coordinates are absent. It has scalar negative-index degree1, A10 and D9;
it is not an open rank-one Lorentz tensor. Inverse denominators include their
constant mass term, so their engineering degree2 is not a claim of a purely
homogeneous momentum numerator. This point is the representative of the
recorded `rank1-d9-00034-2c3b1465245235ff9137c9c0` region and also of several
higher-rank extensions of that region.

Other expensive completed witnesses have the same R1/A10/D9 summary but
different exact supports and dots:

| Inventory ID | Exact integral powers | First application / warm repeat |
|---|---|---|
| point-00159 | `[-1,1,1,1,0,1,1,1,1,1,0,1,1,0,0]` | 1111.775 s / 0.0786 s |
| point-00233 | `[0,1,0,0,1,1,1,1,1,1,0,1,-1,1,1]` | 1062.201 s / 0.9422 s |
| point-00390 | `[1,-1,1,0,1,1,1,1,1,0,0,1,0,2,0]` | 878.037 s / 1.4368 s |

These are first encounters in the shared native sweep, not independent cold
processes or full Vakint front-end timings. They demonstrate that scalar rank
alone is a poor predictor of application cost. The separate first/warm checks
compare exact coefficients and retained guards, not numerical zero samples.

### Final-output validity checks

The core accepts only installed terminal declarations as leaves; missing rules
and dependency cycles are errors. The saved application then requires every
returned leaf to have a published terminal-substitution row. At package loading,
all row targets must belong to the published remaining-terminal set, with valid
family identities, coefficient contexts and conditions. Consequently a
successful flat result contains only those declared family-qualified keys.
There is no catch-all conversion of unresolved integrals into new masters.

Vakint can additionally replace an established factorized terminal by its
verified product chart and existing lower-loop reductions. Those outputs may
contain qualified lower-loop masters rather than literal five-loop terminal
keys; exact routing, factorization and lower-program terminal validation provide
their authority. This is not master minimality or independent certification of
the original saved IBPs. Native mutation/missing-rule/cycle tests exercise the
failure paths; no expensive source replay is repeated per evaluation.

## Completed-result eviction: real package pressure test

An exact comparison of point00163 uses the same package, selected rule policy,
native sparse coefficients and work limits, changing only cache policy/bytes.
One process loads successive handles with eight preparation workers on CPUs24--31;
it drops the first handle before loading the second. These are not independent
cold-process measurements. First application includes deferred owner initialization.

| Measurement | Reference, 1 GiB fail-on-limit | 4 MiB LRU |
|---|---:|---:|
| Package preparation | 37.544 s | 39.360 s |
| First application | 11.838 s | 11.835 s |
| Included deferred-owner preparation | 11.373 s | 11.205 s |
| Warm application | 22.4 ms | 31.4 ms |
| Retained cache entries | 7,627 | 3,603 |
| Retained coefficient/guard bytes | 7,807,476 | 4,194,008 |
| Evicted entries | 0 | 4,839 |
| Same-request recomputed nodes | 0 | 815 |
| Rule applications | 3,753 | 4,334 |

All23 output coefficients and27,570 ordered guards agree exactly across both
policies and warm repeats. Every output key belongs to the authenticated
remaining-terminal set. Whole supervised run104.28s, sampled peak3.82GB.
No general speedup claim follows: eviction reduces retained memory but can
increase exact recomputation. An earlier1MiB trial correctly failed on the
protected working set after substantial eviction; its negative evidence remains.

Frozen driver `saved_application-evict-lru`, SHA256
`e5325f40a29f14b44de6d7b2d6905242e9f978ce04aa07589fef6edd9b34a1f5`;
evidence under `TMP/five-loop-vakint-20261013/`:
`point00163-exact-eviction-comparison-4m` and the unsuccessful
`point00163-exact-eviction-comparison`. Focused release tests576core/24app/6driver
pass. Real Vakint cache-pressure and all813 frontend-admission runs remain pending.

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
factorized-polynomial arithmetic. A subsequent matched comparison on this same
input completed with both representations; sparse remains the default:

| Representation | First application (s) | Warm median (s) | Retained coefficient bytes (GB) | Process peak RSS (GB) |
|---|---:|---:|---:|---:|
| Sparse | 317.829 | 0.3402 | 3.029 | 11.430 |
| Factorized | 485.522 | 0.3522 | 3.374 | 11.948 |

Both report 99,679 operational cache entries, 41,857 rule applications,
2,848,551 coalescing additions and 82 output terms. Each independently passed
exact cold/warm equality. These receipts alone do not compare the full real
output maps across processes; focused unit tests cover cross-representation
equality. Factorized storage is not a general performance win demonstrated by
this input. The two probes ran concurrently on disjoint CPU sets 48–55 and
56–63, with equal 32 GiB coefficient budgets and 100 GB process guards.

Their cold preparation took approximately 154 s each. Newly separated phases
show 80.1–80.6 s owner preparation, 70.0–70.1 s route preparation, 3.0–3.1 s
payload loading, and 0.3–0.4 s terminal-map import. This motivates per-owner
deferred preparation and reuse of the existing parallel route verifier, not
another coefficient codec or CAS implementation.

Evidence: `point154-sparse-final/`, `point154-factorized-final/`, executable
`saved_application-final-eager` SHA256
`d6354da1934a7c94a9ac75bcd66b44ae3e4aa66afcc039a88aa986e0b50acc1b`.

### Deferred preparation and worker comparison

The later frozen `saved_application-lazy-batch` executable (SHA256
`a0f3b21d3e97fa8a144dd7907eaadab66249ac7d798137f49211d18759303d8c`)
ran the same four degree-zero cases with one and eight preparation workers.
Both completed and each independently checked exact first/warm coefficients
and guards. Native route parity across worker widths is also unit-tested;
these process receipts do not themselves compare full output maps across runs.

| Boundary | One worker | Eight workers |
|---|---:|---:|
| Cold package preparation | 77.286 s | 36.904 s |
| Of which route preparation | 68.718 s | 28.665 s |
| Deferred owner preparation during four applications | 13.962 s | 13.611 s |
| Prepared owner count after four cases | 2 / 67 | 2 / 67 |
| Whole supervised process | 94.262 s | 54.215 s |
| Sampled process-tree peak RSS | 3.021 GB | 3.006 GB |

The second case's first application includes about 14 s of one-time owner
initialization; its warm call takes 29--33 ms. Subsequent calls to that owner
do not repeat initialization, including after coefficient-cache clearing.
This is one sequential comparison on CPU 24 versus CPUs 32--39 while other
host workloads were active, not an optimal-scaling or confidence-interval
claim. Evidence: `runtime-lazy-one-cpu/` and `runtime-lazy-eight/`.
The earlier `runtime-lazy-one/` attempt failed supervisor affinity admission
before starting a child and is not a timing result.

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

The existing native positive-support factorization service independently
confirms 14 numerator-bearing keys with factorized denominator support, besides
the 38 scalar products. This is not yet a numerator-factorization identity.
Evidence: `native-positive-supports/`, 160.331 s including cold loading and
7.731 GB sampled peak RSS. The diagnostic links the frozen native libraries;
no independent Python linear-algebra calculation is used as authority.

## Subsequent integration evidence

The generated census now reconciles the complete reference list: 166 connected
scalar skeletons produce 7,548 loop-preserving contraction representatives,
101 native incidence classes and, after exact momentum matching, all 67 integral
classes including 19 factorized classes. These counts use the explicitly
recorded scalar generator settings, not QCD numerator filters. Zero-flow
bridges are handled separately at ingress as scalar propagator factors.

The latest retained Vakint library executable passes 101 tests, with two
explicitly ignored tests. Evidence:
`TMP/vakint-five-loop-20261013/release-final-native-tests.log`;
executable `TMP/five-loop-vakint-20261013/vakint-101-native-tests`, SHA256
`7d0eca42782f6b6a72540f91395b70941d6ea16bea52fb89aa8d224c9edfb39d`.
This snapshot predates the subsequent dummy-index bridge repair and zero-flow
bridge extraction, which require their own regression runs.

Deferred owner preparation reduces a later real Vakint cold load to 72.890 s,
with zero of 67 owner programs prepared at load time. The first scalar witness
prepares one owner in 2.868 ms. Route checks remain eager. This receipt is not
a matched comparison against the root executable's earlier 154 s boundary;
it includes Vakint product/chart preparation and uses a different executable.
Evidence: `release-admission-suite.log`, with retained example SHA256
`212f130e7933ee5738303dd24b5da55e88c15be4b75354a626864597129e0b7e`.

The real GeV^10 denominator-cancellation witness passes in its initial and
expanded presentations. Its alternate-LMB failure has been traced to loss of
dummy-index scopes: expansion precedes restoration of scalar-product notation,
and a squared cross contraction is consequently interpreted as a product of
self-contractions. The denominator routing witness itself is correct. The fix
uses the existing FeynKit scalar bridge and must not broaden the recorded D=9
admission predicate. Full alternate-LMB success is not yet claimed.

All 52 factorized terminal checks now pass: 38 positive products spanning all
19 factorized support classes, plus 14 numerator-bearing product charts,
including the three cross-component projections. Each result has exact
first/warm coefficient and guard equality and contains only qualified lower
families. Existing native scaleless-sector analysis handles free-loop factors;
scoped batch memoization eliminates repeated four-loop subreductions between
transport endpoints. The previous one-million-rule failure no longer occurs,
without increasing the default limits or generating any rule.

The full test took 214.91 s wall / 207.34 s user + 5.87 s system CPU, including
cold package loading, with peak RSS 4,186,928 KiB (3.99 GiB), on CPU 0.
Evidence: `release-all-factor-products-batch.log` and `.time`; retained
executable `vakint-scope-batch-native-tests`, SHA256
`78dda97b5573ea3206e3ca19cd73ea3ff26e7672f96d7e1ffbe591a10b510810`.
This is a completed test versus an earlier failed test, not a matched speedup
ratio. Nor is warm equality an independent oracle for all 52 products;
independent lower-factor and projector controls are tested separately.

The outer configurable product budget is intersected with each provider's
existing policy. In particular, shipped four-loop candidate sessions retain
their original 1 GiB / 16-million-coefficient-term native cache ceiling. Raising
the outer product allowance alone does not raise that inner ceiling; all 52
controls pass within it. Native cache allowance spans a transported batch,
while each guarded endpoint also respects its existing per-call work ceiling.

The external-artifact setting and loader passed an independent source audit:
an unset path rejects five-loop use before consulting a warmed cache; explicit
loading changes settings only after success; package identity includes the
canonical path and full preparation/product options. There is no five-loop
embedded data or automatic download. Same-path loaded bytes remain immutable
until an explicit reload. Runtime tests remain part of the acceptance gate.

## Outstanding acceptance work

The following matrix distinguishes graph registration, recorded scope and
successful application. A passing census is not an application-coverage claim.
"Pending" includes implemented changes whose final executable has not passed
the corresponding test yet.

| Requirement | Current evidence | Status |
|---|---|---|
| External package; no embedded/download fallback | Local 281 MB package; manifest digest above; loader/setting source audit; path-absent and wrong-path tests | Implemented; warmed-setting checks included in loaded examples |
| 67 classes, 19 factorized classes, native DOT/pinches | Generated contraction census, all registered DOT round trips and the 118-test native suite pinned to RustRed `bf5b2882` | Passed on that snapshot |
| Exact recorded admission, including holes | 804-record inventory, 41 native-empty records; 763 nonempty domains; package/admission tests | Passed |
| Admitted representatives and feasible boundaries | 813 deduplicated concrete witnesses; normalized frontend-only census: 811 pass, two native routing-work refusals | Scalar-expansion failures repaired; explicit routing-budget retest pending; full IBP execution remains separate and incomplete |
| Exact replay of all retained finite terminal maps on export | All 939 source keys verified against the pinned publication; 905 remaining keys | Passed |
| Portable authenticated loading and lazy owners | 30 release application/loader/route-preparation tests, including corruption, moved source, first-use error and threaded route checks | Passed |
| Coefficient application, guard retention, cycles/errors and cache identity | 586 related release core regressions; exact public 4 MiB LRU pressure comparison | Passed; broader real-input sweep still in progress |
| Simultaneous numerator/LMB and orientation invariance | Loaded primary suite: scalar-degree 0/5/11 controls in original, expanded, standard/detailed DOT and alternate/indexed LMB forms | Passed at `bf5b2882`; not an all-domain invariance proof |
| Positive denominator cancellation versus explicit pinch | Six positive-propagator comparisons including single, dotted, multiple, mixed powers and nonunit masses | Passed in the loaded primary suite |
| Homogeneous GeV^10 and higher admitted numerator boundaries | Homogeneous degree-10 factorized control; scalar-degree 5/11 inverse-denominator examples | Passed for the named controls; a separate full-parent degree-10 request was outside the pinned union |
| Factorized symbolic output and cross contractions | Independent 5×1L and 3L×2L comparisons; all 38 positive and 14 numerator-terminal reductions, including three cross projections | 52-terminal sweep passed after batch reuse |
| General common mass and zero-flow bridges | Nonunit-mass controls and three constant massive-bridge cases in the loaded primary suite | Passed |
| No FORM dependency and preserved lower-loop behavior | Invalid FORM setting in acceptance harness; explicit comparison to baseline `6203c6cbb` | New gates pass; four pre-existing tensor/scalar integration failures reproduced on both baseline and current source, not counted as passes |
| Rust and Python/HEPKit DOT examples | Example sources and Python-feature compile checks | Final runtime examples pending |
| Cold, first/warm, RSS, output and cache measurements | Native JSONL case receipts and matched measurements above | In progress; shared-cache first application is not a per-case cold process |
| Source-only delivery | RustRed through `bf5b2882` pushed, including guarded LRU; runtime package excluded | Compact weighted lowering and Vakint push pending |

- Complete admitted inventory reductions, including higher-rank cases, with
  resource outcomes reported individually.
- Repeat the affected loaded comparisons after the compact weighted-lowering
  change; baseline-pinned LMB/cancellation and factor-product gates already pass.
- Preserve the recorded baseline failures rather than weakening assertions or
  describing the entire lower-loop suite as green.
- Report matching, tensor, scalar application, terminal normalization and
  rendering times separately in the final Vakint acceptance table.
- Finish Python/HEPKit runtime smoke testing with one coherent native host;
  Python-feature compilation alone is not an end-to-end Python check.

### Whole-numerator lowering gate

The completed frontend census took 2,919.213 s with 2.994 GB peak RSS. Its 84
failures at scalar rank 6 and above were intermediate expression/work limits,
not out-of-union keys or missing rules. The redesigned weighted-only path uses
Symbolica's simultaneous literal replacement to change the complete numerator
into denominator coordinates before native polynomial collection. Original
rational poles are retained before any cancellation. Native `collect_factors`
then precedes rational support estimation. Legacy unweighted lowering is not
changed, and resource limits are not increased.

The integrated optimized rerun uses executable SHA256
`57583ef3ad43bc822f8d2b92d9a722ef4040384240f4485a6f25656fc5229c0e`,
evidence `vakint-admission-census-813-whole-weighted/`. It finishes with 737 passes
and 76 resource refusals in 774.46 s native. Case-by-case comparison finds 46
previous failures fixed and 38 newly refused cases: this is not an accepted
upgrade, despite the lower total failure count. The newly exposed conservative
denominator-support estimates are being corrected and retested without treating
resource refusals as mathematical noncoverage. Core release regressions already
pass. A separate physical-ladder diagnostic
replays the prepared numerator only to check lowering/admission; the full slow
acceptance test must still regenerate its QCD numerator and projector itself.
The earlier full preparation succeeded in 1,288.550 s, then failed a conservative
rational support bound before reduction. That failed run is not an accepted
physical ladder reduction.

The subsequent numeric-only normalization repair uses native Symbolica
`expand_num` inside the coordinate-change path without expanding outer powers.
Its tight one-term-budget rank-11 regression and 527 related core tests pass.
The complete external census now passes 811 of 813 inputs in 1,059.22 s including
loading (1,020.31 s for the audit itself), with 2.663 GB peak RSS. All previous
scalar-expansion failures are removed; the two remaining failures are
`point-00352` and `point-00356`, whose native entry transports request
161,313,306 and 386,902,320 polynomial-operation units against the existing
64-million allowance. These are operational refusals, not missing-rule or
outside-scope evidence. A separately logged run with an explicitly configured
larger routing budget is pending. No IBPs were applied by this audit.

Frozen integrated executable SHA256:
`f012756286c1a416b965c104cd149791ce4b8b974be5654a1353e2eb71d592f0`;
evidence: `vakint-admission-census-813-numeric-normalized/`.

The exported package inherits the saved scope's authority. It does not claim
independent certification, minimality, or unrestricted five-loop closure.
