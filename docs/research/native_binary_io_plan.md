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
An explicit optional import policy normalizes each unique table entry once
using Symbolica itself. It is useful for measuring that cost or checking a
producer without a normalization guarantee; it does not harden the native
parser against hostile bytes.

## Implementation sequence

1. Add only the new core persistence files and this design while the current
   frontend milestone is being checked. Do not race its Cargo or module edits.
2. After that checkpoint, enable Symbolica's `bincode` feature and the ordinary
   bincode 2 dependency, update the lockfile, and wire the generic core module.
3. Replace candidate string records with coefficient IDs and native records.
   Retain all provenance and semantic admission. The evolving RustRed format
   may break compatibility; a one-off migration tool is not a permanent shim.
   The initial candidate vertical slice retains the small family source as its
   geometry reconstruction input while eliminating coefficient expression
   parsing. Follow it with a native generic family record so source text becomes
   provenance only. This intermediate slice is not the completed uniform I/O
   migration, and the certified proof codec remains unchanged until step 6.
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

## Certified-artifact migration: implementation and replay boundary

The certified codec lives in
`crates/rustred-core/src/foundry/artifact/persistence/`. Its current transport is
already binary, but `coefficient.rs` writes each sparse numerator/denominator
and arbitrary-precision integer separately. The migration should replace that
algebra transport, not replace the derivation plans or mathematical verifier.

A bounded implementation slice is:

1. Bump the evolving artifact schema. Use the same universal envelope with
   `BinaryProgramKind::Certified`, one Symbolica state section, one native
   coefficient table and one structural program section. The existing private
   metadata/family/source/rule/terminal record grammar can remain inside the
   structural section; there must not be a second public outer file format.
2. In `binary.rs`, share an interner between `Writer` children and a decoded
   table between `Reader` children. Nested dependency artifacts and opaque
   proof snapshots participate in the same table. Preserve current aggregate
   structural and replay-work budgets; charge native algebra bytes per unique
   table entry rather than once per reference.
3. Replace the coefficient functions in `coefficient.rs` with typed table
   references. Base values must match the reconstructed base variable map;
   indexed values must match the regenerated indexed map before receiving an
   indexed-context seal. Polynomial references additionally require a unit
   denominator. Native structural validation is not a substitute for these
   use-site context checks.
4. Preserve exact integers in affine primitive matrices. The current
   `source_port/plans.rs` calls `encode_integer` and `decode_integer` for those
   entries. They can use native constant rational-polynomial entries on the
   empty variable map, with explicit constant/integer admission, rather than
   retaining a private arbitrary-precision integer codec.
5. Keep ordinary-source regeneration, translated-source requests, original
   domain/cell replay, exceptional guards, descent, zero-sector proofs,
   factorization/dependency witnesses, supported root bounds, homogeneity and
   terminal-cover installation unchanged. A certified envelope is a request
   to replay these proofs, never itself a proof.

The principal production files are `persistence/{mod,binary,coefficient}.rs`,
`persistence/{k6,source_port,two_loop}.rs`, the limits and error definitions, and
`artifact/model.rs` for the schema and public load documentation. Most of
`family.rs`, `semantic.rs` and `source_port/plans.rs` should retain their existing
mathematical structure and call the shared transport seam. This is independent
of the application-level candidate codec.

### Comparing replayed mathematics without comparing ambient state

The existing loaders compare encoded source/rule snapshots and, for several
grammars, the complete regenerated artifact against the incoming bytes. They
must not compare complete native files after this migration: Symbolica state
IDs and unrelated registry entries depend on process history.

Use two distinct checks:

- For local source/rule snapshot checks, the replay writer may use a lookup
  built from the decoded coefficient table. A lookup hit must compare the full
  native coefficient, including its ordered variable map; a serialized integer
  ID is never accepted as evidence of equal coefficients. Duplicate equal table
  values require an explicit policy, not an assumption that interning will
  preserve their IDs. The simplest generated-format policy rejects duplicate
  entries as noncanonical before replay. Missing replay values fail closed.
- At the final artifact comparison, independently serialize the reconstructed,
  installed artifact with a **fresh**, first-occurrence interner. Compare its
  structural program with the incoming structural program and compare their
  ordered coefficient tables by native mathematical payload, not by packed
  Atom bytes or State bytes. This independent comparison must check table
  lengths and every entry. It therefore rejects unused extra entries, reordered
  or duplicate entries that violate the generated canonical dictionary, and
  changed coefficients even if local replay used the input lookup. The input
  table must not seed this final independent encoder.

The fast generated-data precondition supplies normalized coefficients; native
coefficient equality includes exact numerator, denominator and variable order.
Tests of a deliberately nonnormalized producer should use the explicit native
normalization path rather than silently claiming representation equality means
general rational-function equivalence. No new GCD should run for every ID use.

Alternatively, a future typed semantic record comparator could admit arbitrary
dictionary ordering by resolving every reference. That is a larger change and
is not necessary for the first generated-format migration. Do not drop existing
source, rule or terminal witnesses merely to avoid handling the comparison.

### Certified acceptance tests

Retain the current K1, K3, K6 and generic source-port replay tests, including
coordinate and affine cases and bounded root scopes. Add fresh-process loading
with unrelated/reordered native symbol state. Compare exact reductions and
family/ordering/scope/terminal ownership, not whole native files.

Explicit mutations must still reject: a candidate envelope presented as a
certificate; an altered source coefficient; a wrong base/indexed context ID;
a rational-valued polynomial guard; an altered source weight or translation;
changed affine conjunction grouping; a swapped rule coefficient reference;
wrong nested dependency binding; an added or removed terminal; and changed root
or mass-homogeneity metadata. Duplicate dictionary entries and unused entries
must exercise the stated canonicality policy. A claimed `Certified` kind cannot
bypass ordinary-source replay or the closing installer.

## Follow-up experiment: native factorized rational coefficients

The current applier uses Symbolica's ordinary
`RationalPolynomial<IntegerRing, u16>`, not its
`FactorizedRationalPolynomial<IntegerRing, u16>`. The latter stores an expanded
numerator and scalar contents together with denominator factors and their
multiplicities. It is not a fully factorized expression representation.

The current public API already provides
`FromNumeratorAndFactorizedDenominator::from_num_den`, native factorization,
addition, multiplication and exact cancellation. Addition aligns matching
denominator factors and tests divisibility after summing numerators;
multiplication cancels against factors before multiplying numerators. There is
no need for RustRed to implement this algebra. Initial conversion should request
native factorization once per admitted coefficient, not once per accumulation.

An internal coalescing/cache representation is a plausible future experiment:
keep factored coefficients through repeated additions and products, then use
native polynomial operations to return the existing ordinary coefficient at a
public result or persistence boundary. Repeated conversion back and forth could
erase any benefit. The observed large number of coalescing additions motivates
measurement but is not evidence of a speedup.

This is not a drop-in global type replacement: the current Atom coefficient
enum has no factorized-rational variant, factor ordering is not a canonical
mathematical identity, and the native `InternalOrdering` implementation is
currently unfinished. The numerator can still swell. Any experiment needs
exact cancellation/zero, repeated factors, differing factors, constant signs,
context-map, master-coefficient parity, wall-time and peak-memory tests against
the unchanged ordinary path. It remains after the binary I/O and EPSILON
terminal-deduplication work, not part of their acceptance boundary.
