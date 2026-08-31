# Six-loop closure scaling audit

## Status and verdict

This note is independent design guidance for the eventual unit-mass six-loop vacuum campaign. It
is **not closure evidence**, an artifact claim, or authorization to begin Stage 2 production. The
exact promotion requirements in [`GOAL.md`](../../GOAL.md) and the execution sequence in
[`six_loop_execution_runbook_2026.md`](six_loop_execution_runbook_2026.md) remain authoritative.

The audited proposal combines:

```text
exact uncovered complement
    -> counterexample-guided upward walk
    -> positive-margin simplex probes
    -> modular support/source-circuit discovery
    -> exact Symbolica reconstruction and ordinary-source replay
    -> bulk, actual residual faces, then rays
    -> exact finite complement, with nonminimal terminals allowed
```

The proof architecture is sound as a semidecision procedure when exploration is fair and only exact
cover deltas can establish progress. Its current literal execution strategy -- a complete dense
simplex and a fresh scheduler for every point -- is not a credible `K = 21` production algorithm.
`K = 6` is a plausible closure target now; `K = 10` and `K = 15` must provide measured promotion
or kill gates before a six-loop campaign.

## Evidence and authority boundary

For an `L`-loop vacuum family,

```text
K = L(L + 1) / 2
q = L^2
```

are respectively the complete scalar-product dimension and the number of ordinary momentum-space
IBP sources. Thus `K = 6, 10, 15, 21` correspond to three through six loops.

Only all of the following can authorize a closing artifact:

1. every admitted relation replays exactly into regenerated ordinary IBP sources;
2. coefficient guards, sectors, symmetry routes, and lower-sector maps are exact;
3. every canonicalized child is strictly lower in one common well-founded artifact ordering;
4. exact cover subtraction leaves only owned cells and an explicitly enumerated finite terminal
   complement;
5. every retained terminal has an affordable, non-circular evaluation plan; and
6. epsilon-pole debt is globally bounded for the supported all-rank domain.

Modular rank, support stability, a completed schedule, a finite tested shell, path agreement, or
numerical parity are useful evidence only. Adding misses as masters is valid only after the exact
remaining complement is proved finite; an uncovered ray or positive-dimensional box cannot be a
terminal.

The current implementation respects this boundary:

- `interior_simplex` creates immutable proposal tasks and has no closure authority;
- its executor is serial and retains compact telemetry rather than epoch-bound circuits;
- `interior_replay` extracts exact relative source and residual support shapes, but deliberately
  omits coefficient content and retains only guard cardinalities; equality is therefore neither
  exact-rule equality nor admission evidence; and
- a scheduler success still needs canonical replay, owner compilation, exact cover delta, and the
  ordinary closure coordinator before it can affect closure.

## Dense-simplex and translated-source envelope

A complete positive simplex in `f` free directions through total degree `p` contains

```text
S(f, p) = binomial(f + p, p)
```

targets. The table uses the conservative `f = K` envelope and shows `targets / q * targets` ordinary
translated-source rows for one selected box.

| `K/L/q` | degree 2 | degree 3 | degree 4 | degree 5 |
| --- | ---: | ---: | ---: | ---: |
| `6/3/9` | `28 / 252` | `84 / 756` | `210 / 1,890` | `462 / 4,158` |
| `10/4/16` | `66 / 1,056` | `286 / 4,576` | `1,001 / 16,016` | `3,003 / 48,048` |
| `15/5/25` | `136 / 3,400` | `816 / 20,400` | `3,876 / 96,900` | `15,504 / 387,600` |
| `21/6/36` | `253 / 9,108` | `2,024 / 72,864` | `12,650 / 455,400` | `65,780 / 2,368,080` |

At `K = 21`, degrees six through eight contain `296,010`, `1,184,040`, and `4,292,145`
targets. The current executor admits at most `4,096` tasks by default, so one full-dimensional K21
box fits through degree three but not degree four; several maximal boxes lower that ceiling.

For one box, retaining only the simplex offsets, lattice targets, and target shifts has an
approximately `24 K S(K,p)`-byte coordinate floor. At K21 this is about `6 MiB`, `32 MiB`,
`142 MiB`, `569 MiB`, and `2.0 GiB` for degrees four through eight, before keys, allocations,
probes, reports, matrices, and Symbolica coefficients.

Matrix storage is the more immediate risk. Under the explicitly illustrative assumption of 50
nonzeros per row, K21 degree-three and degree-four inputs contain about `3.64` and `22.77` million
entries. At 12--16 bytes per input entry, that is roughly `44--58 MiB` and `273--364 MiB` before
row pointers, duplicate primes, provenance, scratch, or elimination fill. Tenfold degree-four fill
is already a multi-gigabyte workload.

A blind signed neighborhood is worse. A signed `L1` ball has

```text
D_K(h) = sum(j = 0..min(K,h), 2^j binomial(K,j) binomial(h,j)).
```

At K21, radius three contains `13,287` shifts and `478,332` ordinary rows; radius four contains
`5,167,044` ordinary rows. Signed exploration must therefore remain obstruction- and
incidence-directed, with a fair bounded fallback rather than global materialization.

Measured K6 evidence also warns against per-target rebuilding. One eight-epoch dependency-sector
run scanned `63,573` candidate rows and `645,012` exact source terms without finding a hit. The
simplex executor currently builds zero-source incidence for bootstrap and each nested scheduler
rebuilds the same zero-source batch and incidence. Repeating even the measured K6 visit count over
the `2,024` K21 degree-three targets would exceed `1.3` billion source-term visits. This is an
illustration, not a K21 cost prediction.

These envelopes exclude sectors, topology parents, guard strata, and faces. A single K21 coordinate
family is also not a universal six-loop topology manifest: matcher-derived multi-parent families
and exact routing witnesses remain mandatory. Naively enumerating all coordinate faces costs
`2^21`; even one 15-propagator parent has `2^15` pinch masks before symmetry. Exact complement
refinement must expose only the faces that remain live.

## The interpolation trap

`S(f,p)` is unisolvent only for a polynomial with a known total-degree bound. Parametric reduction
coefficients are rational in the indices and dimension, and their denominator polynomials define
applicability guards. After clearing denominators, determinant bounds derived from a selected
circuit exist but are generally unusable: at `f = 21`, dense degree-ten interpolation already
needs

```text
binomial(31, 10) = 44,352,165
```

samples. Stable support at successive degrees and held-out modular points do not prove that a
higher-degree term or a new guard is absent. A positive target simplex also does not bound the
signed translated-source offsets selected by inverse incidence.

Complete simplexes are therefore valuable controls, falsifiers, and bounded fallbacks. They should
not be the default mechanism for reconstructing a K21 rule.

## Recommended generic-small-circuit architecture

The preferred path uses sampled points to discover structure and Symbolica to derive the actual
generic rule:

1. Select a target from the exact maximal uncovered component and build a shared relative-source
   skeleton from regenerated ordinary sources.
2. Across several primes and index points, discover a small, translation-stable source circuit or
   obstruction block rather than a complete dense rule coefficient table.
3. Reassemble only that small circuit at generic symbolic indices. Ordinary translated IBP entries
   are affine in the index variables before elimination.
4. Use Symbolica's exact rational-polynomial and sparse-linear-algebra primitives to solve the small
   symbolic circuit directly. This recovers rational coefficients and guard polynomials without a
   global dense interpolation degree.
5. Replay the result against regenerated ordinary sources, compile it under one production
   ordering, and require exact strict descent.
6. Subtract only the admitted owner's exact domain from the complement. Schedule the guard or
   coordinate faces actually exposed by that subtraction, followed by any surviving rays.
7. If support changes, exact replay fails, or no cover delta results, enlarge the circuit,
   obstruction block, source radius, free-axis set, or degree under a deterministic fair schedule.
   A complete dense design remains the final bounded fallback for each declared degree.

This turns a potentially huge interpolation problem into a series of small exact symbolic solves.
It remains a proposal until K6 measurements establish stable circuit sizes, lift rates, guard
growth, and cover shrink.

## Execution safeguards

- Share the immutable completed source batch, incidence index, relative CSR pattern, graph routes,
  and coefficient context across a block of targets and primes. Workers should own only field
  values, obstruction state, and bounded caches.
- Batch nearby targets into one modular frame or obstruction basis. Do not construct an independent
  scheduler, incidence index, and physical frame for every simplex point.
- Stream compact negative telemetry; retain or replay only promoted circuits. Exact replay should
  have a lower concurrency cap than modular discovery because Symbolica scratch growth is not
  covered by RustRed's logical-cell budgets.
- Require deterministic multi-prime support and pivot consensus with bad-prime detection. Modular
  evidence may nominate a rule but never admit it.
- Sparse or anisotropic interpolation may begin with axes selected by obstruction incidence, but a
  fair deterministic schedule must eventually add every free axis and declared degree. Heuristic
  pruning cannot establish failure.
- An ordering portfolio may propose relations. Every shipped rule must be reoriented and certified
  under one common artifact ordering; rules from incompatible descent orders cannot be mixed
  directly.
- Exact graph symmetry may canonicalize whole decorated tasks and transport certified rules. It may
  not discard independent source directions. In the maximally symmetric S7 proxy, one source
  template produces 42 orbit images of exact rank 36; formula storage compression is not row-rank
  compression.
- Bulk degree does not bound boundary degree. Published seedless examples require level two on a
  double-box boundary and levels three or four on pentabox boundaries-of-boundaries. Every live
  face, guard intersection, and ray needs its own measured budget.
- A finite nonminimal terminal set records at least terminal count `t`, independent quotient rank
  `r`, evaluator-system size `m`, and maximum epsilon-pole debt. Reject without a new compression
  or evaluation method when `t > 1,000`, `t/r > 10`, `m > 300`, evaluation is circular, or pole
  debt is unbounded.

## Promotion ladder

| gate | Feasible work now | Required evidence | Prohibited inference |
| --- | --- | --- | --- |
| K6 | Complete degree `0..3` campaigns, cautiously `4..5`; all five registered graph classes; every live guard/face/ray | Exact source replay, deletion controls, strict descent, zero uncovered positive-dimensional components, deterministic artifact, measured circuit/lift/fill data | A modular hit or stable simplex support is closure |
| K10 | Matcher-derived multi-parent manifest including nonplanar K3,3, a dense planar control, and banana; complete modular degree `<=3`; selected exact lifts | Rows, nonzeros, fill, peak RAM, source radius, guard count, exact certificate size, terminal `t/r`, worker scaling | One symmetric parent predicts all four-loop families |
| K15 | Source/operator generation and complete degrees `1..2`; degree three only after projected fill and memory gates | Compact exact provenance, stable physical-stratum rank, bounded boundary growth, affordable terminal/evaluator estimates | A successful K6 implementation justifies a closure campaign |
| K21 | Source-span and symmetry controls, one or a few bulk-box degree `<=3` dry runs, shared-matrix bandwidth/fill studies | Successful K10 and K15 gates plus a frozen multi-parent manifest and credible resource model | Artifact production, global simplex execution, or a six-loop closure claim |

## Status of candidate accelerators

- **Generating-function descendants** provide a strong way to nominate missing operator directions,
  but published examples stop at sunset and two-loop double boxes. Their degree cutoff is tunable,
  and selected path consistency when the master count is unknown is not RustRed's exact-cover proof.
- **Seedless lowering operators** strongly support the bulk/face/ray organization, but published
  examples are two-loop double box and pentabox; higher boundary levels are needed and some
  propagator-lowering and optimization questions remain open.
- **Syzygy-constrained and Baikov logarithmic sources** can remove artificial dots and reshape
  source systems, but current high-rank demonstrations are two-loop. They are source
  preconditioners, not permission to drop ordinary-source provenance.
- **Black-box Krylov, Wiedemann, and FGLM** can reduce fill after a finite operator/frame is known.
  They neither prove finiteness nor expose every exceptional guard and must retain recoverable
  source cofactors.
- **Graph-orbit compression** can reduce repeated task discovery and split equivariant blocks. It
  does not generally reduce the mathematical `L^2` source rank, especially after decorations and
  guard specialization reduce stabilizers.
- **SpideR's finite-field bottom-up reducer** is strong evidence that sparse modular rule application
  can traverse a graph of order `10^8` integrals efficiently. Its publication does not disclose a
  systematic six-loop rule-generation or exact guarded closure algorithm.

## Symbolica boundary

The vendored public Symbolica `2.2.0` API was checked before proposing new algebra. It provides
rational and polynomial domains, native exact division, `Zp64`, `SparseRowReducer`, dense solves,
CRT and rational reconstruction, univariate Newton interpolation, F4 polynomial Gröbner bases, and
Graphica canonical labeling. RustRed should delegate those operations.

No public general free-module syzygy or module-intersection API, Wiedemann/block-Krylov controller,
or Scalar-FGLM implementation was found. RustRed would have to own those semantics, scheduling,
provenance, and resource policy if a measured gate justifies them, while continuing to use
Symbolica for arithmetic and polynomial algebra.

## Primary sources

- R. N. Lee, [*Presenting LiteRed*](https://arxiv.org/abs/1212.2685): the symbolic search is
  heuristic, ordering-sensitive, and not proved to terminate.
- R. N. Lee, [*LiteRed 1.4*](https://arxiv.org/abs/1310.1145): solve generic rules, derive uncovered
  applicability alternatives, then search neighboring fixed-index points and translate successful
  relations. This is the closest published form of the upward-walk clue.
- B. Feng et al., [*An Algorithm for the Symbolic Reduction of Multi-loop Feynman Integrals via
  Generating Functions*](https://arxiv.org/abs/2605.09541): iterative descendants, rule extraction,
  lattice coverage, and tunable degree cutoffs.
- L. de la Cruz and D. A. Kosower,
  [*Seedless Reduction of Feynman Integrals*](https://arxiv.org/abs/2602.22111): generic lowering
  operators and explicit bulk/boundary/boundary-of-boundary construction.
- S. Smith and M. Zeng,
  [*Feynman Integral Reduction using Syzygy-Constrained Symbolic Reduction Rules*](https://arxiv.org/abs/2507.11140):
  sector syzygies, operator reshuffling, and small symbolic neighborhood solves.
- C. Dlapa et al.,
  [*Nonlocal-in-time tail effects in gravitational scattering*](https://arxiv.org/abs/2604.25916):
  the SpideR symbolic-rule and finite-field sparse back-substitution application.

## Decision

The present architecture should continue toward K6, but dense simplex sampling must be treated as a
bounded control rather than the eventual K21 engine. The preferred scaling hypothesis is:

```text
exact complement
    + modular obstruction-guided small circuits
    + generic exact Symbolica solve and replay
    + refinement only on actual residual strata
    + finite, affordable nonminimal terminals
```

That hypothesis becomes credible for six loops only after it survives the measured K10 and K15
gates. Until then, K21 results are proposal telemetry, not closure evidence.
