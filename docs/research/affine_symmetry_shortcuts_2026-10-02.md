# Affine sector symmetries as additional shortcut relations

Date: 2026-10-02. This investigation concerns rule quality, not master
minimization. Production is observed read-only. The complete four-loop and
frozen five-loop input scopes are unchanged.

## Why this differs from the preceding experiments

The preceding fixed 752-row Laporta diagnostic did not eliminate its nominated
24-term difficult combination. Quotienting its columns by the verified loop
exchange `k2 <-> k3` reduced 1,322 columns to 1,306, but still found no target
pivot. None of the 24 difficult terms had its exchanged partner in that matrix.
This is a bounded negative result, not a claim that those integrals are
independent or that additional identities cannot help.

A sector symmetry need only permute the active propagators. Its action on
auxiliary numerator coordinates can instead be affine. Such a symmetry gives
an exact relation between one integral and a finite linear combination, rather
than just an alias between two keys. This is the relevant distinction in
[Duhr et al., sections 2.2 and 3](https://arxiv.org/html/2604.08332v1).
The paper motivates this search; it does not verify the maps below or predict
their performance. No new group-theory, graph-canonicalization or CAS engine is
needed for the finite tests: RustRed already verifies momentum maps and uses
Symbolica to expand their numerator action.

## Two concrete falsifiers

### Rank-one reflection in the FG sector

Use the existing ten-denominator four-loop input, with active sector
`1111111100`. The reflection `k3 -> k4-k3` exchanges D3 and D7 and sends

```text
D9 -> D1 + D3 + D7 - D4 + D5 + 1 - D9.
```

When the two exchanged denominator powers agree and D10 is absent, the parent
with one D9 numerator occurs in its own transformed image with coefficient -1.
Moving it to the left gives coefficient 2 and a six-term rank-zero RHS. This
is a relation for the complete integral combination, not a claim that each
expanded odd monomial vanishes separately.

The first point is `[2,1,1,1,1,1,1,1,-1,0]`, with positive-power sum A=9,
numerator rank R=1 and index difference A-R=8. It belongs to the actual required
query `phys-d8-a13-r5-1111111100`. The native diagnostic must check containment,
fresh first-match selection, the full old RHS including zero-sector terms,
the map and its square, exact isolation, and every new term's saved-order
descent. Dimension stays symbolic. A smaller raw RHS is only a structural lead:
the old dispatcher may already discard some provably zero sectors.

### Affine reflection of the difficult owner481 combination

In sector `0111100001`, the map `S: k4 -> k1-k4` exchanges D4/D5 and fixes the
other active denominators. It preserves the old rule151 point
`[0,1,1,2,2,0,0,0,0,1]`, but its inactive action includes

```text
D6 -> 1-D1+D2+D4+D5-D6+D8
D7 -> 1-D1+D3+D4+D5-D7+D9.
```

It therefore was not covered by the preceding full-family permutation test.
For the 24-term difficult combination W, the integer-power inventory bounds a
single S expansion by 120 contributions before Symbolica collection. S commutes
with the earlier exchange P, allowing the separately tested averages
`(W+S(W))/2` and `(W+P(W)+S(W)+PS(W))/4`. All 45 remaining siblings and original
conditions stay present. Direct pair identification alone does not cancel the
two paired coefficients; only complete native collection can establish whether
the affine contributions help.

These are point-level diagnostics. There is no new source-bank expansion,
ordinary-IBP certificate, terminal, reusable rule or parametric closure claim.
Any later mixed symmetry/IBP relation must retain separate symmetry provenance;
the ordinary-source replay service cannot authenticate it by relabeling it.

## Acceptance and evidence

Exact validity comes first, then complete successor work using the same shared
owner/routing pool. Only a useful local result justifies insertion into an
unchanged full four-loop comparison and then a representative five-loop pilot.
Previous source-proved local shortcuts saved roughly 24-26% of five-loop
successor-domain work, but their broad four-loop insertions increased work.
That negative transfer is why term count alone is not the deployment criterion.

Ignored raw evidence is under
`TMP/postlaunch-20261002/tangent-integration/`: `reflection-r1-v1`,
`affine-block-v1`, and `symmetry-block-v1`. Compilation and solver timings are
recorded separately. The first reflection adapter compilation failed on a
JSON-macro recursion limit before any native test; the minimal compile-only fix
raises that limit. Results below must distinguish diagnostics from actual
campaign savings.

### Rank-one native result

The corrected adapter compiles against the cached optimized libraries in
122.6085s. The diagnostic finishes in 1.8182s inclusive; native loading and
checking take 0.09359s, including 0.01193s for reflection construction/replay.
The native required-query and fresh batch0/rule9 checks pass. The exact complete
RHS changes from 26 to six terms, maximum numerator rank from one to zero,
and coefficient display length from 92 to 19 bytes. Maximum absolute degree
stays nine. All six terms strictly descend in the persisted comparator.
The map squared is the identity and the complete isolated residual is zero.

Independent inspection confirms none of the 35 original saved terms has a
zero-sector flag; nine coefficients specialize exactly to zero, leaving the
26 baseline endpoints. Thus this is not a raw-count improvement caused by
retaining otherwise discarded zero sectors.

### Complete four-loop successor unions

Both counterbalanced pairs use the same optimized CLI, all 16 owners and 508
routes, W16 on CPUs32-47, fresh graphs and complete endpoint sets. Every arm
finishes and passes fresh-process native reinspection of every scheduled
domain with reference options off. No errors, uncovered obligations or
frontiers remain in these local unions.

| Quantity | Saved / reflection, forward | Saved / reflection, reverse |
| --- | ---: | ---: |
| Discovered domains | 7,150 / 6,274 | 7,250 / 6,315 |
| Native inspections | 5,787 / 5,245 | 5,679 / 5,167 |
| Successor events | 147,046 / 117,632 | 144,300 / 118,165 |
| Traversal, seconds | 0.5610 / 0.4821 | 0.5548 / 0.4610 |
| Whole walk process + cold check, seconds | 3.9564 / 3.5126 | 3.9800 / 3.9484 |

Domain work falls 12.25% and 12.90%; traversal falls 14.08% and 16.91%.
Whole-arm time falls 11.21% and 0.79%, so the sustained time gain is not yet
robust. The complete pairs take 8.7962s and 9.2193s inclusive. Scheduling can
change concrete graph counts between repetitions; exact byte identity is not
claimed. Parent application and the separately reported proof cost are outside
these successor-only timings. These are not the complete 58-query campaign.

Decision: retain this genuinely different local rule as a lead, not a deployment
recommendation. The next native test uses the corresponding actual five-loop
required R1 point and independently verifies its own reflection and baseline;
it must not import four-loop coefficients or assume a global family isomorphism.

### Affine averaging: valid but counterproductive

The second map and both complete averages pass native verification. However,
averaging is not a useful rule-selection policy for this block:

| Complete row | Terms | Terms increasing F relative to the parent |
| --- | ---: | ---: |
| Original | 69 | 24 |
| S-average of W, with original siblings | 99 | 41 |
| Four-element average of W, with original siblings | 163 | 82 |

All candidates still descend in the saved order and have maximum F=9 and R=2.
Exactly 120 first-image contributions are expanded; the whole verification
uses 289 transport calls and 649 contributions, taking 0.23914s natively and
1.60494s including the wrapper. Compilation takes 122.7847s separately.
The initial compiled adapter was not run: review found that a deliberate early
visitor stop would be returned as an error. Removing that stop lets the same
bounded scan finish; no mathematical condition or limit was changed.

Park these two averaging candidates; neither warrants a recursive walk. The
maps remain useful verified evidence, but using their relations selectively in
a mixed symmetry/IBP elimination is a different, untested proposal. None of the
24 W keys has a nontrivial self coefficient under S, so the successful rank-one
isolation mechanism cannot simply be copied onto this block. No source-bank
extension or proof of irreducibility follows from this negative result.

## Independent five-loop transfer

The actual required query `phys-d10-a14-r4-011110111001001` contains
`[0,1,2,2,1,0,1,1,1,-1,0,1,0,0,1]`, A=11, R=1, A-R=10. In this family the
native map is `k2 -> k3-k2`, exchanging D2/D9 and sending
`D10 -> D4+D2+D9-D3+D15+1-D10`. This is verified directly in the five-loop
family on its actual support, not through an assumed four-loop isomorphism.

Native fresh first-match selects batch0/rule35. All 95 original terms and
denominator guards are retained; 31 coefficients specialize exactly to zero,
leaving 64 endpoints. None is zero-sector tagged. Native map verification,
complete involution replay and isolation with constant pivot 2 produce six
strictly descending rank-zero terms with coefficients +/-1/2. Maximum absolute
degree falls from 12 to 11. The baseline has 54 rank-one and ten rank-zero
endpoints, so this is a genuinely different reduction.

The diagnostic takes 3.3581s inclusive, 1.00056s natively, including 0.00930s
for reflection construction/replay. Adapter compilation takes 124.3102s
separately. Independent audit accepts the raw result and drained process
groups. Original parent conditions remain required; their evaluation at child
indices is diagnostic only, because the parent rule is not being reapplied to
those children. No new rule has been published or installed.

The completed five-loop forward pair uses all 67 owners and 8,246 routes plus
the existing repair overlay. Both arms close and pass full native cold-All/Off
reinspection. Domains change 66,991 to 66,321 (-1.00%), native inspections
51,422 to 52,260 (+1.63%), successor events 3,760,156 to 3,334,282 (-11.33%).
Traversal falls 21.2893 to 19.0958s, but full walk-process plus cold-verification
time increases 181.6577 to 182.3609s (+0.39%). The whole pair takes 368.5197s.
These are still successor-only timings, not full116 performance or parent
application costs. No descendant is clipped to the parent's D10 slice.

Decision: park promotion; do not run a reverse pair to chase a timing when
domain work barely changes and native inspections increase. A generic reflection
proposer and symbolic-chart producer remain design-only. The user subsequently
requested stopping after this checkpoint; the consolidated report is
`CODEX_NEW_RULES_STUDY.md`. Production and its inputs remain unchanged.
