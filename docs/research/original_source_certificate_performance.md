# Original-source certificates: provenance composition and measured bottlenecks

Status, 2026-09-16: relative-face implication, native singleton-specialized
affine reasoning and bounded exact face refinement are implemented. The
latest selected FG214 release audit now admits **161/161** exact-replayed and
uniformly descending rules in **85.10 seconds wall**. It still exits 8:
the stored and checked predicate-cover gates reject an abstract Boolean
valuation, so **neither sector closure nor a four-loop artifact is established**.
The exact diagnostic proves the first reported valuation unrealizable; a
separate consistency correction is the next step, not part of this measured
revision. The focused debug gate passes **427 tests**. Every exact replay,
guard, descent, ownership and cold-load gate remains required.

## Geometry follow-up after provenance composition

The first geometry slice uses the existing Symbolica-backed affine service
to prove exclusions relative to the actual child fixed face:
`parent ∩ child_fixed_face ⊆ child_equalities`. It no longer requires an
equality valid only on that face to follow from the unrestricted parent.
Native exact row reduction also combines coupled affine equations after
specializing singleton box coordinates. Both are exact domain proofs, not
new elimination algorithms, numerical sampling or additional master choices.

The resulting serial `--release --locked` FG214 diagnostic retains the same
161 candidate rules and eleven finite terminal candidates:

| Boundary | Completed first geometry slice |
| --- | ---: |
| Search | 15.430 s |
| Exact audit | 84.417 s |
| Whole-process wall | 100.29 s |
| CPU | 99.53 s (98.86 user + 0.67 system) |
| Peak RSS | 97,412 KiB |
| Admitted replay/descent rules | 159/161 |
| Exit | 8; incomplete proof, no artifact |

At that first geometry checkpoint, rules 69–73 pass their exact gates. The
remaining **zero-based** failures
are **rule 100, RHS term 34**, and **rule 101, RHS term 26**; these are later
terms than those reported at the preceding revision. Both fail stored-guard
uniform descent geometry. The stored and checked covers again report one
uncovered box and zero unbounded boxes, but those counters stop at the first
failing Boolean valuation and are **not a census** of uncovered domains.
They do not justify dropping either candidate or declaring the infinite
complement empty. Logs, command, timings and profile are retained at
`/tmp/rustred-affine-face-release.HLRRJy/`.

The follow-up in `parametric/affine/box_bounds.rs` chooses **one** shortest
equation-dependent finite axis, provided it has at most **eight integer
values**. It exhausts that axis and requires a proof of emptiness on **every**
resulting face, leaving all other finite and infinite intervals unchanged.
The eight-face cap is a generic work limit, not a K6/FG-specific rule. This is
neither recursive splitting nor a Cartesian-product expansion, and it never
samples an infinite ray. Native Symbolica integers, polynomial substitution
and exact equation reduction supply the algebra; aggregate matrix and
polynomial work is preflighted. A face that cannot be proved empty, an
unsupported equation or an exceeded budget leaves the result inconclusive,
never falsely empty.

The new diagnostic preserves the first failing box and exact Boolean
equations, distinguishing true, false and unassigned atoms. It labels this
an abstract Boolean witness, **not a concrete uncovered integral or a closure
certificate**. The debug gate passes **427 tests, zero failures and five
existing ignored workloads** in 1.67 seconds; evidence is
`/tmp/rustred-fg214-geometry.mC9IJi/width-tests.stdout`.

The follow-up's full **selected-sector** release audit has now completed:

| Boundary | Bounded-face follow-up |
| --- | ---: |
| Search | 15.086 s |
| Exact audit | 69.747864194 s |
| Whole-process wall | 85.10 s |
| CPU | 84.46 s (84.22 user + 0.24 system) |
| Peak RSS | 100,912 KiB |
| Exact-replayed rules | 161/161 |
| Uniformly descending rules | 161/161 |
| Redundant rules / additional replay guard branches | 0 / 0 |
| Replay source entries / finite terminal candidates | 11,217 / 11 |
| Exit | 8; predicate-cover rejection only, no artifact |

Thus both remaining rule-geometry failures are resolved by the exact gates,
without dropping rules or adding terminals. Stored and checked predicate
coverage still fail on the same first abstract Boolean box; one reported
bounded box and zero unbounded boxes remain nonexhaustive counters. The
sector is **not closed**. This is not a rerun of the full 124-sector family
campaign. Raw outputs and timings are
`/tmp/rustred-affine-width-release.CNbu4g/selected-sector.stdout` and
`selected-sector.time`.

The release **stored-cover-only diagnostic** has completed in **13.37 seconds
wall** (13.072 s search, 0.139 s predicate diagnostic). It deliberately removes
source traces and therefore performs **no successful original-source replay**;
this timing must not be substituted for the full selected audit above. Its
first Boolean box fixes the physical
indices to `(-1,1,1,0,1,-1,1,1,-2,0)`. Atom 2 requires
`q=1-n9+n5 != 0`, yet exact substitution gives `q=1-0-1=0`.
This particular reported Boolean witness is thus proved unrealizable, not
merely suspected to be a spurious gap. It does **not** establish consistency
or coverage of later valuations. A separate narrow singleton-consistency
correction is being developed from this evidence; it is not part of the
completed selected release audit. Raw evidence is
`/tmp/rustred-affine-width-release.CNbu4g/stored-cover-only.stdout`.

K1/K3/K6 core-driver artifacts after the first geometry slice are byte-identical
to their prior provenance counterparts; fresh-process inspection and canary
reductions also pass and match. This K6 invocation uses the **unnamed
diagnostic input**, producing 8,925,944 bytes, 623 rules, 5,640 cells and 38
masters in **2.78 seconds wall**. It is not the canonical named CLI artifact
or a new public-API performance benchmark. All timings here are single
shared-host observations, not controlled speedup comparisons.

The final bounded-face snapshot repeats this lower-loop gate successfully:
K1/K3/K6 generation, fresh-process inspection and canary reduction all pass;
both artifacts and reductions remain byte-identical to the preceding producer.
The unnamed K6 input above takes **2.88 seconds wall** (2.47 user + 0.37 system),
with 245,772 KiB peak RSS, and retains SHA256
`834505478c3c682a3427f53884afcf098555a53cc6198cba2d5397e1a6c9e9dd`.
Generation uses the new public-core release driver; cold inspection/application
use the preceding compatible frozen CLI on these identical bytes. This does
not constitute a rebuild of the current CLI/Python frontend or a four-loop
cold-load test. Records are in `/tmp/rustred-affine-width-release.CNbu4g/`.

## Historical provenance-composition checkpoint

The following records the preceding revision, before the geometry fixes
above. Its selected FG214 result was **154/161** in **106.60 seconds wall**;
its full physical-FG CLI attempt exited fail-closed in **138.07 seconds**
without an artifact. References below to unimplemented geometry corrections
describe that historical checkpoint, not the current implementation.

### Implemented provenance composition

`solver/precondition/provenance.rs` records the existing preconditioner's
forward polynomial identities, `B_new = a B_left - b B_right`, in an optional
immutable DAG. It records structure and native Symbolica coefficients, not a
new elimination or CAS algorithm. The ordinary solver entry remains untraced:
it allocates no provenance arena and does not clone scales for this feature.
`SourcePortAudit::check_sector` regenerates the traced preconditioner once for
the sector's verification pass; search does not propagate expanded transitive
source combinations.

After the existing selected-frame exact replay has recovered its weights,
`ordinary/provenance.rs::compose` groups them by seed and walks only reachable
DAG edges backwards to obtain ordinary-source weights. Native arithmetic
combines seed translation with canonical recentering before fixed-coordinate
specialization and affine-chart restriction, preserving the original order. Foreign
coefficient maps, invalid roots, overflow, movement of absolute numeric
coordinates, and nontangent affine translations reject the proposal. A
vanishing polynomial scale does not invalidate its forward identity; no
inverse-preconditioner or specialized-rank assumption is used.

Composition is a fast proposal, **not new authority**. It is admitted only
when inherited source conditions are index-free, existing exact pole-domain
checks allow its weights, and full unprojected original-row weighted replay
reproduces the desired identity. An inconclusive composition, new unsupported
pole or replay failure takes the unchanged compact/full exact proposal routes
and their same-quotient fallbacks. Original normalization, exceptional-domain
checks, strict descent and cover compilation still follow. The DAG is neither
serialized nor trusted by cold loading: artifacts retain ordinary-source
certificates under the existing replay contract.

The integrated `solver::` and `foundry::artifact::source_port` debug selection
passes **359 tests, zero failures, five existing ignored workloads**, in
1.57 seconds. This includes seven DAG and five composition regressions, with
an explicit fast-path-versus-fallback witness and a nonconstant affine scale
under nonzero tangent recentering. Independent implementation and mathematical
audits checked the proof boundary. Evidence is retained at
`/tmp/rustred-provenance-integration-tests.NtDJwA/`; these are correctness
results, not release performance measurements.

### Selected release grounding

Both selected-sector drivers use the same external FG mathematical input,
natural ordering, global zero census of 281 masks, and physical root domain
`n8,n9<=0`. They link a `--release --locked` core build and run serially on CPU
34 with nested pools capped at one. Compilation precedes the timed process.

| Selected sector | Search | Exact audit | Whole-process wall | CPU | Peak RSS | Result |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| FG106 | 0.331 s | 0.270 s | 0.70 s | 0.69 s | 12,288 KiB | 99/99 replayed and descending; zero gaps/issues; exit 0 |
| FG214 | 12.521 s | 93.884 s | 106.60 s | 105.77 s | 96,392 KiB | 154/161 replayed and descending; incomplete proof; exit 8 |

FG214 retains eleven finite terminal candidates. Its failing **zero-based**
rule ordinals are **69–73, 100 and 101**. The errors arise in stored-guard
geometry, before the affected rules can receive successful replay/descent
admission; they must not be described as seven disproved identities or seven
original-source residual failures. The first reported nonlower RHS term is
term 0 for rules 69, 70, 72, 100 and 101; term 2 for rule 71; and term 1 for
rule 73. Their exact affine targets and exceptional subcases require a
separate domain-aware investigation, not omission based on numerical probes.

Both stored and checked cover counters report **one uncovered box and zero
unbounded boxes**. Crucially, `predicate_cover::check_valuations` returns on
its **first failing Boolean valuation**. These are not an exhaustive census
of all missing domains; they do not prove that only one finite master is
missing, that every infinite ray closes, or that the rejected affine rules
can be discarded. The checked rules' full exact predicate cover is incomplete,
and no artifact is published by either selected-sector diagnostic.

Independent inspection of the exact dumped cases identifies two narrow,
generic geometry deficiencies for the **reported** failing pieces:

- Rules 69–73 need an exclusion implication relative to the child fixed face,
  `parent ∩ child_fixed_face ⊆ child_equalities`, rather than implication from
  the parent alone. For example, rule 69's parent is
  `n0-2*n8-n9-1=0`; its excluded face `n9=0` also carries
  `n0-2*n8-1=0`. That child equation follows exactly on the face, so the
  reported activation of `n9` from 0 to 1 is outside the true rule domain.
- Rules 100/101 need coupled equations combined after specializing singleton
  box coordinates. For rule 100, `n8=0` together with `n8-n9-1=0` forces
  `n9=-1`, then `n0-3*n9-3=0` forces `n0=0`, contradicting its reported
  `n0∈[-2,-1]` box. Checking the equations separately misses this contradiction.

These proposed corrections are **not implemented or counted as passing** in
the measured run. They must reuse native chart restriction/row reduction,
preserve genuinely new child equations and feasible near-neighbors, and work
identically in warm and cold verification. A separate bounded check covers
all 3,125 tested corner points with the 161 stored candidates/terminals but
finds twelve actual holes after dropping the seven rejected rules. This finite
check alone is not an unbounded cover proof. Separately, independent exact
audit verifies three infinite rays owned by rules 69/70/72 and excludes every
other owner throughout their respective integer-parameter ranges. Removing
those owners therefore leaves genuine infinite gaps, not merely the first
bounded piece reported by the Boolean checker. This does not certify the
rejected rules: their geometry corrections and complete exact gates remain
unimplemented at this checkpoint.

For each independently checked ray, `n1=n2=n4=n6=n7=1`:

| Sole applicable stored owner | Remaining coordinates | Integer range |
| --- | --- | --- |
| 69 | `n3=n5=0`, `(n0,n8,n9)=(-t-1,-1,-t)` | `t>=3` |
| 70 | `n0=n5=0`, `(n3,n8,n9)=(-t,1-2*t,-2)` | `t>=2` |
| 72 | `n3=n9=0`, `(n0,n5,n8)=(-t,-2*t,-1)` | `t>=2` |

The independent audit enumerates every fixed-face-compatible rule, excludes
its full equation/exception domain along the ray, and checks that none of
the eleven finite terminals lies there. These rays are explicit infinite
holes in the retained 154-rule cover, not evidence that new IBPs necessarily
need to be discovered: the rejected stored owners may already supply the
needed identities once all their proof obligations have been discharged.

The first Boolean failure may itself involve unrealizable equality assignments,
but its actual box/assignment must be retained and checked before claiming
that explanation. Exact dumps, bounded checks and the independent audit are
at `/tmp/rustred-fg214-geometry.mC9IJi/`, especially `AUDIT.md`.

A sampled window from the new FG214 run contains 724 CPU-cycle samples with
no lost samples. Inclusive stacks place **99.68%** in selected-frame
`source_port::replay::replay_rule`, **84.61%** in native exact
`SparseRowReducer::add_row`, and **47.53%** in polynomial GCD. These overlapping
fractions are local stack attribution, not additive timings or whole-run
shares. The sampled bottleneck is now selected-frame replay, not the second
original-source membership proposal profiled in the older full runs. The
sample does not identify an active rule ordinal or prove that the original
fallback is never used elsewhere.

Release CLI K1/K3/K6 generation, fresh-process inspection and reduction
canaries also pass. Using the canonical `examples/input/three_loop_k6.toml`,
the new K6 payload retains 623 rules, 5,640 cells and 38 masters in
**8,926,013 bytes**. One-, two- and six-worker outputs agree exactly, SHA256
`ce62d5cdad5350ddd8b8213d7ceee7c183b39ffe56fe76565831c0e390e9e290`.
The dotted canary output is byte-identical to the prior compact-support
producer's output. Serial CLI generation takes **3.23 seconds wall**, 2.75 s
user + 0.44 s system, and 248,860 KiB peak RSS. Its frozen executable SHA256 is
`63922deebb116877df6e66250b530e3a4e8214bfd5123b54fc64f7ba5bf7c1d9`.

The earlier core-driver regression used an unnamed diagnostic input; its
8,925,944-byte artifact and 2.67-second timing are a distinct input identity
and invocation, not the canonical CLI artifact above. All timings are
shared-host observations, not paired medians or controlled speedup claims.
Raw selected runs, lower-loop regressions, commands and profile are at
`/tmp/rustred-provenance-certificate.gSddBU/`.

### Full physical-FG CLI rerun

The frozen CLI above was run against `examples/input/four_loop_fg.toml`,
natural ordering, nonpositive auxiliary indices 8 and 9, two workers on CPUs
32/33, nested pools capped at one, and a 600-second upper bound. The actual
process **exits 8 after 138.07 seconds**, not by timeout. It uses **176.18 s
CPU** (172.94 s user + 3.24 s system) and **298,780 KiB** peak RSS.

| Recorded boundary | Result |
| --- | --- |
| All sector searches finished | **39.9 s**, at the monitor's 0.1-second resolution |
| Search output | **124 sectors, 9,272 rules, 145 finite terminal candidates** |
| FG106 audit | **99/99**, zero issues/gaps, finished at **44.7 s** |
| Successful sector audits before FG214 | **32** |
| FG214 audit | Begins **46.9 s**, finishes **137.4 s**, **154/161** replayed and descending |
| Failure | Same seven zero-based guard-geometry ordinals **69–73, 100, 101**, plus incomplete exact cover |
| Durable output | **None**; remaining sector audits, lowering, installation and cold canary were not reached |

The search-output counts are unchanged from the preceding compact-support run.
That older run reached its **600.14-second** cap; this run completes the
formerly stalled audit and exposes its proof failures. These shared-host
runs are neither repeated nor hardware-isolated, and discovery itself is
unchanged by provenance composition. Do not present the different search
times, total wall times or peak RSS values as controlled speedup ratios.

The full CLI profile also records 724 CPU samples, none lost, but its
frame-pointer call stacks are largely unresolved. It does **not** support the
selected-driver percentages above as full-run attribution. The reported
99.68% replay / 84.61% sparse reduction / 47.53% GCD fractions belong solely
to `selected214.perf.data`; `fg.perf.data` is separate evidence with that
limitation. Raw full-run records are `fg.sh`, `fg-generate.stderr`,
`fg-generate.time` and `fg-perf-report.txt` in the same evidence directory.

The next complete validation must follow the generic geometry corrections
through all remaining sectors, combined installation, cold loading and
application. Finishing the former bottleneck does not discharge those
mathematical obligations or establish four-loop closure.

## Historical first slice: compact modular support

`ordinary/native/support.rs` uses native finite-field evaluation and GPLU
patterns to propose a compact physical source set with two deterministic
probes. It keeps original-source, accepted-U and native-L indices distinct,
retains full dependency ancestry, and bounds input/fill/pattern storage.
`native::propose_verified` lifts that support exactly, expands weights into
their original positions and requires full unprojected replay. Failure retries
the unchanged complete exact proposal within the same quotient.

`ordinary/guards.rs` admits no new index-pole face: it uses existing native
exception extraction and exact excluded cases, or proves a coordinate face
disjoint from every current application box. Unsupported coupled geometry
selects fallback. Index-dependent inherited source conditions disable pruning.
The original-source normalization multiplier is already proved polynomial, so
it cannot introduce a new denominator; the existing downstream conditions and
normalization gates remain unchanged.

The ordinary namespace passes 24 tests, including 17 new regressions. The
broader source-port namespace passes 121 tests, with five existing larger
workloads ignored in that debug gate. These protect sampled-zero/pole/rank
misses, source/L bookkeeping, false modular membership, exact lifting, full
replay, guard admission, affine cases and persistence. Debug correctness times
are not performance evidence. Logs are at
`/tmp/rustred-original-source-tests.VD8gMH/`.

A frozen pre-change selected-sector-214 release run finds 161 rules and eleven
finite residual candidates in 11.980 seconds, then reaches its 180.01-second
bound during audit (178.57 seconds CPU, 91,704 KiB peak RSS). This is a focused
baseline, not closure or a controlled cross-machine comparison. The updated
release driver repeats this workload and the full physical FG campaign below.
Evidence is at `/tmp/rustred-compact-certificate.LQpBJ4/`.

The first updated release measurements now confirm K1/K3/K6 generation,
fresh-process inspection and canary application. K6 still has 623 rules,
5,640 cells and 38 masters; its new ordinary-source certificates produce
8,923,663 bytes. One-, two- and six-worker artifacts agree exactly, and the dotted
canary output is byte-identical to the previous producer's output. Serial
generation took 3.32 seconds wall in this shared-host regression, not a
controlled performance comparison.

The selected FG106 boundary regression also passes all 99 replay/descent
checks with zero gaps or issues (1.02 seconds wall, 549 ms audit). FG214 still
reaches a 180.03-second bound: pruning alone has **not** resolved that sector.
The new driver links the app-feature release library, while the old selected
baseline linked the core-only library; concurrent host load also differs.
The observed solve times (31.274 versus 11.980 seconds) therefore must not be
presented as a controlled pruning speed comparison. Discovery is unchanged by
this patch.

The full updated physical-FG CLI run finishes all 124 sector searches by
**59.4 seconds**, again generating **9,272 rules and 145 finite residuals**.
It passes the same 32 sector audits and starts sector 214 at **79.5 seconds**,
then reaches the **600.14-second** bound without writing an artifact.
CPU time is **651.32 seconds** (645.68 user + 5.64 system), with
**443,624 KiB** peak RSS. This establishes that compact support alone is
insufficient on this workload; it does not establish a speed or memory ratio
against the earlier shared-host run.

The frozen CLI SHA256 is
`c045724de27170fcaaa29681cd61315f117b446d848ea56c59902f578fc80aa6`.
Its K6 artifact SHA256 at all three worker counts is
`315542ebe00b588892dc161ce92a37764a89ed36deac1c84bbd0c0f64df18e48`.
Commands, raw outputs, timings and profiles are retained at
`/tmp/rustred-compact-certificate.LQpBJ4/`. These are CLI release checks;
the Python extension was not rebuilt for this certificate-only slice.

In a 15-second sample of the updated full run, 724 CPU samples (none lost)
attribute 98.32% inclusive cost to native exact `SparseRowReducer::add_row`,
93.57% to `ordinary::native::propose_projected`, and 64.17% to polynomial GCD.
Again these are overlapping local stack fractions. The profile does not
distinguish compact versus fallback calls or identify the active rule ordinal.

The implemented monitoring slice adds a `CheckingRule` event before every rule's
geometry/replay check. It carries only sector, ordinal, count and elapsed time;
the CLI renders every rule start in its existing overwriting TTY field or
opt-in plain stderr log. Artifact bytes and publication authority are unchanged.
The core source-port gate passes 122 tests (five existing larger tests ignored),
17 focused application/monitor tests pass, and the CLI progress/stdout-byte
parity test passes. Independent review verifies caller-thread ordering,
incomplete-sector rejection and observer-panic behavior. These debug tests are
correctness checks, not a performance measurement. Evidence is retained at
`/tmp/rustred-checking-rule-app.c2BbvV/` and
`/tmp/rustred-precondition-provenance-tests.uscoaI/`.

## Structural motivation for retaining the derivation

An external diagnostic of the frozen pre-pruning solver identifies repeated
work in FG214. All 25 fully fixed rules, ordinals 136–160, retain the same
1,493-source numerical trace over 268 seeds: 4,288 regenerated original rows
per rule. They account for 107,200 of the sector's 125,184 potential original
row instances. This is a structural count, not per-rule timing attribution.
Rule 100 also has a large affine payload: 207 selected sources over 29 seeds
and 12,066 RHS coefficient monomials. Detailed data are at
`/tmp/rustred-fg214-rule-shapes.vnNEle/`.

Independent API/mathematical audit motivated the now-implemented follow-on: record
the existing polynomial preconditioner's exact forward operations once, then
compose that derivation with the selected frame's exact weights. Every
preconditioner operation is a native polynomial identity
`B_new = a B_left - b B_right`; no polynomial division is introduced by the
provenance trace itself. Specialized rank loss obstructs inversion, not this
forward identity. An optional immutable row-operation DAG can avoid solving
the second original-source membership problem, without constructing expanded
transitive combinations in the search path.

Seed translation, fixed-coordinate specialization, affine restriction and
canonical recentering must retain their current exact order and tangent checks.
Selected-frame weights can still have poles, so complete unprojected replay,
no-new-pole admission and the current certificate fallback remain mandatory.
Only original-row coefficients are persisted; cold loading trusts no DAG.
The existing normal preconditioner entry must allocate no trace arena.

The fully numeric shared trace also permits a future bounded frame-reuse slice if exact
ordered recipes, source owner, ordering, sector, zero census and specialization
context match. Persisted offsets remain target-relative. A general projected
matrix cannot be cached merely by source IDs, because its omission predicate
depends on the application domain. This cache is still a follow-on design,
not an implemented speedup or artifact authority. The provenance composition
above is implemented independently of that potential cache.

## Historical corrected full FG run before provenance composition

After conservative symbolic zero projection fixed the selected sector-106
replay obstruction, the full physical FG release campaign used the external
`examples/input/four_loop_fg.toml`, natural ordering, nonpositive coordinates
8 and 9, and two workers pinned to CPUs 34 and 35. Its native census contains
281 global zero proofs; the physical scope contains 132 zero and 124 nonzero
sectors.

All 124 searches finish by **57.1 s**, returning **9,272 candidate rules and
145 finite terminal candidates**. Exact checking passes 32 sectors, including
all 99 rules of sector 106. At **80.9 s** it begins sector **214**, which has
161 rules, and remains there until the **600 s** bound. The final process
measurements are **600.13 s wall, 651.21 s CPU** (647.10 user + 4.11 system),
**551,840 KiB peak RSS**, and exit status **124**. No artifact is published.

A live **15-second** profiling window while checking sector 214 records
**724 CPU-cycle samples**, with no reported lost samples:

| Inclusive sampled stack | Share |
| --- | ---: |
| Native `SparseRowReducer::add_row` | 97.90% |
| `source_port::ordinary::native::propose` | 95.67% |
| Native multivariate polynomial `gcd` | 64.90% |

These are nested inclusive percentages, not additive phase timings. They
identify the bottleneck in that window, not its share of the entire campaign.
This was a shared-host diagnostic, not an isolated benchmark. The monitor
does not identify which of sector 214's 161 rules was active; the profile
does not justify inventing that ordinal or its matrix dimensions.

Evidence: `/tmp/rustred-safe-projection.DMp3co/fg-generate.stderr`,
`fg-generate.time`, `fg-perf-children.txt`, `fg.perf.data`, and `fg.sh`.
The frozen CLI SHA256 is
`7cd40693f4ac25cb7cbcc43ee3fbbb42757e70acb5fbc61a13a6365aa5508e56`.

## Why the original complete proposal is expensive

`foundry/artifact/source_port/ordinary.rs::weights` regenerates every ordinary
IBP at each distinct seed in a candidate's retained source trace. For four
loops that is 16 ordinary rows per seed. It asks
`ordinary/native.rs::propose` to express the desired identity in this corpus.

The complete proposal reduces an augmented matrix `[A | I]`: physical
integral columns followed by a distinct identity column for **every** source
row, and a separate desired-row marker. Even physically dependent source rows
become independent in the augmented matrix. Exact elimination therefore keeps
processing them and propagates expanded rational source-combination tails.
This is a structural reason for coefficient/GCD swell; it is not evidence of
a slow or incorrect Symbolica GCD implementation.

This work is separate from modular discovery and selected-rule exact lifting.
Selecting `SemiNumerical` or `SparseTargetOnly` for those discovery phases does
not currently accelerate this independent original-source membership proposal.
The full unprojected replay in `native::verify` remains essential: the earlier
FG failure showed why success in an assumed-sector quotient is not authority.

## Design implemented by the first slice

Use a bounded modular support proposal before the existing exact proposal.
Do not change source generation, coefficient translations, mathematical
zero predicates, ordering, or rule payloads in this slice.

1. Build one exact projected corpus for the current proposal quotient, keeping
   the original unprojected rows and the exact source-index mapping intact.
   Preserve every physical column in the registry even when its coefficient
   happens to vanish at the chosen sample.
2. Evaluate coefficients with Symbolica's native finite-field conversion and
   `evaluate_with_coeff_map`. A zero denominator is an unlucky sample, not a
   zero term. Use a fixed, bounded prime/sample schedule for reproducibility.
3. Stream only the **physical** rows into native
   `SparseRowReducer<Zp64>` with `LuLMode::Pattern`. Keep a permanent zero
   sentinel so full physical rank does not suppress the next native L row.
   Track original-row IDs, accepted-U-row IDs and native-L-row IDs separately:
   dependent inputs append L patterns, while empty inputs append neither.
4. Append the desired physical row. Only if it reduces to zero modularly,
   inspect that row's native L dependencies and walk their accepted-row
   ancestors. This produces a candidate source subset, not a certificate.
   Handle an empty desired image explicitly; do not read a stale previous
   L row or conclude that an exactly nonzero desired row is zero.
5. Keep selected original rows in a deterministic order, initially their
   original order. Run the existing exact proposal on this compact subset.
   Expand the resulting weights back into the original request vector, with
   exact zero weights elsewhere.
6. Run **unchanged full unprojected weighted replay** against the full desired
   identity. Continue through original-row normalization, source conditions,
   certificate-pole extraction, exact exceptional-domain coverage, descent,
   and the existing publication/load pipeline.

This needs only structural sparse bookkeeping around native algebra. Existing
code already supplies the critical patterns:

- `solver/discovery.rs::Discovery` tracks accepted U rows versus native L
  input rows and traces dependencies. Its target-pivot-specific API must not
  be abused by inventing a synthetic physical integral. A narrow shared CSR
  dependency helper is preferable to another discovery engine.
- `foundry/completion/spired/modular/kernel.rs::capture_dependent_dependencies`
  handles the dependent-final-row case, including L-row-count assertions and
  the zero sentinel. The new membership trace needs the complete ancestor
  closure, not a target-pivot shortcut imported without proof.
- `solver/search.rs::Probe::evaluate` already uses native numerator and
  denominator evaluation and rejects sampled poles.
- `solver/discovery/variables.rs::FrameVariables` validates coefficient maps
  and performs native context compaction/restoration. It can be narrowly
  factored if needed, without introducing a new coefficient representation.

### Deterministic fallback and guard risks

A failed modular rank test, an unlucky sample, or a compact exact frame that
does not lift is **inconclusive**. Retry within a fixed budget, then use the
existing complete exact proposal. The strict quotient and the speculative
assumed-parent-sign quotient must remain distinct.

Crucially, failure of a pruned proposal's unprojected replay must permit
fallback within the **same quotient** before rejecting the rule or moving
to the other quotient. Different span solutions can behave differently on
coefficient-zero or activation faces. Projection of an individually zero
source product does not authorize multiplying it by an arbitrary pole and
discarding the resulting product; full weighted replay checks precisely this.

A different valid certificate can also introduce new denominator poles even
when its final RHS is unchanged. Those poles must take the existing exact
guard/exception path. They may not be silently dropped, sampled away, or
treated as covered because the old certificate had no such pole. If the new
certificate creates unsupported or uncovered exceptional faces, try a
deterministically bounded alternative or the old unpruned certificate; do not
turn an old success into a false closure or hide missing faces. Equally, do
not misreport one failed certificate as proof that the candidate identity is
invalid.

## Follow-on if compact exact proposals remain expensive

### Recover only one exact source-weight vector

After modularly selecting an independent physical source subset, exact GPLU
can use `LuLMode::Full` on those physical rows and a desired-only marker.
The source rows have marker zero; the desired row has marker one. If exact
membership holds, the desired row reduces to that marker alone. Recover its
source weights by solving `L^T w = e_last`, rather than propagating an identity
tail for every row throughout physical elimination. The desired component
must be checked and normalized; the source weights have the opposite sign.

`solver/discovery/target_only.rs::solve_transposed_lower` already implements
this structural transpose plus native sparse normalization/back-substitution
with square-L, diagonal and pivot checks. Factor/reuse that narrowly rather
than implement another solver. Its current materializer returns a physical
target row, not an original-generator certificate, so the surrounding
membership contract still needs explicit integration and tests.

Dependent exact prefixes, changed pivots, or a surviving physical remainder
must fail the compact proposal. Native GPLU/triangular solving and full replay
remain responsible for all algebra. No speedup is claimed before measurement.

### Native semi-numerical reconstruction of certificate weights

The pinned local Symbolica 3 API now provides
`poly::reconstruction::reconstruct_rational_function_over_q`, taking a
finite-field black box, native variable map, `ReconstructionMethod`,
`ReconstructionOptions`, and `max_primes`. It owns interpolation, CRT,
rational lifting and its cross-prime checks. RustRed must not reimplement
these algorithms.

The existing `solver/discovery/semi_numerical.rs` demonstrates the reusable
integration: cache the complete native GPLU output vector for each
`(prime, point)` while reconstructing scalar coefficients, and restore the
original variable map. For this bottleneck, the reconstructed quantities must
be **original-source weights**, not merely the candidate's physical RHS.
Freeze the compact row support, column ordering and generic pivot contract
across samples; return `None` for poles or unlucky rank/pivot changes rather
than interpolate different underdetermined solutions at different points.

Reconstruct every needed weight, including ones zero at the first probe;
a sampled zero is not exact support authority. Native reconstruction's final
cross-prime acceptance is probabilistic, so exact unprojected replay remains
the decisive authority. Preserve every reconstructed denominator condition.
Budget the shared cache and total work: native `max_probes` is per prime
image, and separate scalar coefficients each have their own reconstruction
budget. It is not automatically an aggregate certificate-work limit.

## Native API audit and acceptance for the next slice

The audit checked workspace resolution (`symbolica = 3.0.0`, local vendor
patch), public implementation, and existing RustRed consumers:

- `vendor/symbolica/lib/numerica/src/tensors/sparse.rs`: `SparseRowReducer`,
  `LuLMode`, `add_row`, `u`, `l`, `pivots`, `back_substitute`, native sparse
  multiplication, and `SparseMatrix::solve`. L patterns and solver operations
  are already present; no custom elimination is needed.
- `vendor/symbolica/src/poly/reconstruction.rs` and
  `poly/reconstruction/rational.rs`: deterministic options, native Q-valued
  reconstruction, budgets, retries and the stated probabilistic final check.
- Existing discovery, target-only and semi-numerical consumers listed above.

Required regressions include unlucky samples/primes, sampled-zero desired
rows, empty/dependent-input L bookkeeping, source-index remapping, repeated
integral columns, full-replay rejection of quotient-only relations, poles on
activation faces, same-quotient fallback, unchanged fixed/affine restrictions,
and deterministic results across worker counts. Keep the sector-106 boundary
reproducer as an adversarial gate. Compare compact versus full exact proposals
by unprojected equality and complete guard coverage, not by assuming identical
certificate weights.

Instrument proposal input rows/columns, modular rank, retained support, L/U
nonzeros, probe attempts, fallback reason, exact-proposal time, full-replay
time and new guard count. First run a bounded sector-214 diagnostic, then
K1/K3/K6 regressions, and then the full physical FG publication/cold-reload
attempt. Do not infer a whole-campaign speedup from a faster modular kernel.
