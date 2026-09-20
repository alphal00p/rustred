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

## Follow-up: factorized symbolic field reaches the shared numerical tail

After the independently audited factorized-field milestone `80bddadf`, the
same public sector client was linked to frozen optimized libraries and rerun
on the same external input, natural ordering, source definitions and depth-two
numerical search. The only solver policy change from the original sparse
diagnostic was `SymbolicExactBackend::SparseFactorized`. At that milestone the
shared numerical-case exact lift still used ordinary rational polynomials.

The representative sector again completes: **177 rules, one finite residual**,
with all **6,898 exact coefficients, variable maps and complete source/case/
guard/RHS structure** equal to the saved ordinary output. Solver core is
6.093 s, symbolic materialization 0.188 s, whole process 25.04 s and peak RSS
75,312 KiB. This fresh smoke check is not a paired performance comparison.

The physical-parent attempt had a longer, declared **900-second process
deadline**, still one worker on CPU 82 with the 32 GiB virtual-address cap.
It reaches **290 completed symbolic cases in 66.965 s of solver time**, then
starts the shared numerical solve on **49 fully fixed cases**. The process
does not return a complete sector solution before its deadline: exit 124,
900.11 s whole wall, 889.40 s user + 1.80 s system CPU and **1,128,984 KiB**
peak RSS. No final coefficient/structure snapshot is written. These are 49
inputs to an unfinished numerical solve, not 49 proved independent masters.

Two short sampling attachments were added after the tail persisted. A
15-second flat sample spans modular discovery and exact work and therefore
does not measure a whole-phase fraction. A later five-second, 49-Hz DWARF-stack
sample contains 232 samples with zero lost: **95.26% inclusive** lies under
the shared exact materializer. Its native ordinary-RP GPLU stack spends most
observed time in rational-polynomial addition/multiplication and native
polynomial GCD/division. These are inclusive weights of a small late CPU
sample, not percentages of the entire 900-second run. The run is an
instrumented diagnostic, not an unprofiled production timing.

The next implementation extends the existing native factorized-field adapter
to **multiple requested pivots in one shared exact elimination**, without
changing numerical discovery, source traces, row/column ordering or terminal
semantics. It is generic in integral arity and loop count. Capture each requested
native U row at its pivot, retain its caller ordinal, then restore ordinary
coefficients and original variable maps before rule creation. It neither
implements a new CAS kernel nor asserts that the pending cases will close.
The extension and a new end-to-end comparison require their own tests and run;
the completed symbolic prefix alone is not evidence of full-family closure.

Evidence: `TMP/five-loop-factorized-parent.FtQ1Vs/`, including frozen library/
client/input hashes, the exact smoke comparison, both raw sample datasets,
observer events, process status and resource reports. All associated solver
and sampler processes have terminated.

## Shared numerical factorized field: exact gates, incomplete parent

The follow-up uses `NumericalExactBackend::SparseFactorized` for the numerical
union lift as well as factorized symbolic lifting. Source discovery and the
depth-two finite-case search are unchanged. The 11 focused numerical tests and
complete release core suite (2,211 passed, 31 ignored) pass. A fresh selected
sector again returns 177 rules and one residual; post-exit cold comparison
passes all 6,898 coefficients, both variable maps and full rule/source/guard
structure. One comparison was accidentally launched while the producer was
still writing its structure file; its partial-file failure is retained but
excluded. The complete-file comparison, not that launch, is the gate.

The full physical-parent sector still **does not finish** within its declared
900-second process limit. Exit is 124, whole wall 900.48 s, CPU 890.69 user +
3.95 system seconds and peak RSS 790,768 KiB. Its 290 symbolic cases finish at
252.052 s of solver time, followed by the same 49 fully fixed inputs. No final
sector snapshot is produced. These two censored runs do not establish a field
speedup or memory saving: their completed work and runtime conditions differ.

Two small late stack samples distinguish the phases. The first records 52
samples, all in modular discovery. The second records 122 samples, with 99.18%
inclusive under the new native factorized-field exact union lift. Native FRP
addition, polynomial exact division and multiplication dominate that second
window. Neither sample loses events. The 5/10-second sampler-supervisor limits
yield only approximately 1.06/2.49 seconds of recorded CPU-event coverage;
they are not whole-run phase percentages. The parent is an instrumented
diagnostic, not an unprofiled timing comparison.

Because even preparation slowed relative to earlier measurements, an additional
fresh old-then-new control pair uses the same representative, CPU 82, depth two,
120-second limits and frozen release binaries. Both complete and pass the full
6,898-coefficient/structure comparisons:

| Fresh representative control | Prior binary, ordinary numerical lift | New binary, factorized numerical lift |
|---|---:|---:|
| Zero-census preparation | 58.232 s | 55.421 s |
| Solver core | 20.850 s | 20.935 s |

The unchanged prior binary previously prepared the census in 15.621 s and
solved this sector in 6.093 s. Thus the cross-run slowdown is not evidence of
a new numerical-field regression. Its cause has not been established. This
single fresh pair is a correctness/control check, not a general performance
claim; its numerical exact frame has only seven source rows.

The next independently reviewed experiment varies the **existing generic
`numerical_depth` input**, not the symbolic strategy. Depth zero still processes
each unresolved fixed center's zero-displacement seed, shared modular GPLU and
every winning exact lift; it is not a skip. For these 49 inputs it admits at
most 49 distinct centers and 1,225 source-row instantiations, though exact
arithmetic can still be expensive. It may leave a larger finite residual set,
which is acceptable under the nonminimal-basis policy. Symbolic coverage,
geometry/guard errors, exact recovery and descent remain unchanged. A returned
sector still requires cold admission and application/reachability checks and
does not certify all 2,656 family sectors or master independence.

Evidence: `TMP/factorized-numeric-parent.iwUFAe/`; independent interpretation
and depth-policy audit:
`TMP/five_loop_numerical_depth_zero_audit_2026-09-20.md`. All runs in this
section are terminal. The depth experiment has separate frozen inputs and
evidence in `TMP/finite-corner-depth.xL5pJk/`.

## Existing depth-zero policy: first completed physical-parent sector

The depth-zero run **returns successfully and completes serialization**, exit
0, with **310 rules and 29 finite residuals**. Its 290 symbolic cases lead to
49 fixed numerical inputs. Solver core is **66.850 s**; the shared numerical
phase takes **0.240 s**, including **0.041 s** for native factorized exact
lifting of 200 trace rows. The modular search sees 1,200 rows, 1,196 independent
rows and 4,378 integral columns. This result uses the existing generic caller
option, not a loop-specific algorithm or a relaxed exact-replay gate.

Whole process wall is 133.58 s, CPU 86.20 user + 46.51 system s and peak RSS
265,276 KiB. That boundary includes a 16.349 s family-wide zero census, verbose
unbuffered diagnostic text output and a roughly 17 MiB native coefficient
snapshot; it is **not** production-artifact generation time. The freshly built
external client first passed the depth-two representative gate: all 6,898
coefficients/maps and complete structure agree with its saved predecessor.

The physical parent is one sector, not the complete 2,656-sector family. The
29 retained integrals are explicit finite residuals, not proved independent
masters. Full-family generation, cold rule-owner admission and recursive
application/reachability remain separate gates. The depth-two timeout and
depth-zero completion also have different finite search workloads and must not
be turned into an arithmetic-backend speed ratio.

A second fresh run uses the same executable, input, ordering and depth, retaining
factorized symbolic arithmetic but using **ordinary numerical arithmetic**.
It also finishes with 310 rules and 29 residuals. After both producers exit,
native comparison verifies all **56,990 exact coefficients and both ordered
maps**, plus identical source/case/guard/RHS/residual structure.

| Depth-zero parent, one run per numerical field | Ordinary | Factorized |
|---|---:|---:|
| Solver core | 72.293 s | 66.850 s |
| Shared numerical phase | 0.315 s | 0.240 s |
| Exact union lift, included above | 0.115 s | 0.041 s |
| Whole diagnostic process | 143.13 s | 133.58 s |
| Peak RSS | 262,992 KiB | 265,276 KiB |
| Exit status | 0 | 0 |

The numerical exact subphase is now small; it cannot explain the whole-core
timing difference. One ordered observation per field is not a general speedup
measurement. The useful result is that a **generic, existing finite-search
policy** permits this parent sector to return exact rules and a finite
nonminimal residual set without searching for additional terminal relations.
Cold coefficient comparison is not cold rule-owner admission or certification.

## Public finite-case depth control

The existing generic finite-case search bound is now available as
`FamilyCandidatesRequest::numerical_depth`, CLI `--numerical-depth`, and Python
`family_candidates(..., numerical_depth=...)`. The default remains two, and
zero still searches initial fixed seeds. It neither disables exact lifting nor
asserts that retained finite residuals are independent masters. No production
algorithm is selected by loop count or topology name.

The default policy tag and native program layout remain unchanged. Nondefault
depths have canonical policy metadata; malformed tags are rejected before
native State import. Generation reports and cold inspection expose the chosen
depth. Release validation passes 94 application unit tests, 68 integration
tests and 36 Python tests using a freshly built CLI and extension. Shared
fixtures check same-depth ordinary/factorized arithmetic and one/two-worker
results, exact cold coefficient maps, defaults, policy admission and public
input errors. An independent implementation/mathematical audit finds no blocker.

Evidence: `TMP/numerical-depth-api-release-*.log` and
`TMP/numerical-depth-public-interface-audit.md`. Compilation is excluded from
all solver timings above. This public API milestone is not a new five-loop
solve; the next run supplies the unchanged physical-family input through the
CLI, rather than recompiling a fixture-specific diagnostic client.

## Preparation follow-up: native modular full-rank screening

The later [completed target/full-factorized parent comparison](factorized_coefficients.md#completed-selected-parent-follow-up-september-20)
establishes exact selected-sector parity, but retains substantial variation in
preparation as well as solver and output times. A separate user-CPU profile
finds 614 of 1,480 interior preparation samples in native rational forward
elimination. This motivates a generic optimization of the zero-sector analyzer,
not another family-specific solver policy.

The integer exponent matrix now first uses Symbolica's native
`Matrix<Zp64>::partial_row_reduce` when the existing work budget can cover both
the screen and a rational fallback. The fixed prime is authenticated once by
Symbolica's deterministic u64 primality test. Full column rank modulo that
prime proves a nonzero integer maximal minor and therefore full rational rank.
Every modular miss keeps the original rational RREF, deterministic primitive
kernel and native integer replay unchanged. Full rank still means only that
the sufficient zero-sector test is **inconclusive**, not that the integral is
nonzero. No reconstruction or custom elimination is introduced.

Original shape, dimension, work and conservative bit admission occur before
the screen. Its checked work reservation is `2 * rank_operations + entries`;
insufficient allowance or arithmetic overflow skips the screen without a new
rejection. Wide matrices also skip it. Modular storage is dropped before any
rational fallback allocation. This depends on matrix structure, not loop count,
sector names or a topology lookup table.

The focused release tests pass for an actual-prime rank-loss counterexample,
840 exhaustive small matrix decisions and primitive witnesses, maximal u16
entries, rectangular/deficient cases, exact work thresholds, overflow, and
unchanged bit/kernel errors. Independent mathematical and implementation audit
passes. The full core release suite passes 2,263 tests with zero failures and
32 existing ignored research tests. The application gate passes 117 library
and 72 integration tests; the fresh release CLI build also succeeds.

The frozen census harness compares every sector's full decision, exact kernel,
raw/effective support, rank/order/row metadata, family binding and complete
parameter-domain provenance. Nonempty domain polynomials use the existing
native binary catalog transport, with exact cold comparison of both ordered
maps. All family descriptions are external inputs. Analyzer construction and
census are timed separately from rendering, native output and comparison.

### Completed paired census measurements

All measurements below completed on September 20 after compilation and without
an overlapping owned build. Both clients use identical diagnostic source,
optimized release linkage, one worker on CPU94, nested pools limited to one,
an 8 GiB address-space cap and unchanged analyzer limits. The old core is the
frozen published `f29d6a84` library; the new core includes the one-sided native
rank screen. Symbolica and TOML library snapshots match exactly. The three
cube pairs alternate old/new, new/old, old/new; no warm-up is removed.

`census` includes analyzer construction and every decision, with decision
retention but no rendering or serialization. GNU wall/CPU/RSS cover the whole
process, including input, output and destruction. Retaining every decision is
a harness memory cost, not a production streaming-memory measurement.

| Pair | Old/new census (s) | Old/new whole wall (s) | Old/new CPU, user+system (s) | Old/new peak RSS (KiB) | Old/new census ratio |
| --- | ---: | ---: | ---: | ---: | ---: |
| 0 | 18.086212 / 7.466656 | 18.17 / 7.55 | 18.05 / 7.49 | 71,964 / 69,584 | 2.4223 |
| 1 | 18.514227 / 7.483701 | 18.59 / 7.56 | 18.41 / 7.50 | 73,544 / 73,136 | 2.4739 |
| 2 | 24.306422 / 7.542163 | 24.38 / 7.61 | 24.23 / 7.55 | 72,828 / 73,636 | 3.2227 |

The observed paired median ratio is **2.47394**, with median census time
18.514227 → 7.483701 s. The slower third old observation is retained; no cause
is assigned to its variation. These three local pairs do not establish a
confidence bound or a universal speedup. There is no consistent RSS reduction.
Analyzer construction remains 0.558–0.640 s across the six runs; enumeration
falls from 17.513/17.926/23.728 s to 6.908/6.903/6.903 s.

Every run visits all **32,768 raw masks** of the external 15-coordinate cube
input, obtaining 5,480 proved-zero and 27,288 inconclusive decisions, no
restriction exclusions and one retained coefficient-domain condition. This
unrestricted preparation census is not the physical-root 2,656-sector solver
campaign. Each pair and all four repeat comparisons preserve every primitive
kernel, rank, support, parameter order, binding and native domain polynomial
with both ordered coefficient maps. No classification becomes new closure
authority.

One fresh pair per smaller external input also completes:

| Input | Raw masks | Proved zero / inconclusive | Old/new census (ms) | Old/new whole wall (s) | Old/new peak RSS (KiB) |
| --- | ---: | ---: | ---: | ---: | ---: |
| FG | 1,024 | 281 / 743 | 60.744691 / 26.035895 | 0.07 / 0.04 | 6,200 / 6,204 |
| K6 | 64 | 26 / 38 | 0.933473 / 0.623472 | 0.01 / 0.01 | 6,200 / 6,180 |
| Formally shifted tadpole | 2 | 0 / 2 | 0.029610 / 0.039540 | 0.00 / 0.00 | 3,096 / 3,084 |

The last row retains a **9.93 µs regression** in this single tiny observation;
it is not evidence of a general small-case speedup. GNU time's zero values are
rounded, not zero work. The shifted input has two domain conditions and checks
raw-inactive/effective-active support plus the `[d, eps]` native map. All ten
native comparisons and all 22 census/compare stage statuses pass. Smaller-case
CPU user/system totals are 0.06/0.00 → 0.03/0.00 s for FG and below GNU time's
0.01 s reporting resolution for the other two inputs.

The fresh-CLI fourteen-stage K6 regression also passes: ordinary generation,
fresh one-/six-worker checkpointing, no-search complete resume, exact native
old/new comparisons, and four independent certifications plus cold canaries.
Each candidate contains 623 rules and 38 residuals across 38 sectors, with
1,417 unique coefficients and 335,561 bytes. Each certificate has 5,640 cells,
38 terminals and 3,731,581 bytes. Every canary reproduces 30 master terms and
93 applications. Resume reuses all 38 shards without search or shard mutation.
The frozen Python reader is an independent exact comparator, not a freshly
built Python generation extension. This is an end-to-end correctness regression,
not an isolated five-loop solver-speed comparison or a four-loop bounded proof.

Evidence: `TMP/zero-census-differential.N1txma/`, with cube `run.nme8j1`, FG
`run.dhs7YX`, K6 `run.tnv2Pa`, shifted input `run.j0bGqY`; release tests in
`TMP/modular-rank-bounded-gate.Hm3zTv/`; end-to-end regression in
`TMP/k6-modular-rank-regression.clUEcS/`. Independent source, mathematical,
provenance and measurement audits pass. Full five-loop closure and a completed
four-loop total-excess-30 certificate remain outstanding.
Evidence: `TMP/five-loop-preparation-profile.XpucXw/` and
`TMP/zero-census-differential.N1txma/`.

## Full physical campaign: further checkpoint progress, not completion

The frozen release CLI from `f49562cb` resumed the complete external cube
campaign from 64 saved sector shards. The mathematical inputs, natural
ordering, `sparse-factorized` backend and numerical depth zero were unchanged.
This segment used one worker on CPU 94, nested pools capped at one, a 32 GiB
virtual-address cap, an 8 GiB checkpoint cap and a declared 900-second deadline
with ten-second forced-termination grace. No owned build or benchmark overlapped.

It ended on September 20 at 08:56:11 UTC with **status 137** after the deadline,
without a final family bundle. Thirteen additional sector files were saved:
**77 of 2,656 physical nonzero sectors** are now durable. Those new files contain
3,647 rules and 335 finite-residual occurrences, occupying 228,647,241 bytes.
The residual count is not a deduplicated master basis. Every original shard
and input remained unchanged. The final observed sector, 4083, was unfinished.

Preparation reported its physical census at 7.5 s and checkpoint admission
at 10.5 s (64 reused / 2,592 pending). These rounded progress timestamps do not
isolate phase costs. Whole-command wall time was 915.91 s. Final solver CPU
time and peak RSS are **unavailable**: forced process-group termination also
killed the timeout supervisor before it could reap the child. GNU time's
0.00 CPU / 3,076 KiB RSS therefore describe only the supervisor and are excluded.
Live observations confirm solver work and modest instantaneous memory, but do
not recover either final statistic.

This is further saved candidate progress, not full-family closure or an
old/new performance ratio: the earlier six-worker attempt used a different
binary, prefix and deadline. The generic rank-screen preparation gain measured
above must not be extrapolated to complete five-loop generation. Evidence and
the measurement limitation are retained in
`TMP/five-loop-rank-screen-resume-one.SiVaOA/`.

A separately declared 600-second continuation from those 77 shards, using
the identical frozen CLI/input and one-worker policy, also ends incomplete:
status 124, **zero additional saved sectors**, still working on sector 4083.
Foreground timeout preserves supervisor reaping this time. Its whole-process
measurements are **600.15 s wall, 576.25 s user + 22.34 s system CPU, and
163,652 KiB peak RSS**. All input/prefix checks pass; no final bundle exists.
The census timestamp is **172.0 s**, versus 7.5 s in the preceding segment,
with checkpoint admission at 184.2 s. This much slower preparation is retained
as unexplained variability: no owned build overlapped, but neither phase CPU
attribution nor a cause is established. Different prefixes and deadlines
preclude a matched solver-speed ratio. Evidence:
`TMP/five-loop-rank-screen-resume-next.7EU4nQ/`. The subsequent release build
started only after this campaign's owned processes had exited.

## Full-root exact-lift follow-up

A separate 1,800-second run of the external five-loop banana input also ends
incomplete: no saved sector or final bundle, 1,800.88 s wall and approximately
7.56 GiB peak RSS. Short CPU recordings identify native factorized sparse-row
addition/multiplication in the sampled interval. The
[exact-lift report](five_loop_exact_lift_profile.md) records the full workload,
profiling limitations, independent audits and the next existing-backend control.
This shared-load, instrumented observation is not a completed-family benchmark.
