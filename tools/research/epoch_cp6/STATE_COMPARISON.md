# Cross-version CP6 mathematical-state comparison

`compare_state.py` compares **drained, exclusively owned, immutable** CP6
checkpoints without reimplementing the native typed-record codec. It understands
the manifest/scalar version pairs used by the historical and S5 builds, while
using the unchanged `EPC6PART` version-1 sections for state comparison.

Run each build's native cold-All verification and the existing acceptance gate
first. This tool does not authenticate BLAKE3, prove closure, replay IBP sources,
or replace either native gate. SHA256 here only compares actual local bytes.

```bash
cd /common/dev/rustred
python -B tools/research/epoch_cp6/compare_state.py \
  OLD_RUN/checkpoint NEW_RUN/checkpoint \
  --left-queries FROZEN_QUERIES.json --right-queries FROZEN_QUERIES.json \
  --output NEW_COMPARISON.json
```

Exit 0 means the compared durable state is equal; 1 means a difference; 2 means
the comparison was refused (unknown schema, missing sections, unfinished
dispatch, malformed input, changed files, etc.). The report is created, never
overwritten. Read the report's `limitations`, even on exit 0.

Required equal:

- Domain geometry in ID order; node flags; live lookup bits; ledger.
- Ordered dependency edge targets, including chosen target IDs; anchor scopes,
  pins and stamps; frontier counts; dominant orthants; rescue state if present.
- Sealed/inspected tracker flags (cached closed bit2 is excluded explicitly).
- Owner digests, admitted query IDs/roles/domain mappings, input frontiers,
  complete original query-file bytes, logical walk counts and fixed-cut settings.
- Drained dispatch epoch/root/window/cursor. Session/sequence counters are only
  transport receipts and are reported separately.

Reported but not required equal:

- Manifest/scalar/record versions, request binding hash and record representation.
- Physical lookup and verify counters, cached closure status/revisions/timing,
  helper reservations and logical scratch configuration, storage digests.

The comparison does **not** claim full record/event or generated-source equality.
Those data are absent from public checkpoint summaries, and S5 sidecars require
the native typed reader. Unchanged durable edge/anchor sections retain the graph
choices needed for this narrower fixed-cut comparison. The two bound launch
commands still need independent option-equivalence review: the versioned request
hashes cannot be compared directly across different walk semantics.

For new-versus-new helper measurements, separately require identical logical
scratch allowances. Physical-counter equality across identical new layouts is
also a separate stronger gate; cross-version layout counters legitimately differ.
Do not loosen the equality of graph choices to accommodate a storage change.

Synthetic tests:

```bash
cd /common/dev/rustred/tools/research/epoch_cp6
python -B -m unittest test_compare_state -v
```

Tests use temporary directories only under repository-local ignored `TMP/`.
Their synthetic sections are comparison fixtures, not valid native proof
artifacts. They mutate graph targets, geometry, ledger, anchors, roles, logical
counts and cached/physical diagnostics independently.
