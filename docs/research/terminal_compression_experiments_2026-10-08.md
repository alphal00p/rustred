# Terminal-compression experiments — 8 October 2026

Work in progress. No unrestricted closure, minimality, or master-independence
claim is made. These experiments do not modify the user's live campaign.

## Inputs and authority

The four-loop control is the portable 28-raw/20-normalized-terminal publication
from the combined 58-query control. The five-loop input is the frozen
829-terminal publication of `five-loop-rank-ladder-THE-ONE`, scope `52afacba…`
(starting R≤2, D≤9). The latter is a symbolic inventory overapproximation, not
an independence proof or a census of precisely reachable concrete targets.

All elimination uses Symbolica's exact sparse rational-polynomial reducer.
The ordinary path generates its IBP equations directly. The assisted path also
specializes every applicable saved candidate rule and adds verified weighted
routing identities; it preserves conditions and unresolved integral columns.
Consequences of candidate rules retain their candidate provenance status.

The assistance queue is finite: requests come from terminal/seed keys and
ordinary-source support, not recursively from every saved equation's RHS.
This is deliberately one experiment, not a claim to have exhausted every
useful consequence of the saved rules.

## First measured results

Optimized executable SHA256:
`7bec0eedb2da0418e8c270fe058f85bbc80baf191ec100735af882e4e543ab83`.
Compilation took 10m09s and is excluded below. All runs used four reserved
workers; exact elimination is serial. Wall time includes loading, preparation,
checkpointing and publication. RSS is the sampled native-process high-water
value, not total host memory. These are single controls, not statistical speed
claims. Builds and other users' work were active on other CPUs.

| Input | Source depth | Saved assistance | Remaining | Wall s | CPU s | Peak RSS MiB | Outcome |
|---|---:|---|---:|---:|---:|---:|---|
| Four-loop | 0 | off | 19 | 0.254 | 0.246 | 14.0 | completed |
| Four-loop | 0 | on | 19 | 2.283 | 4.191 | 77.0 | completed |
| Four-loop | 1 | off | 19 | 1.722 | 1.703 | 58.1 | completed |
| Four-loop | 1 | on | 19 | 105.422 | 106.605 | 306.7 | completed |
| Five-loop | 0 | off | 829 | 12.072 | 11.967 | 354.4 | completed |
| Five-loop | 0 | on | **821 provisional** | 898.210 | 1004.573 | 4997.1 | interrupted, checkpointed |

Four-loop source depths zero and one contain 320 and 5,552 ordinary equations.
Both depth-one results were cold-reloaded in a fresh process, with unchanged
native-state identities and terminal counts. The original four-loop source
files were checked unchanged.

The five-loop assisted experiment was cooperatively stopped by the
orchestrator after progress became expensive, **not** because the finite
worklist had completed. Its checkpoint contains 5,777/20,725 ordinary rows,
107,643 saved-equation rows, 28,184 independent rows and 110,521 columns. It
reports `paused`; generation334 was saved in0.363s (15,121,579bytes). The
comparison harness expected completion and therefore asserted on the legitimate
pause exit-code4. This assertion is not a solver error. The external SIGINT was
sent by the orchestrator, not the harness's automatic-interruption flag.

Evidence is under `TMP/terminal-compression-20261008/`: `four-loop-pair-1`,
`four-loop-depth1-pair`, and `five-loop-pair-1`, including commands, events,
receipts, manifests and native states. The user's separate refinement process
was not signalled. No completed five-loop assisted timing or final reduction
from829 is claimed. The measured four-loop result is negative: this particular
assistance policy adds cost without further compression.

## Distinct strategies, not just deeper versions of the first

### Lower-loop and factorization reuse

An exact eligibility census found only **one** independent scalar tadpole
product in each input; both have all unit powers. Thus differently dotted
products cannot supply a pair of terminal labels to relate in these data.
That narrow proposal is parked without adding a zero-benefit production API.

A further exact-Symanzik diagnostic found five positive-support coloop
candidates among the four-loop20 labels and50 among the five-loop829 labels
(36 scalar,14 with numerators). None has a dotted isolated scalar factor or
an existing lowered-key pair. All14 numerator candidates retain their isolated
factor in the support including numerator slots: singleton factor reflections
therefore appear trivial. These are diagnostic candidates, not admitted
factorization or reflection identities. Connected-component and more general
weighted numerator symmetries remain under investigation using existing
verified momentum-map/transport machinery.

### Targeted containing-sector cancellation equations

Implemented a separate finite source-selection policy. For each raw terminal,
promote selected nonpositive indices directly to+1, retaining every other
index. One or more simultaneously promoted axes are configurable. These
ordinary-source points are **not** declared terminals. Directly promoting−3
to+1 differs from a small signed-L1 neighborhood, and can expose equations in
larger sectors whose combinations eliminate their extra integrals.

The exact selected seed inventory is persisted; completed rows survive
extension and resume. The method is topology/loop-count generic, retains source
conditions, and never clips generated auxiliaries to the starting rank.
Six new focused tests and an independent mathematical/code audit passed.
Native/Python controls are integrated. Frozen optimized executable SHA256
`0147466d76d8effb3a9adf335766a244c38880d92d072d88bce57c89df848186`
(11m08s compilation excluded) produced these complete four-loop controls:

| Promoted slots | Ordinary equations | Remaining | Wall s | CPU s | Peak RSS MiB |
|---:|---:|---:|---:|---:|---:|
| up to1 | 1616 | 19 | 0.659 | 0.638 | 24.9 |
| up to2 | 3248 | 19 | 1.114 | 1.089 | 42.5 |
| up to4 | 4832 | 19 | 1.972 | 1.945 | 61.8 |

Both results cold-resume in a fresh process to identical native states and
counts; source files remain unchanged. A separate real-SIGINT depth4 control
paused at3896source rows and resumed to a byte-identical final native state.
An attempted in-place policy change was rejected without changing checkpoint
files. Thus these selections do not improve
the four-loop control's compression. The five-loop one-slot-promotion run adds
4845seeds (141850ordinary equations in total), and completed with829remaining
after about669.5s. Thus it too is a negative compression result at this finite
source selection. Independent cold-resume verification is finishing separately.

### Weighted circuit equations

A more specific census found ten **already verified** symmetry generators on
two five-loop supports. Fourteen rank-one normalization attempts had failed
only because transformed positive integrals were absent from the declared
terminal list (`UnboundPositiveOutput`), not because the maps were invalid.
The proposed alternative admits the complete homogeneous equation
`I - T(I) = 0`, retaining every new integral as an auxiliary column. The same
native weighted transport also supports higher numerator powers and dots.
This is a distinct identity source, not a deeper signed-L1 search. Existing
bounded native Symbolica transport performs all expansion. Six focused tests,
60existing normalization regressions and independent core/application audits
passed. CLI/Python integration has independent flags for saved and circuit
assistance; circuit-only preparation never loads saved candidate owners.

**Completed positive pilot:** the broader finite bank (including dotted and
higher-rank inputs) prepares20verified maps on4supports,338nontrivial equations
for103targets, with61080retained terms. Combining these with the20725ordinary
depth-zero sources reduces the five-loop inventory **829→780**. All829raw
integrals were reduced to the remaining set, and their exact coefficient maps
matched after reloading the written native state. This is49proved relations,
not a minimality or independence claim. The four-loop control remains19.

This measurement uses the isolated generic Rust core driver, SHA256
`19e385ed8cec177881f6d6f5c6f80b07921167f315b684b6540af1f0259aec9a`, rather than
the application wrapper. Its47.159s total includes2.556s preparation,
14.234s search/checkpoint work, and30.368s exhaustive cold-load/application
verification. These boundaries are **not** directly interchangeable with the
application timings above. Evidence: `five-loop-circuit-core-depth0.log` and
its native state under the experiment directory. A matched core ordinary
baseline finished829→829 in35.359s:2.191s preparation,9.617s search/checkpoint,
23.551s exhaustive verification. Circuit equations therefore add about4.62s
of search/checkpoint work in this single comparison for49fewer labels.

An independent matched repeat used that same frozen driver and source native
state (`ab988251…`), sequentially on CPUs48–51 with inner pools fixed to one.
GNU time measured each complete native process, including exhaustive exact
warm/cold application checks for all829raw keys. Peak RSS here is the kernel
process high-water measurement, not the sampled RSS used in the earlier table.

| Frozen core mode | Remaining | Wall s | CPU s | Peak RSS MiB | Preparation s | Search/checkpoint s | Exact warm/cold verification s |
|---|---:|---:|---:|---:|---:|---:|---:|
| Ordinary depth0 | 829 | 30.95 | 30.73 | 339.5 | 2.090 | 9.492 | 19.203 |
| Circuit + ordinary depth0 | 780 | 41.99 | 41.73 | 467.3 | 2.372 | 13.939 | 25.426 |

The internal phase timings exclude a small amount of process startup/shutdown
included in external wall time. This repeat adds about4.45s search/checkpoint
work for the same49relations, not a statistical performance claim. A separate
cold-census audit loaded all four original outcomes (ordinary, circuit,
positive-dot lowering, and their combination), plus both repeated states.
Their exact remaining-key sets agree within the829and780groups; every state
is complete with no pending rebuild. No native-byte identity assumption was
used for this comparison. The removed-key set independently matches the
classification below. Source and checked output files remained unchanged.
Evidence: `TMP/terminal-compression-20261008/core-timed-repeat-zbmpmec7/`,
especially `summary.json` and `independent-audit.json`.

The49eliminated labels comprise35dotted scalars,11rank-one and3rank-two
numerators. All have six positive slots. This classifies outputs, not the
equation mechanism responsible: attributing the scalar reductions solely to
dot permutations would require a separate row ablation.

### Targeted positive-dot lowering

As a cheaper Laporta source-selection variant, add `a-e_i` only where `a_i≥2`.
This exposes raising terms targeting the original dotted integral without
adding every signed-L1 neighbour. The five-loop inventory needs154new points,
24575ordinary equations total, versus533825for full depth one. The completed
pilot remains829; its36.846s includes22.164s exhaustive cold verification.
Thus this narrow selection alone does not help. It is a variant of ordinary
Laporta seeding, not a new identity class. Its combination with circuit
equations also completes with780remaining, in50.177s including29.811s cold
verification; it contributes no further compression in this test.

### Larger normalization budget: an existing method was not being exercised

A direct audit found that conservative parametric normalization skipped all829
inputs before examining any support. The precise failure was prospective
Symbolica polynomial-product admission:20970terms requested against a20000
wrapper limit. The U polynomial has1398terms; constructing F also multiplies
it by the15-term common-mass polynomial, though this normalizer uses U.
The existing general Symanzik limits admit this calculation. Native colored
graph canonicalization already includes the actual propagator powers, and
every proposed alias is replayed by exact full-U equality; no new CAS or graph
implementation is needed.

With only that budget substitution, the public alias API finds164exact scalar
aliases in1.444s (557eligible scalar keys,63supports). Full normalization also
recovers14rank-one weighted projections, giving829→651 before IBP elimination.
The same generic core driver, separately frozen as
`86417bee4aa944246ea50a157918cc6c1df84c47d0b286ef33f050e0f9d9462a`,
was then used for fresh finite sessions. The driver modes use the same
public API and no additional relation source.

| Five-loop mode | Initially normalized | Ordinary rows | Remaining | Total s | Cold verification s |
|---|---:|---:|---:|---:|---:|
| Standard budget, ordinary |651|16275|613|54.780|31.902|
| Standard budget, circuit + ordinary |651|16275|608|64.157|36.673|

Both completed and checked exact warm/cold reductions for all829raw inputs.
These use fewer ordinary sources because normalization itself reduced the
seed inventory; they are not equal-row-budget comparisons. Four-loop standard
budget controls, with and without circuit equations, remain19. No independent
or minimal master-basis claim follows.

Integration must preserve the normalization policy used by each artifact:
the current native decoder regenerates the saved finite normalization plan.
Widening a global default silently would therefore invalidate valid old
checkpoints. Explicit persisted resource profiles and a fresh finite search
when changing profile are the planned application boundary; core defaults
remain unchanged during these experiments.

### Deferred performance aid

The assisted run has substantial redundant-row work. A native Symbolica
finite-field dependency prefilter could select which saved equations merit
exact elimination. This is an enabling performance option, not a new identity
class or an exact authority. See
[the audited design](terminal_modular_prefilter_2026-10-08.md). It is deferred
until the distinct source/symmetry experiments guide priorities. Global
dimensional-shift expansion remains deferred; reopen only for a sparse subgraph
case with concrete evidence.

## Validation and remaining work

The integrated application release
`3156153f950cd11ac44daf5a0e810f96c4840f16735975b498739db8703154b5`
passed matched CLI controls (compilation9m43s excluded):

| Input/mode | Remaining | Wall s | CPU s | Sampled peak MiB | Separate cold-resume s |
|---|---:|---:|---:|---:|---:|
| Four-loop ordinary |19|0.254|0.246|not reliably sampled|0.228|
| Four-loop circuit |19|0.305|0.277|not reliably sampled|0.228|
| Five-loop ordinary |829|12.253|12.154|351.0|1.645|
| Five-loop circuit |780|17.843|17.676|487.7|1.797|

Every result was inspected and cold-resumed in fresh processes to the same
native-state identity; complete source-directory hashes remained unchanged.
The CLI boundary includes package loading, refinement and publication, but
not the core driver's exhaustive application of all829raw inputs. Focused
native app/CLI tests24/24 and Python integration tests91/91 passed, along with
the418candidate tests,18finite-session tests and60normalization tests noted
above. The larger-normalization-budget driver result608 remains a separate
pilot until its explicit persisted policy is integrated and tested.

At the first slice, 418 candidate-reduction tests and18 app/CLI master tests
passed, together with Python lifecycle tests and12 finite-session tests.
The containing-sector slice adds six passing core tests (18 total). Separate
agents implemented and reviewed guards, weighted coefficients, aggregate
budgets, checkpoint transitions and Python mode isolation.

Still required: integration of the explicit normalization budget policy,
combined-strategy follow-up where evidence warrants it, and a coherent tested
commit/push. An assisted refinement cannot yet be imported by publish-only
scope extension; that path rejects rather than silently discarding its rows.
The source and refined package remain usable. This limitation must be resolved
before recommending the assisted workflow for progressive production scopes.
