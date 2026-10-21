# Modular/reconstruction acceleration of finite master refinement

## Status

The exact parallel implementation is delivered in `4d020891`. Its frozen CLI
remains `TMP/releases/20261021-parallel-master-refinement/rustred`, SHA256
`8afc9d5ceba1124cb9077de09085f041d9e5f28be82b09163bc212e7f816580a`.
The user can resume the existing refinement with 128 workers independently of
this investigation. No modular refinement backend has been enabled, no live
campaign was signalled, and no artifact/master convention was changed here.

Finding: there is a credible larger optimization than merely parallelizing the
existing exact U updates. Discover **terminal-only consequences** over finite
fields, reconstruct their source circuits with Symbolica, and exactly verify
those circuits without constructing the entire new symbolic auxiliary basis.
This is a recommendation, not a measured end-to-end speedup.

## Evidence from this workload

The immutable R6/D10 publication has 939 terminal keys, 30 proved terminal
relations and 909 remaining keys. Its retained finite state has 51,250
independent rows and 1,188,423 nonzeros. Most exact work maintains auxiliary
integral equations, rather than directly producing new master relations.

The next 64 rows add only 30 independent rows. Exact serial execution takes
145.576/145.593 seconds; the delivered 32-worker executor takes 63.354/62.684
seconds with eight-row blocks. All resulting native checkpoints are identical.
A short live profile has substantial native polynomial division, multiplication,
GMP/integer destruction and allocator costs. These are genuine symbolic costs,
not just a queue waiting for work. See `../parallel_master_refinement.md`.

## Symbolica APIs audited

Pinned source: vendored Symbolica 3.0 development commit `ef0db494`, including
its Numerica sparse implementation. Local public APIs and existing RustRed
call sites were inspected, in addition to primary literature.

- `symbolica::poly::reconstruction::reconstruct_rational_function_over_q`
  owns finite-field interpolation, CRT/rational lifting, support reuse and
  fresh-prime checks. Methods include Automatic, HuMonagan, CuytLee and balanced
  Zippel variants. Its own documentation explicitly calls its fresh-prime check
  probabilistic: a proposal is not an exact IBP certificate.
- Native polynomial `evaluate_with_coeff_map` and `ToFiniteField` supply the
  black-box samples. `SparseRowReducer<Zp64>` supplies modular linear algebra.
- The existing generic `SemiNumerical` target materializer subsequently runs an
  exact sparse GPLU replay. Simply routing refinement through that API would
  retain a major part of the cost this optimization should remove.
- The existing `reconstruct_projected_source_weights` adapter and its
  source-weight materializer are more suitable. They reconstruct weights and
  check the **entire** exact product `W*A == r` using Symbolica sparse matrices,
  without exact GPLU in that materialization path.
- That adapter currently requires an independently ranked harder prefix and
  reconstructs every original source-weight slot, including zeros. An ordinary
  Laporta stream with many dependent inputs is not automatically eligible.
  A selected independent subframe with original-source ordinal maps is needed;
  an eligibility miss must not be reported as absence of a relation.

Relevant implementation:

- `crates/rustred-core/src/solver/discovery/semi_numerical.rs`
- `crates/rustred-core/src/solver/discovery/semi_numerical/source_weights.rs`
- `.../source_weights/projected.rs`, `probe.rs`, `cache.rs`, `validation.rs`
- `vendor/symbolica/src/poly/reconstruction.rs`, `reconstruction/rational.rs`
- `vendor/symbolica/lib/numerica/src/tensors/sparse.rs`

No new CAS, interpolation or rational-reconstruction arithmetic was implemented.

## Actual reconstruction API diagnostic

The optimized generic `refinement_reconstruction_probe` example loads the same
published native state. It selects eight distinct nonconstant terminal-rule
coefficients, sorted by numerator+denominator term count. These depend on one
variable, have degree at most nine and at most seventeen combined terms.

| Symbolica method | Coefficients exactly recovered | Total reconstruction time | Oracle probes | Maximum primes |
| --- | ---: | ---: | ---: | ---: |
| Automatic | 8/8 | 3.190 ms | 328 | 3 |
| HuMonagan | 8/8 | 1.713 ms | 537 | 3 |
| CuytLee | 8/8 | 1.214 ms | 328 | 3 |

All 24 comparisons use exact native rational-polynomial equality, not a
statistical zero test. These microtimings include reconstruction/evaluation but
not state loading or compilation. The initial Automatic call includes startup
overhead. They do not establish a meaningful universal method winner.

**Crucial limitation:** this diagnostic's oracle evaluates coefficients already
known exactly. It does not solve the IBP matrix at every sample. Therefore the
numbers prove API functionality on real saved coefficients, not a millisecond
master-refinement backend or any reduction speedup. Actual modular solve,
dependency discovery and exact source-product costs must still be measured.

Reproduction after building the example in release mode:

```bash
cargo build --release --locked -p rustred --example refinement_reconstruction_probe
target/release/examples/refinement_reconstruction_probe NATIVE_STATE.rrbin 8 16
```

The last argument is the caller's prime budget, not a solver-wide hard-coded
ceiling. The degree allowance is derived from the known coefficient for this
API diagnostic; a real unknown-function oracle needs an adaptive search policy.
Evidence: `TMP/master-refinement-parallel-20261021/coefficient-reconstruction.jsonl`
and the accompanying build/resource receipts. Evidence and native data are not
committed.

## Recommended next backend: terminal-targeted exact circuits

1. Keep the already proved terminal maps and immutable exact source definitions.
   Evaluate ordinary finite seeds over generic dimension samples in prime fields.
   Do not evaluate at `d=4`, where dimensional factors can vanish.
2. Eliminate auxiliary columns numerically and detect consequences supported
   only on the existing terminal block. Retain native L-pattern/dependency
   information for promising target relations rather than eagerly expanding
   full characteristic-zero row combinations.
3. Freeze a compact selected source frame and its ordering. Retry poles,
   unlucky samples and changed pivot traces; do not feed a piecewise pivot
   branch to a rational-function oracle.
4. Reuse Symbolica reconstruction and the existing source-weight machinery to
   lift the proposed relation/circuit. Share full matrix images across scalar
   coefficient callbacks, as the existing image cache already does. Re-solving
   the matrix separately for each scalar coefficient would waste most of the gain.
5. Verify `W*A=r` exactly over every physical column. In particular, every
   auxiliary coefficient must cancel exactly. Preserve source conditions and
   reconstructed weight poles. No sampled zero may promote an auxiliary to a
   master or authorize an omitted output coefficient.
6. Publish only exact master-to-master substitutions under the existing terminal
   ordering and guards. Checkpoint discovery/lifting separately from accepted
   exact maps. A failed reconstruction keeps previous maps and can retry or use
   the exact parallel fallback; it does not invalidate earlier proved work.

A useful variant reconstructs only source weights, then obtains `r=W*A` by
exact multiplication. It avoids reconstructing every output coefficient. A
proposed sparse weight support can also be checked by full exact multiplication;
missing a weight then causes rejection, not a false identity. The existing
strict canonical-target API should not silently lose its stronger guarantees:
any more permissive arbitrary-circuit mode needs a separately explicit contract.

This approach needs no proof of a minimal basis. Its objective is to find useful
exact relations cheaply, not to reconstruct all auxiliary U rows merely to
report a finite-search cursor. Parallelism can span independent prime/sample
images and target circuits in the same bounded worker pool. Adaptive
reconstruction can still impose serial phases, so 128-core saturation is not
promised.

## Cheaper optional screen and its caveat

A modular rank screen could select rows before exact insertion. A nonzero
specialized minor, at an admitted prime/point, witnesses generic independence.
A zero sampled minor does **not** prove characteristic-zero dependence.

For this nonminimal-master use case, discarding some candidate sources cannot
invalidate exact relations derived from the retained ones; it may miss extra
reductions. Thus an explicitly labelled heuristic screening policy is viable.
It must not masquerade as an exact exhaustive row-span/minimality certificate,
and cannot silently change the meaning of existing exact-search checkpoints.
Use an explicit search policy and preserve old exact maps. Repeated independent
samples improve discovery reliability but are not substituted for exact proof.

## Pilot gate before integration

Use the same immutable input, native normalization and terminal convention.
Include positive cohorts containing known nontrivial terminal relations as well
as no-new-relation prefixes: a method finding nothing quickly is not a win.
Measure full modular discovery + sampling + reconstruction + exact verification,
CPU, RSS, probe count, trace size and accepted relation count. Compare against
the delivered parallel exact backend, not only the slower serial baseline.

Reject the approach for a cohort if too many/high-degree source weights or a
large exact product dominate, or if the source-frame eligibility gate cannot
admit it. Preserve those negative results. Keep the exact parallel release
usable while this separate backend is evaluated. No campaign restart or new
IBP generation is required to run these refinement experiments.

## Primary references

[Kira 2.0](https://arxiv.org/pdf/2008.06494), sections 2.2, 3.1 and 4.1,
uses finite-field systems with coefficient reconstruction and discusses batched
probes and parallel evaluations. Its architecture supports investigating this
direction, but its speedups are not evidence for RustRed's workload.
[FireFly](https://arxiv.org/abs/1904.00009) describes reconstruction in the IBP
setting; RustRed should use Symbolica's implementation, not import or recreate
FireFly. The same distinction applies to reconstruction discovery versus exact
source-product authority in the proposed backend.
