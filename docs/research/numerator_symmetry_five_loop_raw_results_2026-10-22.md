# Five-loop raw numerator symmetries: 904 → 873 retained basis keys

## Measured result

The complete frozen five-loop retained inventory has **939 original keys** and
904 remaining basis keys after previously proved refinements. Applying the new
denominator-preserving numerator-symmetry equations directly to that finite
inventory proves **31 further dependencies**, leaving **873** basis keys.

This experiment performs **no new ordinary IBP generation and no saved-rule
application**. It is separate from the much more expensive experiment that
tries to apply the saved parametric rules to every transformed child.

Two fresh-process runs produced byte-identical final native sessions. An
independent public-core verifier checked every raw map and inherited guard.
No production campaign, publication or numerical master data was modified.

## Frozen authority and algorithm

The baseline native session is a fresh relations-only snapshot of the pinned
R≤6, D≤10 publication. Its 35 inherited terminal relations, 939 raw keys and
108,524 conservative applicability guards are retained. The new native symmetry
capsule has 939 source keys and 961 complete homogeneous equations. Each
momentum-map witness independently verifies the full weighted denominator
multiset, masses, powers, unit Jacobian and complete numerator polynomial.

The public core helper:

1. Cold-loads the baseline using its exact `conservative-v1` normalization recipe.
2. Cold-loads and replays the native symmetry certificate against that family.
3. Binds the finite provider and drains its pending keys/equations only.
4. Stops before any ordinary source step, requiring that cursor to remain zero.
5. Writes the native result, composes every old map through the result, checks
   remaining-only outputs and retention of all original conditions, then cold
   reloads the result and compares exact maps/guards.

All algebra, numerator transport, sparse elimination, coefficient arithmetic and
native serialization use the existing RustRed/Symbolica services. There is no
custom CAS, interpolation, numerical zero test or hard-coded five-loop relation.

## Exact work and interpretation

| Quantity | Before | After |
| --- | ---: | ---: |
| Original raw keys | 939 | 939 |
| Remaining terminal basis | 904 | **873** |
| Independent retained rows | 35 | 859 |
| Ordinary source rows processed | 0 | 0 |
| Provider source keys processed | 0 | 939 |
| Provider equations processed | 0 | 961 |
| Total columns | 939 | 155,633 |
| Auxiliary columns | 0 | 154,694 |
| Sparse nonzeros | 779 | 290,946 |
| Retained applicability guard records | 108,524 | 108,524 |

There are 66 changed raw maps because some original aliases feed the 31 newly
eliminated basis keys. Exact composition checks all 939 maps and 2,270
coefficient products. All source bytes remain unchanged.

The 154,694 auxiliary columns are **not closed** and are not counted as new
masters. The result is 31 proved terminal-to-terminal consequences of a finite
symmetry system; draining that provider is not exhaustion of an ordinary IBP
span, full orbit enumeration or a minimal-master certificate. No independent
master count of 873 is claimed.

P102's common-family key
`[0,0,1,-1,0,0,1,0,0,1,0,1,1,1,1]` remains an identity map in this raw-only
assay. Its analytic relation is independently established by the separate P102
regression, but routing its scalar/pinch offspring to the current retained basis
requires additional normalization/application beyond this deliberately cheap
raw-only pass. The 31 measured eliminations must not be described as already
installing P102's specific published-name identity.

## Timing and resources

The frozen optimized core library was reused. The small assay driver was built
with `opt-level=3`, ThinLTO and four codegen units; compilation is excluded.
Both invocations were pinned to CPUs 232–235, with one active exact reducer and
a caller-provided 128 GB RSS allowance. The allowance was not approached.
These are shared-host observations, not parallel scaling or speedup claims.

| Boundary | Run 1 | Run 2 |
| --- | ---: | ---: |
| Baseline/certificate cold preparation | 10.650 s | 10.199 s |
| Helper work through final exact checks | 15.382 s | 15.130 s |
| Full GNU-time process wall | 16.07 s | 15.75 s |
| CPU user + system | 15.93 s | 15.60 s |
| Peak RSS | 1,338,900 KiB | 1,337,652 KiB |

The preparation boundary includes importing the retained exact maps/guards and
replaying the complete symmetry capsule. The remaining approximately five
seconds include assisted elimination, final writing and cold verification.

An independent verifier took 3.447 s, excluding its compilation. It checked all
939 before→after compositions, 2,270 exact coefficient products, all 108,524
guards and source-byte immutability.

## Reproducibility and a preserved negative result

Baseline BLAKE3:
`83429a971cbbeaf2acc415773f79622060d758148f813bb2dd6b9d6a782d0707`.

Provider BLAKE3:
`cee6e2daf43903846aad7844308ab8d839d5d1411c3c45ed50201f0026d563c3`.

Both result sessions have SHA256:
`d7018221688a7342e0d8f0a704df1c7d5f2f9af1ea8eac28ee755f16b1504992`.

The measured assay executable has SHA256:
`924252fc0fb1ba076ebe006fb69f9b77a4ffaa09d3a65d69207cc92d1a59b713`.

An initial attempt incorrectly supplied unbounded normalization preparation
instead of the saved conservative profile. Native replay correctly rejected it
with `normalization differs from regenerated proof`, before any equation work.
That failure and its receipt are preserved. The fix was to select the original
normalization semantics, not weaken or bypass native validation. Operational
source/row/column/I/O grants remain unbounded and RAM-governed.

The portable research helper under
`tools/research/numerator_symmetry_retained_inventory/` accepts the original
normalization profile and an optional diagnostic target key explicitly, without
dispatching on a topology name or loop count. Its output remains a research
native session, not an automatic rewrite of a user's production publication.
The documented generic CLI was also compiled and executed: it produced the
same native result bytes in 15.82 s full-process wall, 15.68 CPU seconds and
1,338,932 KiB peak RSS. Its helper SHA256 is
`c38f2c6ce8f1f217e536e25173a87c379250e82e930e53be786150c0a94d51cf`.

Local evidence, deliberately not committed:

```text
TMP/numerator-symmetry-five-loop-raw-evidence-20261021/
  assay.stderr                         # initial rejected profile
  guard-receipt.json                   # initial failed attempt
  conservative-v2-result/after.rrbin
  conservative-v2-result/inputs.json
  conservative-v2-result/summary.json
  conservative-v2-repeat-result/after.rrbin
  conservative-v2-repeat-result/summary.json
  conservative-v2{,-repeat}.{stdout,stderr,time}
  generic-confirmation/receipt.json
  generic-confirmation/native/after.rrbin
  generic-confirmation/native/summary.json
TMP/numerator-symmetry-delivery-20261021/
  check_retained_maps.five-loop-raw.json
```

The exact 31 dependencies can be inspected through
`TerminalRelationSession::apply_all_terminals` on the cold native result.
Human-readable coefficient rendering is diagnostic only; native maps and
replayed witness capsules retain authority.
