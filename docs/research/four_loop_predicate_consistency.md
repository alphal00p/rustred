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
