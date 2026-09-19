# Native Symbolica binary programs

## Goal and scope

Replace candidate-program coefficient strings and TOML parsing with a compact,
topology-independent binary container using Symbolica's native Atom binserde
and shared State context. Preserve the current rule applier, every source seed,
case, guard, ordering, terminal and family binding. Candidate data must remain
explicitly uncertified. No IBP generation is needed to migrate the saved
four-loop programs.

The first vertical slice is candidate I/O. After its correctness and measured
benefit are established, move certified artifacts onto the same envelope and
native coefficient table in a separate cohesive slice. Their existing compact
sparse-polynomial codec is already binary: do not replace it with expanded
expression trees, or promote candidates merely because their envelope is
binary. Existing certificate grammars can retain explicit mathematical limits;
the transport itself must not assume vacuum graphs, unit mass, one parameter,
sixteen indices, or a particular loop count.

This work is separate from terminal deduplication and changes no tensor reducer,
replacement engine, scalar application algorithm or Vakint default backend.

## API audit and representation

The workspace uses Symbolica 3.0.0 from `vendor/symbolica`. Definitions were
checked against its import/export tests and GammaLoop's existing state-aware
binary callers. Relevant public APIs are:

- `Atom::to_num`, `AtomView::write` and `Atom::import_with_map`;
- `Atom: bincode::Encode + Decode<C: HasStateMap>` with the `bincode` feature;
- `State::export_partial`, `State::import`, `StateMap` and `HasStateMap`;
- `NumView::get_coeff_view` and
  `CoefficientView::RationalPolynomial(...).deserialize()`.

RustRed's coefficient type is precisely Symbolica's
`RationalPolynomial<IntegerRing, u16>`. Wrap it as a native numerical atom:

```rust
let mut atom = Atom::new();
atom.to_num(symbolica::coefficient::Coefficient::RationalPolynomial(value));
```

Use `to_num`, not `Atom::num`: the latter collapses zero and loses its
rational-polynomial variant and variable map. This numerical atom stores native
sparse coefficient/exponent arrays, not an expanded symbolic expression.
Decoding reconstructs those arrays without printing, expression parsing or GCD.
When imported variable-list IDs differ, Symbolica may first decode and repack
the coefficient while remapping it; measure that linear work before seeking
an upstream fast path.

Direct bincode derivation on the rational-polynomial struct is not available:
the pinned `IntegerRing` lacks the required traits and `RationalPolynomial`
has no direct implementation. The numerical Atom route already supplies the
needed native functionality. RustRed must not implement another integer,
polynomial, variable-table or rational-reconstruction codec.

Native state export has two qualifications. `export_partial` filters ordinary
symbols but includes all registered variable lists and finite fields, together
with their dependent symbols. It is not a strictly reachable-only snapshot.
Also its IDs depend on prior process state. Export state only after creating
all dictionary atoms, which register their variable lists. Use the native API
once per program and record the actual state-section size on the four saved
assets. Request a narrower upstream API only if measured overhead warrants it;
never reset global Symbolica state inside Vakint.

The current native Atom bincode format writes a `usize` length but reads eight
bytes. The first transport version explicitly supports 64-bit hosts; do not
claim cross-word-size portability without a native API change and tests.
Semantic portability across different symbol/resource registration orders is
mandatory, but byte-for-byte canonicality across dirty processes is not claimed.

## Container and ownership

A new `rustred::persistence` module owns a versioned, bounded envelope and one
native coefficient dictionary. Application frontends only steer it. The
envelope has a distinct candidate/certified kind and length-delimited sections
for family data, provenance, ordering/domain data, rules/terminals, native state,
native coefficient atoms and optional proof or alias witnesses. It carries no
topology dispatch. Sections can be inspected as borrowed slices.

Coefficient references are compact integer IDs. Intern exact native numerical
atoms by structural equality; hash collisions must still compare full atoms.
Use deterministic first-occurrence IDs in the existing canonical sector/rule
order, without treating native bytes as a process-independent mathematical
fingerprint. Decode each distinct coefficient once, then pass native values
to the unchanged applier. Any remaining clones required by its existing
ownership model are measured separately, not hidden under a zero-copy claim.

The generic family record must represent loop and external momentum labels,
coefficient parameters, dimension, affine denominator matrix/constants,
external Gram data and power shifts. Keep original user input as provenance,
not the only way to reconstruct runtime family geometry. The initial frontend
may still admit only its existing const-generic arities; that is separate from
the transport schema.

Optional future terminal aliases retain raw declared keys and contain explicit
same-family source/representative/momentum-matrix witnesses. They are not
unvalidated pairs and do not alter current default output during this slice.

## Admission and native trust boundary

Check outer bytes, section/frame lengths, entry counts, state bytes, individual
and aggregate atom bytes before allocation or native calls. Check the native
Atom length against its containing frame, require exact consumption, reject
unexpected atom/coefficient variants, and retain existing map, sparse-layout,
denominator, case, key, sector and ordering validation. Perform admission once;
do not add hot-path rehashing or repeated authentication.

Native Symbolica import is a trusted-generated-data decoder, not a hardened
hostile-input parser. In particular, State import and packed polynomial import
can allocate from inner counts before discovering truncation. Outer byte caps
and panic handling do not prove allocation safety for malicious native blobs.
The new API and documentation must state this limitation rather than claiming
otherwise or duplicating Symbolica's private format parser in RustRed.

Packed rational-polynomial decoding also preserves, rather than proves,
coprimality. The fast path accepts native payloads written from RustRed's
normalized generated coefficients and retains exact structural validation;
it must not claim that a shape check proves coprimality of arbitrary forged
arrays. A future fully adversarial native boundary belongs in a bounded
Symbolica API. No cryptographic authentication ceremony is introduced here.

## Implementation sequence

1. Add only the new core persistence files and this design while the current
   frontend milestone is being checked. Do not race its Cargo or module edits.
2. After that checkpoint, enable Symbolica's `bincode` feature and the ordinary
   bincode 2 dependency, update the lockfile, and wire the generic core module.
3. Replace candidate string records with coefficient IDs and native records.
   Retain all provenance and semantic admission. The evolving RustRed format
   may break compatibility; a one-off migration tool is not a permanent shim.
4. Migrate the already saved H, FG, BMW and X programs without discovery.
   Cold-load in separate processes and compare complete native rules, guards,
   sources, terminals and exact canary reductions against the saved originals.
5. Switch Vakint's shipped programs and thin loader only after those checks.
   Repeat all fifteen numerical references and sixteen propagator-pinch pairs
   with the invalid-FORM RustRed lane; keep FMFT and all default APIs unchanged.
6. Move certified artifacts to the same envelope/native table, preserving
   their proof/replay ownership. Then pursue the separately approved terminal
   deduplication and later five-loop work.

## Tests and measured acceptance

Focused tests cover zero/constant/general rational-polynomial values, large
integers, mixed base/index variable maps, deduplication, repeated coefficients,
state export after table construction, wrong maps, bad IDs, duplicate sections,
unsupported status/version/word size, truncation and trailing bytes. Native
function attributes and namespaces must survive transport; incompatible
attributes fail closed with `conflict_fn=None`.

Fresh-process tests deliberately register unrelated symbols, finite fields and
polynomial variable lists in different orders before loading. Compare exact
decoded coefficients and reductions, not merely counts or native bytes. Tests
must never call unsafe global `State::reset` inside a shared test process.

For each of the four existing assets, record old/new encoded and compressed
sizes, native state/table sizes, unique coefficient counts, cold wall/CPU time
and peak RSS. Separate input/decompression, envelope/native decode, family/zero
preparation, existing applier construction and first reduction when the
instrumentation supports that boundary. Compare the same release build,
affinity, inputs and reductions, with no compilation or IBP regeneration in
the timed load. The H cold-loader observation near nineteen seconds and X's
large resident peak are the immediate regressions to improve; a small state
section must not be mistaken for the dominant gigabyte-scale text overhead.

Every slice receives an independent API/implementation audit and semantic
round-trip checks. Report native decoder limitations, incomplete migrations
and measured performance honestly before committing/pushing a coherent
milestone.
