# Raw numerator-symmetry assay on a retained inventory

This topology- and loop-count-generic research helper cold-loads an existing
`TerminalRelationSession` and a native `TerminalNumeratorSymmetryEquations`
capsule. It ingests only the capsule's complete exact equations, through the
existing assisted-session API. It does not load or apply candidate programs,
generate ordinary IBPs, evaluate numerical masters, or mutate source artifacts.

Provider draining stops before `step_with_provider` would advance the ordinary
source cursor. Auxiliary columns remain obligations, never newly declared
masters. The result can therefore contain unresolved auxiliary columns while
still proving additional terminal-to-terminal dependencies. Neither a minimal
basis nor closure of those auxiliaries is claimed.

## Build and invocation

Compile `study.rs` against the same frozen optimized `rustred`, `serde_json`
and `blake3` libraries and native GMP/Blake3 paths as the application. This small
driver is not an engine rebuild. Use an optimized driver for timing; the
recorded study used `-C opt-level=3 -C lto=thin`.

```bash
python -B tools/research/numerator_symmetry_retained_inventory/run.py \
  --binary /path/to/optimized-retained-inventory-assay \
  --baseline /path/to/baseline.rrbin \
  --provider /path/to/numerator-symmetry-certificate.rrbin \
  --output /path/to/new-evidence-directory \
  --profile conservative \
  --max-rss-bytes 128000000000
```

CPU placement is external, e.g. `taskset -c 232-235` before the Python command.
The RAM allowance is caller-owned; there is no timer, row count or index-degree
cutoff. The runner monitors only its new process group, authenticating PID and
process-start ticks before stopping its own disposable helper for RAM pressure.
It never signals a production campaign.

Optional `--target-powers-json '[...]'` records that raw key's before/after exact
map in the summary. It is diagnostic input, not a topology name or solver branch.

The original named normalization profile **must match the baseline**. Choosing
a looser normalization budget can change the regenerated proof/representative
set and correctly fails cold validation. `conservative` and `standard` reproduce
the application's existing versioned recipes; these are proof semantics, not
new solve cutoffs. Operational row, column, source and native-I/O grants are
unbounded, with RAM controlled externally by the explicit allowance.

## Evidence and validation

The new output directory contains command/resource receipts and native data:

```text
receipt.json
process.time
stdout.jsonl
stderr.log
native/inputs.json
native/after.rrbin
native/summary.json
```

`after.rrbin` includes the exact coefficient maps. Every before map is composed
through the final maps and compared exactly, every inherited guard is retained,
outputs are remaining terminals only, and the final session is cold-decoded and
checked again. Both input files must still have their original bytes afterward.
The native symmetry capsule independently replays its complete momentum maps
and numerator equations on cold loading. No expression strings grant authority.

An atomic `checkpoint.rrbin` is written at completed row boundaries roughly once
per minute; a cancellation/stop file is checked before the next row. The helper
is a research assay, not the full production dispatcher: an opaque exact row
cannot necessarily stop or save mid-operation. RAM-censored runs are explicitly
reported as incomplete and must not be called successful refinement.

To inspect a relation from a cold session, use the existing public API with the
same profile and input bounds:

```rust
let maps = session.apply_all_terminals()?;
let exact_row = maps.get(&target_key);
let remaining = session.remaining_terminals();
```

The five-loop measurement is documented in
[`numerator_symmetry_five_loop_raw_results_2026-10-22.md`](../../../../docs/research/numerator_symmetry_five_loop_raw_results_2026-10-22.md).
