# Independent P102 exact witness audit

The source of this regression is the public external-data repository, pinned to
commit `aca9b1d324044165c2981213a920d6828d679dc0`. `verify_p102.py` checks the SHA256
of all four unmodified DOT files and the model before using native HEPKit ingress.
This is test/research data, not a P102-specific rule in RustRed's solver.

Download these five files from
`https://raw.githubusercontent.com/alphal00p/vakint-external-data/aca9b1d324044165c2981213a920d6828d679dc0/`:

| Upstream path | Local filename |
| --- | --- |
| `5L/model.json` | `model.json` |
| `5L/masters/P102/graph.dot` | `P102.dot` |
| `5L/masters/P106/graph.dot` | `P106.dot` |
| `5L/masters/P162/graph.dot` | `P162.dot` |
| `5L/masters/P86/graph.dot` | `P86.dot` |

Run with a Python environment providing Symbolica and HEPKit:

```bash
python -B tools/research/numerator_symmetry/verify_p102.py \
  --inputs /absolute/path/to/downloaded/files \
  --allow-model-fingerprint-adapter
```

The last option is deliberately explicit. With the installed Symbolica 3.0.1
community build, the loaded pinned model has fingerprint `2ce7c32b…`, whereas the
saved DOTs carry `67fadf26…`; unmodified native ingress correctly rejects this
serialization mismatch. The research driver first checks the exact source-model
hash, particle identity/PDG/spin/color/mass/width, scalar propagator and interaction
IDs. It then replaces **only** that metadata field in memory. Actual downloaded
DOTs, routing, `lmb_id`, numerator, graph, masses and factors are untouched. The
new and old metadata fingerprints are printed in the result. This is not an
automatic production bypass of model authentication.

## Stored basis and exact identity

The native P102 basis is `(a,b,h,c,q)=(l0,l1,l2,l3,l4)`. Its chord edge IDs are
`[1,2,4,5,6]`; edge 1 has `lmb_id=0` and its denominator **is present**. The seven
denominators, in physical-edge order, are

```text
Dp = (q-a-b+c)^2-m², Da=a²-m², Db=b²-m²,
Dhq=(h+q)²-m², Dh=h²-m², Dc=c²-m², Dq=q²-m².
```

Let `p=q-a-b+c` and `r=(p,a,b,-c)`, so `sum(r)=q`. Every permutation `pi` defines
the exact source-to-target loop map

```text
a' = r[pi(1)], b' = r[pi(2)], h' = h,
c' = -r[pi(3)], q' = q, p' = r[pi(0)].
```

The audit checks all 24 integer matrices through Symbolica's matrix determinant
and exact HEPKit scalar products. Every determinant is `+1` or `-1`, so the
absolute integration Jacobian is one in dimensional regularization. Every map
permutes the full seven-denominator multiset. No pointwise numerator invariance
is assumed: the script explicitly checks that `N` differs from its orbit average.

For the **complete** stored numerator `N=(a+b-c)^2-m²=(q-p)^2-m²`, the exact orbit
average is

```text
average(N) = q²/2 + (Dp+Da+Db+Dc)/4
           = Dq/2 + m²/2 + (Dp+Da+Db+Dc)/4.
```

Only after integration against the invariant weighted denominator product can
`N` be replaced by this average. Each of the four `r`-line pinches has the same
integral. Therefore

```text
P102 = pinch(p) + pinch(q)/2 + m² P106/2
     = P162 + P86/2 + m² P106/2.
```

All graph overall factors, symmetry factors, numerator prefactors and projectors
are exactly one. This identity is for the usual scalar-integral normalization,
with a common five-loop measure; it is not an equality between full Feynman-model
amplitudes carrying an extra propagator `i` per remaining edge. At unit mass its
coefficient identity is `P102=P162+P86/2+P106/2`. All terms have mass dimension
`5d-12`; scalar P106 itself has dimension `5d-14`.

## Exact representative routing, not graph-isomorphism inference

The script checks each remaining source denominator against the native target
denominators, including the mass term and signs:

| Pinch | Source loops in the target's stored basis `(t0,…,t4)` | Source edge → target edge |
| --- | --- | --- |
| `p` → P162 | `(t0,t2,t1,t4,-t3)` | `1→0, 2→3, 3→1, 4→2, 5→5, 6→4` |
| `q` → P86 | `(t0,t1,t2,t3,-t2-t4)` | `0→0, 1→1, 2→2, 3→5, 4→3, 5→4` |

Both matrices have determinant `-1`. P106 has exactly the P102 graph edges and
stored routing, but numerator one. Pinching `p` contracts its endpoints: the
remaining `a,b,c` lines become three tadpole factors, while `h,q,h+q` form the
two-loop sunset. The measure is still **five-loop**, not four-loop. Pinching `q`
gives the six-line five-loop banana. These conclusions use the explicit momentum
witnesses in addition to graph structure.

## Controls and limits

- Unequal masses among the four permuted lines invalidate the full `S4` action.
- Unequal powers among those lines invalidate its weighted stabilizer. A smaller
  stabilizer may remain useful; equal masses alone are insufficient.
- Powers/masses on `h`, `h+q`, and `q` can differ from the four `r` lines provided
  the map actually leaves their weighted denominators fixed. The regression uses
  the published unit powers and common mass.
- The complete numerator is expanded by native Symbolica/HEPKit operations. No
  assumption that a complete numerator factor already equals a propagator is
  required.
- This audit does not install an alias into a live campaign, establish minimality,
  or claim that external P-names are native family identities. The implementation
  must retain native fingerprints, signed indices, exact map/proof provenance,
  and all established published identities.

The existing core authority to reuse is `sector::symmetry::verify`, including its
independent bilinear denominator replay, then
`sector::symmetry::integral_transport::compile/Prepared::transport` for complete
inactive-coordinate numerator expansion. `verify` alone accepts generic maps;
the caller must additionally enforce the **weighted active-sector stabilizer**.

## API audit and executable result

The pinned Symbolica source exposes `Matrix::det`, exact expression expansion and
simultaneous `replace_multiple`; its Python matrix constructor is
`Matrix.from_nested`, not item assignment. Existing HEPKit
`Kinematics.scalar_product` expands bilinear expressions in declared momenta.
The source/examples/tests and existing RustRed verifier were inspected before the
regression driver was written. Executable probes and the full audit passed on
the existing Symbolica 3.0.1/HEPKit Python build, without a solver rebuild.

One installed native tensor-view limitation was separately observed:
`diagram.numerator_expression(in_lmb=True)` rejects the serialized `LoopMomentum`
function as missing tensor flags. The audit does not suppress that error or
pretend this path works: it reads the native diagram's complete scalar numerator
and makes an exact simultaneous `LoopMomentum(i,mink(d)) → K(i,mink(d))` coordinate
conversion before comparing it to HEPKit's scalar-product polynomial. That check
is exact. It is independent of the core symmetry-transport regression needed by
the production feature.

## Binding the actual saved campaign, not only a constructed test family

`common_family_gate.rs` independently cold-loads an existing publication through
the public `rustred_app::load_master_reduction` API. It does not generate rules,
start a campaign, refine masters, or write back to its input. Its output contains
the authenticated full source-family fingerprint, the native mapping result,
raw-terminal membership, the presently published exact substitution, and a
manifest digest checked before and after the operation.

The pinned native test was run against the immutable generation-119 publication
`e9b2c25c749f937723c8d8412721e90b8e04a2f2425362cc36d3ca5ed63a714a`
of `five-loop-minimal-r0-d10-incremental`, with 939 raw keys and 904 retained basis
keys. The exact saved-family fingerprint's SHA256 was
`44b030daa664fca0c71f4bb930624b098348f6184bfe46ca3ad83b86ef71df3b`,
matching the external-data family identity. This digest comparison is additional
provenance; the mathematical test uses the complete native family and exact
denominator equations, not this label.

The stored `common_basis_to_graph_loops` matrix is replayed as `k_common=M*l_graph`:

```text
M = [[ 0,-1, 0, 1,0],
     [-1, 0, 0, 1,0],
     [-1,-1, 0, 1,1],
     [-1,-1, 0, 1,0],
     [-1,-1,-1, 1,0]].
```

The generic verifier authenticates **all fifteen** induced denominator rows.
The physical common-family coordinates `[2,6,9,11,12,13,14]` map to the seven DOT
denominators above; negative coordinate 3 maps to the complete numerator
`(a+b-c)^2-1`. Native integral transport then proves the exact index identity

```text
common: [0,0,1,-1,0,0,1,0,0,1,0,1,1,1,1]
   DOT: [1,1,1, 1,1,1,1,-1,0,0,0,0,0,0,0].
```

The common key **was present** in the actual raw-terminal set, and its published
reduction was still the identity with coefficient one. That application map
carried 108,524 inherited nonzero conditions; the audit does not remove them or
mistake them for new conditions on the symmetry itself. Thus this P102 is a
genuine currently retained key, not merely a synthetic example or one already
eliminated by the saved refinement.

For an independent helper build, compile the source against the **same matching
RustRed/core/app/dependency rlibs** used by the chosen optimized backend. The
measured build used `-C opt-level=0 -C lto=thin -C codegen-units=4` for the small
driver and optimized existing libraries. Rustc requires `-L dependency=...`, the
matching GMP/Blake3 native search paths, and `--extern` arguments for `rustred`,
`rustred_app`, `serde_json`, and `blake3`. Do not arbitrarily mix rlibs from
different backend builds. Once linked, invocation is simply:

```bash
/absolute/path/to/common_family_gate /absolute/path/to/pinned/publication
```

On the recorded host, a read-only helper invocation constrained to four permitted
CPUs took **368.801 seconds** wall time, 355.657 user CPU seconds, 8.455 system CPU
seconds, and 7,349,232 KiB peak RSS. This is a **cold publication load and exact
mapping-validation boundary, not numerator-symmetry search time**. A two-second
sample late in the run placed the work in
`TerminalCollectionPlan::from_native_bytes → codec::record → Intern::conditions`,
including exact coefficient validation and atom interning. No speed claim about
the new search can be drawn from this load receipt.

The observed manifest BLAKE3 remained
`4dc25b431404712b7311546c675e3ea22b10d349a2ee94eb47c54f2f39cee6c1`.
The original large campaign and its moving publication pointer were not changed.
