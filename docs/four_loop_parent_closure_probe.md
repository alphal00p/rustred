# Four-loop external-parent closure probe

This is a bounded runtime probe of the three external four-loop unit-mass
parents that are not the H input. It deliberately uses the unchanged generic
`family-close` CLI, with no topology dispatch, FORM rules, ordering hints, or
sector selection.

## Protocol

- RustRed release executable frozen from revision `a435599`:
  `fba965172949c7c94caa6f68954310b7ed2d3ccae279cdbc2d73c85797c59260`.
- Inputs: `examples/input/four_loop_x.toml`,
  `examples/input/four_loop_bmw.toml`, and `examples/input/four_loop_fg.toml`.
- Natural coordinate ordering; no `--permutation`.
- Two RustRed workers pinned to physical CPUs 32 and 33.
- Fresh process and fresh output path per parent.
- Five-minute wall-time censor per process; `RAYON_NUM_THREADS=2` and all
  nested BLAS/OpenMP pools capped at one.
- A durable artifact had to be written before any cold inspection or physical /
  dotted reduction canary could run. None of these probes reached that stage.

The Python driver and complete stdout/stderr/measurement records are retained
outside the repository at:
`/tmp/rustred-four-loop-parent-probes.u9dfmK/`.

## Results

| parent | wall time | CPU time | peak RSS | exit/status | artifact | diagnostic |
|---|---:|---:|---:|---|---|---|
| X | 300.117876 s | 596.531212 s | 1,210,032 KiB | `-9`, timeout-censored | none | no sector diagnostic before censor |
| BMW | 300.082759 s | 596.357321 s | 845,304 KiB | `-9`, timeout-censored | none | no sector diagnostic before censor |
| FG | 233.533606 s | 462.959997 s | 1,087,096 KiB | `8`, typed exact-case failure | none | sector 397 retains unsupported nonlinear equalities |

The X and BMW processes were killed only after the five-minute bound. Their
absence of an artifact is therefore an incomplete run, not a mathematical
claim that their families cannot close. FG completed its bounded search far
enough to return a typed unsupported-case error, but also did not produce a
publishable artifact.

## Current-head FG rerun

After the exact disjunctive nonlinear-case engine landed, FG was rerun from the
current release binary rather than the historical `a435599` executable. The
protocol was otherwise unchanged: fresh process, two pinned workers on CPUs 32
and 33, nested pools capped at one, and a 300-second wall bound. It ran for
`235.31 s` wall (`465.39 s` user CPU, `1,092,112 KiB` peak RSS), then returned
status `8` at sector `397` with:
`incomplete exact case intersection: a branch retains unsupported nonlinear
equalities`. No artifact was written. Thus the newer engine removes neither
the need for an authenticated affine/nonlinear coverage partition nor the
fail-closed publication gate; it only advances the point at which this
particular parent is diagnosed.

## Current-head H rerun

The current release binary was also run on the generic external H family with
the same two-worker, CPU-pinned, fresh-process protocol. It reached the source
port publication gate in `101.66 s` wall (`199.58 s` user CPU, `660,572 KiB`
peak RSS), with `60/63` rules replayed and descending. The run was rejected
fail-closed because seven coordinate boxes remained uncovered and three affine
candidate rules could not be omitted by the rectangular cover. The repeated
coupled equality on the affected face is
`-1 - n2 + 2*n0 = 0` (with the corresponding fixed coordinates and sector
signs included in the diagnostic). No artifact was written. This is a useful
four-loop milestone: H now reaches the exact affine-ownership boundary rather
than failing in source search, but persistence and authenticated coverage of
that affine locus are still required before publication.

### H rerun after exact-empty affine pruning

The exact-empty fast path was then enabled for affine target and exceptional
branches and the same release campaign was repeated. It completed in
`101.16 s` wall (`198.88 s` user CPU, `658,384 KiB` peak RSS), but the
publication result was unchanged: `60/63` rules replayed and descended, seven
uncovered boxes remained, and three nonempty affine ownership branches could
not be discarded. The affected coupled branch is the genuine locus
`-1 - n2 + 2*n0 = 0` in the original index-variable naming (with the fixed
face coordinates and sector signs applied); it is not an empty contradiction.
Thus pruning proved-empty branches removes no part of the H obstruction. The
next sound step is to persist and authenticate this exact affine carrier and
to compile a coverage partition that treats its rectangular face only as a
runtime prefilter, never as the coverage certificate itself.

### H rerun after affine runtime/persistence plumbing

After the exact affine carrier was threaded through replay and the source-port
codec (commits `60164de` and `2f33965`), a fresh release run was performed from
the same generic H input. It still did not publish an artifact, but the failure
mode moved further into the authority pipeline: `60/63` rules replayed and
descended, with seven uncovered rectangular pieces. Two retained candidates
(rules 42 and 48) were rejected because full original-source replay still had
an unproved residual product, while rule 41 exposed one nonempty exceptional
affine branch. Its exact fixed face is
`[None,None,None,None,Some(0),Some(1),Some(1),Some(0),Some(1),Some(1)]`
and its coupled equation is `-1 - n2 + 2*n0 = 0` in the original index map.
This run therefore identifies two independent requirements: repair the exact
source-product replay for the two candidates, and compile the affine
exceptional branch into a certified DNF owner partition. The affine payload
codec alone is not a closure proof and remains deliberately rejected by the
installer.

### H reruns after exact-chart ordinary replay

The ordinary replay path was then changed to restrict coefficients with the
authenticated affine chart when a rule itself is affine.  This is an exact
Symbolica-owned operation; it does not sample the chart or rectangularize it.
The natural ordering was rerun in a fresh release process (two workers pinned
to CPUs 32 and 33).  It took **100 s** and reached the same `60/63` boundary:
two rules still had generic residual products and one rule retained a
nonempty affine ownership branch.  Thus the two residuals are not fixed by
affine replay alone; they are ordinary source-projection obligations.

The reverse ordering `9,8,7,6,5,4,3,2,1,0` was also rerun from the same
release binary.  It reached a later sector in **148 s**, with **68/70** rules
replayed and descending and **zero uncovered coordinate boxes**.  Its sole
remaining issue was the exact residual

```text
(-n2)/(-2+n4-n2+d) I(n0,0,n2+1,1,n4,0,1,1,0,1)
```

on sector `[false,false,false,true,false,false,true,true,true,true]`.  The
numerator is not identically zero on that sector, so this is not an affine
exceptional ray and must not be discharged by Janet/Ore or by adding a guard
`n2=0`.  The current interpretation is a source-seed/translated-zero-census
projection mismatch (or an invalid fallback candidate), to be resolved by
provenance instrumentation and strict-vs-fallback proposal comparison.

A nearby ordering `9,8,7,6,5,4,3,2,0,1` reached **73/74** replayed and
descending with zero uncovered boxes in **154 s**, but stopped on one
unsupported nonlinear exceptional case.  These ordering results confirm that
ordering is a useful bounded search dimension, not evidence of four-loop
closure; no run produced a durable artifact.

### H reverse rerun after disjoint affine accounting

The source-port counters were corrected after the exact-H SpIRed differential
check exposed an accounting overlap: an affine candidate that replayed but
failed the rectangular descent proof was counted both as `exact_replayed` and
as redundant, while a sector-empty affine candidate was silently dropped.
Executable and redundant candidates are now disjoint; a proved-empty affine
domain is counted as a vacuous redundant candidate without being treated as an
unresolved affine cover.

The release reverse-order campaign was rerun with this correction. It passed
the former failing sector (`68` executable rules plus `2` redundant affine
candidates out of `70`) and advanced to sector
`0010011001` (`[false,false,true,false,false,true,true,false,false,true]`) in
`272 s` wall time under the same two-worker pinned protocol. That sector has
`91/94` replayed and descending rules, one uncovered coordinate box, and three
nonempty affine candidates. Their common feasible exceptional equation is
`1 + n4 - 2*n7 = 0`; one branch additionally fixes `n8=0`. These are genuine
infinite affine loci (`n7=-t`, `n4=-1-2t`, `n8=-u`, with `t,u >= 0`), not
proved-empty domains. The current artifact bridge therefore rejects them
fail-closed, as it must. The remaining coordinate descent issue is likewise
reported rather than sampled away.

The matched natural-order single-sector run remains an independent oracle
check: RustRed and the exact-H SpIRed C++ output each produce `56` rules, and
Symbolica validates all `56` domains, guards, sector signs, and coefficients.
The reverse run is consequently a stress test for affine ownership and replay,
not a source of imported rules.

### Exact predicate carrier checkpoint

The source-port geometry now separates a rectangular prefilter from exact
affine exceptional predicates. The first regression uses
`1+n0-2*n1=0` in the nonpositive quadrant: `(-1,0)` and `(-3,-1)` are on the
excluded infinite ray, while `(0,0)` is not. The old box-only entry point still
rejects that partition, so the prefilter cannot accidentally become coverage.
Immutable exclusion metadata can pass through the checked-rule/lowering
carrier and is tested on original powers by runtime cells. Artifact
installation still rejects populated exclusions pending the exact predicate
cover proof and its durable encoding. This is an ownership-plumbing slice,
not a new four-loop closure result.

### Affine replay/descent grounding (2026-09-16)

A dedicated release test now regenerates the `0010011001` H sector from
ordinary family input, with reverse coordinate ordering and the independently
computed zero-sector census. The first run generated **94 rules in 1.058 s**;
generation plus conditional source replay/descent took **19.546 s**. These are
single observations, not a controlled benchmark. Compilation is a separate
8m17s build, not part of the solver measurement.

The exact integer-boundary correction removed rule 67's descent obstruction:
on `n4=0`, `1+n4-2*n7=0` would require `2*n7=1`, which has no integer solution.
The new check uses Symbolica integer gcd/divisibility and interval arithmetic
on the defining equations, not samples or a trusted cached matrix. Its
negative control retains the boundary when an integer solution exists.

That run passed replay/descent for **93/94** rules. Rule 54 failed original
source replay on a term containing `I(0,0,1,-1,n4,1,2,n7,n8+1,0)`.
Its stored exceptions include `n8=0`, but the exceptional case is represented
as the conjunction of the parent's coupled equation and `n8=0`. The previous
prefilter left this excluded boundary present because the whole child was
classified as affine. The fix proves, with the parent's native chart, that
the child's coupled equations are already implied by the target; only then
is the child's fixed coordinate face subtracted from the replay prefilter.
Genuinely new coupled constraints remain exact predicate exclusions.

A second release run after that correction generated **94 rules in 0.927 s**
and took **21.825 s** through replay/descent (9m01s compilation separate).
It still passed **93/94** rules: the residual moved to
`I(0,0,1,-1,n4,1,1,n7+1,n8+1,0)` in rule 54. The original offending term
was removed, but this exposed another conservative geometry gap. On the
affine target, excluding `n7=0` fixes both `n7=0` and `n4=-1`. The rectangular
complement still contains `n7=0,n4!=-1`, although that entire slice violates
`1+n4-2*n7=0`. The per-sign-cell integer contradiction check already used by
descent must therefore also be used by original-source zero-product replay.
No sector-coverage certificate was reached in this run, and no artifact was
written.

A separate bounded Boolean/box coverage checker now tests affine targets and
exception conjunctions without conflating their boxes with ownership.
Missing children, cycles, mismatched predicates, nonfinite terminals and
budget exhaustion fail closed. Integer-impossible true branches may be
discarded only by recomputing exact contradiction evidence. This checker and
the conditional grounding test are **not** a published artifact: durable
lowering, guard validation, installation and cold reloading remain separate
required gates.

The shared affine restriction service now rebuilds the native Symbolica
RREF from defining equations for cold replay. It preserves rational scale
and rejects zero denominators before cancellation. Both target and exclusion
predicates must use the original generator's physical index map; an internally
consistent but permuted chart is not sufficient. Independent review prompted
an explicit adversarial regression for this distinction.

The next guard-proof slice can stay narrow. For the observed H denominator
`(n8-1)*(1+n4-2*n7)`, the first factor cannot vanish when `n8<=0`, and the
second describes an explicitly excluded affine branch. After optional target
restriction and native base-parameter coefficient splitting, each factor of
one nonzero coefficient equation must either miss the box or imply a complete
excluded predicate. Symbolica's existing `Factorize` and `try_div` supply the
algebra. The required implication is **factor-zero implies exclusion**, not
the reverse; every excluded equation and its fixed-coordinate face must hold.
This is the next implementation design, not an already-enabled admission path.

After adding that per-sign-cell contradiction test to zero-product replay,
the next release run passes **all 94 original-source replays**, with 93 rules
also passing descent. Generation took **0.937 s**; the complete conditional
replay/descent pass took **21.023 s** (8m38s compilation separate). Rule 54 now
reaches descent and is rejected at RHS term 24, whose shift raises `n7` by two.
The flagged sign cell has `n4<=-2`, `n7` in `{-1,0}`, and `n8<=-1`.
On the required affine equation, `n7=0` is impossible there, while `n7=-1`
forces `n4=-3`. The actual numerator of that term factors as
`3*(n7+1)*(4+3*n8+6*n7-3*d)`, so it vanishes on the feasible slice; its
denominator remains nonzero as a generic polynomial in `d` there. A whole-box
coefficient test must not conflate these two integer slices. Independent
inspection confirms that the current finite-axis traversal does not recheck
affine emptiness at each leaf. The next correction is therefore affine-aware
finite-leaf coefficient vanishing, not an inferred missing IBP. The
sector-coverage assertion still is not reached, and artifact publication
remains disabled for indispensable affine owners.

Validation for this checkpoint includes 82 focused source-port and 107 affine
debug tests (overlapping selections, no failures), independent implementation
and mathematical audits, and a successful release K6 durable-artifact/reducer
regression. The earlier release snapshot also passed all 1,776 nonignored
library tests, with 31 diagnostics ignored. None of those ignored diagnostics
is counted as a four-loop closure success.

### Predicate-aware lowering milestone (2026-09-16)

The finite-leaf correction now preserves the original affine equations while
enumerating only genuinely finite integer axes. Each leaf is checked for exact
emptiness before its coefficient is required to vanish; feasible leaves still
retain the original denominator check. No infinite direction is sampled.

The release `four_loop_affine_sector_grounding` rerun now passes:

| Gate | Result |
| --- | --- |
| Generated rules | 94 in 1.205 s |
| Original-source replay and strict descent | 94/94, reached at 22.329 s |
| Exact predicate cover | success: 1 predicate, 100 clauses, 3 Boolean nodes |
| Lowering through the cold-cell verifier | 1,202 rule cells |
| Whole diagnostic | 44.44 s wall, 43.87 s user + 0.12 s system, 30,784 KiB peak RSS |

The diagnostic deliberately runs the original replay/descent pass and then
the production `check_sector` pass again before lowering; its total is not
the timing of a single solver pass or a complete family artifact. It was
pinned to CPU 34 with one Rayon/OpenMP/BLAS thread. This is one shared-host
release observation, not a repeated controlled benchmark. Compilation took
7m40s and is outside the timed test process.

Guard checking now uses Symbolica's native factorization and exact polynomial
division. A factor's zero set must either miss the actual affine domain or
imply every equation and fixed coordinate of an excluded predicate. The
application path retains these predicates on original integral powers.
The source-port plan tag is bumped to `0x703`, including exact exclusions;
old layouts are rejected, not migrated. Cold loading regenerates source
identities and validates cached affine charts once per parent. Installation
checks the exact predicate cover, never only its rectangular prefilter.

The K1 and K3 release durable tests pass (0.11 s together), and the complete
K6 generation/cold-load/application regression passes in 5.77 s: 623 rules,
5,639 cells, 38 typed terminals, 26 zero sectors and 8,916,745 encoded bytes.
Final-source debug selections pass 95 source-port, 117 affine and 10 codec
tests (overlapping filters, not a unique combined count). Independent
implementation and mathematical audits found no blocker. The release test
snapshot predates the final excluded-chart rejection hardening and diagnostic
omission-accounting fix; those additions are included in the final debug gates
and the ensuing CLI build. They do not change the successful H derivation.

The earlier debug-only H probe reached its explicit 120-second cap; that
inconclusive attempt is not counted as a pass or a release timing. The release
run above establishes exact coverage and executable lowering for **one H
sector**, not a whole-family cold-loadable artifact. The next run uses the
generic `family-close` CLI with the external literal-unit H input and reverse
coordinate order.

### Full H rerun after predicate-aware lowering

The subsequent whole-family CLI run used the released `da70bf4` implementation,
the unchanged literal-unit H input and reverse coordinate order
`9,8,7,6,5,4,3,2,1,0`. It used two workers on physical CPUs 32/33, with nested
BLAS/OpenMP pools limited to one, and a fresh output path. The executable
SHA-256 was
`58aceec22284bd68e824deb9a91a5be5e6934d0aa1b24b96697c0083b0171021`.

The run reached its explicit 600-second limit: **600.16 s wall, 843.97 s CPU
(840.81 user + 3.16 system), 849,296 KiB peak RSS**, exit 124. It produced
neither an artifact nor an error diagnostic. The old CLI exposed phase timings
only after successful completion, so this observation does **not** locate the
remaining cost in discovery, replay, lowering, or final coverage. CPU utilization
alone cannot identify the phase. No licensing failure was reported. Compilation
preceded the timed process and took 9m29s.

The complete invocation and measurement records are retained at
`/tmp/rustred-h-predicate-close.qBzEqd/`. The next implementation slice adds
non-authoritative live generation and installation progress, preserving the
exact proof path and artifact bytes. The next full run must use these events
to identify the actual bottleneck instead of interpreting this timeout as a
mathematical failure or a successful closure.

The progress implementation now has independent code/mathematical audits and
114 focused core passes (five diagnostic tests remain ignored), plus twelve
app/unit and four CLI passes. The checks include an actual affine owner and
its complementary excluded domain surviving cold artifact loading, exact
artifact-byte parity with/without observers, serial/parallel K3 parity,
fail-closed incomplete installation, and binary stdout remaining unchanged
under `--progress`. Private observer helpers erase callback types to avoid
duplicating the proof pipeline. Quiet redirected CLI runs bypass renderer
locking and app-event construction. A fresh release run is the next timing
gate; these debug checks are not performance measurements.

### Full H rerun with observed checking boundaries

The next run froze the release executable from `2df001df` into an external
evidence directory before any subsequent source edits. Its SHA-256 is
`1fcc6eb1163acbc487b5afc34db93e1c155aaf650c0920fac2ffa73b33e8d7e9`.
The input, reverse priority, two workers and physical CPUs 32/33 were unchanged;
the command added `--progress` and used a **1200-second** wall bound. This is
still unrestricted closure of all 1024 coordinate masks, not a physical-root
scope. Compilation took 5m58s and preceded all timed runs.

The process ended at its bound with **1200.24 s wall, 1352.60 s CPU
(1344.59 user + 8.01 system), 1,012,324 KiB peak RSS**, exit 124. No artifact
was written, and no cold inspection or H reduction canary could run. There
was no mathematical failure or licensing error diagnostic before the censor.
The dedicated process and its timeout parent both exited; it was not restarted.
The host also carried development compilation, including an unpinned release
build during late H checking. Thus these are diagnostic, timeout-censored
observations on a shared host, not isolated performance benchmark numbers.

Unlike the previous silent run, the phase log identifies what happened:

| Boundary | Observed result |
| --- | --- |
| Native census | 728 nonzero sectors plus 296 proved-zero sectors |
| All independent sector searches finished | 164.6 s from application entry |
| Search output, before artifact audit | 41,028 candidate rules and 893 finite residual entries |
| Completed exact sector audits | 47 sectors, reporting 2,939 exact-replayed rules |
| First 47 audits | 164.6–273.3 s, about 108.7 s |
| Next audit: mask `0010011011`, numeric mask 868 | Started at 273.3 s and remained active until the bound, over 926 s |
| Lowering, final installation and encoding | Not reached |

Sector 868 had already produced **128 candidate rules and 11 finite residuals**
at 20.7 s during parallel search. Its unresolved runtime cost is therefore
inside `SourcePortAudit::check_sector`, not ongoing modular discovery. The
observer does not expose how many individual rules in that sector had finished
checking. Nor do finite residual counts themselves authenticate master terminals.

The former affine obstruction, mask `0010011001` (numeric mask 612), now passes
the actual whole-family checking path: all 94 rules, zero uncovered pieces,
zero issues, in 17.6 s (197.5–215.1 s). Mask `0010011010` (356) then passes its
68-rule check in 58.2 s. This advances the mathematical integration beyond the
earlier affine failures while leaving complete H publication unresolved.
Sector 868 activates auxiliary coordinate `D10` (zero-based index 9), so it
lies outside an explicitly requested physical H scope with `n9<=0`. Avoiding
that sector through a persisted physical-domain contract changes the declared
closure task; it is not a proof of unrestricted closure or an optimized solver.
Sector 356, by contrast, is inside that physical scope and still has a measured
58.2-second audit cost.

Read-only thread observations during the long check show the main thread
running while both existing sector workers wait on futexes. No user-space
sampling profiler was available, and the process was not interrupted to obtain
a stack. The event spans several exact operations, so it would be unjustified
to attribute the long check specifically to polynomial gcd, factorization, or
any other Symbolica routine from these measurements alone. The historical
600-second run remains unprofiled; these observations belong to this new run.

The next profiling boundary should separate:

1. sector-basis preparation;
2. each rule's selected-frame exact GPLU;
3. independent original ordinary-source membership GPLU/fallback;
4. exact identity replay;
5. guard and application-domain reconstruction;
6. uniform descent;
7. stored and checked predicate covers.

Repeated application-partition/affine-owner construction and repeated predicate
constraints are static exact-reuse candidates, not measured dominant costs.
Any optimization must retain the original-source, guard, descent and coverage
proofs; the candidate generator's success cannot substitute for those gates.

The same frozen executable first passed generic external-input K1 and K3
generation, fresh-process inspection and dotted canaries. Generation took
0.13 s and 0.04 s respectively in single smoke observations. These are not
comparative benchmark medians. All protocols, the frozen binary, inputs,
phase logs and external measurements are in
`/tmp/rustred-four-loop-progress.AKMOli/`.

### FG's exact exceptional condition

The earlier `sector 397` error identifies an ordinal in the sorted nonzero
sector list, **not** a bitmask. A selected-sector release diagnostic now maps
it to physical-coordinate mask `1001110010` and captures its exact fixed face:

```text
[1, n1, 0, 1, 1, 1, n6, n7, 1, 0], with n1,n6,n7 <= 0
g = 1 + 2*n7 + n7^2 + n6^2 + n1 + n1*n7 - 3*n1*n6 + 2*n1^2 = 0
```

The whole family has 743 nonzero and 281 proved-zero sectors. Isolating this
sector takes 0.20 s wall (84 ms preparation, 106 ms solving), **not** a whole
family closing time. Native Symbolica factorization retains one irreducible
quadratic factor. This is not a coupled affine constraint missed by the affine
carrier, and increasing its factorization budget does not make it affine.
The captured typed payload and native factorization are retained at
`/tmp/rustred-fg-sector-diagnostic.6o3yj9/`.

Reversing coordinate order changes the quadratic but still fails safely on
this same mask (193 ms solving). Native Symbolica solving returns conditional
radical branches, not unconditional affine charts. The nonlinear locus has
infinitely many admissible integer points, so it cannot be discarded. The
mask also activates FG's auxiliary `D9` as a propagator, outside its physical
root domain. Any smaller closure contract must be explicit and persisted,
with exact sector coverage and RHS containment; it cannot silently filter the
current unrestricted command. See
[the detailed diagnosis](research/four_loop_fg_exceptional_geometry.md).

A subsequent explicitly scoped physical-FG search (`n8,n9 <= 0`) completes
all **124 nonzero sectors**, with 132 native proved-zero sectors, in **33.47 s
wall** (65.91 s user + 0.39 s system, 114,080 KiB peak RSS, two workers). It
returns 9,264 proposed rules and 145 finite residuals. No original-source
audit, descent/coverage installation, cold loading or artifact publication was
performed by this diagnostic. Thus it motivates an explicit root-domain
contract; it does not establish either a physical FG artifact or unrestricted
K10 closure. The reverse-order polynomial above also has only one admissible
integer zero, unlike the natural-order infinite locus; the detailed report
distinguishes these cases and records native solver results.

### First physical-FG artifact attempt with an explicit root contract

The generic persisted root-domain implementation was then exercised through
the full release CLI, not just the search-only driver:

```text
rustred family-close --input examples/input/four_loop_fg.toml --input-format toml \
  --nonpositive-indices 8,9 --n-cores 2 --progress --output <fresh-path>/fg.rr
```

This used natural coordinate order, two workers on physical CPUs 34/35,
nested pools capped at one, and a 1200-second bound. The release executable
was frozen after the explicit-scope development slice following `2df001df`;
its SHA-256 is
`b01f0c4c8e96d41e1deaf53b34aed36fdae9672f725d824a6e5e402fe988b64b`.
Compilation took 7m55s and ended before the timed process. Lower-loop tests
and Python binding installation could run concurrently on the shared host.

| Boundary | Observed result |
| --- | --- |
| Declared domain | `n8,n9<=0`, all 256 masks in that downset |
| Native census | 124 nonzero and 132 zero masks in scope; all 281 global zero proofs retained |
| All sector searches completed | 32.9 s from application entry |
| Search output | 9,264 candidate rules and 145 finite residual entries |
| Successful exact sector audits | 20 |
| First failing audit | Numeric mask 106, physical-coordinate mask `0101011000` |
| Failure summary | 93/96 replayed and descending, zero uncovered boxes, three issues |
| Whole process | 50.11 s wall, 81.61 s CPU (79.94 user + 1.67 system), 196,640 KiB peak RSS |
| Exit / artifact | Exit 8, not timeout; no artifact written |

At 49.9 s, full original-source replay rejected these residual products:

```text
rule 85: (-1/n8) I(n0+1,1,-1,2,0,0,1,0,n8,0)
rule 86: (-1/n8) I(n0+1,1,-1,2,0,0,1,-1,n8,0)
rule 87: (-1/n8) I(n0+1,1,-2,2,0,0,1,0,n8,0)
```

This is a physical-sector original-source proof gap, not the earlier nonlinear
exception on a positive auxiliary propagator. Discovery has finished; no
lowering, final installation, cold inspection or reduction canary was reached.
The zero-uncovered-box count does not validate the three rejected identities.

Read-only inspection points to symbolic boundary activation in discovery's
zero-sector projection. The displayed products lie in rank-deficient mask 74
when `n0<=-1`; at `n0=0`, `n0+1` becomes positive and activates full-rank mask 75.
The coefficient `-1/n8` does not vanish on that face for negative `n8`.
The current discovery helper inherits a parent sign for symbolic powers,
whereas exact replay partitions their actual signs. This is a concrete next
diagnostic target, not yet a complete provenance diagnosis: the three original
candidate domains and source traces must be inspected before changing search
or ownership. It is not sound to omit the rules merely because the other
reported boxes cover the sector.

The frozen binary, hashes, invocation, full logs and measurements are retained
at `/tmp/rustred-four-loop-fg-scope.4ueeaV/`. This failure localizes the next
correctness investigation before further broad profiling or full-family reruns.

#### Selected-sector confirmation of the admitted boundary

A release diagnostic against the frozen `09cef8e3` library then repeated
sector 106 alone, with the same native global-zero census and solver options.
It reproduces all 96 candidate rules and the exact three replay failures in
**0.82 s wall, 0.80 s CPU, 9,216 KiB peak RSS** (218 ms search, 490 ms audit).
The driver exits successfully after printing an incomplete audit; this is
diagnostic execution success, not a closing artifact.

The actual cases remove the remaining domain ambiguity:

| Rule | Canonical target | Only excluded face |
| --- | --- | --- |
| 85 | `I(n0,1,0,1,0,1,1,0,n8,0)` | `n8=0` |
| 86 | `I(n0,1,0,1,0,1,1,-1,n8,0)` | `n8=0` |
| 87 | `I(n0,1,-1,1,0,1,1,0,n8,0)` | `n8=0` |

Only `n0,n8` are symbolic; the rest are fixed. All three rules therefore admit
`n0=0,n8=-1`, as independently checked by substituting the complete boundary
point into their exceptional equations with Symbolica. There are no affine
conditions or hidden `n0` exclusions. The native census directly confirms
mask 74 is proved zero and mask 75 is not.

Each candidate has a 22-source dependency trace. Full source seeds, basis
rows, ordinary rows, RHS coefficients and exceptional cases are retained at
`/tmp/rustred-fg106-trace.O1QCHP/`. This localizes the obligation to a boundary-
unsafe source certificate/discovery projection, not a missing CLI display of
an excluded face. A nonzero residual invalidates that replay proposal; it
does not alone prove the final candidate identity false, since a different
complete original-source certificate may exist. The correction must preserve
the exact replay gate and be tested against this small reproducer before a
new full physical-family run.

A supplemental temporary reconstruction uses Symbolica's native sparse
reducer and the frozen original-source proposal helper to expose rule 85's
weights. It exactly reproduces the stored projected row from a raw pivot
`I(n0-1,1,0,1,0,1,1,0,n8,0)` with a **+1 translation of `n0`**. The strict
whole-domain proposal fails; the assumed-parent-sign fallback succeeds.
Its 64 original rows have 29 nonzero weights, all `±1/n8` or `±2/n8`, so none
has an `n0=0` pole. Native full replay rejects the same residual product.
This rules out a merely delayed `n0` weight-pole exception for this proposal
and records the recentering that can reactivate a prematurely projected
column. The supplemental diagnostic takes 0.83 s wall; it neither publishes
an artifact nor proves that no alternative valid certificate exists.

#### Conservative projection passes the selected-sector proof gate

The next release build retains a symbolic source column unless the native
zero census proves it zero for every assignment of its symbolic sector signs.
This is deliberately conservative and invariant under later canonical shifts;
it does not implement new algebra or weaken original-source replay.

The selected sector 106 rerun now passes **99/99 original-source replays and
99/99 uniform-descent checks**, with **zero issues, zero stored/checked
uncovered or unbounded boxes, and one finite terminal candidate**. The old
snapshot produced 96 rules, of which 93 passed and three failed. No additional
replay guard branches were needed in the new report.

The retention has a measured cost in these small single-run diagnostics:
search rises from **218 to 557 ms**, exact audit from **490 to 1,002 ms**,
and whole-process wall time from **0.82 to 1.68 s**. The corrected process
uses **1.66 s CPU** (1.61 user + 0.05 system) and **12,288 KiB peak RSS**,
versus 0.80 s CPU and 9,216 KiB before. These are shared-host observations
with changed rule workloads, not comparative benchmark medians; the new
driver also links the core-only release feature build. Both exclude compilation.

This is a successful selected-sector replay/descent/coverage check, **not a
full FG artifact**. Durable lowering, combined installation, cold inspection
and application remain to be tested by the next complete physical-family
CLI run. The aggregate diagnostic does not expose the newly generated case
domains, so the extra three rules are not yet identified as particular fixed
faces. Evidence is `updated.stdout` and `updated.time` in
`/tmp/rustred-fg106-trace.O1QCHP/`.

#### Matched unchanged C++ reference agrees with the old candidates

A temporary release C++ driver then used the exact ten FG denominators, sector
106, natural static ordering, numerical depth 2, and the complete **281-mask
native zero census**. It invokes the unchanged SpIRed solver serially on CPU
36; the unrelated vendored `vac4.cpp` family was not used.

It produces **96 rules and one master candidate in 549.566 ms search**,
**0.60 s process wall, 0.55 s CPU, and 9,228 KiB peak RSS**, exit 0. C++ rules
85--87 have exactly the same targets, sole `n8=0` exclusion, and four RHS
terms as the frozen old RustRed candidates, after translating C++'s one-based
coefficient names. This rules out a simple candidate transcription mismatch;
it does not turn those candidates into authenticated original-source proofs.

The corrected RustRed search's 557 ms is similar in this single shared-host
observation, but includes a changed 99-rule workload. Its additional 1,002 ms
exact audit is not performed by the C++ driver, so their total runtimes are
not equal-workload performance comparisons. Temporary source, native census,
complete numbered rules, three generated reference files, and timing evidence
are in `/tmp/rustred-fg106-cpp.QSBJtn/`. No C++ solver or production Rust code
was modified for this comparison.

#### Corrected full physical FG run reaches exact certificate swell

The next complete release CLI attempt retains the same external input, natural
ordering, nonpositive indices 8 and 9, and two workers on CPUs 34 and 35.
All **124 nonzero-sector searches** finish by **57.1 s**, producing **9,272
candidate rules and 145 finite terminal candidates**. This is eight more rules
than the old discovery run; the finite-terminal candidate count is unchanged.

The exact audit now passes **32 sectors**, including sector 106's **99/99
rules with zero issues and zero uncovered boxes**. It enters sector **214**
with 161 rules at **80.9 s** and remains there until the new 600-second cap.
The full process uses **600.13 s wall, 651.21 s CPU** (647.10 user + 4.11
system), and **551,840 KiB peak RSS**, exiting **124**. No artifact is written,
so cold inspection and application are still not attempted for FG.

A **15-second live CPU profile**, containing **724 cycle samples** and no
reported lost samples, localizes this stall. Inclusive stacks contain native
`SparseRowReducer::add_row` in **97.90%** of samples,
`ordinary::native::propose` in **95.67%**, and native polynomial GCD in
**64.90%**. These overlap: they are not additive timings or measurements of
the full campaign. The profile does not identify the active rule ordinal
within sector 214. This is a shared-host bounded diagnostic.

The source-level finding is concrete: the original-source membership proposal
augments every source row with its own identity column, `[A | I]`, making even
physically dependent rows independent and carrying all their rational source
weights through exact elimination. The smallest proposed next slice is native
modular dependency-support pruning before the existing exact proposal, with
full unprojected replay and every guard gate unchanged. A failed compact
proposal must retain deterministic fallback within the same quotient; new
certificate poles must not hide exceptional faces. The detailed design and
native Symbolica reuse points are in
[`original_source_certificate_performance.md`](research/original_source_certificate_performance.md).
That proposal was not implemented at this checkpoint; the subsequent bounded
implementation and rerun are recorded below.

Evidence, frozen executable, invocation and full profile are retained at
`/tmp/rustred-safe-projection.DMp3co/`. This run moves the active physical FG
blocker from the now-resolved sector-106 proof gap to the cost of original-
source certificate proposal at sector 214; it does not establish that all
remaining exact checks will succeed once that cost is reduced.

#### Compact certificate-support rerun (2026-09-16)

The bounded native modular support proposal is now implemented, with exact
lifting, no-new-index-pole admission and full unprojected replay. The fallback
retains the complete exact proposal within the same quotient. Independent
implementation/mathematical audits and 121 source-port tests pass (five existing
larger workloads ignored). Release K1/K3/K6 generation, fresh-process inspection
and canary reductions pass; K6 bytes agree at one, two and six workers.

The full physical FG rerun still does **not** close. Its 124 searches finish by
**59.4 s**, yielding the same **9,272 rules and 145 finite candidates**. After
32 passed sector audits it starts sector 214 at **79.5 s**, then reaches the
**600.14 s** bound (651.32 s CPU, 443,624 KiB peak RSS). No artifact is written.
A 15-second profile again points to exact certificate proposals: 98.32%
inclusive native sparse elimination, 93.57% `propose_projected`, and 64.17%
polynomial GCD. These overlapping local samples cannot identify the active
rule or distinguish compact from fallback proposals.

A separate structural diagnostic finds that 25 fully fixed rules share an
identical 1,493-source selected trace over 268 seeds. Independent replay can
therefore regenerate 4,288 ordinary rows for each of those rules. The next
slice retains the preconditioner's native-polynomial row-operation derivation
for exact coefficient composition instead of repeating a membership solve.
Full original-source replay, pole admission, descent and coverage remain
mandatory. This is a follow-on design, not an already measured speedup.
Evidence: `/tmp/rustred-compact-certificate.LQpBJ4/` and
`/tmp/rustred-fg214-rule-shapes.vnNEle/`.

## Interpretation

None of the four external parents currently has a cold-loadable RustRed
artifact. In particular, no physical or dotted reduction canary is reported:
the CLI publishes atomically only after complete sector coverage and exact
installation, and all runs stopped before publication. The observations
do show materially different profiles: the initial X and BMW attempts consume
their five-minute budgets; unrestricted FG encounters nonlinear geometry;
unrestricted H now completes discovery but stalls in exact checking; scoped
physical FG initially exposed three exact-replay failures in about 50 seconds,
then conservative projection resolved those failures and the next full run
reached a separately profiled certificate-proposal bottleneck at sector 214.

These runs do not authorize hard-coded relations, topology-specific dispatch,
sampled coverage, or a claim of four-loop closure. The next implementation
requirement is to accelerate the physical FG original-source certificate
proposal without weakening replay or exceptional-domain coverage, then repeat
complete scoped validation. The unrestricted objective retains its separate
nonlinear-geometry and exact-check cost challenges. Publication stays fail-
closed for every unproved identity or region.
