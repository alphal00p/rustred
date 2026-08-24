# Induced three-loop F5 D2/N1 manifest

Date: 2026-08-12

## Outcome

The selected 123-seed four-loop shell induces 31 nonzero local `F5,D2/N1`
branches at each of three finite-field images.  They belong to 31 distinct
parent boundary keys, but collapse to nine labelled six-entry F5 power vectors
and six orbits under the true F5-mask stabilizer.

Those six orbits are the complete `F5,D2/N1` orbit domain: before symmetry
there are 15 possible dot placements on the five active lines, and their
stabilizer quotient has six elements.  Consequently a target-driven exact
service may freeze six oriented targets, but it cannot omit an F5 D2/N1 orbit.

This is **modular discovery evidence**, not an exact reduction certificate.
The parent inventory and the nonzero affine-branch decisions were observed in
finite fields.  Production must reconstruct every parent map and coefficient
over `Q(d,m2)` and replay its exact provenance.

## Probe

The manifest mode is in
[`four_loop_next_shell_rank.rs`](../../tools/four_loop_next_shell_rank.rs):

```text
rustc --edition=2024 -D warnings -O \
  tools/four_loop_next_shell_rank.rs -o /tmp/four_loop_next_shell_rank

/tmp/four_loop_next_shell_rank 1000003 17 --f5-d2n1-manifest
/tmp/four_loop_next_shell_rank 1000033 29 --f5-d2n1-manifest
/tmp/four_loop_next_shell_rank 1000037 37 --f5-d2n1-manifest
```

Every incidence line retains:

- the H-family parent power vector and numerator position;
- the factor component index, reference offset, global basis slots, and active
  parent positions owned by that component;
- the lowered local reference position;
- the labelled six-entry F5 target; and
- its canonical six-entry target.

Thus the 31 parent incidences remain distinguishable even when they request
the same local F5 integral.

Here an incidence means one nonzero owned F5 branch from a unique boundary
key.  The probe's elimination-column inventory deduplicates repeated
appearances of that key in different raw rows; raw-row multiplicity remains a
separate production manifest field.

## Three-image comparison

The complete incidence support, including ownership metadata, was identical
at all three images:

| `(p,d)` | branch incidences | parent keys | labelled targets | canonical targets | support checksum |
|---|---:|---:|---:|---:|---|
| `(1000003,17)` | 31 | 31 | 9 | 6 | `7dc254a84e41920b` |
| `(1000033,29)` | 31 | 31 | 9 | 6 | `7dc254a84e41920b` |
| `(1000037,37)` | 31 | 31 | 9 | 6 | `7dc254a84e41920b` |

The checksum is deterministic FNV-1a over the printed support keys.  It is a
convenient comparison aid, not a cryptographic certificate or a substitute
for exact replay.

All parents have topology `H` and product `T1^1*F5^1`.  The F5 ownership split
is:

| component ownership | incidences |
|---|---:|
| component 0, reference offset 0, global slots `[0,1,2]` | 19 |
| component 0, reference offset 0, global slots `[0,2,3]` | 6 |
| component 1, reference offset 1, global slots `[1,2,3]` | 6 |

The parent numerator positions occur with multiplicities `6 -> 6`, `7 -> 20`,
and `9 -> 5`.  Every retained F5 branch lowers completed local position 5, the
inactive sixth tetrahedron line.

The nine labelled targets and their parent-incidence multiplicities are:

| labelled local powers | incidences |
|---|---:|
| `[1,1,1,1,3,-1]` | 2 |
| `[1,1,1,2,2,-1]` | 7 |
| `[1,1,1,3,1,-1]` | 4 |
| `[1,1,2,1,2,-1]` | 2 |
| `[1,1,2,2,1,-1]` | 4 |
| `[2,1,1,1,2,-1]` | 3 |
| `[2,1,1,2,1,-1]` | 7 |
| `[2,1,2,1,1,-1]` | 1 |
| `[3,1,1,1,1,-1]` | 1 |

After joint orientation of all six powers, the six targets are:

| canonical local powers | parent incidences |
|---|---:|
| `[1,1,1,1,3,-1]` | 6 |
| `[1,1,1,2,2,-1]` | 7 |
| `[1,1,2,1,2,-1]` | 2 |
| `[1,1,2,2,1,-1]` | 4 |
| `[2,1,1,1,2,-1]` | 11 |
| `[3,1,1,1,1,-1]` | 1 |

## Stabilizer proof boundary

The stabilizer computation itself is exact integer combinatorics.  The probe
enumerates every permutation of the five active F5 lines while fixing the
inactive sixth line.  Because routing positions 0, 1, and 2 are the coordinate
basis, the eight possible sign choices for their advertised images exhaust
the candidate loop maps.  A candidate is retained only if its determinant is
`+/-1` and it maps all six tetrahedron routings to the advertised lines up to
sign.

The resulting power-permutation action has order four:

```text
[0,1,2,3,4,5]
[0,2,1,4,3,5]
[0,3,4,1,2,5]
[0,4,3,2,1,5]
```

The probe prints one unimodular loop-map witness for each permutation.  A
simultaneous reversal of every loop momentum lies in the kernel of the action
on denominator powers, so it does not create another power permutation.

The exact combinatorial domain contains five triple-dot placements and ten
two-double-dot placements.  Canonicalizing all 15 under the four permutations
above gives exactly the same six representatives as the induced modular
manifest.  This orbit-completeness statement is checked on every invocation.

## Production consequence

`ThreeLoopF5D2N1Reducer` now provides an exact initial service for all 15
labelled inputs.  It deterministically rebuilds the complete authenticated
`(D,N)=(2,1)` three-loop pipeline, checks the order-four stabilizer, validates
all 135 target IBPs, and rebuilds on replay.  Its focused exact test passes in
about 196 seconds.  This is a rebuild oracle, not a compact persisted proof:
the generic table exposes neither source-row weights nor exceptional factors.

A previously reported follow-up claim that six inputs and 54 rows gave rank 42
with paw-only target normal forms is retracted.  The standalone probe omitted
a factor of two from off-diagonal derivative contractions.  The corrected
system has 65 columns, rank 35, and 30 free coordinates at all three images;
every target retains genuine F5/B4 support.  The native RustRed generator and
an independent exact derivative expansion agree and correctly reject the
false compact closure.  The 306-row exact rebuild oracle above remains the
certified service.

The complete four-loop integration must still:

1. freeze the exact 31-key owned-branch manifest, every raw-row occurrence,
   and the exact affine maps;
2. authenticate the F5 family and the stabilizer witnesses;
3. replay native rows and exact source-row weights for all six targets;
4. close reached proper sectors and scalar B4 D2 terms with existing exact
   services; and
5. record every exceptional factor in `d`.

Until the exact parent manifest and component dispatcher compose the landed
oracle into every occurrence, the four-loop higher boundary remains partial.
The six canonical targets are covered lower-loop requests, never masters.
