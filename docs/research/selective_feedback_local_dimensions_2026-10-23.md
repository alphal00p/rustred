# Selective symmetry–IBP feedback and local dimension-pair recurrences

Status: implementation and validation in progress. No new five-loop gain or
completed new production refinement is claimed here yet.

## First: feed useful auxiliary targets back into finite refinement

The existing complete-numerator symmetry equations can leave auxiliary
integrals in the exact sparse row space. They are not new masters. Some link
two protected terminal integrals only after an additional ordinary IBP or
symmetry equation is generated at an auxiliary point.

For example, rows linking `T1` to `A` and then `A` to `B` need not eliminate a
terminal immediately. An equation generated at `B` can finally connect `T1` to
`T2`. Stopping after the first no-gain round misses that chain. Conversely,
recursively enqueuing every expanded numerator monomial proved far too costly
in the previous experiment.

The new opt-in policy selects one actual terminal-linked auxiliary key per
positive support, minimizing `(A+R, R, A, signed index tuple)`. It tries that
support once while the protected terminal rank remains fixed. New supports
can enter without a rank gain; old supports reopen only when that rank strictly
decreases. Every new integral remains auxiliary, and every accepted equation
retains its exact source and conditions.

There are finitely many supports for a fixed family and finitely many strict
rank decreases. This bounds the number of selected attempts mathematically,
without a hard-coded work or round ceiling. It does not bound coefficient
swell or provide a practical runtime estimate. Exhaustion means a fixed point
of this selected policy, not a minimal basis or exhaustive IBP saturation.

Each round must bind the complete parent native row-space snapshot, maps and
guards, selected keys, regenerated source witnesses, exact task vector and
committed progress. Finalization may retain proved maps from an interrupted
round without claiming that its remaining search completed.

## Second: compile a local dimension pair back to the original dimension

Choose a loop-integration direction and a momentum `K` normalized along it.
Let `q1,...,qE` span the spectator directions needed by all active denominator
and numerator factors. Define

```
H = Gram(q1,...,qE)
G = det H
P = det Gram(K,q1,...,qE)
U(A) = sum_i alpha_i * a_i * E_i^+
```

Here `alpha_i` is the coefficient of `K^2` in the complete affine quadratic
factor, including factors with negative indices. With `B_i = E_i^-`, the local
dimension-pair identity becomes the fixed-d equation

```
[2 U(A) P(B) - (d-E) G(B)] I_d = 0.
```

The order matters: `[A_i,B_i]=1`, so the first product uses the original `a_i`,
not `a_i` after the lowering monomial has acted. Both intermediate shifts are
local to the selected subintegration; they are not a shift of every loop of
the parent family. No shifted-dimensional master is exported.

An independent ordinary-IBP certificate uses the polynomial vector

```
V = G*K - sum_ij q_i * adj(H)_ij * (K.q_j).
```

It satisfies `div V=(d-E)G` and gives the same equation. Polynomial weights
must be expanded into regenerated ordinary sources at shifted seeds. Multiplying
an already integrated equation by a loop-momentum polynomial would be wrong.
Existing native Symbolica polynomial arithmetic and RustRed source generation
are reused rather than implementing another algebra kernel.

For `D=K^2-m^2`, the tadpole control is

```
(2*a-d)*I(a) + 2*a*m^2*I(a+1) = 0.
```

Mass constants, dotted lines, signed numerator powers and factors such as
`d-4` stay exact. An omitted numerator spectator changes the identity; such a
nomination must be enlarged or rejected, never silently treated as scalar.
The final polynomial certificate need not divide by a Gram determinant or
`d-E`. Any divisions made later by elimination retain their ordinary guards.

This is a structured way to nominate useful finite IBP combinations. It does
not guarantee new independent identities or a minimal set of masters. Dense
global dimension-shift expansions are deliberately not the default approach.

## Validation and measurements

The first release-linked single-worker lower-loop probe compares the same
generator cohort and depth-zero ordinary search with/without the selected
feedback loop. These are single diagnostic runs, not paired performance claims.

| Input | Basis before ordinary search | Baseline final | Feedback final | Baseline seconds | Feedback seconds |
|---|---:|---:|---:|---:|---:|
| K6 | 5 | 5 | 5 | 0.402 | 0.602 |
| H | 22 | 19 | 19 | 14.02 | 18.63 |
| FG | 16 | 15 | 15 | 6.01 | 8.01 |
| BMW | 17 | 15 | 15 | 7.61 | 8.82 |
| X | 19 | 16 | 16 | 15.83 | 18.43 |

All 1,193 raw maps compose and retain their original guards. Cold loading passes.
Selected rounds: 4/6/5/5/5; targets: 15/162/105/79/109. Each case has actual
no-gain rounds exposing new supports before exhausting the finite selected
queue. The mechanical behavior is validated; no physical master-count benefit
or speedup is established by these lower-loop controls. A real tadpole test
does establish that a selected auxiliary source can add an exact relation
without promoting that source to the protected master roster.

Evidence: `TMP/selective-symmetry-acceptance-20261023/lower-matrix.summary.json`.
Pending: final native app lifecycle gates, pinned five-loop results, and the
second-stage implementation/measurements. Production campaigns and their frozen
executable are not modified by these tests.

## References

- [Tarasov, hep-th/9606018](https://arxiv.org/abs/hep-th/9606018): dimension-changing
  relations and tensor integrals.
- [Bitoun et al., 1712.09215, section 2.5](https://arxiv.org/html/1712.09215): paired
  dimension shifts and their connection to momentum-space IBP operators.
- [Pikelner, FMFT, 1707.01710](https://arxiv.org/abs/1707.01710): localized
  propagator-subgraph/convolution methods motivating the structural selection.
