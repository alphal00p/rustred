# Exact numerator-symmetry experiments at three and four loops

## Result

The new generic numerator-polynomial symmetry service was tested against the
complete packaged three-loop K6 program and all four packaged four-loop parent
programs, using existing rules only. No parametric IBPs were regenerated and
every finite ordinary-source cursor stayed at zero.

The three-loop five-element normalized basis does not shrink. At four loops,
rank-one numerator probes recover one exact dependency in each parent
presentation. These four dependencies are copies of **one physical relation**,
not four independent master eliminations. The common exact full-U convention
changes from 23 to 22 outputs. Rank-two probes find nothing further.

This does **not** improve on the previously reported 20-output result that also
used finite ordinary/diagonal IBP refinement. That larger refinement was not
rerun here, and its old local experiment directory was not present after the
workspace relocation. The present experiment isolates symmetry-derived rows
and demonstrates a generation-free route to an already familiar dependency.

## Frozen inputs and comparison boundary

The actual source is the current local Vakint integration's
`crates/vakint/data/rustred/` directory. K6 supplies all 38 raw declared terminal
keys. H, FG, BMW and X supply all 1,155 raw keys and their independently replayed
native normalization sidecars, giving 74 family-local positive outputs.
Numerical terminal-value catalogs were not read or copied.

Inputs were copied/decompressed into a new evidence directory and SHA256-pinned
before execution. All 34 source/copy checks still agree after both runs. No
production campaign or packaged Vakint asset was modified.

The experiment uses three finite input inventories per family:

1. Every original raw terminal key.
2. The raw keys plus all extra negative-index degree-one monomials on inactive
   coordinates around each published output support.
3. The same construction through extra negative-index degree two.

The provider verifies each denominator-preserving momentum transformation and
transports the complete numerator, including affine mass constants. Every full
equation is then reduced with the original sealed/candidate program and, for
four loops, the original terminal sidecar. Repeated terms are coalesced with
native exact Symbolica coefficients. The existing `TerminalRelationSession`
eliminates only those supplied rows and the already-established package maps;
it never advances an ordinary IBP source.

The before and after sessions use the same raw keys, normalization and integral
ordering. Every old raw map composed through the new map agrees exactly with
the new result. Inherited guards remain present; outputs contain remaining
terminals only. Provider witnesses and final session maps are cold-imported and
checked independently. No sampled or numerical zero tests are used.

The numerator witness certifies the integration-variable identity. A terminal
equation obtained by applying a saved candidate program also inherits that
program's existing authority. This experiment does not upgrade uncertified
candidate source provenance or establish new arbitrary-index family closure.

## Counts

| Input | Raw keys | Existing normalized basis | Raw-key symmetry only | With degree-one probes | With degree-two probes |
| --- | ---: | ---: | ---: | ---: | ---: |
| K6 | 38 | 5 | 5 | 5 | 5 |
| H | 386 | 22 | 22 | 21 | 21 |
| FG | 145 | 16 | 16 | 15 | 15 |
| BMW | 179 | 17 | 17 | 16 | 16 |
| X | 445 | 19 | 19 | 18 | 18 |
| Four-loop family-local sum | 1,155 | 74 | 74 | 70 | 70 |
| Four-loop exact cross-family U convention | — | 23 | 23 | 22 | 22 |

The family-local sum is not an independent-master count. The combined row uses
the existing exact `VacuumFamilyAliasPlan` on the remaining positive keys,
identically before and after the symmetry step. It adds no diagonal IBP rows.

| Input | Degree-one inputs / witnessed equations | Degree-two inputs / witnessed equations |
| --- | ---: | ---: |
| K6 | 122 / 306 | 269 / 1,182 |
| H | 458 / 361 | 649 / 1,244 |
| FG | 204 / 260 | 373 / 1,222 |
| BMW | 240 / 329 | 412 / 1,303 |
| X | 509 / 461 | 685 / 1,378 |

Every equation term was successfully reduced by the supplied old rules; there
were no unsupported-rule failures. Some nonzero rows in raw parent coordinates
become zero under the baseline normalization. Such rows are not counted as new
dependencies.

## The recovered relation

In H coordinates, the eliminated output is

```text
[0,2,0,0,1,1,1,0,1,0]
    = (2*d-5)/5 * [0,1,0,0,1,1,1,0,1,0]       (m^2 = 1).
```

The corresponding dotted five-line banana occurs once in each parent. Its
mass-restored form is

\[
I(2,1,1,1,1) = \frac{2d-5}{5m^2} I(1,1,1,1,1).
\]

This is consistent with homogeneity of a four-loop five-line scalar integral
and equal-mass line symmetry. For example, a complete transformed numerator
at H key `[-1,2,0,0,1,1,1,0,1,0]`, reduced with the existing program, gives
`(5-2*d)*I_scalar + 5*I_dotted = 0`. The native witness retains the actual
momentum map, complete source equation and conservative application guards.
No banana-specific relation was hard-coded into the engine or experiment.

## Release measurements and reproducibility

The helper was compiled with `opt-level=3`, ThinLTO and the frozen optimized
core/app libraries. Compilation is excluded. Each case is a fresh process on
one physical CPU with one Rayon worker; the four families ran concurrently on
disjoint CPUs 240–243. This is a shared-host reproducibility check, not a speedup
comparison or a cold-filesystem benchmark.

The full-process boundary includes package loading, all three finite symmetry
inventories, exact rule application, native certificate writing and cold replay,
all-map comparisons, and cold final-session verification.

| Input | Wall seconds, two runs | CPU seconds, two runs | Peak RSS MiB, maximum |
| --- | ---: | ---: | ---: |
| K6 | 4.47 / 4.66 | 4.42 / 4.60 | 277.1 |
| H | 24.36 / 24.37 | 24.17 / 24.15 | 1,341.4 |
| FG | 22.61 / 22.77 | 22.44 / 22.58 | 866.1 |
| BMW | 30.08 / 29.82 | 29.85 / 29.58 | 1,236.0 |
| X | 46.17 / 46.16 | 45.82 / 45.80 | 2,537.1 |

First-run package loading was 0.730 s for K6 and 4.192 / 1.802 / 2.876 / 8.408 s
for H / FG / BMW / X. The degree-one stage alone took 0.917 / 4.647 / 4.024 /
4.946 / 8.259 s respectively, including its certificate replay and exact-map
checks. Combined cross-family post-processing took 3.53 / 3.60 s.

Both runs produce identical counts, exact displayed coefficient maps, and
byte-identical native baseline sessions, provider certificates and final
sessions. These checks cover every original key, not selected canaries.

Measured helper SHA256:
`961461fa4ef76772a1d11b79113e4a0cbb07e5b95a092dd11768911eee69bdb1`.

## Integration limitation and recommendation

The initial production assistance flag prepares the raw-plus-normalized retained
inventory only. It does **not** automatically generate the extra degree-one or
degree-two probe inputs used in this study. Thus the direct flag must not be
advertised as already obtaining 74→70 on this package: direct raw-key-only
discovery produced zero new dependencies.

The measured result motivates a separately specified finite numerator-probe
inventory, or reuse of the explicitly selected finite source seeds as symmetry
inputs. That is a search-policy extension, not new algebra. Discovery remains
heuristic; no minimality or exhaustive symmetry-group claim is made.

The rank-two no-gain result is retained: increasing polynomial degree costs
additional work and is not automatically useful. Neither these experiments nor
P102's separate regression imply that every nonminimal retained key is removed.

## Reproduction and local proof evidence

Source helpers live under
`tools/research/numerator_symmetry_lower_loops/`. `prepare.py` pins and copies
only rule/normalization/descriptor inputs. The release `study` helper accepts
`k6`, `h`, `fg`, `bmw`, `x`, and the final `combined` comparison. `run.py` records
exact commands, binary identity, CPU placement and GNU-time receipts.
`summarize.py` checks repeat determinism and all original source hashes.

Local, deliberately untracked evidence:

```text
TMP/lower-loop-numerator-symmetry-evidence-20261021/
  inputs/manifest.json
  reproducibility.json
  run1/receipts.json
  run2/receipts.json
  run{1,2}/{k6,h,fg,bmw,x}/
    baseline.rrbin
    rank-{0,1,2}-provider.rrbin
    rank-{0,1,2}-session.rrbin
    rank-{0,1,2}-maps.json
    rank-{0,1,2}-translated.json
    rank-{0,1,2}-summary.json
  run{1,2}/combined/summary.json
```

The native provider capsules plus the pinned input programs are replayable
exact certificates. JSON coefficient strings are human-readable diagnostics,
not authority or a replacement for the native Symbolica format.
