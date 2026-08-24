# Vakint tensor projector rank-10 orbit design

Status: design and audit only. This work is deferred behind the current generic
parametric `SolvejSector` closure. Nothing in this document claims that the
rank-10 projector, orbit witness, or tests have been implemented.

RustRed must continue to use pure Rust and Symbolica. The readable FORM files
below are validation oracles only; neither FORM nor copied rank-specific tables
may enter the production path.

## Audit conclusion

RustRed's admitted vacuum tensor projector is mathematically the same global
O(d) projector used by Vakint: it enumerates perfect matchings, forms

```text
G(pi, sigma) = d ^ cycles(pi union sigma),
```

and applies `G^-1` to the scalar contractions of the source tensor. This is
topology- and loop-count-independent. For well-formed vacuum covariant inputs
that the current API accepts, no incorrect tensor formula was found.

The production representation is nevertheless incomplete. It stores and
inverts the full matching Gram matrix, defaults to a hard rank-eight ceiling,
and has no rank-6, rank-8, or rank-10 projector validation. Vakint's
`MAXRANK=8` is only a preload threshold: its procedure discovers a rank-10 term
and then loads `pvtab10.h`. RustRed's rank-eight setting is instead a hard
rejection.

The smallest generic closure is to derive the inverse by double-coset orbits of
matching pairs. At rank 10 this replaces a 945 by 945 Symbolica solve with a 7
by 7 solve, without hardcoding a coefficient, topology, or loop count.

## Source evidence

Vakint preloads ranks 2, 4, 6, and 8 in
[`tensorreduce.frm:32-175`](../../vendor/gammaloop/crates/vakint/form_src/alphaloop/tensorreduce.frm#L32).
The number of metric-pairing classes is 1, 2, 3, and 5 respectively. The
generic loader is at
[`tensorreduce.frm:177-199`](../../vendor/gammaloop/crates/vakint/form_src/alphaloop/tensorreduce.frm#L177),
and the complete seven-class rank-10 oracle is
[`pvtab10.h:3-673`](../../vendor/gammaloop/crates/vakint/form_src/alphaloop/pvtab10.h#L3).

The actual procedure:

- contracts same-index internal vectors before treating outside metrics at
  [`tensorreduce.frm:218-224`](../../vendor/gammaloop/crates/vakint/form_src/alphaloop/tensorreduce.frm#L218);
- removes odd internal rank at
  [`tensorreduce.frm:226-233`](../../vendor/gammaloop/crates/vakint/form_src/alphaloop/tensorreduce.frm#L226);
- constructs source contraction representatives at
  [`tensorreduce.frm:240-259`](../../vendor/gammaloop/crates/vakint/form_src/alphaloop/tensorreduce.frm#L240);
- applies the pairing-class coefficients at
  [`tensorreduce.frm:261-292`](../../vendor/gammaloop/crates/vakint/form_src/alphaloop/tensorreduce.frm#L261);
- merges outside metrics and `vec1` spectators at
  [`tensorreduce.frm:294-361`](../../vendor/gammaloop/crates/vakint/form_src/alphaloop/tensorreduce.frm#L294); and
- detects a rank above the preload threshold and loads the additional table at
  [`tensorreduce.frm:369-385`](../../vendor/gammaloop/crates/vakint/form_src/alphaloop/tensorreduce.frm#L369).

Vakint's template does not override `MAXRANK` and invokes the procedure directly
at
[`run_tensor_reduction.txt:13-22`](../../vendor/gammaloop/crates/vakint/templates/run_tensor_reduction.txt#L13).
Its Rust bridge always makes `pvtab10.h` available at
[`vakint/src/lib.rs:2507-2514`](../../vendor/gammaloop/crates/vakint/src/lib.rs#L2507).
The optional `TENSORSYMTABLE` branches reference `pvtab8sym.h` and
`pvtab10sym.h`, but those files are not vendored and the template never defines
that switch. RustRed should derive such grouping from the input, not reproduce
that unavailable data path.

RustRed's dense implementation is visible at:

- default `max_rank=8`, `max_pairings=105`, and dense resource ceilings:
  [`generic_tensor_projector.rs:45-119`](../../src/generic_tensor_projector.rs#L45);
- perfect-matching generation and contraction-cycle counting:
  [`tensor.rs:764-888`](../../src/tensor.rs#L764);
- dense non-covariant Gram construction, inversion, and expansion:
  [`generic_tensor_projector.rs:2165-2381`](../../src/generic_tensor_projector.rs#L2165);
- the duplicate covariant Gram builder:
  [`generic_tensor_projector.rs:2501-2597`](../../src/generic_tensor_projector.rs#L2501);
- checked dense Gauss-Jordan inversion:
  [`generic_tensor_projector.rs:2600-2696`](../../src/generic_tensor_projector.rs#L2600); and
- witnesses that retain every entry of `G^-1`:
  [`generic_tensor_projector.rs:583-608`](../../src/generic_tensor_projector.rs#L583)
  and
  [`generic_tensor_projector.rs:756-781`](../../src/generic_tensor_projector.rs#L756).

The current statement that an orbit implementation can preserve the dense
witness format is at
[`generic_tensor_projector.rs:953-954`](../../src/generic_tensor_projector.rs#L953).
That is not practical at rank 10: preserving the stored dense inverse would
still retain 893,025 `RationalPolynomial` values.

## Matching orbits and equivalence to the FORM tables

For rank `r=2n`, fix the canonical matching `alpha`. The union of `alpha` and
another matching is a disjoint union of alternating cycles. Half of each cycle
length gives a partition of `n`. This full partition, not merely the number of
cycles, labels the double coset.

| rank | matchings `(r-1)!!` | orbit classes `p(n)` | class cardinalities |
|---:|---:|---:|---|
| 2 | 1 | 1 | `1` |
| 4 | 3 | 2 | `1, 2` |
| 6 | 15 | 3 | `1, 6, 8` |
| 8 | 105 | 5 | `1, 12, 12, 32, 48` |
| 10 | 945 | 7 | `1, 20, 60, 80, 160, 240, 384` |

These are exactly the 1, 2, 3, 5, and 7 groups in Vakint's `pvgtab` tables.
For example, after `d=4-2*ep`, the two rank-four coefficients at
[`tensorreduce.frm:67-68`](../../vendor/gammaloop/crates/vakint/form_src/alphaloop/tensorreduce.frm#L67)
become

```text
(d+1) / (d (d-1) (d+2))
-1    / (d (d-1) (d+2)),
```

which are the diagonal and off-diagonal entries of RustRed's rank-four inverse
Gram matrix.

`SlotPairing::contraction_cycles` currently returns only the number of connected
components at
[`tensor.rs:822-841`](../../src/tensor.rs#L822). Rank 8 already has distinct
partitions with the same component count, such as `2+2` and `3+1`; the orbit
kernel therefore needs a new full cycle-partition classifier.

## Generated orbit-quotient algorithm

The production algorithm should be derived for any admitted even rank:

1. Enumerate all perfect matchings `M` in the existing deterministic order and
   choose `alpha = M[0]`.
2. Compute `type(alpha,sigma)`, the descending alternating-cycle partition, for
   every `sigma` in `M`.
3. Order the distinct partitions canonically. Retain their cardinalities and
   the first matching of each type as representative `tau_mu`.
4. Build the quotient matrix entirely over the family's Symbolica coefficient
   context:

   ```text
   Q[mu,lambda] =
       sum over sigma with type(alpha,sigma)=lambda
           d ^ cycles(tau_mu union sigma).
   ```

5. Solve

   ```text
   Q h = e_identity,
   ```

   with checked exact arithmetic. `e_identity` is one only for partition
   `1+...+1`.
6. During tensor expansion, obtain an inverse-Gram entry without a dense
   matrix:

   ```text
   G_inverse[pi,sigma] = h[type(pi,sigma)].
   ```

7. Recontract every quotient representative and verify `Q h=e_identity`
   independently of output assembly.

The solve sizes are 5 by 5 at rank 8 and 7 by 7 at rank 10. Building rank 10's
quotient needs only `7*945` representative-to-matching contractions. Explicit
output can still require up to `945^2=893,025` candidate pairs, which is an
intrinsic worst case when all vector slots are distinguishable, but it no
longer requires cubic dense elimination or a dense coefficient witness.

For practical vacuum numerators, group before emitting terms:

- group output matchings by the resulting `(covariant, post-contraction
  factor)`; and
- group source matchings by the resulting loop scalar-product monomial.

Accumulate the orbit coefficients over each pair of groups. This recovers the
same internal/outside symmetry optimization as the FORM procedure from actual
vector identities and spectator structure, without a loop-specific rule.

## Witness, guards, and resource accounting

The dense witness must be replaced by a new schema or an enum variant. An orbit
witness must retain at least:

- rank and deterministic matching list;
- ordered cycle partitions, class cardinalities, and representatives;
- the exact quotient matrix and class coefficients;
- all quotient pivot and final coefficient guards; and
- enough grouping provenance to replay the emitted numerator.

Replay must rebuild the classification, reproduce `Q`, prove `Q h` is the
identity column, and reproduce output grouping. Re-running the same top-level
projector and equality-comparing, as currently done at
[`generic_tensor_projector.rs:840-857`](../../src/generic_tensor_projector.rs#L840)
and
[`generic_tensor_projector.rs:931-949`](../../src/generic_tensor_projector.rs#L931),
is useful but is not an independent quotient identity check.

Resource counters must report actual retained/work data: orbit count, quotient
entries, quotient elimination operations, class coefficients, grouping entries,
and output candidates. `P^2` remains the candidate ceiling, but it must not be
reported as allocated Gram or inverse entries. The aggregate assumptions in
[`generic_tensor_polynomial.rs:880-961`](../../src/generic_tensor_polynomial.rs#L880)
and
[`generic_tensor_polynomial.rs:1000-1040`](../../src/generic_tensor_polynomial.rs#L1000)
must change with the witness. The projector is currently stateless, and a tensor
polynomial rebuilds it for every source at
[`generic_tensor_polynomial.rs:484-495`](../../src/generic_tensor_polynomial.rs#L484);
an orbit kernel may be shared by rank and exact dimension after the derived
path is correct.

After this change, the default even-rank policy may admit rank 10 and 945
matchings. Odd rank must be tested before applying the even-projector rank cap,
so a rank-nine vacuum loop tensor returns exact zero as in Vakint rather than a
rank-limit error.

## Admitted scope and semantic caveats

This design closes Vakint-style tensor reduction for vacuum integral families,
not the broader external-momentum tensor basis.

- Both projector entry points reject a family with physical external momenta at
  [`generic_tensor_projector.rs:985-988`](../../src/generic_tensor_projector.rs#L985)
  and
  [`generic_tensor_projector.rs:1113-1116`](../../src/generic_tensor_projector.rs#L1113).
  Numerator-only `p(...)` vectors are typed spectators and do not enlarge the
  family's scalar-product basis.
- The covariant path has the correct internal-vector-before-outside-metric
  ordering at
  [`generic_tensor_projector.rs:1356-1604`](../../src/generic_tensor_projector.rs#L1356).
  The public non-covariant path and legacy projector instead validate combined
  vector/metric multiplicity first at
  [`generic_tensor_projector.rs:2023-2064`](../../src/generic_tensor_projector.rs#L2023)
  and
  [`tensor.rs:1182-1216`](../../src/tensor.rs#L1182). They reject raw Vakint
  syntax such as `(k_mu k_nu)^2 g(mu,nu)` and must delegate to the shared
  precontraction semantics or be retired from production.
- Covariant precontraction rejects more than two total vectors at one label
  before considering their types at
  [`generic_tensor_projector.rs:1409-1426`](../../src/generic_tensor_projector.rs#L1409).
  This is stricter than FORM. Two internal vectors plus an outside spectator
  have a unique required first contraction and should not be rejected merely
  because the total endpoint count is three. Contract the internal pair first,
  then validate the remaining outside component.
- Symbolica loop identities are matched exactly and simultaneously at
  [`symbolica_tensor_numerator.rs:503-558`](../../src/symbolica_tensor_numerator.rs#L503),
  and loop, spectator, and mixed dots are decoded at
  [`symbolica_tensor_numerator.rs:679-764`](../../src/symbolica_tensor_numerator.rs#L679).
  This is topology-independent and preserves decorated identity atoms.
- Opaque scalar spectator weights are retained by compilation but deliberately
  fail `try_weighted_sources` at
  [`symbolica_tensor_numerator.rs:307-348`](../../src/symbolica_tensor_numerator.rs#L307).
  Vakint carries such factors through tensor reduction, so full Vakint parity
  still requires a proof-preserving outer Atom coefficient that does not widen
  the family field.
- Components with genuinely ambiguous Einstein multiplicity should continue to
  fail with a typed error. Rank extension must not weaken those checks.

## Existing validation and missing evidence

Current RustRed tests establish only the lower-rank seam:

- authenticated rank 2, identical-vector rank 4, one trace, and odd rank 3:
  [`generic_tensor_projector.rs test:64-220`](../../tests/generic_tensor_projector.rs#L64);
- spectator rank 1/2 and the frozen one-loop Vakint A/B equations:
  [`generic_tensor_projector.rs test:223-285`](../../tests/generic_tensor_projector.rs#L223)
  and
  [`generic_tensor_projector.rs test:338-431`](../../tests/generic_tensor_projector.rs#L338);
- the raw repeated-index Vakint quartic through covariant polynomial
  precontraction:
  [`generic_tensor_polynomial.rs test:57-133`](../../tests/generic_tensor_polynomial.rs#L57);
- dense rank-four coefficients and `G*G^-1=1` in the legacy projector:
  [`tensor_reduction.rs:108-191`](../../tests/tensor_reduction.rs#L108); and
- Symbolica A/B parsing, arbitrary identities, decorated indices, and dummy
  collision handling:
  [`symbolica_tensor_numerator.rs test:89-178`](../../tests/symbolica_tensor_numerator.rs#L89)
  and
  [`symbolica_tensor_numerator.rs test:220-334`](../../tests/symbolica_tensor_numerator.rs#L220).

The rank-6 assertion in
[`tensor_reduction.rs:49-60`](../../tests/tensor_reduction.rs#L49) checks only the
number of matchings; it does not project a rank-6 tensor. The one-loop tensor
matrix stops at rank 4
([`vakint_one_loop_tensor_matrix_oracle.rs:201-431`](../../tests/vakint_one_loop_tensor_matrix_oracle.rs#L201)),
and the two-loop Vakint fixture precontracts its apparent quartic term to rank
zero plus a separate rank-two term
([`vakint_two_loop_tensor_ibp_oracle.rs:96-148`](../../tests/vakint_two_loop_tensor_ibp_oracle.rs#L96)).
The 3-, 4-, and 5-loop end-to-end tests exercise only rank-two tensors at
[`certified_three_loop_vakint_oracle.rs:296-337`](../../tests/certified_three_loop_vakint_oracle.rs#L296),
[`certified_four_loop_factorized_oracle.rs:173-210`](../../tests/certified_four_loop_factorized_oracle.rs#L173),
and
[`certified_five_loop_factorized_oracle.rs:174-207`](../../tests/certified_five_loop_factorized_oracle.rs#L174).

There is no RustRed projection test at rank 6, 8, or 10 and no exact comparison
against `pvctab6`, `pvctab8`, or `pvctab10`. Vendored Vakint's dedicated tensor
tests themselves contain only one-loop A/B and one two-loop fixture at
[`tensor_reduction_tests.rs:1-115`](../../vendor/gammaloop/crates/vakint/tests/tensor_reduction_tests.rs#L1).

## Implementable acceptance matrix

All tests must use the licensed GMP Symbolica build, run in parallel through
the existing runner at [`scripts/test.sh:1-31`](../../scripts/test.sh#L1), and
must not invoke FORM.

| ID | Test | Required assertion |
|---|---|---|
| ORB-1 | Orbit inventory at ranks 2/4/6/8/10 | Matching counts, ordered partitions, class counts, and the cardinalities in the table above are exact and sum to `(r-1)!!`. |
| ORB-2 | Quotient identity | For every rank, rebuild `Q` and prove `Q*h=e_identity` exactly in Symbolica. |
| ORB-3 | Dense differential | At ranks 2/4/6/8, every orbit-selected coefficient equals the existing dense inverse and full recontraction yields the identity. Dense code remains test-only after migration. |
| ORB-4 | Vakint coefficient oracle | With `d=4-2*ep`, compare every derived class coefficient exactly with `pvctab2`, `pvctab4`, `pvctab6`, `pvctab8`, and `pvctab10`. Oracle strings may live in tests; production may not contain them. |
| ORB-5 | Identical-vector moments | At ranks 2, 4, 6, 8, and 10, every metric pairing has coefficient `(k^2)^n / product(j=0..n-1, d+2j)`. Rank 10 emits 945 pairings within defaults. |
| ORB-6 | Mixed loop identities | Deterministic rank-6/8/10 patterns over one through five loop identities agree with dense rank-6/8 and quotient recontraction; no loop-count dispatch appears. |
| PRE-1 | Vakint precontraction | Raw `(k_mu k_nu)^2*g(mu,nu)` becomes `(k^2)^2*g(mu,nu)` with no projector denominator. |
| PRE-2 | Outside endpoint priority | Two same-index loop vectors plus a spectator and/or outside metric contract the loop pair first, then preserve/contract the outside component. |
| PRE-3 | Metric graph | Open chains, vector-ended chains, spectator-ended chains, loop-loop and spectator-spectator endpoints, and closed traces produce the exact metric/vector/dot or power of `d`. |
| ODD-1 | Odd ranks | Ranks 1, 3, and 9 return exact zero even with spectator covariants; rank 9 does not fail the even-projector cap. |
| SPC-1 | Spectator structures | Free, fully contracted, and mixed spectator patterns at ranks 2/4/6/8/10 retain exact identities and canonical scalar products. |
| ATM-1 | Symbolica boundary | Exact loop-map ordering, decorated indices, indexed sums, linear dots, private-dummy collisions, unknown loop identities, and opaque weights have deterministic success or typed failure. |
| WIT-1 | Orbit witness tampering | Changing a partition, cardinality, representative, quotient entry, class coefficient, guard, grouping record, or output term makes replay fail. |
| LIM-1 | Resource boundaries | Exact and one-below limits cover pairings, orbit classes, quotient entries/work, class coefficients, grouping entries, output candidates/terms, guards, and retained bytes. |
| GEN-1 | Loop-count independence | The same rank-2/4/6 kernel is exercised against artificial vacuum families with 1, 2, 3, 4, and 5 loop momenta and produces the same invariant coefficients. |
| E2E-1 | Existing scalar oracles | Frozen one-loop A/B and two-loop tensor-plus-generated-IBP results remain unchanged after the kernel migration. |

Only after this matrix passes should rank 10 become a default-supported
production claim. Until then, RustRed's honest tensor scope remains generic
vacuum projection through an unvalidated dense rank-eight implementation, with
observed end-to-end evidence only through rank four.
