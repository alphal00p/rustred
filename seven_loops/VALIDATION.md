# Seven-loop input-study validation — 2026-10-19

This is an offline topology/input preparation milestone. No seven-loop
parametric sector solve, master evaluation, application backend or closure walk
was launched. Existing production campaigns and their frozen binaries were
not modified.

## Measured graph preparation

The existing native HEPKit phi³ generator retained self loops, repeated graph
edges, tadpoles, self energies, zero-flow bridges and factorized cases. It
completed 2,592 connected seven-loop cubic multigraphs, each with 12 vertices
and 18 edges. The independent adjacency checker also counts 85 simple cubic
graphs within that raw collection.

| Step | Time on one allocated core | Result |
|---|---:|---|
| Native enumeration and raw DOT export | 5.422 s | 2,592 graphs |
| Factorization, compression, exact alias checking | 4.345 s | 2,227 factorized; 365 nonfactorized, in 359 classes |
| Constructive maximal-parent cover | 222.984 s | all 359 classes covered by 57 maximal parents |
| Quotient contraction traversal | 95.718 s | 4,401 classes from 62,015 candidate subsets |
| Independent input/coverage audit | 136.854 s | PASS |

These are input-preparation times under concurrent host workloads, **not IBP
generation performance**. They do not include compilation or all family-export
overhead and are not a controlled benchmark comparison.

## Contraction census and coverage authority

All 4,401 full-loop contraction classes have explicit parent/subset
representatives: 3,885 nonfactorized and 516 products. Their line-count census is:

| Propagators | 18 | 17 | 16 | 15 | 14 | 13 | 12 | 11 | 10 | 9 | 8 | 7 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Nonfactorized | 57 | 219 | 592 | 911 | 953 | 663 | 336 | 116 | 31 | 6 | 1 | 0 |
| Factorized | 0 | 0 | 14 | 39 | 89 | 120 | 114 | 77 | 41 | 16 | 5 | 1 |

The generic `unique_contractions.py` algorithm starts with every maximal
parent and contracts every possible next edge of each newly discovered matroid
class. Matroid isomorphisms preserve the set of contraction classes, so repeated
representatives need not be expanded again. It uses existing native Symbolica
graph canonicalization and the existing offline finite cycle/subset helpers;
no custom polynomial, matrix, or rational-reconstruction implementation was
introduced.

The census was not obtained by sampling or a depth cutoff. The finite queue was
exhausted after expanding all 4,401 classes. It deliberately records no labelled
occurrence multiplicities: those would require counting the full 14,942,208
parent/subset pairs or a separate multiplicity algorithm.

The independent audit does **not** import the producer's binary cycle-space or
canonicalization helpers. It enumerates graph cycles from adjacency and checks:

- momentum conservation, cubic/loop counts and fundamental integer routings;
- all 365 nonfactorized raw-graph alias maps and 359 parent embeddings;
- exact integer matrices of determinant ±1 and every signed momentum equality;
- native rank 28 for every exported scalar-product basis;
- each of the 4,401 representative forest contractions and factorization labels;
- inclusion of every maximal parent and every next-edge contraction class,
  establishing the finite graph-class cover by induction;
- TOML/DOT momentum provenance and rejection of four corrupted routing witnesses.

This establishes graph/input coverage within the parent contraction scope, not
recursive IBP reduction closure. Individual momentum maps for every duplicate
labelled subset are not constructed or installed in an application backend.

## Differential and interface tests

- Quotient traversal matches **every canonical class** and both line-count
  histograms in the existing exhaustive four-, five- and six-loop censuses:
  16, 67 and 435 classes. Reordered/nontrailing physical coordinates preserve
  the census. Removing a descendant while repairing its summary counts is
  still rejected by the independent next-edge coverage check. Three tests pass.
- Five seven-loop input/steering tests pass, including 57 portable K28 bases,
  exact cubic cover and exactly one selected generation job for every one of
  the 4,401 classes.
- All 64 shared steering, supervisor, pipeline, dashboard and native-interface
  tests pass with the new executable, including seven real-native/PTY tests.
  The historical six-loop manifest remains readable; restart instructions from
  the seven-loop adapter retain the correct adapter path.
- Existing six-loop independent audit still passes unchanged data: 14 parents,
  435 contraction classes, four rejected witness mutations (1.142 s).

Reproduce focused tests from the repository root with the compatible Python
interpreter and Symbolica license in the environment:

```bash
python -B -m unittest discover -s six_loop_study/topology -p test_unique_contractions.py -v
python -B -m unittest seven_loops.test_study -v
PYTHONPATH=six_loop_study:tools/research/runtime_order \
RUSTRED_PREPARATION_VIEWER="$PWD/seven_loops/bin/rustred" \
  python -B -m unittest test_generate test_shared_generation_scheduler \
    test_generation_scheduler test_generation_monitor test_pipeline test_preparation_tui
```

The graph tests require `symbolica.community.hepkit`'s compatible Symbolica
installation. The steering tests alone need ordinary Python 3.11 or newer.

## Native executable and source validation

The separate release build enables generation arities **1, 21 and 28**:

`seven_loops/bin/rustred`

SHA256:
`f6ad7f3a896f16b7e85901152c9293cb42dc87917502bbcba827c0dcb6c72d0f`

Build wall time was 276.417 s on eight allocated cores, reusing existing
dependency artifacts. No Rust solver changes were needed. The binary is local
and ignored by Git; all source/input preparation is committed.

`tests/native_sources.py` successfully reads all 57 exported families and
derives **49 ordinary source IBPs per parent**, 2,793 identities total, in
23.907 s, including separate process startup, output and Python checking.
These are ordinary source equations, **not a closing parametric rule set**.

Metadata-only generation preparation passes with 57 parents, 4,401 selected
sector jobs, one shared 32-worker pool, a 600 GB allowance and rank zero. No
native sector generation is invoked. The generation launcher retains the
shared Ratatui dashboard, completed-sector checkpoints and the user-selected
aggregate RAM guard; no runtime/count cutoff was added.

Local untracked detailed evidence:

- `TMP/seven-loop-release-20261019/` — optimized build command/result;
- `TMP/seven-loop-native-sources-20261019/` — all source outputs and timings;
- `TMP/seven-loop-prepared-20261019/` — metadata-only preparation, not a run.

Existing one-through-six-loop application or certification dispatch is not
silently enlarged by the specialized generation build. No claim is made that
seven-loop generation will complete quickly, that R0 generation proves all R0
successors reducible, or that the finite residual basis will be minimal.
