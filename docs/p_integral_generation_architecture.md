# Closed p-integral reduction programs: generation architecture

Status: design (milestone M1, 2026-11-03). Nothing described here is implemented.
It is implemented in M2 (1L-3L) and M3 (4L) and reused unchanged at 5L.

Amended 2026-11-03: Forcer's rule cascade is adopted (plan §3.5; DECISIONS ~11:00, ~11:20, ~11:50,
~12:50; evidence in `WS/SCRATCHPAD/forcer-tricks/` and `WS/notes/forcer-tricks-*.md`):

- the lanes become Zero > SeriesParallel > Insertion > Carpet > Triangle > Diamond > Custom;
- eps-lines (Forcer's crossed lines) and Gamma tags are first-class;
- the graph engine is a new graphica-based crate `rustred-topology`, with a cached, eps-sensitive
  topology-analysis service (§2.4).

Scope:

- Massless propagator-type integrals ("p-integrals"):
  - one external momentum `q`, every line massless;
  - integer propagator powers and arbitrary integer numerators.
- Reduction coefficients stay exact in `Q(d)`; nothing is expanded in `eps` inside RustRed.
- The vacuum lanes, artifacts and caches are untouched. Every new behaviour is opt-in or
  lives in new modules.

Evidence labels:

- [M] measured;
- [R] read in the code at the cited `path:line` (main `3e83ae91`);
- [D] derived;
- [E] estimate.

Paths:

- `WS` = `/shared/vhirschi/t3code/vakint_dev/p_integral_dev`.
- Probes written for this design: `WS/SCRATCHPAD/rustred-arch/`:
  - `lane_census.py`: lane of every labelled sector, 2L-5L;
  - `canary_lanes.py`: lanes of the measured 5L canary sectors;
  - `np4_lanes.py`: lanes of the 4L non-planar failures.
- Earlier evidence: `WS/study/pint-closure-study/REPORT.md` (strategies S1-S15, loci L0-L4,
  experiments E1-E4) and `WS/study/pint-study/notes/{rustred,feasibility}.md`.
- Forcer citations: `pf` = `generation/pathforcer.py` and `fa` = `forcer/forcer-aux.h` of
  benruijl/forcer `68a30bf`; `Th` = Ruijl's thesis; `FP` = the Forcer paper (arXiv:1704.06650);
  `DR` = the diamond-rule paper (arXiv:1504.08258).

## 0. Decisions

1. **Closure by construction, no walk.**
   - Every owner class is generated all-rank: no `max_numerator_rank`, `SearchFinite`.
   - Coverage holds for every integer point because of five things together:
     - each owner program is complete;
     - every rule descends strictly;
     - the route census is complete;
     - zero proofs cover the scaleless sectors;
     - every place where the generator refused is recorded in a finite ledger and discharged
       locally.
   - Correctness never depends on the ledger, because runtime dispatch checks every guard
     exactly. Only coverage depends on it.
2. **Owner = marked-matroid class.**
   - The class key is the existing projector-graph key of symmetry discovery.
   - The glued `q` line is added as one extra row with a new `External` vertex colour.
   - Whitney twists are merged for free, because the projector is a matroid invariant.
   - Each line row is coloured by its eps units `j` (Forcer's crossed lines). The eps on the
     external line is excluded: it is a prefactor and lives in the Gamma tag (§3.2).
3. **Seven lanes, Forcer's cascade, chosen per DAG node by a fixed decision function.**
   - The lanes, in order: `Zero` > `SeriesParallel` (fast path of Insertion) > `Insertion` >
     `Carpet` > `Triangle` > `Diamond` > `Custom` (SpIRed, all-rank, with ledger; Forcer's custom
     nodes, eps-nodes included; called Generic in the first draft).
   - Forcer's order is insertion > carpet > triangle > diamond > custom [R pf:1392, :2763;
     Th p.67].
   - Reserved lanes with frozen interfaces: only `Product` (block factorisation) and
     `HigherInsertion` (integrating higher-loop insertions; 283 -> 279 custom nodes at 5L [M]).
   - `ShiftedPrimitive` is dissolved into `Insertion` plus `Custom` on eps-nodes.
4. **Cascade census** [M] (exact; replaces the first draft's WL lane census):
   - 4L: 427 DAG nodes (Forcer's 438 with eps-on-q keys), 21 custom.
   - 5L: 6,197 nodes, 283 custom: 129 5L integer, 88 4L-eps, 42 3L-eps, 14 2L-eps and 10 lower
     integer.
   - M1's exact series-parallel / triangle / generic class counts: 4L 79 / 107 / 94; 5L
     547 / 1,898 / 1,697 (the WL lower bounds were 544 / 1,867 / 1,636).
   - On the measured 5L canary (E1), 26 of the 33 batch-B failures are rule nodes. Section 3.6
     has the details.
   - The three failing 4L non-planar sectors are insertion nodes.
5. **Refusals become typed ledger entries.**
   - A new `LocusPolicy::Ledger` turns an exceptional branch the generator cannot admit into a
     `DeferredLocus`, instead of aborting the sector at the `?` in
     `solver/sector.rs:484-490`.
   - The rule is still published with all of its exception branches, so dispatch refuses it on
     the deferred locus.
6. **Discharge ladder.**
   - L0 (exact emptiness) and L1 (affine-bilinear divisor split) run inside the
     case-intersection engine.
   - L2 (portfolio fall-through, chart re-solve, deeper search) runs in the rule selection and
     the sector queue.
   - L3 (locus-chart templates: eps-triangle, S0-scan rules) and L4 (bounded rank split) are
     post-hoc rungs whose output is owner overlays. Triangle and Diamond are lanes, not rungs.
   - Ledger entries arise only in custom nodes.
   - Whatever is left falls through to the runtime safety net: local Laporta through the bridge,
     cached as an overlay rule.
7. **Global descent certificate.**
   - The measure is lexicographic: (number of positive lines `t`, then the owner's compiled
     order).
   - It requires support-primary owner orders and routes that preserve `t`.
   - Lanes that are not rule-based emit only terminals or keys with smaller `t`.
8. **Route census.**
   - It covers all labelled sectors of all top families, plus the three subsector routes of each
     triangle owner.
   - Every route is a signed affine loop map (`q -> ±q`), verified by the existing family-map
     verifier.
   - Routes from caller-defined families are found on demand and cached.
9. **Basis change.**
   - Terminals are mapped onto the reference master basis: Forcer's up to 4L, GGPPSS at 5L.
     The map is obtained by reducing the reference masters with our own library, exactly in
     `d`.
   - Kira runs as a clean-room external process, only for relations that are still missing.
   - The result ships as a relations layer v2 with row provenance.
   - The series-parallel lane's masters carry closed Gamma-function values.
   - Terminals carry Gamma tags; vakint expands each tag exactly, to any eps depth.
10. **Application.**
    - One new Ordered adapter (`PKey` = presentation + Gamma tag + key, stages Route/Apply).
    - Unchanged `modular_polynomial` `Z_p(d)` replay and concurrent windows.
    - Step, tail and result caches keyed by owner class, so they are shared across families,
      diagrams and users.
    - Caches are versioned by constants, never by program digests.

## 1. Vocabulary

| Term | Meaning |
|---|---|
| top family | An L-loop p-family: one (L+1)-loop cubic vacuum parent with one line cut. Counts: 1/1/3/11/64 at L = 1..5. `C = L(L+1)/2 + L` coordinates (2/5/9/14/20). |
| presentation | An `IntegralFamily` plus the role of its coordinates. The kinds are: labelled top family, owner presentation, caller family. Identified by `PresentationId(u32)`. |
| labelled sector | A (presentation, positive-line mask) pair. |
| owner class | The marked-matroid class of a nonzero sector (glued graph `Gamma + e0`, `e0` = the `q` line). Identified by `OwnerClassId(u64)`. |
| owner | The program serving one class: a presentation, a lane, a program, its terminals and a descent witness. |
| route | A verified map from a labelled sector to its owner's presentation. It is a signed affine loop map. |
| eps-line | A line whose power carries `j` eps units, `a + j eps` (Forcer's crossed line). It comes from an integrated insertion and is never pinched. |
| eps-node | A DAG node with at least one eps-line; its family has `power_shifts`. |
| Gamma tag | A sparse small-integer exponent vector over `Gamma(1 + m eps)`, `m != 0`. Tags add along a path. |
| DAG node | A topology of the discovery DAG (§2.4): eps-coloured canonical graph plus its lane. |
| terminal | A key at which reduction stops: (tag, trivial node) and (tag, custom-node residual). |
| ledger | `LocusLedger`: every generator refusal and how it was discharged. |
| library | Everything above for one loop count, plus the relations layer and version constants. |

Invariant: every key the application meets is one of three things:

- a zero (zero proof);
- a terminal;
- a key of exactly one owner, reached by exactly one route (Route stage), then reduced by that
  owner (Apply stage).

## 2. Owner classes, presentations and the census

### 2.1 Class key

- **Discovery today.** `sector/symmetry/discovery` builds a graph from the rational projector
  `P = Q (Q^T Q)^-1 Q^T` of the line-momentum matrix and canonicalises it natively.
  - A signed line permutation is realisable by a loop change of basis iff it preserves `P`
    [R `discovery/mod.rs:1-18`].
  - `P` depends only on the cycle space, so the key is a matroid invariant. Hence 2-isomorphic
    (Whitney-twisted) sectors get the same key.
- **Change 1.** `eligible()` (`discovery/mod.rs:63-74`) admits `E = 1` families with a numeric
  Gram matrix (done in P0).
  - From M2 it also admits power shifts (`discovery/mod.rs:69`): eps-nodes are owners too.
  - Each line row is coloured by its eps units `j`. The eps on the external line is excluded: it
    is a prefactor and lives in the Gamma tag.
- **Change 2.** `line_momentum()` (`discovery/line.rs:52`) currently skips lines with external
  components. It now returns rows over `(k_1..k_L, q)`.
- **Change 3.** One virtual row `e0 = (0,..,0,1)` is appended (the glued line) with a new
  `VertexColor::External` (`discovery/mod.rs:92-96`).
  - The projector lives on the `(L+1)`-dimensional space of the glued vacuum support.
  - A signed permutation that fixes `e0` up to sign is a loop map `k -> A k + b q` with
    `q -> ±q`. That is exactly the marked-matroid symmetry [D, pint-study `rustred.md` §1].
- **Identifier.**
  - `OwnerClassKey { loops, lines, canonical: Arc<[u8]> }` holds the canonical graph encoding.
  - `OwnerClassId(u64)` is the low 64 bits of `blake3(canonical)`. It is displayed as
    `L4T09-<16 hex>`.
  - The digest is a name, not a certificate. Its stability is governed by
    `P_CLASS_KEY_VERSION`, which is bumped when the Symbolica canonical form or the encoding
    changes.
- **Rejected alternative.** Graph canonisation with marked external vertices gives 4,142 classes
  at 5L instead of the 2,243 marked-matroid classes, and routes would then also need explicit
  Whitney-twist maps.
- **Lane decision on a class.** The lane matchers are graph tests, so they run over every graph
  representative of the class, or use matchers stated in cocircuit terms.
  - The action is class-invariant in 22/22, 183/183 and 2,242/2,243 classes at 3L/4L/5L. The one
    5L exception is a Whitney pair with its triangle on one twist only [M].
- **CI test.** The graph-side marked-matroid key and the projector key partition the 180,644 5L
  labelled sectors identically (2,243 classes).

### 2.2 Presentations

Each class has one owner presentation, chosen deterministically:

- **Induced** (series-parallel and custom lanes).
  - The labelled sector that comes first in the order (t descending, family name, mask) is the
    representative.
  - The owner presentation is its top family, with the owner sector = that mask.
  - Consequences:
    - owners of the same top family share one `IntegralFamily` and one `SourceSystem`;
    - every child of a custom rule is a labelled sector of that top family, which the census
      already routes;
    - this is the configuration in which E1 and E2 were measured.
- **Triangle** (triangle lane).
  - The family has the class's `t` lines and a loop basis adapted to the chosen triangle:
    - take a spanning tree of the sector graph that contains `a1` and `a2` but not `b`;
    - route `q` along the tree path between the external vertices;
    - the cotree momenta are the loop momenta, so `k := mom(b)` occurs only in `b`, `a1` and
      `a2`.
  - The ISPs come from the existing deterministic completion `family/isp/completion.rs`, which
    adds scalar-product unit rows. In this basis every added ISP is either `k.X` (linear in `k`)
    or free of `k`. That is exactly the presentation the triangle rule needs (§3.3).
  - The sector is the top sector. Only three subsectors are reachable (`b`, `c1` or `c2` dropped)
    and each gets one route.
- **Insertion-adapted** (insertion lane). The loop momentum runs through the bubble pair; the
  transverse decomposition of §3.2 applies.
- **Carpet-adapted** (carpet lane). The outer loop momentum is `p`; no ISP contains `p1` or `p2`
  (fa:1105).
- **Diamond-adapted** (diamond lane). Dots avoid the A-lines; the basis is `k_i`/`l_i`
  (Th eq. 26).
- **eps-node** presentations. Fewer coordinates, `power_shifts`, padded to `N` in `PKey`.
- **Caller** families (vakint input built from an Atom or a HEPKit dot graph with an arbitrary
  routing). They are registered at run time and routed on demand (§5.4).

A presentation is a pure function of (NodeKey, lane), so that caches stay stable. Forcer's
parent-aware dot-basis choice (minimal `Sum (#rewrite terms)^2`, Th p.68) is allowed only as a
frozen, versioned choice.

### 2.3 Census (`rustred p-census`)

Input (`p-families.json`, schema `rustred.p-families.v1`): one entry per top family, with:

- `name` and `loops`;
- `vertices`, and `lines`, each line with `endpoints` and `momentum`;
- `isps`;
- `external: {in, out}`;
- `gram: [[-1]]`: Minkowski `q.q = -1`, i.e. Euclidean `Q^2 = 1`, as vakint registers its
  families (P0 admits `q.q = ±1`, `docs/p_integrals.md`).

Sources:

- at L <= 4: glue-and-cut of the vakint/FeynKit vacuum parents;
- at 5L: `WS/study/pint-closure-study/scratch/families` (64 families plus `index.json`).

The census carries the graph (as `FamilyPresentation` momentum-routing metadata), because the
lane tests are graph tests on the representative.

Roots and passes:

- Roots = every integer sector class of every top family, eagerly. The resulting IBP set is
  identical to demand-driven discovery [M], and eager roots cover arbitrary caller sectors.
- `p-census` = DAG pass A (§2.4) plus the existing integer labelled-sector route census below,
  kept as a cross-check.
- Completeness: every DAG edge target is a node or a zero proof.

Steps of the labelled-sector route census:

1. Run the zero analyzer over all `2^t` masks of each family (`sector::zero::Analyzer`, exact;
   existing `ZeroTerminalProof` tags).
2. Compute the class key of each nonzero mask, then assign the representative and the lane (§3.1).
3. For each non-representative sector:
   - derive the signed line permutation, then solve the integer loop map `(A, b, c)`;
   - check `det A = ±1` and `c = ±1`;
   - run `sector::symmetry::verify` (two families, external Gram congruence [R
     `verify/mod.rs`, `verify/kinematics.rs:9-60`]).

   The census stores `(A, b, c)` only. `integral_transport::compile` runs lazily at load time.
4. Completeness check: the number of routes plus zeros equals `2^t` for every family. Every class
   has an owner or a ResourceStop ledger entry, so `MissingOwner`
   (`candidate_reduction/routed/model.rs:173-176`) cannot happen.

Cost [M/E]:

- the graph criterion takes 8 s over all 180,644 5L labelled sectors in Python;
- the RustRed census is minutes at 4L and under 1 core-h at 5L [E].

### 2.4 Discovery DAG and topology service (`crates/rustred-topology`)

A new crate `rustred-topology` [R graph-tools]:

- It depends only on graphica (`symbolica::graph`).
- Not linnet: linnet has no 2-vertex cuts, no SPQR and no matchers, and its default features pull
  `gammaloop-workspace-hack` and rand 0.8 (`linnet/Cargo.toml:45,69,79-80`). linnet, FeynKit and
  HEPKit stay as ingress (dot graphs) and as rendering of matches and of the DAG.

API:

- `PTopo::normalise`: zero test; series merges that add eps units.
- `graph_key`: eps-coloured canonical form plus automorphisms.
- `class_key`: the marked-matroid key of §2.1.
- `decompose`: the 2-separations of `Gamma + e0`.
- `matches(order)`: the lane roles (`a_i`, `b_i`, `c_i`, spectators, `k`-line), the adapted basis
  and the children.
- `reduction_graph(roots, order)`: Forcer's Alg. 2, level-ordered by `t`.

Output and cache:

- `p-topologies.json`: nodes, actions, roles, edges and edge maps; versioned by
  `P_TOPOLOGY_VERSION`.
- In-process cache `DashMap<GraphKey, Arc<NodeInfo>>`. The census, the generation and the runtime
  caller routing share it.
- Cost: 5.35 us per canonisation; the 5L DAG in 3.6 s [M].

Gate: the 4L DAG equals Forcer's 438 nodes node by node, with identical actions and 21 custom;
43 nodes at 3L; 7 at 2L [M simulator].

## 3. Lanes

### 3.1 Decision function (`propagator/lane.rs`)

```text
lane(node) =
  Zero                 if the zero analyzer proves the sector scaleless
  SeriesParallel       if Gamma + e0 is series-parallel (no K4 minor; a matroid property)
  Insertion(pair)      the first parallel pair (one-loop bubble) not containing e0
  Carpet(r)            an element parallel to e0 with a nontrivial blob
  Triangle(tri)        if the representative graph has an ADMISSIBLE triangle
  Diamond(dia)         if the representative graph has an admissible diamond (§3.5)
  Custom               otherwise
```

**Admissible triangle** (exactly Forcer's rule):

- lines `b`, `a1`, `a2` form a 3-cycle;
- both endpoints of `b` are internal vertices of degree 3;
- their third lines `c1` and `c2` are active lines, with `c1 != c2`;
- `c1` and `c2` are not triangle lines;
- `b`, `c1` and `c2` are integer lines; `a1` and `a2` may carry eps;
- numerators are only `k.X` (pf:1450-1481, fa:1536-1542).

In momentum terms, `mom(a1) - mom(b)` and `mom(a2) - mom(b)` are, up to sign, the momenta of two
distinct active lines. When the third line is `q`, or `p1` is the sum of two lines, the rule
would contain `p1^2` terms that do not descend. Such triangles are inadmissible.

Choice among admissible triangles:

1. most series-parallel children;
2. then Forcer's canonical image that minimises the powers of the lowered lines (pf:2242-2245).

The choice is deterministic.

Census of the first draft [M] (M1 lanes only, WL lower bounds; the generic column now splits
into Insertion, Carpet, Diamond and Custom, see §0 item 4):

- About 92.7% of 5L labelled sectors contain some graph triangle. Only 26.1% take the triangle
  lane, that is, they are not series-parallel and have an admissible triangle.
- Of the 1,636 generic 5L classes:
  - 1,018 have only inadmissible triangles;
  - 1,488 contain an insertion;
  - 148 are primitive;
  - 60 are primitive and triangle-free (pint-study: 69).
- At 4L, 89 classes are generic; 76 contain an insertion and 7 are primitive and triangle-free.

| L | t <= 7 | t = 8 | t = 9 | t = 10 | t = 11 | t = 12 | t = 13 | t = 14 |
|---|---|---|---|---|---|---|---|---|
| 5 (sp/tri/gen classes) | 11/0/0 | 70/1/19 | 213/27/159 | 250/249/487 | 0/701/574 | 0/610/290 | 0/242/92 | 0/37/15 |
| 4 (sp/tri/gen classes) | 37/1/8 | 42/14/31 | 0/53/36 | 0/30/11 | 0/6/3 (t=11) | | | |

Consequences:

- Every 5L sector with t <= 7 takes the series-parallel lane. Those are the sectors where
  E1 batch B had 5 FAIL and 5 TIMEOUT.
- The first draft's generic classes are concentrated in t = 8..13 and carry insertions. The
  cascade sends them through Insertion and Carpet: 283 custom nodes remain at 5L, 21 at 4L.
- Rule order does not change the custom set: Forcer order, carpet-last and triangle-first each
  give 283 [M]. It changes the DAG size (5,773-6,250 nodes) and the runtime. Default: Forcer
  order; ablation at M5.

### 3.2 SeriesParallel lane: one-loop insertions and G-functions (`propagator/series_parallel/`)

SeriesParallel is the fast path of the Insertion lane (§3.2b): the whole class in one closed
step. Both share one bubble kernel (Th eqs. 15-17, the transverse decomposition below).

**What it computes.** For a sector whose glued graph is series-parallel, the integral at any
integer point `n` (positive lines, nonpositive numerators) is closed-form:

`I(n) = R_n(d) * Gamma-product(class)`

- The `Gamma` arguments are all of the form `a + j*d/2` with integer `a` and `j`.
- The lane returns `r_n(d) = R_n(d) / R_T(d)` times the class master
  `T_class = I(1,..,1; 0,..,0)` in the owner presentation.
- Coefficients are exact in `Q(d)`, with factored linear denominators.
- Powers that depend on `eps` occur only internally: a merged line has power `a + b - d/2`.

**Plan** (`SpPlan`, computed once per class from the representative graph):

1. The reduction sequence of the glued graph:
   - series merges: two lines with the same momentum; their powers add;
   - parallel merges: a bubble, which integrates one loop.
2. The loop basis adapted to the sequence:
   - one loop momentum `l_i` per parallel merge, flowing only through that bubble's two
     sub-networks;
   - this basis exists because series-parallel reduction is a nested 2-sum decomposition;
   - store the integer map from the presentation's basis, computed with exact integer linear
     algebra and checked to be unimodular (`det = ±1`, `q` fixed).

**Evaluation** at a key, innermost bubble first:

1. Expand the numerator monomial `prod_j S_j^(-n_j)` in the scalar products of the adapted basis
   (integer linear map; Symbolica polynomial).
2. For bubble `i`, the lines are `l` and `l - P`; `P` is the merged momentum, a combination of
   outer loop momenta and `q`. Reduce the `l`-dependence with the transverse decomposition:
   - `l.Y = (l.P)(P.Y)/P^2 + l.Y_perp`;
   - `l.P = (l^2 + P^2 - (l-P)^2)/2`, which shifts the two powers;
   - `<l_perp^{mu1}..l_perp^{mu2m}>` = `(l_perp^2)^m` times the sum over pairings of
     `prod g_perp`, divided by `(d-1)(d+1)...(d+2m-3)`; odd ranks vanish;
   - `l_perp^2 = l^2 - (l.P)^2/P^2`;
   - `g_perp(Y,Y') = Y.Y' - (Y.P)(Y'.P)/P^2`.
3. Each surviving term is a scalar bubble
   `G(alpha, beta) (P^2)^{d/2-alpha-beta}` with
   `G = Gamma(d/2-alpha) Gamma(d/2-beta) Gamma(alpha+beta-d/2) / (Gamma(alpha) Gamma(beta) Gamma(d-alpha-beta))`.
   - The bubble becomes one line of power `alpha + beta - d/2 + shifts`.
   - New numerators appear in the outer momenta.
4. Continue to the line parallel to `e0`. With `q^2 = 1` the result is a sum of
   (rational in `d`) x `Gamma`-monomial.
5. Normalise every `Gamma(a + j d/2)` to the class's canonical offset per slope `j`. The Pochhammer
   ratio is rational in `d`.
   - The remaining `Gamma` factors form a Gamma tag: an exponent vector over `Gamma(1 + m eps)`,
     `m != 0` (this replaces the first draft's assert "one `Gamma` type per class").
   - `1/Gamma(nonpositive integer)` = 0 (a massless tadpole).
   - A `Gamma(nonpositive integer)` in a numerator is a typed refusal: `Uncovered`, then the
     runtime safety net.

**Data.** `SpPlan` is small JSON. `sp-masters.json` gives, per class master, the exact
`Gamma`-monomial value for vakint, at any `eps` depth, with no numerical table.
The class master is `T_class = (tag, trivial node)`.

**Ordered integration.**

- A series-parallel key's Apply step is `OrderedStep { terms: {}, terminals: {T_class: r_n(d)},
  nonzero_conditions: [] }`.
- It is pure, cacheable and has no children. Its descent is trivial.

**Cost control.**

- Bubble kernels are memoised per (bubble, exponent pattern).
- An optional per-key term budget refuses with `Uncovered` (exact runtime fallback), never with a
  truncation.

**Tests (shared helpers).**

- An independent G-function closed form at 1L for `a, b <= 6` and rank <= 8: exact equality in
  `Q(d)`.
- Exact equality with SpIRed rules on every series-parallel class at 2L and 3L. The test generates
  both programs for the same sector.
- Routing invariance under random unimodular maps and `q -> -q`.

M5 measures the series-parallel one-step form against per-bubble insertion steps.

### 3.2b Insertion lane (`propagator/insertion.rs`)

Forcer's insertion rule [R Th eqs. 15-17 p.42; pf:1483-1496].

**Step.**

- Integrate the first one-loop bubble (two lines with the same endpoints; any eps on its lines,
  any integer powers, any numerator in the bubble momentum) with the shared bubble kernel of §3.2.
- Output: terms (relative tag, child eps-node key, `Q(d)` weight), after series merges.
- The edge map is not bijective: `D^-a D^-b -> D^-(a+b)`, and the eps units add.
- The child has `L - 1` loops.

**No exception branches** [D]. The only integer-argument Gammas are `1/Gamma(line power)`, which
is zero for an absent line. Every other Gamma has eps slope >= 1.

Counts [M]: 335 nodes at 4L, 4,838 at 5L. Insertion takes the custom count from 1,741 to 283
overall (5L integer 1,618 -> 129; 4L 123 -> 21).

### 3.2c Carpet lane (`propagator/carpet.rs`)

Forcer's carpet rule [R Th eq. 18 p.44 = FP eq. 26 p.35; pf:1413-1448].

- Applies when a line `r` joins the two external vertices and the rest is any `(L-1)`-loop
  two-point blob; eps is allowed.
- Step: the double sum over `sigma` and `j`, with tensor reduction of the `p_i.q` numerators.
- Output: the `(L-1)`-loop blob key with its integer powers unchanged, and a relative tag from
  `G(alpha + Sum a - L_sub d/2 - sigma, beta)`.
- No exception branches [D].

Counts [M]: 24 nodes at 4L, 281 at 5L. At 5L the carpet maps nodes onto the 4L integer library.

### 3.3 Triangle lane: Forcer's rule as a RustRed template (`propagator/triangle.rs`)

**The rule.** In the triangle presentation (§2.2), the IBP row `(d/dk) . k` of the owner family is
exactly Ruijl's eq. 25 [R thesis, `notes/literature/ThesisBenRuijl.txt:2470-2560`]:

`(d + N - n_a1 - n_a2 - 2 n_b) I = n_a1 A1+(B- - C1-) I + n_a2 A2+(B- - C2-) I`

where `N = sum over the k-linear ISPs i of (-n_i)`.

**eps on the apex lines.** `a1` and `a2` may carry `j1` and `j2` eps units.

- The pivot becomes `d + N - n_a1 - n_a2 - 2 n_b - (j1 + j2)(4 - d)/2`. Its `d` coefficient is
  `1 + (j1 + j2)/2 != 0`, so the rule stays exception-free.
- `b`, `c1` and `c2` must be integer lines. eps there makes the node custom (93 such blocked
  nodes at 5L [M]).

**Construction (no hand-written formula is trusted):**

1. `SourceSystem::from_family` on the triangle presentation, then pick the row with derivative `k`
   and contraction `k`.
2. Validate its shift-operator shape: the zero-shift coefficient is the pivot above, and exactly
   four shifted terms remain.
3. Build the `RuleCandidate` on the generic case of the top sector:
   `target = I(n)`, RHS = the four terms divided by the pivot.
4. Pass it through the existing `finish_rule` (`solver/sector.rs:591-620`).
   - The strict-descent check runs against the owner order.
   - `extract_exceptions` returns **no branch**: the pivot's `d` coefficient is 1, so the factor
     is "impossible" [R `exception.rs` coefficient-list rule].
5. On any shape mismatch, fall back to the custom lane for that class, and record why.

**Order.**

- The order is support-primary (empty `pre_support_degree_rows`).
- Its first degree row has weight 1 on `b`, `c1` and `c2`; the default rows follow, so every axis
  is covered.
- Every RHS term lowers `b`, `c1` or `c2` by one. Within the sector the measure therefore drops
  by 1, and at power 0 the support shrinks. The rule descends strictly, and never raises the rank.

**Program.** The owner program is one `SectorRule`, with no residuals, all-rank by construction.

- It uses the existing candidate payload format, so the Ordered engine, the step cache and the
  serialisation are unchanged.
- Each application costs 4 children with one linear `d` factor, which is cheap for
  `modular_polynomial`.
- The frontier is the `(a1, a2, b, c1, c2)` lattice per numerator. Ordered merging turns
  Forcer's "summed" recursion into dynamic programming.
- We keep this recursive form. Forcer's default is the summed form (`TRIANGLESMETHOD 3`,
  fa:479-640); M5 compares the two.

**Exactness note.** The intermediate factors `d + integer` vanish at `d = 4` for some indices.
- Because reduction stays in `Q(d)` and the final coefficient onto a fixed basis is unique, these
  intermediate spurious poles do not matter.
- Only the basis choice matters (§6).

**Measured fit.**

- All 42 triangle tops of E1 batch A take this lane. SpIRed solved them all, with a median of
  17 s; the template needs no generation.
- In batch B, the 5 triangle-lane failures (1 FAIL, 4 TIMEOUT) need no generation at all. They
  include the Petersen 8-line three-quadric locus `10011011101100000000`.

### 3.4 Custom lane: SpIRed, all-rank, with ledger

Custom owners are the custom DAG nodes only: 2 / 21 / 283 at 3L / 4L / 5L [M]. They include
eps-nodes with `power_shifts`.

Runs `family_candidates_from_family_controlled` on the induced presentation, with
`selected_sectors` = the owner masks of that top family. Settings:

- `SectorSolveOptions::default()`: unrestricted, `SearchFinite`, numerical depth 2;
- `RuleSelectionPolicy::BoundedPortfolio` with at least 3 alternative source orders and
  fall-through (§4.3);
- `LocusPolicy::Ledger`.

The p generation request rejects `max_numerator_rank` and `RetainRankFinite`; only L4 uses
them, per locus.

eps-axes:

- An eps-axis is always in the support, with excess `|n - 1|` (Th eqs. 57-59). It is implemented
  as two charts per eps-axis [E].
- The admission gates that refuse power shifts are lifted in M2 (T7): `discovery/mod.rs:69`,
  `install/source_port.rs:699`, the unit-scale shape of P0 (`family/unit_scale.rs`, whose scale law
  then counts the shifts) and the terminal services.
- The coefficient context stays `{d}`, so `Z_p(d)` replay still applies.

To measure in M3: in the support-primary SpIRed order, a tie-break that prefers right-hand sides
in rule-closed subsectors over custom subsectors.

Accelerator kept from the first draft: a triangle-first source order, which visits the
`(d/dk).k` rows of inadmissible triangles first. SpIRed already finds "d-benign" triangle-shaped
pivots there [M E2].

### 3.5 Diamond lane and reserved lanes

**Diamond** (`propagator/diamond.rs`), promoted from rung L3 to a primary lane [R Th eqs. 26-35
pp.47-52; DR eqs. 3-8; pf:1030-1061; arXiv:1504.08258]:

- Shape: `L + 1` paths T-`x_i`-B through 3-valent `x_i`, plus optional T-B spectator lines.
  `b_i` and `c_i` are integer lines; `a_i` may carry eps.
- Template: built from the `Sum (d/dk_i).k_i + Sum (d/dl_i).l_i` rows in the diamond-adapted
  presentation, validated like §3.3.
- Its pivot `E` has `d` coefficient `L + S + Sum j_a/2 + Sum j_s > 0`, so it is exception-free.
- Counts [M]: 4 nodes at 4L, including `no3`, which Forcer overrides with `SchemeZno3`; 42 at 5L.
- Carpet and diamond together remove about 60 5L integer custom classes (32 + 28 of the first
  draft's generic classes) [D].

Reserved lanes (interfaces frozen in M2, implemented only when a gate needs them):

| Lane | What | Gate |
|---|---|---|
| `Product` | A sector made of a chain of blocks between the external vertices: the product of block reductions after tensor reduction in `q` (the same transverse kernel as §3.2). It needs product terminals. | The 5L canary shows failures in low-`t` factorised sectors. |
| `HigherInsertion` | Integrate higher-loop insertions; 283 -> 279 custom nodes at 5L [M]. | 5L, decided from the canary. |
| `EpsTriangle` | Mincer's eps-central-line triangle (Mincer eqs. 15-16). | 5L, decided from the canary. |

`ShiftedPrimitive` is removed: it is dissolved into Insertion plus Custom on eps-nodes.

### 3.6 What the cascade does to the measured failures [M]

First-draft table (M1 lanes):

| Set | Series-parallel | Triangle | Generic |
|---|---|---|---|
| E1 batch A (64 tops) | 0 | 42 OK | 17 OK, 1 FAIL (Petersen top), 4 TIMEOUT (cf742) |
| E1 batch B (128 lower) | 37 OK, 6 FAIL, 7 TIMEOUT | 30 OK, 1 FAIL, 4 TIMEOUT | 28 OK, 5 FAIL, 10 TIMEOUT |
| 4L non-planar refusals | `11011001011000` (curve), `11110101000000` (intrinsic quadric) | - | `11010101011000` (integer-empty; L0) |

Cascade cross-reference:

- Batch B, 33 non-OK sectors: 26 are rule nodes (insertion 22, triangle 3, diamond 1) and 7 are
  custom (bc09 t = 11, 12, 13, 13; cf742 t = 10, 12; 94d07 t = 12).
- 18 of them have no custom descendant.
- Against the first draft's lanes (18), that is 8 more, all first-draft generic.
- The three 4L non-planar refusals are insertion nodes `d176`, `d172` and `d95`.
- The 22 first-draft generic tops are 3 diamond + 19 custom.

The heavy 5L tails (Petersen bc09, cf742) stay custom. They are 5L work: in-sector parallelism
and checkpointing (S14), and the reserved lanes.

## 4. LocusLedger and the discharge ladder

### 4.1 From refusal to ledger entry (`solver/sector.rs`, `solver/sector/ledger.rs`)

- **Scope.** Ledger entries arise only in custom nodes; the rule lanes are exception-free.
- **Option.** `SectorSolveOptions.locus_policy: LocusPolicy { Abort (default; vacuum unchanged),
  Ledger }`.
- **New admission method.** `SectorRule::admit_exceptional_cases_partial` returns
  `(children, discarded, deferred: Vec<DeferredBranch>)`. It iterates the rule's exception
  branches exactly like `admit_exceptional_cases_counted` (`sector.rs:95-118`). For one branch,
  `intersect_many` (atomic per branch) can fail in two ways:
  - **Deferrable failures:** `UnsupportedGeometry`, `RepeatedState`, `Budget{..}`,
    `Admission(UnsupportedNonlinear | Coordinate(UnsupportedGeometry))`. The whole branch becomes
    one `DeferredBranch`, carrying the `CaseIntersectionError`
    (`intersection/model.rs:115-125`: original and unresolved parent and conjunction).
  - **Hard failures** (`InvalidInput`, `NativeAlgebra`, `Resources`) still abort.
- **Queue behaviour.** In `solve_case_queue_inline` (`sector.rs:484-490`) and in `rule_trial`
  (`selection.rs:152-160`), under `Ledger`:
  - admitted children are enqueued as today;
  - the rule is pushed with its **complete** `exceptions`;
  - each deferred branch becomes `DeferredLocus { rule_ordinal, branch_ordinal, error }`, stored
    in a new field `SectorSolution.deferred`;
  - the `NonProgress` checks are unchanged.
- **Soundness.**
  - Dispatch tests every branch of every rule exactly at the concrete point
    (`evaluator.rs:233-296`), so the published rule never fires on the deferred locus.
  - `rule_covers` already treats unsupported geometry as "not covered" (`sector.rs:661-700`).
  - The points of the locus are therefore covered by a later rule, discharged below, or reported
    `Uncovered` at run time.
- **Whole-sector refusals.** These are `ResourceStop` (per-owner wall-time or RSS budget from
  the request), `CaseBudget` and `NonProgress`. Each yields one entry with the generic case and
  `OwnerProgram::Missing`; every key of that class then uses the runtime safety net.
- **Persistence.**
  - The candidate payload gains an optional `deferred` section in `candidate_bundle/save.rs`
    (bump the bundle schema constant).
  - The app writes `locus-ledger.json`, schema `rustred.p-locus-ledger.v1`:

```json
{"schema":"rustred.p-locus-ledger.v1","loops":4,"library":"<P_OWNER_LIBRARY_SEMANTICS>",
 "entries":[{"owner":"L4T07-1f..","presentation":3,"sector":"11010101011000","rule":17,"branch":2,
   "parent":{"fixed":[null,1,..],"equalities":["n3-n5+1"]},
   "conjunction":["2*n1^2-n1*n2-12*n2^2"],"unresolved_parent":{..},"unresolved_conjunction":[..],
   "cause":"unsupported-geometry",
   "discharge":{"rung":"L0","kind":"binary-form","certificate":{"a":2,"b":-1,"c":-12,"disc":97}}}]}
```

`Discharge` takes one of these values:

- `Pending`;
- `Empty{certificate}` (L0);
- `Split{rung, cases, overlay_rules}` (L1, or L0 bounded);
- `Rescued{how: PortfolioOrder(i) | ChartResolve | DeepSearch{radius}, overlay_rules}` (L2);
- `Template{EpsTriangle | S0Scan, overlay_rules}` (L3);
- `Bounded{r_cap, overlay_rules}` (L4);
- `Runtime` (residual: local Laporta at run time).

Shipping gate: no entry is `Pending`. The count of `Runtime` entries is reported, and it must
be 0 at 4L. The 4L ledger is likely empty, so this holds trivially there; the rung tests use the
5L locus files.

### 4.2 L0 and L1, inside the intersection engine (`solver/case/intersection/`)

They only replace places where the engine returns `UnsupportedGeometry` today. A vacuum program
that generated successfully therefore produces the same rules.

- **L0 `emptiness/`.** Dispatcher `classify(parent, equations, indices, sector, limits)` returns
  one of `Empty`, `Refine(affine branches)`, `Points(coordinate cases)` or `Unknown`.
  - Cheap tests run right after the definite-quadratic pass (`engine.rs:345-370`):
    - `binary_form.rs`: an equation is a homogeneous quadratic in two free variables with a
      non-square discriminant. Then `x = y = 0` is the only rational zero, so the branch is
      refined to `{x = 0, y = 0}` and admission decides. Example: the 4L non-planar
      `2x^2 - xy - 12y^2`, discriminant 97, plus a sign violation, is empty.
    - `orthant.rs`: substitute `n = 1 + y` on active axes and `n = -y` on inactive axes, with
      `y >= 0`. If every coefficient is strictly of one sign and the constant is nonzero, there
      is no root (a Pólya-type certificate).
    - `bounded.rs`: if the quadratic part is definite on the chart, return the integer points of
      the bounding box as numerical cases, at most `max_points` (default 10^5); otherwise
      `Unknown`.
  - The last resort, before `return Err(UnsupportedGeometry)` (`engine.rs:477`, `:486`), is
    `congruence.rs`: no common root modulo `m` for some `m` in {2,3,4,5,7,8,9,11,13,16}, over
    the chart's free variables (`m^k <= 10^6`). The certificate is `m`.
- **L1 `affine_bilinear.rs`.** This is a third refiner in the pass at `engine.rs:402`, between
  `bilinear_integer` and `linear_resultant`.
  - Authenticate `Q = F*G + c`: factor the degree-2 part into two integer linear forms `f*g`
    (Symbolica); solve for the integer offsets `f0` and `g0` from the linear part; require
    `c != 0`.
  - Then `F*G = -c`: for every signed divisor `delta` (`integer_divisors::visit`), one affine
    branch `{F = delta, G = -c/delta}`.
  - It generalises `bilinear_integer.rs:1-131` (single variables only).
  - Example: the Petersen top locus, `Q = w(n1 - n2 - 4w + 6) - 2` with
    `w = n12 - n10 - n9 + n8`, gives 4 cases.
- **New counters** in `CaseIntersectionStats`: `emptiness_proofs`, `congruence_tests`,
  `bounded_points`, `affine_bilinear_children`.

### 4.3 L2 (`solver/sector/selection.rs`, `solver/sector.rs`)

1. **Portfolio fall-through.**
   - A baseline optional refusal tries the alternatives instead of returning. This is the study
     patch in `WS/study/patches/closure-study-admission-arity-portfolio.diff`, which replaces
     `selection.rs:39-45`.
   - The p strategy ships at least 3 alternative source orders, or iterates. Today the portfolio
     admits only 1-2 alternatives.
2. **Chart re-solve.**
   - Applies to a deferred branch whose `unresolved_parent` is strictly contained in the current
     case.
   - Enqueue that affine chart as a normal pending case, tagged
     `CaseOrigin::LocusChart{ledger}`, with a per-case search override.
   - The parent rule does not apply on the locus, so dispatch falls through to the chart's rule,
     which is pushed later.
3. **Deeper search.** On the chart, use a larger seed radius. This is the `probe_owner_case`
   path that found the vacuum frontier rule at depth 2 in 0.34 s.

If the chart refuses on the same conjunction, the locus is intrinsic. It goes to L3 and L4.

Measured: L2 rescues the 4L curve and empty cases (301 and 192 rules, 10-13 s). Both are now
series-parallel or L0 anyway.

### 4.4 L3, L4 and the runtime contract

- **L3: locus-chart templates**, on the locus chart: the eps-triangle and S0-scan rules. Its
  output is overlay rules. Triangle and Diamond are lanes now (§3.3, §3.5), not rungs.
- **L4: bounded rank split**, on the locus only.
  - `intersect_many_with_max_numerator_rank(branch, R_cap)`, with `R_cap = 17` [E].
  - The rank slices are solved by `solve_case_queue` on fragment cases. The existing API accepts
    initial cases (`sector.rs:335-350`).
  - The output is overlay rules, appended to the owner through the existing overlay mechanism
    (`routed_campaign/prepare/overlays.rs:69`).
  - Points above `R_cap` go to the runtime.
- **Runtime.** `PUncoveredPolicy { Reject, LocalLaporta{max_depth, max_seconds}, Preserve }`.
  - The default is `LocalLaporta` for vakint and `Reject` for validation runs.
  - On `Uncovered` (`evaluator.rs:209-211`), call `bridge::solve_laporta_for::<N>`
    (`bridge/mod.rs:225`, `:411`) on the owner presentation:
    - targets = the key;
    - preferred = the owner terminals;
    - `until_stable`.
  - Accept the result only if:
    - every RHS key is strictly below the key in the owner order;
    - every same-sector residual is a declared terminal.

    Otherwise fail closed.
  - The accepted rule is a fully fixed overlay record, keyed by (owner class, key), with
    provenance `runtime-laporta`. It is cached in the shared cache (§7.4).

## 5. Route census, zero proofs, descent and the runtime contract

### 5.1 Routes

`SignedAffineLoopMap { loop_linear: L x L i64 (det ±1), loop_external: [i64; L], external_sign: ±1 }`
is the census record. It becomes the existing `MomentumMap { loop_linear, loop_external,
external_linear }` (`sector/symmetry/model.rs:106-133`).

The chain at load time:

- `verify(source, target, map)` checks the loop and external counts, the Gram congruence and the
  denominator bijection.
- `integral_transport::compile` gives `Prepared` transport: unit-coefficient bijection of
  positive lines, and multi-affine expansion of numerators.

None of these changes for externals: `verify` already accepts two different families
[R `verify/mod.rs:20-80`].

The app-level gates at `routed_campaign/prepare.rs:260` and `:360` stay vacuum-only. The p
library has its own loader (§7.1).

### 5.2 Route table contents

- every nonzero labelled sector of every top family (4L: 4,519; 5L: 180,644);
- 3 routes per triangle owner;
- none for series-parallel owners;
- one edge map per DAG rule edge (insertion, carpet, triangle, diamond): a verified `(A, b, c)`
  plus a merge list (the insertion edge map is not bijective, §3.2b).

Children of custom rules are labelled sectors of their own top family, so they are already
covered.

### 5.3 Zero proofs

There is one per zero labelled sector: the analyzer decision, replayed at load with the existing
`ZeroTerminalProof::LeePomeranskyRankDeficiency`. Zero subsectors of triangle presentations are
proved at generation time.

### 5.4 Caller families (entry routes)

`PLibrary::register_family(Arc<IntegralFamily>) -> PresentationId` accepts any vakint family:

- `E = 1`, Gram `[[±1]]`, massless;
- any loop routing, any `q` orientation.

The first key of a new (family, mask) pair:

1. computes the class key;
2. looks up the owner;
3. derives and verifies the map;
4. caches the route by (family fingerprint, mask).

This makes "any loop-momentum routing" correct by construction. vakint does not need to
reproduce RustRed's canonical presentations.

### 5.5 Descent certificate (`propagator/descent.rs`, checked at library load)

Load-time checks:

1. every owner order is support-primary (`CompiledOrder::is_support_primary`);
2. every rule is checked per RHS term by the existing strict-descent comparison, and stores a
   `DescentWitness { SameSectorStrict | Subsector }`;
3. every route and every DAG edge map is verified; a route maps `t` lines to an owner with the
   same `t`, and its outputs are Apply keys, never routed again;
4. series-parallel, insertion and carpet steps emit terminals, or keys with smaller `t` (and
   `L - 1` loops);
5. overlays satisfy check 2;
6. eps-axes are forced active in every support mask. The `power > 0` masks are fixed at
   `sector/ordering.rs:26`, routed `ordered.rs:111-112` and `layered.rs:103-110`.

`t` counts eps-lines too; they are never pinched.

Then the lexicographic measure (`t`, Route > Apply, owner order) strictly decreases along every
step. That is well-founded: the supports are finite, and each owner order is a validated
descriptor.

This closes the mixed-order caveat of `docs/runtime_integral_order.md` ("well-foundedness of
one descriptor is not a route-global theorem"), because within one `t` a key only ever meets one
owner's order.

## 6. Terminals and the master basis

- **Terminals.** There are two kinds, each with a Gamma tag:
  - (tag, trivial node): series-parallel class masters `T_class` and the ends of insertion and
    carpet chains, with closed values (`sp-masters.json`);
  - (tag, custom-node residual): finite residuals of custom owners.

  Triangle and diamond owners have none.
- **Reference basis.** Up to 4L it is Forcer's masters (Masters.prc; the authors gave
  permission). At 5L it is GGPPSS's 281 masters.
  - 4L: Forcer's 16 masters in Forcer's own convention, 7 of them with eps-lines (FP App. C),
    plus tags.
  - `p-basis` maps residuals within the same node.
  - A translation table maps tags to and from `GschemeConstants` and `G10..G40`.
  - The ALARM sentinels of Masters.prc feed the vakint Oep guard.
  - 5L: one recorded de-insertion witness per (tag, node) gives an integer 5L integral for Kira.
    If the system is not square, cross-tag relations are needed.
  - The reduction coefficients onto a fixed basis are unique.
  - So our exact `Q(d)` coefficients onto Forcer's basis equal Forcer's own. Whatever master
    depth suffices for Forcer suffices for us.
  - At 5L, GGPPSS chose their basis to remove bad denominators.
  - We therefore adopt the reference basis instead of selecting our own.
- **When to change basis.** Switch to an eps-finite basis (Chetyrkin et al. hep-ph/0601165) or a
  d-factorising basis (arXiv:2002.08042, 2002.08173) only for a sector where the maximum
  `(d-4)`-valuation of the coefficients on the benchmark grid exceeds the depth of the master
  tables. This is measured in M4, and the vakint Oep guard fails closed in the meantime.
- **Procedure (`rustred p-basis`), one-off per loop count:**
  1. Route each reference master (a graph plus indices) to its owner, and reduce it with the
     library: `F = A T + (series-parallel part)`, exact.
  2. If `A` is square and invertible: `T = A^-1 F`. The rows have provenance `rustred`. Check
     them at two fresh `d` values modulo `2^61-1` by re-reduction.
  3. If there are more terminals than masters (cross-sector or hidden relations), look for the
     missing relations in this order:
     - first RustRed: lift the vacuum gate in `terminal_relations/collection/prepare.rs:43` and
       use bridge Laporta with preferred masters (`bridge/basis.rs`) in closing sector boxes plus
       growth rounds. This is the lesson of the vacuum 640 -> 131 reduction: boxes, not
       radius-2 seeds.
     - then the clean-room Kira stopgap, as in `5l-pilot/rustred-dev/rollout/kira-stopgap`:
       - job files: owner presentation as `integralfamilies`, targets = terminals, preferred =
         reference masters, Pareto boxes `(L+dots+1, rank+1, dots+1)`;
       - Kira 3.1 with Fermat runs as an external process; Kira with FireFly is rebuilt for 5L;
       - the results are converted with the Symbolica basis-change tool into rows with
         provenance `kira-stopgap`;
       - verification: RustRed IBP and transport identities re-substituted at two fresh `d`
         modulo `2^61-1`;
       - "RustRed wins" when both provenances exist.
- **Shipping.**
  - `layer/layer.json`, schema `rustred.saved-application-layer.v2` (`layer.rs:1-140`):
    - rows tagged `rustred`, `kira-stopgap` or `identity` (series-parallel masters);
    - masters with `class` and `origin` fields.
  - The base is bound to the library version constant and the terminal inventory. A structural
    check at load verifies that every source is a terminal and every coefficient is exact.
  - Expected size: 1L-3L trivial (3L: 2 custom owners); 4L 16 masters plus tags.

## 7. Application and performance

### 7.1 Library and adapter (`crates/rustred-core/src/propagator/`)

```rust
pub struct PresentationId(pub u32);
pub struct OwnerClassId(pub u64);
pub enum OwnerLane {
    Zero, SeriesParallel, Insertion, Carpet, Triangle, Diamond, Custom,
    Product, // reserved
}
pub struct GammaTag(/* sparse small-integer exponents over Gamma(1 + m eps), m != 0 */);
pub enum OwnerProgram<const N: usize> {
    SeriesParallel(Arc<SpPlan>),
    Insertion(Arc<InsertionPlan>), Carpet(Arc<CarpetPlan>),
    Rules(Arc<CandidateOwnerPrograms<N>>), // triangle or diamond (1 rule) or custom (+ overlays)
    Missing { ledger: u32 },               // whole-sector ResourceStop: runtime only
}
pub struct POwner<const N: usize> {
    pub class: OwnerClassId, pub lane: OwnerLane, pub presentation: PresentationId,
    pub program: OwnerProgram<N>, pub terminals: Vec<IntegralKey>, pub order: CompiledOrder,
}
pub struct PLibrary<const N: usize> { /* presentations, owners (lazy), routes, zeros, layer */ }
impl<const N: usize> PLibrary<N> {
    pub fn load(dir: &Path, options: PLibraryOptions) -> Result<Self, PError>;
    pub fn register_family(&self, family: Arc<IntegralFamily>) -> Result<PresentationId, PError>;
    pub fn reduce(&self, request: PReductionRequest) -> Result<PReduction, PError>;
}
pub struct PReductionRequest {
    pub inputs: Vec<(PresentationId, IntegralKey, Coefficient)>,
    pub arithmetic: OrderedWeightArithmetic,   // modular_polynomial by default
    pub scheduler: OrderedSchedulerOptions,    // threads, window size
    pub cache: Option<PCacheHandle>,
    pub uncovered: PUncoveredPolicy,
    pub onto_masters: bool,                    // compose the layer
}
```

Module layout:

- `class.rs`, `census.rs`, `presentation.rs`, `lane.rs`;
- `series_parallel/{plan,gamma,bubble,eval}.rs` (the bubble kernel is shared with `insertion.rs`);
- `insertion.rs`, `carpet.rs`, `triangle.rs`, `diamond.rs`, `descent.rs`, `tag.rs`;
- `application.rs`: the `OrderedKernel`, `OrderedStepper` and `ConcurrentOrderedKernel`
  implementation;
- `cache.rs`, `overlay.rs`.

The app crate adds `application/propagator/` with the CLI commands `p-census`, `p-generate`,
`p-discharge`, `p-basis`, `p-validate`, `p-canary`, `p-package` and `p-cache`.

The admission `CandidateFamilyAdmission::UnitScalePropagator` (`family/unit_scale.rs`) and the
homogeneity proof `CommonMassHomogeneityProof::UniformExternalScale` come from P0 (rustred-p0,
`docs/p_integrals.md`).

**Kernel.**

- `Key = PKey { presentation, tag: GammaTag, stage: Route | Apply, powers }`. Alternatively the
  tag is a batch dimension of the weight. eps-node keys are padded to `N`.
- The coefficient context stays `{d}`; `modular_polynomial` is unchanged.
- Phases come from the DAG nodes: `t` descending, then Route and Apply per node. Children always
  land in the same or a later phase, as the engine requires (`ordered_reduction/mod.rs:1-120`).
  Loop counts mix within one `t` level: Forcer's level 8 holds 13 3L and 87 4L topologies [M].
- Step, tail and result cache identities stay tag-free; their values carry relative tags. This
  corrects the claim of `WS/notes/forcer-tricks-ordering.md` that tails and results need the tag
  in their identity [D].
- Apply steps by lane:
  - series-parallel, insertion and carpet: the closed steps;
  - rule-based owners: the existing `CandidateEvaluator` dispatch;
  - `Uncovered`: the overlay, then local Laporta.
- The engine, the windows, the tail and the step cache are reused as they are.

**Lazy loading.** Owners are prepared on first use (the existing `owner_preparation:
on_first_use`). At 4L most owners are templates or plans, so cold start is dominated by the
21 custom owners [E].

### 7.2 `Z_p(d)` replay

- `modular_polynomial` needs a coefficient context with at most one variable
  (`arithmetic.rs:19`).
- The p library guarantees context `{d}`:
  - `q^2 = 1` is numeric;
  - homogeneity `(q^2)^{L d/2 - sum n}` is restored by vakint;
  - power shifts (eps-lines) are in `d` only;
  - Gamma tags stay outside the weights.
- At concrete indices every step coefficient lies in `Q(d)`:
  - triangle: one linear factor;
  - series-parallel: products of linear Pochhammer factors;
  - SpIRed: specialised rules;
  - routes: rational constants.
- The step cache stores the factored denominators, so the factorised partial-fraction lanes stay
  narrow.
- The 5L vacuum rung replays 66.5M ops over about 37 primes in about 10 min [M]. The p case reuses
  this path unchanged.

### 7.3 Multi-core evaluation of one input (16+ threads)

1. **Concurrent windows** (`ordered_reduction/parallel.rs`). Steps are pure functions of the key,
   so they are computed by work stealing inside a window, with bit-identical results for any
   thread count. All p steps are pure, including series-parallel steps and route expansions.
2. **Replay lanes.** `ModularCommon.threads` replays primes in parallel; images are independent
   per prime.
3. **Route expansion.** A numerator of rank `R` over `m` ISPs can expand to many terms; at 5L,
   `C(R+m, m)` monomials per key. `Prepared` transport is chunked over the multinomial terms
   inside one step. This is new, and it is the one place where a single step is parallelised.
4. **Request batching.** vakint submits all contributions of one input as one linear combination,
   so the frontier is shared.

The tail table closes single-terminal suffixes in one product per entry key (`tail/mod.rs:1-60`).

### 7.4 Shareable cache, keyed by owner class and route

- **Binding.** A new `OrderedStepCacheAdapter::PropagatorLibrary`. The binding's
  `family_fingerprint` field carries the library identity string
  `p-library:L<loops>:<P_OWNER_LIBRARY_SEMANTICS>:<P_CENSUS_VERSION>`.
  - Version constants only: no program digest (user rule).
  - The vacuum stamps are untouched.
- **Record identity.**
  - Apply records: (Apply, owner class `u64` in the shard `class` field, powers in the owner
    presentation).
  - Route records: (Route, source presentation class, powers).
    - For library presentations the class is the presentation's `u64` id.
    - For caller families it is the low 64 bits of the family fingerprint (an identity, as in
      today's binding).
  - Shard = (stage, support, class) (`step_cache/mod.rs:273-288`).
  - The codec already takes a 64-bit class and `i8` powers (`codec.rs:211-241`), so arity 20 and
    rank up to 127 fit.
- **Why records are shared.** Everything below the entry route is expressed in owner-class
  coordinates. One class's records therefore serve every family, diagram and user that reaches
  that class.
- **Layers.** `<root>/<library-version>/` contains:
  - `steps/` (Ordered step shards);
  - `tails/` (tail tables, single terminal, shareable);
  - `results/`: completed `(class, key)` reductions with guards, using the existing
    `ApplicationCacheRecord` and running-cache packs;
  - `overlays/`: runtime Laporta rules.
- **Concurrency.**
  - Read-only immutable bases (packs).
  - A per-process delta directory `delta-<host>-<pid>-<utc>/`.
  - `rustred p-cache merge` folds the deltas into a new base generation (atomic rename).
  - Records are pure functions of their identity, so a merge is a union. Duplicate identities
    must be byte-identical, otherwise the delta is rejected.
- **Shipping.** vakint-external-data may carry a warm base for the benchmark grid. vakint exposes
  the cache root (for example `VAKINT_P_CACHE_DIR`); the vakint-design agent decides its name.

### 7.5 Targets (plan §3.8)

- 4L single-threaded: on par with Forcer for each cell.
- 16 threads: better than Forcer.
- Cold start dominated by artifact loading.
- Measured on the benchmark grid: L = 1..4, rank {0,2,4,6,8}, dots {0,1,2}, cold and warm, with
  1 and 16 threads.

## 8. Data formats and version constants

```text
p-library-L<k>/
  library.json        rustred.p-owner-library.v1: versions, presentations[], owners[{class, lane,
                      presentation, payload, terminals, order}], counts
  families/*.rrbin    native family records (top families, adapted and eps-node presentations)
  census.rrbin        routes (SignedAffineLoopMap) + zero proofs; census.json summary
  owners/*.rrbin      candidate payloads (custom, triangle, diamond), gzip 32 MiB chunks as today
  overlays/*.rrbin    L2-L4 discharge rules
  sp/plans.json       SpPlan per series-parallel class;  sp/masters.json  Gamma-monomial values
  locus-ledger.json   rustred.p-locus-ledger.v1
  layer/layer.json    rustred.saved-application-layer.v2 (+ layer.rrbin)
  topology/p-topologies.json   DAG nodes, actions, roles, edges, edge maps (§2.4)
  topology/edges.rrbin         verified edge maps
```

Version constants (hand-bumped, in `propagator/mod.rs`):

- `P_CLASS_KEY_VERSION`;
- `P_CENSUS_VERSION`;
- `P_OWNER_LIBRARY_SEMANTICS`: bumped when any lane can return a different step for the same
  identity;
- `P_SP_LANE_SEMANTICS`;
- `P_TRIANGLE_TEMPLATE_VERSION`;
- `P_LOCUS_LEDGER_SCHEMA`;
- `P_TOPOLOGY_VERSION`, `P_GAMMA_TAG_VERSION`, `P_INSERTION_KERNEL_VERSION`,
  `P_CARPET_KERNEL_VERSION` and `P_DIAMOND_TEMPLATE_VERSION`.

`ORDERED_STEP_SEMANTICS` is not bumped: existing adapters do not change.

Packaging:

- 1L-3L are embedded in vakint. The 3L built-in library has 43 DAG nodes and 2 SpIRed owners
  (NO and the 2L kite with an eps-line).
- 4L goes to vakint-external-data `p/4L/`.

5L readiness:

- no `<= 16` assumption in the new code (`N` = compiled capacity: 16 up to 4L, 20 at 5L);
- `PresentationId` is `u32` and `OwnerClassId` is `u64`;
- owner masks up to 64 bits;
- payloads loaded lazily per owner.

## 9. Validation

| L | Exact (in `Q(d)`) | Against Forcer (FORM 5 backend; symbolic eps coefficients plus numerics with constants substituted) |
|---|---|---|
| 1 | Series-parallel lane equals the closed G-function form, `a, b <= 6`, rank <= 8 | grid |
| 2 | Series-parallel lane equals SpIRed rules on all 7 series-parallel sectors; kite triangle template equals SpIRed kite rules | grid; kite `6 zeta3`, eps^0..eps^4 |
| 3 | Lane cross-check on every series-parallel and triangle class (both programs generated in the test); glue-and-cut `LA(1..1) = NO(1..1) = 20 zeta5` | LA/BE/NO tops plus every class at 2 points |
| 4 | Ledger: every entry discharged and checked at points aimed at its locus; routing invariance on 100 inputs (unimodular maps, `q -> -q`); `cross_check_exact` on a modular subset | stratified grid (11 tops x rank x dots), 2 random points per class, physics row (gluon self-energy, ladder type) |

Additional 4L acceptance:

- `p-validate` traces 10^4 random in-scope points (support only, no coefficients) with
  `PUncoveredPolicy::Reject`. Required: 0 `Uncovered`, 0 `MissingOwner`, 0 descent failures.
- A Forcer-frozen fixture set is kept as data for `just test_vakint`.

Cascade checks (added with the Forcer cascade):

- the 4L DAG equals Forcer's: 438 nodes, identical actions, 21 custom (eps-on-q convention);
- exact symbolic comparison with Forcer output, after mapping tags to G-constants;
- the insertion kernel against Forcer's `reduceG` identities;
- the carpet against Th eq. 18 on 2L and 3L closed forms;
- the diamond template against SpIRed on Forcer's 4 diamond nodes.

Test placement:

- RustRed unit tests per rung use `WS/study/pint-closure-study/scratch/runs/e2/locus_*.json`.
- Slow cases go to the nextest default-filter, never to `slow::` modules.

**5L canary** (`rustred p-canary`; numbers only, no 5L code; at most 16 cores, guarded, about
30-50 core-h; run at every milestone):

- Suite: the 64 tops, the 128 batch-B sectors, and the cf742 e13 top (`runs/e1/jobsA.txt`,
  `jobsB.txt`, `runs/e4`).
- For each sector, report the lane. For custom sectors also report outcome, time, RSS, rules,
  residuals and ledger entries by cause and rung.
- Also report the pass-A census numbers (§2.3) and the eps-owner generation sample.
- Compare with the E1 baseline (resultsA/B.jsonl) and with the §3.6 table.
- Warning signs:
  - lane drift;
  - any regression of an E1 OK sector;
  - a `Pending` entry;
  - more than 5 residuals for one owner.

## 10. Work breakdown

Sizes: S <= 2 days, M 3-5 days, L 1-2 weeks of agent time.

The dependency on P0 (rustred-p0: admission of `E = 1`, homogeneity, row counts) holds for every
task.

| # | Task | Size | Depends | Output, check |
|---|---|---|---|---|
| **M2** | | | | |
| T1 | Class key: External colour, `q` rows, glued `e0`, eps-unit colours; `OwnerClassId` | M | P0 | Unit: kite, LA/BE/NO classes; Whitney-twist pair maps |
| T1b | `rustred-topology` crate (§2.4) and the Forcer DAG golden test | M | T1 | 4L DAG = Forcer's 438 nodes, 21 custom; 3L 43; 2L 7 |
| T2 | `p-census`: eager roots, DAG pass A, family input, zero proofs, maps, verify; completeness | M | T1b | 1L-4L counts = graph census (8/175/4,519); every DAG edge target a node or a zero; 0 MissingOwner |
| T3 | Lane planner plus presentations (induced, insertion, carpet, triangle, diamond, eps-node); `lane.rs` | M | T2 | Lane counts equal the cascade census (§0 item 4) |
| T4 | Bubble kernel shared by SeriesParallel and Insertion, with Gamma tags: plan, transverse bubble, step | L | T3 | §9 rows L1-L3 exact; kernel vs Forcer's `reduceG` identities |
| T4b | Carpet kernel | M | T4 | Carpet vs Th eq. 18 on 2L/3L closed forms |
| T5 | Triangle template with eps on the apex lines: row extraction, shape check, order, `finish_rule` | S | T3 | Kite and 3L tops exact against SpIRed |
| T6 | `PLibrary` and the Ordered adapter (`PKey` with tag, padded arity, eps support masks), routes and edge maps, lazy owners, overlays hook, step-cache binding | L | T2-T5 | 1L-3L reductions through the Ordered engine (exact and `modular_polynomial`); vacuum tests unchanged |
| T7 | Custom lane through `family_candidates_from_family_controlled`; `power_shifts` admission and eps-axis order; `LocusPolicy` plumbing (no rungs yet) | M-L | T3 | First eps owner = the 2L kite; 3L custom owners generated all-rank |
| T8 | `p-package`; built-in 1L-3L bytes; `PReductionRequest` API used by vakint | M | T6, T7 | vakint integration agent green |
| T9 | 1L-3L basis = Forcer's masters (`p-basis`, RustRed provenance); `sp-masters.json`; tag translation table | S | T8 | Forcer battery 1L-3L |
| T10 | Benchmark v1 numbers; canary run #1 (lanes only plus custom re-runs) | S | T8 | Table |
| **M3** | | | | |
| T5b | Diamond template | M | T5 | Template vs SpIRed on Forcer's 4 diamond nodes |
| T11 | L0 `emptiness/` (binary form, orthant, bounded, congruence) | M | - | 4L `11010101011000` proved empty; vacuum suite unchanged |
| T12 | L1 `affine_bilinear.rs` | S | - | Petersen top locus gives 4 cases; synthetic tests |
| T13 | L2 fall-through, at least 3 alternatives, chart re-solve, deep search | M | T7 | E3 cases rescued |
| T14 | `p-discharge`: ledger driver, L3 locus-chart templates, L4 bounded split, overlays, gates | M | T11-T13 | 0 `Pending` at 4L (the 4L ledger may be empty; rung tests use the 5L locus files) |
| T15 | 4L census and generation: the 21 custom nodes | S | T6, T14, T5b | Library L4 |
| T16 | Runtime safety net (`LocalLaporta` overlay, cache) | M | T6 | Forced `Uncovered` test is exact |
| T17 | 4L basis: the within-node map onto Forcer's 16 masters (plus tags) | S-M | T15 | Layer v2; fresh-`d` checks |
| T18 | 4L validation (§9) | M | T17 | All rows green |
| T19 | Performance: `modular_polynomial` default, 16 threads, chunked route expansion, cache (base and delta, merge) | M | T6 | Benchmark targets §7.5 |
| T20 | Canary run #2 with the eps-owner cost and the rule-order ablation; gate decisions for Product, HigherInsertion, EpsTriangle | S | T15 | Table plus a recommendation |

Risks and mitigations:

1. **T6 is the largest refactor.** Keep it additive: a new module and a new adapter. The vacuum
   paths and stamps stay byte-identical, which is tested.
2. **Bubble-kernel cost at rank 8.** Memoise bubble kernels; key the cache by class. The term
   budget refuses with `Uncovered` (exact fallback).
3. **Triangle and diamond shape mismatches** (sign conventions, ISP choice). The templates are
   validated by RustRed's own rows, and any mismatch falls back to the custom lane.
4. **4L basis not square.** Bridge Laporta in boxes first; the Kira pipeline is ready.
5. **Census key instability across Symbolica updates.** `P_CLASS_KEY_VERSION` and
   `P_TOPOLOGY_VERSION`, plus golden tests on the 3L classes and the 4L Forcer DAG.
6. **5L custom tails** (bc09, cf742, 94d07). Out of M2/M3. The canary gates S14 (in-sector
   parallelism and checkpointing) and the reserved lanes.

Open points to settle in M2/M3 (measured, not guessed):

- The exact marked-matroid class counts per lane, recomputed by RustRed against the cascade
  census of §0 item 4.
- The terminal count of the 21 4L custom owners against Forcer's 16 masters.
- Whether the 4L reference basis ever exceeds the depth of the Masters.prc tables (Oep guard
  telemetry).
- The series-parallel one-step form against per-bubble insertion, and the recursive against the
  summed triangle (M5).

## References

- Chetyrkin, Tkachov, Nucl. Phys. B192 (1981) 159: IBP and the triangle rule.
- Chetyrkin, Tkachov 1981: the carpet rule. Tkachov 1983: the summed triangle.
- Larin, Tkachov, Vermaseren, the Mincer manual NIKHEF-H/91-18: carpet origin; eps-central-line
  triangle, eqs. 15-16.
- Ruijl, Ueda, Vermaseren, Forcer: arXiv:1704.06650. Ruijl's thesis §3.4 (triangle eq. 25,
  diamond). Diamond rule: Ruijl, Ueda, Vermaseren, PLB 746 (2015) 347, arXiv:1504.08258.
- Forcer source: benruijl/forcer `68a30bf` (`pathforcer.py`, `forcer-aux.h`, `Masters.prc`).
- Grozin, Lectures on QED and QCD, hep-ph/0508242: G-functions and massless one-loop tensor
  integrals.
- Lee, Smirnov, Smirnov: arXiv:1108.0732 (4L masters). GGPPSS: arXiv:2104.08272 (5L masters).
- Chetyrkin, Faisst, Sturm, Tentyukov: hep-ph/0601165 (eps-finite basis). Smirnov, Smirnov:
  arXiv:2002.08042. Usovitsch: arXiv:2002.08173.
