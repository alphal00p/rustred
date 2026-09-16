# Converging on topology-independent certification

This is the architecture objective and current diagnosis, not a claim that the
present verifier is complete. It answers the 2026-09-16 concern about repeatedly
encountering new four-loop proof gaps.

## Scope of the claim

The intended proof rules depend on algebra and index-domain structure, not graph
names or loop counts. A rule identity, affine implication, excluded-domain proof
or descent certificate must apply unchanged to every topology that presents the
same mathematical obligation. New topologies should normally supply new data,
not require new proof code.

There are three distinct goals:

1. **Soundness:** an accepted certificate establishes the stated claim.
2. **Completeness for a specified domain language:** every valid obligation in
   that language can be resolved in principle, with an unbounded complete
   procedure. The current bounded verifier does not yet establish this property.
3. **Practical performance:** proofs are found and checked within realistic
   memory/time limits. Mathematical completeness does not guarantee this.

We should aim for the second property for the coordinate/affine-integer
coverage/domain sublanguage used by the vacuum lane, while retaining explicit
unsupported/inconclusive results for richer conditions. This does not include
every nonlinear denominator-guard obligation already arising in that lane;
those need separately specified algebraic certificates and completeness limits.
This is not an unconditional promise of a fast,
terminating certifier for every topology or unrestricted integer-polynomial
predicate. The unrestricted integer-polynomial decision problem is undecidable;
that fact does **not** establish undecidability of the actual IBP-derived subclass.
[Matiyasevich's account](https://logic.pdmi.ras.ru/~yumat/H10Pbook/par_1_1.htm).

## What recent failures actually show

| Obligation | Current evidence | Generic remedy |
| --- | --- | --- |
| FG115, rule 76: apparent upward step | The entire failing piece satisfies a complete saved exclusion; no live rule application exists there | Prove exclusion containment relative to the target, not relative to its larger rectangular hull |
| H229: prospective affine-substitution size | A fixed coordinate is not retained in a later proof chart, making the conservative expansion estimate unnecessarily large | Preserve exact singleton restrictions consistently through all guard coefficients and exclusions |
| X155: consistency work | A configured cumulative proof budget is exhausted | Share/reuse proof state and small certificates; a larger budget alone is not a completeness argument |
| Remaining sectors/guards | Not all final publication checks have succeeded | Continue checking; do not infer that all unseen failures are merely verifier limitations |

For FG, write the affine target as `T = 1+n2+n9 = 0`, and one whole
exclusion as `E = 1+n8+n9 = 0`. The rejected piece fixes `n8=-1,n9=0`.
Consequently E vanishes throughout the piece; T additionally fixes `n2=-1`.
There is no point on **piece AND target AND NOT exclusion**. A checker demanding
that T hold throughout the larger rectangle misses this simple implication.

The offending coefficient is `-(1+n8)/((1+n8+n9)*(3+n8+2*n9))`.
It is 0/0 on the rejected piece, so declaring the coefficient zero would be
wrong. The correct proof removes an empty application domain before asking for
descent. This is a representation/proof-composition gap, not a topology-specific
IBP identity. The exact diagnostic and negative nearby control are retained in
`/tmp/rustred-fg115-bundle-diagnostic.QLM96W/DIAGNOSIS.md`.

## Intended common proof contract

Represent the actual application domain consistently as:

```text
sector signs AND coordinate bounds AND target equations
AND NOT (whole exclusion 1 OR whole exclusion 2 OR ...).
```

Each exclusion remains its complete conjunction. Work relative to the target
and preserve integer/lattice conditions, original denominator restrictions and
fixed faces. An affine rectangular hull is not the actual domain.

The remaining obligations then separate cleanly:

- **Identity:** regenerate ordinary sources and verify an exact linear
  combination, after justified denominator clearing. This does not depend on
  knowing the graph's name or solving its exceptional geometry.
- **Applicability:** prove the original denominators valid on the actual domain
  or refine it into guarded pieces. Never infer safety from a cancelled pole.
- **Descent:** check every nonzero child against a well-founded integral order
  on that same application domain. Excluded regions are not obligations.
- **Coverage:** account for the entire admitted domain through rules, proved
  zero sectors and explicit finite terminals. Fixed-point exhaustion or tested
  integer points alone do not prove this.

Search-facing checks, publication and cold loading should share the semantics
and elementary proof services, while cold verification remains independent of
untrusted cached results. Independent verification means recomputing/checking
evidence, not maintaining divergent meanings of an affine domain.

## More general small witnesses, not an endless list of pattern fixes

For a stubborn guard with coefficient equations Cj=0, search for a compact exact
identity `T = sum_j Uj*Cj`, where T is already proved nonzero on the domain.
This refutes simultaneous vanishing. Modular arithmetic and numerical probes
can discover candidate multipliers; only exact verification and the domain
nonvanishing proof authorize the conclusion. This generalizes several captured
four-loop guards without encoding their individual formulas in the engine.

Bounded-degree algebraic infeasibility certificates provide the relevant
methodological precedent, but degree/support growth remains a real scaling
risk. [De Loera et al., NulLA](https://www.math.ucdavis.edu/~deloera/researchsummary/jsc09_issac08.final.pdf).
Native Symbolica polynomial, factorization, linear-algebra and available
reconstruction services must supply the CAS work; its
[polynomial APIs](https://symbolica.io/docs/polynomials.html) also expose
Groebner-basis functionality. API availability alone does not supply a complete,
efficient proof service for all integer domains. Audit the pinned APIs before
implementing a particular service.

## Acceptance criterion for this direction

Progress means that one generic service handles entire classes of obligations,
with topology-free regression tests and independent audit, rather than merely
moving a campaign to the next ordinal. Required negative controls include
mixed exclusion conjunctions, widened domains with genuine zeros, non-integral
affine solutions, poles, non-descending live terms and exhausted budgets.

The generator and certifier remain separate. Experimental numerical Vakint
comparisons can proceed using explicitly uncertified candidates, but neither
those tests nor successful generation establish whole-family closure. The
current four-loop certification and full numerical acceptance goals remain open.
