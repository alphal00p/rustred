#let navy = rgb("#17324d")
#let blue = rgb("#2878b5")
#let cyan = rgb("#dff3fa")
#let green = rgb("#2b8a66")
#let pale-green = rgb("#e5f5ed")
#let orange = rgb("#c66a18")
#let pale-orange = rgb("#fff0df")
#let red = rgb("#b94646")
#let pale-red = rgb("#fde9e7")
#let grey = rgb("#677386")
#let pale-grey = rgb("#f2f5f7")
#let line-grey = rgb("#cdd5dc")
#let ink = rgb("#172033")

#set document(
  title: "Why closing K6 is harder than adding more seed integrals",
  author: "RustRed research handoff",
  date: datetime(year: 2026, month: 9, day: 3),
)

#set page(
  paper: "a4",
  margin: (top: 19mm, bottom: 19mm, left: 20mm, right: 20mm),
  footer: context {
    set text(size: 7.5pt, fill: grey)
    line(length: 100%, stroke: 0.35pt + line-grey)
    v(3pt)
    grid(
      columns: (1fr, auto),
      [RustRed research note · 3 September 2026],
      [#counter(page).display("1")],
    )
  },
)

#set text(font: "DejaVu Sans", size: 9.4pt, fill: ink, lang: "en")
#set par(justify: true, leading: 0.62em)
#set heading(numbering: "1.")
#set list(indent: 1.25em, body-indent: 0.55em, spacing: 0.35em)
#set enum(indent: 1.25em, body-indent: 0.55em, spacing: 0.35em)
#set table(inset: 5pt, stroke: 0.4pt + line-grey)
#show heading.where(level: 1): it => block(above: 1.5em, below: 0.7em)[
  #set text(fill: navy, weight: "bold")
  #it
  #line(length: 100%, stroke: 0.8pt + blue)
]
#show heading.where(level: 2): it => block(above: 1.2em, below: 0.45em)[
  #set text(fill: navy, weight: "bold")
  #it
]
#show heading.where(level: 3): it => block(above: 0.9em, below: 0.35em)[
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

#let arrow = text(size: 13pt, fill: grey)[→]

#align(center)[
  #v(12mm)
  #tag[RESEARCH EXPLAINER]
  #v(8mm)
  #text(size: 25pt, weight: "bold", fill: navy)[
    Why closing K6 is harder than adding more seed integrals
  ]
  #v(5mm)
  #text(size: 13pt, fill: grey)[
    Ordinary IBP descent, Janet completion, infinite rays, and expression swell
  ]
  #v(14mm)
  #block(
    width: 88%,
    inset: 13pt,
    radius: 6pt,
    fill: cyan,
    stroke: 0.8pt + blue,
  )[
    #text(size: 11pt, weight: "bold", fill: navy)[The key idea]
    #v(4pt)
    An algorithm can finish generating every consequence required by its
    chosen Janet rules and still leave infinitely many integrals unreduced.
    That means the supplied relation model is too small for finite closure—not
    that Janet forgot another item in its queue.
  ]
  #v(13mm)
  #grid(
    columns: (1fr, 1fr, 1fr),
    gutter: 7pt,
    panel([1 · Descent], [Reduces many concrete integrals but may leave a
      direction untouched.], fill: pale-grey),
    panel([2 · Completion], [Generates all mandatory consequences of the
      current symbolic relations.], fill: pale-orange, stroke: orange),
    panel([3 · Closure], [Additionally proves that only finitely many terminal
      integrals remain.], fill: pale-green, stroke: green),
  )
  #v(18mm)
  #text(size: 9pt, fill: grey)[
    RustRed research handoff · 3 September 2026
  ]
]

#pagebreak()

#outline(title: [How to read this note], indent: 1em)

#pagebreak()

= The problem in one page

RustRed seeks *parametric* integration-by-parts (IBP) rules. A parametric rule
does not reduce just one integral with fixed powers. It reduces an infinite
class of integrals whose propagator and numerator powers are represented by
symbolic integers.

For the three-loop single-scale vacuum problem, a complete scalar-product basis
has six independent coordinates. RustRed calls this the *K6 family*. A point in
its lattice can be pictured as

$
  I(n_1, n_2, n_3, n_4, n_5, n_6), quad n_i in ZZ.
$

The real lattice is six-dimensional, so the drawings in this note use two
dimensions. The logic is unchanged: a two-dimensional ray becomes a ray, plane,
or higher-dimensional cone in K6.

#panel([Three different success claims], [
  *A reduction hit* says one chosen integral reduced.
  *Janet queue exhaustion* says every required prolongation of the current
  symbolic relation module was processed.
  *Family closure* says the queue exhausted *and* the exact complement is
  finite, all remaining points are explicit terminals, and all coefficient
  guards and boundary sectors are covered.
], fill: pale-orange, stroke: orange)

The currently recorded K6 experiments did *not* reach Janet queue exhaustion.
They stopped earlier because exact rational-function coefficients grew to tens
of millions of polynomial terms or divisor searches hit their work cap. There
is therefore no honest count of “rays left after Janet” for K6 yet.

The research found two separate cure paths:

#grid(
  columns: (1fr, 1fr),
  gutter: 9pt,
  panel([If the queue has not exhausted], [
    Make exact completion scalable: use shift-aware finite-field discovery,
    batched sparse elimination, signatures, order screening, compact traces,
    and a final exact replay.
  ], fill: cyan, stroke: blue),
  panel([If it exhausts but the complement is infinite], [
    The relation/domain model must change: test parametric annihilators,
    localization, inverse-shift and boundary coupling, symmetry,
    factorization, and parent/supersector relations.
  ], fill: pale-red, stroke: red),
)

== A short glossary

#table(
  columns: (1.05fr, 3.2fr),
  table.header([*Term*], [*Plain-language meaning in this note*]),
  [Parametric rule], [A recurrence valid for symbolic integer powers. Here
    “parametric” means parameterized by indices.],
  [Feynman-parametric annihilator], [A differential operator that kills the
    integrand written in Feynman parameters. This is a second, related meaning
    of “parametric.”],
  [`Mom`], [The module generated by ordinary momentum-space IBP identities.],
  [`Ann`], [The larger candidate module obtained from operators annihilating
    the Lee--Pomeransky integrand.],
  [Saturation], [Dividing out a polynomial factor on the open region where it
    is nonzero, while keeping its zero region as a separate branch.],
  [Guard], [The exact condition under which a chosen pivot or division is
    valid.],
  [Complement], [Lattice monomials not owned by any leading reduction rule.],
)

= From an integral family to a lattice problem

== What “parametric” changes

Suppose a conventional reduction asks only for

$ I(3, 1, 0, 2, 0, -4). $

A finite Laporta-style solve can generate enough equations near that point and
return a result. That result does not automatically tell us how to reduce

$ I(3, 1, 0, 2, 0, -4000) $

or every other point in the same family. RustRed instead seeks rules such as

$
  I(n_1 + 1, n_2, dots) =
  c_0(d, bold(n)) I(n_1, n_2, dots)
  + c_1(d, bold(n)) I(n_1, n_2 - 1, dots),
$

with exact rational functions $c_i$. One symbolic rule can cover an infinite
orthant of lattice points—provided its leading coefficient is nonzero and the
right-hand side is strictly lower in the chosen order.

#panel([A rule is more than an equation], [
  To be executable, a symbolic identity needs an orientation, a domain guard,
  and a proof that every right-hand-side integral is lower. The same equality
  can be an excellent recurrence in one order and useless or cyclic in another.
], fill: cyan, stroke: blue)

== Shift monomials as addresses

Choose a reference integral. Let $E_1$ mean “increase the first relevant power
by one,” $E_2$ mean “increase the second,” and so on. Then

$ E_1^a E_2^b $

is an address in a lattice. The *leading monomial* of a rule tells us which
translated points that rule can reduce. If a leading monomial divides a target
monomial, the rule may cover the target after a suitable translation.

This turns the closure question into a geometric one:

- covered monomials are reducible by at least one leading rule;
- uncovered monomials are candidate terminals;
- a finite uncovered set can be a nonminimal master basis;
- an uncovered ray or cone contains infinitely many points and cannot be
  shipped as a finite master basis.

= Why naive descent leaves rays

== The smallest possible example

Consider a toy family $I(a,b)$ with nonnegative shift coordinates. Suppose the
only symbolic lowering rule has leading monomial $x$, where $x$ represents a
positive shift in $a$:

$
  R(a,b): I(a+1,b) -> A(a,b) I(a,b) + "lower sectors".
$

Every point with $a >= 1$ can descend. Nothing changes $b$ when $a=0$.

#table(
  columns: (0.7fr, 1fr, 1fr, 1fr, 1fr, 1fr, 1fr),
  align: center,
  table.header(
    [$b$ ↓, $a$ →], [$0$], [$1$], [$2$], [$3$], [$4$], [$5$],
  ),
  [$5$], table.cell(fill: pale-red)[$I(0,5)$], table.cell(fill: pale-green)[$I(1,5)$], table.cell(fill: pale-green)[$I(2,5)$], table.cell(fill: pale-green)[$I(3,5)$], table.cell(fill: pale-green)[$I(4,5)$], table.cell(fill: pale-green)[$I(5,5)$],
  [$4$], table.cell(fill: pale-red)[$I(0,4)$], table.cell(fill: pale-green)[$I(1,4)$], table.cell(fill: pale-green)[$I(2,4)$], table.cell(fill: pale-green)[$I(3,4)$], table.cell(fill: pale-green)[$I(4,4)$], table.cell(fill: pale-green)[$I(5,4)$],
  [$3$], table.cell(fill: pale-red)[$I(0,3)$], table.cell(fill: pale-green)[$I(1,3)$], table.cell(fill: pale-green)[$I(2,3)$], table.cell(fill: pale-green)[$I(3,3)$], table.cell(fill: pale-green)[$I(4,3)$], table.cell(fill: pale-green)[$I(5,3)$],
  [$2$], table.cell(fill: pale-red)[$I(0,2)$], table.cell(fill: pale-green)[$I(1,2)$], table.cell(fill: pale-green)[$I(2,2)$], table.cell(fill: pale-green)[$I(3,2)$], table.cell(fill: pale-green)[$I(4,2)$], table.cell(fill: pale-green)[$I(5,2)$],
  [$1$], table.cell(fill: pale-red)[$I(0,1)$], table.cell(fill: pale-green)[$I(1,1)$], table.cell(fill: pale-green)[$I(2,1)$], table.cell(fill: pale-green)[$I(3,1)$], table.cell(fill: pale-green)[$I(4,1)$], table.cell(fill: pale-green)[$I(5,1)$],
  [$0$], table.cell(fill: pale-red)[$I(0,0)$], table.cell(fill: pale-green)[$I(1,0)$], table.cell(fill: pale-green)[$I(2,0)$], table.cell(fill: pale-green)[$I(3,0)$], table.cell(fill: pale-green)[$I(4,0)$], table.cell(fill: pale-green)[$I(5,0)$],
)

#align(center)[
  #tag([red column: uncovered infinite ray], fill: pale-red, color: red)
  #h(6pt)
  #tag([green cells: reducible], fill: pale-green, color: green)
]

Algebraically the leading ideal is ⟨$x$⟩. Its standard monomials
are

$ 1, y, y^2, y^3, dots $

so the complement is infinite. Reducing a million green points does not alter
that conclusion.

== A less obvious ray

Now suppose the leading monomials are $x^2$ and $x y$. The rules cover all
points with $a >= 2$ and all points with $a >= 1, b >= 1$. The standard
monomials are

$ 1, x, y, y^2, y^3, dots $

Again there is an infinite $y$-ray. The reducer may look impressively effective
on random high-degree points because almost the entire visible grid is covered.
The narrow ray remains fatal for a finite artifact.

#panel([The six-dimensional K6 analogue], [
  A K6 leading module may cover almost every sampled point while missing one
  thin direction such as $E_4^r$, or a cone such as
  $m E_2^a E_5^b$. Random finite tests will almost never prove that such a cone
  is gone. Closure needs an exact monomial-complement certificate.
], fill: pale-red, stroke: red)

== The cheap finite-complement test

For a single monomial ideal in $K$ variables, the complement is finite exactly
when the leading ideal contains a pure power of every axis:

$
  E_1^(p_1), E_2^(p_2), dots, E_K^(p_K).
$

If $E_i^(p_i)$ is absent, the pure sequence

$ 1, E_i, E_i^2, dots $

is an explicit infinite ray. If every pure power is present, all standard
monomials lie in the finite box $0 <= e_i < p_i$. For a relation module with
several components, the test is repeated component by component.

= What Janet completion actually does

== Multiplicative and nonmultiplicative directions

Ordinary polynomial division asks whether one leading monomial divides another.
Janet division assigns each basis leader a set of *multiplicative* directions.
Translations in those directions are already owned by that leader. A
translation in a *nonmultiplicative* direction creates an obligation that must
be reduced and checked.

The workflow is:

#grid(
  columns: (1fr, auto, 1fr, auto, 1fr),
  gutter: 5pt,
  align: center,
  step([1], [Orient], [Choose the leading shift monomial.], color: blue),
  arrow,
  step([2], [Queue], [Create mandatory nonmultiplicative prolongations.], color: orange),
  arrow,
  step([3], [Reduce], [Compute an exact Ore normal form.], color: green),
)
#v(6pt)
#grid(
  columns: (1fr, auto, 1fr, auto, 1fr),
  gutter: 5pt,
  align: center,
  step([4], [Admit or certify zero], [A new leader changes ownership; zero needs a witness.], color: blue),
  arrow,
  step([5], [Repeat], [Recompute masks and process every new obligation.], color: orange),
  arrow,
  step([6], [Inspect complement], [Only after the queue is empty.], color: green),
)

Every nonzero new normal form is a consequence of the original identities. It
does not add new physics; it exposes a leader that was hidden in combinations
of the existing relations.

== A toy completion that succeeds

Take the monomial generators $x^2$ and $y$, so there are no polynomial tails.
Use variable sequence $(x,y)$ and the Janet convention that maximal degree is
tested inside classes with equal earlier-variable prefixes. The generator $y$
then has a nonmultiplicative $x$ direction. Its prolongation $x y$ is not yet
Janet-covered, so it is processed and added. The next relevant prolongation
$x^2 y$ is covered by $x^2$. The queue then exhausts with leaders

$ x^2, y, x y. $

Because the ordinary leading ideal already contains the pure powers $x^2$ and
$y$, its complement consists only of $1$ and $x$. Janet completion has made
the ownership disjoint and executable; the pure powers prove finiteness.

== A toy completion that exhausts but does not close

Return to the singleton monomial generator $x$, with the same variable sequence
and convention. Both directions are multiplicative for this one leader. There
is no nonmultiplicative prolongation to queue. The Janet queue is empty
immediately.

But the complement is still

$ 1, y, y^2, dots $

This is the simplest concrete reason why queue exhaustion and family closure
are different. Janet has completed the ideal ⟨$x$⟩ perfectly. It
was never asked to invent a relation containing a power of $y$.

#panel([What completion guarantees], [
  Every mandatory Janet consequence of the supplied module has an owner or an
  exact zero reduction.
], fill: pale-green, stroke: green)

#panel([What completion does not guarantee], [
  That the supplied module contains enough independent directions to leave a
  finite quotient.
], fill: pale-red, stroke: red)

= Why “just add a point from the ray” is not enough

This tempting idea mixes up three different objects:

1. an *integral family*, which already contains all allowed integer powers of
   its denominators and scalar products;
2. a *target integral*, which is one point in that existing family;
3. a *new identity*, which can enlarge the relation module or expose a new
   symbolic leader.

Adding a concrete target does not automatically add a new identity.

== One point does not cover a ray

In the toy family above, suppose the uncovered ray is

$ I(0,0), I(0,1), I(0,2), dots $

and we add $I(0,7)$ to a seed list. Three outcomes are possible:

- we declare it a terminal: one point is named, but infinitely many other ray
  points remain;
- we generate ordinary IBPs at that point: within the same chart and away from
  guard zeros, these are specializations of translated universal templates,
  not new generic identities;
- a finite solver combines those equations with nearby points and reduces
  $I(0,7)$: useful for that target, but not yet a symbolic rule valid for every
  $b$.

Even adding the first million points of the ray leaves the rest infinite.

#align(center)[
  #panel([Finite point patch], [
    $I(0,0), I(0,1), dots, I(0,10^6)$
  ], fill: pale-orange, stroke: orange)
  #v(5pt)
  #text(size: 14pt, fill: grey)[↓]
  #v(5pt)
  #panel([Still missing], [
    $I(0,10^6+1), I(0,10^6+2), dots$
  ], fill: pale-red, stroke: red)
]

What is actually needed is a *symbolic* relation with a leader containing a
power of the missing direction, for example

$
  C(a,b) I(a,b+1) = "strictly lower integrals",
$

valid for generic $b$ and with every zero of $C$ handled separately.

== Specializing can destroy the information needed for a parametric rule

Consider the schematic identity

$
  b I(0,b+1) - (b+1) I(0,b) = 0.
$

At the concrete seed $b=7$, this is a perfectly useful numerical-index
equation. At $b=0$, its intended pivot vanishes. A rule inferred only from the
$b=7$ equation cannot reveal the exceptional branch, and one sample cannot
prove the dependence on $b$.

One may sample many integer points and reconstruct a candidate rational
function. That is a powerful *discovery* technique, but the resulting symbolic
identity must still be replayed exactly, its pivot guard recorded, and its
zero branches closed. Numerical-index equations do not become universal merely
because enough of them look similar.

== After true completion, translated seeds are already inside the module

Suppose the original symbolic IBPs generate a left module $J$. A symbolic
translation within the same sector chart and guard branch is multiplication by
a shift monomial and is already an element of $J$. A concrete seed equation is
an evaluation of such a translated template, not literally the same symbolic
module element. Away from guard zeros it supplies no new generic relation. A
boundary or guard-specialized seed can instead reveal a separate fiber, which
must be represented and closed as its own branch. Once a correct Gröbner or
Janet basis of $J$ is complete, ordinary in-branch translations cannot enlarge
$J$ or change whether its quotient is finite.

Before completion, a strategically chosen concrete seed can still be valuable:
it may expose a useful combination much earlier and avoid a terrible search
path. This is an acceleration, not a structural cure.

#panel([The decisive question], [
  Did the new input contribute a genuinely new, exactly valid relation class
  —for example a parametric annihilator, symmetry, factorization, boundary, or
  parent-family identity—or merely another specialization of relations already
  in the completed module?
], fill: cyan, stroke: blue)

== Adding a “topology” often changes nothing either

A complete K6 scalar-product family already provides coordinates for every
three-loop vacuum numerator and denominator power in that routing. Feeding an
integral with different concrete powers does not add a new denominator or a
new graph relation; it selects another point in the same six-dimensional
lattice.

Adding a genuinely different parent family *can* help if it supplies a relation
that vanishes or becomes invisible after restricting too early to a child
sector. This is the origin of some hidden or “magic” relations. The resulting
relation must be transported back with an exact routing/boundary witness. A
topology name by itself is not a proof.

= Why Janet completion can cause expression swell

== Exact coefficients are shifted rational functions

A K6 rule has coefficients depending on the dimension $d$ and six symbolic
indices. A typical row is schematically

$
  sum_u frac(P_u(d,bold(n)), Q_u(d,bold(n))) E^u = 0.
$

When a row is translated by $v$, its coefficient is not unchanged:

$ E^v c(bold(n)) = c(bold(n)+v) E^v. $

To cancel two leaders, exact normal-form arithmetic cross-multiplies translated
polynomials. A schematic cancellation looks like

$
  B(bold(n)+v) R_1 - A(bold(n)+u) R_2.
$

The final result may be small, yet each expanded product can contain millions
of terms before common factors and cancellations become visible.

== Three amplifiers interact

#grid(
  columns: (1fr, 1fr, 1fr),
  gutter: 7pt,
  panel([Coefficient support], [Products of multivariate polynomials in seven
    coefficient variables create large transient numerators.], fill: pale-red, stroke: red),
  panel([Basis and obligations], [Each admitted leader changes Janet ownership
    and creates or invalidates further prolongations.], fill: pale-orange, stroke: orange),
  panel([Search and provenance], [Every growing row is repeatedly scanned,
    translated, copied, and accompanied by an exact source witness.], fill: cyan, stroke: blue),
)

The old eager design materialized these rational coefficients repeatedly. Its
first implementation also compared many terms with many basis leaders through
a flat divisor scan. Later indexed lookup removed that flat scan, but hundreds
of millions of index queries and enormous intermediate supports remained. Even
if the number of logical obligations is only in the thousands, the physical
work can therefore still be immense.

== What the measured K6 runs show

#table(
  columns: (1.5fr, 1.1fr, 2.4fr),
  table.header([*Observed quantity*], [*Measured range*], [*Interpretation*]),
  [Basis rows reached], [59--91], [Janet discovered substantial new coverage before stopping.],
  [Completion iterations], [861--3,177], [The search was active; no queue exhausted.],
  [Default-cap additions], [16.8--25.2 M terms], [Eager exact expansion dominated several orbits.],
  [Raised-cap preflight request], [94.1 M terms], [A conservative projected addition size, not materialized canonical output.],
  [Divisor-work stop], [67,108,864 visits], [Flat leader lookup amplified basis growth.],
  [Post-Janet complement], [Not reached], [No final ray count exists.],
)

Monic normalization was tested and did not materially change five of six
structural trajectories. This is useful negative evidence: canonical row
scaling alone does not attack the dominant support and scan amplification.

Later indexed copy-on-write runs reached 88 rows and 4,097 iterations in the
natural order, and 100 rows and 5,232 iterations in an alternate order. Neither
queue exhausted. An attributed rejected payload retained 1,826,367
numerator-plus-denominator terms: source-provenance numerators were the largest
piece, physical numerators were also substantial, and denominators were
secondary.

#panel([Current implementation boundary], [
  RustRed now has exact lazy coefficient/provenance/guard circuits, shared
  immutable epochs, indexed coefficient-free Janet geometry, and exact normal
  forms against a frozen input epoch. The production admission,
  prolongation, collision/autoreduction, and full completion-driver seams are
  not yet present. Consequently the exact-lazy foundation cannot yet launch a
  complete K6 campaign by itself.
], fill: pale-orange, stroke: orange)

== Ordering helps, but cannot perform magic

Changing the order changes which term leads each row and therefore which
cancellations happen early. It can radically alter basis size, fill-in, and
coefficient swell. K6 has $6! = 720$ simple coordinate-priority permutations,
so a broad modular screen is realistic.

But ordering cannot change the finite or infinite dimension of the quotient of
the same exact module. It is a performance cure only. An order that appears to
turn an infinite completed quotient into a finite one signals a changed domain
or a faulty certificate.

= A scalable completion workflow suggested by the literature

The most credible architecture separates three trust levels.

#panel([An experimental synthesis, not an imported theorem], [
  F4, modular noncommutative Gröbner methods, signatures, and involutive
  completion are established in related algebraic settings. No reviewed paper
  proves their combined use for RustRed's guarded, multivariate,
  inhomogeneous Ore module. The adaptation needs its own correctness proof and
  exact replay tests.
], fill: pale-orange, stroke: orange)

#grid(
  columns: (1fr, auto, 1fr, auto, 1fr),
  gutter: 5pt,
  align: center,
  step([1], [Discover cheaply], [Finite fields, sparse batches, signatures,
    traces, and multiple orders.], color: orange),
  arrow,
  step([2], [Certify exactly], [Replay over rational functions; prove every
    obligation, guard, and finite complement.], color: blue),
  arrow,
  step([3], [Ship simply], [Load one immutable exact artifact; no modular CAS
    participates in ordinary reduction.], color: green),
)

== Shift-aware finite-field batches

F4-style algorithms collect many related normal forms into one sparse matrix.
Common monomial multiples are prepared once and row-reduced together. Working
at several finite-field points avoids carrying giant symbolic rational
functions during exploration.

The sampling must respect shifts. If a row contains $E^u c(bold(n))$, then a
sample at point $a$ evaluates the coefficient at $a+u$, not at $a$. Each
derived row therefore needs a replayable recipe or arithmetic circuit. A
single sampled scalar row cannot later be translated soundly.

== Signatures and trace reuse

A signature remembers where a candidate came from. Known syzygies and
rewritable signatures can reject many rows before they are expanded. A stable
finite-field run can also record its column support, reducers, pivots, and zero
rows. Other primes and points replay that trace cheaply.

These devices guide discovery. Because general modular noncommutative Gröbner
verification is probabilistic in the inhomogeneous case, RustRed must still
replay the accepted trace exactly and independently check all Janet or Ore
completion obligations.

== Reconstruct only what survives

Finite-field reconstruction is most useful when the final retained
coefficients are much smaller than the rejected intermediate expressions.
RustRed should reconstruct final lowering or border rows, not every transient
candidate. If even the final coefficient is intrinsically huge, an exact lazy
or factorized circuit is the honest artifact representation.

== Parallel work without copying the entire exact world

Independent prime, point, and ordering lanes can run concurrently and exchange
small structural traces. Only the winning trace needs expensive exact replay.
This is much more memory-efficient than giving every worker a private copy of
every expanded exact row. Exact certification remains deterministic and should
produce the same artifact for a fixed order and trace policy. Different chosen
orders may produce different bases, but their exactly certified modules and
quotient dimensions must agree. Real scheduling must also respect the active
Symbolica license; the last measured campaign admitted only one licensed
process at a time.

= What to do if a completed K6 module still has rays

At that point the performance problem has been answered. The remaining problem
is algebraic: the module being completed is not the full finite relation space
on the intended domain.

== First: name the missing geometry exactly

For each module component:

1. record every missing pure-power axis;
2. compute standard pairs to describe the surviving cones;
3. use each cone as a target for a new symbolic leader;
4. compare the final terminal count with an independently aligned generic
   master count.

This replaces “try more seeds” with a falsifiable request such as “find an
exact operator whose leader intersects $m E_2^a E_5^b$.”

== Second: compare momentum IBPs with parametric annihilators

For the Lee--Pomeransky polynomial $G$, every operator annihilating $G^s$ maps,
through Mellin transform, to a polynomial shift relation. The literature
distinguishes

“momentum IBPs” ⊆ “first-order annihilators” ⊆ “all annihilators”.

The first inclusion can be strict before coefficient localization, and whether
ordinary momentum IBPs generate everything after rational localization remains
an open question in general.

This creates a precise test. First build a complete set of irreducible scalar
products and make the conversion from the literature's normalized integral to
RustRed's normalization explicit. Then compute first-order parametric
annihilators, transform them to shifts, and reduce them by RustRed's completed
momentum-IBP module. A nonzero remainder is a valid annihilator relation that is
missing from the modeled localized `Mom`; it may expose an implementation or
domain mismatch, or a genuinely stronger `Ann` relation.

Membership $P in "Ann"$ makes the parametric identity valid independently of a
multiplier. If only $q P$ has a momentum-IBP certificate and RustRed divides by
$q$ to derive $P$ from `Mom`, that particular derivation is valid where
$q != 0$, and the branch $q=0$ must be closed separately. Second-order
annihilators are a justified next tier; they have succeeded where first-order
ones were insufficient in a four-loop form-factor calculation.

RustRed intends to work over rational functions in $d$ and the indices, so
generic coefficient saturation should already be implicit. If an explicit
saturation still adds relations, that exposes a localization or module-building
mismatch. Saturation removes only components supported where the inverted
factor vanishes; it cannot erase a generic positive-dimensional component.
Weyl closure is a related, expensive open-set calculation, not an ordering
cure. Inverting a shift is more dangerous still because it changes the
positive-sector problem and crosses boundaries.

== Third: audit the domain and its boundaries

The physical finiteness theorem concerns all integer shifts and all ordinary
IBP consequences. A positive-shift chart can lose information when a backward
shift crosses a sector wall. Therefore compare, on small pilots, the
sector-local algebra with a rational double-shift algebra, but never import an
inverse shift without an exact boundary owner.

Also audit:

- lower-sector feedback and every guard-zero specialization;
- graph automorphisms and affine loop reroutings;
- scaleless zeros and product/factorization identities;
- relations visible only in a parent topology, supersector, or uncut family;
- higher-dimensional critical varieties that signal hidden “magic” relations.

== Fourth: generate a relation aimed at the cone

Recent generating-function algorithms differentiate operator equations in the
directions left uncovered by current rules. Seedless algorithms solve directly
for generic-index lowering operators. Syzygy-constrained methods suppress
unhelpful raised propagator powers. All three are better targeted than a blind
rectangular seed expansion.

They remain source generators, not closure proofs. A descendant of an already
completed module cannot change that module unless it exposes a relation class
that the original encoding omitted. Every retained relation still needs exact
provenance and guard coverage.

== Fifth: consider a finite border instead of a huge monomial basis

An independent Lee--Pomeransky critical-point or Euler-characteristic
calculation can supply a generic master rank, after matching sector and symmetry
conventions. The simple Milnor-number count assumes proper isolated critical
points; higher-dimensional critical varieties require a general
Euler-characteristic or regulated treatment. If $r$ basis monomials are
expected, solve directly for the border
obtained by shifting those $r$ monomials in every direction. Exact commuting or
flat connection matrices can certify consistency. Every border rule must also
have exact source membership, and the chosen order ideal must be proven to span
the candidate quotient; rank matching plus flatness alone is not enough.

This may represent a finite quotient even when every convenient Gröbner order
produces a large basis. The published border theory is currently for rational
Weyl algebras, not RustRed's guarded difference algebra, so adapting it is a
research project—not an off-the-shelf solution.

= The combined K6 decision workflow

#table(
  columns: (0.65fr, 1.8fr, 2.6fr),
  table.header([*Gate*], [*Exact observation*], [*Meaning and response*]),
  [A], [Queue hits a resource stop], [No statement about final rays. Improve modular batching, signatures, indexing, traces, and ordering.],
  [B], [Queue exhausts; pure power exists on every axis], [Complement is finite. Enumerate terminals, close every guard branch, and publish even if the basis is nonminimal.],
  [C], [Queue exhausts; a pure power is missing], [The chosen module has an infinite quotient. Record standard pairs and stop tuning orders.],
  [D], [A parametric annihilator has nonzero normal form], [It is missing from modeled `Mom`, or genuinely extends it. Add it with exact annihilator provenance; guard only divisions used in its derivation.],
  [E], [No first-order difference], [Audit higher annihilators, double shifts, boundaries, factorization, and supersectors.],
  [F], [An exact-source, spanning, rank-matched flat border is certified], [A finite alternative representation exists even if a monomial completion is unwieldy.],
)

#v(6pt)
#panel([Nonminimal is acceptable], [
  The goal is not to reproduce MATAD's smallest symbolic master basis. A finite,
  universal, manageable terminal set is enough if its members can be evaluated
  once and shipped. What is forbidden is calling an infinite family of ray
  points “extra masters.”
], fill: pale-green, stroke: green)

= Outcome of the literature research

The research produced five robust conclusions.

== There is no Janet-only cure for positive dimension

Janet is a completion engine. It can certify all consequences required by its
division while leaving an infinite monomial complement. Once genuinely
complete, more prolongations or translated seeds from the same generators
cannot alter quotient dimension. Ordering can change cost and basis shape, not
finite versus infinite dimension.

== Parametric annihilators are the strongest missing-relation probe

The Mellin-transform correspondence makes them broader and more diagnostic
than blind source walks. The first practical experiment should compare
localized momentum IBPs and first-order annihilators on K3, then one hard K6
sector, targeting exact standard pairs. Higher-order annihilators are a
literature-supported escalation.

== Domain repair may matter as much as new equations

All-integer finiteness need not be visible in one positive sector chart.
Backward shifts, guard saturation, lower-sector boundaries, and parent-family
relations must be transported without losing exceptional cases. Hidden or
magic relations are a concrete warning against isolating sectors too early.

== The current K6 blocker is still computational

Existing runs stopped before queue exhaustion. The best-supported response is
shift-aware finite-field F4-style batching, signature/syzygy pruning, trace
reuse across primes and points, safe-order screening, and reconstruction only
of retained outputs. Exact lazy replay remains the final authority.

== A finite border is a promising fallback representation

If a trusted rank is available but all useful term orders swell, a finite set
of basis monomials plus exact border/connection rules may avoid a large Janet
overbasis. Its adaptation from rational Weyl to guarded shift algebras must be
proved and tested; it is not yet a RustRed capability.

#panel([Recommended next research sequence], [
  *First,* make one K6 queue actually exhaust without changing the relation
  authority.
  *Second,* inspect the exact complement.
  *Third,* only if it is infinite, run the momentum-versus-annihilator and
  domain/boundary audit.
  *Fourth,* target each exact standard pair with a stronger symbolic generator.

  *Finally,* certify a finite quotient, all guards, and a cold reproducible
  artifact.
], fill: cyan, stroke: blue)

= References and further reading

The following are primary sources used for the conclusions above.
The 2025--2026 generating-function, seedless, triangular, border-basis, and
magic-relation papers are preprints as of this snapshot; they are promising
evidence, not established high-loop termination results.

- Gerdt and Blinkov, _Involutive Bases of Polynomial Ideals_.
  #link("https://arxiv.org/abs/math/9912027")
- Seiler, _A Combinatorial Approach to Involution and δ-Regularity_.
  #link("https://arxiv.org/abs/math/0208247")
- Smirnov and Petukhov, _The Number of Master Integrals Is Finite_.
  #link("https://arxiv.org/abs/1004.4199")
- Lee and Pomeransky, _Critical Points and Number of Master Integrals_.
  #link("https://arxiv.org/abs/1308.6676")
- Bitoun, Bogner, Klausen, and Panzer, _Feynman Integral Relations from
  Parametric Annihilators_.
  #link("https://arxiv.org/abs/1712.09215")
- Barakat et al., _Feynman Integral Reduction Using Gröbner Bases_.
  #link("https://arxiv.org/abs/2210.05347")
- Böhm et al., _Complete Sets of Logarithmic Vector Fields for IBP
  Identities_.
  #link("https://arxiv.org/abs/1712.09737")
- Feng et al., _An Algorithm for the Symbolic Reduction of Multi-loop Feynman
  Integrals via Generating Functions_.
  #link("https://arxiv.org/abs/2605.09541")
- de la Cruz and Kosower, _Seedless Reduction of Feynman Integrals_.
  #link("https://arxiv.org/abs/2602.22111")
- Smith and Zeng, _Feynman Integral Reduction using Syzygy-Constrained
  Symbolic Reduction Rules_.
  #link("https://arxiv.org/abs/2507.11140")
- Liu and Mitov, _Untangling the IBP Equations_.
  #link("https://arxiv.org/abs/2512.05923")
- Crisanti et al., _Magic Relations and Critical Varieties of Feynman
  Integrals_.
  #link("https://arxiv.org/abs/2605.29789")
- Rodriguez and Sattelberger, _Border Bases in the Rational Weyl Algebra_.
  #link("https://arxiv.org/abs/2510.23411")
- Faugère, _A New Efficient Algorithm for Computing Gröbner Bases (F4)_.
  #link("https://doi.org/10.1016/S0022-4049(99)00005-5")
- Decker, Eder, Levandovskyy, and Tiwari, _Modular Techniques for
  Noncommutative Gröbner Bases_.
  #link("https://arxiv.org/abs/1704.02852")
- Hofstadler and Verron, _Signature Gröbner Bases, Bases of Syzygies and
  Cofactor Reconstruction_.
  #link("https://arxiv.org/abs/2107.14675")
- Peraro, _FiniteFlow: Multivariate Functional Reconstruction using Finite
  Fields and Dataflow Graphs_.
  #link("https://arxiv.org/abs/1905.08019")
- Klappert, Klein, and Lange, _Interpolation of Dense and Sparse Rational
  Functions and Other Improvements in FireFly_.
  #link("https://arxiv.org/abs/2004.01463")

#v(10pt)
#align(center)[
  #text(size: 8pt, fill: grey)[
    This document explains research directions. It does not claim that K6 is
    closed, that a post-Janet ray count exists, or that any proposed backend has
    been implemented.
  ]
]
