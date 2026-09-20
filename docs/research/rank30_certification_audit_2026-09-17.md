# Rank-30 certification checkpoint (2026-09-17)

## September 20 follow-up: total-excess native owner

The public Rust request
`CandidateCertificationRequest::with_max_total_excess_degree(D)` now invokes
the consuming `SourcePortAudit::install_complete_through_total_excess`.
Its entry contract is `sum(max(n_i-1,0) + max(-n_i,0)) <= D` within the declared
root downset: dots and negative powers both count. Complete source replay,
guards, descent, actual executable-cell coverage and every live RHS successor
check remain mandatory. Independently proved descendant-sector bounds may
exceed the entry bound. The existing reducer and scalar lowering reject an
out-of-scope start before cache access or mutation. `None` preserves the
unrestricted path. The matching CLI flag
`certify-candidates --max-total-excess-degree D` and Python keyword
`rustred.certify_candidates(..., max_total_excess_degree=D)` steer that same
service. Their distinct numerator-only option still rejects because dots remain
unbounded. Neither option is silently reinterpreted as the other.

A distinct `BoundedCertified` native kind requires a version-1 scope section
containing the degree convention, root, entry degree and canonical successor
map. The common transport remains version 1 and the source proof remains V6.
Cold loading reconstructs sources and actual cells, reproves their coverage
and successor containment, then compares native structure, exact coefficients
and both variable maps. Saved scope is not a trusted proof flag. Bounded owners
cannot enter the unrestricted vacuum-capability loader; zero-only roots remain
unsupported. Native State/Atom readers still require trusted generated input.
See the [contract and interface boundary](rank_bounded_certification.md).

### Initial native-bridge release verification

All final gates below pass. Counts describe distinct gate executions, not a
new count of numerically accepted Vakint cases.

| Gate | Result | Retained evidence under `TMP/` |
| --- | --- | --- |
| Core library | 2,281 passed, zero failed, 32 ignored; 1,021.98 s execution, excluding 55m26s compilation | `bounded-native-successor-gate.l2WF8U/core.log`, `core.status` |
| Fresh native process | Two passed, zero failed, one child-process helper ignored; 0.12 s execution | `bounded-native-successor-gate.l2WF8U/core-native.log`, `core-native.status` |
| Application | 121 unit and 72 integration tests passed, zero failed | `bounded-native-successor-gate-fixed.hkPJLi/app.log`, `app.status` |
| Python binding Rust tests | Eight passed, zero failed | `bounded-native-adapter-gate.TciDG4/python.log`, `python.status` |
| Release CLI and fresh extension | Successful CLI/extension builds and default-API smoke; 38 Python/CLI tests passed in 56.325 s | `bounded-native-adapter-gate.TciDG4/cli.status`, `python-build.status`, `python-import-default.status`, `python-tests.log`, `python-tests.status` |
| Frozen source verification | All 40 manifest entries match | `bounded-native-adapter-gate.TciDG4/source-after.log` |

The native-process tests exercise altered Symbolica registration order and
exact bounded cold replay. Independent source/mathematical and genericity
audits found no blocker. Singleton sign partitions now borrow their original
box, and immutable source ordering keys are built lazily once per RHS.
These are generic reuse changes, not topology-specific shortcuts, new algebra
or relaxed proof budgets. Observational counters distinguish retained
propagation from final actual-cell checks.

The unchanged unrestricted K6 control also passes without rule generation:
the new CLI cold-loads the saved certificate, recertifies saved candidates,
and cold-applies the resulting certificate. A fresh Python process compares
native structure, coefficients and both maps exactly, and reproduces the full
saved canary report: 5,640 cells, 38 terminals, 30 output terms and 93 rule
applications. All four stage statuses are zero in
`TMP/bounded-native-k6-saved-control.HgUlGr/`. This is a compatibility regression,
not a new generation measurement or evidence of four-loop closure.

Failed setup evidence is retained, not counted as a passing execution. The
initial app test run passed 120 tests and failed one at a new test-only
`toml::Value::FromStr` document parse. Both such calls were changed to
`toml::from_str` without changing assertions. The next
isolated Python build lacked an interpreter, fixed by explicitly naming the
existing workspace interpreter. The standalone FG client needed a harness-only
`SmartString` error conversion for its build. These records remain in the
original gate directories and `fg-bounded-native-acceptance.DMv9MT/CLIENT_BUILD_FIX.md`.

### Frontend and progress follow-up

The separate release/locked/offline gate
`TMP/total-excess-frontends-progress-gate.B97PKw/` completes successfully. Its
five stage statuses are zero, and the root wrapper reports
`FRONTEND_PROGRESS_RELEASE_GATE_PASS`. No core gate was rerun during this
follow-up; the preceding counts retain their original checkpoint meaning.

| Gate | Actual result |
| --- | --- |
| Application | 131 unit and 74 integration tests passed; zero failures or ignored tests |
| CLI build | Release CLI built successfully and copied into the frozen test snapshot |
| Python binding Rust tests | Eight passed; zero failures or ignored tests |
| Fresh extension build | Matching release extension built and copied into that snapshot |
| Python/CLI runtime | 41 passed in 52.884 s; zero failures or skips |
| Frozen inputs | All 13 source-manifest entries and all three executable/package snapshot entries match after execution |

The frontends steer the existing consuming installer. Strict optional-u64
validation accepts zero, 31 and the representable maximum without promising
that any requested proof fits its resource budget; omitted/`None` remains
unrestricted. Generation does not accept the option. Total-excess and
numerator-only requests conflict rather than silently changing meaning.

External K1/K3 fixtures establish exact native-structure, coefficient and
variable-map equality across the public Rust request, CLI and Python. Fresh
CLI processes load Python-produced bounded bytes and a fresh Python process
loads CLI-produced bytes. Dotted, negative-index and pinched starts reproduce
the unrestricted reductions inside scope; out-of-scope starts are rejected.
Additional tests cover E0/E31, no-overwrite and restrictive proof-budget
failures. These are small input-driven regressions, not loop-specific solver
branches or a complete four-loop certificate.

Independent source review also covers the observational progress changes:
existing exact-materialization scalar events reach application/CLI stages;
per-live-job frame records are updated before the unchanged shared throttle,
invalidated on transitions and removed on completion. Exhaustive event
projection, worker isolation, missing/overflowed headers, lazy disabled
observation and the existing 10,000-event throttle test pass. No new exact
callbacks, CAS work or row/expression snapshots are introduced. Progress cannot
refresh inside a native call without a callback, and frame counters are not
exact-input identities. Public enum variants are source-visible; persistence
and owner authority are unchanged.

The retained independent audit is
`TMP/exact-materialization-progress-independent-audit-2026-09-20.md`; a measured
receipt is `TMP/bounded-frontend-progress-acceptance-2026-09-20.md`. This
follow-up has no failed stage or skipped cross-frontend subprocess test.
Neither full-FG failure below is superseded. No Vakint asset, dependency pin
or numerical acceptance claim changes.

### Complete saved FG E30 attempt: resource-limited, not certified

The new consuming installation was run on the unchanged saved full FG program:
124 nonzero sectors, 9,272 rules, 145 raw finite terminals and 126,549 source
trace entries. No IBPs were regenerated. The entry degree is 30 and all input,
publication and proof resource limits retain their defaults. It ran with one
worker on CPU 93, a 600-second external deadline and 32 GiB virtual-address cap
(not an RSS cap). Concurrent work makes this a shared-load observation, not
an isolated benchmark or a paired comparison with earlier diagnostics.

The process exits with status 1 and typed `AppErrorKind::Limit`, naming
`total-excess successor geometry`. All 47 completed local reports have zero
issues and zero uncovered boxes. Retained successor propagation completes 46
sectors before stopping within sector 107; the final actual-cell pass and
installation are not reached. The rejected charge is:

| Counter | Consumed | Attempted after next charge | Limit |
| --- | ---: | ---: | ---: |
| Boxes | 212,518 | 212,526 | 1,000,006 |
| Coordinate cells | 10,996,840 | 10,997,160 | 12,000,072 |
| Work | 11,999,957 | 12,000,280 | 12,000,072 |

Whole-process wall time is **12.54 s**, user CPU **12.21 s**, system CPU
**0.23 s**, and peak RSS **211,972 KiB**. The public certifier reports failure
after 12.349654 s; its observer reaches preparation at 0.167124 s and prepared
sources at 0.197431 s, then the failed successor event at 12.291067 s. No
complete replay/lowering/encoding boundary exists for this failed run.
No bounded artifact or success report is written, and the separate cold-load
and runtime-canary stage is not run. Input/library hash checks pass.

Evidence: `TMP/fg-bounded-native-acceptance.DMv9MT/` contains `certify.log`,
`certify.err`, `certify.status`, `certify.time`, the frozen client and hash
records. This is neither a completed full-FG certificate nor evidence of a
missing IBP. No four-loop Vakint asset or dependency pin changes.

### Separately declared caller-resourced attempt: deadline, still incomplete

One follow-up used the unchanged full saved program, entry E30 and frozen
optimized libraries, with four explicit caller allowances: cover boxes
4,194,304; cover coordinate cells and work 268,435,456 each; rule-derivation
endpoint cells 65,536. The cold loader was prepared with matching allowances.
Defaults, exact proof semantics, predicate/algebra limits and output ceilings
were not changed. This was one bounded experiment, not automatic quota tuning.

It reached the 600-second deadline with status 124: **600.12 s wall,
584.92 s user, 14.55 s system, 233,476 KiB peak RSS**. All 48 completed local
reports have zero gaps/issues and account for 3,094 replayed rules; 48 retained
successor passes complete. The last completed sector is 203, and the last
callback starts rule ordinal 78 of sector 115. There is no completed lowering,
actual-cell pass, installation, artifact, or cold run. All recorded input and
library hashes pass. A timeout is incomplete evidence, not a failed identity.

The first 47 local sector/count tuples match the default-policy run exactly;
the first 46 retained snapshots also match after excluding limits and elapsed
time. Nevertheless this run contains substantial unexplained delays already
in preparation, before the changed proof-work allowances can explain them.
Do not infer a speed ratio or attribute those delays to sector reordering.
These remain shared-load exploratory measurements.

Evidence and independent interpretation are retained in
`TMP/fg-e30-caller-policy.4Ap28o/`, including `TERMINAL_AUDIT.md`.
No further certification retries or quota increases belong to this slice.
The optional full-FG certificate remains unfinished; the shipped four-loop
numerical programs, terminal catalogs and Vakint dependency pin are unchanged.

## Historical checkpoint record

The dated sections below preserve the earlier implementation boundaries and
measurements. Their statements about missing total-excess publication/native
transport describe those checkpoints, not the implementation above.

### Follow-up: degree-scoped exact audits (2026-09-19)

The public Rust diagnostic `SourcePortAudit::audit_sector_through_total_excess`
now requests coverage through an explicit **total excess**
`sum(max(n_i-1,0) + max(-n_i,0)) <= D`. It still performs the existing full
original-source replay, original guard checks and supplied-domain strict-descent
proofs. Its report records `max_total_excess_degree: Some(D)`; the ordinary
whole-sector report records `None`. A diagnostic report cannot publish an
artifact. The current complete-program installer additionally rejects bounded
report metadata, and the CLI/Python rank-certification option remains
fail-closed as described below.

The coverage implementation reuses the exact Boolean/predicate and `BoxCover`
traversal. A coordinate hull bounds traversal but is **not** the claimed domain:
an uncovered rectangle matters only if its lower corner satisfies the exact
degree sum. This avoids enumerating simplex points. Affine exceptions remain
exact predicates, and unresolved feasibility still fails closed. Degree probes
share the existing cumulative geometry-work allowance. Numerator-only geometry
retains genuinely unbounded positive powers; it is not silently converted into
a total-power bound.

The checker remains conservative for coupled affine predicates intersected
with the sum-bound. A follow-up tightens each remaining box before the existing
native affine checks: for counted axis `i`, its upper endpoint is at most
`D - sum(lower_j, j != i)`. This is the exact coordinate hull of the
box/simplex intersection, not a replacement for that intersection as an owner.
It resolves the small diagonal example `x=y`, `x+y<=1` with an explicitly owned
origin: `x>=1` forces `y=0`, and conversely. No new polynomial arithmetic,
sampling, or general affine/simplex feasibility solver is introduced. The
same cumulative geometry allowance pays for the extra coordinate work, and
unresolved affine feasibility still fails closed. Numerator-only bounds leave
positive axes uncapped.

An internal, immutable `EntryScope` binds the family, root sector and exact
degree convention. A proposed rectangular proof envelope composes the existing
successor-cover owner, proves entry containment by the same complement-minimum
test, and checks source/RHS images using one cumulative budget. It is not an
artifact certificate or a new CAS. This contract is a reusable boundary;
full bounded installation, persistence and runtime admission are still pending.

### Complete-program successor-envelope diagnostic

`SourcePortAudit::audit_complete_through_total_excess(&family, sectors, D)`
now consumes the same solved-sector tuples as the existing installer, but
returns an immutable `SourcePortTotalExcessAudit`, not a `ClosedArtifact`.
It validates the complete nonzero sector census beneath the root, checks the
family and common ordering, then processes sectors from harder to easier using
the persisted ordering's sector-corner keys. Every entry sector initially
receives bound `D`.

Each sector undergoes unchanged full original-source replay, guard checks and
uniform strict descent, with coverage requested through its propagated degree.
Only retained checked rules contribute edges. Their physical index shifts are
partitioned at sign changes; independently proved zero sectors and exact empty
or zero-coefficient branches may be removed. Same-sector descent cannot
increase total excess. A strictly lower-sector edge propagates the bound
derived below. An unexplained nonlower or out-of-root edge fails closed.
The report exposes both `max_entry_total_excess_degree()` and the complete
`successor_degrees()` map, so descendants are never silently subjected to the
entry cap. Input mask iteration order is not used as the reduction order.

One cumulative structural allowance covers the census and sign partitions.
Existing native algebra limits remain per proof query; this is not a global
Symbolica scratch-memory bound. Affine predicates are preserved and may still
cause a conservative failure. No installer accepts this diagnostic as an
artifact, and no persistence or runtime scope check is bypassed.

### Why entry bounds and descendant bounds must differ

Independent inspection of the saved four-loop rules found actual escapes:

* H: `(-29,0,0,1,1,1,-1,1,1,0)` has negative degree and total excess 30.
  Its first applicable stored rule contains, with coefficient one,
  `(-29,0,-1,1,1,2,-1,0,1,0)`: negative degree 31, total excess 32.
* FG: `(0,0,1,1,1,1,1,1,0,-30)` similarly produces
  `(0,-1,1,1,1,2,1,0,0,-30)` with the generically nonzero coefficient
  `(d+29)/(240-120*d)`, again increasing the degrees to 31 and 32.

These are checks of saved rule domains and index shifts, not new source
certificates. Same-sector numerator increases also occur while dots decrease,
so an unbounded-dot numerator scope needs more than a sector-only argument.

For an explicitly requested total-excess scope, the existing strict ordering
gives a simpler route: within one sector total excess cannot increase. Across
sectors, a conservative bound for a physical shift `s` is
`E(child) <= E(parent) + sum(abs(s_i)) + p(parent) - p(child)`, where `p` is
the number of positive indices. Propagating such bounds through the finite
strict sector-order graph gives proposed per-sector degree envelopes. Each
envelope must still pass coverage, source replay, guards and successor checks;
none of this assumes a minimal terminal basis.

The next integration should reuse whole-domain replay/descent proofs where
they succeed, attach the entry/envelope semantics at artifact level, recheck
them on cold loading, and enforce entry scope before memoization. Bound 30 is
not itself a cure for every old cover failure: the historical difficult FG214
abstract Boolean witness had total excess only four. Its feasibility obligation
must still be discharged exactly.

Independent counterexamples and integration audit are retained in
`TMP/bounded_certificate_math_audit_2026-09-19.md`. No four-loop bounded
artifact is claimed at this checkpoint.

The preceding focused release gate passed **306 tests**, with five pre-existing ignored
workloads. This includes ten entry/envelope contract tests, five degree-cover
tests (one exhaustively compares all 512 subsets of a 3x3 grid at three bounds),
and two source-port integration regressions. The first run had one overstrong
test expectation for the affine diagonal example above; it was corrected to
require the conservative rejection, with no production proof weakened. Both
logs remain in `TMP/bounded-scope-release-tests*.log`. Independent source and
mathematical reviews approve the implementation; formatting checks pass.
The subsequent full release core suite also passes: **2,187 passed, 31 ignored,
zero failed** in 56.37 s (compilation excluded). Its record is
`TMP/bounded-scope-full-core-tests.log`. This is a regression gate, not a
four-loop artifact certification run.

### Measured FG214 follow-up and regression gate

The saved four-loop FG sector `0110101100` (mask 214) now passes the actual
selected-sector exact audit through **total excess 30** with unchanged default
proof limits. All 161 rules replay from original IBP sources and uniformly
descend; stored and checked predicate covers both leave zero uncovered and
zero unbounded boxes, with no issues and 11 explicit terminal declarations.
All 43,582 saved `SeedSource` entries were retained. The resulting replay
report counts 11,217 nonzero original-source contributions; these are different
censuses, not contradictory source counts.

The release, single-worker CPU-83 run completed in **74.837 s** for the audit
call and **75.15 s** for the whole process, using 74.40 s user plus 0.11 s system
CPU and 101,376 KiB peak RSS. The audit timer includes exact replay, descent
and both coverage checks; loading, the zero-sector census and owner setup are
outside it. No candidate search, rule regeneration or proof-budget increase
occurred. The 300-second external limit and 32-GiB virtual-address-space cap
were not reached. This is one completed observation, not a timing distribution.

The earlier cheap probe independently passed stored-guard coverage in 0.709 s
(1.11 s whole process), but deliberately removed source traces and proved no
identities. It remains separate evidence, not part of the exact proof result.
Evidence and reproducible clients are in `TMP/fg214-degree-cover.52OJcp/` and
`TMP/fg214-original-audit.wUmO3W/`, with native input/library hashes and the
unchanged production codec used to read the saved candidate solution.

**This is not a complete FG-parent artifact.** It does not check the complete
parent census under propagated successor degrees, lower the rules into runtime
cells, persist the scope, cold-replay a bounded owner, or test runtime entry
admission. Those are the next gates. In particular, degree 30 here is a
selected-sector coverage bound, not a claim that all its descendants have
degree at most 30.

### Complete FG-parent attempts (2026-09-19): structural budget, not closure

Two subsequent release attempts use the unchanged saved full FG parent:
**124 sectors, 9,272 rules, 145 terminal declarations and 126,549 source-trace
entries**. Both start with total excess 30 in every entry sector and propagate
larger descendant bounds in checked reduction order. These are complete-parent
requests, not attempts on a selected easier subfamily. Neither completes.

| Measurement | Default caller policy | Explicit larger caller policy |
| --- | ---: | ---: |
| Completed sector audit reports | 49 | 68 |
| Completed successor-propagation stages | 48 | 67 |
| Exact source replay and descent checks passed | 3,271 | 4,555 |
| Gaps / issues in completed reports | 0 / 0 | 0 / 0 |
| Largest degree among checked sectors | 96 | 106 |
| Audit wall time | 120.768 s | 119.413 s |
| Whole process wall time | 121.21 s | 119.97 s |
| Peak RSS | 205,824 KiB | 205,824 KiB |
| Exit status | 8, typed resource exhaustion | 8, typed resource exhaustion |

Both stop during successor propagation with
`ResourceBudgetExhausted { resource: "total-excess successor geometry" }`.
The error identifies the shared structural ledger, not which individual
subquota was exhausted. The default attempt stops after sector 115
(`1100111000`, degree 80); the second stops after sector 158
(`0111100100`, degree 89). Completed reports agree exactly over their common
prefix when elapsed times are removed. No failed identity or uncovered branch
was reported before the resource limit; unvisited sectors remain unchecked.

The explicit policy changes only the cover replay allowances: requested boxes
1,000,006 → 1,048,576, requested coordinates 12,000,072 → 16,777,216, and shared
split work 12,000,072 → 16,777,216. These are shared coverage/descent/envelope
allowances, not a successor-only knob. Native affine work remains 4,194,304,
predicate atoms remain 32, and exact-algebra and uncovered-box limits remain
unchanged. Neither production defaults nor proof semantics change.

The runs use the same frozen optimized libraries, input and codec, one worker
on CPU 83, a 600-second external deadline and a 32-GiB virtual-address-space
cap. Neither external limit is reached. This is one observation per policy,
not a performance comparison: the amounts of completed work differ. Raw
evidence is in `TMP/fg-parent-degree30.PZl0OF/` and
`TMP/fg-parent-degree30-policy.ayJHfx/`. No candidate search or rule regeneration
is performed; original IBP sources are regenerated and replayed. Bounded
artifact installation, persistence, cold verification
and runtime entry checks remain pending independently of this diagnostic.

The new complete-program diagnostic is exercised on real K1 and K3 solver
outputs, including arbitrarily ordered input sectors, propagated bounds,
foreign/nonlower successor rejection, exact zero pruning and cumulative
geometry limits. Exhaustive small-box tests check the tightened degree hull's
coordinate extrema, and extreme endpoints and numerator-only infinite axes
are covered. Independent mathematical and implementation reviews pass.
The focused release suite now passes **319 tests, five ignored**; the full core
release suite passes **2,200 tests, 31 ignored, zero failures**, in 41.35 s
excluding compilation. Logs are `TMP/total-excess-focused-release-tests.log`
and `TMP/total-excess-full-core-release-tests.log`; formatting checks pass.

## Original numerator-only decision (still unsupported)

RustRed now exposes an explicit, fail-closed request for a bounded entry
degree.  `max_negative_index_degree` is accepted only up to 30.  A request is
rejected before candidate decoding with a diagnostic that names the missing
successor-closed artifact contract; RustRed never silently runs the existing
whole-family certification and relabels it as a rank-bounded proof.  Values
above 30 are input errors.

This is deliberately a capability boundary, not a claim that a rank-30
four-loop artifact is complete.  It prevents expensive unrestricted retries
while the bounded proof path is being implemented.  The ordinary
`certify-candidates` behavior and artifact schema remain unchanged when the
option is omitted.

## Historical missing integration at the original checkpoint

The repository already contains useful exact geometry:

* `source_port/scope/rank.rs::RankScopeBudget` enumerates the requested entry
  set
  \[
    E_D = \{n : \sum_i \max(-n_i,0) \le D\}
  \]
  as deterministic negative-index slices with genuine infinite positive-power
  rays.  Checked arithmetic and cumulative geometry budgets are in place.
* `predicate_cover::certify_predicate_cover_within` proves ownership of a
  required union, but only proves coverage.  It does not prove source replay,
  guards, strict descent, or closure of rule RHS images.
* `scope/successor.rs::SuccessorClosedScope` admits an immutable destination
  box union, checks entry containment, and checks translated RHS images with
  the existing exact sign-partition geometry.  It is an internal building
  block and intentionally does not own affine predicates, source traces, or a
  durable artifact.

The publication path still has the following whole-family assumptions:

1. `SourcePortAudit::check_sector_with_observer` runs predicate coverage over
   the complete sector orthant.  The scoped coverage helper is not wired into
   replay or descent.
2. `CheckedProgram` lowers retained applications without carrying a semantic
   proof domain.  Source rows and all original guards are checked, but no
   finite rank envelope is attached to a rule cell.
3. `ClosedArtifact` stores only the rectangular root bounds.  Its durable
   codec has no sum-bound, proof-domain union, or successor-closure witness.
4. `Reducer::validate_target` checks the rectangular root prefilter only.  It
   cannot enforce a caller's exact entry degree after cold loading.

Consequently, using the old installer after parsing a rank option would either
still require the expensive unbounded certificate or publish an artifact that
could accept an out-of-scope target.  Both outcomes would misrepresent the
mathematical guarantee, so the new option fails closed.

## Original integration checklist

The smallest sound rank-30 implementation must compose all of the following,
without introducing a new CAS:

1. Carry an immutable `EntryScope` and independently proposed proof envelope
   through the checked artifact owner. Keep existing whole-domain cell proofs
   where they succeed; attach narrower domains to cells only when their proofs
   actually use scope restrictions. Source replay retains the original full
   rows, never a sampled or uncertified projection.
2. Prove coverage of the envelope, exact source applicability and guards, and
   strict descent on every retained cell.  A rank slice is not evidence by
   itself.
3. Run `SuccessorClosedScope` after exact RHS pruning for every rule that can
   be selected by runtime precedence, including affine exceptional branches.
   Any escape is an inconclusive failure, not a sampled success.
4. Version and persist the entry predicate and proof envelope.  Cold decoding
   must repeat the same containment and replay checks from untrusted bytes.
5. Make the reducer enforce the exact entry predicate before cache lookup and
   scalar lowering.  Descendants are governed by the persisted inductive
   envelope, not by the entry bound.

Only after these gates are connected should the CLI/Python option become a
working bounded certificate.  A larger envelope or full-sector widening may
be added later, but must report both the requested entry set and the proved
successor domain.

## Tests for this checkpoint

The option is covered at the application and CLI boundaries:

* `CandidateCertificationRequest::with_max_negative_index_degree(30)` returns
  an execution error naming the absent successor-closed scope and does not
  decode or certify the bundle.
* Degree 31 returns an input error before any native work.
* `rustred certify-candidates --max-negative-index-degree 30` exits nonzero,
  emits no artifact bytes, and reports that an unbounded fallback was refused.
* The help text and Python stub document the same bound and fail-closed
  behavior.

These tests intentionally do not claim closure.  Existing unrestricted K1/K3
candidate certification and all existing source-port geometry tests remain
the authority for the current artifact path.
