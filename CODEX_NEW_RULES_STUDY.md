# New-rules study and stopping checkpoint

Later update: a separate full-scope campaign is now prepared at the user's
request; see the October2 follow-up at the end and its linked launch note.

Date: 2026-10-02. Development branch: `main`. This report supersedes older
suggestions to launch additional rule experiments immediately. The user asks
that the current experiments finish, findings and tested work be pushed, and
the agent stop. Production lifecycle remains exclusively user-controlled.

## 1. Executive conclusion

We have found several **exact and locally cheaper reduction identities**, but
have not established a new rule set that materially improves the complete
frozen five-loop QCD-renormalization campaign. In particular, fewer immediate
RHS terms repeatedly failed to predict fewer recursive obligations. The best
local reductions remain useful research leads, not a demonstrated production
upgrade. No newly optimized full-campaign input is recommended at this checkpoint.

The code checkpoint contains tested, optional mechanisms for changing rule
discovery, mathematical ordering and selection without hard-coding a topology.
The latest experiments reuse that engine and Symbolica's exact arithmetic.
They do not modify production, narrow the 116 required queries, replace the
67-owner pool, invent masters or discard inconvenient descendants.

Full scoped five-loop closure remains open. Complete four-loop controls and
the helper-free five-loop rank-zero control establish substantial capability.
They do not prove that the current broad symbolic worklist will stabilize on
the full physical numerator scope, or that it will do so within practical time.
There is no defensible completion ETA yet.

## 2. What the live campaign currently establishes

Read-only observation around 08:08 UTC on October2, from
`campaigns/five-loop-a1-epoch-frontier-repaired-20261001/runs/20261001T091314.583624Z/status.json`:

| Reported quantity | Value |
| --- | ---: |
| Elapsed session time | 82,506.6 s, approximately 22.9 h |
| Discovered domains | 186,055,103 |
| Local completions | 141,152,976 |
| Pending domains | 13,673,929 |
| Recorded recursively closed domains | at least 25,777,839 |
| Recorded closed starting roots | 13 / 67 |
| Reported frontiers | 0 |
| Process RSS | approximately 155.8 GB |
| Last-hour pending growth per completion | -0.110 |

The heartbeat was fresh and the process had no exit status. The recursive
closure snapshot was **2,529 seconds old**. Its count is a conservative,
scan-batched observation, not an instantaneous statement about recent closure.
Earlier that morning a new scan recorded about 2.59 million additional closed
domains. It is therefore wrong both to say nothing closes and to infer eventual
completion merely because the local pending queue is shrinking.

The 67-root display is not a count of independently proved required queries.
The actual acceptance target remains all 116 frozen required queries, with
their reachable obligations discharged and reusable output cold-loaded.
Auxiliary helper status and independently closed required scope must remain
distinguished. Zero frontiers says no missing-rule failure has been reported
on inspected work; it is not a termination certificate.

### Why finite physical inputs do not give the current queue a simple bound

Starting numerator, positive-power and index-difference bounds constrain the
physical requests. The solver also operates on symbolic regions, wider helper
domains and routed descendants. Thirteen auxiliary starting A domains in the
reviewed request are genuinely unbounded. More generally, repeatedly replacing
correlated regions by enclosing regions can introduce work beyond the finite
starting set. Actual code retains A/D information on many paths; this is not a
claim that it simply throws every physical bound away.

For illustration only, a recurrence can transfer power from one index to
another while preserving their sum. Independently bounding the two coordinates
can forget that correlation and enlarge a later request. This example explains
why finite input cardinality alone does not bound a symbolic worklist; it is
not a diagnosed proof that this exact recurrence causes the current campaign.
No general stabilization proof or observed full-physics completion was obtained.

## 3. Completed avenues and what they taught us

The table separates a local endpoint experiment from an unchanged complete
cohort. Compilation is never a solver timing. Detailed receipts and prior
commands are indexed in `CODEX_PROGRESS.md` and the linked research notes.

| Avenue | Actual evidence | Decision |
| --- | --- | --- |
| Source visitation / rule portfolios | A1 remains the production baseline. R-primary has repeated modest four-loop work benefits, but transfer and preparation costs limit its case. A lower-owner program swap explained 92.8% of an observed excess in Route inspections. | Rule quality matters; no universal winner. |
| Global absolute-degree-first comparator | Complete 4L domains -3.64%, without a timing win. A 5L local comparison gave -0.75% domains, +0.40% arm time and a changed terminal basis, 25 to 28. | Do not promote this order globally. |
| Coefficient-support refinement | One exact zero-face exclusion removed a false successor obligation, but splitting increased immediate inspection work. | No broad splitting optimization. |
| Exact composition / hidden zeros | Six extra zeros in a seven-child screen, but 439 to 433 retained terms and no decrease in harder endpoints. Earlier local-reduction-before-routing increased domains by 4.73%. | Cancellation is real; work saving is unproved. |
| Target-directed finite Laporta | 17 fixed targets solved and source-replayed; no zero-RHS shortcut. A joint 24-term block remained unreduced in the fixed 752-row bank. | Do not expand seeds blindly or call misses masters. |
| Permutation symmetry before elimination | Same bank: 1,322 to 1,306 columns, still no target-block pivot; none of the 24 difficult keys had its partner in that bank. | Bounded negative, not irreducibility. |
| Global paired dimensional recurrences | Fixed-d composites had 4,157 / 4,195 terms and over 2,200 harder targets. They were not oriented reduction rules. | Park dense global shifts at this target. |
| Local dimension-return / subgraph identity | Source-proved local 4L endpoint work fell roughly 28%; two 5L pairs fell 23.65-26.18% in domains. Full walk+cold time was mixed: +0.71% / -6.38%. | Strongest local work lead, not a full-campaign gain. |
| Installing those broader alternatives | Eager broad46 and narrow37 insertions increased complete 4L work. Partition-preserving broad46 also increased domains: 26,025 to 26,211; arm time 15.509 to 15.796 s. All cold checks passed. | Keep the optional capability, park these candidates. |
| Affine block averaging | Native exact maps were valid, but a 69-term row with 24 harder terms became 99/41 or 163/82. | Reject those averages; no recursive benchmark warranted. |
| Selective rank-one reflection | Exact 4L and 5L identities give six numerator-free terms. Local 4L work improves; 5L transfer is much weaker than the fanout reduction. | Useful distinct relation source; not a production upgrade. |

This is not evidence against Laporta, symmetry or dimensional identities in
general. It identifies where the tested variants spend more work than they
remove. Conversely, the positive local dimension-return figures cannot be
advertised as savings for all 116 five-loop queries.

## 4. Latest completed reflection experiments

### Mechanism, rather than another blind ordering sweep

A sector symmetry can preserve all active propagators while acting affinely on
numerator coordinates. It then gives a linear relation, not merely an alias
between integral keys. This distinction is emphasized in
[Duhr et al., sections 2.2 and 3](https://arxiv.org/html/2604.08332v1).
Our maps and relations were independently checked with RustRed's existing
native verification and Symbolica-backed finite expansion; the paper is not
their certificate.

For the actual four-loop FG sector, reflection `k3 -> k4-k3` exchanges D3/D7
and sends D9 to `D1+D3+D7-D4+D5+1-D9`. Equal powers on the exchanged lines
allow the parent with one D9 numerator to be isolated with constant pivot 2.
The complete six-term result has no numerator. No expanded odd monomial is
discarded individually, and dimension remains symbolic.

The point `[2,1,1,1,1,1,1,1,-1,0]` has A9/R1/D8 and lies in required
`phys-d8-a13-r5-1111111100`, whose bounds are A<=13, R<=5 and D=8.
Fresh saved-rule selection gives batch0/rule9. Its 35 original terms specialize
to 26 nonzero endpoints; none is zero-sector tagged. Native verification of
the map squared, the complete identity and all endpoint descents passes.

| 4L local successor-union quantity | Forward: saved / reflection | Reverse: saved / reflection |
| --- | ---: | ---: |
| Discovered domains | 7,150 / 6,274 | 7,250 / 6,315 |
| Native inspections | 5,787 / 5,245 | 5,679 / 5,167 |
| Successor events | 147,046 / 117,632 | 144,300 / 118,165 |
| Traversal seconds | 0.5610 / 0.4821 | 0.5548 / 0.4610 |
| Full walk process + cold verification seconds | 3.9564 / 3.5126 | 3.9800 / 3.9484 |

All four arms finish and pass complete native cold-All reinspection with
reference options off, using the same 16 owners and 508 routes. Domain savings
are 12.25% and 12.90%; whole-arm savings are 11.22% and 0.79%. These are
successor-only comparisons, excluding parent application and separately measured
proof preparation, not complete 58-query campaign timings.

### Five-loop transfer

The independently verified five-loop map is `k2 -> k3-k2`. It exchanges D2/D9
and sends D10 to `D4+D2+D9-D3+D15+1-D10`. The actual required point is
`[0,1,2,2,1,0,1,1,1,-1,0,1,0,0,1]`, A11/R1/D10. Fresh selection gives
batch0/rule35: 95 original terms become 64 nonzero endpoints, none zero-sector
tagged. Reflection gives six, reduces maximum rank 1 to 0 and maximum absolute
degree 12 to 11. All exact identity, involution and descent checks pass.

The forward pair uses the unchanged 67-owner / 8,246-route pool, including its
existing repair overlay. It does not clip descendants to the parent's physical
D10 slice: the D11 child is retained. Each fresh arm uses W16 on CPUs32-47.
Both arms finish and pass full cold-All/Off verification, including every
native inspection and all 64/six starting roots, with no violations, frontiers
or uncovered obligations.

| 5L local successor-union quantity | Saved64 | Reflection6 | Change |
| --- | ---: | ---: | ---: |
| Discovered domains | 66,991 | 66,321 | -1.00% |
| Native inspections | 51,422 | 52,260 | +1.63% |
| Successor events | 3,760,156 | 3,334,282 | -11.33% |
| Owner preparation, seconds | 66.4277 | 66.1994 | reported separately |
| Traversal, seconds | 21.2893 | 19.0958 | -10.30% |
| Full walk process + cold check, seconds | 181.6577 | 182.3609 | +0.39% |

The pair finishes in 368.5197s inclusive and all four process groups drain.
This is a **negative promotion result**: no meaningful reduction in total
domain work, more native inspections, and a slightly slower complete arm.
No reverse run was launched to chase a favorable timing. No full116 campaign
gain or mathematical closure claim follows from this local completed pair.

### What would be needed for reusable parametric rules

The successful relation is not installed as a generic parametric artifact.
A smallest useful chart fixes the exchanged powers to one, the selected
numerator power to minus one and other inactive powers to zero, while leaving
active spectator powers free. The checked symmetry fixes those spectators.
Broader equal-but-free exchanged powers need a diagonal equality, not a box.

A generic proposer can detect an aligned two-line subgraph from typed family
coefficients, propose its reflection, and let the existing verifier accept or
reject it. No topology name, fixed loop count, new tensor reducer or custom CAS
is necessary. However, the current ordinary-source producer does not certify
a symmetry-origin relation merely because concrete transport succeeds. A small
typed symbolic-chart proof boundary is missing. Its new cases must also be
tested for fragmentation when installed in the complete four-loop cohort.
That implementation is explicitly **not started at this stopping checkpoint**.

## 5. Code, validation and runnable checkpoint

Already implemented and pushed mechanisms include runtime mathematical-order
descriptors, source-visitation controls, exact original-source combination
checking, and optional partition-preserving alternate-rule dispatch. The latter
first partitions with ordinary rules and only substitutes an alternative valid
on an entire existing piece. It does not silently split a domain, erase an
exception, replace a terminal or turn an unknown applicability test into true.
Default/unmarked owner output remains unchanged. Marked policy metadata binds
the owner payload and is checked during loading and reuse.

Recent native validation includes 386 candidate-reduction regressions, 119
candidate-bundle tests (two external-data tests intentionally ignored), the
focused nine core and two app policy tests, complete unchanged 58-query
four-loop cold reinspection, and the local comparisons above. These are
specific executed suites, not a claim that every unrelated repository test
was run. Separate agents audited implementation, mathematics and raw results.

No production-engine source was changed in this final reflection investigation.
It used the same previously built optimized CLI throughout:

```text
SHA256 8ef80b52da13866dbe09d817acd004ec3fc9ca2130d85411ac758398086cf0a7
```

The executable is frozen locally at:

```text
/common/dev/rustred/TMP/releases/20261002-new-rules-study/rustred
```

It is byte-identical to the binary used in all latest comparisons. Its CLI
help smoke test passes. At finalization the cached optimized native suites
were rerun: **386 core tests pass; 119 app tests pass, two intentionally ignored**.
The relevant core/order/app source and Cargo inputs have no subsequent owned
changes. No expensive full rebuild is needed for documentation-only changes.

Use the root checkout's explicit-path Nix flake, not an untracked nested flake:

```bash
cd /common/dev/rustred
nix develop path:/common/dev/rustred --command \
  /common/dev/rustred/TMP/releases/20261002-new-rules-study/rustred --help
```

If rebuilding later, the tested build recipe is:

```bash
cd /common/dev/rustred
nix develop path:/common/dev/rustred --command env \
  CARGO_INCREMENTAL=0 \
  CARGO_TARGET_DIR=/common/dev/rustred/TMP/codex-runtime-discovery.280crc/target-native \
  cargo build --release --locked --offline -j8 -p rustred-app --lib --bin rustred
```

Keep the existing license in the environment; it is not saved in this report
or release metadata. The frozen binary is a development checkpoint, not a
recommendation to replace the production executable or regenerate owners.
The unrelated FeynKit worktree changes and pre-existing untracked files belong
to other work and are preserved, not staged into this checkpoint.

## 6. Assessment and next restart boundary

I would not promise that the currently running full-physics campaign will
finish. It is still making valid local progress, and many concrete sectors and
the rank-zero scope are demonstrably solvable. But the gap between local
completion and recursive required-query closure is large; root closure has
not demonstrated broad convergence. A shrinking pending queue alone cannot
resolve that uncertainty.

The strongest practical lesson is to optimize the **shared descendant graph**,
not just the first recurrence's term count. The 5L reflection removes almost
all immediate numerator-bearing branches, yet produces a comparable downstream
workload. Reuse of expensive lower-sector work is a plausible explanation,
not a demonstrated identity between the two entire graphs. Indiscriminate symmetry
averaging or wider shortcut cases can create additional obligations.

If work resumes, prioritize one bounded, generic symmetry-chart implementation
and an unchanged whole-cohort installation test, or a genuinely targeted mixed
symmetry/IBP combination for a measured expensive descendant block. Reuse the
existing exact/native services and maintain the earlier negative results. Do
not reopen large seed searches, scheduler redesign or higher-loop work merely
because this local relation is attractive. Terminal minimization, numerical
master evaluation and further Vakint work remain subsequent stages.

The current running campaign is neither stopped nor altered. No tested new
full-campaign rule input justifies asking the user to abandon its accumulated
work. The frozen development CLI is available for future controlled runs;
successful local tests do not establish compatibility with the large live
checkpoint, and no such live resume rehearsal was performed here. Any future
rule-payload change must respect checkpoint input identity rather than silently
reusing decisions made with different rules.

## October2 follow-up: separate full-scope launch prepared

At the user's subsequent request, the37-term rank-two identity has now been
exported into the actual five-loop owner and checked for native activation,
not merely tested from its descendants. All147 sources replay over seven free
positive outer powers; all36 descent cells pass, and the old fallback rules
and terminals remain. A separate full116-required-query/67-helper campaign is
prepared, **not started**. See the
[launch and validation note](docs/research/five_loop_new_rules_campaign_2026-10-02.md).
The local24–26% domain saving remains promising but is not a measured full-run
gain. The earlier stopping checkpoint and uninstalled-status statements above
describe the preceding experiments, not this later preparation.
