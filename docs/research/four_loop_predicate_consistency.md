# Four-loop guard coverage: exact consistency and measured status

The four-loop H/X/BMW/FG studies are external inputs. None has yet produced a
closed, cold-validated artifact. This report distinguishes discovered rules,
valid identities and complete coverage; none of these can replace another.

## Why a reported unbounded box need not be a missing rule

An owner consists of a rectangular index domain, an optional affine target,
and excluded exceptional predicates. Coverage partitions their equations into
Boolean zero/nonzero assignments. Treating equations as unrelated Boolean
atoms is conservative: it can reject valid coverage, but must not prove
coverage of an actual hole. The first rejected abstract box is neither an
exhaustive complement census nor necessarily a realizable integral.

Two release examples expose the distinction:

| Case | Fixed face | Required zero | Required nonzero | Contradiction |
| --- | --- | --- | --- | --- |
| H370 | `n0=-1` | `-1-n7-n3+2*n0` | `3+n7+n3` | The expressions are negatives. |
| FG214 | `n3=-1` | `-1-n8+n5+2*n3` | `-3-n8+n5` | The expressions are identical. |

These statements hold on the whole reported box, including its unbounded
directions. No finite sampling, terminal addition or dropped rule is involved.

## Implemented corrections

### Complete exceptional intersections in replay

Discovery already uses `Case::intersect_many` for an exact conjunction whose
solution may be a union of cases. Replay incorrectly used its single-case
counterpart. It now uses the same existing disjunctive service, retains every
admitted child and deduplicates only by complete containment. An unsupported
sibling fails the whole operation; a partial union is never published.

The actual H370 example combines an affine target with
`-n7-n7^2+14*n3+3*n3*n7+6*n3^2=0` and `n3=0`. Native exact case refinement
exposes the existing coordinate exception. The release audit increases from
133/134 to **134/134 replayed and descending rules**, without changing the
candidate IBPs or adding a master.

### Bounded reuse of exact fixed-coordinate substitutions

A traversal-local cache binds one immutable sector and atom table. Exact box
keys retain every endpoint; values are only zero, nonzero constant, unknown
or unsupported classifications. Symbolica still performs every substitution.
No polynomial or expanded expression is stored in the cache.

The limits are 4,096 boxes, 131,072 atom slots and 131,072 coordinate cells.
Capacity exhaustion falls back to the same bounded calculation. Native work
is charged on a cache miss, under the unchanged 4,194,304-work allowance.
Exhaustion is a typed incomplete result, never proof that a branch is empty.

This removes the observed FG work-budget obstruction, but cannot by itself
infer relations between two nonconstant equations.

### Bounded native affine implication

After cheap individual-literal checks, the consistency service can join the
remaining assigned affine equations. It validates their original support,
index maps and native variable contexts, substitutes fixed coordinates with
arbitrary native integers, and calls the existing `canonical_equalities`
adapter to Symbolica/Numerica exact row reduction.

For the true system `T` and a required-nonzero polynomial `f`:

1. If `T` is proved inconsistent, the entire Boolean branch is impossible.
2. If `T` and `T` augmented by `f=0` are both consistent and have equal rank,
   then `T` implies `f=0`; that contradicts the required `f!=0`.
3. An inconsistent augmented system does **not** refute `f!=0`. For example,
   `a=0` and `a-1!=0` are consistent, although adding `a-1=0` is impossible.

This is a sufficient exact test, not a general integer inequality solver.
Unsupported inputs and native failures remain inconclusive. Equation/matrix
sizes and all native work are bounded. No custom elimination, interpolation
or rational reconstruction was added.

The combined cache/replay/implication gate passes **473 tests**, zero failures,
with five existing workloads ignored. Separate implementation and independent
mathematical/code reviews pass. Its release results are recorded below;
these focused tests do not themselves establish four-loop closure.

## Frozen cache/replay release observations

These measurements precede affine implication. All use `--release --locked`,
external family input and complete original-source authority. Compilation is
outside the runtime boundary. Runs were concurrent on disjoint CPU sets on
a shared host, so the timings are not controlled speed comparisons.

| Family | Whole-family search | Publication result | Wall time |
| --- | --- | --- | --- |
| FG | 124 sectors, 9,272 rules, 145 finite candidates; 55.181s | 32 earlier sectors pass; FG214 has 161/161 valid identities but the contradictory abstract witness remains | 139.95s |
| H | 314 sectors, 21,360 rules, 386 finite candidates; 58.482s | 56 earlier sectors pass; H370 has 134/134 valid identities but the contradictory abstract witness remains | 83.20s |
| BMW | 133/134 searches complete; partial totals 8,964 rules and 177 finite candidates | Search rejects an unsupported root-free quadratic before any publication audit | 84.44s |
| X | 326/328 searches complete; partial totals 19,713 rules and 433 finite candidates | First reported search error is another root-free quadratic under an affine target | 197.32s |

No four-loop artifact is written. K1/K3/K6 still regenerate byte-identical
artifacts and pass fresh-process validation and master-only canary reductions.
Unnamed K6 takes 2.99s in this regression, retaining 623 rules, 5,640 cells and
38 masters. This is not a new CLI/Python or C++ comparison benchmark.

BMW's first obstruction is `n1^2-3*n1+4=0` in sector223. Since
`4*(n1^2-3*n1+4)=(2*n1-3)^2+7`, this branch is empty even over the reals.
The intersection engine already calls native complete factorization; its
unchanged irreducible nonlinear factor falls through to unsupported geometry.
The existing indexed-algebra service already recognizes that an irreducible
univariate factor of degree above one has no rational/integer root. Reusing
those native-backed semantics is a separate next slice, not part of affine
implication. Multivariate nonlinear branches must continue to fail closed.

X's first reported error is sector214: `n8^2-8*n8+3=0` under the affine
target `-1-n8+2*n5=0`. This quadratic likewise has no rational/integer roots;
its discriminant is 52, not a square. It is also strictly positive throughout
this sector's `n8<=0` half-line. Only the first search error is returned, so
the other unsuccessful search's cause has not been identified.

Local evidence: `/tmp/rustred-cache-replay-release.E8j6Wm/`,
`/tmp/rustred-affine-implication.qNiw1l/`, and
`/tmp/rustred-bmw-cache-replay.szlM8t/`, with X at
`/tmp/rustred-x-cache-replay.sSIK5G/`. Temporary binaries/logs are not shipped.

## Native implication release results

The subsequent frozen release certifies H370 completely: **134/134 replayed
and descending rules, zero stored or checked uncovered regions**, 11 finite
terminals, no issues. Search takes 2.682s, exact audit 8.223s, whole selected
process 11.02s. This closes the observed sector obstruction, not the family.

The whole H campaign searches all 314 sectors in 52.842s, then advances from
56 to **91 completely passing sector audits**, including H370. It stops at
H214's native affine-consistency work limit after 112.97s wall. FG searches
all 124 sectors in 32.773s, passes 32 complete audits and hits the same work
limit at FG214 after 109.92s wall. Neither stopping sector returns a final
audit aggregate, so its last rule-start event cannot certify every rule.
Neither family writes an artifact. These are shared-host observations.

K1/K3/K6 artifacts and fresh-process master-only applications remain identical
to the baseline. K6 generation is byte-identical at workers 1, 2 and 6. Native
joint row reductions currently repeat across changing false-literal prefixes;
bounded reuse must key the exact box, complete true-literal set and immutable
context, without treating a weaker true system or failed operation as proof.
The work allowance is not raised. Release logs:
`/tmp/rustred-implication-release.fVtfmq/`. A same-CPU paired K6 check, separate
from these concurrent observations, is at
`/tmp/rustred-k6-implication-pairs.iD04Su/`.

The three same-CPU serial pairs have median wall observations of 2.72s for
cache/replay and 2.77s with implication, with the pairwise ordering reversing
in the last pair. This noisy small check does not establish a speed ratio or
confirm the apparent twofold increase in an earlier concurrent observation.
The FG work-budget exhaustion, not that isolated timing, motivates reuse.

## Root-free exceptional factors

The next independently audited correction reuses the complete factorization
already returned by Symbolica. An irreducible factor supported on exactly one
authenticated index variable and having degree greater than one cannot have
a rational root, hence cannot have an integer root. Such a zero-equation
branch is empty. No local root finder, discriminant routine, sampling or extra
factorization is introduced. Linear factors still undergo ordinary integer
admission; coupled nonlinear factors remain unsupported. All raw factor terms
are charged before filtering, and every remaining OR sibling must complete.

This slice passes **484 focused tests**, zero failures, five existing ignored
workloads, plus independent proof/code review. The actual BMW223 release
audit passes all **60/60 replayed and descending rules**, with two finite
terminals, zero uncovered regions and no issues, in **5.21s**. X214 now
searches successfully (160 rules, 11 finite candidates, 8.593s), but its exact
audit reaches the native affine-consistency work limit after 50.30s total.
It returns no final rule or coverage aggregate.

The full BMW release search now completes **134/134 sectors**, with 9,024
candidate rules and 179 finite terminal candidates, in 68.815s. Publication
passes **45 complete sector audits**, then reaches the same consistency work
limit in BMW158. The final rule-start event is not a completed sector report.
The process exits after 178.17s wall, writes no artifact, and provides no
mathematical uncovered-domain witness. The full X search also completes all
328 sectors (19,980 rules, 445 finite candidates) in 218.721s. Publication
passes **31 complete sector audits**, then stops at X460 on the same
consistency-budget error, after 257.47s wall. No artifact is written. The
frozen release predates the next slice's detailed counters, so these errors
do not distinguish the exact exhausted work stage from a local admission cap.

K1/K3/K6 regenerate unchanged bytes, pass current-core fresh-process loading
and master-only canary reductions, and K6 is byte-identical at workers 1/2/6.
The unnamed serial K6 regression is 3.47s wall. These concurrent shared-host
Rust API runs are not controlled performance ratios or new CLI/Python builds.
The next generic slice reuses successful native true-system and false-extension
outcomes with exact keys and the same work allowance; it is not included in
this frozen release.

Evidence: `/tmp/rustred-univariate-case.T3Icsy/`,
`/tmp/rustred-univariate-audit.wJ98A8/`,
`/tmp/rustred-univariate-release.VxseCS/`, and
`/tmp/rustred-bmw-univariate-release.k7WHbl/`.

## Reusing completed native consistency checks

The next slice stores only successful scalar outcomes of native affine
reductions. Its key retains the full exact box and complete true-atom ordinal
set, bound to one immutable sector/atom-table context. Sparse false-atom
outcomes reuse only the corresponding completed extension. Original input
admission precedes lookup; native failures and exhausted work are not proof.
Both caches share the existing entry/slot/coordinate limits, and the native
work allowance remains 4,194,304. No matrix or polynomial survives a call.

All **495 focused tests pass**, zero failed, five existing ignored, with an
independent mathematical/code audit. An opt-in internal diagnostic reports
actual native calls, substitutions, successful estimated-work charges, hits,
fallbacks and the exact exhausted stage. These counters carry no authority.

Selected release audits now pass BMW158 (**155/155**), FG214 (**161/161**)
and H214 (**152/152**), each with zero uncovered regions and no issues.
Their respective per-traversal work charges are 1,774,911, 4,051,298 and
2,018,045. The two cover traversals remain independent, not one shared cache.
X460 still exhausts the cumulative allowance: an extension needs 363 units
with 84 remaining. There is no cache-capacity fallback or native failure.
That resource error is not a mathematical coverage witness.

The full FG campaign now searches all 124 sectors and passes **every sector
audit: 9,272/9,272 exact replays and strict descents, zero stored/checked gaps
or issues**. Publication subsequently fails while lowering FG114 rule66/83
into durable cells: `guard zero locus is not proved outside the complete
affine application domain`. Seventeen sectors have lowered by that point.
The process exits after 232.39s, with 2,959,544 KiB peak RSS, and writes no
artifact. This is a new, later guard-proof boundary—not whole-family durable
closure or a successful numerical Vakint reduction.

BMW searches all 134 sectors, passes **97 complete sector audits** (formerly
45), then reaches the native-work allowance in BMW107 after 439.00s wall.
The error is a base reduction requesting 363 units with 23 remaining, not a
local shape limit. No final aggregate is returned for that sector and no
artifact exists. H now also passes **all 314 sector audits: 21,360/21,360
exact replays and strict descents, zero gaps or issues**. Durable lowering
completes 56 sectors before H370 rule66/134 exceeds the prospective affine
guard-chart integer-bit bound. The process exits after 338.17s, with
7,441,576 KiB peak RSS, and writes no artifact. This is a different later
resource bound from the consistency allowance. A redundant whole-X rerun is
deferred until its reproduced selected-sector limit changes.

A 15-second DWARF-unwound profile during BMW's exact audit records 724 samples
and zero lost samples. Inclusive sampled costs are 99.35% in source replay,
92.94% in native sparse rational-polynomial reduction, and 64.07% in polynomial
GCD. These nested percentages overlap and apply only to that window. They
identify a replay bottleneck, not affine-consistency row reduction; neither
accounting units nor one sampled interval imply whole-run phase percentages.

K1/K3/K6 bytes, current-core fresh-process checks and master-only canaries
remain unchanged; K6 workers1/2/6 are byte-identical. Its unnamed serial
regression is 1.76s wall, not a controlled speed comparison. No CLI/Python
frontend was rebuilt for this internal proof slice. H214 was inadvertently
probed twice on disjoint CPUs; both pass and neither is a benchmark.

Next priorities are the actual FG guard/domain counterexample and an explicit
bounded work policy if needed for X/BMW, without relaxing any mathematical
or cold-load gate. More cache-key changes require evidence of missed reuse.
Release evidence: `/tmp/rustred-consistency-reuse-release.kKCcQ1/`.

## Joint coefficient-zero refinement

The captured FG114 rule66 denominator is

```text
Q = (n8+3)*(d-n0) + 2*n8^2 + 5*n8 + 1.
```

To vanish identically in the generic dimensional parameter `d`, both its
coefficient equations must vanish. The coefficient of `d` forces `n8=-3`,
but the remaining equation is then the nonzero constant 4 for every `n0`.
There is no common exceptional locus. This does not assert the absence of
poles at specially chosen numerical values of `d`.

The existing indexed-algebra service already factors a coefficient with
Symbolica and replays the resulting integer-root hyperplanes against the
complete coefficient conjunction. It now removes a hyperplane if any native
restriction is a nonzero constant. A nonzero nonconstant restriction stays
inconclusive, and replay continues in case a later equation supplies a
constant contradiction. Only all-zero restrictions make a surviving
hyperplane exact. Every surviving root remains in the returned union.
The full root-by-equation replay remains precharged under unchanged limits;
no algebraic primitive, root finder or topology-specific rule was introduced.

Eight new regressions cover the actual denominator, mixed exact/conservative
survivors, later contradictions, real codimension-two intersections, arbitrary
native-integer roots and admission limits. The extended combined suite passes
**553 tests**, zero failures, five existing ignored, with independent
mathematical and implementation audit.

The new full release run searches all 124 FG sectors in 43.628s and passes
all **9,272/9,272 original-source replays and strict descents**, with zero
stored/checked gaps or issues. Every sector audit completes by 220.820s.
FG114 then lowers completely to 1,363 cells, confirming the previous actual
publication obstruction is cleared. Thirty-two sectors lower completely
(previously 17), before FG214 rule74/161 reaches
`affine guard chart exceeds prospective integer-bit budget`.

The process exits normally with the error code after **246.78s wall**,
275.49s user and 10.86s system time; peak RSS is 5,120,096 KiB. It is not
stopped by its 1,200s external timeout. No artifact is written or cold loaded.
This later conservative resource bound is not evidence of a missing IBP.
The exact private guard triggering it still needs capture; candidate RHS
formulas alone cannot identify a source-weight/provenance guard.

The next support-aware check should query Symbolica's native `contains()`
against both fixed coordinates and the authenticated chart's actual pivot
positions. If neither occurs, zero-locus restriction is a no-op and needs no
dense expansion estimate. Such a shortcut must retain original admission,
work and degree checks and must not itself imply guard nonvanishing. This
next check is not implemented or validated by the present release.

K1/K3/K6 artifacts and exact reductions remain unchanged, with current-core
fresh-process loading and master-only canaries passing. K6 workers 1/2/6
produce identical bytes. Serial generation observations are 0.06s/0.02s/2.00s
for K1/K3/K6; these concurrent shared-host runs are not controlled speed
comparisons or new CLI/Python benchmarks. H/X/BMW were not redundantly rerun
for their distinct remaining obstructions.

Evidence: `/tmp/rustred-joint-guard-refinement.m3uTgL/`,
`/tmp/rustred-joint-guard-release.QQBORj/`, and the read-only chart design in
`/tmp/rustred-chart-noop-design.lsF8sJ/DESIGN.md`.

## Actual chart-support failures and fixed-cell refinement

Failure-only diagnostics now capture the actual lowered guard, exact target,
box, exclusions and failed prospective bound. They are enabled only by
`RUSTRED_AFFINE_GUARD_DIAGNOSTICS=1`, have a 16 KiB block limit with explicit
truncation status, and cannot replace the original proof error. Successful
guards do not format diagnostics or inspect the environment. Existing bounded
formatters share one crate-private implementation.

The diagnostic-only release baseline passes 595 focused tests and repeats
both full campaigns without changing proof decisions. FG passes all 124 sector
audits, then fails lowering FG214 rule74 after 273.17s wall; H passes all 314
audits, then fails H370 rule66 after 337.76s. Neither writes an artifact.
The actual FG guard is

```text
Q = -10 - 13*n8 - 3*n8^2 - 12*n5 - 5*n5*n8 - 2*n5^2
    + 3*d*(1+n8+n5).
```

Its chart replaces `n3` using `2*n3-n8-1=0`; the other fixed coordinates
also do not occur in `Q`. Nevertheless the old dense expansion estimate
charged 1,089 terms and 230,868 bits for this nine-term polynomial.
The actual H guard likewise contains none of the chart's substituted variables:
its chart replaces `n0`, while the polynomial uses only `d,n3,n7`.
Its 15 terms were charged 19,965 prospective terms and 6,308,940 bits.

The support-aware correction queries Symbolica's existing `contains()` on
authenticated fixed and pivot positions. An unaffected guard is returned
unchanged **after** the original map, size, bit, degree and work checks.
This proves only that substitution is unnecessary, not guard nonvanishing.
Affected polynomials retain the existing prospective bounds.

The H capture supplies another exact simplification: its actual application
cell fixes `n3=0`, with `n7<=-2`, and has no affine exclusions. Substituting
that singleton with the existing native fixed-polynomial service leaves
the coefficient of `d` equal to `3*n7*(n7+1)`, nonzero throughout the cell.
The verifier now uses supported cell singletons before its existing native
coefficient zero-locus analysis, after chart restriction and original input
admission. All selected singleton-by-input-term work is charged before native
substitution. Checked physical-coordinate conversion leaves an unrepresentable
machine endpoint symbolic, never narrowed. An identically zero restriction
remains inconclusive. No interval is replaced by a sampled representative.

Independent source and mathematical audits pass. New regressions cover both
captured failures, genuine zero guards, fixed/pivot support, foreign contexts,
input limits, singleton work charges and unrepresentable endpoints. The FG
regression includes the captured exclusion, but singleton specialization can
prove that example directly; dedicated implication tests separately cover
the exclusion proof.

Capture evidence: `/tmp/rustred-guard-capture.Z0r0WH/`.
Independent audit: `/tmp/rustred-chart-diagnostics-audit.F5hc5S/AUDIT.md`.
The enabled correction's release evidence is recorded separately in
`/tmp/rustred-chart-support-release.FT3gS3/`. Debug and release builds pass;
the broader diagnostic-inclusive focused gate passes **603 tests**, zero
failed and 16 existing ignored. All eight new guard regressions and five
chart-support tests ran.

Both full reruns clear their captured guard failures. FG again passes all
124 sector audits and 9,272 replayed/descending rules with zero gaps/issues;
H passes all 314 and 21,360 respectively. Thirty-two FG sectors and 56 H
sectors lower completely. The next failures occur later within the same
sectors: FG214 rule76 and H370 rule90 reach the existing combined endpoint
storage preflight. The formula is `(3*retained_rhs_terms + 4)*arity*2`:
11,600 cells for FG's 192 retained RHS terms and 8,720 for H's 144, against
the unchanged allowance of 8,192. These are conservative buffer counts,
not numbers of distinct domains, uncovered regions or masters. The three
independently constructed proof buffers cannot simply be removed from the
estimate without first changing ownership/sharing.

FG exits with code 8 after 240.70s wall, 264.21s user and 8.58s system time,
with 5,098,392 KiB peak RSS. H exits with code 8 after 327.18s wall, 373.94s
user and 10.35s system time, with 7,475,724 KiB peak RSS. Neither reaches its
1,200s timeout or writes an artifact. The next narrow step is exposing the
already-public `ParametricRuleLimits.max_domain_bound_endpoint_cells` to
generation while retaining its existing caller-controlled cold-load setting
in `ArtifactLoadLimits.rule_derivation`. No default was raised in this slice.

K1/K3/K6 artifact bytes and exact canary reductions are unchanged. Each passes
current-core fresh-process validation and master-only application; K6 workers
1/2/6 produce identical bytes. Its artifact retains 623 rules, 5,640 cells,
38 masters, 26 zero sectors and 8,925,944 bytes. Serial core generation takes
0.03s/0.03s/2.40s for K1/K3/K6 on this shared host; these are regression
observations, not controlled speed ratios. CLI/Python frontends were not
rebuilt for this internal verifier slice. BMW/X were not redundantly rerun
for their separate unchanged native-consistency budget obstruction.

## Explicit resource-policy release (2026-09-16)

`SourcePortLimits` now carries caller-owned rule-derivation and consistency
allowances through both sector covers, retained programs, lowering and final
installation. Cold loading independently selects `ArtifactLoadLimits`; no
resource setting is encoded into artifact authority or mathematical identity.
The defaults remain 8,192 endpoint-storage cells and 4,194,304 consistency-work
units. Zero is restrictive, not an unlimited sentinel. A single consistency
budget is shared by all boxes/branches in each whole traversal; native matrix,
term, cache and other local bounds remain unchanged.

The Rust application, generic CLI and public Python API expose both settings
for generation and loading. Output reports record selected settings as decimal
strings without narrowing `usize` to TOML's signed range. Report schemas become
family-close v3, inspect v3 and reduce v2; durable artifact encoding is unchanged.
Consistency exhaustion has typed errors. Existing endpoint failure taxonomy
is unchanged. Raising either allowance authorizes additional exact proof work,
never a weaker proof or self-authorizing artifact.

Independent code and mathematical audits pass. The core gate passes **606
tests**, zero failed and 16 existing ignored. Eleven frontend test targets
pass 132 tests. A twelfth target initially fails one stale assertion requiring
the obsolete specialized K6 example generator; its test-only correction checks
the actual generic `family-close`/external-input contract and passes 2/2 after
rebuilding. Original failure evidence is retained. No production behavior was
changed to satisfy that obsolete assertion.

The rebuilt local release wheel passes all **29 public Python API tests**
against the rebuilt CLI, with zero skips, in 46.830s (46.98s process wall).
The first wheel run passed 28 tests and failed a worker-width test because the
harness restricted the process to two CPUs while requesting four. The unchanged
test/wheel suite passes with four-CPU affinity; both sets of logs are retained.
Combined current frontend gates are 134 Rust tests and 29 Python tests passing.

Full release public CLI runs use the unchanged external physical parent inputs,
two workers, endpoint allowance 65,536, consistency allowance 67,108,864, and
a 1,800-second wall bound. All finish with exit 8 before that bound:

| Family | Sector audits | Fully lowered sectors | Wall / user / system seconds | Peak RSS KiB | Obstruction |
| --- | --- | ---: | --- | ---: | --- |
| H | 314 passing; 21,360 replayed/descending rules | 66 | 318.00 / 356.27 / 15.75 | 8,846,224 | H282 displayed rule51: weaker coordinate guard recheck |
| FG | 124 passing; 9,272 replayed/descending rules | 41 | 232.45 / 258.72 / 7.18 | 6,715,388 | FG158 displayed rule58: same recheck |
| BMW | 134 passing; 9,024 replayed/descending rules | 31 | 485.26 / 535.01 / 7.65 | 4,656,532 | BMW230 displayed rule49: guard/exclusion implication |
| X | 65 passing, then first abstract complement in the 66th audit | 0 | 340.06 / 465.19 / 5.40 | 951,324 | X394 abstract assignment is inconsistent |

All H/FG/BMW sector audits have zero reported gaps/issues. X searches all
328 sectors (19,980 rules, 445 finite residual candidates), then stops during
the 66th audit; its 4,409 replayed/descending rules include that stopping sector.
These counts do not establish publication or terminal authority. No four-loop
artifact is written. These are shared-host observations with verbose progress
and proof diagnostics, not controlled comparisons against earlier drivers or
SpIReD. Compilation was separate: core release 3m36, frontend release 8m20.

### Coordinate replay seal: next narrow correction

H/FG's exact original-domain service already proves every guard on the actual
application piece, then replays original sources and proves strict descent.
Its private sealed record binds those guards, origins, RHS, source owner and
domain. No later step introduces a new denominator or widens the application.
However, the private `RuleCell` constructor consumes that guard proof only for
affine cases; coordinate cases repeat an older weaker bounds-only check.
Independent tracing finds this duplicate check causes the observed failures.
The proposed correction consumes the same private proof for coordinate cells,
retaining all public/unsealed checks and fresh cold reconstruction. It remains
a separate implementation/test/release gate, not part of these timings.

### BMW: a retained exclusion already contains the guard zero locus

The captured polynomial is `Q=C0+d*C1`, with `a=n3`, `b=n4`, `c=n9`:

```text
C0 = -3+2c+c^2+4b+2bc-8a-2ac+2ab-3a^2
C1 = 1-c-2b+3a
C0 + (a+c+2)*C1 = c-a-1
```

Generic-d vanishing requires both coefficient equations. Their exact identity
implies `a=c-1`, then `b=c-1`. The retained target `2n0-a-c-1=0` gives `n0=c`,
exactly the second retained exclusion. This particular obstruction is a missing
proof capability, not a missing IBP. The generic sufficient proof candidate is
native Symbolica ideal membership of exclusion equations in the ideal generated
by guard coefficients and target equations. Symbolica already provides native
Gröbner bases/reduction; RustRed must not implement polynomial elimination.
Resource safety and exact admission require a separate audit before integration.
Further API review identifies an even narrower reuse path: the existing
`Case::intersect_many` first absorbs affine equations, restricts remaining
polynomials through its native chart, and repeats. For this guard, affine `C1`
already turns `C0` into an affine condition; native F4 need not be invoked.
The cold verifier needs runtime-arity reuse of that policy, not a topology- or
arity-specific dispatcher.

### X: exact finite refinement, not sampling

The first abstract branch imposes `A0=-1-n6+n0=0` and
`A1=-4-4n6-n4+2n2=0`. It also has `n0<=-2`, `n4<=0`, `-1<=n2<=0`.
But `A1-4*A0=-4*n0-n4+2*n2` is at least six on that box, a contradiction.

An equivalent proof reuses narrower existing services: exhaust the two integer
faces `n2=0` and `n2=-1`, leaving every other axis unbounded as before. `A1=0`
implies that two atoms assigned nonzero, `4+4n6+n4` and `6+4n6+n4`, equal
`2n2` and `2n2+2` respectively. Each face therefore contradicts one false atom.
Existing native rank implication suffices after exact singleton specialization.
The existing bounded finite-axis geometry policy can guide this refinement;
all children must share the traversal work budget. This is exhaustive integer
partitioning, not sampled coverage. Only the first reported branch is analyzed.

### Regressions and evidence

Current release public CLI K1/K3/K6 generation, fresh-process inspection and
canary reduction pass. Artifacts are byte-identical to the prior baseline;
complete reduction reports agree exactly after removing only the report schema
tag and newly declared load-resource fields. K6 high-resource runs at one, two
and six configured workers also produce identical bytes. Those worker tests
used a two-CPU affinity and are determinism tests, not six-core timing claims.
The first shell harness attempt failed `taskset` argument parsing before RustRed
started; corrected retry logs are separate and preserved.

Evidence: `/tmp/rustred-publication-policy-release.4PE5dn/`, including
`controls.hYiJg2/`; independent audit:
`/tmp/rustred-resource-policy-audit.U77NS9/AUDIT.md`.
Public CLI SHA-256:
`800fa4c2bbd01b7b7e12eddc3622735a8a53410dd05019e080f3e7db49bb91b9`.
