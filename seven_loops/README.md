# Seven-loop single-mass vacuum input study

This directory prepares **inputs, not solved IBPs**. It reuses the generic
HEPKit/FeynKit/Symbolica topology scripts and the shared RustRed generation
launcher from `six_loop_study/`; there is no seven-loop solver implementation
or hard-coded seven-loop relation. Existing campaigns are untouched.

## Topology scope

The unfiltered connected scalar phi³ enumeration completed at seven loops:

| Quantity | Count |
|---|---:|
| Cubic vertices / graph edges | 12 / 18 |
| Connected cubic multigraphs, including products and zero-flow bridges | 2,592 |
| Graphs classified as factorized | 2,227 |
| Nonfactorized generated graphs | 365 |
| Nonfactorized classes after equal-momentum compression and matroid deduplication | 359 |
| Maximal eighteen-propagator parent classes | 57 |
| Full-loop contraction classes, including products | 4,401 |
| Nonfactorized / factorized contraction classes | 3,885 / 516 |
| Scalar-product coordinates per completed family | 28 |
| Physical / auxiliary coordinates per parent | 18 / 10 |

All 359 cubic-derived nonfactorized classes must have exact contraction and
signed unimodular momentum-routing witnesses to the 57 maximal parents before
export. The same generic pipeline found 2, 4 and 14 maximal parents at four,
five and six loops. For L≥2 a connected cubic vacuum graph has
`V=2(L−1)` and `E=3(L−1)`.

The **359 is not the full seven-loop contraction census**. That larger census
contains 4,401 classes. Visiting every labelled physical subset would mean
`57 × 2^18 = 14,942,208` subsets. Instead, the generic offline census contracts
each possible next edge of each distinct matroid class once. Contraction
commutes with matroid isomorphism, so expanding one representative preserves
the full class inventory. This visits 62,015 candidate subsets and takes 95.72 s
on one core here. It does not count all labelled occurrences.

The independent checker uses graph-adjacency cycles, authenticates each
representative's forest contraction, and checks that all maximal parents and
every next-edge contraction class occur in the inventory. Lower-loop controls
match every canonical class of the existing exhaustive 16-, 67-, and 435-class
censuses. The manifest selects exactly one generation sector per class,
including factorized descendants. Add `--full-downsets` to the generation
launcher to repeat all labelled physical-root downsets instead.

There is no assembled cross-parent routed application artifact or recursive
IBP-closure claim. Explicit momentum maps for every duplicate labelled
contraction are not constructed or installed.

Graphs with higher-valence vertices arise by contracting cubic expansions.
Repeated equal-mass denominators with momenta equal up to sign combine by
adding their powers. Matroid canonicalization proposes identifications; exact
integer loop maps with determinant ±1 authenticate the saved integral aliases.
Zero-flow bridges contribute mass factors and products, not zero massive
integrals. Arbitrary unrelated masses are outside this topology equivalence.

## Files

- `topology/cubic_graphs.json`, `topology/raw_dot/`: native unfiltered output.
- `topology/classified_census.json`: factorization and exact alias witnesses.
- `topology/maximal_parent_cover.json`: explicit parent/contraction witnesses.
- `topology/contraction_census.json`: all 4,401 contraction-class representatives.
- `topology/dot/`: 57 native FeynKit DOT parent diagrams.
- `topology/families/`: 57 ordinary RustRed TOMLs, with unit mass `q²−1`.
- `topology/manifest.json`: relative paths, 28-coordinate bases, explicit
  physical/auxiliary roles and DOT-edge routing maps.
- `topology/prepare.py`: generic preparation driver; no IBP work.
- `generate.py`: thin adapter to the existing shared, coloured-dashboard
  generation supervisor. Ordinary output/checkpoint binaries stay in `runs/`.
- `VALIDATION.md`: measured preparation and validation results.

The topology manifest uses the loop-independent
`rustred.vacuum-study.topologies.v1` schema. The same reader continues to accept
the historical six-loop manifest so existing prepared runs are not rewritten.

## Regenerating topology inputs

Use a compatible Symbolica installation containing `symbolica.community.hepkit`.
The local verified installation is
`TMP/community-intermediate-cache-20261014/venv/bin/python`. A normal RustRed
Nix shell alone supplies Python/build dependencies, not that HEPKit extension.

```bash
python -B seven_loops/topology/prepare.py --threads 1
```

This reruns enumeration, classification, exact parent cover, native basis
completion, the quotient contraction census, selected-sector export and the
independent witness checker. To visit every labelled subset as a slower
diagnostic rather than using the class quotient:

```bash
python -B seven_loops/topology/prepare.py --threads 1 --labelled-subsets
```

That optional exhaustive enumeration has **not** been timed to completion at
seven loops. Neither census has an implicit wall-time cutoff. Classification
still does not install cross-parent application routes or prove IBP closure.

## Compiling and preparing generation

The ordinary six-loop frozen executable has no 28-coordinate dispatch. A
separate optimized build is needed; the local delivered `seven_loops/bin/rustred`
is ignored by Git. To build from source:

```bash
nix develop
RUSTRED_RUNTIME_ARITIES=1,21,28 CARGO_TARGET_DIR=TMP/seven-loop-build \
  cargo build --release --locked -p rustred-app --bin rustred \
  --no-default-features --features native,cli,runtime-arity-selection
```

Select an actually available CPU allocation before launching. The following
is **preparation only** (no `--start`), with example CPU IDs, not a reservation:

```bash
export RUSTRED_7L_RUN="$PWD/seven_loops/runs/scalar-r0-shared"
python -B seven_loops/generate.py \
  --directory "$RUSTRED_7L_RUN" \
  --manifest seven_loops/topology/manifest.json \
  --executable seven_loops/bin/rustred \
  --rank 0 --workers 32 --cpus 96-127 \
  --execution-mode shared --max-memory-bytes 600000000000
```

For a source rebuild, substitute `TMP/seven-loop-build/release/rustred` for
`--executable`. Do not overlap CPU reservations with a running campaign.
Preparation freezes a portable copy of inputs and executable. Start explicitly:

```bash
python -B seven_loops/generate.py \
  --directory "$RUSTRED_7L_RUN" --resume --start
```

The same command resumes after interruption, reusing completed-sector shards;
an active sector restarts. Shared work stealing uses the full configured compute
pool across all ready parents and sectors, with no fixed per-parent core split.
The default parent-coordinator count is all 57. Optional `--jobs N` and
`--max-active-sectors N` control admission for memory; `all` removes the latter
cap. These controls may be changed on shared-mode resume.

There is one aggregate RAM guard, with the existing default 5% save/stop margin,
and no arbitrary wall-time, graph-count, sector-count or swap-growth cutoff.
Real allocation/IO failures, representational limits and unsupported exceptional
geometry can still stop a calculation. The sparse-factorized backend is default;
the supplied specialized build does not add reconstruction support.

`R=0` limits entry negative-index degree, not positive propagator powers.
Successful rule generation alone does **not** establish reduction of every
rank-zero input, because successors can leave the searched exceptional scope.
No seven-loop parent IBP campaign was started in preparing this directory.
