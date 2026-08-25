# LiteRed example parity acceptance matrix

Date: 2026-08-25

This document makes the published LiteRed notebooks a first-class progress
metric for RustRed.  It supplements, rather than replaces, the generic
algorithm and Vakint-oracle acceptance criteria.  Passing a notebook fixture
must never be achieved with a topology-specific production recurrence.

## Upstream test inventory

The published LiteRed 1.84 archive contains the package source and eight
example notebooks, but no `Tests/`, `VerificationTest`, or regression-suite
tree.  The vendored LiteRed2 Git tree likewise contains source plus
`Examples/example1.nb`, `Examples/example2.nb`, and `Examples/NewDsSet.nb`,
not an independent test suite.  Assertions embedded in the Mathematica source
are implementation invariants, not end-to-end acceptance tests.

The eight old notebooks contain input and explanatory cells but no saved
Mathematica `Output` cells.  They therefore supply exact family declarations
and target expressions, but not complete evaluated target answers.  The
official examples directory also publishes saved basis archives (`b2.zip`,
`p3.zip`, `p4.zip`, `t4.zip`, and `v3.zip`) containing plain-text generated
IBPs, sector tables, symmetry tables, and parametric `jRules`.  These external
GPL artifacts are useful frozen oracles; RustRed does not invoke Mathematica
or FORM to consume them.

Output-bearing notebooks supplied later can add exact target-result snapshots
without changing the fixture identity.

The initial inventory uses the files served by the official
[`Examples/` directory](https://www.inp.nsk.su/~lee/programs/LiteRed/Examples/)
on 2026-08-25:

| Upstream artifact | SHA-256 |
|---|---|
| `LiteRed example 1.nb` | `14ed645b11255a25ee6b12b25844ce4d864033fd89653a72c2f8ab8286fcf8f4` |
| `LiteRed example 2.nb` | `ad2ae7c997cad936a516440466c68fe322345444d7ec6e5bb12c7afc624c1f47` |
| `LiteRed example 3.nb` | `c46ea2c78ddd5d13cca18e13bc2b6567d9ea2361868d88212704d3407507b2b6` |
| `LiteRed example 4.nb` | `8c678e43d2896555809df544c481a46da280313e87987a76a7038f8f259c6225` |
| `LiteRed example 5.nb` | `7a7527cb0a3f76b6243739757f61701dae8a579e842ecb74fec27663ec28393c` |
| `LiteRed example 6.nb` | `2ca636ce31ed44a86242c4cf4533849fca0be4214d9f50b18a0b1e9645043d77` |
| `LiteRed example 7.nb` | `27c3ccdd94fddfea5c74e959d2f06a9abaf57b2c54f260b3666a0a04df4dda4b` |
| `LiteRed example 8.nb` | `fe26d580a69c5b2d1463313dc465a987d2f4979123e2c552bacfe60d8d400a7d` |
| `b2.zip` | `d8a1e0a0a18829817b759579559ca4df9457f0d23a8616a2825437fd093c36cf` |
| `p3.zip` | `aafc10e1ca016b90efd0a9c83bb4ea4fbfe6a9fd94f47e868513ea17d6efd42f` |
| `p4.zip` | `00839cd41ef9c83982a240c391076ba88aa2a0326a004f48209223227c1f1c49` |
| `t4.zip` | `f2a6bd7de6ad49b170a3609e7e9026cda62d0f40e9e8c1568252c8ccb831ec6d` |
| `v3.zip` | `ca80d062104018753763ca13251a348cf733699efc8a81766b2d24349005a709` |

## Status levels

Each example is tracked at cumulative, monotonic levels:

0. **Inventoried**: the upstream notebook, archive URL, hash, family metadata,
   and target cells are identified.
1. **Input parity**: a compact Symbolica or hybrid RustRed fixture normalizes
   to the same loops, external momenta, Gram data, and denominator basis.
2. **Identity parity**: RustRed derives the expected ordinary-IBP and LI row
   counts, and every row passes exact specialization/replay checks.
3. **Sector/symmetry parity**: zero, simple, unique, mapped, internal-symmetry,
   and external/cross-family symmetry data agree up to canonical naming.
4. **Parametric-rule parity**: generic RustRed sector solving derives guarded
   rules equivalent to the saved LiteRed `jRules`, after Symbolica
   normalization and exceptional-locus comparison.
5. **Target-reduction parity**: every notebook `IBPReduce` target reduces to
   the same unreplaced master combination.  Equivalent numerator spellings
   must also close to the same result.
6. **Auxiliary parity**: notebook differential derivatives, dimensional
   recurrences, manual-rule hooks, graph metadata, or master-count stopping
   behavior are reproduced where they are part of the example.

`Inventoried` is not a computational pass.  Reports must give the highest
level actually exercised by automated Rust tests.

## LiteRed 1.x notebook matrix

For a complete affine family with `L` loops and `E` external momenta, the
expected generated row counts are `L*(L+E)` ordinary IBPs and
`E*(E-1)/2` LI identities.

| Example | Family | `L` | `E` | basis size | ordinary / LI | Distinguishing acceptance surface | Current full-example level |
|---|---|---:|---:|---:|---:|---|---|
| 1 | one-loop off-shell massless vertex `v1` | 1 | 2 | 3 | 3 / 1 | external kinematics, a general linear-combination reduction, invariant differentiation, lowering and raising dimensional recurrences | Inventoried; not ported as a complete fixture |
| 2 | two-loop on-shell mass operator `p2` | 2 | 1 | 5 | 6 / 0 | massive/on-shell Gram constraint, sector and graph metadata, scalar target reduction | Inventoried; not ported as a complete fixture |
| 3 | two-loop on-shell vertex `v2` | 2 | 2 | 7 | 8 / 1 | explicit irreducible-numerator coordinate, negative powers, restricted sector analysis, lowering and raising dimensional recurrences | Inventoried; not ported as a complete fixture |
| 4 | three-loop on-shell propagator `p3` | 3 | 1 | 9 | 12 / 0 | ISP numerator conversion, graph/master inspection, manual extra-rule injection, weighted reduction, lowering and raising dimensional recurrences | Inventoried; not ported as a complete fixture |
| 5 | three-loop on-shell vertex `v3` | 3 | 2 | 12 | 15 / 1 | three numerator/ISP coordinates, large external family, lowering and raising dimensional recurrences | Inventoried; not ported as a complete fixture |
| 6 | two-loop box `b2` with parameters `s,t` | 2 | 3 | 9 | 10 / 3 | two numerator coordinates, nontrivial three-external Gram matrix, two kinematic parameters, invariant differentiation of a dotted target | Inventoried; not ported as a complete fixture |
| 7 | four-loop massive tadpole `t4` | 4 | 0 | 10 | 16 / 0 | connected four-loop vacuum family, first coordinate reserved for numerators, full-sector solve, lowering and raising dimensional recurrences | Inventoried; not ported as a complete fixture |
| 8 | three related two-loop vertex bases `v1/v2/v3` | 2 | 2 | 7 each | 8 / 1 each | first coordinate reserved for numerators, explicit external-momentum maps, internal and cross-family symmetries, master-count stopping, mixed-basis negative-power reduction | Inventoried; not ported as a complete fixture |

The current RustRed suite tests several ingredients appearing in this table,
including external-momentum IBP/LI generation, ISPs, symmetry foundations,
vacuum tensor lowering, and concrete vacuum reductions.  Ingredient overlap
does not promote any row above `Inventoried`: none of the eight exact notebook
workflows is currently an end-to-end RustRed acceptance test.

## LiteRed2 notebook matrix

| Notebook | Family | `L` | `E` | basis size | ordinary / LI | Distinguishing acceptance surface | Current full-example level |
|---|---|---:|---:|---:|---:|---|---|
| `example1.nb` | one-loop massive triangle | 1 | 2 | 3 | 3 / 1 | `NewDsBasis`, sector solve and masters, differential systems in three invariants, `IBPReduce`/`FermatIBPReduce` parity | Inventoried; not ported as a complete fixture |
| `example2.nb` | reverse-unitarity `e gamma -> e gamma gamma`, bases `gr1/gr2` | 2 | 2 | 7 each | 8 / 1 each | cut denominators, graph-to-denominator conversion and graph attachment, cross-family symmetry, master-basis changes, differential systems | Inventoried; not ported as a complete fixture |
| `NewDsSet.nb` | related three-loop HQET families | 3 | 1 | 9 for independent bases; 11 indices in overcomplete `HQET5` | 12 / 0 per independent family | dependent-denominator relations, new independent bases, partial-fraction Groebner basis, `PFReduce`, cross-family mapping, then IBP reduction | Inventoried; not ported as a complete fixture |

These three notebooks are tracked separately because LiteRed2 changes both
the API and, for `NewDsSet.nb`, the family model.  In particular, the
dependent/overcomplete partial-fraction path is a known missing RustRed
capability; it must not be reported as passing merely because independent
affine-family generation works.

## Oracle policy

- Notebook and archive URLs plus SHA-256 hashes define upstream fixture
  identity.  Large saved LiteRed rule archives stay external unless their
  licensing and repository role are made explicit.
- Family translations are reviewed mechanically against the notebook cells;
  inferred momentum conventions are not silently accepted.
- Relation and reduction comparisons canonicalize coefficients with public
  Symbolica APIs.  RustRed must not grow a parallel Mathematica evaluator or
  CAS merely to read an oracle.
- Parametric-rule comparison includes guards and exceptional loci, not just a
  generic-point coefficient equality.
- Target comparisons leave masters unsubstituted and canonicalize sector and
  symmetry mappings before comparing names.
- Missing output cells remain `oracle pending`.  RustRed output is never used
  as its own expected result.
- All production code exercised by these fixtures remains topology- and
  loop-count independent.  Notebook-specific data belongs only in fixtures.

## Implementation order

1. Check in a versioned metadata manifest and compact Symbolica translations
   for examples 1--8 and the three LiteRed2 notebooks.
2. Promote every family through input and identity parity; these are fast,
   parallel tests and do not wait for full sector solving.
3. Import structural sector/symmetry census data from the saved archives as
   opt-in oracle fixtures with stable hashes.
4. As generic `Ready -> WhenBad -> publication -> sector iteration` lands,
   promote examples in increasing cost order and compare selected saved
   parametric rules.
5. Add exact result snapshots from output-bearing notebooks when supplied,
   then enable target-reduction and dimensional/differential-recurrence parity.
