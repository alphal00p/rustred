# RustRed project goal

## Preamble — user directive (verbatim)

````text
Ok, but did you not read all the markdown listed in the HANDOFF (especially in `./docs/research`)?

the goal is very ambitious, and it is to get inspiration from LiteRed2 (see `./FOR_REFERENCE_ONLY_DO_NOT_PUSH/LiteRed2`) which is a poorly software-designed software in Mathematica (you cannot run it) to generate closing parametric IBPs).

you must implement a version of that, RustRed, in fully rust and maximally using Symbolica (see public API in `./FOR_REFERENCE_ONLY_DO_NOT_PUSH/symbolica`), optimally parallelized and highly efficient, to push to up to 6-loop IBPS (using algorithms fully general in topology and loop counts) suitable in practice for example to build the parametric IBPS for the 6-loop single scale vacuum integral showing up in the 6-loop QCD beta function we aim to compute as a breakthrough eventually using the R-formulat for BPHZ renormalisation as implemented in gammaloop (e.g. see `./FOR_REFERENCE_ONLY_DO_NOT_PUSH/gammaloop`), so such IBPs can be used in a new reduction mode of vakint (see `./FOR_REFERENCE_ONLY_DO_NOT_PUSH/gammaloop/crates/vakint`) which is *pure* rust and symbolica, with numerical masters that will come from AMFlow.

Anyway this is a highly ambitious project you have to delegate multiple agents to (you're mostly just the orchestrator) with the main goal for now to be able to produce closing  parameteric IBPs for 6-loop single scale (actually no scale effectively, m\_uv can be set to 1) vacuum integrals, which will be used for the rest of the program culminating in the 6-loop QCD beta function (though all rustred algos must remain fully generic, but with dedicated lanes highly optimized for that use-cased dynamically automatically opted into).

Re-read thoroughly the HANDOFF, the existing code, and the research markdown files, and layout the goal clearly in GOAL.md (incl. this message verbatim as a preamble) and assign it to yourself.

Remember you must design a very clear and well structured professional code, highly optimized and using Symbolica for all CAS taskes (triple-check any CAS feature really is not available in Symbolica before implementing your own solution for such CAS task).

To unlock multicore use of Symbolica, you can and should use:

`export SYMBOLICA_LICENSE=`dcec4a5e#6a95649c#7dca8216-8afe-57c8-975e-03eb5e68e4ee
(not sensitive info)

Remember, before implementing in vakint the application of the parametric IBPs for performing the reduction (NEVER USE FORM) you must first focus on being able to find them for 6-loops with RustRed (first goal). Howver you can use vakint as an oracle to compare reductions (up to four-loops which is the max currently supported in Vakint). RustRed only generates the parametric IBPs, then a new tensor reduction and IBP application mode `RustRed` in vakint will use rustred to actually perform the reduction But that new mode and anything you do SHOULD NEVER USE FORM, MATHEMATICA, Sympy or anything like this. Pure RUST + Symbolica implementation here.
Also never escalate commands but find workaround if sanbox is being hit.
````

## Authority and staged assignment

The preamble above records the long-term scientific motivation verbatim. The
latest approved plan in this section supersedes its sequencing and defines the
only active assignment for the primary Codex agent (`/root`). The primary
agent acts mainly as architect, orchestrator, integrator, and final verifier;
independent research, implementation slices, and adversarial audits are
delegated whenever useful.

Development is divided into two hard-gated stages:

- **Stage 1 is active:** close and publish single-scale vacuum parametric-IBP
  artifacts through three loops, implement their FORM-free scalar application
  in Vakint, and reproduce Vakint's end-to-end expectations through three
  loops. Vakint continues to use its existing FORM tensor reduction before the
  new scalar backend when tensor numerators are present.
- **Stage 2 artifact production is deferred and must not start without new
  user guidance:** do not enhance tensor reduction, integrate speculative
  collaborator tensor work, or run four- through six-loop closure campaigns.
  The current user direction does authorize deep algorithm research and
  bounded, falsifiable foundry prototypes during Stage 1. LiteRed2 is a
  correctness baseline rather than an architecture target: candidate methods
  must be judged creatively against the eventual six-loop scaling problem,
  with independent research and adversarial viability audits. This permission
  does not authorize claiming or producing a four- through six-loop artifact.

The existing experimental RustRed tensor service and GammaLoop
`TensorReductionMode::RustRed` adapter are frozen. They may remain in their
repositories, but Stage 1 must not extend, redesign, or make them part of the
active acceptance path. Vakint's established `TensorReductionMode::Form`
remains its tensor default and the Stage 1 tensor prepass.

## Current evidence boundary

The clean workspace refactor is complete. The root is a virtual Cargo
workspace containing package/library `rustred`, `rustred-app`, and
`rustred-python`; obsolete prototype solvers, authored recurrences, and legacy
compatibility layers have been removed.

The live core compiles topology-neutral families, generates generic ordinary
IBP and LI source rows, authenticates physical and auxiliary family
presentations, verifies supplied affine symmetries, analyzes requested zero
sectors, and provides deterministic campaign primitives. Its foundry can
derive concrete-anchor and guarded fixed-sector recurrences over Symbolica
`K(n)`, select a requested physical pivot with deterministic Symbolica RREF
while keeping provenance columns free, prove uniform descent, exactly replay
source combinations, partition target-sector domains, and stream proper-
subsector obligations.

The core now freshly generates and seals the canonical unit-mass one-loop
partition and equal-mass two-loop sunset as mathematical and durable closing
artifacts. Their tagged binary representations authenticate complete source,
rule-cell, projection, symmetry, factorization, terminal, and homogeneity
semantics once at the untrusted load boundary. The sunset owner derives all
four ordinary sources, closes its generic and exceptional cells, routes exact
`S3` symmetries, and feeds its pinched face into the immutable one-loop
dependency. A topology-independent deterministic memoizing reducer applies
both sealed owners without repeating whole-artifact authentication. The Rust
application API, `campaign` CLI, and public `import rustred` Python package
generate, inspect, load, and apply those actual artifact bytes with typed
errors and deterministic output.

RustRed still does **not** close or publish the three-loop `K = 6` family and
deliberately does not own evaluated master values. Vakint now ships and lazily
loads the `K = 1` and `K = 3` artifacts on branch `vakint_rustred`. Its opt-in
FORM-free RustRed scalar backend reduces the registered one-loop tadpole,
two-loop sunset, and pinch, maps typed terminals to the existing MATAD basis,
restores a general common mass, and optionally applies Vakint's pure-Rust master
values. Nontrivial scalar tests pass with an invalid FORM path, raw and
substituted results agree with MATAD, the existing FORM tensor prepass remains
unchanged, and the full applicable Vakint regression suite is green through
two loops. Three-loop closure and adapter coverage remain open.

The three-loop search nevertheless now starts from an exact, test-only
pressure manifest rather than an informal topology list: it authenticates the
six-denominator unit-mass family, all nine ordinary sources, and the complete
order-24 `S4` edge action and eleven sector orbits. It freezes an internally
validated five-class Vakint routing snapshot with its exact upstream revision
and source-blob provenance; live cross-repository matching remains an
integration gate. Generic factorization tests separately certify both the
`K3 x K1` sector and both inequivalent `K1 x K1 x K1` spanning-tree sectors.
The first exact top-sector recurrence is derived from the complete nine-source
span as a test-only rule cell with retained provenance, guards, application
bounds, and strict-descent proofs. Exact residual projection on the canonical
five-line face additionally derives the two inequivalent positive dotted-edge
cells from all nine sources, retaining the complete 26-sector zero routing and
strict-descent evidence. Six exact generated-source cells now partition the
negative inactive-power direction into disjoint endpoint and bulk owners on
the all-unit active-power, adjacent active-dot, and opposite active-dot
domains. Their exact endpoint pruning, held-out replay, machine bounds, guard
domains, and `S4` routing are pinned. This advances but does not close the
five-line sector: its scalar corner and the remaining fixed-point branches stay
open. On the canonical irreducible
four-line face, the same exact projection boundary now derives a guarded
canonical-dot multi-excess cell from one target-aligned translated source span
and a canonical mixed numerator/dot cell from the untranslated span. Exact
`S4` tests route all four dotted and all eight mixed placements; the pure-dot
cell's `n1 - 1` guard excludes its isolated corner. A complete depth-one search
also derives endpoint and bulk cells for `J(0,1,1,1,1,n)` over every
representable `n<0`. Independent five-row reprojection proves the bulk through
the `i64::MIN` target; its pinched numerator children remain open instead of
being mislabeled as factorization terminals. A fixed-corner residual
projection now lowers that isolated dot to the scalar corner with the exact
rows `ordinary-ibp:0:0` and `ordinary-ibp:1:0`. A one-dot translated
projection of the complete nine-row ordinary-source layer supplies a
recurrence for the opposite two-dot orbit; exact RREF selects five rows and
produces three strictly descending right-hand-side terms. Those terms remain
subject to the surrounding fixed-point obligations. Exact global
canonicalization covers all four isolated-dot placements and both raw
opposite-pair placements. A generic bounded same-sector search planner now
constructs complete deterministic L1 translation diamonds with exact resource
preflight. A separate topology-neutral reachability planner now applies
ordered rule cells exactly over finite concrete root graphs: terminals precede
rules, guards and coefficients are specialized exactly, zero branches are
dropped, raw descent is proved before optional symmetry routing, and the
deterministic uncovered frontier carries typed work/storage counts. It is a
discovery census, never an infinite-domain closure witness. At the four-line
corner, depths zero and one retain typed target
misses, while depth two contains 28 offsets and the complete 252-row translated
ordinary-source span. Exact targeted RREF selects 16 rows and yields the
adjacent-pair recurrence

\[
J(0,1,1,2,2,0)=
-\frac{J(0,0,2,2,2,0)}{4(d-4)}
+\frac{(d-3)(3d-8)(3d-10)}{64(d-4)}J(0,1,1,1,1,0),
\]

with nine retained exact guards, complete projection replay, strict descent,
and exact `S4` routing of all four raw adjacent placements. The first RHS term
is the already certified spanning-tree `K1 x K1 x K1` dependency. The same
complete diamond separately yields the first deeper endpoint required by the
opposite-pair recurrence,

\[
J(0,1,1,1,3,0)=
\frac{3}{8(d-4)}J(0,0,2,2,2,0)
+\frac{(d-7)(3d-8)(3d-10)}{128(d-4)}J(0,1,1,1,1,0).
\]

Its exact targeted RREF again selects 16 rows, retains nine guards and the
complete 252-row projection replay, and descends only to the certified path
factorization and the prospective scalar four-line terminal. A third target
selection on a separately retained copy of that complete span exhausts the
remaining unit-dot decoration orbit,

\[
J(0,1,2,2,2,0)=
\frac{38-11d}{32(d-4)}J(0,0,2,2,2,0)
+\frac{3(d-3)(d-2)(3d-8)(3d-10)}{512(d-4)}J(0,1,1,1,1,0).
\]

This exact cell selects 17 RREF rows, retains nine guards and the complete
252-row replay, and routes all four raw three-distinct-dot placements under
`S4`. A fourth complete depth-two projection now derives a parametric rule on
the selected repeated-edge ray `J(0,1,1,1,N,0)` for every structural target
`N >= 3`. The pivot shift `[0,0,0,0,2,0]` is obtained from 50 selected source
contributions (358 source terms) and has eight RHS terms, 32 exact guards, and
367 replay keys. Schema-V3 replay uses 1078 exact operations at free index one
and 1080 at held-out indices two and eight. After fixed-coordinate
specialization, a uniform-sign leading-in-`d` coefficient proof shows that no
guard is identically zero in `d` at any positive free index; individual
exceptional dimensions remain guarded. Exact `S4` canonicalization routes all
four choices of repeated active edge.

Two further independently retained copies of the complete depth-two
four-line-corner span derive exact singleton recurrences for the two
inequivalent placements of powers two and three. The adjacent target
`J(0,1,1,2,3,0)` selects 17 generated source contributions containing 105
terms; the opposite target `J(0,1,2,1,3,0)` selects 18 contributions containing
113 terms. Each has two strictly descending children, nine guards, complete
252-row residual-projection replay, and exact schema-V3 concrete replay. All
twelve ordered placements route into the two cells under the authenticated
`S4` action. The remaining deeper mixed-dot points and all numerator faces
remain open.

A complete depth-three search then spans 84 translations and all 756 generated
ordinary rows for the exact corner target `J(0,1,2,2,3,0)`. Its exact
elimination selects 46 rows. Those generated rows are independently
retranslated and reprojected on a one-free-index face, producing a guarded
recurrence for one `S4` orbit of `J(0,1,2,2,N,0)`, structurally `N >= 3`.
The parametric rule uses 13 source contributions containing 90 source terms, has five RHS
terms, seven guards, 96 replay keys, and 275 exact schema-V3 replay operations.
The anchor free index one and held-out indices two and eight reproduce the same
exact metrics; a uniform-sign leading-in-`d` proof establishes that no guard is identically zero
for any positive free index. Its concrete i64 application box owns
`3 <= N <= i64::MAX - 1` and rejects the overflowing final endpoint. The
complete 756-row free-index projection itself retains the typed
exceptional-anchor diagnosis instead of silently selecting a
different rule. A separate complete depth-three projection derives the first
complementary-orbit singleton `J(0,1,2,3,2,0)` from 46 selected contributions
containing 310 source terms, with four RHS terms, 22 guards, 315 replay keys,
939 exact concrete replay operations, and typed target absence through depth
two. Its fixed application box and exact 16/8 `S4` orbit split prevent a ray
overclaim. The rest of that complementary ray and the first exposed descendant
ray `J(0,1,1,2,N,0)` remain explicit obligations. The fixture exposes no
installable artifact until the complete rule fixed point is closed.

Two generated three-line path recurrences now continue the scalar four-line
inactive-numerator child. Disjoint endpoint/bulk cells first lower one exact
`S4` orbit of `J(0,0,2,n,1,1)` and then the undotted
`J(0,0,1,n,1,1)` lane for every representable `n<0`. Their complete depth-one
source spans, algorithmically selected machine-safe rows, full-i64 replay,
guards, descent, terminal routing, and symmetry boundaries are pinned. The
decorated path has five inequivalent `S4` orbits; only the certified one is
owned and the other four remain explicit closure obligations.

The complete untranslated nine-row span also derives disjoint endpoint/bulk
cells for the factorized bridge-dot numerator orbit
`J(0,n,2,1,1,1)`, `n<0`. Independent compact reprojection retains five and six
production sources, respectively; both final cells are guard-free, replay
through target power `i64::MIN`, and descend strictly. The endpoint terminates
in two authenticated factorization sectors. The bulk replaces its mixed
dot/numerator frontier with one decorated-path and one undotted
factorized-face obligation, without promoting either to a terminal.

The decorated bridge descendant `J(-1,0,1,0,2,1)` now has its own exact
singleton cell. The complete untranslated nine-row span selects rows 0 and 3,
and production independently reprojects only those rows into a guard-free,
strictly descending recurrence. Its 24-image `S4` orbit is disjoint from the
other four decorated-path placements. Both children already route to the
installed decorated-path endpoint or factorization owner 2, so this cell
reduces the finite frontier by one without creating another obligation. A
candidate bulk mixed-numerator lane that would increase that frontier remains
deliberately uninstalled.

The remaining direct bridge-bulk child `J(0,-1,1,1,1,1)` is likewise owned
only at its exact endpoint. A complete depth-one search retains all 63 rows,
selects eight, and independently reprojects those eight for production. The
sole guard is `d-1`; exhaustive decoration of every inactive edge over the
12-image scalar-sector orbit reproduces exactly the endpoint's 24-image orbit.
All three children route to factorization owners 2, 0, and 2, so this second
singleton also removes one frontier node without creating another. No
negative-power bulk is inferred.

A third exact endpoint owns only the `S4` orbit of
`J(0,-1,2,2,1,1)`. Its complete depth-one search again retains all 63 rows and
selects nine; production independently reprojects those rows and eliminates
the complete-system's spurious `d-1` guard. All four children are immutable
factorization terminals with owner ordinals 2, 0, 1, and 2. Exhaustive
classification of the six inequivalent two-dot/numerator placements proves
the singleton boundary and keeps the neighboring bulk and higher-dot lanes
open.

The irreducible four-line numerator cells now live under the semantic
`four_line::numerator` module. A complete depth-one search derives an exact
singleton for `J(0,1,2,2,1,-1)`, the placement where the inactive numerator
is incident to both active dots. It selects complete ordinals 18, 21, 27, 28,
and 30; production independently reprojects those five rows and removes the
complete elimination's spurious `6-3d` guard. Exact `S4` enumeration proves
three inequivalent placement classes and owns only this one. Its four children
route to the installed adjacent-pair and triple-dot cells, factorization owner
2, and the unresolved scalar four-line corner, so the finite frontier shrinks
without acquiring a new node.

The same semantic numerator module derives exact endpoints for the opposite
inactive-numerator pair `J(-1,1,1,1,1,-1)` and its one-dot child
`J(-1,1,1,1,2,-1)`. The first independently reprojects four selected rows
from a complete 63-row depth-one search and retains the single effective
`3d-4` guard; the second reprojects two selected untranslated rows and is
guard-free. Both expose `J(0,0,1,-1,2,1)`. A separate guard-free depth-zero
three-line endpoint selects ordinary rows 0 and 3 for that shared child and
routes only to the installed undotted-path cell and factorization owner 2.
Exact `S4` partitions keep all three singleton domains disjoint from the
remaining numerator placements, and the coordinated cluster adds no uncovered
descendant.

The irreducible scalar four-line face now also has a guard-free bulk owner for
the complete machine-wide ray `J(0,1,1,1,2,N)`, `N<=-2`. Its depth-zero
search starts from all nine ordinary rows and independently reprojects selected
ordinals 0, 3, and 4 over `i64::MIN+1<=n5<=-1`, so the target reaches
`i64::MIN`. The exact pivot, source, and right-hand-side coefficients are
replay-pinned; strict descent routes every child to the existing scalar
numerator or decorated-path recurrences, except for the already-open scalar
corner at the endpoint. Exact `S4` ownership covers only the one-dot/inactive-
numerator orbit and rejects its endpoint, higher-dot, two-dot, and two-negative
neighbors.

The opposite inactive-pair endpoint now continues through the exact bulk ray
`J(-1,1,1,1,1,N)`, `N<=-2`. A complete depth-one span selects five generated
rows and independently reprojects them over the machine-safe domain. The new
child is handled by a coordinated three-line path cluster: exact depth-one
rules own the two inequivalent inactive-pair placements
`J(0,-1,1,N,2,1)` and `J(-1,0,2,N,1,1)`, while an untranslated six-row rule
owns `J(0,0,1,N,1,2)`, `N<=-2`. Complete-versus-compact selection witnesses,
guards, full-i64 endpoints, descent, and exact `S4` nonownership are pinned.
These are regression fixtures for the systematic completion work, not a
sample-driven closure representation.

The first K6-specific consumer of that generic planner now runs a deterministic
test-only fixed census. Its 115 submitted probes reduce under exact `S4` to 44
roots and discover 89 nodes: 46 rule cells are registered and produce 53
applications, 27 nodes terminate through independently checked zero or
factorization proofs, and nine remain explicitly uncovered. Three are scalar
corner certification obligations and six are genuine recurrence witnesses;
their exact inventory is frozen in the breakthrough research note. The census
checks first-applicable overlap ownership and never labels the
scalar top, five-line, or four-line corners as masters. It measures the present
frontier; it does not weaken the required zero-uncovered fixed-point
publication gate.

The first exact completion-geometry prototype maps every one of those 46
cells into the corresponding sector-local nonnegative lattice. Exhaustive
`3^6` membership comparisons per cell (33,534 comparisons in total) agree
with `RuleCell::assignment_for_target`. Exact Symbolica expansion in the base
parameters turns every retained guard into simultaneous integer-polynomial
coefficient equations. Of 205 guard occurrences, 119 have an immediate
nonzero constant equation and the remaining 86 depend on exactly one index;
exact GCD, factorization, and replay find every common integer root. None lies
inside its owning application box, so the current 46 cells have no unowned
guard-zero branch. The mapping still keeps guards separate from structural
coverage and leaves all 276 outer coordinate endpoints as explicit extension
obligations. Of these, 61 reach a maximal rule-safe application endpoint but
only 35 actually touch the `i64` chart carrier; neither condition is treated
as a proof of mathematical infinity. On the two sectors containing the six recurrence
witnesses, the exact guard-blind structural complements contain respectively
20 and 32 disjoint boxes after subtracting 7 and 19 rule boxes. Both retain
six-dimensional varying boxes and more than one million carrier points. This
is a lower bound on the true uncovered set and a precise diagnosis of missing
all-rank coverage, not a terminal count or closure claim.

The physical-frame half of the next bounded completion experiment is now an
executable test-only prototype. It deterministically regenerates the complete
one-sided degree-one through degree-three plans for `S6`, `S5`, and both `S4`
sector representatives from the nine ordinary sources. In particular, the
`S4a` degree-one plan has 63 translated rows, 157 raw physical columns, and
630 nonzero entries. Raw shifts alone enter the checked CSR pattern; exact
source/translation provenance remains a row sidecar, and no `S4` quotient is
taken. The next step is the target-specific A0 modular query and exact-lift
boundary. A0 is only a modular discovery and provenance kernel.
Every target has its own forbidden-column rank query; provenance columns never
enter that physical rank; no `S4` quotient is taken during assembly; and a
modular miss remains inconclusive. Only an exactly lifted and replayed relation
may refine the exact decorated-stratum complement. Completion state remains
separate by sector, fixed/free coordinates, application box, and guard branch.
A finite strictly descending rewrite partition may close on an affordable
nonminimal typed terminal set without constructing a minimal quotient or all
shift-action matrices.

The MATAD oracle fixes the eventual in-family basis boundary without being used
as a rule generator. The scalar six-line and four-line corners map directly to
`miD6` and `m_uv^4 miBN`, respectively. The scalar five-line corner is a third
independent RustRed terminal but is not identical to MATAD's `miD5`: MATAD's
definition includes a massless `1/p^2` auxiliary denominator outside the fixed
all-massive K6 lattice. Exact raw MATAD oracles for three symmetry-equivalent
missing-edge representatives fix the unit-mass basis-change row

\[
T_5(d)=
\frac{4\,\mathrm{miT111}\,\mathrm{Gam}(1,1)}{(d-3)(d-4)}
+\frac{3(d-4)}{2(d-3)}\,\mathrm{miD5}
+\frac{8-3d}{8(d-3)}\,\mathrm{miBN}
+\frac{16\,\mathrm{Gam}(1,1)^3}{(d-2)(d-3)^2(d-4)^3}.
\]

Vakint will own this exact row and restore the common physical factor `m_uv^2`;
the degree-six denominator factorization above is independently checked with
Symbolica. These three typed candidates are not installed as artifact masters
until the remaining numerator fixed point and publication checks are complete.

The bounded scalar/odd/rank-two RustRed vacuum projector and its optional
Vakint adapter are existing experimental capability. They do not establish
rank-generic tensor reduction and are frozen for Stage 1.

## Stage 1 objective

Build a production-grade, topology- and loop-count-independent offline rule
foundry, written entirely in Rust and using GMP-enabled Symbolica as its sole
CAS. For the active pressure domain it must derive guarded, strictly
descending, exactly replay-certified, coverage-closed replacement systems and
persist them as deterministic reusable artifacts.

Stage 1 freezes a matcher-derived manifest with all eight Vakint graph classes
through three loops:

| Family artifact | Coordinates `K` | Ordinary sources | Required Vakint coverage |
|---|---:|---:|---|
| one-loop tadpole | 1 | 1 | the one-loop class |
| two-loop sunset | 3 | 4 | the sunset and its pinch |
| three-loop K4/Mercedes | 6 | 9 | the parent and four inequivalent contractions |

One sector-complete unit-mass artifact is produced for each row of this table.
The `K = 6` artifact must prove coverage of all five registered three-loop
classes; topology names are manifest labels and fixtures, never algorithmic
dispatch keys. Pinches, symmetries, factorization, and routing are handled
through generic proved transformations.

All production algorithms remain generic in topology and loop count, including
the Rust library, `campaign` CLI, and public Python package. Python users write
`import rustred`; `rustred._rustred` is private and top-level `import _rustred`
is unsupported. Generic non-vacuum family construction and identity generation
remain first-class even though Stage 1 closure pressure is the vacuum manifest.

### Unit-scale contract

The closing search and shipped tables use a proved common squared mass
`s = m^2 = 1`. For `L` loops and powers `a`,

```text
I(a; s) = s^(L*d/2 - sum(a)) I(a; 1).
```

Consequently the coefficient restoring a unit-scale reduction from target
`a` to master `b` is

```text
c[a -> b](s) = s^(sum(b) - sum(a)) c[a -> b](1).
```

This includes negative auxiliary powers. Specialization requires authenticated
single-scale homogeneity and nonzero scale evidence; dimensional analysis does
not excuse a convention or routing mismatch.

## Definition of closure

RustRed distinguishes mathematical in-process closure from durable production
publication. A root may become the immutable in-process `ClosedArtifact` only
when it establishes items 1–6 below. It may be called a published or production
artifact only after it also establishes item 7:

1. The exact family, coefficient/index contexts, kinematics, metric and
   propagator conventions, routing, cuts, power shifts, ordering, and freshly
   generated source set are bound to one canonical identity.
2. Every required ordinary IBP and LI identity is generated generically. For
   `L` loops and `E` external momenta,
   `K = L(L+1)/2 + LE`, with `L(L+E)` ordinary sources and `E(E-1)/2` LI
   sources.
3. Every rule carries exact integer-domain and nonzero-polynomial guards,
   strict well-founded descent, source provenance, and a zero residual against
   freshly regenerated identities.
4. Zero, symmetry, cross-family maps, factorization, product structure, and
   proper-subsector dependencies are proof-bearing.
5. Every generic and exceptional branch reaches a descending rule, an already
   closed dependency, an explicitly enumerated master, or an independently
   certified zero/product/factorized terminal. A residual or failed search is
   never a terminal.
6. Solved dependencies feed back immutably until the reachable dependency
   graph reaches a deterministic fixed point with no uncovered, unsupported,
   resource-limited, interrupted, or unresolved leaf.
7. The deterministic durable representation is bounded and validated once at
   the untrusted load boundary before conversion to the sealed owner; the
   reduction hot path does not repeat whole-artifact authentication.

Finite-field or numerical samples may propose candidates, but exact
regenerated-source replay is mandatory.

Closure does not require a *minimal* master basis. If exact all-rank coverage
proves that the residual complement is finite, RustRed may publish those
finitely many keys as an explicit, versioned set of evaluation terminals.
Merely observing finitely many misses in a bounded census is not such a proof:
the misses may lie on an uncovered infinite ray or algebraic guard locus. For
Stage 1, every accepted nonminimal terminal must additionally carry either an
exact basis-change row to Vakint's existing MATAD masters or a separately
validated, shipped high-precision Laurent evaluation. At higher loops, a
finite nonminimal set may instead receive precomputed AMFlow values. Minimality
is an efficiency and canonicalization objective, not a closure requirement.

For the eventual six-loop programme, favorable closure scaling takes priority
over reproducing a historically minimal basis. Every campaign therefore
records a terminal budget: terminal count, simultaneous AMFlow feasibility,
required precision and storage, and numerical conditioning. The finite
universal terminal set must remain small enough to evaluate once at very high
precision and ship, but it need not be minimal. At three loops, Vakint's MATAD
mode is an authorized offline oracle for discovering exact relations missing
from RustRed's current rule set and for producing high-precision reference
values. Oracle output may guide and validate freshly replayed RustRed rules or
terminal maps; FORM/MATAD never enters the production RustRed scalar path.

The provisional direct-numerical-basis budget is green only for at most 100
terminals whose largest measured auxiliary AMFlow system has dimension at
most 100. Counts through 1,000 terminals and system dimension 300 are
conditional on a successful simultaneous-evaluation pilot; larger proposals
return to completion or compression unless measurements justify a reviewed
exception. Exact finite closure alone does not prove AMFlow computability.
Every pilot must also bound accumulated `(d-4)` pole depth with index rank,
because an unbounded spurious-pole order would require an unbounded Laurent
table even for a finite terminal set. These are falsifiable engineering gates,
not claims about AMFlow's theoretical limits.

## Stage 1 implementation tracks

### Closing foundry and artifacts

1. Refine exact coefficient and guard applicability on target cells, including
   exceptional equality and nonzero branches.
2. Add translate-before-substitute residual search, immutable lower-sector
   feedback, and a deterministic fixed point with proof-bearing symmetry,
   zero, factorization, product, mapping, and terminal providers.
3. Introduce versioned immutable artifact ownership only with its first closed
   family. Keep incomplete resumable workspaces structurally distinct from
   installable artifacts.
4. Close, replay, publish, and independently audit the `K = 1`, `K = 3`, and
   `K = 6` families in that order.

### Rule application and public APIs

Add a deterministic, memoized, strictly descending artifact applier. It
selects applicable guarded rules, detects malformed artifacts or cycles at the
trusted boundary, returns exact coefficients of typed master keys, and restores
the common scale by homogeneity. It never regenerates the artifact during an
ordinary reduction.

Expose closing-artifact generation, inspection/replay, and reduction through
the Rust library, `campaign` CLI, and `import rustred` Python API. All three
frontends call the same application services and produce the same deterministic
semantics.

### Vakint scalar backend

On GammaLoop branch `vakint_rustred`, add the opt-in scalar evaluation backend
`EvaluationMethod::RustRed(RustRedEvaluationOptions)` and
`EvaluationOrder::rustred_only()` without changing existing defaults or
behavior. The adapter:

- consumes Vakint's existing topology match and simultaneous routing witness;
- never rematches a graph, dispatches on a topology name, or duplicates the
  topology registry;
- applies shipped immutable RustRed artifacts and never regenerates them at
  evaluation time;
- returns exact coefficients of typed RustRed evaluation terminals; Vakint
  uses an exact MATAD-basis map when one exists, otherwise it substitutes a
  separately validated shipped high-precision Laurent table generated once
  with MATAD; future four-loop data may use exact FMFT reductions, but the
  currently shipped FMFT numerical tables are mostly only 26--50 digits, so
  generic 20,000-digit terminal data would require regeneration or AMFlow;
- exposes master substitution control, enabled by default; and
- reports no FORM dependency and never invokes or falls back to FORM for
  scalar IBP reduction or master substitution.

Production artifacts are generated once, checked into and shipped with Vakint,
and loaded once. A local path dependency may be used while co-developing; every
pushed GammaLoop milestone pins the exact validated RustRed Git revision.

“FORM-free RustRed backend” refers precisely to the scalar IBP application and
master-substitution tail. For tensor-bearing inputs, Stage 1 intentionally runs
Vakint's unchanged FORM tensor prepass first. Such a complete tensor-bearing
evaluation is therefore not claimed to be FORM-free. Scalar or already
tensor-reduced inputs can exercise the RustRed backend with an invalid FORM
path to prove that the backend itself has no hidden FORM dependency.

## Stage 1 milestones and acceptance

1. Commit and push the authoritative staged goal and documentation.
2. Close and publish the one-loop artifact with the artifact/reducer spine.
3. Close the two-loop family, cover its pinch, and expose Rust, CLI, and Python
   artifact/application APIs.
4. In parallel with three-loop closure, integrate and validate Vakint RustRed
   scalar reduction through two loops.
5. Close the `K = 6` family and prove coverage of all five registered
   three-loop graph classes.
6. Pass all applicable single-scale Vakint acceptance tests through three
   loops, update documentation, commit and push both repositories, then pause.

Acceptance requires:

- exact regenerated-source replay, strict descent, explicit terminals, and no
  uncovered branch in each installed artifact;
- exact finiteness of any nonminimal terminal complement, plus exact MATAD
  basis-change rows or independently validated high-precision Laurent values
  for every such Stage 1 terminal;
- deterministic artifacts and reductions across supported worker counts;
- guard selection, termination, memoization, symmetry routing, terminal-only
  output, and non-unit-mass restoration tests;
- exact raw terminal-coefficient tests inside RustRed and matching numerical
  Laurent-series expectations against AlphaLoop/MATAD across the applicable
  Vakint harness; exact cross-backend raw-master comparison is required only
  where an explicit common-basis map exists;
- scalar RustRed-backend tests with an invalid FORM path;
- tensor-bearing tests using the unchanged FORM tensor prepass followed by the
  FORM-free RustRed scalar tail; and
- unchanged Vakint public API conventions, defaults, and existing FORM-backed
  behavior, together with a negative test that obsolete RustRed artifact
  schemas are rejected rather than migrated, dual-decoded, or used through a
  fallback.

PySecDec comparisons are optional, non-gating corroboration.

## Stage 2 — deferred, not authorized

Stage 2 preserves the long-term ambition from the historical preamble, but no
four- through six-loop artifact-production or high-loop performance campaign
may begin until the user provides the collaborator's tensor-reduction
direction and explicit permission. Stage 1 may research and prototype the IBP
breakthrough now, provided every experiment is bounded, topology-neutral, and
directly falsifiable on the three-loop `K = 6` pressure family. Stage 2
includes:

- integrating or replacing tensor reduction and making it generic in rank;
- changing Vakint's tensor preprocessing away from FORM;
- closing four-, five-, and six-loop vacuum manifests;
- high-loop-specific distributed-memory, reconstruction, and extreme
  efficiency work; and
- the eventual six-loop QCD beta-function evaluation chain.

Stage 1 code must not preclude Stage 2. Research prototypes become durable
infrastructure only after measured K6 evidence; attractive but unvalidated
architecture is documented and killed or retained explicitly.

## Engineering and repository invariants

- Production RustRed and the Vakint RustRed scalar backend use only Rust plus
  GMP-enabled Symbolica. They never execute FORM, Mathematica, SymPy, or
  another CAS. The explicitly retained Vakint FORM tensor prepass is an
  external legacy stage, not a RustRed algebra provider.
- Search Symbolica's public API, Rustdoc, source, examples, and tests before
  implementing any algebraic primitive. RustRed owns physics meaning,
  authentication, guards, ordering, provenance, resource admission, and exact
  replay; it does not grow a second CAS or graph-isomorphism engine.
- Symbolica's intrinsic graph generation/canonization/isomorphism facilities
  are the symmetry-candidate authority. RustRed owns physics-colored encoding
  and exact momentum/routing replay.
- Use semantic module ownership; do not revive chronological, `generated`,
  `residual`, `runtime`, `legacy`, or `misc` buckets. RustRed has no pre-release
  compatibility promise. Vakint preserves its public API conventions, defaults,
  and existing FORM-backed reduction methods, but it deliberately provides no
  compatibility layer for obsolete RustRed parametric-IBP artifact schemas:
  shipped and user-supplied artifacts must use the single current schema.
- Validate untrusted inputs and durable artifacts at their boundary. Do not
  accumulate repeated internal authentication ceremonies in the hot path.
- Deterministic parallel work uses one bounded coordinator/pool, shared
  immutable state, RAM-aware admission, stable ordinals, and sorted merges.
  Stage 1 implements only the parallelism justified by three-loop workloads;
  high-loop execution belongs to Stage 2, while scaling models and bounded
  K6 experiments are active research inputs now.
- `FOR_REFERENCE_ONLY_DO_NOT_PUSH` is ignored and never enters RustRed history.
  GammaLoop inside it is a separate repository and branch.
- Never escalate commands. Use rollback-sized commits, push passing milestones
  frequently, and configure every Git operation with:

  ```text
  user.name=ValentinHirschi
  user.email=valentin.hirschi@gmail.com
  ```

Do not claim Stage 1 complete until all three artifacts cover the frozen
through-three-loop manifest and the Vakint RustRed scalar backend reproduces
the applicable acceptance suite. At that point, pause; do not roll directly
into Stage 2.

## Stable project documentation

- [Architecture](docs/architecture.md)
- [Algebra and Symbolica boundary](docs/algebra.md)
- [Frozen tensor boundary and Vakint sequencing](docs/tensor.md)
- [Closing-rule foundry design](docs/foundry.md)
- [Application, Python, and Vakint interfaces](docs/interfaces.md)
- [Validation and oracle ladder](docs/validation.md)
- [LiteRed2 semantic reference](docs/references/litered2.md)
- [Parametric-IBP breakthrough research](docs/research/parametric_ibp_breakthrough.md)
- [Independent breakthrough viability audit](docs/research/parametric_ibp_breakthrough_audit.md)
- [Primary-literature synthesis through 2026](docs/research/parametric_ibp_literature_2026.md)
- [Finite-frame breakthrough candidates](docs/research/finite_frame_breakthrough_2026.md)
- [Symbolica finite-frame feasibility audit](docs/research/symbolica_finite_frame_feasibility.md)
- [Nonminimal-terminal viability audit](docs/research/nonminimal_terminal_viability_audit_2026.md)
- [Independent six-loop candidate shootout](docs/research/six_loop_candidate_shootout_2026.md)
- [Universal nonminimal closure evidence update](docs/research/universal_nonminimal_closure_review_2026.md)
- [Graph-orbit and Baikov source-compression audit](docs/research/graph_orbit_baikov_source_compression_2026.md)
- [Executable K6 breakthrough prototype specification](docs/research/k6_breakthrough_prototype_spec_2026.md)
- [Six-loop algorithm and implementation update](docs/research/six_loop_algorithm_update_2026.md)
- [Vakint K6 oracle and terminal-budget audit](docs/research/vakint_k6_oracle.md)
- [Current CLI contract](docs/CLI.md)
