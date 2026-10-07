# Three-loop K6: 38 terminal keys, five integral types

The three-loop notebook's certified artifact has **38 labelled terminal keys**,
not 38 independent master integrals. Every key is a scalar sector corner with
powers zero or one. Equivalent momentum routings were left as separate keys
in that artifact. The existing native terminal normalizer identifies 33 unit
aliases and leaves five representatives; no additional IBP generation is
needed to find these equivalences.

## Names and conventions

Write `I(a1,a2,a3,a4,a5,a6)` with denominators, in order,

```text
k1² − m², k2² − m², k3² − m²,
(k1−k3)² − m², (k1−k2)² − m², (k2−k3)² − m².
```

Zero means a pinched denominator, not a massless propagator. All present
propagators have the same nonzero mass. The table uses the topology labels of
[Lee, Fig. 2](https://arxiv.org/pdf/1203.4868); numerical normalization and
Euclidean/Minkowski sign conventions must still be matched before borrowing
values from a publication.

| Label | Integral type | One representative | Equivalent raw keys |
| --- | --- | --- | ---: |
| `T3,1` | Product of three one-loop tadpoles | `I(0,0,1,0,1,1)` | 16 |
| `T4,1` | Equal-mass two-loop sunset × one-loop tadpole | `I(0,0,1,1,1,1)` | 12 |
| `T4,2` | Four-line, three-loop basketball (vacuum banana) | `I(0,1,1,1,1,0)` | 3 |
| `T5,1` | Five-line connected vacuum, a pinched Mercedes | `I(0,1,1,1,1,1)` | 6 |
| `T6,1` | Six-line tetrahedron / Mercedes | `I(1,1,1,1,1,1)` | 1 |

Thus there are three nonfactorizing three-loop representatives and two lower-loop
products. For example, `I(1,1,1,0,0,0)` and `I(0,0,1,0,1,1)` are both three
independent tadpoles after a unit-Jacobian change of loop variables. No new
numerical master value is needed for the second spelling.

All 38 keys are listed below as six digits in the denominator order above.
Every key in a row equals that row's representative with coefficient one.

```text
T3,1: 001011 001101 001110 010011 010101 010110 011010 011100
       100011 100101 100110 101001 101010 110001 110100 111000
T4,1: 001111 010111 011011 011101 100111 101101
       101110 110011 110110 111001 111010 111100
T4,2: 011110 101011 110101
T5,1: 011111 101111 110111 111011 111101 111110
T6,1: 111111
```

## What was checked

On 2026-10-07, a native graph-derived K6 candidate was generated and passed to
the existing `IBPFamily.normalize_candidate_terminals` service. Its raw terminal
set matched all 38 keys of the previously cold-loaded certified artifact.
The normalizer returned `unique_raw_terminals = 38`, 33 unit aliases, five canonical
terminals and no skipped shapes. All 38 returned relation coefficients were
checked to be one. This is exact routing/factorization normalization, not a
numerical coincidence or a separately implemented Python algebra kernel.

The certified source-input artifact and the graph-derived candidate use
different internal symbol/family identities. A normalizer must receive the
candidate from its own `IBPFamily`; it must not bypass identity checks merely
because the displayed graphs agree. The notebook retains its original raw
certificate and labels that count explicitly. Its displayed raw reduction
terms have not silently been replaced by a different certificate.

Closure and minimality are separate claims. The normalizer does not prove
linear independence of the five values. Vakint's conventional master symbols
also need not equal these representatives individually: notably, its five-line
corner is expressed using `miD5` and lower-topology contributions. A topology
name is not a license to substitute a differently normalized master value.
