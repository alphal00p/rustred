# Four-loop saved-rule coverage control

## Scope and acceptance

This is a control for the user-started five-loop traversal, not new IBP
generation. The required four-loop starting envelope is the analogous
conditional renormalizable full-UV-jet envelope: `A <= 19`, `R <= 12`,
`D = A-R >= 7`. All descendants must remain obligations, including those
outside that envelope. The gauge/forest assumptions of
[finite starting domains](../finite_starting_domains.md) still apply.

All four parent coordinate families are included: H (314 saved nonzero
sectors), X (328), BMW (134), FG (124). Their 900 sectors contain 59,636 saved
rules and 1,155 raw declared terminal keys. Family names identify experimental
inputs only; no topology-specific engine behavior is introduced.

Completion requires an exhausted descendant worklist, zero unresolved frontiers
or errors, drained workers, and discharged obligations. Initial-entry publication
and a zero-error prefix do not meet that condition. This test does not establish
original-source identities or certified unrestricted completeness.

Initially the user's running five-loop process was kept unchanged. The user
subsequently authorized stopping an impractical run and relaunching through
their Zellij tab after successful four-loop validation. At 20:34 UTC the
verified supervisor received cooperative SIGINT, requesting generation 13 of
its checkpoint; the native process was not killed. Generation 13 finished writing
39,286,209,131 bytes in 398.735 seconds with `paused=true`; orderly process
shutdown is tracked separately. At that point the backlog
had grown at every hourly sample (7.83 million at one hour, 27.34 million at
eleven hours), with about 231 GB RSS. This supports abandoning the inefficient
baseline; it is not a proof of mathematical nontermination. Checkpoint completion
and a new-input restart must be recorded separately.
Four-loop controls use six separate physical cores (50–55), sequentially by
family, with a 100 GB local RAM guard and reserved headroom for the live
five-loop job. The executable is the same frozen release binary as the live
job, SHA-256 `5746feb1630341b5d4cd8a7c211c823b0c8d96c6813d28f164f4ae0b91c248bb`.
There is no elapsed-time or cumulative-work cutoff.

## Uniform native transport

Both the existing four-loop monoliths and the five-loop owner shards use the
same generic native binary container and Symbolica state/atom codec. The current
selected-owner loader unnecessarily requires single-sector inputs. A temporary
transport-only repack selects sectors without changing coefficient frames,
roots, orderings, rule formulas, terminals, or rank policy. An independent audit
checked all 900 shards and 640,962 copied native frames. This is not a new
four-loop format, and no experimental query restriction enters the saved rules.

The rules' intended unrestricted scope remains distinct from this bounded
starting-domain test. Vakint's eventual re-export must retain unrestricted
packages and their existing candidate/certified status.

## Initial FG traversal and bottleneck

The first finite-envelope FG traversal was checkpointed and stopped cleanly
after 1,352,311 completed native inspections. It had 764,122 pending logical
domains, zero observed frontiers and zero errors. Supervisor wall time was
1,035.244 seconds, including checkpointing and shutdown; child user plus system
CPU was 2,634.492 seconds, with 17,381,628 KiB peak RSS. This is a stopped prefix,
not a completion timing or proof of nontermination.

A 20-second, 49 Hz user-CPU profile of that process collected about 2,000 samples
without lost samples. `DomainPowerSummary::contains` accounts for 48.33% of
sampled exclusive CPU across thread groups, and admission-thread
`AggregateIndex::find_controlled` another 10.45%. Coordinator preparation and
ordered commitment consumed about 90% of recent coordinator wall time.
The demonstrated problem is overlapping-domain admission and containment,
not expensive new symbolic IBP discovery. A growing backlog gives no credible
time-to-completion estimate.

## Full-orthant falsifier

Submitting all 124 FG sectors as unrestricted orthants exercises the same native
guard, RHS, descent and terminal handling, while making successor containment
cheap. The complete native session took 2.796105 seconds: 1.166803 seconds
preparation plus 1.629302 seconds traversal. Peak RSS was 196,632 KiB. All 124
inspections genuinely ran; 359,994 successors reused admitted orthants and no
general containment comparisons were needed.

**This stronger cover is incomplete.** There are 85 unresolved dispatch guards
on 13 owners: 25 equality and 60 excluded-conjunction cases. The ledger correctly
reports 111 locally discharged obligations and 13 blocked obligations, not
closure. There are no failed inspections, missing-support errors or routing
transitions.

Every unresolved box has an exact maximum `D <= 5`, outside the original
`D >= 7` starting envelope. Descendants may nevertheless enter these boxes, so
the frontiers cannot simply be dropped. Likewise, the 111 locally clear sectors
are not automatically a closed subgraph: their escaping dependencies must still
be inspected. Failure of this broader auxiliary cover is not evidence of a
missing rule in the requested finite starting scope.

The next experiments retain every original finite query while testing selective
larger anchors and bounded refinements. Any claimed improvement must finish the
entire resulting dependency workload, not merely its original entries.

## Evidence

Workspace-only raw evidence is under
`TMP/four-loop-saved-descendants.VaNmUN/`; the transport audit is under
`TMP/four-loop-region-control.eazKG2/`; the profile is under
`TMP/four-loop-fg-live-profile.uUttDy/`. These directories are deliberately not
repository inputs. The full-orthant independent audit records result SHA-256
`aaf00ccccf8617c0a0ede8c2dcdc935348157a75cfddd35a686fe45ef8ad7390`.

## Finite-rank auxiliary covers

The next FG experiment retained all 124 original queries verbatim and added 124
full orthants bounded only by numerator rank 12. The same native walker genuinely
finished: 98,869 native inspections plus 40 discharged aliases, zero pending
domains/frontiers/errors, and a fully discharged ledger. Independent record-level
audit passes. Preparation took 1.174756 s and traversal 13.264753 s, for
14.439510 s native session time and 18.055 s supervisor time. Peak RSS was
1,123,764 KiB; child CPU was 44.395978 s.

The auxiliary rank limit was **not** a descendant cutoff: 87,348 native
inspections had rank 13 and 11,273 rank 14. All were retained and discharged.
The strategy checks a larger set than the original FG starting envelope, so
its complete result also covers that required envelope. This is not a matched
completed-run speed ratio against the unfinished baseline.

The same rank-only strategy on BMW exhausted in 6.870613 s of traversal plus
1.765804 s preparation, but left 14 guards unresolved in a single auxiliary
owner with unbounded positive powers. That is failure of the auxiliary cover,
not yet a demonstrated gap in the required entries. The next experiment bounds
both positive power and numerator rank in the auxiliary anchors while preserving
every original query and every escaping descendant.

The successful BMW refinement bounds positive powers only on the unresolved
sector's upstream cardinality cone: all sectors with at least six active
propagators get auxiliary `A <= 19, R <= 12` regions, while lower sectors retain
rank-only auxiliary regions. The six is derived from this input's unresolved
guard owner, not an engine constant. Native accepted transitions cannot increase
sector cardinality in this common ordering, and no basis-changing routes occur.
Every original query and every escaping descendant remain obligations.

BMW then completes in 32.181261 s traversal plus 1.488346 s preparation, with
147,233 native inspections, 11,718 discharged aliases, zero pending work and zero
frontiers. The largest scheduled rank is 13. Supervisor time is 38.072 s and
GNU whole-command time 38.53 s; peak RSS is 1,771,680 KiB. The raw result reports a
fully discharged ledger; independent record-level review passes, including
capped-owner descendants with positive-power bounds up to 31. Thus neither 19
nor 12 was used to clip required descendants.

## Complete four-parent control

All four final controls now complete and pass independent record-level review.
Every original query is retained; all auxiliary regions are actual inspected
obligations. All results have zero frontiers, errors, pending work or unresolved
ledger obligations, and their worker pools are drained.

| Family | Native inspections | Traversal (s) | Preparation (s) | Whole command (s) | Whole CPU (s) | Peak RSS (KiB) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| FG | 98,869 | 13.265 | 1.175 | 18.52 | 44.90 | 1,123,764 |
| BMW | 147,233 | 32.181 | 1.488 | 38.53 | 103.14 | 1,771,680 |
| H | 24,680 | 12.254 | 2.538 | 16.50 | 40.89 | 636,764 |
| X | 46,826 | 32.114 | 4.352 | 38.51 | 96.37 | 1,217,988 |

The four successful whole-command spans sum to 112.06 seconds. This excludes
the negative exploratory runs, which remain part of the investigation record
above. Each family runs separately with six workers and the same frozen native
binary. These are shared-host observations while five loops ran or checkpointed;
they are not a matched completed-run speed ratio or a five-loop ETA.

H and X use rank-only anchors as FG does; only BMW needs the data-selected
positive-power fallback. Across the four controls, 317,608 native inspections
and 12,374 delegated obligations are discharged. Descendants above the original
rank and positive-power envelope are explicitly present. Coverage concerns the
specified starting sets and all their saved-rule descendants, not unrestricted
IBP-identity certification or an arbitrary routed family's termination proof.

The generic Python staging helper now exposes optional auxiliary rank/positive
power bounds and an explicit selected-owner list for the latter. The defaults
remain byte-preserving staging with no anchors. It retains the exact original
query bytes, leaves all original query objects in order, and submits auxiliary
regions through the unchanged native guard/descent/dependency pipeline. Changed
input requires a new campaign: an old checkpoint cannot be silently rebound to
the augmented query set. No new CAS operation or loop-specific engine was added.

The focused and existing Python steering suite passes 66 tests; an independent
implementation audit also passes. A second complete pass uses exactly the same
inputs and executable. Independent full-record comparison finds identical
geometry, native/guard/dependency counters and final outcomes, apart from timing,
checkpoint bookkeeping and scheduling-resource diagnostics. Its traversal times
are FG 12.905 s, BMW 31.847 s, H 12.749 s and X 32.044 s; the whole-command spans
sum to 109.94 seconds. These two completed passes establish reproducibility, not
a statistical machine-independent benchmark.

The revised five-loop input has passed preliminary matching and was launched
in the user's existing Zellij tab at 21:06 UTC. Its recursive result remains open.
The unrestricted Vakint package refresh and matched numerical/performance gates
are now in progress; they must not inherit this control's entry restrictions.

## Repeat on freshly generated Vakint packages

The later Python producer example regenerates the unrestricted rule programs
from native sources, changing the historical 59,636-rule inventory to 59,509
rules while preserving all 900 sectors and 1,155 raw terminals. Consequently,
the preceding receipts are not silently attached to the new packages. A separate
complete pass uses those fresh programs with the exact same successful query
bytes and native options; only filesystem paths differ.

Independent full-record review again finds 317,608 native inspections plus
12,374 delegated obligations, all 1,800 initial queries inspected, every ledger
responsibility discharged and drained worker/escrow state. Errors, frontiers,
pending work, problems and both unsupported support-transition counters are zero.
FG/BMW/H/X traversal takes 12.956/31.698/12.472/32.316 seconds; whole-command
spans sum to 108.35 seconds. No new timing speedup or unrestricted certificate is
claimed. See [the package refresh record](vakint_native_refresh_2026-09-24.md)
and workspace evidence in `TMP/fresh-four-loop-control.ouqfIS/`.
