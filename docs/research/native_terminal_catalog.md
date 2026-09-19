# Native exact terminal-value catalogs

## Scope and authority

This completes the small remaining persistent-algebra boundary identified after
the candidate and certified IBP-program native-I/O migrations: Vakint's four-loop
offline terminal-value catalogs. These are finite maps from typed integral keys
to exact expressions in the existing FMFT PR basis. They are not rule sets,
proofs of closure, or demonstrations that the master basis is minimal.

The reusable implementation lives in RustRed's `persistence::ExactTerminalCatalog`.
It has no assumptions about vacuum graphs, loop count, dimension-symbol spelling,
or index magnitudes. Values are arbitrary exact Symbolica Atoms, including
functions; they are not converted into rational polynomials. Vakint's wrapper
only resolves values offline and steers RustRed encoding/loading. Ordinary
evaluation never invokes FORM or regenerates IBPs or terminal values.

This change is distinct from the algebraic terminal-alias service. Dictionary
deduplication compresses identical supplied values without changing any of the
1,155 declared terminal keys. It neither derives new integral identities nor
alters the scalar applier. Existing certified K1/K3/K6 programs and their small
source-defined master formulas are unchanged; unrelated FMFT numerical tables
are not being redesigned.

## API and representation

The API comprises:

- `ExactTerminalCatalog::try_new(fingerprint, arity, coverage, terms)`;
- `encode_native(BinaryIoLimits)`;
- `decode_generated(bytes, expected_fingerprint, expected_arity, BinaryIoLimits)`;
- immutable family, arity, coverage and typed-term accessors.

`TerminalCatalogCoverage::{Partial, Complete}` records a producer's finite
coverage declaration. A `Complete` flag alone does not compare against an IBP
program's actual terminals. Vakint retains that independent exact-key-set check.

The existing common native envelope has a separate `TerminalValues` kind and
three sections: Symbolica state, structural catalog records, and native values.
The structural records use bincode's standard integer representation for all
signed i64 coordinates and dictionary IDs. Small coordinates are compact, while
the full i64 range is retained. There is no hand-written integer or CAS codec.
Each distinct exact Atom is encoded once with Symbolica's `Encode` implementation;
decoding uses its `HasStateMap` context. IDs follow first occurrence in sorted
integral-key order. The shared framing helpers were extracted without changing
the existing rational-polynomial coefficient wire bytes.

The reader requires matching family identity and arity, strictly sorted unique
keys, valid first-occurrence IDs, no unused/duplicate value records, exact section
consumption, and valid bounded outer framing. Every native frame is preflighted
before state import. A catalog cannot load as a candidate or certified artifact,
and those kinds cannot load as a catalog. No compatibility decoder for the former
line-oriented catalog format remains in production.

Exactness checks use Symbolica's native node and coefficient APIs. They reject
approximate or nonfinite numbers, including those hidden in function arguments,
powers, and packed rational-polynomial variable maps. Ordinary Atom walking does
not traverse those packed maps, so their native `Function`/`Power` variables are
checked recursively. Exact rational-complex and finite-field coefficients remain
valid in the generic transport; physical consumers retain their own domain and
symbol requirements.

Native Atom/state bytes are **trusted generated data**, not a hostile-file parser.
RustRed enforces caller-owned outer byte/count limits and returns typed errors,
but Symbolica's native state reader is not an allocation sandbox. Import may
extend process-global symbol state. Native bytes can depend on ambient IDs;
mathematical equality and preserved namespaces, not byte identity across unrelated
process histories, are the portability contract.

## Exact migration evidence

The four saved text catalogs were copied recoverably to the workspace-local
`TMP/terminal-catalog-native-migrate.F2mdqy/` evidence directory. A small offline
helper parses the former text exactly once, creates the new owner, encodes it,
and compares every key/value and all metadata on reload before creating an output
file. A separate fresh process imports each new file after unrelated Symbolica
symbols are registered; expected PR expressions are parsed only **after** native
import. Every imported expression is compared by exact Atom equality and an exact
zero-difference check. No FMFT calculation or IBP solve is part of conversion.

| Parent | Terminal keys | Distinct values within parent | Former bytes | Native bytes |
| --- | ---: | ---: | ---: | ---: |
| H | 386 | 23 | 25,632 | 8,845 |
| FG | 145 | 18 | 10,309 | 5,654 |
| BMW | 179 | 20 | 13,514 | 6,391 |
| X | 445 | 21 | 33,565 | 9,417 |
| Total | 1,155 | 26 across all parents | 83,020 | 30,307 |

All **1,155/1,155 exact comparisons passed**, both during conversion and in fresh
consumer processes. Total size falls by 52,713 bytes (63.5%). These files are
uncompressed; no gzip setting is involved. Per-parent shared-state sections are
800, 676, 636 and 676 bytes respectively. Metadata includes the original verbose
family fingerprint, so the result is not merely a packed list of master symbols.

Single release-profile diagnostics on this shared host, with CPUs 94–99 and nested
pools limited to one, measured:

| Parent | Fresh native import | Encode after exact comparison |
| --- | ---: | ---: |
| H | 0.361 ms | 0.119 ms |
| FG | 0.189 ms | 0.064 ms |
| BMW | 0.256 ms | 0.078 ms |
| X | 0.469 ms | 0.132 ms |

These phase timers exclude file reading, process startup, compilation, unrelated
symbol seeding (including initial Symbolica activation), and later exact
comparison. They are single-sample diagnostic observations, not a paired
statistical benchmark or an end-to-end reduction speedup claim. The purpose of
this migration is unified exact native I/O and compact storage; these catalogs
were already small relative to the parametric rule programs.

## Validation and delivery gates

Focused core tests cover exact expressions, zero, empty/partial catalogs, full
i64 indices, compact structural records, truncation, malformed lengths/counts,
budgets, wrong family/arity/kind, and dictionary invariants. An independently
authored fresh-process test covers clean and dirty symbol histories, symmetric
functions, same-spelling different namespaces, packed-polynomial constants/zero
with different ordered maps, conflicting symbol attributes, and hidden floats.

The release persistence suite passed **66/66**, including six new catalog tests.
Both independent native-catalog test parents passed; the ignored child entry
point was exercised by the fresh-process parent. Existing coefficient and
certified-program fresh-process parents also passed. Retained logs are
`TMP/native-terminal-catalog-persistence-release.log`,
`TMP/native-terminal-catalog-context-release-final.log` and
`TMP/native-terminal-existing-contexts.log`.

Core compilation, all-value migration checks and the independent codec audit
are complete. The normal release/locked/offline/no-default-features
`candidate_bundle` example build also passed. Its four fresh `verify 10`
invocations loaded the unchanged saved candidate programs with the new catalogs
and confirmed family identity and exact declared-key equality for 386, 145, 179
and 445 terminals respectively. No `generate` operation was invoked. Evidence
is retained as `{h,fg,bmw,x}.example-verify.{log,time}` beside the migration helper.

Downstream pinned-Vakint validation remains a separate gate:
this note does not yet assert its completion.
The downstream migration retains the existing raw terminal declarations, public
backend/default behavior, master substitutions and numerical tolerances. The
public fifteen-reference and sixteen-expanded-propagator/pinch suites must be
rerun after the published RustRed revision is pinned; no catalog or IBP generation
is necessary for those runtime acceptance tests.
