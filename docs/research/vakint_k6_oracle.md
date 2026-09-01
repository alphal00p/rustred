# Vakint K6 oracle and nonminimal-terminal scaling audit

## Scope and claim discipline

This note preserves the exact offline Vakint/MATAD route for the current
three-loop `K = 6` family and audits the proposed higher-loop completion lanes
under the following policy:

- a terminal basis need not be minimal;
- it must nevertheless be finite, universal for the declared family, and
  complete for every supported index;
- every terminal must either map exactly to an evaluated basis or have a
  realistically obtainable, versioned numerical value; and
- FORM, MATAD, FMFT, and AMFlow are offline oracles or data generators only.
  The production RustRed/Vakint scalar path remains Rust plus Symbolica.

The conclusions are deliberately asymmetric. Exact finite closure is a
mathematical property. Affordable terminal evaluation is an engineering
property. Passing either gate does not imply passing the other.

The quantitative terminal limits below are provisional engineering gates, not
theorems about AMFlow. Published AMFlow benchmarks do not establish
six-loop, 20,000-digit throughput for a large terminal basis.

## Complete family input versus Vakint matcher roots

Vakint's five registered three-loop graph classes are routed integral roots
inside one complete six-coordinate family; they are not five independent
integral families.  In RustRed's canonical slot order their sector
representatives are

```text
I3L                    [1,1,1,1,1,1]
I3L_pinch_6            [0,1,1,1,1,1]
I3L_pinch_1_6          [0,1,1,1,1,0]
I3L_pinch_3_6          [0,0,1,1,1,1]
I3L_pinch_1_3_6        [0,0,1,0,1,1]
```

The maximal K4 root already generates the complete physical contraction
downset of all 64 masks.  Canonicalization under the authenticated `S4`
action partitions it into five zero/scaleless orbits containing 26 raw
sectors and six full-loop-rank closure orbits containing the remaining 38.
The latter include the star spanning-tree representative
`[0,0,1,1,0,1]`, whose orbit has size four and which is deliberately absent
from Vakint's five user-facing matcher classes. That orbit has an installed
unimodular `K1^3` factorization. Hence the five matcher roots plus the zero and
factorization authority cover every physical mask: 34 raw masks are seeded by
the matcher portfolio, 26 are scaleless, and the remaining four are the star
product. A publication campaign must still saturate and canonicalize the
maximal root's complete physical downset, but it should ISP-complete each
pinch as an internal sector-local search chart. Those charts remain subordinate
to one parent K6 artifact rather than becoming unrelated published families.

This saturation is necessary but not sufficient to discover a closing rule
set. Adding roots changes the coordinates, source schedule, and lower-sector
feedback seen by the bounded solver; it does not add algebraic identities to
the foundry. In
particular, the two persistent dimension-five boxes from the compact walk are
already numerator-bearing points of the bottom path sector and already see
all nine ordinary K6 IBP sources.  Their first exact reduction action is an
affine loop-routing identity derived from the family and its authenticated
unimodular factorization basis, not another topology and not an ordinary IBP
pivot.  The complete-family campaign will expose every such obligation, but
closure additionally requires that generic factorized-numerator action to be
compiled, authenticated, and installed as an owner.

## Executive verdict

At `K = 6`, Vakint gives a precise and useful but not universal oracle. Its
canonical `I3L` slots are exactly RustRed's slots, and public Vakint settings
can return either exact coefficients of MATAD master symbols or high-precision
Laurent values. Scalar corners and the established single-/double-inactive
witnesses can be queried without topology rematching. A newly isolated Vakint/
MATAD routing defect incorrectly returns zero for some simultaneous
triple-inactive negative-power inputs, so those cases require exact RustRed
source replay and an independent angular/factorization check until the oracle
adapter is fixed.

The nonminimal-terminal policy is sound only as a stopping criterion paired
with a real completion algorithm. It does not turn a sampled list of misses
into a basis and it does not make those misses cheap to evaluate. The
near-term portfolio should therefore be:

1. standard-pair/guard coverage as the closure authority;
2. triangular tubes for targeted strata;
3. signature/Janet or border completion as the systematic finiteness engine;
4. exact graph-orbit and lower-sector reuse; and
5. early stopping at the first terminal complement that is both exactly
   zero-dimensional and below a measured numerical-evaluation budget.

For a direct 20,000-digit six-loop numerical basis, up to roughly one hundred
real terminals is a reasonable green packaging target. Between one hundred
and one thousand is conditional on an actual simultaneous AMFlow pilot.
Above one thousand should be treated as a failed stopping point unless a
compact exact basis map or a much smaller AMFlow system is demonstrated.
These thresholds are driven more by numerical-system size and one-off
generation cost than by final disk space.

## Exact Vakint/MATAD oracle

### Stable family slots

Vakint defines

```text
I3L(msq,n1,n2,n3,n4,n5,n6)
```

with the same six denominator slots as RustRed's complete three-loop family:

| slot | denominator |
| ---: | --- |
| 1 | `k1^2 - m^2` |
| 2 | `k2^2 - m^2` |
| 3 | `k3^2 - m^2` |
| 4 | `(k3-k1)^2 - m^2` |
| 5 | `(k1-k2)^2 - m^2` |
| 6 | `(k2-k3)^2 - m^2` |

The local source is
`FOR_REFERENCE_ONLY_DO_NOT_PUSH/gammaloop/crates/vakint/src/topologies.rs`.
Vakint's MATAD adapter applies the edge permutation

```text
[4, 5, 6, 1, 2, 3]
```

so that

```text
I3L(muvsq,n1,n2,n3,n4,n5,n6)
  -> s4m^n1*s5m^n2*s6m^n3*s1m^n4*s2m^n5*s3m^n6.
```

The implementation is in `src/matad.rs`, symbol
`Vakint::matad_evaluate`. Short-form matching intentionally retains zero
powers. MATAD's `matad-ng.hh` is designed to handle a denominator power `<= 0`
by converting the corresponding inverse propagator to numerator scalar
products before descending to a simpler topology. That mechanism works for
the established diagnostic inputs below, but the current Vakint route is not
sound for every multi-negative `I3L` tuple; negative-power support must be
validated per input class rather than assumed globally.

In particular, direct Vakint/MATAD evaluation reports zero for the path input
`I3L(1,-1,-1,1,-1,1,1)` and the star input
`I3L(1,-1,-1,1,1,-1,1)`. Exact combinations of the nine ordinary K6 IBPs,
independently checked by angular averaging of the factorized topology, give
the nonzero normalized coefficients

```text
path:  2 (d+2)^2 / d^2
star:  (d^2-8) / d^2
```

relative to the installed factorized root. These two tuples are an explicit
oracle-negative regression set. They must not be used to reject a replayed
RustRed relation, and the future Vakint fix must reproduce the exact nonzero
values before triple-inactive MATAD comparisons become gating.

### AlphaLoop forensic: an interior/face/ray closure hierarchy

A direct trace through Vakint's AlphaLoop route resolves the apparent
contradiction. Vakint first maps canonical `I3L` powers to AlphaLoop's
internal order as

```text
[n1,n2,n3,n4,n5,n6] -> [n1,n3,n2,n4,n6,n5].
```

The path representative has a guard-free loop-routing identity that removes
one inactive numerator by expanding a fixed affine combination of denominator
lowering operators and the common mass. At numerator rank `N` its direct
expansion has simplex cardinality `binomial(N+6,6)`. RustRed must not store or
apply that rank-growing expansion: the same exact affine identity can instead
be compiled into a constant-width one-step recurrence, derived automatically
from the authenticated denominator forms and unimodular routing witness.

After a guard-free routing of the star representative, the genuine IBP part
forms a nested parametric hierarchy. In the internal chart

```text
U(x,y,z) = uvid(3,1,x,y,1,z,1,1),
x,y <= 0, z < 0,
```

the maximal interior rule has 14 children and pivot
`G = y+z+2-d`. Every same-sector child strictly lowers the total inactive
depth `H = -x-y-z`; the other children pinch an active line. At `z=0` the
route switches to a two-dimensional face rule with pivot `x+y+2-d`, then to a
one-dimensional ray with pivot `x+2-d`, and finally to the installed
triple-tadpole factorization terminal. AlphaLoop obtains exactly

```text
path:  2 (d+2)^2 / d^2
star:  (d^2-8) / d^2
```

on the two oracle-negative inputs. MATAD loses these expressions in its
three-loop partial-fraction/mass-pattern prepass before its top-family reducer;
the zeros are therefore a preprocessing defect, not zero-integral evidence.

This trace is diagnostic only. No FORM rule, support, coefficient, topology
name, or pivot may enter production RustRed. The oracle-free regression starts
from the family, its nine regenerated ordinary IBPs, graph/routing compiler,
zero sectors, and immutable lower owners. It must rediscover and replay the
interior rule, subtract its exact owned box, then let the exposed face and ray
choose the next sources. Repeating the run with every oracle fixture disabled
is mandatory.

There is an important interpolation trap. At the boundary point `z=-1`, all
terms proportional to `z+1` disappear, so a single triple-negative endpoint
cannot reveal the generic source or child support. Candidate lifting must use
a deterministic interior unisolvent set, require transported-support
stability at additional points, solve over exact symbolic indices with
Symbolica, and retain boundary points as separate closure obligations. The
observed affine coefficient degree and shift radius are useful bounded K6
priors, never completeness assumptions.

### Current nine diagnostic inputs

The three scalar corners are:

```text
topo(I3L(muvsq,0,1,1,1,1,0))
topo(I3L(muvsq,0,1,1,1,1,1))
topo(I3L(muvsq,1,1,1,1,1,1))
```

The six representative recurrence points are:

```text
topo(I3L(muvsq,0,-1,1,2,2,1))
topo(I3L(muvsq,0,-2,2,2,1,1))
topo(I3L(muvsq,0,1,1,2,4,0))
topo(I3L(muvsq,0,1,1,2,5,0))
topo(I3L(muvsq,0,1,2,3,3,0))
topo(I3L(muvsq,0,1,3,2,3,0))
```

The scalar six-line and four-line corners provide immediate checks:

- `[1,1,1,1,1,1]` maps to `miD6`;
- `[0,1,1,1,1,0]` maps to `m_uv^4 miBN`; and
- `[0,1,1,1,1,1]` is a distinct all-massive five-line terminal, not
  MATAD's `miD5`. The exact basis-change row involving `miT111`,
  `Gam(1,1)`, `miD5`, and `miBN` is recorded in `GOAL.md`.

These nine inputs are diagnostic points. The six witnesses sample
positive-dimensional strata; any accepted RustRed owner must prove the whole
stratum and every exceptional branch.

The ignored GammaLoop oracle test now pins the complete canonical raw
expression by byte length and FNV-1a fingerprint and checks exact Symbolica
subatom support against every MATAD master name. The independently replayed
results are:

| input class | exact raw master support | leading Laurent power |
| --- | --- | ---: |
| four-line scalar corner | `miBN` | `epsilon^-3` |
| five-line scalar corner | `Gam(1,1)^3`, `Gam(1,1) miT111`, `miD5`, `miBN` | `epsilon^-3` |
| six-line scalar corner | `miD6` | `epsilon^-1` |
| witnesses `[0,-1,1,2,2,1]`, `[0,-2,2,2,1,1]` | `Gam(1,1)^3`, `Gam(1,1) miT111` | `epsilon^-3` |
| witnesses `[0,1,1,2,4,0]`, `[0,1,1,2,5,0]` | `Gam(1,1)^3`, `miBN` | `epsilon^-2` |
| witnesses `[0,1,2,3,3,0]`, `[0,1,3,2,3,0]` | `Gam(1,1)^3`, `miBN` | `epsilon^0` |

The last witness pair has an identical exact canonical expression. All nine
expanded evaluations reach `epsilon^1` at 80-digit working precision; four
representative Laurent series are independently pinned to 55 decimal digits.
This proves that the current diagnostic points need no new numerical master at
three loops. It does not prove a recurrence for the positive-dimensional
witness strata and therefore does not change the K6 closure gate.

### Exact raw master coefficients

Use the Rust API with:

```rust
let settings = VakintSettings {
    form_exe_path: form_path.into(),
    evaluation_order: EvaluationOrder::matad_only(Some(MATADOptions {
        expand_masters: false,
        ..MATADOptions::default()
    })),
    integral_normalization_factor: LoopNormalizationFactor::FMFTandMATAD,
    use_dot_product_notation: true,
    allow_unknown_integrals: false,
    ..VakintSettings::default()
};

let vakint = Vakint::new()?;
let result = vakint.evaluate_integral(
    &settings,
    vakint_parse!("topo(I3L(muvsq,0,-1,1,2,2,1))")?.as_view(),
)?;
```

This is the same route as `matad_raw_settings` in
`tests/rustred_scalar_evaluation_tests.rs` and
`assert_exact_raw_rustred_matad_peer` in `tests/test_utils.rs`.

The public result retains exact MATAD master symbols, but Vakint has already
performed:

- `d -> 4-2*ep`;
- Euclidean-to-Minkowski sign restoration;
- common-mass restoration;
- loop-normalization handling; and
- replacement of MATAD's private epsilon symbol.

For the literal FORM expression in `d`, retain the generated `out.txt` by
setting `clean_tmp_dir: false` or `VAKINT_NO_CLEAN_TMP_DIR=T`. The debug
message `MATAD: raw result from FORM` is emitted after `d -> 4-2*ep`;
`out.txt` is the pre-conversion expression.

### High-precision Laurent evaluation

Use:

```rust
let settings = VakintSettings {
    form_exe_path: form_path.into(),
    evaluation_order: EvaluationOrder::matad_only(None),
    integral_normalization_factor: LoopNormalizationFactor::FMFTandMATAD,
    run_time_decimal_precision: 20_000,
    number_of_terms_in_epsilon_expansion: 5,
    use_dot_product_notation: true,
    ..VakintSettings::default()
};
```

After `evaluate_integral`, pass `muvsq=1` and `mursq=1` to
`Vakint::full_numerical_evaluation` and read
`NumericalEvaluationResult::get_epsilon_coefficients`.

Vakint sets

```text
maximum epsilon power = number_of_terms - loop_count - 1.
```

MATAD supports at most three loops and `number_of_terms <= 5`. At three
loops, the maximum setting therefore returns through `epsilon^1`, normally
the five coefficients from `epsilon^-3` through `epsilon^1`.

The ten shipped MATAD master entries carry approximately 20,095 to 20,100
decimal digits. A request for 20,000 digits leaves only about 95 to 100 guard
digits. Increasing `run_time_decimal_precision` beyond the source precision
cannot invent further correct digits.

The optional Python community module exposes:

```python
from symbolica import E
from symbolica.community.vakint import Vakint, VakintEvaluationMethod

method = VakintEvaluationMethod.new_matad_method(expand_masters=False)
vakint = Vakint(
    evaluation_order=[method],
    form_exe_path=FORM_PATH,
    run_time_decimal_precision=20000,
    number_of_terms_in_epsilon_expansion=5,
    integral_normalization_factor="FMFTandMATAD",
)
raw = vakint.evaluate_integral(
    E("topo(I3L(muvsq,0,-1,1,2,2,1))", default_namespace="vakint")
)
```

Its parameter dictionary uses Python `float` values, so Rust is preferable
when non-unit parameters themselves require high precision. Unit mass and
renormalization scale are exact in this use case.

### Executable and command-line limits

The local `FOR_REFERENCE_ONLY_DO_NOT_PUSH/form5` directory is a
React/JavaScript form library, not the FORM computer-algebra executable.
During this audit the available FORM executable was:

```text
/nix/store/b72akpm5kyfzks88x9c1hlpwlw8v48lm-form-4.3.1/bin/form
```

That host-specific Nix path reports FORM 4.3. A current oracle test can be run
from the GammaLoop root with:

```bash
FORM_PATH=/nix/store/b72akpm5kyfzks88x9c1hlpwlw8v48lm-form-4.3.1/bin/form \
  cargo test -p vakint --no-default-features \
  --test integral_alphaloop_vs_matad_tests \
  test_integrate_3l_no_numerator -- --nocapture
```

Vakint has no dedicated CLI accepting an arbitrary `I3L` expression. The
appropriate nine-point and ray-depth oracle is a table-driven Rust test or
the optional Symbolica community Python module.

### Existing peer-harness consequence

`compare_evaluations_impl` currently invokes an unconditional exact
RustRed-versus-MATAD raw-master assertion whenever a RustRed lane is present.
That is ideal when RustRed maps every terminal exactly into MATAD's basis. If
the production K6 artifact intentionally retains a different numerical
terminal basis, the harness needs an explicit comparison policy:

- exact raw master coefficients when an exact basis map exists; or
- high-precision Laurent parity when bases intentionally differ.

Numerical parity never replaces RustRed's exact source replay and closure
proof.

## Correction concerning the four-loop oracle

Current Vakint/FMFT can produce exact FORM reductions at four loops, but its
checked-in numerical table is not generally a 20,000-digit oracle.
`src/fmft_numerics.rs` is about 144 kB; most tagged floating constants carry
roughly 26 to 50 digits, four carry 100 digits, and only six are tagged near
20,100 digits. The statement that current Vakint/FMFT can directly evaluate
arbitrary four-loop terminals to 20,000 digits is therefore false.

High-precision four-loop parity requires one of:

1. exact maps into an independently regenerated high-precision FMFT basis;
2. freshly generated AMFlow values; or
3. another independently validated numerical source.

This limitation does not weaken FMFT as an exact offline reduction oracle.

## Two independent terminal gates

### Gate 1: exact finite universal closure

For every sector and every guard branch, the leading-rule monomial ideal must
have a zero-dimensional complement. Equivalently, every standard pair has no
free coordinate. The certificate must also prove:

1. every structural point is owned by a descending rule, zero,
   factorization, lower artifact, or explicit terminal;
2. every pivot and denominator guard is nonzero on its declared domain or
   has all zero branches separately owned;
3. lower-sector and factorized terms normalize through immutable owners;
4. all overlaps have the same exact normal form; and
5. every rule replays from regenerated ordinary sources.

The terminal count `t` is the exact cardinality after zero sectors,
factorizations, lower-loop products, and authenticated symmetry orbits have
been removed. A compact symbolic description of a finite complement is not
enough if its cardinality is astronomically large.

Universality means that the manifest applies to every supported integral key
of the declared family, not only to the finite target list used in one QCD
calculation. A campaign-specific numerical table may cover a smaller
epsilon/rank envelope, but the exact symbolic reducer must state that envelope
instead of silently presenting it as all-rank numerical substitution.

### Gate 2: practical evaluation affordability

For each finite terminal set record:

- total terminal count and symmetry-orbit count;
- independent quotient-dimension estimate `r`, when available;
- redundancy `t/r`;
- number `c` of stored Laurent coefficients;
- desired output precision `P_goal` and working precision `P_work`;
- maximum pole order in exact terminal coefficients;
- cancellation loss measured at representative high-rank targets;
- AMFlow target count, maximum differential-system dimension `m` over the
  recursion tree, and number of epsilon samples;
- one-off wall time, CPU time, peak resident memory, checkpoint size, and
  restart behavior; and
- final artifact size and load-time memory.

No terminal set is practically accepted from `t` alone. AMFlow may reduce many
requested terminals to a much smaller internal master system, in which case
`m << t`. It may instead enlarge the auxiliary-mass family and produce
`m >= t`. The latter is especially dangerous at six loops.

There is also a potential circular dependency. AMFlow constructs differential
systems using IBP reduction. A six-loop single-scale RustRed terminal may
induce auxiliary-mass or Feynman-parameter families that are not covered by
the original single-scale artifact. Finite RustRed closure does not prove that
AMFlow can build and solve those enlarged systems. A real pilot with the
chosen reducer is mandatory before calling the terminal basis computable.

## Precision, Laurent depth, and storage

### Payload formula

For `P` decimal digits, one real multiprecision mantissa needs approximately

```text
ceil(P * log2(10) / 8) bytes ~= 0.4153 P bytes.
```

At `P = 20,000` this is about 8.3 kB per real coefficient. Decimal text needs
about 20 kB per coefficient before syntax and metadata. Complex values double
both figures.

For a real six-loop terminal stored from `epsilon^-6` through
`epsilon^1`, `c = 8`:

| terminal count | decimal digits only | binary mantissas only |
| ---: | ---: | ---: |
| 10 | 1.6 MB | 0.66 MB |
| 100 | 16 MB | 6.6 MB |
| 1,000 | 160 MB | 66 MB |
| 10,000 | 1.6 GB | 664 MB |

The table excludes exponents, checksums, uncertainty metadata, indices,
allocation overhead, exact coefficient maps, and arithmetic temporaries.
For the current three-loop five-coefficient table, the corresponding direct
payload is about 100 kB of decimal text or 41.5 kB of binary mantissas per
real terminal.

Disk space alone is therefore not the dominant objection to hundreds of
terminals. Thousands make source embedding and eager parsing unattractive,
but a versioned binary asset with lazy loading can still store them. The
harder cost is generating and validating their values.

### Differential-system memory proxy

If a high-precision differential system is dense, one real `m by m` matrix
requires at least:

| `m` | one dense real matrix at 20,000 digits |
| ---: | ---: |
| 100 | about 83 MB |
| 300 | about 0.75 GB |
| 500 | about 2.1 GB |
| 1,000 | about 8.3 GB |

Complex matrices double these figures. A solver retains multiple matrices,
series coefficients, factorizations, and temporary values, so these numbers
are payload floors, not peak-memory predictions. Sparsity can change the
scaling substantially and must be measured rather than assumed.

[AMFlow 2.0](https://arxiv.org/abs/2607.08477) benchmarks 316 target masters
on a three-loop five-point family at 20 correct digits. Its two recursion
modes encounter first-step systems of 150 and 521 masters. This is evidence
that systems with hundreds of components are operational at moderate
precision, not evidence for 20,000-digit six-loop feasibility. The original
[AMFlow paper](https://arxiv.org/abs/2201.11669) likewise identifies CPU time
and available RAM as the practical restrictions and explains that epsilon
coefficients are reconstructed from several high-precision numerical
samples.

### Laurent-depth hazard

Finite terminal count does not imply finite all-rank numerical-table depth.
For a requested output through `epsilon^q`, if an exact reduction coefficient
has a pole of order `s` at `d=4`, the terminal value is needed through at
least `epsilon^(q+s)`.

Repeated parametric-rule application can make `s` grow with dot or numerator
rank even when the number of terminal symbols is fixed. In that case:

- exact all-rank symbolic reduction remains valid;
- one fixed, finitely truncated numerical terminal table is not an all-rank
  evaluator.

At least one of the following must then be supplied:

1. an epsilon-finite terminal basis and rules with a proven uniform pole-depth
   bound;
2. a terminal evaluator capable of generating arbitrary epsilon order; or
3. an explicit maximum target-rank/output-order envelope for the shipped
   numerical table.

Every scaling experiment must measure `s` along increasing-rank rays. Precision
in decimal digits and Laurent depth are independent budgets.

### Conditioning and guard precision

Exact closure can still yield inaccurate evaluated answers when terminal
contributions cancel. For a target `I = sum_i c_i T_i`, record an empirical
cancellation loss such as

```text
loss = log10(sum_i |c_i T_i| / |I|).
```

The working precision must exceed the desired result precision by this loss,
the AMFlow integration/fitting loss, and a validation margin. A fixed
20,000-digit terminal table cannot promise 20,000 correct digits for targets
whose cancellation loss is itself large or rank-dependent.

## Provisional affordability policy

The following is a decision policy for pilots, not a mathematical claim:

| status | direct six-loop numerical basis |
| --- | --- |
| green | `t <= 100`, `c <= 8`, measured `m <= 100`, bounded pole depth, and a successful 20,000-digit representative pilot |
| conditional | `100 < t <= 1,000` or `100 < m <= 300`; accept only with measured simultaneous AMFlow scaling, restartable generation, and a sub-gigabyte shipped asset |
| reject pending compression | `t > 1,000`, `m > 300`, `t/r > 10`, an artifact approaching 1 GB, or pole depth/cancellation loss growing without a declared bound |

The `t/r` thresholds are optimization signals, not closure tests. A ratio
below four is desirable; four through ten calls for a measured comparison
between extra completion work and numerical work. A ratio above ten should
normally send the complement back to the completion queue.

An exact map from thousands of RustRed terminals to a small numerical basis
can override the direct-table count, because then the numerical dimension is
the mapped basis size. The exact-map expression size, evaluation time, and
maximum epsilon-pole depth still need budgets.

## Candidate-lane audit

### Signature-filtered Janet/Ore completion

This is the strongest lane for proving finiteness because it turns
nonmultiplicative prolongations into explicit obligations and exposes a
leader ideal whose standard pairs can be checked. The nonminimal policy makes
it more attractive: completion should stop as soon as the complement is
zero-dimensional and affordable, rather than continuing merely to minimize
`t`.

Its main risks remain completion-basis explosion, guard-stratum explosion, and
large exact coefficients. Record the curve

```text
(completion degree, rule count, guard count, terminal count, bytes, time)
```

after every batch. The useful stopping point minimizes total projected cost,
not terminal count alone.

Falsification: reject the current order/source representation when rule and
standard-pair growth is exponential before the terminal count enters the
conditional budget, or when exact guard splitting leaves unsupported
positive-dimensional loci.

### Standard-pair-guided triangular tubes

This remains the best immediate K6 lane. It targets only uncovered free
directions and can be stopped once the coverage authority reports a finite
complement. It is not independently complete: a fixed-width tube that misses
a relation says nothing about masterhood.

Nonminimal terminals reduce the need to force every finite corner onto a
minimal basis, but they do not excuse an infinite standard pair. Tube width,
retained rows, lower-sector work, and terminal cardinality must be tracked
against rank.

Falsification: reject fixed-width tubes as the high-loop default if width or
accumulated lower-sector tubes grow with requested rank, or if each closed
stratum exposes more free strata than it removes.

### Generating-function border/Pfaffian completion

This lane has the highest upside for the numerical policy. A compact derivative
order ideal and flat connection naturally expose a finite state dimension that
could also be close to the dimension of an AMFlow differential system.

Its proof obligations are the hardest: source-module membership, equality to
the original IBP ideal, exceptional index loci, and translation into a
strictly descending discrete reducer. Flatness alone is insufficient.

Falsification: require stable order-ideal size under increasing derivative
degree, exact ordinary-source replay for every border relation, independent
quotient-rank agreement, and a bounded discrete pole depth. Otherwise this is
only a compact differential model, not a RustRed closure artifact.

### Landau/Fitting and critical-locus source compression

This lane can estimate quotient dimension, reveal missing components, and
generate compact candidate sources. It cannot certify finite rewrite closure
or orient rules by itself. Under the nonminimal policy its most valuable
output is a credible lower bound `r` against which `t/r` can be measured.

Falsification: abandon it as a default prepass when saturation/component work
costs more than the downstream solve, or when equal-mass exceptional
components proliferate without yielding new exact source classes.

### Closure-first nonminimal terminals

This is a stopping policy, not a discovery lane. It becomes valid only after
the standard-pair and guard complement is exactly finite. It is particularly
useful when the final relations needed only for basis minimality are much more
expensive than evaluating a few extra terminals.

Its characteristic failure is a mathematically finite but unaffordable
complement. A second failure is AMFlow circularity: the terminal values may
require reductions of enlarged families that the available reducer cannot
provide.

Falsification: do not accept a terminal manifest until its exact count,
epsilon-depth envelope, numerical-system dimension, precision loss, and
one-off generation plan all pass the affordability gate.

### Decorated graph/minor dynamic programming

Exact graph orbiting, factorization, and immutable child artifacts are
essential multiplicative savings. They reduce the total numerical basis by
identifying equivalent terminal values and removing lower-loop products.
They do not make the remaining decorated top-sector solve easier by
themselves. Dots, inactive numerators, guards, mass labels, and routing data
can destroy most automorphisms.

Falsification: report raw terminals, exact decorated orbits, and transported
terminal values separately. Stop crediting topology-only symmetry if the
decorated orbit ratio approaches one or transported warm starts fail exact
replay.

### Modular target separation and sparse linear algebra

Modular rank selection is a scalable discovery accelerator. It does not prove
that the candidate universe was sufficient, that the complement is finite,
or that the selected exact rule has affordable coefficients.

Falsification: support must stabilize over held-out primes and samples, the
exact lift must replay, and the lazy obstruction frontier must be complete
relative to its declared universe. Track nonzeros, fill, and per-worker memory
rather than row count alone.

## Falsification experiments

### K = 6: complete three-loop laboratory

1. Query MATAD for the three corners and six witnesses above. Then query
   representative points at depths 20 through 50 on every open ray. Use this
   only to infer likely recurrence order, guard factors, basis-vector rank,
   and lowering direction.
2. Construct the exact standard-pair and guard complement. Report the first
   batch at which every standard pair has no free coordinate.
3. At that first finite point, report raw terminal count, exact symmetry
   orbits, factorized/lower-loop terminals, and independent MATAD-basis rank.
4. Plot marginal completion cost against terminal reduction:

   ```text
   delta(rows, fill, exact bytes, time) / delta(terminals removed).
   ```

5. Query exact MATAD maps for every proposed terminal. Prefer those maps over
   independent 20,000-digit K6 tables.
6. Reduce targets along all six rays at ranks `1,2,4,8,12,20,40` and record
   the maximum `(d-4)` pole order of every terminal coefficient. A growing
   order falsifies a fixed-depth all-rank numerical table.
7. Run numerical parity with the runtime FORM path deliberately invalid after
   the artifact and terminal maps are loaded.

K6 acceptance is zero uncovered free strata and exact source replay. A small
terminal count or numerical agreement alone is insufficient.

### K = 10: four-loop scaling bridge

For a complete four-loop vacuum scalar-product family, `K = 10` and the
ordinary source count is `q = 16`. Signed diamonds contain:

| radius | translations | complete ordinary rows |
| ---: | ---: | ---: |
| 2 | 221 | 3,536 |
| 3 | 1,561 | 24,976 |

Use one high-symmetry sector and one low-symmetry decorated sector.

1. Run exact backward incidence, modular target separation, triangular tubes,
   and the first Janet batches against the same ordering.
2. Record terminal cardinality and `t/r` after every batch. A single
   representative sector above `t = 1,000` or `t/r = 10` is a negative signal
   before K21.
3. Measure standard-pair construction and compressed size without
   materializing every finite terminal until its cardinality passes the
   budget.
4. Use FMFT for exact reduction maps where supported. Do not treat the current
   Vakint/FMFT numerical table as a 20,000-digit reference.
5. Run an AMFlow precision/batch ladder on `1,8,32,128` representative
   terminals at 100, 1,000, and 5,000 digits. Run a 20,000-digit pilot on the
   smallest stable batch. Record target count and maximum internal system
   dimension separately.
6. Measure coefficient pole order and cancellation loss along dot,
   numerator, and mixed rays through rank 40.

The K10 bridge passes only if the finite-complement size, modular fill, guard
count, and AMFlow system dimension have slopes compatible with the
conditional budget. Exact four-loop closure is not required merely to run
these bounded pilots.

### K = 21: six-loop controls and genuine pilot

At six loops, `K = 21` and the complete ordinary source count is `q = 36`:

| source radius | translations | ordinary rows | raw-nonzero upper bound | potential shift columns |
| ---: | ---: | ---: | ---: | ---: |
| 2 | 925 | 33,300 | 15.4 million | 143,529 |
| 3 | 13,287 | 478,332 | 221 million | 1,244,979 |
| 4 | 143,529 | 5,167,044 | 2.39 billion | 9,041,957 |

Full diamonds are controls only.

1. Begin with the six-loop banana family, then one 15-propagator/six-ISP
   trivalent family. For the latter use both a high-symmetry sector and an
   asymmetric decorated control.
2. Stream the radius-two modular matrix with both 36 ordinary sources and the
   seven-generator Lie subset. Do not materialize the full 143,529-column
   universe. Report bytes per admitted nonzero, pivot fill, and peak memory
   for one, two, and four workers.
3. Run a radius-three incidence dry run before numerical elimination. It must
   show whether lazy generation avoids the nominal 478,332 rows.
4. Compute the structural standard-pair dimension after every completion
   batch. Require monotone progress and a finite-cardinality forecast before
   enumerating terminal keys.
5. Test tube ranks `1,2,4,8,12,20,40` on one-axis, zigzag, and mixed
   numerator/dot paths. Reject a fixed-width claim when required width grows
   with rank.
6. Measure exact coefficient pole depth and cancellation loss on the same
   paths. An unbounded trend blocks a fixed-depth numerical artifact even if
   symbolic closure succeeds.
7. Once a candidate terminal orbit set enters the conditional budget, run
   AMFlow on batches `1,8,32,128` and record the maximum differential-system
   dimension anywhere in the recursion. Extrapolate neither 20-digit nor
   low-loop performance blindly to 20,000 digits.
8. Require restartable checkpoints and two independent high-precision
   validations for every terminal value before shipping the immutable table.

Stop the candidate architecture when:

- a positive-dimensional standard pair survives without a queued owner;
- guard decomposition produces unsupported infinite algebraic strata;
- terminal count exceeds 1,000 before lower sectors are included;
- `t/r` extrapolates above ten;
- maximum AMFlow system dimension exceeds 300 without demonstrated sparse
  high-precision scaling;
- exact pole depth or cancellation loss grows without a declared envelope;
- modular support remains unstable under held-out primes; or
- peak memory approaches the radius-three control without reducing the
  uncovered dimension.

Passing the banana control is not proof for the trivalent family. Passing
rank 40 is not an all-rank proof. Those experiments are filters before the
exact completion certificate.

## Reasons numerical parity can fail after exact closure

Even a correct finite rewrite system can disagree numerically because:

- terminal Laurent series are not deep enough for spurious coefficient poles;
- the requested 20,000 digits omit rank-dependent cancellation loss;
- `d=4-2*epsilon`, common-mass homogeneity, Euclidean/Minkowski signs, or
  normalization conventions differ;
- a K6 routing permutation or decorated symmetry map is wrong;
- independently rounded redundant terminals violate exact relations;
- AMFlow uncertainties or epsilon-fit errors are correlated but treated as
  independent;
- relative comparison is used near a true zero;
- an `Oep` sentinel survives beyond the available terminal depth; or
- terminal values are correct but attached to the wrong typed integral key.

Exact raw basis comparison is stronger whenever an exact basis map exists.
When the terminal basis intentionally differs, numerical parity should use
several precisions and compare the stability of every Laurent coefficient,
not only one summed value.

## Decision rule

A nonminimal candidate terminal set may be published only when the following
three statements are independently true:

1. **Closure:** an exact replayed certificate proves that every supported
   integral reduces to the finite typed set.
2. **Evaluation:** exact maps or measured AMFlow runs provide every required
   Laurent coefficient with the declared precision and rank/order envelope.
3. **Operations:** the checked-in artifact and numerical tables fit the
   packaging, load-time memory, deterministic reduction, and Vakint acceptance
   budgets.

Minimality is optional. Finiteness, universality, completeness, and practical
evaluation are not.
