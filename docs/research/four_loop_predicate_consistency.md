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

The separate root-free univariate factor correction passes 44 focused tests
and independent review. It is not included in this release snapshot and has
not yet passed a full BMW/X release campaign. Evidence:
`/tmp/rustred-univariate-case.T3Icsy/` and
`/tmp/rustred-univariate-audit.wJ98A8/`.
