# Campaign JSON contract (WP-0a / WP-0b)

Hand-written JSON Schemas (draft 2020-12) of every file the campaign, its dashboards and its checkers exchange. The
schemas are the CONTRACT (plan D-36); the Rust structs round-trip every example; `validate.py` is the ONE validator
(gate, Rust tests, website loader).

| path | what |
|---|---|
| `*.schema.json` | one schema per format (`x-rr-names` = the `schema` values it accepts, `x-rr-select` = discriminator, `x-rr-struct` = the Rust type, `x-rr-owner`, `x-rr-freeze`); `common` / `progress-common` hold shared definitions |
| `examples/` | one valid example per schema (`<stem>.json` or `.jsonl`); as-built formats copy the owners' goldens, `ledger-record.jsonl` is written by the as-built ledger writer |
| `fixtures/cdir-{empty,mid,finished,stale}/` | campaign directories: ledger (`ledger/MANIFEST.json`, segments, snapshot) written by the AS-BUILT `Ledger` with a manual clock, progress files derived by the AS-BUILT `progress::derive`; hand-written documents beside them carry `"fixture_origin": "hand"`; `cdir-finished` uses an invented toy family set (genericity, R41 P-5); `fixtures/INDEX.json` = registry |
| `validate.py` | `validate.py FILE...`, `--schema STEM`, `--cdir DIR [--now UTC]`, `--canon FILE`, `--blake3 FILE`, `--list`; exit 0 valid, 1 invalid, 2 missing, 3 usage |
| `test_validate.py` | named mutation cases that must be rejected + canonical vectors |
| `canon-vectors.json` | shared vectors of the canonical JSON rule (plan 3.3.0) and of the contract's record form |

Regenerate fixtures and the ledger example: `RR_UPDATE_FIXTURES=1 light_build.sh <lane> test -p rustred-campaign --test schema_roundtrip -- --test-threads 1`
(then review the diff). Tests: `light_build.sh <lane> test -p rustred-campaign-core --test 'schema_*'`, same for
`-p rustred-campaign-contract --test schema_roundtrip` and `-p rustred-campaign --test schema_roundtrip`.

## Versioning (how to add a field)
- Every document names `<name>.v<MAJOR>`. Readers refuse an unknown MAJOR and ignore unknown fields where the schema is
  open (no `additionalProperties: false`).
- ADDITIVE = minor (no name change): a new OPTIONAL field in an open object, a new value of an open string, a new
  progress file. Steps: edit the schema, extend one example, add the field to the struct (or let `extra` carry it),
  append one line to `plan/auto-campaign/INTERFACE-CHANGES.md` (I-INT + C-DASH sign).
- MAJOR (`v2`, coordinator signs too): removing or renaming a field, changing a type or unit, making a field required,
  adding a value to a CLOSED enum or an unknown field to a CLOSED object (the ledger, job.json, site policy, control),
  changing digest algorithm, canonical form, or the meaning of a value.
- Closed objects (`additionalProperties: false`) are the ones whose as-built reader refuses unknown fields or that are
  hashed / replayed (ledger records): there even an addition is a major change for old readers.

## Conventions
Digests are lower-case hex BLAKE3 (as built; the plan's sha256 is an optional extra field in the spec only). Files
named by records are FileRefs `{path, bytes, blake3}` relative to the campaign directory or `large:`-prefixed.
Integers only in identifying documents (spec, accept, ledger); progress files may carry floats. Nothing in a schema
assumes a particular family set (node names, arities, counts are data).
