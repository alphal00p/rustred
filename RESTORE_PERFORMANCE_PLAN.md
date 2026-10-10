# Rank-extension restore: diagnosis and narrow repair

## Request

Investigate THE_ONE's rank-6 extension spending almost six hours in `restore`,
and fix the problem without losing saved work or changing the requested scope.
Leave the live campaign running until the user chooses a restart; never replace
its executable, checkpoint, or inputs in place.

## Measured diagnosis

The frozen solver `54302272…` is active on one core, not deadlocked. Generation
33 contains 139,772,626 domains, 1,405,075,888 edges and 22,659,469 anchor records.
Restore serially rechecks exact union coverage of every anchor. A ten-second
99 Hz user-CPU profile (901 samples, none lost) finds 36.85% in lattice
`Region::nonempty`, 12.10% in `Region::with`, 13.10% in memory copying and
substantial allocation. This is checkpoint restoration, not the optional
publication deep-verification step. Its current phase has no internal progress.

The current source already has an independently tested faster lattice kernel.
However, the new capacity-dispatch binary needs a separate physical-width
restore fix before it can safely replace the frozen physical-width solver.

## Implementation and delegation

- `restore_parallel_fix`: preserve all exact coverage/provenance checks while
  parallelizing independent immutable anchor checks within the configured
  worker budget. Keep deterministic failures and bounded transient memory.
  Add restore-stage/progress events and cooperative cancellation. Cancellation
  must not trigger previous-generation fallback or adopt a new writer session.
- `restore_capacity_fix`: repair saved physical-width versus storage-capacity
  handling in anchor/record restore, without weakening original byte digests
  or permitting nonzero/open nonexistent coordinates. Test resume and resave.
- `rank6_restore_audit`: independent code/mathematical audit of both lanes,
  corruption, cancellation, determinism and resource boundaries.
- Root: coordinate interfaces/builds, verify the actual diagnosis, run focused
  tests and representative release controls, inspect live progress read-only,
  document measured results and provide a safe restart procedure if warranted.

## Acceptance and limits

No IBP generation, rule choice, starting-scope or checkpoint proof is weakened.
Test serial/parallel equivalence, bad anchor rejection, worker limits, visible
progress, cancellation without fallback and cross-capacity restore/resave.
Use small real checkpoint controls and a bounded large-checkpoint diagnostic
in scratch space where practical. Do not claim full restore speedup from a
kernel microbenchmark. Preserve unrelated dirty work. All temporary evidence
lives under `TMP/rank6-restore-diagnosis-20261010/`. Commit/push coherent tested
changes with the requested Git identity; no production/campaign data included.

## Large-checkpoint follow-up

The first bounded copied-checkpoint probe spent more than six minutes in lookup
reconstruction without reaching anchors. Independent source audits identified
a redundant retirement scan on every restore insertion, despite an empty
retirement set. Implement a restore-specific prepare/insert path for its fresh
indexes, sharing ordinary insertion/orthant maintenance. Preserve generic index
retirement (which also removes pre-existing empty rows). Require old/new index
layout, lookup, live-selection and resave equivalence; profile the pre-fix
large probe and repeat it with the final build. Do not report the censored probe
as a completed restore or as evidence of anchor throughput.
