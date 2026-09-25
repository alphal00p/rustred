# Optional application-cell refinement: implementation and controls

This experiment targets the expensive inspection of saved parametric rules,
not rule generation. The live five-loop campaign remains on its frozen binary
and unchanged Ordered policy. The new option is off by default; the results
below do not establish five-loop closure or a campaign completion estimate.

## What changes

`OwnerAppliedCellRefinement::SingleFiniteAxis { max_cardinality }` optionally
partitions an already matched application cell into singleton slices when
exactly one coordinate varies over a sufficiently short finite interval. All
other coordinates must be fixed, and endpoints must fit the existing physical
index API. Ineligible and affine-adapter cases retain the existing path.

Matching happens once. Every child retains the original selected rule, source
guards and coupled A/R/D predicates. Each complete equal-shift coefficient group
is applied intact using existing Symbolica arithmetic and existing validity and
descent checks. There is no new CAS primitive, topology dispatch, worker pool,
or assumption that a numerator-rank bound also bounds positive powers.

The motivation is smaller symbolic expressions: fixing one more index can make
exact coefficient specialization much cheaper even when more application cells must be
visited. Conversely, narrower successor pieces can increase downstream queue
work. Both effects must be measured.

Work allowances and cancellation remain shared by the original inspection.
After a child has emitted effects, a later failure cannot fall back to replaying
the unsplit parent. Statistics distinguish per-shift refinement steps/cells from
matched pieces and completed inspections. The complete policy, including the
cardinality threshold, is bound into checkpoint replay.

The Rust request exposes the typed policy through `OwnerAppliedLimits`.
The CLI and Python steering scripts expose
`--apply-cell-refinement-max-cardinality N`; omission means Off. Guarded-apply
JSON limits expose `applied.cell_refinement_max_cardinality`. This is not a newly added
native `import rustred` owner-walk API. See [CLI usage](../CLI.md).

## Validation

Independent implementation and mathematical reviews cover exact partitioning,
original source authority, complete shift groups, exceptional faces, resource
accounting, cancellation and replay. Release-profile checks pass:

- 58 focused core tests, including 15 new planner/native tests;
- 705 application/CLI tests, with one existing ignored test;
- 60 Python steering tests;
- explicit licensed two- and six-worker native tests that replay an on-disk
  refined prefix and reproduce the completed records.

The first broad application run found a setup error in the new synthetic
checkpoint test: after restore it supplied a finished physical part without
marking the restored responsibility Started. The test now follows actual resume
dispatch; all original assertions remain. That failed run is retained and is
not counted as a pass. No runtime workaround was introduced for it.

The corrected broad test used an isolated optimized correctness cache. Timing
executables instead use the normal release library profile throughout; no
test-cache library enters the comparison. Local evidence is retained under
`TMP/cell-refinement-gate.Z2I41S/`,
`TMP/cell-refinement-fast-test.fnnozm/` and
`TMP/cell-refinement-frontend.hQK5wl/`.

## Same-query five-loop inspection

The exact saved head 9694360 is input data, not an engine specialization. Both
processes load the same 67 immutable owner programs once. One runs Off/On/Off;
the other runs On/Off/On, with threshold 2. Both use the same freshly linked
optimized executable, CPUs 50–55, serial inner pools and a RAM-only guard.
There is no elapsed/work cutoff. Compilation and production checkpoint writes
do not overlap these measurements; unrelated host activity remains possible.

| Measurement | Off | Refinement On |
|---|---:|---:|
| Median native inspection wall | 7.4406 s | 3.4009 s |
| Range across three calls | 7.4206–7.5328 s | 3.3763–3.4819 s |
| Median native process CPU | 7.37 s | 3.38 s |
| Matched pieces | 139 | 139 |
| Shift groups | 36,974 | 36,974 |
| Refinement steps / visited children | 0 / 0 | 20,511 / 41,022 |
| Successor callbacks | 36,354 | 56,516 |
| Native operations | 128,341 | 205,742 |

This is a **2.19× local serial wall-time improvement**, accompanied by 55.46%
more successors and 60.31% more native operations. It is not a parallel or
recursive speedup. All six calls complete with zero local gaps, unresolved
pieces, problems and unsupported successors. Same-mode non-timing receipts
match exactly across processes. Conditional successors remain 461; three
optional original-coefficient refusals remain visible in each call.

Program loads take 13.53 and 13.40 seconds. Whole helper processes take 34.41
and 30.22 seconds, including loading, three calls, output and teardown, with
about 5.97 GB peak RSS. RSS is not isolated per mode. The bounded, time-selected
piece diagnostics are not a full coefficient-output certificate. Raw results
and an independent audit are in `TMP/post-match-refinement.nptTGC/`.

## Complete recursive controls

Use the existing four-parent input controls and all 900 saved nonzero sectors,
the same frozen release CLI, Ordered W6/H256, original roots plus auxiliary
covers, and every escaping descendant. Only the optional refinement policy
changes. Two opposite-order rotations were prepared in advance; timings are
serialized and use CPUs 56–61, a RAM-only guard and no work/time deadline.
These are the [restricted starting domains](four_loop_saved_cover_control_2026-09-24.md),
A<=19, R<=12, A-R>=7, with the previously validated auxiliary covers. They
are not a new arbitrary-index family-closure claim.

The first FG pair exhausts all 98,909 obligations, including 98,869 native
inspections, in both modes. Native traversal takes 12.7971 seconds Off versus
12.8942 seconds On, a 0.76% increase; this is not an FG speedup. Refinement
visits 10,272 children in 5,136 steps, adding 4,362 events and 8,000 native
operations without changing the number of native or logical responsibilities.

All 16 predeclared executions now finish successfully. The complete two-rotation
timings, including the slower reverse BMW result, are:

| Parent | Off traversal, rotations 1 / 2 (s) | On traversal, rotations 1 / 2 (s) | Median paired On/Off wall ratio |
|---|---:|---:|---:|
| FG | 12.7971 / 12.9488 | 12.8942 / 12.7883 | 0.9976 |
| BMW | 31.0235 / 31.1944 | 31.0711 / 34.4256 | 1.0526 |
| H | 12.1016 / 12.2091 | 12.1020 / 12.1214 | 0.9964 |
| X | 31.7281 / 31.7715 | 31.8592 / 31.5697 | 0.9989 |

These are native traversal times; loading, final output and guard polling are
separate. Median paired whole-child CPU ratios are respectively 0.9990, 1.0215,
0.9955 and 0.9987. Those CPU measures include the rest of the child process and
are not isolated traversal CPU. Whole-child peak RSS ranges from approximately
0.65 to 1.83 GB. No measured process overlaps a production checkpoint write.
Two pairs per parent on a shared host do not establish statistical
noninferiority or a general speedup.

BMW changes from 147,233 to 147,166 native responsibilities and from 158,951 to
158,866 logical obligations; the other parents retain their counts. This is
changed cover/reuse geometry, not permission to drop obligations: both modes
must discharge their complete initial, native, delegated and anchor ledgers.
The author validators and independent full-record review pass those gates with
zero pending work, frontiers, errors and unsupported successors in every run.
The independent review streams 1,319,758 records and 1,270,298 native
publications, checks forward delegation chains and completed same-owner anchors,
and reconciles all initial queries, pool/ledger state and final checkpoints.
Same-mode native counters repeat exactly across rotations. The sum of the eight
Off traversals is 175.7742 seconds versus 178.8315 seconds On, a 1.74% increase;
whole-child CPU increases 0.65%. All raw results and audits are retained under
`TMP/cell-refinement-four-parent.UM8Gi1/`.

## Production implication

Keep refinement opt-in. The local result is encouraging for costly symbolic
cells, but current live profiles also show admission-heavy phases: more
successor fragments can offset cheaper algebra. The four-parent controls show
no general recursive benefit; promotion would require relevant five-loop
traversal evidence, not the single-head ratio. Ready publication, task
grain, index improvements and auxiliary-domain changes remain distinct
experiments and must not be credited to this measurement.
