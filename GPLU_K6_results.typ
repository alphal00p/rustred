#let navy = rgb("#17324d")
#let blue = rgb("#2878b5")
#let cyan = rgb("#dff3fa")
#let green = rgb("#2b8a66")
#let pale-green = rgb("#e5f5ed")
#let orange = rgb("#c66a18")
#let pale-orange = rgb("#fff0df")
#let red = rgb("#b94646")
#let pale-red = rgb("#fde9e7")
#let purple = rgb("#7253a6")
#let pale-purple = rgb("#eee8f7")
#let grey = rgb("#677386")
#let pale-grey = rgb("#f2f5f7")
#let line-grey = rgb("#cdd5dc")
#let ink = rgb("#172033")

#set document(
  title: "What happened when RustRed tried K6",
  author: "RustRed performance and closure study",
  date: datetime(year: 2026, month: 9, day: 9),
)

#set page(
  paper: "a4",
  margin: (top: 18mm, bottom: 19mm, left: 19mm, right: 19mm),
  footer: context {
    set text(size: 7.4pt, fill: grey)
    line(length: 100%, stroke: 0.35pt + line-grey)
    v(3pt)
    grid(
      columns: (1fr, auto),
      [RustRed · GPLU/SpIReD K6 measurement note · 9 September 2026],
      [#counter(page).display("1")],
    )
  },
)

#set text(font: "DejaVu Sans", size: 9.25pt, fill: ink, lang: "en")
#set par(justify: true, leading: 0.61em)
#set heading(numbering: "1.")
#set list(indent: 1.2em, body-indent: 0.52em, spacing: 0.33em)
#set enum(indent: 1.2em, body-indent: 0.52em, spacing: 0.33em)
#set table(inset: 4.5pt, stroke: 0.4pt + line-grey)
#show heading.where(level: 1): it => block(above: 1.5em, below: 0.7em)[
  #set text(fill: navy, weight: "bold")
  #it
  #line(length: 100%, stroke: 0.8pt + blue)
]
#show heading.where(level: 2): it => block(above: 1.15em, below: 0.45em)[
  #set text(fill: navy, weight: "bold")
  #it
]
#show heading.where(level: 3): it => block(above: 0.85em, below: 0.32em)[
  #set text(fill: blue, weight: "bold")
  #it
]
#show link: set text(fill: blue)

#let panel(title, body, fill: pale-grey, stroke: line-grey) = block(
  width: 100%,
  inset: 9pt,
  radius: 4pt,
  fill: fill,
  stroke: 0.7pt + stroke,
)[
  #text(weight: "bold", fill: navy)[#title]
  #v(3pt)
  #body
]

#let tag(label, fill: cyan, color: navy) = box(
  inset: (x: 6pt, y: 3pt),
  radius: 10pt,
  fill: fill,
)[#text(size: 7.7pt, weight: "bold", fill: color)[#label]]

#let step(number, title, body, color: blue) = block(
  width: 100%,
  breakable: false,
  inset: 8pt,
  radius: 4pt,
  fill: color.lighten(88%),
  stroke: 0.7pt + color.lighten(25%),
)[
  #grid(
    columns: (24pt, 1fr),
    gutter: 7pt,
    align: (center, top),
    circle(radius: 10pt, fill: color)[#text(fill: white, weight: "bold", size: 8pt)[#number]],
    [*#title*\
     #body],
  )
]

#let verdict(label, body, color, fill) = block(
  width: 100%,
  inset: 10pt,
  radius: 5pt,
  fill: fill,
  stroke: 0.9pt + color,
)[
  #text(weight: "bold", size: 10.2pt, fill: color)[#label]
  #v(4pt)
  #body
]

#let arrow = text(size: 13pt, fill: grey)[→]
#let yes = text(weight: "bold", fill: green)[hit]
#let no = text(weight: "bold", fill: red)[no hit]

#align(center)[
  #v(10mm)
  #tag[MEASURED K6 STATUS]
  #v(7mm)
  #text(size: 25pt, weight: "bold", fill: navy)[
    What happened when RustRed tried K6
  ]
  #v(4mm)
  #text(size: 13pt, fill: grey)[
    Keeping the hour-long legacy control separate from the new GPLU/SpIReD test
  ]
  #v(11mm)
  #block(
    width: 89%,
    inset: 13pt,
    radius: 6pt,
    fill: pale-red,
    stroke: 0.9pt + red,
  )[
    #text(size: 12pt, weight: "bold", fill: red)[K6 is not closed yet]
    #v(4pt)
    The first new modular sweep inspected all six root-sector representatives
    at depth two in about *2.14 seconds*. That was not a closing artifact. A
    later targeted diagnostic did admit and commit one exact generic-root
    owner for `011111` from its 136-row trace. It also produced one unresolved
    guard child, while four other roots still had no depth-two modular hit.
    An audit additionally found a carrier mismatch in the owner-cover delta,
    so its geometric coverage observation awaits a corrected rerun.
  ]
  #v(9mm)
  #grid(
    columns: (1fr, 1fr),
    gutter: 9pt,
    panel([Legacy control], [
      *1:00:18 wall time*\
      about *119.5 CPU minutes*\
      about *752 MiB resident memory*\
      no completed report or artifact
    ], fill: pale-orange, stroke: orange),
    panel([New root diagnostic], [
      *2.137 s total*\
      6 symmetry-orbit roots\
      2 modular hits, 4 bounded misses\
      0 authenticated rules, 0 closed roots
    ], fill: cyan, stroke: blue),
  )
  #v(13mm)
  #text(size: 8.8pt, fill: grey)[
    The two measurements exercise different engines and must never be merged
    into one speedup claim.
  ]
]

#pagebreak()

#outline(title: [Contents], indent: 1em)

#pagebreak()

= Executive answer

The present evidence answers three different questions:

#table(
  columns: (1.35fr, 0.72fr, 3.15fr),
  table.header([*Question*], [*Answer*], [*Evidence*]),
  [Can the new front end scan a useful finite K6 source window quickly?],
  [#text(weight: "bold", fill: green)[Yes]],
  [All six root representatives, four probes each, and all signed-L1
    translations through depth two completed in 2.137 seconds with an early
    exact-support cap.],
  [Did it discover any target-producing modular dependencies?],
  [#text(weight: "bold", fill: green)[Yes]],
  [It found candidates for sectors `001011` and `011111`, at the second
    configured probe.],
  [Did RustRed produce even one new authenticated K6 recurrence in this run?],
  [#text(weight: "bold", fill: red)[No]],
  [The dependency traces had 560 and 136 translated source rows. The cap of
    eight stopped them before exact symbolic materialization.],
  [Has a later targeted diagnostic admitted an exact K6 recurrence?],
  [#text(weight: "bold", fill: green)[Yes]],
  [The `011111` generic-root trace was admitted exactly and one owner was
    committed. Its guard child remains pending, and the audited carrier
    mismatch makes the reported cover delta provisional.],
  [Is the complete K6 family closed?],
  [#text(weight: "bold", fill: red)[No]],
  [The original sweep left every sector `OwnerFree`. The later single-owner
    result neither discharges its exceptional guard nor the four bounded
    misses, and no global artifact exists.],
)

#panel([The central performance finding], [
  For this bounded experiment, *modular discovery is not the slow part*.
  The first candidate in `001011` is algebraically far too large to be a useful
  “compact” exact lift: its dependency closure contains 560 rows. An uncapped
  attempt on that same first orbit ran for more than seven minutes and exceeded
  700 MiB before it was interrupted. The comparison strongly implicates exact
  compact-frame construction/materialization and rational-polynomial
  expression swell. It does not yet prove a finer phase attribution because
  that interrupted run had no internal stage timestamps.
], fill: pale-orange, stroke: orange)

There is a second, independent issue. For four orbit representatives, the
depth-two search produced no modular target dependency at any of four probes.
Those are *bounded misses*, not mathematical no-go results. They say that this
ordering and this small translation window did not reveal a relation. They
motivate fair deeper shells, alternative orderings, source preconditioning,
and independent probe scheduling.

= What K6 means here

RustRed writes the three-loop complete single-scale vacuum family as

$ I(n_1, n_2, n_3, n_4, n_5, n_6). $

The six integer coordinates describe propagator or irreducible-numerator
powers in a complete scalar-product basis. Hence the shorthand *K6*: this is a
six-coordinate family, not a six-loop calculation. The present pressure case
is a *three-loop* family.

A sector mask such as `001011` records which of the six lines are active. With
one-based positions, `001011` means that coordinates 3, 5, and 6 are active.
Its corner integral is

$ I(0, 0, 1, 0, 1, 1). $

The authenticated graph symmetry groups the non-scaleless full-rank sectors
into six orbits:

#table(
  columns: (0.42fr, 1.30fr, 0.70fr, 0.85fr, 2.15fr),
  table.header([*No.*], [*Representative*], [*Orbit size*], [*Active lines*],
    [*Meaning in this experiment*]),
  [1], [`001011`], [12], [3], [root searched; large modular candidate found],
  [2], [`001101`], [4], [3], [root searched; no depth-two hit],
  [3], [`001111`], [12], [4], [root searched; no depth-two hit],
  [4], [`011110`], [3], [4], [root searched; no depth-two hit],
  [5], [`011111`], [6], [5], [root searched; modular candidate found],
  [6], [`111111`], [1], [6], [root searched; no depth-two hit],
)

“Full rank” here excludes sectors already removed by the elementary scaleless-
loop proof. It is not an assertion that every listed integral is analytically
nonzero. Nor are these six masks six separate hard-coded topologies: they are
canonical representatives under the authenticated slot action.

== Sector coordinates and the unresolved rays

It is useful to replace signed powers by nonnegative sector coordinates
$x_i$:

$
  x_i = cases(
    n_i - 1 & "if line " i " is active",
    -n_i & "if line " i " is inactive".
  )
$

The sector corner is $bold(x)=0$. A rule that owns a positive-dimensional
region should reduce every lattice point in that region to strictly lower
points. A single explicit terminal at the corner does not do this.

For `001011`, two elementary rays are

$
  R_("active") = { I(0,0,1+t,0,1,1) : t in NN },
$
$
  R_("numerator") = { I(-t,0,1,0,1,1) : t in NN }.
$

The first raises an active power; the second deepens an inactive numerator.
There are analogous coordinate rays in all six directions, plus mixed planes
and cones. Before an exact rule is admitted, all of these remain part of the
unowned positive-dimensional region.

#align(center)[
  #block(width: 83%, inset: 8pt, fill: pale-grey, radius: 4pt)[
    #align(center)[
      #text(size: 8pt, fill: grey)[two-dimensional picture of a six-dimensional sector]
      #v(6pt)
      #grid(
        columns: (auto, auto, auto, auto, auto),
        gutter: 7pt,
        align: center,
        [#text(fill: blue)[●] corner terminal],
        [#arrow],
        [#text(fill: red)[●—●—●—…] uncovered ray],
        [#arrow],
        [#text(fill: red)[uncovered planes/cones]],
      )
      #v(4pt)
      #text(size: 8.2pt)[A terminal owns one point. A parametric rule must own
        an entire region and descend from every point in it.]
    ]
  ]
]

The diagnostic report calls each remaining root `OwnerFree` and reports one
uncovered box with `uncovered_finite=false`. In this vocabulary, “finite” means
that the residual has collapsed to finitely many *fixed terminal keys*, not
merely that machine-representable `i64` coordinates impose a remote safety
endpoint. Mathematically, the family still contains unbounded ray-like
directions. Because no candidate crossed the exact authority boundary, the run
closed *none* of them.

The complement is a cone-like lattice region, so there is not a short finite
list of “the rays.” The following table exhibits one coordinate ray inside
each still-uncovered orbit root. Every $t in NN$ is a different integral, and
mixed changes of two or more powers give further planes and cones.

#table(
  columns: (0.9fr, 2.65fr, 1.3fr),
  table.header([*Orbit*], [*One explicit unresolved ray*], [*Root result*]),
  [`001011`], [$I(0,0,1+t,0,1,1)$], [modular hit only],
  [`001101`], [$I(0,0,1+t,1,0,1)$], [bounded miss],
  [`001111`], [$I(0,0,1+t,1,1,1)$], [bounded miss],
  [`011110`], [$I(0,1+t,1,1,1,0)$], [bounded miss],
  [`011111`], [$I(0,1+t,1,1,1,1)$], [modular hit only],
  [`111111`], [$I(1+t,1,1,1,1,1)$], [bounded miss],
)

For masks with inactive positions there are also numerator rays, obtained by
replacing one inactive zero by $-t$. Even the two “modular hit” rows in the
table remain uncovered: proposal evidence does not install an owner.

= Two experiments, not one

The most important interpretive rule for these measurements is that the
hour-long run and the 2.137-second run are different programs of work.

#grid(
  columns: (1fr, auto, 1fr),
  gutter: 7pt,
  align: (top, center, top),
  panel([A · Legacy owner-cover control], [
    `campaign run-waves`\
    old `ProbeCampaignAdapter` path\
    bounded atomic orbit waves\
    no SpIReD target stream\
    no useful completion report in one hour
  ], fill: pale-orange, stroke: orange),
  [#text(size: 18pt, fill: red)[≠]],
  panel([B · New SpIReD root diagnostic], [
    one equality-root attempt per orbit\
    signed-L1 source shells\
    dual modular reducers\
    direct dependency traces\
    exact lift stopped at eight rows
  ], fill: cyan, stroke: blue),
)

Therefore:

- the one-hour control does not measure GPLU/SpIReD throughput;
- the 2.137-second sweep does not measure full K6 closure;
- dividing one time by the other would be a meaningless speedup number;
- neither run produced a cold-loadable K6 artifact.

== Experiment A: the hour-long legacy control

The command requested four workers and launched the previous bounded
full-rank-wave campaign. The first canonical wave has width two, so only two
tasks were available. The process consequently stayed near 198% aggregate CPU
instead of saturating four cores.

#table(
  columns: (1.22fr, 1.05fr, 3.1fr),
  table.header([*Observed quantity*], [*Value*], [*What it establishes*]),
  [Wall time before interruption], [1:00:18], [The first useful completion
    boundary was not reached in one hour.],
  [CPU time], [about 119.5 min], [Two cores were effectively occupied for most
    of the run.],
  [Aggregate CPU], [about 198%], [Consistent with the first wave's width of two,
    despite `n_cores=4`.],
  [Resident memory], [about 752 MiB], [The old campaign retained a sizeable
    working set, but did not show runaway growth during the later observation.],
  [Completed stdout report], [none], [No per-phase or per-orbit result crossed
    the reporting boundary.],
  [Written artifact], [none], [No closure result exists from this run.],
)

Operational snapshots showed roughly 667 MiB after about 74 seconds and a
working set near 752 MiB by approximately 8.5 minutes. At that point the
process had accumulated about 17 CPU minutes, again consistent with two busy
workers. It continued without a task-completion report until manually stopped.

#verdict([What happened during that hour], [
  Two first-wave legacy tasks ran continuously inside the old owner-cover
  campaign. They did not finish far enough to emit the expected report or
  artifact. The external process observations establish total time, CPU use,
  concurrency, and memory. They do *not* reveal how much of the hour belonged
  to modular sampling, exact algebra, cover compilation, or another internal
  phase. Any finer attribution would be invented.
], orange, pale-orange)

This control was useful precisely because it exposed two engineering problems:
coarse wave width underuses the requested worker budget, and progress emitted
only at a large task boundary makes a long run opaque. It is not evidence
against the mathematics of case-directed completion.

== Experiment B: the new six-orbit root diagnostic

The new release diagnostic was designed to answer a narrower question quickly:
*what happens at the first generic equality case of every K6 orbit when the
SpIReD target stream is actually exercised?*

Its fixed inputs were:

- nine freshly generated ordinary K6 IBP source rows;
- canonical target $I(bold(n))$, i.e. zero structural shift;
- signed-L1 translations through depth two;
- four case-valid finite-field probes;
- a fresh exact predecessor/owner ledger for each orbit;
- at most 32 post-hit streamed rows and eight distinct compact-lift attempts;
- an ingress cap of eight selected exact rows;
- a release build, with compilation excluded from the reported 2.137 seconds.

For six translation coordinates, the number of offsets is

$ 1 + 12 + (12 + 4 binom(6,2)) = 1 + 12 + 72 = 85. $

There are nine ordinary sources, so one complete depth-two probe streams

$ 85 times 9 = 765 " rows". $

Four complete no-hit probes therefore stream $4 times 765 = 3060$ rows. This
arithmetic is visible in the measured census and is a useful check that the
fair shells really completed.

=== The four probes

#table(
  columns: (0.42fr, 1.25fr, 0.72fr, 2.65fr),
  table.header([*No.*], [*Prime*], [*$d$*], [*Index chart point*]),
  [1], [`998244353`], [29], [`(0, 0, 0, 0, 0, 0)`],
  [2], [`998244353`], [43], [`(1, 1, 1, 1, 1, 1)`],
  [3], [`1000000007`], [29], [`(2, 3, 5, 7, 11, 13)`],
  [4], [`1000000009`], [43], [`(13, 11, 7, 5, 3, 2)`],
)

Using the same prime in the first two probes is intentional here because the
index and dimension specialization differs. A production fairness schedule
must eventually vary both points and primes rather than growing source depth
forever at one unlucky specialization.

= How the modular GPLU-style search is applied

Each translated ordinary identity has the form

$
  sum_s c_s(d, bold(n) + bold(a))
  I(bold(n) + bold(a) + bold(s)) = 0,
$

where $bold(a)$ is the source translation and $bold(s)$ is a structural shift
inside one ordinary IBP row. At a modular probe, RustRed evaluates
$c_s(d,bold(n)+bold(a))$ directly in a prime field. It does not first construct
the exact shifted rational polynomial.

The exact owner snapshot and ordering classify every integral column into
three roles:

#grid(
  columns: (1fr, 1fr, 1fr),
  gutter: 7pt,
  panel([Target], [The stable logical column for $I(bold(n))$ that this case is
    trying to solve.], fill: cyan, stroke: blue),
  panel([Allowed], [A term already justified as lower or previously owned. It
    need not be cancelled by the new combination.], fill: pale-green, stroke: green),
  panel([Forbidden], [A non-target term that is not yet legal on the right-hand
    side. The combination must cancel it.], fill: pale-red, stroke: red),
)

Allowed columns are omitted from discovery. Every row is streamed once into
two incremental Symbolica sparse reducers:

#align(center)[
  #block(width: 94%, inset: 9pt, fill: pale-grey, radius: 5pt)[
    #grid(
      columns: (1.1fr, auto, 1.35fr, auto, 1.35fr),
      gutter: 7pt,
      align: center,
      panel([Translated row], [$c_F$ on forbidden columns\
        plus target coefficient $c_T$], fill: white),
      [#arrow],
      panel([Reducer F], [forbidden columns only\
        measures $"rank"(F)$], fill: pale-red, stroke: red),
      [#arrow],
      panel([Reducer F+T], [same forbidden columns\
        plus stable target\
        measures $"rank"(F|T)$], fill: cyan, stroke: blue),
    )
  ]
]

A target candidate exists when adding the target column changes the relevant
rank/pivot condition:

$ "rank"(F | T) > "rank"(F). $

Equivalently, some linear combination of the streamed identities cancels all
forbidden columns while retaining a nonzero target coefficient. The reducer's
lower-factor pattern records *direct* predecessor edges. RustRed walks those
edges backward only after a hit, retaining a dependency DAG rather than
expanding every transitive row combination throughout the stream.

#panel([Terminology precision], [
  This document uses “GPLU” as the project shorthand for the planned
  Gilbert–Peierls/LU-pattern dependency workflow. The pinned public Symbolica
  reducer provides incremental sparse reduction and an `L` pattern, but its
  forward path still uses a dense scratch row. It is therefore not yet a
  textbook reachability-driven Gilbert–Peierls kernel. That distinction is
  important: dense forward scans and dense dependency patterns can contribute
  to large traces even when the input identities are sparse.
], fill: pale-purple, stroke: purple)

== Why a modular hit is not an IBP rule

The modular computation answers only an existence question at one prime and
one point. A valid parametric recurrence still requires:

#enum(
  [regenerating precisely the selected translated ordinary sources;],
  [repeating the candidate construction in exact Symbolica
    rational-polynomial arithmetic;],
  [proving the resulting identity has zero exact residual;],
  [extracting every denominator and leading-coefficient guard;],
  [proving all remaining integrals are strictly lower under the persisted
    ordering;],
  [splitting and rescheduling every exact exceptional equality case;],
  [compiling owners and proving that the final uncovered complement is finite.],
)

Only then may the rule enter an immutable artifact. This strict boundary is why
the measured candidates close no rays yet.

= The measured SpIReD results

#table(
  columns: (1.03fr, 0.52fr, 0.78fr, 0.85fr, 0.78fr, 0.82fr, 0.77fr),
  table.header(
    [*Orbit*], [*Size*], [*Result*], [*Probe / rows*], [*Trace rows*],
    [*Elapsed*], [*Published*],
  ),
  [`001011`], [12], [#yes], [2 / 1456], [560], [106.6 ms], [no],
  [`001101`], [4], [#no], [4 / 3060], [—], [296.8 ms], [no],
  [`001111`], [12], [#no], [4 / 3060], [—], [482.6 ms], [no],
  [`011110`], [3], [#no], [4 / 3060], [—], [549.5 ms], [no],
  [`011111`], [6], [#yes], [2 / 1000], [136], [27.8 ms], [no],
  [`111111`], [1], [#no], [4 / 3060], [—], [642.0 ms], [no],
)

The six per-orbit times sum to about 2.105 seconds. Roughly 32 milliseconds of
remaining setup/reporting overhead gives the measured all-orbit total of
2.137 seconds.

Every final ledger had the same authority status:

#align(center)[
  #grid(
    columns: (1fr, auto, 1fr, auto, 1fr),
    gutter: 8pt,
    align: center,
    panel([Before search], [one declared corner terminal\
      no rule owner], fill: pale-grey),
    [#arrow],
    panel([After bounded search], [`OwnerFree`\
      root still pending], fill: pale-orange, stroke: orange),
    [#arrow],
    panel([Residual], [one positive-dimensional uncovered box\
      not terminal-finite], fill: pale-red, stroke: red),
  )
]

== What relation did it “stumble on”?

For `001011`, the stream found a finite-field dependency after an aggregate
1456 rows, during the second configured probe. Walking the direct predecessor
edges showed that the candidate depends on *560 translated ordinary source
requests*. For `011111`, the corresponding numbers were 1000 streamed rows
and 136 dependency rows.

These are precise candidate identities, but they are *not exact parametric
IBPs*. RustRed does not yet have authenticated coefficients, a denominator, a
right-hand side, guards, or a descent proof for either one. Consequently there
is no honest formula to print in this note. Writing a plausible-looking
recurrence from the modular residue would fabricate authority that the run did
not produce.

#verdict([The exact-row cap did its job], [
  The diagnostic cap was eight selected rows. Since 560 > 8 and 136 > 8, both
  candidates were stopped *before* building the exact selected-source epoch.
  The cap is not a claim that no valid relation exists. It converts a likely
  symbolic blow-up into a fast, typed, reproducible incomplete result.
], blue, cyan)

#panel([Behavior after this measured sweep], [
  The measured table above is retained as historical evidence: that version of
  the root diagnostic returned when its oversized first candidate hit the cap.
  The now-tested target runner instead treats an oversized support as an
  ordinary candidate-local inconclusive result and continues its bounded
  post-hit window and alternative-support search. It still constructs no exact
  epoch for the oversized trace and grants no authority. New ordering studies
  must therefore be compared with each other, not silently substituted into
  the historical 2.137-second census.
], fill: pale-green, stroke: green)

== Follow-up: one exact `011111` root owner

A subsequent targeted release diagnostic deliberately raised the exact limit
for the smaller `011111` witness. This time the 136-row dependency trace passed
through exact materialization and admission. The literal census was:

#table(
  columns: (1.55fr, 1.15fr, 2.75fr),
  table.header([*Quantity*], [*Observed*], [*Meaning*]),
  [Internal target-run time], [145.202 ms], [Discovery, exact admission, guard
    extraction, and owner commit inside the measured target run.],
  [Setup-inclusive time], [about 171 ms], [The surrounding one-orbit
    diagnostic, excluding compilation.],
  [Source requests / streamed rows], [1394 / 1064], [The bounded source stream
    stopped after the admitted candidate rather than exhausting four full
    depth-two probes.],
  [Modular hits / lift requests], [1 / 9], [One target-producing modular event;
    nine compact-lift proposals were considered in its bounded window.],
  [Exact admissions], [1], [The 136-row trace produced a valid exact algebraic
    recurrence; this is no longer merely modular evidence.],
  [Guards / committed owners], [1 / 1], [The generic root gained one owner and
    one exceptional guard case was proposed.],
  [Ledger snapshot], [`rev=1`, owners 1, terminals 1], [The reported status was
    `Incomplete(GuardIncomplete)`, not closed.],
  [Residual report], [1 uncovered box, 1 guard-incomplete case], [One proposed
    guard child remains pending.],
)

#verdict([A genuine exact step, still not K6 closure], [
  The exact algebraic admission is valid: RustRed rebuilt the selected ordinary
  sources, obtained an exact recurrence, passed admission, and committed an
  owner. It does *not* discharge the coefficient-zero branch of that rule. The
  ledger therefore correctly reports `GuardIncomplete`; a generic rule cannot
  own the locus on which its leading coefficient vanishes.
], green, pale-green)

An adversarial audit found a separate geometric bookkeeping defect in this
diagnostic: the owner-free baseline used for its delta comparison was taken on
the full orthant rather than on the source-safe carrier. This does not undo the
exact algebraic recurrence, but it makes the observed owner-cover delta and any
claim of geometric shrinkage *provisional*. The carrier comparison must be
fixed and the run repeated before treating that delta as authoritative.

A serial equality-case driver is also being connected to this root path. Its
current diagnostic probe portfolio does not yet match the intended case-valid
probe schedule; that wiring mismatch is being fixed. Until the corrected
driver reruns, its bounded carrier is useful for exercising control flow but
cannot prove global closure of an unbounded sector.

== Thirty-ordering bounded screen

After oversized supports became ordinary candidate-local inconclusive results,
the same release executable screened 30 deterministic pinned global ordering
representatives. Every run used depth two, four probes per root, exact-row cap
eight, post-hit window 64, all six root orbits, and a single test thread. A
priority list below is the persisted *rank by coordinate slot*, not a K6
topology label.

Every ordering gave the same coverage result:

- `001011` had one modular hit;
- `011111` had six modular hits across the bounded continuation;
- `001101`, `001111`, `011110`, and `111111` had no modular hit after 3060
  rows each;
- no cap-eight run admitted a rule; every ledger remained `OwnerFree` with one
  positive-dimensional uncovered box.

The detailed candidate census falls into three compact profiles:

#table(
  columns: (0.45fr, 2.05fr, 2.05fr),
  table.header([*Code*], [*`001011`*], [*`011111`*]),
  [A], [`rows=3050`, `hits=1`, `lift_requests=9`],
       [`rows=2228`, `hits=6`, `lift_requests=22`],
  [B], [`rows=2992`, `hits=1`, `lift_requests=15`],
       [`rows=2228`, `hits=6`, `lift_requests=22`],
  [C], [`rows=3050`, `hits=1`, `lift_requests=9`],
       [`rows=2270`, `hits=6`, `lift_requests=14`],
)

#block(width: 100%)[
  #set text(size: 7.8pt)
  #table(
    columns: (2.6fr, 0.9fr, 0.52fr),
    table.header([*Rank by slot*], [*Total (ms)*], [*Profile*]),
    [`0,1,2,3,4,5`], [3069.266], [A],
    [`0,1,2,3,5,4`], [2774.234], [B],
    [`0,1,2,4,3,5`], [2928.470], [A],
    [`0,1,2,4,5,3`], [2781.875], [B],
    [`0,1,2,5,3,4`], [2745.850], [A],
    [`0,1,2,5,4,3`], [2728.770], [B],
    [`0,1,3,2,4,5`], [2786.279], [A],
    [`0,1,3,2,5,4`], [2804.341], [B],
    [`0,1,3,4,2,5`], [2588.342], [A],
    [`0,1,3,4,5,2`], [2539.085], [A],
    [`0,1,3,5,2,4`], [2540.823], [A],
    [`0,1,3,5,4,2`], [2532.825], [A],
    [`0,1,4,2,3,5`], [2653.519], [A],
    [`0,1,4,2,5,3`], [2605.839], [A],
    [`0,1,4,3,2,5`], [2678.323], [A],
    [`0,1,4,3,5,2`], [2814.695], [A],
    [`0,1,4,5,2,3`], [2803.082], [B],
    [`0,1,4,5,3,2`], [2679.163], [A],
    [`0,1,5,2,3,4`], [2716.180], [B],
    [`0,1,5,2,4,3`], [2625.718], [A],
    [`0,1,5,3,2,4`], [2615.171], [B],
    [`0,1,5,3,4,2`], [2775.539], [A],
    [`0,1,5,4,2,3`], [2667.850], [B],
    [`0,1,5,4,3,2`], [2788.268], [A],
    [`0,2,3,4,5,1`], [2656.890], [C],
    [`0,2,3,5,4,1`], [2749.653], [C],
    [`0,2,4,3,5,1`], [2713.258], [C],
    [`0,2,4,5,3,1`], [2649.527], [C],
    [`0,2,5,3,4,1`], [2615.716], [C],
    [`0,2,5,4,3,1`], [2765.603], [C],
  )
]

Across these single observations, the minimum was 2532.825 ms, the median
2714.719 ms, the mean 2713.138 ms, and the maximum 3069.266 ms. Peak resident
memory ranged from 9236 to 15376 KiB, with a median of 12312 KiB. The fastest
single observation was rank-by-slot `0,1,3,5,4,2`, but these sequential runs
were neither repeated nor isolated from shared-host load. It is therefore a
screening result, not evidence of a statistically meaningful “winning”
ordering.

The built-in natural ordering, which has no explicit coordinate-priority
overlay, is distinct from the explicit identity list `0,1,2,3,4,5`. Its
separate cap-eight control took 2541.666 ms. The previously suggested priority
`5,3,4,2,0,1` took 2926.693 ms, and the explicit reverse priority
`5,4,3,2,1,0` took 2538.435 ms. They too changed neither bounded hit coverage
nor rule admission.

Three cap-16 checks gave the following internal totals:

#table(
  columns: (2.25fr, 1fr, 1.4fr),
  table.header([*Ordering*], [*Total*], [*Outcome*]),
  [built-in natural], [2369.801 ms], [0 rules admitted],
  [`0,1,3,5,4,2`], [2505.045 ms], [0 rules admitted],
  [`5,3,4,2,0,1`], [2534.059 ms], [0 rules admitted],
)

Raising the cap from eight to sixteen therefore exposed no small admissible
witness in these checks. The continuation diagnostic does not yet print the
support size of every rejected candidate, so the matrix cannot honestly name
its smallest trace. The literal known first-hit traces remain 560 and 136 from
the earlier measurement; the later exact `011111` run is the evidence that the
136-row trace can in fact be materialized and admitted.

#panel([What the ordering screen says], [
  Ordering already changes row rejection, fill chronology, and the number of
  compact-lift proposals. At this shallow shell it does *not* rescue any of the
  four modular misses, nor find a cap-small owner for the two hit roots. The 30
  representatives form a principled deterministic portfolio, but keeping the
  six canonical orbit representatives fixed means this is not an exhaustive
  proof over all relative orderings. Deeper fair shells, preconditioning, and
  trace-quality scoring remain necessary.
], fill: pale-purple, stroke: purple)

The useful discovery is therefore structural:

- `001011` does have a target-producing dependency in the tested modular
  window, but the first dependency path is enormous;
- `011111` also has one, and its first path is about four times smaller; the
  follow-up exact diagnostic has now admitted that 136-row generic recurrence,
  but its guard child remains open;
- the present row ordering and pivot chronology are selecting poor exact
  witnesses, especially in `001011`;
- searching only until the first hit is not an adequate performance policy.

== The four no-hit roots

For `001101`, `001111`, `011110`, and `111111`, every configured probe consumed
all 3060 rows without a target rank change. No exact stage was attempted.

This does not imply that these sectors lack a parametric recurrence. A finite
exact minor can vanish at an unlucky point or prime, and a valid relation may
need a translation outside the depth-two diamond. The correct status is:

#panel([Bounded discovery miss], [
  “No candidate was found for this ordering within signed-L1 depth two and the
  four listed specializations.” It is *not* “this orbit is a master,” “this
  ray is closed,” or “no relation exists.”
], fill: pale-orange, stroke: orange)

The elapsed times differ because the number and order of dynamically observed
forbidden columns, sparse fill, and dense scratch-row work differ by sector.
The current report does not expose enough per-row reducer telemetry to
attribute the 296–642 ms spread more finely.

= The uncapped first-orbit attempt

Before adding the ingress cap, the same new root diagnostic began with
`001011`. It did not print the first-orbit result after more than seven minutes,
used approximately one CPU core, and grew beyond 700 MiB resident memory. It
was interrupted before any rule, owner, report line, or artifact was produced.

The capped rerun later demonstrated that:

#enum(
  [the modular candidate for this orbit is found in about 0.1 seconds;],
  [its exact dependency trace has 560 rows;],
  [returning before exact epoch construction avoids the long stall and large
    working set.],
)

Together, these observations strongly implicate the exact compact-lift path:
rebuilding hundreds of exact shifted rows and reducing their multivariate
rational-polynomial coefficients can trigger denominator products, repeated
GCDs, and rapid intermediate support growth.

#panel([What is proved, and what is inferred], [
  *Proved by the censuses:* modular scouting reaches the candidate quickly;
  the candidate has 560 dependency rows; an early cap makes the attempt return
  quickly.\
  *Strongly inferred:* the uncapped time and memory were spent in compact
  exact-frame construction/materialization and its Symbolica algebra.\
  *Not yet measured:* the division among exact source translation, sparse
  matrix construction, rational-polynomial row reduction, GCD/cancellation,
  replay, and guard extraction. Stage-level timers are required before naming
  one of those subphases as the sole bottleneck.
], fill: pale-purple, stroke: purple)

This distinction matters because “expression swell” is a mechanism, not yet a
complete profile. The next diagnostic must report exact input rows and terms,
intermediate nonzeros, polynomial terms and degree, GCD work, peak memory, and
time around every non-interruptible Symbolica call.

= Where time is currently spent

The evidence supports the following bottleneck map.

#table(
  columns: (1.5fr, 0.95fr, 2.95fr),
  table.header([*Stage*], [*Current evidence*], [*Interpretation*]),
  [Structural source preparation and modular coefficient evaluation],
  [fast in bounded sweep], [Included in the 2.137-second all-orbit total. Not
    the dominant root cost at depth two.],
  [Dual incremental modular reduction], [fast enough], [Four complete
    3060-row misses take 0.30–0.64 s each. There is optimization room, but no
    minute-scale problem here.],
  [Dependency extraction], [cheap but produces large output], [The direct
    `L`-pattern walk quickly exposes traces of 560 and 136 rows. Trace *quality*,
    not extraction latency, is the problem.],
  [Exact compact-frame construction/materialization], [strongly implicated],
    [The uncapped 560-row candidate runs for minutes and consumes hundreds of
    MiB; the preflight cap makes it return in 0.1 s.],
  [Exact replay, guards, and descent], [reached for the 136-row trace],
    [The complete admitted target run took 145.202 ms, but its subphases are
    not timed separately. The 560-row path remains unmaterialized.],
  [Exceptional-case fixed point], [one child exposed, not discharged], [The
    exact `011111` owner proposed one guard child; the ledger remains
    `GuardIncomplete`.],
  [Legacy owner-cover control], [opaque and slow], [One hour before its first
    report boundary; its phases cannot be recovered from external telemetry.],
)

The immediate optimization target is thus not “make every finite-field
operation a little faster.” It is “make the winning exact dependency much
smaller, and avoid exact rational-polynomial elimination until that has been
achieved.”

= What Gregor's case-directed strategy says to do next

Gregor's notes change how the results should be read. The purpose of the
finite-field pass is not to obtain the first dependency at any cost. It is to
find a *small, useful witness* for one exact case, recenter it to the canonical
target, lift only that witness, and recursively handle the exact loci where its
pivot coefficient vanishes.

The K6 measurements make five consequences concrete.

== Improve the witness, not only the arithmetic

The first `001011` hit contains 560 rows. Even perfect exact arithmetic cannot
make that an attractive starting point. RustRed should compare candidates in a
bounded post-hit window, scoring at least:

- dependency-node and edge counts;
- predicted exact polynomial term work;
- forbidden-column fill and pivot density;
- expected guard degree and branch count;
- source translation depth;
- stability across held-out probes.

The existing post-hit lane is the right seam. It should not accept the first
rank gain when a slightly later row yields a dramatically smaller dependency.

== Precondition ordinary sources without losing provenance

A sector- and ordering-local sparse exact RREF can remove dependent raw source
directions before translation. Denominators may be cleared and rows made
primitive, while preserving a transformation back to the nine generated
ordinary sources for final replay.

This is an acceleration, not new mathematical authority. A raw-source lane
must remain a fair fallback because specialization can make a generally useful
preconditioned row degenerate on one equality case. The K6 experiment should
compare raw and preconditioned streams by hit depth, trace size, modular fill,
and exact-lift cost.

== Treat ordering as part of the solver

The dependency trace is a consequence of source chronology, integral-column
order, and pivot choices. A poor order can turn a short relation into a dense
560-row ancestor closure. RustRed should race a small deterministic,
symmetry-reduced ordering portfolio and retain the cheapest exact candidate.

The comparison must remain fair: preferences may reorder sources *within* a
signed-L1 shell, but no source can be starved forever. If a small static
portfolio still produces outliers, a bounded bandit/MCTS policy can use early
fill and trace-growth signals to abandon bad orders before exact materialization.

== Continue the four misses fairly

For the four no-hit orbits, move diagonally through a schedule of increasing
translation depth, independent primes, index points, and orderings. Do not run
depth 3, then 4, then 5 forever at the same specialization. If a finite exact
minor exists, some prime and point should preserve it; if a finite translated
witness exists, some shell should contain it.

This is a conditional semi-decision procedure, not a known finite-depth bound.
Resource exhaustion must save resumable progress and remain “incomplete.”

== Delay exact coefficient construction

Gregor's fast route relies on modular discovery and rational-polynomial
reconstruction. Symbolica does not yet expose the required optimized sparse
multivariate rational-function reconstruction API. RustRed should therefore:

- retain a narrow materializer interface;
- use exact Symbolica arithmetic only for genuinely small pruned traces now;
- reject or defer huge traces with typed resource diagnostics;
- plug in Symbolica's reconstruction materializer when the public API arrives;
- not implement a competing interpolation/CRT/CAS framework internally.

Even after reconstruction exists, a 560-row trace is undesirable. Better
ordering and preconditioning remain essential because reconstruction reduces
coefficient cost, not combinatorial fill.

= Completing a root is still not completing K6

Suppose a future ordering produces the exact generic rule

$ I(bold(n)) = sum_j r_j(d,bold(n)) I(bold(n)+bold(delta)_j). $

If its leading coefficient contains a factor such as $n_1$, the rule applies
only where $n_1 != 0$. The equality face $n_1=0$ must become a new case. On that
face, a term that would otherwise enter an inactive-line supersector may vanish
because its coefficient is also proportional to $n_1$. This coefficient-aware
boundary analysis can avoid unnecessary children, but it must be exact.

#align(center)[
  #grid(
    columns: (1fr, auto, 1fr, auto, 1fr),
    gutter: 8pt,
    align: center,
    panel([Generic case $C$], [search translated sources\
      find exact rule], fill: cyan, stroke: blue),
    [#arrow],
    panel([Rule valid on $g != 0$], [publish only after replay and descent\
      owner covers generic bulk], fill: pale-green, stroke: green),
    [#arrow],
    panel([Exceptional child $C and g=0$], [raise equality rank\
      search again or retain a fixed terminal], fill: pale-orange, stroke: orange),
  )
]

When guards are coordinate-linear, every nonredundant child fixes another
coordinate, so equality rank increases and recursion depth is at most six.
Coupled affine guards require the planned exact lattice chart. Non-affine or
unsupported exceptional geometry must fail closed.

The production driver must also turn genuinely activating inactive-boundary
faces into working cells, while keeping its temporary bounded search envelope
separate from the logical equality case. These components are necessary before
a root rule can grow into a zero-uncovered artifact.

= A concrete next-run programme

The live measurements and Gregor's strategy together suggest the following
ordered programme.

#step(1, [Add exact-stage telemetry], [
  Timestamp source regeneration, exact epoch construction, matrix assembly,
  Symbolica row reduction, replay, guard extraction, descent, and owner
  compilation. Record rows, terms, nonzeros, polynomial degrees/support, and
  RSS around native calls. This turns the strong exact-swell inference into a
  profile.
])
#v(5pt)
#step(2, [Shrink the pathological candidate], [
  Concentrate trace-quality work on the still-pathological 560-row `001011`
  witness. Compare raw and provenance-preserving preconditioned sources, then
  score a bounded post-hit ordering portfolio. Retain `011111` as a positive
  exact-admission regression and seek a smaller trace only if profiling makes
  its 136-row materialization material.
], color: purple)
#v(5pt)
#step(3, [Deepen the four misses], [
  Resume signed-L1 shells fairly across new primes and points. Persist
  prepared structural rows and reducer-compatible progress where possible;
  restarting depth zero for every probe/depth expansion wastes the chronology.
], color: orange)
#v(5pt)
#step(4, [Consolidate the admitted root rule], [
  Fix the source-safe-carrier baseline and serial probe wiring, then reproduce
  the exact `011111` admission through the production driver. Preserve its
  regenerated-source replay, guard extraction, and strict descent evidence;
  process the proposed guard child instead of treating the generic owner as
  root closure.
], color: green)
#v(5pt)
#step(5, [Run the equality-case fixed point], [
  Feed guard-zero and inactive-boundary cases into the most-generic-first
  worklist. Keep positive-dimensional misses pending; only fully fixed leaves
  may use the explicit nonminimal terminal policy.
])
#v(5pt)
#step(6, [Attempt full K6 again], [
  Run all six orbit roots through the production SpIReD driver, cold-write the
  artifact, reload it in a fresh process, prove zero uncovered cases, and
  reduce canary integrals. Only this result warrants the phrase “K6 closes.”
], color: red)

The full K6 attempt should be repeated after each coherent change to ordering,
preconditioning, compact tracing, or exact materialization. A small bounded
root sweep remains the fast regression, while the full closure run supplies
the real end-to-end pressure.

= What success will look like

A future successful log must contain more than a short runtime and six modular
hits. At minimum it must demonstrate:

#table(
  columns: (1.45fr, 3.6fr),
  table.header([*Gate*], [*Required evidence*]),
  [Discovery], [Every positive-dimensional root/child has a finite candidate;
    modular support is stable under held-out probes.],
  [Exact authority], [Every rule reconstructs from freshly generated ordinary
    sources and replays to exact zero.],
  [Applicability], [All leading coefficients and denominators have explicit
    guards; every zero locus is discharged.],
  [Termination], [Every right-hand-side integral is strictly lower under the
    artifact's persisted order.],
  [Coverage], [All six sector orbits and their symmetry images have zero
    positive-dimensional uncovered complement.],
  [Terminals], [Only an explicit finite terminal manifest remains; a
    nonminimal basis is acceptable.],
  [Reproducibility], [Deterministic artifact identity across supported worker
    counts; fresh-process load and canary application pass.],
  [Integration], [The shipped artifact drives Vakint's FORM-less RustRed
    scalar backend through all applicable three-loop acceptance tests.],
)

#verdict([Present status], [
  The new approach has reached one exact algebra milestone, not a closure
  milestone. It turns the root scan from an opaque hour-scale attempt into a
  two-to-three-second, orbit-by-orbit diagnosis, and it can admit the 136-row
  `011111` generic recurrence in about 145 ms internally. Its guard child, four
  bounded discovery misses, the 560-row `001011` expression-swell path, and the
  audited carrier/probe wiring defects remain. No K6 artifact can yet be
  published.
], red, pale-red)

= Reproduction boundary

The diagnostic source lives in
`crates/rustred-core/src/foundry/campaign/k6_spired_root_diagnostic.rs` and is
marked ignored because it is a release-only K6 pressure test. The intended
invocation is:

#block(width: 100%, inset: 8pt, fill: pale-grey, radius: 4pt)[
```sh
export SYMBOLICA_LICENSE='<valid local licence>'
nix develop --command cargo test --release -p rustred \
  k6_spired_root_attempt_reports_every_full_rank_orbit \
  -- --ignored --nocapture
```
]

Compilation and dependency download are outside the 2.137-second logical test
boundary. The exact-row cap and source depth are diagnostic policy, not
production closure settings. Repeating the command after changing either one
must record the new configuration alongside time, CPU, peak RSS, row counts,
trace sizes, rule admissions, pending cases, and cover status.

The legacy hour-long control used `campaign run-waves` with the autonomous K6
configuration and four requested cores. Its exact command line and temporary
output location belong in the run transcript, but its runtime must remain
labelled “legacy owner-cover control,” never “GPLU K6.”

= Final plain-language summary

RustRed now has a fast way to ask: “among these translated ordinary IBPs, can a
finite-field linear combination isolate my target while cancelling everything
that is not already reducible?” For all six K6 root sectors, a small search
takes roughly two to three seconds in the measured depth-two configurations.
It says “yes” in two sectors and “not yet” in four, across all 30 tested
coordinate-priority representatives.

The two “yes” answers need 560 and 136 source rows in the first measured
traces. The smaller `011111` trace has now been materialized exactly and
committed as a generic-root owner. Its coefficient-zero guard case is still
pending, and an audited carrier mismatch makes its geometric delta provisional
until rerun. Trying to materialize the larger `001011` trace is still the
operation most strongly implicated by the seven-minute, 700-MiB interrupted
attempt. A safe cap catches that path before the expensive work begins.

The global status therefore remains incomplete. The audit checkpoint retains
the safe distinction between closure of a bounded diagnostic carrier and
closure of the full sector, plus the nonparallel serial-probe portfolio. A
direct attempt to make the owner-free planner use the finite compiler carrier
was reverted because the existing planner contract deliberately starts from a
symbolic unbounded orthant; reconciling those two scopes remains an explicit
architectural seam rather than a hidden closure assumption.

Work now pauses at this reproducible checkpoint. The next implementation pass
will begin from Gregor's SpIReD reference code once it is supplied under
`vendor`: use that winning execution path to resolve the carrier contract,
discharge the guard child, improve the `001011` witness, and deepen the four
misses. Exact inactive-face and equality-case recursion must then reach a fixed
point. Only a cold-reloaded artifact with zero positive-dimensional complement
establishes K6 closure.
