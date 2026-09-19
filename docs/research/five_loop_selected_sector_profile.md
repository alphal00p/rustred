# Five-loop cube: selected-sector bottleneck diagnostic

The complete cube family did not finish in either earlier six-worker,
30-minute attempt. This follow-up deliberately **does not repeat that family
campaign**. It isolates one previously completed sector and the unfinished
physical parent, using the same external
[`five_loop_cube.toml`](../../examples/input/five_loop_cube.toml), natural
ordering, source definitions, zero-sector census, and numerical-corner depth
two. No solver or computer-algebra algorithm was changed.

## Scope and measurement boundary

Two sectors were selected before measurement:

- Mask **3734**, `011010010111000`: the first sector completed in both old
  progress logs, each reporting 177 rules and one finite residual.
- Mask **4095**, `111111111111000`: the twelve-edge physical parent, unfinished
  in both old logs. Coordinates 12, 13 and 14 remain nonpositive ISPs.

Each sector/backend runs in a fresh process, serially on CPU 82, with nested
pools capped at one, a 180-second external deadline, and a **32 GiB virtual
address-space cap**, not an RSS cap. The parent additionally receives a 99-Hz
userspace flat CPU profile. This is an instrumented phase diagnostic on a
shared host, not a production performance comparison or completed-family
timing. The parent is not profiled concurrently with the representative sector.

A workspace-only client links already-built optimized core/Symbolica libraries
and uses the public input compiler and sector solver. There is no Cargo/library
rebuild, topology-specific relation, FORM input, rule reuse, or certification.
The new observer exposes the semi-numerical backend's **retained exact replay**;
it does not remove it. Major events are flushed immediately, including replay
boundaries; detailed discovery/exact-row events are throttled to 100 ms.
Observer formatting/I/O is inside the measured solver interval.

Input SHA256:
`0247e0d8fab665a59d34cbfac0d20f94ea33e282e926870842edaaf53fcac77f`.
Client SHA256:
`4ae48af9c7e007b4650524580e9a848a8d5fb65ec048f1be488af004a273cecb`.
Library and source hashes are retained with the raw evidence. An initial
launcher typo in the license was stopped and retained separately as a setup
failure; it is not part of these measurements.

## Completed representative: exact agreement

Both backends return **177 rules and one finite residual**, with 176 symbolic
cases and two numerical cases. Fresh-process comparison passes all **6,898
exact coefficients and their ordered contexts**, together with the case,
source-provenance, guard, RHS-integral and residual structure. Neither output
establishes full-family closure or master minimality.

| Mask 3734, one run/backend | Sparse exact | Semi-numerical |
|---|---:|---:|
| Solver core, including preconditioning | 6.668 s | 8.641 s |
| Discovery, including source instantiation/modular GPLU | 5.018 s | 5.745 s |
| Exact materialization / reconstruction plus replay | 0.318 s | 1.530 s |
| Internal exact replay, included in preceding row | — | 0.224 s |
| Numerical-corner search | 1.218 s | 1.259 s |
| Whole process wall | 28.00 s | 33.02 s |
| Whole process peak RSS | 75,244 KiB | 77,732 KiB |

The whole-process boundary also includes input preparation, the complete
family-wide zero census (5,480 proved-zero sectors), output serialization and
shutdown. The core boundary excludes those. Both symbolic runs use 42 selected
exact frames and 134 direct
hits. All 42 reconstructions and replay checks succeed, without exact-support
recovery. Discovery, not replay, dominates this representative.

## Parent: retained partial phase evidence

Both parent processes reach the declared deadline with **exit 124**. Neither
returns a complete sector solution, writes a complete coefficient/structure
result, or reaches the numerical-corner tail. All owned clients were checked
absent afterward; no generation job was restarted.

| Mask 4095, censored diagnostic | Sparse exact | Semi-numerical |
|---|---:|---:|
| Whole command wall, including preparation/profiler | 180.56 s | 180.37 s |
| User + system CPU | 177.43 + 0.93 s | 177.28 + 1.19 s |
| Peak RSS | 204,572 KiB | 194,048 KiB |
| Completed symbolic-rule events | 281 | 274 |
| Symbolic cases started | 282 | 275 |
| Materializations started | 213 | 206 |
| Last retained solver event | 161.439 s | 159.142 s |
| Discovery through that event | 29.841 s | 23.514 s |
| Materialization outside semi-numerical replay | 130.476 s | 51.347 s |
| Completed exact replay intervals | — | 83.602 s |
| Canonicalization + guards + geometry | 1.122 s | 0.679 s |

These event-derived phase sums end at the last flushed event. Sparse then has
an open materialization; semi-numerical has an open replay. The respective
last-event-to-stdout-EOF receipt intervals are approximately 0.358 and 1.690 s,
including termination/transport delay, and are **not counted as completed
phase work**. Semi-numerical starts 206 replay checks and completes 205, with
zero reported replay failures or support-recovery requests.

The backends have not completed identical total workloads, so their partial
rule counts, RSS or censored wall times do not establish a speedup. They do
identify the pressure point: exact materialization consumes **80.8%** of the
sparse observed interval; reconstruction outside replay consumes **32.3%** of
the semi-numerical interval and exact replay another **52.5%**. Exceptional
geometry and guard extraction are small in this particular prefix.

### A concrete difficult case

Case 265 leaves only the first power, native symbol `n0`, free; the powers are
`[n0,2,2,1,1,1,1,1,1,1,1,1,0,0,0]`. Both logs report a selected frame with
**997 source rows, 3,458 integral columns and two active coefficient variables**
(`n0` and dimension). Sparse materialization takes 19.812 s. The corresponding
semi-numerical frame spends about 4.322 s before replay and 14.261 s in replay,
with a 1,490-term reconstructed output. These are single observed case
intervals, not a repeated matched-frame benchmark. They show that few remaining
parameters do not imply a small elimination problem.

## Profile: where the arithmetic time goes

Profiles retain only the client PID and use a conservative one-second interior
trim relative to monotonic stdout receipts. Phase assignments use the flushed
observer boundaries; receipt/observer overhead prevents exact attribution at
the boundary. There are **13,930 sparse** and **14,085 semi-numerical** interior
samples. These are exclusive sampled CPU weights, not inclusive call costs or
wall-time fractions.

- Sparse materialization receives 11,377 samples. Its leading native symbol is
  polynomial `heap_division_packed_exp` (902 samples, 7.93% of that phase),
  followed by allocation and native arithmetic/container services.
- Semi-numerical exact replay receives 7,513 samples and shows the same kind
  of polynomial-division/allocation work.
- Semi-numerical reconstruction outside replay receives 4,469 samples:
  finite-field GPLU `scatter_with_touched` accounts for 1,837 (41.11%),
  `add_row` for 622 (13.92%), native polynomial evaluation for 448 (10.02%),
  and finite-field inverse for 396 (8.86%).
- Discovery samples also expose structural zero-support checks, dynamic GPLU
  column insertion and integral ordering. Its leading zero-support helper is
  28.23% of sparse discovery samples and 31.80% of semi-numerical discovery
  samples, not of the whole run.

The reconstruction route therefore has **two measured costs**: repeated
finite-field elimination and retained characteristic-zero replay. It is not
simply stalled in optional artifact certification, guard geometry, or a reported
reconstruction failure. No general six-loop conclusion follows from this
particular parent prefix.

## Bounded next experiment, not an implementation claim

Keep existing validation intact. The most direct next test is the already
implemented `SparseTargetOnly` exact backend on a frozen hard case such as 265,
with full ordinary-row/context equality and an external cap. A coefficient
variable-order comparison on that same case is also narrower than changing the
family search. The subsequent target-only test is recorded below; variable
ordering was not changed or tested.

For reconstruction, investigate the measured native finite-field GPLU workload
before proposing new algebra. The current implementation **already shares a
complete target-row image between coefficients at the same prime and point**;
adding that cache again would not address a missing feature. Any subsequent
optimization must use audited Symbolica public APIs, preserve pivot/guard
semantics and exact validation, and be tested on the same selected frames.

Raw evidence, execution scripts, exact representative snapshots, hashes,
event accounting and per-symbol memberships are retained in
`TMP/five-loop-sector-profile.Ijiilz/`. Independent review checked the source,
scope, exact comparison, phase sums, censored statuses, PID/clock filtering and
profile interpretation. No additional full-family or Möbius campaign is part
of this diagnostic.

## Follow-up: re-solve only case 265 with the existing target-only backend

The parent logs retain frame sizes and coordinates, **not** the selected rows
or their seed provenance. The exact materializer dispatcher is private, so
the public-API follow-up is not a direct replay of a frozen frame. Instead,
each backend independently re-solves the single externally supplied coordinate
case with identical family sources, preconditioning, ordering, prime and seed.
There is no parent-sector queue traversal. The caller uses the existing public
exception extractor and checks strict descent; no new production seam or CAS
code is introduced.

Both runs complete under a 60-second process cap, serial CPU 82 and 32 GiB
virtual-memory limit. The exact comparison verifies the **complete ordered
997-entry `SeedSource` trace**, canonical target/case, inherited source
conditions, all **1,489 RHS coefficients and ordered variable maps**, and all
**three exceptional guard branches**. Both discover 2,442 rows from 98 seeds.
Thus the selected source identities/order agree exactly, although discovery is
repeated rather than replayed from saved input rows.

| One re-solved coordinate case | Sparse | SparseTargetOnly |
|---|---:|---:|
| Discovery before materialization | 0.482 s | 0.498 s |
| Exact materialization | **14.637 s** | **2.971 s** |
| Single-case core, including preconditioning | 15.129 s | 3.481 s |
| Native guard extraction, outside core timer | 0.028 s | 0.025 s |
| Whole process wall, including preparation/output | 36.24 s | 25.06 s |
| User + system CPU | 34.44 + 1.45 s | 23.32 + 1.49 s |
| Whole process peak RSS | 96,796 KiB | 73,060 KiB |
| Process status | 0 | 0 |

The observed exact-lifting ratio is approximately **4.93×** for this single
case. Target-only first eliminates **1,297 harder/target columns** instead of
all 3,458 columns, taking 1.929 s. Its native triangular weight solve takes
0.590 s and yields 747 nonzero weights; multiplying back into the full selected
source rows takes 0.422 s and restores all 1,490 terms including the pivot. These
subphases exclude small preparation and intermediate overhead. The complete
normalized RHS is identical, not truncated to the smaller block.

This result is actionable evidence for broader tests of an **already
implemented** backend, not an automatic default switch or a prediction of
full-family runtime. It is one ordered observation per backend, with observer
I/O included and no profiler attached; the whole-process interval is dominated
in part by the unchanged family-wide zero census. Other frames may violate the
target-only backend's independent-prefix requirement or have different costs.
No validation was weakened, and neither artifact certification nor another
full-family generation was run.

Evidence: `TMP/five-loop-single-case.H1gcs8/`, containing the external case,
source/client/library hashes, phase records, full source trace, native exact
snapshots, fresh comparison and resource reports. Both processes are stopped.
