# Four-loop numerator terminals: independent symmetry-first critique

Status: research and independently audited bounded scratch test, 20 September
2026. No production change or catalog-derived discovery identity.
This is the independent mathematical lane accompanying the deeper finite-target
study requested after [EPSILON.md](../../EPSILON.md).

## Recommendation

Test **weighted numerator relations from verified support symmetries before
enlarging the integer-IBP seed pool**. This is materially different from the
shipped unit-weight alias service: a momentum substitution can preserve every
active denominator while sending an inactive numerator to an affine sum of
denominators. The resulting integral equals a linear combination, not usually
another single integral key.

The [finite-source study](finite_terminal_relations.md) leaves 179 family-local
representatives: 74 nonnegative keys and 105 keys with five powers `+1`, one
power `-1`, and zeros elsewhere. Eliminating all 105 into the existing positive
span would leave at most 74 representatives. That is a **conditional upper
bound** in the proposal; the initial result below now verifies its premises on
this saved corpus. Actual support admission, exact relations, and closure of
newly generated positive keys are all necessary. Removing 80
keys without adding any would already bring the shipped count below 100.

### Initial bounded result

The approved same-input scratch process completed all 105 projections and
post-hoc exact catalog checks. Its explicit closed output census is **74**:
H 22, FG 16, BMW 17, X 19, with no unresolved/unbound relations or new output
keys. All 21 supports have full rank four, five circuit members and no coloops;
84 generator maps pass exact verification. The independently hand-derived FG
example below is reproduced. Native combination replay, full record counts,
positive output closure, hashes, status and resource logs were independently
audited. Whole process: 6.92 seconds wall, 5.39 user + 1.48 system seconds,
1,050,048 KiB peak RSS, below its 120-second/8-GiB virtual-address-space limits.

Evidence is `TMP/terminal-support-symmetry.xCn7kw/`, including
`independent-audit.md`. The separately authorized fresh-process repeat in
`TMP/terminal-support-symmetry-repeat.LjS1P7/` reproduces all eight identity and
witness files byte-for-byte, including exact combination weights and closed
output sets. It completes in 8.25 seconds with all 105 catalog checks passing
again. This is a same-family exact finite-normalization result,
not a proof of minimality or whole-program closure. The nine older positive
IBP identities are not used. Production still returns up to 179 representatives;
generic implementation, a native sidecar, runtime measurement and numerical
acceptance remain separate delivery gates.

## Why the earlier misses do not disfavor this route

The radius-zero and FG radius-one experiments retained unrecognized integrals
as auxiliary columns and eliminated only their explicitly generated IBPs.
Their zero terminal-only row space describes those source pools, not all IBP
and change-of-variable relations. The nine later positive identities appeared
only after quotienting generated columns by additional exact equivalences.

The present implementation explicitly skips negative indices in both
`terminal_normalization/corank_one/mod.rs` and `parametric/mod.rs`. Thus none
of the 105 numerator keys was tested by these positive-power equivalence
methods. Unchanged numerators are a designed admission boundary, not evidence
of independence. Nor did the finite-source experiments project arbitrary new
IBPs through the existing candidate reducer; that is a separate next test.

This proposed extension is established reduction practice. Azurite constructs
affine momentum transformations from graph symmetries and derives their action
on numerator scalar products, then combines numerator-symmetry and IBP
relations. Its cut calculations distinguish lower-sector contributions that
must not be silently omitted in an uncut identity.
[Azurite, sections 2.1–2.3](https://arxiv.org/pdf/1612.04252).

## Conditional mathematical argument

The strategy is generic in loop count `L` and supplied denominator arity `N`;
the four-loop counts below are an input/test instance, not dispatch constants.
Let a support contain `L+1` distinct denominators `D_i=q_i²−m²`, all at power
one, with no external momenta or analytic power shifts. Require exact momentum
rank `L` and exactly one primitive dependence. If its nonzero coefficients
all have absolute value one, orient the `c` circuit momenta so that

\[
q_1+\cdots+q_c=0.
\]

The remaining `L+1−c` momenta are independent coloops. A basis comprising `c−1`
circuit momenta and those coloops spans all `L` loops. Circuit permutations
and independent coloop sign reversals act linearly on this basis, preserve the
denominator product and have determinant `±1`. In original coordinates every
such map is obtained by conjugation, so a nonunit determinant of the chosen
basis alone does **not** imply a nonunit Jacobian of the map.

Average a quadratic numerator over these symmetries. Coloop sign reversals
remove cross terms involving a coloop. On the circuit, permutation symmetry
and `sum(q_i)=0` leave only `sum(q_i²)` among invariant quadratic forms. Thus
the averaged numerator lies in the span of the active squares and a constant.
Writing squares as `D_i+m²` yields only the scalar `(L+1)`-line integral and its
`L`-line pinches. No tensor integration formula or dimension shift is needed.

The equality is **after integration**, by a change of variables with verified
unit absolute Jacobian. It does not assert pointwise equality between the
original numerator and its average. The polynomial equality to replay is the
average (or a linear combination of symmetry differences), not the original
numerator by itself.

This is a sufficient condition, not a license to discard other cases. Unequal
circuit magnitudes, unequal masses/powers, deficient rank, nonconstant map
conditions, or a nonunit Jacobian require a different argument or a retained
terminal. Rational momentum-map entries are not mathematically forbidden;
the existing corank-one alias proposer conservatively requires integer entries.
A first experiment can preserve that stricter gate and report any resulting
misses separately. Generic `symmetry::verify` still establishes the actual map.

### Hand-derived FG discriminator, not an oracle input

For the current FG input, take support `S={2,3,5,7,8}` and numerator `D1`.
Its saved terminal is `(-1,1,1,0,1,0,1,1,0,0)`. Choose oriented momenta

\[
(q_1,q_2,q_3,q_4,q_5)
=(k_2,-k_3,k_4,-k_1+k_3-k_4,k_1-k_2).
\]

They sum to zero, the first four have determinant `±1`, and `k1=q1+q5`.
The symmetric average satisfies

\[
\operatorname{Av}(q_1+q_5)^2=\frac3{10}\sum_{i=1}^5q_i^2.
\]

Consequently, in the repository's `q²−m²` convention,

\[
I_S[D_1]=\frac3{10}\sum_{i\in S} I_{S\setminus\{i\}}
            +\frac{m^2}{2} I_S[1].
\]

All five four-line pinches have unimodular independent momentum bases, so each
is the same four-tadpole product. This gives `3/2 T4 + m²/2 B5` without any FMFT
input. A diagnostic must regenerate momentum witnesses and exact coefficient
identities; this explanatory formula must not become a hard-coded topology rule.

## Small executable protocol

Use unchanged saved H/FG/BMW/X native programs and their input families. Do not
load terminal-value catalogs until all proposed relations are frozen.

1. Reconstruct the existing 179 representatives and group the 105 numerator
   keys by positive support. Reuse the existing native square-factor proposal
   and exact basis/circuit checks. Record admitted/skipped support counts,
   ranks, primitive circuits, basis determinants, and all skip reasons.
2. For each admitted support, propose only `c−1` adjacent circuit transpositions
   and `L+1−c` individual coloop sign reversals: **L generators**, not a
   factorial orbit enumeration. Build maps with Symbolica's native matrix
   inversion/multiplication. Verify every map through `sector::symmetry::verify`:
   same family, unit Jacobian, constant discharged conditions, and a scale-one
   permutation of the `L+1` active denominator rows. Unused denominator rows
   are allowed to be affine rather than a full-family permutation.
3. The returned `VerifiedMap::denominators()` already supplies
   `D_j -> b_j + sum_i a_ji D_i`. In the common active denominator product,
   each map gives exact integral relations for every `j=1..N`. Retain all
   affine constants and all denominator coordinates. Active coordinates cancel
   to pinches; inactive coordinates remain rank-one numerator integrals.
4. Solve the resulting at most **L*N rows and N+1 columns per support** with
   native Symbolica linear algebra (40 by 11 for these four-loop inputs):
   inactive numerator columns first, then active pinch columns and the scalar
   column. Order within each block by the actual saved
   comparator, not integer sector masks. Store source-map/coordinate labels.
   A direct native `solve_any` query can express a target numerator as a
   combination of verified symmetry rows, active denominators and a constant;
   replay that complete linear combination exactly. Alternatively use native
   GPLU and native matrix inversion/multiplication of its numerator pivot block.
   Do not implement elimination or back substitution as a new CAS kernel.
5. Independently regenerate every symmetry row and check each returned exact
   combination coefficient-by-coefficient. For the optional GPLU route also
   check native `L*U=A` plus invertibility of the selected independent-row block
   of `L`, as in the finite study. Preserve both
   ordered native coefficient maps, including zero/constants. Every retained
   numerator must either have a proved positive-only expansion or remain
   explicitly unresolved; a sampled rank is insufficient.
6. Normalize the **union** of original positive representatives and every new
   scalar/pinch key through the existing exact parameter/routing services.
   Delete only independently proved-zero supports. Report any new positive
   representatives, not just the number of numerator pivots. A fully solved
   numerator block alone does not prove the promised global count.
7. Freeze inputs, maps, original rows, quotient bindings and relations. In a
   separate post-hoc check compare each full relation to the existing imported
   native catalog, with its real epsilon namespace and `d=4−2*ep` where needed.
   Repeat preparation in a fresh process and compare exact results. Failure of
   the oracle comparison is a blocker, not a reason to select different rows
   until the desired catalog expression appears.

Suggested first discriminator: FG's ten numerator terminals. The root-approved
initial scratch experiment may cover the supplied four-family corpus in one
release process on CPU 83, with 120 seconds and 8-GiB virtual address space,
one worker; compilation excluded and no automatic cap increase. Suggested
four-loop corpus accounting caps are 105 supports, 420 verified maps and 4,200
input rows; these must be caller limits rather than constants in a generic
algorithm. No production API change is authorized by this diagnostic. These
are process/structural limits, not per-operation native CAS guarantees. The
independent reviewer has approved the conditional protocol, not unseen results.

## Adversarial checks and interpretation

- A valid active-support map need not permute unused ISP coordinates. Requiring
  a full-family permutation would throw away exactly the weighted relations
  sought here; accepting an unverified affine active row would be unsound.
- Averaging `q1.q2` pointwise is false. Validate the summed polynomial identity
  and each change-of-variable witness separately, including momentum orientation.
- Dots destroy many denominator-product symmetries. Do not transport unit-power
  witnesses to dotted keys without power-preservation checks.
- A pinched coloop can expose a scaleless loop; keep the term until the existing
  exact zero service proves it vanishes. Do not use a graph label as evidence.
- Unit-mass output mixes total power four and five. Restore the scalar term's
  factor `m²`: in general a unit-mass relation from key `a` to key `b` carries
  `(m²)^(sum(b)−sum(a))`. Dropping this factor passes unit-mass tests but is
  wrong at a different common mass.
- A nonminimal finite output is sufficient. Measure preparation, expansion
  sparsity, first application, cache/storage effects and numerical acceptance
  before installing weighted replacements. A smaller label count alone is not
  a demonstrated reduction in application cost.

Kira 3 warns that high-rank numerator symmetry relations can become expensive;
that is a reason to keep this first test at the observed quadratic numerator
degree, not a reason to discard it. Its targeted equation selection and basis
checks also support reporting unresolved targets rather than assuming a
preferred basis is complete. [Kira 3, sections 3.1.1, 3.2 and 3.6](https://arxiv.org/pdf/2505.20197).

If this lane stalls, the orthogonal next experiment is exact projection of
additional integer IBPs through independently replayed existing rules. Explore
one-line supersectors and symmetry-related routings without promoting their
integrals into the preferred output block: higher-sector identities can expose
lower-sector relations. Kira's documented example reduces a sector's apparent
master count only after including a supersector; it also warns about unwanted
outside-sector masters and the need for a suitable ordering.
[Kira 1.2, section 3.13](https://arxiv.org/pdf/1812.01491).

Projection is not automatically a proof merely because its arithmetic is exact.
Each applied candidate relation needs independent original-source replay and
applicable guards; the source IBP, zero/alias witnesses and projection trace
together justify the terminal identity. Modular selection can choose work but
cannot replace these proofs. A zero projected row may simply repeat relations
already used to build the program. Importing FMFT expressions, inferring
relations from sampled catalog equality, or proving only maximal-cut equality
would not meet the requested independent terminal-reduction objective.

## Conditional delivery contract after a successful diagnostic

A separate core-owned native **terminal-output normalization sidecar** is a
sound narrow delivery boundary. Preserve the raw candidate program and raw
terminal-value catalog; do not mislabel weighted equalities as terminal values
or certified program closure. A dedicated payload kind can reuse the existing
native envelope/Atom-State transport without changing existing payload shapes.

Bind family fingerprint, ordering, exact raw terminal set and a versioned
preparation recipe. Store only claimed raw-to-canonical exact maps and their
coefficients; stored statistics, skip reasons or proof flags are not authority.
On cold loading, core reconstructs the support maps and exact combination
proofs under caller-owned limits, semantically compares the stored expansions
after native-state remapping, then seals the plan. Compare both ordered
coefficient maps even for zeros/constants. A raw-program content digest may be
useful deployment provenance, but is not mathematically necessary when the
equalities depend only on the bound family and raw keys.

For the first rollout, require all final output keys to be a subset of the
original raw declarations. Every raw key has an explicit proved expansion or
an identity fallback; canonical keys map to themselves with coefficient one.
Flatten composition with unit-weight aliases during preparation, forbid cycles,
and perform no proof, recursive expansion or new terminal discovery in Vakint.
Install atomically only into an empty candidate cache, as the existing alias
installer already requires. Failed installation must leave the owner unchanged.

Unsupported mathematical classes may remain identities with typed skip reasons.
Budget/native failures must not silently authorize any stored nonidentity map.
Track raw/admitted/skipped/resolved counts, canonical output union, expansion
terms and coefficients, support-map and solve work, plus decode/proof limits;
distinguish cumulative work from retained memory and native scratch allocation.
Preparation and native loading require trusted-generated provenance, not a
claim that the inner Symbolica parser is hardened against hostile binary data.

The current `CandidateDecomposition::common_mass_squared_power` already returns
`sum(output powers)−sum(original target powers)`. Preserving the original target
while flattening weighted **unit-mass** terminal expansions therefore produces
the required common-mass factor. Do not apply it a second time inside the
sidecar. Exact nonunit-mass tests, raw/coalesced map comparisons, cache tests,
malformed-sidecar failures and the unchanged numerical acceptance matrix remain
delivery gates. Saving this sidecar changes the vendored output-normalization
artifact materially while leaving raw rule/source provenance intact.
