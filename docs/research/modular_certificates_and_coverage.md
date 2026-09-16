# Modular certificates versus exceptional-domain coverage

Independent research note, 2026-09-16. No solver or verifier change is implemented
by this document. Companion studies are
[the completeness literature review](complete_certification_literature.md) and
[the certification architecture objective](certification_convergence.md).

## Main conclusion

Use semi-numerical computation to **find a small exact witness**, rather than
expecting numerical sampling to establish all domain obligations. There are two
separate acceleration targets:

1. Verify source identities or a compact linear-algebra certificate much more
   cheaply than repeating elimination.
2. Prove that each rule's actual integer application domain is valid, descending
   and covered, including exceptional loci that generic samples rarely hit.

The first has strong applicable literature. It does not automatically solve the
second. A method can have excellent probabilistic soundness for matrix rank and
still say nothing about an omitted affine exceptional face.

## 1. Polynomial-matrix verification: useful, but read the theorem's domain

[Lucas, Neiger, Pernet, Roche and Rosenkilde](https://arxiv.org/abs/1807.01272)
give low-communication certificates for computations over univariate polynomial
matrices, including row-space claims, kernels and normal forms. Their protocols
are perfectly complete but probabilistically sound: correct witnesses are
accepted, while false acceptance has a controllable nonzero probability.

The paper explicitly distinguishes polynomial-module structure over F[x] from
vector-space structure over F(x). Point evaluation alone can lose the former.
RustRed must additionally handle several index variables and specialization at
exceptional loci; the univariate results are not a ready-made completeness
theorem for that workload. Nor does the paper's technical use of “complete”
mean that an IBP cover contains every integer integral.

**Use:** fast diagnostic checks of a proposed source combination or sparse
linear-algebra result, or inspiration for retaining a compact exact witness.
**Not sufficient:** independent exact publication or full index-domain coverage.
Do not replace the current exact contract by randomized acceptance implicitly.

## 2. Elimination and sparse products

[Dumas, Kaltofen, Lucas and Pernet](https://arxiv.org/abs/1909.05692) study
certificates for triangular equivalence and rank profiles, including verification
through matrix-vector products. This supports retaining elimination/dependency
information instead of rerunning the full search. Their fast randomized
verification option must be distinguished from deterministic equality checking.

[Giorgi, Grenet and Perret du Cray](https://arxiv.org/abs/2101.02142) develop
fast probabilistic checks for polynomial products and modular products, including
sparse inputs. These results make a useful fast-rejection layer plausible, but
do not provide an unconditional cheap deterministic test for an arbitrary large
arithmetic circuit. They also do not decide where a rational denominator vanishes.

The native Symbolica-backed solver already retains seed support and performs
modular discovery. A smaller target/source witness can therefore be explored
without replacing the search engine or implementing another elimination kernel.
Keep the actual input/rule/source ordering attached to that witness.

## 3. A deterministic modular identity check, with explicit costs

The following elementary construction is a proposal, not a new implemented
backend or a complexity result borrowed from the preceding papers.

For an exact candidate identity, clear rational coefficient denominators to
obtain an integer polynomial F. Establish a sound bound B on the absolute value
of every coefficient of F. If F is the **zero polynomial** modulo each of a set
of distinct primes whose product M satisfies M > 2B, then F is identically zero
over the integers. Every coefficient is divisible by M and lies in (-M/2,M/2).
No interpolation or CRT reconstruction is needed to justify that implication.

Important limitations:

- The modular checks must establish polynomial zero, not merely zero at a few
  sample points. Degree-complete interpolation/evaluation grids are another
  possibility but can be exponential in the number of variables. Such a grid
  needs more distinct values in each variable than its degree bound permits
  roots; a field that is too small does not suffice (`x^p-x` vanishes at every
  element of the prime field without being the zero polynomial).
- B must be an independently justified bound for the actual expression, not a
  guess from observed coefficients or samples. Large loose bounds can erase the
  performance benefit. Native coefficient arithmetic still owns those bounds.
- Clearing denominators proves an algebraic identity; it does not authorize
  applying a division at an integer point where its denominator is zero.
- This construction does not prove emptiness of a polynomial's integer zero set.
- Native modular polynomial operations must be reused. Do not build a parallel
  CRT/interpolation framework inside RustRed.

For the current small guard witnesses, direct exact Symbolica polynomial
arithmetic is likely simpler. A modular exact checker becomes interesting only
if profiling identifies coefficient bit growth, rather than domain partitioning,
as the dominant cost. This is an engineering inference, not a measured speedup.

## 4. A decisive limitation of residue-only domain proofs

The polynomial

```text
P(x) = (x^2-13)(x^2-17)(x^2-221)
```

has no integer root, but has a root modulo every positive integer. Such
polynomials are called intersective; the example appears in
[Lê and Spencer, Intersective polynomials and Diophantine approximation II](https://home.olemiss.edu/~leth/papers/intersective_polynomials_II_2.pdf).
None of 13, 17 and 221 is an integer square, so the absence of an integer root
is immediate in this example.

Consequently, even examining **all** finite moduli would not find a modular
obstruction to this unsatisfiable integer equation. Finite-field tests can be
excellent filters and witness discoverers, but are not a complete substitute
for integer-domain reasoning. This counterexample concerns the general method;
it does not assert that this polynomial occurs in a vacuum IBP campaign.

## 5. A constructive coverage ledger

An additional design proposal is to retain the generator's case-splitting DAG.
For a predicate P, the two children P=0 and P!=0 exhaust their parent domain.
More complicated exceptional conjunctions must preserve their complete Boolean
meaning. Verifying these local split steps can avoid rebuilding a global cover
from thousands of unrelated rule rectangles after generation.

Each leaf would require exactly one of:

- a replayed rule valid and strictly descending on its actual guarded domain;
- an independently proved empty or zero domain;
- a fully specified finite terminal.

This gives a structural completeness proof **if** every leaf is discharged and
the recorded splits are exact. It does not guarantee that discovery terminates,
that every nonempty leaf has a useful rule, or that every infeasible nonlinear
integer leaf can be proved empty by the supported services. Solver heuristics
that prune or rechart a case need explicit implication/equivalence evidence;
they cannot disappear from the proof record.

This approach is compatible with modular proposal generation, Symbolica exact
witness checking, and a separate certification run. It addresses the observed
loss of domain context more directly than merely increasing proof budgets.

## Native API boundary

The current workspace patches Symbolica 3.0 to `vendor/symbolica`. Existing
`solver/discovery/semi_numerical.rs` already calls native
`reconstruct_rational_function_over_q`, native finite-field sparse reduction,
and native rational-polynomial arithmetic. It retains the symbolic frame and
reconstructs its target-row coefficients; it is not a new domain-proof system.

The proposed proof-discovery layer should use the same upstream reconstruction
service for witness coefficients when useful, followed by exact identity and
domain checks. Reconstruction's internal sample verification is not a replacement
for those proof obligations. Any implementation needs a fresh pinned-API audit,
focused positive/negative tests, and measurements against the current exact path.
