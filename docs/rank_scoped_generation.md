# Rank-scoped candidate generation

The optional `max_numerator_rank = R` means
`sum_i max(-n_i, 0) <= R` for **starting integral indices** in the supplied
family presentation. It counts scalar-product numerator degree, not tensor
momentum rank. Positive denominator powers are unbounded. This differs from
the optional certification scope `max_total_excess_degree`, which also counts
raised denominator powers.

The active five-loop goal is all-family coverage at least through R=10,
ideally R=20, allowing a finite nonminimal terminal set. Only afterward come
terminal minimization, published numerical master values and Vakint five-loop
integration. Small examples and isolated exceptional branches do not satisfy
the complete census milestone.

## Latest parallel-campaign snapshot — September 21, 15:39 UTC

Under the authorized 100-core/500-GB aggregate ceiling, the four broad
SearchFinite campaigns have saved **6,221/8,246 distinct labelled sectors**,
representing **66/67 graph classes**. This snapshot excludes the separate
finite-retention experiments. The 7,596 saved occurrences include 1,375
duplicates; 2,025 distinct labels remain unsaved. Only class 30231 is not yet
represented in these broad-search checkpoints. These are generation counts,
not proof of recursive rank-10 closure or a complete universal artifact.

These are durable-file counts, not the progress display's generation counts:

| Published input parent | Saved / scheduled sectors |
|---|---:|
| 30527 | 1,016 / 2,686 |
| 30699 | 2,142 / 2,580 |
| 31740 | 2,002 / 2,656 |
| 32745 | 2,436 / 2,478 |

The sector sets overlap between parents; their sum is not the distinct union.
Four generated solutions, native 3010, 3822, 31780 and 14343,
failed coefficient-table output admission and therefore do not appear among
saved sectors. Their first rejected table additions slightly exceed 512 MiB;
that is **not** a measurement of their complete encoded size. Native 3822
belongs to the last unrepresented class. An isolated explicit-budget retry is
separate from the original campaign and must actually publish before it counts.
The other three output failures belong to already represented classes, so
saved alternatives are preferred to regenerating their large expressions.
This durable-only snapshot was read between 15:39:57.137 and 15:39:57.541 UTC;
standalone retries and retained-policy checkpoints are not silently merged.
Evidence: `TMP/rank10-owner-milestone-inventory.3tBRix/`.

The new class 29550 owner is a different label: external 13887/native 32310,
parent 30527 shard 1091. Its 40,697,123-byte program has 487 rules and 56 finite
residuals. Fresh-process admission takes 8.467 s; the whole diagnostic takes
8.62 s and 259,464 KiB peak RSS. Declared-terminal identities pass. A nontrivial
rank-one input traces 370 keys, applies 2 rules, and leaves 361 uncovered successors
because only that one sector is loaded. The successful diagnostic exit is
not a complete reduction. Saved lower-sector work must be reused to resolve
the frontier; no IBPs were regenerated for this check.

Two isolated ordering alternatives for native 29751 both reach their explicit
900 s deadlines without exporting a sector: reverse ordering stops after
900.04 s wall/892.98 s CPU/390,108 KiB peak RSS, and half-rotation after 900.09 s
wall/892.92 s CPU/813,912 KiB. The former remains in exceptional geometry;
the latter continues exact elimination. These censored observations establish
neither a successful speed comparison nor impossibility of those orderings.
Natural ordering's separately captured residual is integer-empty; a bounded
native-Lex fallback now proves that exact conjunction empty without bounding
its positive-power directions. The independently reviewed release core gate
passes **2,438 tests, zero failures, 32 existing ignored**; all **246 release
application/integration tests** also pass. Its full native 29751 retry passes
the former case 285 blocker, but fails later at case 288 on compact index 156:
265.431 s solver time, 272.94 s process wall, 270.81 CPU-s and 429,796 KiB
peak RSS. It writes no sector program. The complete captured conjunction
forces n2=113 and n1=156, yet another retained equation evaluates to 596904;
native Symbolica returns the unit ideal in 69.871 microseconds. This proves
that particular conjunctive branch empty, not its parent or valid siblings.
It does not justify clipping or bounding positive indices.

The same-policy native 24996 SearchFinite regression does complete:
57.952 s solver time, 64.91 s process wall, 64.43 CPU-s, 192,336 KiB peak RSS,
530 rules and 27 finite residuals. Its candidate bytes exactly match the
earlier successful GrevLex output. Fresh loading and three declared-terminal
identity canaries pass; three nontrivial traces remain partial, with 1,060 /
2,835 /5,093 uncovered keys. Lower-sector dependencies still preclude a
family-closure claim.
The public seven-sector R10 regression reproduces the previous candidate
bundle byte-for-byte: 1,299 rules, 1,208,801 retained terminals and
41,971,427 bytes, in 37.02 s wall/40.88 CPU-s with 373,984 KiB peak RSS.
This is a regression check, not a matched performance comparison.

The fallback is generic: native GrevLex remains the normal form used first.
Only after the existing exact refinements decline an unsupported conjunction
does a structurally small input receive one native Lex attempt per work item.
Admission allows 2–8 equations, at most 3 used variables, 128 terms, degree 4
and 128-bit coefficients. The complete ideal and parent remain authoritative;
guard factors do not discard other conjuncts or siblings. Work/normalization
budgets are shared. These input caps are not a hard time or memory bound on
Symbolica's native F4 call. No new CAS operation or topology-specific rule is
implemented. Gate receipts: `TMP/late-lex-release.OVV0ET/`,
`TMP/late-lex-app-cli-gate.gqCIYf/` and `TMP/late-lex-public-pilot.89wPKx/`.

A 20 s/49 Hz attached profile of the surviving native 29748 sparse-exact run has
967 samples and no lost samples. Almost the whole sampled window is in exact
materialization; native factorized addition is the dominant inclusive path.
This is a local phase profile, not a whole-campaign time fraction. The existing
Symbolica source-weight reconstruction backend's separate same-input/order
control reaches its one-hour deadline without a candidate: 3,600 s wall,
3,570.95 CPU-s and 338,172 KiB peak RSS, status 124. It completes 177 of 178
observed exact reconstruction frames; the unfinished case 325 last reports
column 3018 at 3,553.635 s. This is a censored diagnostic, not a successful
solver timing, a proved reconstruction failure or a matched speed ratio.
Evidence: `TMP/native29748-source-weights.DHS24K/`.
No replacement CAS arithmetic is introduced. The original sparse-exact
continuation subsequently saves native 29748, leaving its selected downset at
**197/198 saved sectors**, with native 29751 still missing. The instrumented
whole continuation takes 1:23:16 wall and 5,117.65 CPU seconds; this is not an
isolated-sector benchmark. The new shard is 231,082,534 bytes with 355 rules
and 44,164 declared terminals. An explicit ingress allowance cold-loads it in
9.017 s. Three complete finite traces visit 43,892 /219,476 /45,011 keys and
leave 39,614 /180,598 /40,592 missing keys. Every observed missing key is in a
strictly lower support and within R10. These are partial reductions, not
proof of universal same-sector coverage; no coefficient back-substitution
was attempted. This retained-policy shard is excluded from the broad-search
inventory above.

Local receipts: `TMP/rank10-durable-commit.b44EpN/`,
`TMP/class29550-owner-cold.TdVOH0/`,
`TMP/root29751-ordering-portfolio.JEqDKU/`, and
`TMP/root30231-grevlex-exact-profile.pmo9qr/`,
`TMP/late-lex-native29751-retain.DAkEC4/`,
`TMP/late-lex-native24996-regression.SMAvLT/`, and
`TMP/native29748-cold-explicit.Qr6lIO/`. Current registered RSS at the
inventory snapshot is 70.85 GB; the historical sampled peak is 337.95 GB.
The aggregate 450 GB soft stop remains below the user's 500 GB ceiling.

### Exceptional-case scheduling and admission follow-up

Three generic refinements address actual failures without changing the rank,
source, finite-retention policy, or positive-power domain:

1. **Integer endpoint slack.** If a row has a finite sector bound, write its
   distance from that bound as a sum of nonnegative integer contributions.
   A coefficient larger than the available slack forces that coordinate to
   its endpoint. For example, the narrowed native 2678 branch requires
   `12u + 5v = 9`, with integer `u,v >= 0`. Hence `u=0`, then `5v=9` is
   impossible. This avoids a 20,736-signed-divisor proposal; it does not prove
   the different, wider original parent empty. Existing Symbolica integer
   arithmetic and RREF perform the algebra. Saved declared charts are unchanged.
2. **Finite rank splitting before expensive elimination.** Only when every
   positive original coordinate is fixed, an explicit rank bound makes the
   remaining numerator simplex finite. If its conservative whole refinement
   tree fits the remaining work allowance, reuse the existing exact splitter
   earlier. The narrowed native 1484 case has three free inactive axes and
   remaining rank 8: 165 simplex points and 220 tree nodes, before its affine
   constraints. Its parent admits 12 integer points; the new guard admits one.
   No positive ray is enumerated, and no leaf is automatically a terminal.
3. **Full-conjunction inconsistency before false compact overflow.** When a
   mixed affine/nonlinear branch proposes an unrepresentable coordinate,
   normalize its entire restricted conjunction with native Symbolica. Discard
   it only if the exact basis contains a nonzero constant. Otherwise preserve
   the original typed overflow. The native 29751 example above exercises this
   path; the valid example `x=156, y^2=1` still fails rather than being clipped.

All work uses existing shared budgets; the native F4 call is not a hard
wall-time/RAM-bounded operation. Independent implementation and mathematical
audits, nineteen new focused tests, and the complete release core gate pass:
**2,457 tests passed, zero failed, 32 existing ignored**. All **246 release
application/integration tests** pass too. The fresh public seven-sector CLI
regression reproduces the previous 41,971,427-byte output exactly in
37.25 s wall /41.13 CPU-s, with 356,384 KiB peak RSS; compilation is separate.
This is an integration regression, not a matched speed comparison. Actual
full-sector retries of the captured failures are separate acceptance steps.
Passing captured geometry cases is not a completed family.
Evidence: `TMP/geometry-three-slice-release.hVECOq/`,
`TMP/geometry-three-slice-app-cli.w9PTom/`,
`TMP/geometry-public-r10-pilot.dCYG5k/` and the independent positive-slack,
finite-rank-priority and compact-overflow design/audit receipts.

The first actual retry, native 2678 with the unchanged SearchFinite/R10/natural/
depth-zero/sparse policy and 8,192-item geometry allowance, subsequently
**finishes and saves its program**: 1,007.895 s solver time, 1,008 rules,
54 finite terminals and 211,758 RHS terms. It passes the former case 345
failure and completes 997 cases. Exceptional geometry takes 1.274 s versus
991.172 s of symbolic search; resolving the geometry exposes the substantial
remaining algebra workload rather than making the whole solve instantaneous.
The 149,605,286-byte file cold-loads in 8.987 s. Three declared-terminal
identity/repeat checks pass, but three nontrivial traces remain partial with
2,332 /6,176 /10,682 missing keys. The diagnostic's successful exit does not
make those complete reductions or prove recursive rank-10 closure.

Solve plus export takes 1,016.25 s wall /1,008.50 CPU-s, with 1,096,584 KiB
peak RSS; the separate cold diagnostic takes 9.90 s wall /9.83 CPU-s,
799,400 KiB peak RSS. No output allowance was raised for this retry.
Evidence: `TMP/geometry-native2678-search.N6un9u/`.

The native 1484 SearchFinite retry passes its former exceptional-geometry
failure, then reaches its unchanged 1,800-second deadline during exact lifting:
1,800.11 s wall /1,787.17 CPU-s /1,104,520 KiB peak RSS, status 124, no output.
Case 525 finishes after roughly 1,142 s; case 526 reaches row 999 of a
1,021-row frame before the deadline. This is algebraic expression cost, not a
new unsupported exceptional domain or an out-of-memory failure. Both captured
faces have fixed positive indices and the relation `v+w=2u+1` on three
nonnegative numerator coordinates. Their rank-10 domains have only 20 and 12
integer points respectively, so the already-implemented finite-retention
policy is a possible separate experiment, not the policy of this censored run.

More importantly, offline verified routing puts native 1484 in already
represented class 28686. Three saved SearchFinite owners are available, the
smallest 101,599,848 bytes. The next action is exact reuse, not another solve.
The same distinction applies to the old broad executable subsequently failing
on native 2678: the successful new standalone output is already preserved,
and that graph class also has saved alternative owners.
Evidence: `TMP/geometry-native1484-search.ztN5er/` and its saved-owner metadata.

The natural-order native 29751 retained-policy control then reaches its exact
one-hour deadline: 3,600 s wall /3,573.13 CPU-s /1,833,392 KiB peak RSS, status
124, no candidate and no cold phase. Both former geometry failures were passed.
Its last event is case 391, row 4,388 of 4,427, with 470,697 reducer nonzeros.
This is deadline-censored exact elimination, not a new unsupported guard or
memory failure. The captured affine chart has only three positive free indices
and equations `n0=3*n2-6`, `n1=4-n2`. Positivity forces `n2>=3` and `n2<=3`,
hence exactly `(n0,n1,n2)=(3,1,3)`. Recognizing this finite positive chart before
symbolic elimination is the next generic refinement to investigate; it is not
implemented by the owner-reuse milestone. Evidence:
`TMP/geometry-native29751-retain.YGc5XI/`.

A separately timed half-rotation run uses the same rank, finite-case policy,
geometry allowance and frozen geometry implementation. Its deliberate
output-only allowance is larger than the prior default: 1 GiB whole file and
aggregate coefficients, 32 million collection entries, 16 MiB per coefficient.
This protects a successful solve from the earlier small export budget; it does
not justify a matched output-pipeline speed comparison. The one-hour run starts
at 15:55:05 UTC with one CPU and a 32 GiB address-space cap and is still running
at this checkpoint. No checkpoint shard is overwritten.
Evidence: `TMP/native29751-current-rotate-half.c4VGp3/`.

## Public interfaces

The existing Rust application request adds one optional field:

```rust,ignore
let mut request = FamilyCandidatesRequest::new(family_input);
request.max_numerator_rank = Some(10);
let generated = family_candidates(request)?;
```

With the CLI, add `--max-numerator-rank 10` to `family-candidates`, alongside
the existing family input and root-sector options. For example, this selected
six-line five-loop pressure case is **not** the entire five-loop census:

```sh
rustred family-candidates \
  --input examples/input/tide_five_loop.toml --input-format toml \
  --nonpositive-indices 3,4,5,6,7,8,9,10,14 \
  --max-numerator-rank 10 --n-cores 1 --exact-backend sparse \
  --numerical-depth 0 \
  --checkpoint-dir TMP/tide-rank10 --progress \
  --output TMP/tide-rank10.rrbin
```

Python's existing `rustred.family_candidates(...)` accepts the same keyword
`max_numerator_rank=10`. Omit it for the unchanged unrestricted path. Rank zero
is valid and still permits arbitrary raised denominator powers. `numerical_depth`
is a separate search radius around fixed cases, not the numerator rank.
The depth-zero example still checks the initial fixed-case IBP sources. It
keeps any remaining fully fixed cases as explicit residuals instead of
spending deeper search effort on shrinking their number; symbolic case
search remains unbounded. This is appropriate for the initial coverage-first
milestone, and does not establish independence of those residuals.

### Deliberately retaining finite leaves

The follow-up implementation adds `finite_case_policy="retain-rank-finite"`
(CLI: `--finite-case-policy retain-rank-finite`, Rust:
`FiniteCasePolicy::RetainRankFinite`). It requires an explicit rank and differs
from `numerical_depth=0`: whenever every active original index is fixed, it
enumerates the remaining inactive-coordinate total-degree simplex, intersects
each point with the **whole** affine case, and keeps all admitted points as
explicit nonminimal terminals. Fully numerical leaves likewise bypass search.
Any unfixed positive direction continues through ordinary parametric solving.
The follow-up release gate passes 2,373 core tests (32 existing ignored),
152 application tests, 15 CLI tests and 43 Python API tests. Eight additional
K1/K3 R=10/R=20 serial/parallel retention smokes pass. A malformed delta-sector
test fixture required its missing delta flag; assertions were unchanged and
the failed baseline receipt is preserved separately.

The default remains `finite_case_policy="search"`. Per-sector limits
`finite_max_visited_points` and `finite_max_retained_terminals` default to
1,000,000 each (CLI names use hyphens; Rust stores them in `finite_case_limits`).
They limit work/storage, not the mathematical domain. Exhaustion returns an
incomplete error, not a successful prefix. The generation policy and checkpoint
identity bind retention and its limits; old search checkpoints cannot silently
be resumed as retention campaigns. This option may save expensive numerator
recurrences but lose useful above-R coverage from those recurrences. Internal
successors remain unclipped and must still be resolved: finite local retention
is not a successor-closure proof.

### Exceptional-geometry work allowances

The Rust request's `case_intersection_limits: CaseIntersectionLimits` controls
the existing exact exceptional-case service. CLI switches and corresponding
Python keywords are:

| CLI switch | Python keyword | Default |
|---|---|---:|
| `--case-max-work-items` | `case_max_work_items` | 4,096 |
| `--case-max-terms-per-conjunction` | `case_max_terms_per_conjunction` | 100,000 |
| `--case-max-normalizations` | `case_max_normalizations` | 1,024 |
| `--case-max-factorizations` | `case_max_factorizations` | 4,096 |

Each exceptional AND conjunction gets a fresh budget shared by its refinement
branches. Work items include refinement and divisor visits, not just distinct
integer points. These are neither whole-sector counters nor hard wall-time or
memory limits; native operations are not interrupted halfway through. Exhaustion
returns an incomplete result with the effective limits and work statistics.
An unsuccessful auxiliary coverage check never suppresses a pending case.

These execution allowances are not part of mathematical checkpoint identity.
A retry with larger allowances can reuse complete saved shards. The reported
limits describe the current invocation, not historical settings for those
shards. Changed allowances can affect redundant scheduling and the particular
rules discovered, so arbitrary-limit runs are not promised byte-identical.
Rank, ordering and finite-retention policy remain separately bound as before.
CLI and Python require positive integers; Rust also permits zero for an explicit
no-work diagnostic. The independently reviewed release gates now pass for
core, application and Python; no new algebra or binary payload schema is
introduced.

The first selected six-line five-loop pilot through R=10 saved six of its
seven nonzero sectors, including the parent, in **50.93 s wall / 59.35 s CPU**,
peaking at **261,212 KiB RSS**. It saved **1,044 rules and 1,024,045 finite
terminals**. The seventh sector, ordinal4 / mask`111000000001010`, failed
at 1,000,001 cumulative visited points against the declared 1,000,000 limit.
The process correctly exited8 without assembling a final candidate bundle.
Its six saved shards are preserved, not relabelled for another policy budget.
This was four workers, sparse-factorized arithmetic and nested Rayon1; the
earlier one-hour pilot used six workers and reconstruction. The difference
is not a controlled speedup measurement. Evidence:
`TMP/tide-r10-finite-four-workers.f7ADgX/` and
`TMP/finite-retention-gate.qs4pQb/RESULTS.md`.

A fresh retry, with the public Rust request's visited-point limit raised to
10,000,000 and larger explicit native transport budgets, **writes all seven
candidate sectors** in **38.84 s wall / 46.55 s CPU**, at **373,256 KiB peak
RSS**. It produces1,299 rules,1,208,801 nonminimal terminals and a41,971,427-byte
bundle. Its reported preparation/generation/encoding times are9.337/27.183/1.792s;
no checkpoint was reused. The small external driver and native libraries are
optimized; compilation is outside this process measurement. This demonstrates
completion of local candidate generation for this selected family, not all
five-loop topologies or recursive rank10 closure. Nine native cold-load and
application canaries pass, including memoized repeats (9.31 s wall, 9.23 s CPU,
300,672 KiB peak RSS); most are already retained leaves. Seven stronger inputs
with raised positive powers and rank-10 numerators trace 47,397 keys, 43,863
rule applications and 3,534 terminals, with zero uncovered keys. The maximum
observed numerator degree is 10 and dot excess is 7. Exact back-substitution
passes only two of these seven inputs: five hit the default 16-million cached
coefficient-term budget (93.66 s wall, 92.80 s CPU, 1,628,572 KiB peak RSS).
These are application-resource failures, not five missing-rule cases or seven
successful reductions. A separate explicit larger-cache repeat uses the same
bundle, without regeneration: six of seven pass with repeat equality in
274.62 s wall / 271.51 s CPU, at 3,182,164 KiB peak RSS. The remaining input
hits the separate unchanged 16-million coalescing-addition work limit. That
repeat is therefore not a seven-input success. Finally, a fresh-process run
raises the explicit work allowance to 128 million additions as well and passes
**all seven exact reductions and their memoized repeats**, using the same
saved bundle: **371.19 s wall / 367.56 s CPU**, 5,587,220 KiB peak RSS. Its
cached output uses 63,207,570 coefficient terms and 3,433,282,950 coefficient
payload bytes. The hard numerator target dominates this run; later targets
reuse its cache, so their short per-request times are not independent cold
measurements. Compilation is separate. None of these finite target lists proves coverage
for arbitrary positive powers. Evidence:
`TMP/tide-r10-finite-larger-budget.9QDI7r/`.

The updated release CLI independently regenerates this same selected family
with the public budget switches in **49.13 s wall / 56.89 s CPU**, peaking at
397,992 KiB RSS. It saves all seven sectors and produces a byte-identical bundle
to the earlier public Rust driver: the same 1,299 rules and 1,208,801 terminals.
No checkpoint is reused. These single runs use different interfaces and a
shared host with other active campaigns, so their time ratio is not a controlled
performance comparison. This is a five-loop end-to-end regression of the
implemented slice, not new all-family closure. Evidence:
`TMP/tide-r10-updated-cli.MW0dOB/RESULTS.md`.

The same frozen seven-target application driver also completed with the
existing factorized-denominator cache, without regenerating any rules:

| Application-cache representation | Wall seconds | CPU seconds | Peak RSS GiB |
|---|---:|---:|---:|
| Sparse, existing default | 371.19 | 367.56 | 5.328 |
| Symbolica factorized | 738.55 | 729.41 | 7.519 |

Both runs pass all seven reductions and their immediate exact same-mode
memoized repeats, with identical trace frontiers and output-term counts.
The diagnostic does not serialize the coefficient maps, so these counts are
not a new independent coefficient-by-coefficient comparison between modes.
The hard second target takes 351.90 versus 705.95 seconds. This is one matched
observation per mode on a shared host, not a general benchmark or a comparison
of generation backends. Retain the sparse default for this workload. Evidence:
`TMP/rank10-factorized-canary.vojes3/AUDIT.md`.

### Four-parent coverage campaign

The expanded user budget permits up to100 compute cores and500GB aggregate
RAM. Four independent release campaigns use20 sector workers each, with
disjoint affinities, native inner pools limited to1, persistent checkpoints,
80GiB per-process address-space limits and an initial two-hour reassessment
deadline. A separate16-core release gate and4-core diagnostic allocation fit
inside the100-core ceiling. The monitor counts their actual process trees and
soft-stops owned solvers at450GB aggregate sampled RSS, leaving headroom.

The independent 07:48 UTC snapshot contains **779 distinct saved labelled
sectors out of8,246**, after removing30 duplicate saved occurrences between
the overlapping parent downsets. All four12-line parents have saved shards.
The779 masks route to52 of67 published classes. These are generation/storage
counts, not completed recursive reductions or certified family coverage.
Many pending masks have not started, so they must not be labelled hard cases.
See `TMP/rank10-inventory-audit.9t07iu/AUDIT.md` and the ongoing batch in
`TMP/tide-r10-four-parent-batch.yGOAw6/`.

A later independent, non-atomic snapshot at **08:38:44–45 UTC**, about one hour
after launch, finds **2,941 distinct saved sectors**, 3,281 saved
occurrences and representations of **63/67** published classes:

| Parent input | Durable sectors / scheduled | Logical checkpoint GB |
|---|---:|---:|
| 30527 | 287 / 2,686 | 3.246 |
| 30699 | 1,069 / 2,580 | 7.216 |
| 31740 | 727 / 2,656 | 7.948 |
| 32745 | 1,198 / 2,478 | 4.279 |

The four unrepresented classes in this particular batch are 28686, 29550,
30231 and 30563. The separately generated finite-retention example covers a
selected 28686 input, but uses another generation policy and is not silently
merged into these checkpoints. At that snapshot all four broad campaigns were
still running; these were not final counts. Aggregate sampled memory was about
214 GB. Parent 30527 has about 12.5 GB virtual-address headroom beneath its
80 GiB process cap; the 450 GB aggregate soft stop is a separate limit.
Evidence: `TMP/rank10-one-hour-inventory.i4Eqtc/REPORT.md`, with the earlier
47-minute snapshot separately preserved.

#### Resource outcomes and preserved progress after the hourly snapshot

All four original broad jobs subsequently exhausted their **individual 80 GiB
virtual-address caps**, not the 500 GB aggregate user allowance:

| Parent | Terminal wall time | CPU seconds | Peak RSS KiB | Outcome |
|---|---:|---:|---:|---|
| 30527 | 1:28:38 | 100,501.95 | 82,902,040 | Allocation abort, signal 6 / launcher 134 |
| 30699 | 1:02:34 | 74,129.96 | 81,627,544 | Allocation abort, signal 6 / launcher 134 |
| 31740 | 1:34:56 | 112,056.64 | 82,326,068 | Allocation abort, signal 6 / launcher 134 |
| 32745 | 1:21:07 | 96,236.79 | 82,934,636 | Allocation abort, signal 6 / launcher 134 |

GNU time's trailing `Exit status: 0` must not override its explicit signal-6
report and the launcher's status 134. Their completed native shards survive.
Explicit resumptions reuse 1,096 and 1,552 saved sectors, respectively, with
16 workers and new startup caps of 112 and 96 GiB. The rank, source input,
ordering, backend and `search` finite-case policy remain unchanged; the tested
new CLI adds the already audited bilinear refinement and public transport
budgets. These are separately timed bounded continuations, not uninterrupted
successful original runs. Evidence: `TMP/tide-r10-30699-resume.lTUYyJ/` and
`TMP/tide-r10-32745-resume.haTZFg/`.
Parents 30527 and 31740 subsequently resume with the validated resultant CLI,
12 workers each and unchanged 80 GiB caps, admitting 305 and 961 prior shards.
Their failed original timings remain separate. The aggregate
monitor's soft-stop selection covers registered frozen-CLI continuations as
well as original solvers, with process-tree ownership and PID/start-time
revalidation before signalling. This is workspace-local runtime supervision,
not a change to the mathematical search or a 450 GB per-process allowance.
At 09:17 UTC six continuations allocate 64 disjoint compute workers in total:
four parent searches and the 29550/30231 finite-retention retries. The latter
use four workers each and 32/64 GiB startup caps, preserving their 364/172
previous shards and all original generation-policy settings. The new geometry
only applies to newly solved sectors; mixed-generation provenance is explicit.
These continuation budgets are bounded reassessments, not promised completion
times. No terminal-minimization or five-loop numerical-integration lane starts.

A new independent **09:05:30–09:06:55 UTC** non-atomic inventory observes
4,076 durable shard occurrences, **3,601 distinct labelled sectors / 8,246**,
and representations of **64/67** published classes. It excludes all
different-policy finite-retention pilots. The remaining 4,645 labelled masks
are not all demonstrated hard cases: many are unscheduled or still running.
Original and resumed shard histories are not independent completed artifacts.
The independent inventory, resource review and documentation audit are saved
in `TMP/rank10-broad-inventory.2HII5h/AUDIT.md`. In checkpoint mode the reviewed
implementation does not accumulate completed `SectorSolution` values.
Some live exact frames reach more than 1.5 million upper-matrix nonzeros;
coalesced logs do not identify the allocation-aborting worker or separate
live coefficient growth from allocator high-water retention. Reduced worker
concurrency is therefore a measured-risk continuation policy, not a proved
memory-leak diagnosis.

Separate `retain-rank-finite` probes targeted the other three representatives
absent from the hourly broad inventory. Each used five workers, R=10, a
900-second deadline, 16 GiB address-space cap and fresh checkpoints:

| Root input | Saved / scheduled sectors | Saved rules | Declared terminals | Outcome |
|---|---:|---:|---:|---|
| 29550 | 364 / 462 | 68,132 | 41,835,853 | Deadline, status 124 |
| 30231 | 172 / 198 | 34,694 | 21,930,362 | Allocation abort, status 134 |
| 30563 | 328 / 328 | 70,482 | 30,431,209 | Generation finished; aggregate entry export cap |

The 29550 and 30563 root shards are present; the 30231 root is not. None of
these three probes wrote a complete assembled bundle. For 30563, an explicit
assembly-only retry with a 64-million entry limit reused all 328 shards,
performed no new sector search and preserved their hashes. It then reached
the separate binary-program section limit: 1,074,983,118 required bytes versus
1,073,741,824 allowed bytes. This is not a mathematical missing-rule failure,
nor a successful final bundle size. Native per-sector candidate shards remain
readable; cross-sector reduction still requires a coherent shared reducer,
not unrelated per-shard applications with reset budgets or rank checks.

These large finite terminal lists are accepted nonminimal candidate output,
not minimal masters. They expose storage/application costs which cannot be
fixed merely by granting more solver RAM. No terminal minimization is started
before the requested family-coverage gate. Evidence:
`TMP/tide-r10-three-missing-roots.fGDaNV/snapshot-terminal.json` and
`TMP/tide-r10-root30563-assembly64m.zTcZUV/`.
Continuation receipts are in `TMP/tide-r10-30527-resultant-resume.tm5KX0/`,
`TMP/tide-r10-31740-resultant-resume.R4Hpsa/`,
`TMP/tide-r10-29550-resultant-resume.M5CZF6/` and
`TMP/tide-r10-root30231-resultant-resume.LIPE76/`.

A later metadata-only broad-search inventory, sampled non-atomically during
**10:32:25 UTC**, has the following counts:

| Parent input | Saved sectors / scheduled |
|---|---:|
| 30527 | 572 / 2,686 |
| 30699 | 1,886 / 2,580 |
| 31740 | 1,533 / 2,656 |
| 32745 | 2,254 / 2,478 |

Their 6,245 occurrences comprise **5,234 distinct labelled sectors / 8,246**,
leaving 3,012 unsaved and 1,011 repeated occurrences. This adds 345 distinct
saved sectors to the 09:55 inventory. Exact input routing maps
these saved sectors to **65/67** census representatives; 29550 and 30231 remain
absent from this search-policy inventory. Different-policy finite-retention
probes are not merged into it. All four original parent shards are now present,
but that does not establish coverage of their descendant sectors or entry rays.
The six live campaigns use four-hour deadlines and 64 solver workers; all are
checkpoint continuations, not fresh regeneration. The global 450 GB soft stop
supervises actual RSS, since individual address-space caps are not simultaneous
RAM reservations. At this snapshot actual registered RSS is 324.31 GB, with no
aggregate resource stop. One current-run failure is explicitly bound to native
sector 7749 (published 20796), an out-of-rank branch encountering compact-index
conversion before rank admission; the remaining campaigns report no new
mathematical failure. Its fix is tested separately, without changing a live
executable. Evidence: `TMP/rank10-search-refresh-1030.6PaFV5/REPORT.md`;
the earlier snapshot remains in `TMP/rank10-search-refresh.007AMj/REPORT.md`.

Subsequent individual allocation aborts preserve 2,262 sectors for32745,
1,886 for30699 and 574 for30527. They are explicit GNU MP allocation failures
(signal6 /wrapper134), not four-hour timeouts or successful generation.
32745 resumes with 16 workers and 160 GiB operating AS;30699/30527 resume with 8/6
workers and 144 GiB each. The latter use the freshly gated rank-admission fix;
completed shards are not regenerated. This reduces the six concurrent solver
pools to 50 workers; all remain under aggregate 450 GB actual-RSS supervision.
The larger hard AS ceilings are not additional RAM reservations. A new30699
sector fails the independent 4096-item exact-geometry work budget, not RAM;
that failed branch remains explicit and cannot be counted as a terminal.

An isolated one-worker source-weight reconstruction probe of the still-missing
native sector29734 (published12823) reaches its15-minute deadline rather than a
CAS failure: **900.03 s wall /893.33 CPU-s /299,768 KiB peak RSS**. It starts247
cases and validates107 native exact source products, but returns no completed
sector or artifact. Its last large frame has2,426 rows,6,773 columns and four
coefficient variables. This is not a completed comparison against the parallel
sparse-exact campaign. Evidence: `TMP/rank10-single-sector-control.6jITZ6/`.

Independent inspection exposes avoidable dimensionality in that frame. Its
case equality is `1+n0-n1+n3+n4=0`, with `n0,n3,n4<=0` and `n1>=1`; all other
indices are fixed. The four nonnegative slacks `-n0,n1-1,-n3,-n4` sum to zero,
so every one is zero. The case is a single integer point, independent of R,
not a positive-dimensional index domain. Four original indices are unfixed
before using the signs, with one affine equality between them; the frame's
four coefficient variables can also include the dimension d and must not be
read as four independent index directions. That probe's affine bound check
excluded impossible rows but did not propagate equality at a finite sector
bound. A narrowly scoped native-integer endpoint-propagation fix was
independently release-tested. It fixes each participating original coordinate
to its sector endpoint, retains the full conjunction, and repeats Symbolica
RREF until no new coordinate is fixed. There are at most as many successful
rounds as integral coordinates. This is an exact row-extremum consequence,
not a general inequality or integer-feasibility solver. The failed probe remains a timeout;
no anticipated speedup or completed sector is reported from this diagnosis.

The first joint release gate exposes an important representation boundary:
2,410 tests pass and two existing declared-chart tests fail when endpoint
inference is applied during chart construction. Independent review also finds
the same risk for saved candidate rules: simplifying a stored affine chart
must not silently change its symbolic target or right-hand side. The correction
preserves exact declared-chart construction and keeps endpoint inference in
search/intersection admission. Existing codec checks and mathematical test
assertions stay intact. The corrected release core gate passes **2,413 tests,
zero failed, 32 existing ignored**, including all 13 affine box-bound tests and
the new declared-chart/search distinction. The full suite takes 134.24 s wall
and 133.19 CPU-s, at 161,548 KiB peak RSS; compilation is separate. The 1,273
source checks and frozen binary check pass. Both the initial failure receipt and corrected
gate are preserved at `TMP/affine-saturation-release.ErPjLt/` and
`TMP/affine-saturation-corrected.iy3ZrG/`.

The frontend release gates pass **242 application tests** (165 unit and 77
integration), eight Python Rust-side tests and **47 public Python API tests**.
The application regression preserves a saved affine rule's target, right-hand
side, source layout and concrete application while separately demonstrating
the stronger search inference. Initial Python packaging fails on an unavailable
Nix dependency; using the already-installed Maturin binary passes on unchanged
source, without escalation or dependency changes. Evidence:
`TMP/case-intersection-frontends-fixed.4teMyS/` and
`TMP/case-intersection-python-packaging.tzmJtQ/`.

A fresh production CLI regression solves the selected seven-sector R10 input
in **35.92 s wall / 43.56 CPU-s**, at **353,820 KiB peak RSS**. Its 1,299 rules,
1,208,801 terminals and 41,971,427-byte bundle are byte-identical to the previous
pilot. There is no checkpoint reuse or compilation in that timing. Concurrent
host activity and changed affinity prohibit treating it as a controlled speed
ratio. Evidence: `TMP/affine-saturation-pilot.pit4e5/`.

The previous 31740 campaign later reaches its individual 80 GiB allocation
limit after saving 1,533 sectors. Its fresh corrected-CLI continuation starts
at 11:41:17 UTC with eight workers and 144 GiB operating AS, reuses all 1,533
shards, and begins saving additional sectors with no initial import failures.
This is production checkpoint-resume evidence, not independent source replay
of every saved rule. Six campaigns now use 46 solver workers; separate bounded
controls and builds remain under aggregate monitoring. Evidence:
`TMP/tide-r10-31740-eight-worker.izpFjC/`. The three isolated failed/stalled-sector
controls have separate outcomes, reported below.

The corrected one-worker native29734 control **completes in 157.128 s solver
time**, or **163.32 s process wall / 162.11 CPU-s**, at **185,232 KiB peak RSS**.
It returns 318 rules, 44,148 finite residuals and 38,965 RHS terms with strict
descent checked. All 197 Symbolica source-weight reconstructions finish and
their native exact source products validate. The input, natural ordering,
R10 finite-retention policy, reconstruction options and 900 s deadline match
the earlier censored probe; the corrected core now finishes within that limit.
This is an actual sector result, not the endpoint microtest. The earlier
timeout cannot supply a completed speed ratio, and case ordinals change when
geometry is refined. The separate native7749 sparse-factorized, search-policy
control also completes: **227.235 s solver / 233.59 s process wall / 231.75
CPU-s**, **372,340 KiB peak RSS**, 629 rules and 88 finite residuals. It no
longer fails on compact conversion of an out-of-scope rank119 branch.

Neither of these initial diagnostic clients saves its returned sector program;
they check strict descent but do not perform independent artifact source replay
or prove recursive family closure. Source preparation and helper compilation
are outside the solver timings. Their receipts are in
`TMP/rank10-affine-controls-licensed.sEIi07/`. Earlier malformed-license launch
failures are preserved separately and are not counted as solver failures.

The native24996 search-policy retry, with only its work-item allowance raised
to 8,192, reaches a different exceptional conjunction at case439. A 20 s
attached profile records 961 samples with no losses, dominated by GMP
large-integer arithmetic; recovered call stacks identify Symbolica's exact
rational Lex-order F4 normalization in the exceptional-case engine. Unwinding
is incomplete, so the recovered caller percentages are not whole-phase time
fractions. It is intentionally stopped after **1,162.98 s wall / 1,153.35 CPU-s**,
at **703,108 KiB peak RSS**, to prioritize the finite-retention experiment;
status143 is an intervention, not its3600s timeout or a CAS failure. No sector
solution or program is returned. Its current affine
case is actually finite at R10: all positive indices are fixed to one, and
the only free indices satisfy `2+3*n0-n1-4*n6=0` with all three nonpositive.
There are eight admitted integer points. The existing explicit finite-retention
policy can bypass symbolic search on this domain; a separately labelled
retention-policy retry is the next experiment. Do not silently substitute that
policy into the broad search-policy checkpoints. Evidence:
`TMP/native24996-geometry-profile.6NKfyH/`.

A separate public single-case solve reproduces the same230-source-row,
726-column exact frame in **3.190 s**, with **0.021 s** guard extraction. Its
captured exceptional union has three simple coordinate branches and one AND
of nine bivariate polynomials in `n1,n6`, of maximum total degree14. The latter
is the native F4 pressure case. The complete capture takes9.40s including
6.155s family/source/zero-census preparation, at94,448KiB RSS. It neither runs
exceptional geometry nor publishes a rule/artifact; the full sector remains
incomplete in the search-policy lane. The next isolated experiment compares
native Symbolica ideal ordering on this actual conjunction, not a synthetic
substitute. Evidence: `TMP/native-case-guard-run.XdGFYz/`.

That native ordering experiment **completes**: Symbolica Q-F4 in graded reverse
lexicographic order reduces the full nine-polynomial,608-term conjunction to
four basis polynomials with26terms in **8.323 s**. All nine original generators
then reduce exactly to zero in1.06ms. The whole isolated process takes8.33s
wall/8.27CPU-s with12,288KiB peak RSS. The earlier full search-policy Lex run is
censored and includes other work, so this is not a completed end-to-end speed
ratio. It identifies a promising native ordering correction, not integer-case
coverage. The subsequent production change computes the native GrevLex basis, then reorders its
polynomials for existing Lex storage without claiming that the result is a
Lex Groebner basis. No custom CAS or FGLM conversion is needed for the
normalizer's equal-ideal contract. Evidence:
`TMP/native-guard-grevlex.G2pmjY/grevlex/`.

That generic change passes independent implementation/mathematical review and
the complete optimized core gate: **2,417 passed, zero failed, 32 existing
ignored**, in132.31s wall/131.29CPU-s, at162,832KiB peak RSS. Four new tests
check exact ideal equivalence in both directions, primitive integer output,
inconsistent/empty inputs and preservation of affine consequences and siblings.
One explicitly verifies that reordering the resulting generators into Lex
storage does **not** make them a Lex Groebner basis. Existing assertions remain
unchanged. Focused intersection/affine/solver selections overlap the full gate.
The application gate also passes all 246 tests; the fresh public seven-sector
regression returns the byte-identical 41,971,427-byte bundle in 35.11 s wall /
40.62 CPU-s, with 381,504 KiB peak RSS. Receipts:
`TMP/grevlex-core-checked.l4kKiV/`, `TMP/grevlex-app-cli-gate.PjWrRV/` and
`TMP/grevlex-public-pilot.HHonuH/`.

The actual **same-policy SearchFinite** sector rerun subsequently completes:
**57.435 s solver time**, 530 rules, 27 finite residuals and 28,996 RHS terms.
It retains the original natural ordering, depth zero, sparse-factorized backend,
R10 and 8,192-item geometry allowance. In the former bottleneck, case439 enters
exceptional geometry at30.127s and publishes its candidate at38.468s; total
geometry work over the sector takes8.674s. Whole solve/export process:
64.73s wall /64.28CPU-s,193,100KiB peak RSS. The native bundle is22,162,261bytes
and cold-loads in8.143s. All three terminal-identity/repeat checks pass; the
nontrivial standalone traces still lack their lower-sector programs and are
not complete reductions. No finite-retention override is used. The original
Lex attempt is censored, so do not report a completed speed ratio. This is
an actual end-to-end sector improvement, not a full-family closure claim.
Evidence: `TMP/grevlex-native24996-search.EfN5LQ/`.

At **11:59:45 UTC**, a fresh non-atomic metadata inventory records **5,546 /
8,246 distinct saved labelled sectors**, 142 more than at11:12. It still
represents65/67 physical classes; 2,700 labelled jobs remain unsaved. The
four parent counts are691/2686,1970/2580,1638/2656 and2388/2478. The separate
finite-retention campaigns are excluded from this union. Parent32745's
16-worker continuation has meanwhile hit its160GiB individual allocation
cap after saving2,381 sectors; a new8-worker/144GiB continuation reuses them
and begins completing more. The six campaigns now allocate38 solver workers
plus bounded diagnostics/builds. Registered actual RSS is140.10GB at the
snapshot, with337.95GB historical sampled peak and450GB aggregate soft stop.
Evidence: `TMP/rank10-search-refresh-noon.w9RDVn/`. Neither represented classes
nor completed labelled sectors establish recursive family closure.

The later **12:42:49 UTC** non-atomic inventory reaches **5,588 / 8,246**
distinct saved labelled sectors: 723/2,686, 1,970/2,580, 1,648/2,656 and
2,396/2,478 in the four parents. There are 6,737 saved occurrences, including
1,149 duplicates; the 2,658 unsaved labels are not necessarily distinct
mathematical problems. Broad-search representation remains 65/67 classes,
excluding separate finite-retention programs. Actual registered RSS is
243.78 GB at this snapshot, below the 450 GB soft stop and 500 GB ceiling.
Evidence: `TMP/rank10-search-refresh-midday.NoKSis/`.

The independent captured geometry test already resolves the separate
five-free-index guard from native sector 24996 with explicitly larger resources:
**94 exact cases in 15.40 ms**, consuming 5,235 work items. Exhaustive native
evaluation of all 3,003 R10 simplex points agrees with the returned union,
including all 274 zeros. All other measured counters fit their original
defaults. The default 4,096-work-item attempt still correctly fails. This is
an exact exceptional-domain result, not a completed sector or solver timing.
Evidence: `TMP/affine-saturation-release.ErPjLt/`.

The next metadata-only snapshot, **11:12:48.843–11:12:49.220 UTC**, saves
**5,404 / 8,246 distinct labelled sectors**, leaving 2,842 unsaved. There are
6,526 saved parent occurrences, including 1,122 duplicate occurrences:
30527 has 655/2,686, 30699 has 1,957/2,580, 31740 has 1,533/2,656 and 32745
has 2,381/2,478. This adds 170 distinct saved sectors since 10:32. All four
parent roots are saved. Still only **65/67 graph classes** are represented,
and only 29 literal published representative masks have saved shards; the
other 36 represented classes use different labels. Exact momentum-map-based
reuse of those owners is a reviewed design, not yet an implemented reduction
path. Different-policy finite-retention probes remain excluded.

Actual registered process-tree RSS is 216.90 GB at this snapshot, with a
sampled historical peak of 337.95 GB and no aggregate stop. Six solver pools
still use 50 workers; bounded compilation uses separate cores. The two known
failures remain explicit: the old executable's rank-before-compact failure
and the newer executable's 4,096-item geometry-work limit. Saved sectors,
represented classes and finite dependency traces do not prove arbitrary-dot
rank-10 closure. Evidence: `TMP/rank10-search-refresh-1112.ygPIqd/REPORT.md`.

Two earlier failures appeared in the original frozen broad-run executable. A
nonlinear exceptional equality `8-3*n6-6*n2+2*n2*n6=0` is unsupported there.
Its equivalent integer equation
`(2*n2-3)*(n6-3)=1` admits only the pairs `(2,4)` and `(1,2)`, subject to the
full original case; this is a missing exact finite-case refinement, not proof
of an infinite residual direction. Separately, some native exports exceed
the default128MiB total coefficient-table budget. The Rust request already
exposes `bundle_limits.max_total_coefficient_bytes`; increasing checkpoint
disk space or process RAM does not increase that independent limit.
The frontend follow-up exposes the existing transport controls as
`--bundle-max-bytes`, `--bundle-max-entries`, `--bundle-max-coefficient-bytes`
and `--bundle-max-total-coefficient-bytes`, with corresponding underscore-named
Python keywords. Defaults and payload schemas are unchanged. Raising a
transport allowance does not change the generation policy and can reuse a
matching checkpoint; changing the rank or finite-leaf policy cannot. The
coalesced progress display also retains the failed sector and ordinal with its
bounded message, avoiding attribution to a different worker's latest event.
It is still a last-error display, not a complete failure journal.
Independent frontend review passes. Release validation passes all 231 app
tests (154 unit and 77 integration), eight Python Rust-side tests and 45 Python
API tests. No assertions were removed or tests skipped. The initial Python
packaging attempt failed in an unrelated Nix dependency refresh before building
the extension; the preserved retry uses the installed Maturin binary and passes
with unchanged source. Evidence: `TMP/frontend-bundle-gate.fjnL8o/VALIDATION.md`
and `TMP/frontend-python-retry.oDspIu/`.

A preparation-only control using the same input, an empty root and the frozen
CLI completes in6.60s with Rayon1 and8.35s with Rayon4 (both on the same four
available CPUs). It proves neither a useful parallel speedup nor that pool
width explains the earlier804s preparation outlier. It generated no rules;
do not present these figures as a five-loop solve. Raw evidence is in
`TMP/tide-preparation-pool-control.QVmvFD/`.

Native candidate metadata, generation reports and checkpoint identities retain
the rank. Resuming a checkpoint with another rank is rejected. The cold native
loader restores the bound even for a zero-only program. Starting integrals
outside the declared rank are rejected before memoization access or mutation.
Successors above the input rank are **not clipped**: every required descendant
must still have an applicable rule or an explicitly declared terminal.

## Saving an isolated sector

Use the Rust application API to keep the result of a public-core single-sector
solve, including an ordering or materializer experiment:

```rust,ignore
use rustred_app::encode_generated_candidate_sector;

// request describes the actual source, root, ordering and finite policy used.
// family, sector and solution are from the completed public-core solve.
let bytes = encode_generated_candidate_sector(
    &request, &family, sector, &solution,
)?;
std::fs::write("TMP/isolated-sector.rrbin", bytes)?;
```

This uses the existing native Symbolica coefficient table, family record and
candidate schema. It performs no source search, zero census, replay or file
operation itself; the example explicitly chooses where to write. The usual
candidate inspection, cold loader and guarded applier consume the result.
Encoding and writing should be timed separately from solving.

The helper checks the supplied family's fingerprint against the input, arity,
root containment, coordinate priority, rank and finite policy, with the usual
transport limits. This is trusted generated transport, not authentication of
solver history: the caller must supply the actual ordering, numerical depth
and other recorded settings. `SectorSolution` does not retain all of that
history. Backend choices remain in the run receipt; they do not change the
semantics of the saved exact formulas. No checkpoint is opened or amended.

Only the supplied sector is stored. A root with a larger downset does not
make that downset complete; missing lower-sector successors remain explicit
`Uncovered` errors. Terminal identity canaries establish basic loading and
application, not recursive reduction or family closure. This surface lets
diagnostic runs keep their successful exact work without inventing a second
serialization format or upgrading candidate authority.

The independent implementation audit and release gate pass all four focused
export tests and **246 application/integration tests** (169unit+77integration),
with zero failures. The full test execution takes63.47s wall/62.30CPU-s at
194,292KiB peak RSS; compilation is separate. Tests preserve exact native
formula/source layouts and nontrivial coordinate priority, reject mismatched
source/scope/transport bounds, preserve missing successors as uncovered, and
verify that export does not open a requested checkpoint or launch search.
Evidence: `TMP/single-sector-export-release.0ROvqw/`. This adds no core,
artifact-schema, CLI or Python behavior change.

### Persisted five-loop controls

The generic optimized helper now saves two actual isolated results using the
already validated affine-endpoint core, **before** the GrevLex change:

| Native sector and policy | Solver time | Whole solve/export wall / CPU | Peak RSS KiB | Rules / explicit residuals | Bundle bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| 7749, SearchFinite | 185.488s | 192.30 / 190.97s | 611,312 | 629 / 88 | 77,843,492 |
| 24996, RetainRankFinite | 10.227s | 16.31 / 16.18s | 77,932 | 348 / 93,392 | 3,877,836 |

The first has the same counts as the earlier227.235s isolated run; shared-host
activity and a non-matched repeat do not establish a speedup. The second
deliberately uses R10 finite retention with10million visited points,1million
retained terminals and8,192 geometry work items. Its93,392 fully fixed leaves
are acceptable nonminimal candidates, not inferred masters. It bypasses the
search-policy stall using a **different policy**, not a faster same-workload
materializer. No broad-search checkpoint is overwritten or relabelled.
Encoding takes0.687s and0.078s respectively, separate from solving.

Both bundles cold-load and pass three terminal-identity/repeat checks. Three
nontrivial inputs per bundle instead expose missing successors: the standalone
7749 traces contain2,433/7,024/13,553 uncovered keys, and the24996 traces
1,057/2,832/5,090. These initial bundles therefore pass **zero of three complete
nontrivial reductions each**, despite successful import and terminal canaries.

A subsequent native read-only census classifies **every**24996 frontier key:
all belong to exactly five strict lower supports, with maximum numerator rank2.
There are no same-support, outside-support or above-R gaps in these three
traces. They perform162/596/1,440 rule applications and reach
1,241/3,450/6,552 keys. This justifies completing the missing downset for those
inputs; it does not establish all-R10 or arbitrary-positive-power coverage.
The full-frontier diagnostic takes7.32s wall/7.26CPU-s, mostly cold loading,
at82,484KiB peak RSS. It regenerates no rules.

The planned source-weight29734 repeat is skipped because its original
sparse-factorized campaign shard becomes available first. That existing shard
is cold-loaded directly; no backend provenance is changed. Its three small
raised-power checks are themselves declared terminals and make zero IBP
applications. The earlier157.128s reconstruction result remains a separate
in-memory-only measurement, not the provenance of this saved shard.

Evidence: `TMP/rank10-sector-export-controls.8ikTZq/` and
`TMP/candidate-frontier-fixed.uYSlMl/`. Independent audits confirm the complete
frontier classification and the distinction between saved candidates, terminal
identity checks and actual recursive reduction.

### Completing and applying that six-sector downset

A separate public RetainRankFinite campaign generates the root plus its five
nonzero pinches: **six of six sectors**, 1,196 rules, 1,017,172 explicit terminals,
and a34,118,966-byte native bundle. It takes17.50s wall/22.30CPU-s at340,544KiB
peak RSS with four workers. This is the pre-GrevLex affine-endpoint executable,
matching the isolated finite-policy experiment. The root is regenerated
because the current checkpoint interface cannot ingest an isolated exported
program; no broad-search shard is retagged.

The first cold helper rejects the saved terminal collection at its implicit
one-million-entry ingress cap, before native import. A separately recorded
retry explicitly matches the producer's32million collection allowance and
leaves its other binary limits unchanged. It cold-loads in8.197s and completes
all five requested successor graphs with zero uncovered keys:

| Concrete input | Reachable keys | Rule applications | Reached terminals | Maximum rank |
| --- | ---: | ---: | ---: | ---: |
| All six positive powers2; no numerator | 1,469 | 1,413 | 56 | 2 |
| All six positive powers3; no numerator | 3,906 | 3,850 | 56 | 2 |
| All six positive powers4; no numerator | 7,261 | 7,205 | 56 | 2 |
| R10 in one numerator, one dot | 69 | 38 | 31 | 10 |
| R10 split across two numerators, three dots | 7,494 | 6,155 | 1,339 | 10 |

The active original axes are2,5,7,8,13,14. The fourth input changes corner
`n2=2,n0=-10`; the fifth changes `n2=3,n5=2,n0=n6=-5`. Tracing the two R10
inputs takes0.001790s and0.292616s respectively. These are finite concrete
tests, not a sample-based proof of all positive-power rays. The modest first
input also completes exact coefficient back-substitution to37 declared
terminals, with an equal memoized repeat, in0.179s. Total cold/check process:
9.39s wall/9.31CPU-s,263,548KiB peak RSS. No rules are regenerated during these
checks and no original-source replay is performed.

The separate exact R10 continuation also **passes both reductions and their
memoized repeats**: the one-numerator input gives31 declared-terminal
coefficients in0.002325s; the split input gives1,317 after exact coalescing,
in2.036167s. Both traces again have zero uncovered keys. Whole fresh process:
10.74s wall/10.66CPU-s,438,524KiB peak RSS, including8.123s cold load. The two
inputs share a reducer/cache, so these per-input intervals are not independent
cold benchmarks. Default coefficient-term, coefficient-byte and coalescing
allowances remain unchanged; the trace/application/pending limits are explicitly
sixteen million, as recorded by the driver.
no source search, terminal minimization or numerical evaluation is performed.

Evidence: `TMP/tide-24996-retained-downset.uuiRMj/`,
`TMP/tide-24996-downset-cold.k7mhsk/` and
`TMP/tide-24996-r10-backsub.cvorto/`.

## Applying a complete checkpoint directory

The Rust application API can consume a complete trusted-local generation
checkpoint directly:

```rust,ignore
use rustred_app::load_generated_candidate_checkpoint;

// The same source, root, ordering, rank, finite-case policy and exact backend
// used to generate this directory, with checkpoint.resume explicitly true.
let (family, mut reducer) =
    load_generated_candidate_checkpoint::<N>(&request, reduction_limits)?;
let result = reducer.reduce_unit_mass(&target)?;
```

It prepares the family once, checks the existing manifest and expected sector
set under the cooperative checkpoint lock, then imports the native sector
records into **one** existing `CandidateReducer`. No solver worker, sector
search, monolithic encoding or terminal minimization is invoked. The directory
is not modified, and missing, inconsistent or corrupt sectors fail rather than
being silently regenerated. Internal dependencies remain in the shared reducer;
entering another sector does not reset limits or reapply the public input-rank
check. Native zero-sector evidence is reconstructed as with the bundle loader.

All collection counts and cumulative coefficient-table entries/bytes are
preflighted across the complete directory before the first native shard import.
The existing per-shard and total checkpoint-byte bounds still apply. Increase
these explicit transport limits only for a measured input that needs them;
the API does not silently raise them. Per-shard native Symbolica state limits
plus the directory-byte limit bound state ingress, not every native allocation.
As for other native loaders, accept only trusted matching-stack outputs.

This avoids the large single-program encoding limit and buffer, **not** the
in-memory cost of the rules and explicit terminal set. Every required terminal
is retained. Loading is structural admission, not regenerated-source replay,
recursive coverage certification or a claim that the terminals are independent
masters. A successful concrete reduction demonstrates only its own reachable
dependency graph. CLI/Python generation and their checkpoint-resume behavior
are unchanged; this additional direct-load surface is currently the Rust
application API.

The source-consistent release gate passes all **237 application tests**
(160 unit and 77 integration), including six new loader tests. They exercise
exact monolithic/sharded parity, cross-sector traversal, above-entry-rank
successors and genuine uncovered children, zero-only rank preservation,
aggregate limits before native import, no-write behavior, identity changes,
locks and incomplete/corrupt input. Independent implementation and mathematical
audits pass. An initial test expected the later manifest-mismatch message for
root narrowing; existing ordinal admission correctly rejects it earlier. Only
that diagnostic expectation was corrected, retaining the rejection kind and
no-write assertions. The failed receipt is preserved separately from the
passing gate in `TMP/checkpoint-load-gate-fixed.mZqdwA/`.

### Real 328-sector application check

The first optimized fresh-process check loads the complete 30563 checkpoint,
including all **30,431,209** explicit nonminimal terminals, in **124.614 s**.
No saved sector is regenerated, and all checkpoint hashes remain unchanged.
The 64-million collection allowance is explicit; the mathematical request and
generation policy remain the original R=10 finite-retention request.

| Starting input | Trace result | Exact terminal output |
|---|---|---:|
| Unit positive powers, R=0 (declared terminal) | 1 key, no uncovered entry | 1 term |
| One raised denominator, R=0 | 56,697 keys, 53,768 applications, no uncovered entry | 1,214 terms |
| One raised denominator, R=1 | 73,942 keys, 70,888 applications, no uncovered entry | 1,257 terms |
| One raised denominator, R=10 | 100,001st key exceeds 100,000 limit | Not attempted |
| Three total dots, split R=10 numerator | Same explicit trace-node limit | Not attempted |
| One pinch, one dot, R=1 | 4,794 keys, no uncovered entry | 180 terms |
| Two pinches, one dot, R=1 | 3,548 keys, no uncovered entry | 155 terms |

All five successful reductions return only declared terminals and pass their
exact memoized repeats. The process correctly exits1 for the two incomplete
checks. Whole-process time is **332.45 s wall / 330.19 CPU-s**, with
**8,355,028 KiB peak RSS**. The one-dot R=0/R=1 per-target intervals are
89.446/101.081 s, including trace, exact coefficient accumulation and repeat;
they share one reducer/cache and are not independent cold scalar timings.
The internal timer excludes final teardown, unlike the whole-process receipt.
No fixed master evaluations or numerical comparisons are involved.

The successful raised-power traces already reach numerator degree2, illustrating
why entry rank must not clip successors. Neither limit-hit R=10 trace returns
a complete frontier; it cannot establish either missing rules or closure.
Arbitrarily raised positive powers still require a generic successor-coverage
argument. Evidence for that first application run:
`TMP/tide-r10-30563-checkpoint-load.6FFOty/`.

Subsequent fresh-process **trace-only** diagnostics reuse exactly those rules
and terminals. Here the common sector corner has unit positive powers at
indices `0,1,2,4,5,6,8,9,13,14` and zero elsewhere. Input A changes `n0=2,
n3=-10`; input B changes `n0=3, n1=2, n3=n7=-5`. Both have entry rank10.

| Input | Unique reachable keys | Rule applications | Distinct terminals | Result |
|---|---:|---:|---:|---|
| A | 697,556 | 628,559 | 68,997 | Complete trace, zero uncovered |
| A with `n14=0` | 430,467 | 387,582 | 42,885 | Complete trace, zero uncovered |
| B with `n13=n14=0` | 80,584 | 74,026 | 6,558 | Complete trace, zero uncovered |
| B, first allowance | Limit 1,000,000 reached | — | — | Incomplete work-budget result |
| B, explicit larger allowance | Limit 4,000,000 reached | 3,497,280 before failure | — | Still incomplete |
| B, final explicit 16-million allowance | 5,239,566 | 4,804,421 | 435,145 | Complete trace, zero uncovered |

The three completed traces observe maximum numerator rank10; the A traces
reach thirteen total dots and the pinched B trace eleven. Their trace times
are 58.348, 31.207 and 5.51 seconds respectively. The first four-target process
takes **343.49 s wall / 340.93 CPU-s**, peaking at **6,638,552 KiB RSS**;
its initial checkpoint load takes 154.369 s. The isolated four-million-node B
retry takes **346.001 s tracing**, with a 142.606 s cold load and **496.51 s
whole-process wall / 492.83 CPU-s / 7,215,308 KiB peak RSS**. Both drivers
correctly exit1 because an input remains incomplete. Input/checkpoint hashes
are unchanged. The incomplete trace returns no partial frontier or maximum
rank, so it cannot establish either a missing rule or its absence.

A final isolated B trace, using the same frozen client and a sixteen-million
allowance, finishes successfully rather than hitting another cap. It takes
**425.529 s tracing**, observes maximum rank10 and thirteen dots, and finds
zero uncovered keys. The cold load takes 181.470 s; whole process is
**615.53 s wall /611.00 CPU-s /7,417,280 KiB peak RSS** with status0. The earlier
failures were work-budget limits for this input, not missing rules. None of
these traces accumulates its 435,145 terminal coefficients.

These checks perform no coefficient back-substitution, original-source replay,
master evaluation or rule generation. They show concrete dependency coverage,
not universal rank-10 family closure. Receipts:
`TMP/tide-r10-30563-boundary-trace.d5plSz/` and
`TMP/tide-r10-30563-four-million.0NT7FB/`; the successful final retry is in
`TMP/tide-r10-30563-sixteen-million.azFTOj/`.

### A second complete saved generation census

Representative29550 now has **462/462** required sector programs saved under
its original R10 finite-retention policy. The refreshed campaign reuses459
and generates the last three with the validated affine-endpoint executable,
not the later GrevLex implementation. The new sectors contribute882 rules
and138,307 explicit residuals; these are additions, not full-family totals.
The whole four-worker process takes463.36s wall/626.86CPU-s and peaks at
5,043,424KiB RSS.

Final monolithic export hits its512MiB coefficient-table cap and exits4.
All462 native shards remain saved. This is an output limit, not a missing
sector, and does not justify regenerating them. A separate complete-checkpoint
cold-load attempt likewise stops at its explicit512MiB aggregate ingress cap
before native import or tracing. Inspection finds1,047,043,921 coefficient-table
bytes across2,697,574,447 total shard bytes. A separately labelled 1 GiB ingress
retry successfully loads all 462 shards and **45,127,877 explicit terminals**
in 217.446 s, without changing generation policy or the 4 GiB checkpoint /
64-million collection bounds. Its R10 one-dot target then reaches the explicit
one-million-key trace cap after 60.255 s and 697,289 rule applications. No
completed frontier is returned. Whole process: 291.29 s wall / 288.97 CPU-s,
12,869,504 KiB peak RSS, status 1 for the incomplete trace. The preserved
candidate data is unchanged. A fresh trace-only retry uses the same client
with sixteen million keys/applications; it neither recompiles nor regenerates
IBPs. That retry also reaches its explicit key limit: 16,000,001 requested
keys after 13,843,476 rule applications and 1,214.584 s of tracing. Cold load
takes 240.477 s; the whole process takes 1,473.67 s wall / 1,461.61 CPU-s and
peaks at 15,826,160 KiB RSS. Neither its memory nor time ceiling is reached.
The typed failure returns no partial frontier, so it establishes neither a
particular missing rule nor zero uncovered successors. No further automatic
cap increase is made. Saved generation, cold-load admission and recursive
application are distinct gates; no uniform rank-10 family closure is claimed.

Evidence: `TMP/tide-r10-29550-affine-refresh.1stuqI/`,
`TMP/tide-r10-29550-cold-trace.y3zvc5/` and
`TMP/tide-r10-29550-coefficient-budget.1mNno7/`; the larger trace receipt is
`TMP/tide-r10-29550-sixteen-million.dlwmIQ/`.

### Parallel progress and a positive-power exceptional face

All four broad SearchFinite campaigns now run the validated GrevLex executable,
reusing their previously saved compatible shards. A non-atomic metadata census
at **September 21, 13:14:14 UTC** finds **5,731/8,246 distinct saved labels**:

| Published parent | Saved / scheduled | Reused on restart | Workers |
| --- | ---: | ---: | ---: |
| 30527 | 767 / 2,686 | 723 | 6 |
| 30699 | 2,052 / 2,580 | 1,970 | 8 |
| 31740 | 1,721 / 2,656 | 1,667 | 8 |
| 32745 | 2,414 / 2,478 | 2,396 | 8 |

The 6,954 parent occurrences include 1,223 duplicate labels. These saved
programs represent 65 of 67 routed graph classes; the two absent classes in
this policy-specific inventory are 29550 and 30231. The separate finite-retention
programs are excluded rather than silently relabelled or merged. Saved sectors
and represented classes are not recursive closure. Actual registered RSS is
18.26 GB at this snapshot, with a historical sampled peak of 337.95 GB. Worker
counts are deliberately below the authorized 100 cores to limit memory growth;
the aggregate monitor's 450 GB soft stop remains in force below the 500 GB ceiling.

The separate 30231 finite-retention campaign has 196/198 saved sectors. Its
refreshed run exposes an unsupported nonlinear equality conjunction in native
sector 29751, rather than a time or work-limit failure. An isolated reproduction
captures the full original and unresolved branch constraints in 154.257 s solver time.
The unresolved face leaves only `n0,n1,n2` free, all positive propagator powers;
every numerator coordinate is fixed to zero. Thus even an input-rank-zero
restriction leaves these directions unbounded. A rank cap alone cannot turn
this face into a finite collection of terminals. Subsequent exact native
analysis finds that this particular residual has **no positive-integer points**,
despite its positive algebraic dimension. The other unsaved sector remains in
exact elimination. No failed branch is classified as a master.

Write the free indices as `x=n0,y=n1,z=n2`. Direct native Lex normalization
takes about 1.1 ms and exposes the necessary factor equation

```text
(z-3) * (1828627*z^2-9250536*z+11488176)
      * (y-2*z+2) * (y+z-3) = 0.
```

Retaining the full original and unresolved conjunction in every branch gives:

- `y-2*z+2=0` forces `x=0`, outside the positive sector.
- `y+z=3` allows only positive integer `(y,z)=(2,1),(1,2)`; their exact `x`
  values are `8/3` and `-1`, both inadmissible.
- `z=3` gives exactly `(-3,0,3)`, `(0,4,3)` and
  `(-90547/258172,2,3)`, none positive integral.
- The quadratic factor in `z` is irreducible over the rationals under native
  factorization, so cannot have an integer root.

An earlier diagnostic using only the wider original conjunction found a ray;
that provisional interpretation was withdrawn once the narrowed residual's
additional equations were retained. This is why whole-conjunction preservation
matters. The production correction is still pending: keep the fast GrevLex
path, and test a structurally bounded native Lex fallback only when the
existing refinements would otherwise report unsupported geometry. The native
algebra diagnosis is not yet a completed sector rerun or a closure artifact.

A separate source-weight reconstruction diagnostic on the older executable
successfully reconstructs a 1,254-term target frame at 834.267 s, then stalls
in the shared old Lex-F4 exceptional normalizer. It is intentionally stopped
at 1,537.54 s wall / 1,526.03 CPU-s, 12,227,744 KiB peak RSS (status 143),
before its one-hour deadline. A 20-second profile confirms that shared geometry
bottleneck. This is neither a reconstruction API failure nor a completed
backend timing comparison; no candidate file is produced.

Receipts: `TMP/rank10-search-refresh-grevlex.qT9JMZ/`,
`TMP/native29751-nonlinear-capture.X5gihG/`,
`TMP/native29751-residual-branches.q0WpkX/`,
`TMP/tide-r10-30231-grevlex-corrected.U3MZvm/`, and
`TMP/tide-root30231-source-weights.efvT5i/`.

## How nonlinear exceptions are handled

The ordinary exact geometry path first uses native normalization and affine or
factor refinements. A follow-up service covers the authenticated two-index
integer form `A*x*y+B*x+C*y+D=0`, with nonzero `A`, by
`(A*x+C)*(A*y+B)=B*C-A*D`. For a nonzero right-hand side within the exact native
u64-primality range, Symbolica integer factorization and exact quotient/remainder
operations enumerate every signed divisor and enforce both congruences. For a
zero right-hand side, the result is two affine faces, not two fixed points.
Every child retains the full original parent and conjunction. Larger constants,
other coefficient variables and other nonlinear forms are not silently sampled
or declared empty. This implementation is topology- and loop-count independent;
the captured five-loop guards are regression inputs, not dispatch keys.

This refinement's work and factorization limits are shared with the enclosing
intersection. Exhaustion invalidates the whole result, never just the remaining
children. The focused release gate checks signed divisors, congruences, zero
product faces, parent restrictions, compact-index limits and adversarial work
limits. The source-consistent release core gate passes **2,385 tests**, with
zero failures and 32 existing ignored tests; all 12 focused bilinear tests
pass. Independent mathematical/implementation review also passes. The core
slice is committed and pushed as `e34df18e`; frontend gates are separate.
Formerly unsupported bilinear regression fixtures were changed to genuine
unsupported Pell loci without altering their no-partial-publication assertions.
One initially incorrect no-integer test fixture was corrected; its failed
receipt is retained. Evidence: `TMP/bilinear-release-final.MYtg1P/RESULTS.md`.
The already running broad campaigns remain on their
frozen older executable and do not inherit this change mid-run.

An additional exact refinement handles authenticated integer equations
`(a*y+b)*x+Q(y)=0`, where `a` is nonzero and `Q` is univariate of degree greater
than one. Symbolica computes `S=Res(a*y+b,Q)`. When `S` is nonzero and its
absolute value is in the supported exact native factorization range, every
integer solution has `u=a*y+b` dividing `S`. Exhaust all signed divisors and
use native zero-remainder checks for `y=(u-b)/a` and `x=-Q(y)/u`. This is a
complete finite integer refinement for the authenticated form, without
assuming coprimality, bounding positive powers, or implementing a resultant
kernel in RustRed. For example, the observed condition
`37-17*y+2*y^2-14*x+4*x*y=0` has resultant 32 and the sole integer pair `(2,3)`;
the full original parent still decides whether that pair is admissible.

Zero resultant, unsupported support, nonlinear coefficient of `x`, or an
out-of-range constant returns no finite inference. Every child retains the
complete conjunction. Dense conversion, native calls and divisor visits share
existing work budgets; exhaustion remains atomic failure. The independent
mathematical and implementation audit passes, as do all eight new focused
tests, all twelve unchanged bilinear tests and the source-consistent release
core gate: **2,393 passed, zero failed, 32 existing ignored**. This does not
solve arbitrary nonlinear Diophantine loci or establish family closure.
Evidence: `TMP/linear-resultant-release.IQkBpR/` and
`TMP/rank10-inventory-audit.9t07iu/LINEAR_RESULTANT_IMPLEMENTATION_AUDIT.md`.
The fresh production CLI also passes the seven-sector R=10 regression without
reusing checkpoints: **34.50 s wall, 39.80 s CPU, 362,116 KiB peak RSS**,
1,299 rules and 1,208,801 declared terminals. Its 41,971,427-byte output is
byte-identical to the prior CLI bundle. This is a source-consistent integration
regression, not a controlled speedup claim; it does not show that the new
resultant branch was visited. The captured guard's focused test establishes
that separately. Evidence: `TMP/linear-resultant-r10-pilot.3FeIfD/`.

### Rank admission before compact index conversion

The broad run for parent 31740 exposed a different admission-ordering defect,
not another nonlinear solver limitation. One exceptional AND branch already
fixes `n1=-8` and forces `n13=-66, n5=-45`. Its negative degree is 119, so it
cannot intersect the requested rank-10 domain. The former path tried to encode
`-66` as a compact power before checking that scope and failed the whole sector.

The correction keeps the already available native Symbolica integer values
until their forced negative-degree lower bound has been checked. Affine
admission similarly checks singleton rows of the existing native RREF before
converting any coordinate. Only distinct, fixed **original** indices count;
coupled chart constants are not index values. Complete input validation still
comes first. This prunes only the impossible AND branch, not a whole original
guard or its valid OR siblings. In-scope and unbounded overflows remain typed
errors. There is no wider packed-index format or additional CAS implementation.

Independent implementation/mathematical review and all eight new focused
release tests pass, including the captured 15-axis branch and exact enumeration
of all 286 points in the original three-index rank-10 simplex. The unchanged
geometry, affine, intersection and solver focused suites also pass. The full
source-consistent core release gate passes **2,401 tests, zero failures,
32 existing ignored tests**. An actual failed-sector retry is a separate check;
the already running campaigns continue using their immutable older executable.
The fresh public CLI also passes the seven-sector R=10 regression in 33.79 s
wall /39.72 CPU-s /356,960 KiB peak RSS, producing exactly the prior 1,299 rules,
1,208,801 terminals and 41,971,427-byte bundle. Byte equivalence is checked;
this is not a controlled speed comparison or a claim to have visited the new
captured-guard branch. Evidence: `TMP/rank-before-compact-pilot.Z3b4kG/`.
Evidence: `TMP/rank-before-compact.fZmoU7/` and
`TMP/rank-overflow-independent-audit.iFN2cn/`.

If exact geometry reaches a genuinely
unsupported nonlinear branch and a rank was supplied, it fixes one still-free
inactive original coordinate to each possible value allowed by the remaining
total numerator degree. Existing Symbolica substitution, affine admission,
factorization and ideal normalization then simplify each child before further
splitting. Already fixed negative powers consume the bound. Positive axes are
never assigned an arbitrary value or bounded by this fallback.

All children share the same geometry work limits. A failed child makes the
entire intersection incomplete; unsupported positive-power geometry, native
algebra errors, compact-index overflow and exhausted budgets are not empty
sets. The returned equality domains may cover more points outside R, but their
claimed completeness is only their intersection with the stated rank bound.
Raw nonlinear rule guards remain intact for exact concrete application.

This service does not introduce a general Diophantine solver, interpolation or
rational reconstruction kernel. Polynomial and integer algebra use the existing
Symbolica API; RustRed orchestrates a finite case queue in original coordinates
and the narrow exhaustive divisor refinement described above.

## Dependency inspection and limits of the claim

`CandidateReducer::trace_targets(targets, CandidateTraceLimits::default())`
follows exact applicable rules and returns all encountered uncovered keys,
explicit terminals, visited zero entries and maximum observed numerator/dot
degrees. It uses the existing guarded candidate evaluator, not another reducer.
Both input entries and distinct reachable keys are bounded; all entries share
the existing application, pending-frame and exact-arithmetic budgets. The
decomposition cache is neither read nor modified.

Only absence of an applicable rule is retained as an uncovered frontier.
Malformed formulas, failed source conditions, non-descending applications and
resource failures still abort. This trace does not independently replay source
identities. Frontier keys are not automatically masters. Since coefficients
are not back-substituted across distinct paths, some frontier terms might later
cancel; the report is a work list, not a count of independent residuals.

Successful generation is still a **candidate program**, not a closed artifact.
In particular, bounded numerator entries with arbitrary dots can have
intermediates of larger numerator degree. A proof or complete reduction must
account for those successors. Current unrestricted and total-excess certificate
installers therefore reject rank-scoped candidates rather than relabeling their
scope. The public rank option is a generation/application capability, not a
claim that all five-loop families have already been solved.

## September 21 validation and five-loop pilots

The release core suite passes **2,360 tests**, with zero failures and 32
existing ignored tests. This includes seven bounded-intersection tests and
fourteen successor-trace tests; these focused counts overlap the full suite.
An independent public-API harness also reproduces the seven intersection
tests against the optimized production library. Its assertion harness was
unoptimized and its runtime is not a solver benchmark.

The application gate passes all 147 private unit tests. All 15 CLI candidate
tests pass five consecutive times against the same optimized CLI binary.
An initial test-harness broken pipe was fixed without changing assertions:
the child can correctly reject arguments before reading stdin. All 42 Python
API tests pass with the optimized extension and four available CPUs. A prior
one-CPU harness incorrectly prevented the two multicore tests from launching;
that failed receipt is retained, and the corrected run changes only affinity.
An independently built unoptimized thin Python binding, linked to optimized
native application/core libraries, also passes all 42 tests. These are
correctness gates, not interchangeable solver timings.

The captured five-loop nonlinear conjunction is checked against the complete
original-coordinate negative-degree simplices: 3,003 points at R=10 and
53,130 at R=20. The admitted exceptional loci contain three and five points,
respectively. This verifies that specific geometry operation, not coverage of
the family or its descendants. K1 and K3 public CLI smoke runs also succeed
at both ranks and with one and two workers.

The first natural-order five-loop six-line/downset pilot uses the exact CLI
example above. With sparse exact materialization it reaches 248 observed rule
hits and case 249, but the ten-minute supervisor expires during exact
elimination before any of the seven nonzero sectors completes. The last event
starts row 511 of 520 in a frame with 1,948 integral columns and 48,019 stored
upper-row nonzeros. Those partial hits are not a saved candidate program.
Actual process wall time is 612.73 seconds, CPU time 600.68 seconds, and peak
RSS 343,316 KiB. A brief late-case sampling profile was attached, so this is
an instrumented diagnostic, not a clean performance comparison.

A same-input reconstruction repeat expires earlier in the common preparation
phase: 610.89 seconds wall, 610.78 CPU seconds, 70,432 KiB peak RSS, no sector
started. Backend selection has not yet occurred at that point. It therefore
does **not** measure reconstruction performance or establish a reconstruction
failure. Fresh small K3 controls succeed with both backends. The preparation
discrepancy requires diagnosis before comparing the two five-loop timings.

Both serial attempts return timeout status 124 and produce no complete
candidate bundle. A later six-worker reconstruction pilot on the same input
uses a one-hour deadline and a 16 GiB address-space limit. It stops at that
deadline (status 124), completing three of seven nonzero sectors and saving
1,234 rules and three
residuals in durable native checkpoint shards. These are three labelled
five-line presentations, not three inequivalent graph classes or the full
five-loop census. Common preparation took about 804 seconds. A separate
fresh-process load of one shard followed by five corner/dot/numerator canaries
passes: 98 reachable entries, 97 rule applications, no uncovered target and
one declared terminal. All five exact reductions and their memoized repeats
succeed. The check takes 7.58 seconds, including 7.52 seconds to load; it
does not replay source identities or prove coverage beyond those entries.
A second cold shard check passes another five entries and their cache repeats:
116 reachable keys, 115 exact applications, no uncovered target, one terminal,
7.62 seconds wall. Both checker processes use an unoptimized generic assertion
harness linked to optimized native libraries, so these are diagnostic timings,
not production application benchmarks.

The campaign's measured wall time is 3,600.00 seconds, CPU time 12,102.89 seconds
(12,067.45 user plus 35.44 system), and peak RSS 6,351,772 KiB. At termination,
the coalesced last event is sector 14339, case 319, reconstructing an exact
frame with 469 source rows, 1,498 integral columns and four effective variables.
Some coefficients need thousands of probes and three primes. The parent had
advanced beyond its earlier case-256 exceptional-geometry phase; the log is
coalesced across workers and is not a complete per-case trace. Its 2,375
observed rule hits include unfinished-sector work and must not replace the
1,234 durably saved rules in the reported result.

A 20-second userspace sampling profile across the live process collects 3,871
samples with no reported lost samples. Native integer multiplication, polynomial
division and Groebner arithmetic appear prominently. It is a local late-phase
sample, not a whole-run attribution; the campaign is consequently instrumented,
not a clean solver-throughput comparison. That reconstruction campaign produced
no complete seven-sector candidate bundle. Original CLI/input hashes still match
after termination.

Positive-power and above-entry-rank successor coverage remain open; terminal
minimization, numerical-master lookup and Vakint five-loop integration have
not begun. Evidence is retained locally in
`TMP/rank-scoped-gate.DWKbBo`, `TMP/rank-public-api-tests.xvfStw`,
`TMP/tide-natural-rank10-public.q6yDHY`, and
`TMP/tide-natural-rank10-reconstruction.2VscxW`,
`TMP/tide-rank10-six-workers.Z0fPdr`, and
`TMP/tide-r10-shard-canary.KwfDWh`.
