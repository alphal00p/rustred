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

## Interpretation

None of the three external parents currently has a cold-loadable RustRed
artifact. In particular, no physical or dotted reduction canary is reported:
the CLI publishes atomically only after complete sector coverage and exact
installation, and all three runs stopped before publication. The observations
do show that the current generic lane encounters materially different resource
profiles: X and BMW consume the full five-minute budget, while FG reaches an
unsupported nonlinear exceptional branch in about 234 seconds.

These runs do not authorize hard-coded relations, topology-specific dispatch,
sampled coverage, or a claim of four-loop closure. The next implementation
requirement is full-family validation of the exact affine ownership path now
implemented, with nonlinear exceptional cases remaining unsupported where
they cannot be proved. Publication stays fail-closed for every unproved region.
