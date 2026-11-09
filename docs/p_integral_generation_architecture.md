# Closed p-integral reduction programs: generation architecture

Status: design (milestone M1, 2026-11-03), amended for M2 on 2026-11-05 (below). It is being
implemented in M2 (1L-3L) and M3 (4L) and reused unchanged at 5L.

Amended 2026-11-03: Forcer's rule cascade is adopted (plan §3.5; DECISIONS ~11:00, ~11:20, ~11:50,
~12:50; evidence in `WS/SCRATCHPAD/forcer-tricks/` and `WS/notes/forcer-tricks-*.md`):

- the lanes become Zero > SeriesParallel > Insertion > Carpet > Triangle > Diamond > Custom;
- eps-lines (Forcer's crossed lines) and Gamma tags are first-class;
- the graph engine is a new graphica-based crate `rustred-topology`, with a cached, eps-sensitive
  topology-analysis service (§2.4).

Amended 2026-11-05 for M2 (`WS/M2_PLAN.md` §1 decisions D1-D12, §3 normative architecture, §4
frozen interfaces, §12 departures; design reports in `WS/SCRATCHPAD/m2-design/`). Where this
document and the M2 plan differ, the plan wins. The normalisation conventions C1-C4 are stated in
`docs/p_integrals.md` ("p-library conventions"). Each amendment is marked "M2 amendment" in place:

1. The production DAG is Forcer's tree (`ChildMode::ForcerTree`); Demand is a diagnostic. 438 vs
   427 at 4L is child semantics, not eps on `q` (§0 item 4, §2.3, §2.4, §9); 335 is the ForcerTree
   insertion count (§3.2b). (`WS/M2_PLAN.md:46`, departure 1 at `:1364`;
   `WS/SCRATCHPAD/m2-design/cascade.md:13-26`.)
2. Eps axes are two-sided inside SpIRed, opt-in, with the injective excess `e(n)` and a hard
   false-knock-out guard; the p-path shift gates are listed with their current lines (§2.1, §3.4,
   §5.1, §5.5). (`WS/M2_PLAN.md:47`, `:60`, §3.4 at `:188`, departures 2 and 20 at `:1368`,
   `:1408`; `WS/SCRATCHPAD/m2-design/rr-code.md:81-146`.)
3. The carpet-adapted contract routes `q` only through `r` (§2.2, §3.2c). (`WS/M2_PLAN.md` §4
   contract text; `WS/SCRATCHPAD/m2-design/critique-phys.md:24-40`.)
4. SeriesParallel is the Insertion step along the chain; `SpPlan` is the test oracle (§3.2).
   (`WS/M2_PLAN.md:50`, departure 4 at `:1376`.)
5. Triangle is a template `PLaneStep` built from RustRed's own IBP row (§3.3).
   (`WS/M2_PLAN.md:49`, departure 3 at `:1374`; `crates/rustred-core/src/solver/source.rs:90`.)
6. Every DAG node gets a lane-adapted node presentation; the owner of a class is its
   min-GraphKey node; `GraphKey` and `OwnerClassKey` are separate identities (§1, §2.2).
   (`WS/M2_PLAN.md:53`, departure 7 at `:1380`; `WS/SCRATCHPAD/m2-design/critique-phys.md:62-88`.)
7. `PLibrary` is non-generic; Custom owners run as `CustomStep<M>` (§7.1). (`WS/M2_PLAN.md:58`,
   departure 19 at `:1405`; `crates/rustred-core/build.rs:98`.)
8. One library program per release scope (built-in: 1L-3L), with the identity string
   `"p-library:scope=<a>-<b>:" + p_versions()` (§1, §7.4, §8). (`WS/M2_PLAN.md:55`, departure 18
   at `:1401`.)
9. The Gamma tag is part of step and tail identities; there is no `results/` layer (§0 item 10,
   §7.1, §7.4). (`WS/M2_PLAN.md:51`, departure 5 at `:1377`; gammaloop
   `crates/vakint/src/rustred_evaluation/cache.rs:41-44`.)
10. `register_family`, the `LocusPolicy` plumbing, the Diamond lane and run-time use of the
    TopologyService move to M3; Diamond detection stays in M2 (§0, §2.4, §3.1, §3.5, §4, §5.4).
    (`WS/M2_PLAN.md:62`, departures 9 and 17 at `:1383`, `:1400`.)
11. The task breakdown of §10 is superseded by `WS/M2_PLAN.md` §5-6 for M2 (departure 16 at
    `WS/M2_PLAN.md:1396`).

Amended 2026-11-09 for M3, the scope 1-4 library (`WS/M3_PLAN.md` §3 decisions D-M3-1..10, §8
deferred items, §12 departures; task notes `WS/notes/rr-{gen4,iface4,portfolio,basis4,assemble4,
diamond}.md`). Where this document and the M3 plan differ, the plan wins. Each amendment is
marked "M3 amendment" in place; the departures are listed in §0 item 11. Code citations are at
rustred `p_integral` `b7fd9f2b` (the M3-T8c final merge). In short:

1. The Diamond lane lost its race: the 4 Diamond nodes ship as Custom owners (C-D, §3.5).
2. The 4L Custom owners are 21 + 4, generated once all-rank, with the LP count stored (§3.4).
3. Generation runs under time and RSS budgets with an opt-in L2 fall-through. The rest of the
   locus ladder (§4) moves to 5L.
4. The 4L basis is Forcer's 16 masters (§6, conventions in `docs/p_integrals.md`).
5. No version constant was added or bumped in M3 (§8).
6. The scope 1-4 package ships in vakint-external-data (D-SIZE (B), §8).

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
   - 4L: 438 DAG nodes in Forcer-tree child semantics, equal to Forcer's 438 node by node,
     21 custom; 3L 43 and 2L 7 (sub-DAGs of Forcer's own file, identical actions).
   - M2 amendment: the earlier "427 DAG nodes (Forcer's 438 with eps-on-q keys)" was a wrong
     cause. Forcer's 438 keys are pairwise distinct with and without the eps on `q` [M
     `WS/SCRATCHPAD/m2-design/cascade.md` §0 item 1]. 427 is the Demand child semantics (children
     = rule outputs only, eager integer roots), 426 with another triangle choice. Demand is not
     closed under the subsector masks that run-time keys reach; ForcerTree is closed under single
     integer-line contractions. The production DAG is therefore ForcerTree; Demand is a
     diagnostic (plan D1, departure 1). The custom set (21) is the same in both.
   - 5L: 6,197 nodes, 283 custom: 129 5L integer, 88 4L-eps, 42 3L-eps, 14 2L-eps and 10 lower
     integer.
   - M1's exact series-parallel / triangle / generic class counts: 4L 79 / 107 / 94; 5L
     547 / 1,898 / 1,697 (the WL lower bounds were 544 / 1,867 / 1,636).
   - On the measured 5L canary (E1), 26 of the 33 batch-B failures are rule nodes. Section 3.6
     has the details.
   - The three failing 4L non-planar sectors are insertion nodes.
5. **Refusals become typed ledger entries.** (M2 amendment: the `LocusPolicy` plumbing is M3;
   no 1L-3L owner needs loci. M2 generates under the default `Abort`; contingency C3 of the plan
   pulls the plumbing into M2 only on an abort.)
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
    - M2 amendment: step and tail caches only, with the Gamma tag in the record identity; no
      result cache (§7.4). One library program per release scope shares one step cache across
      its loop counts (§7.4).
11. **M3 amendment: decisions and departures of the 4L milestone** (`WS/M3_PLAN.md` §3 at
    `:183-279`, §8 at `:1098`, §12 at `:1289`).
    - **One program per engine, scope 1-4 supersedes 1-3 (D-M3-1).** With a scope 1-4
      library present, every 1L-4L input goes through it. There is no cross-program step
      sharing: the 1L-3L part runs cold under 1-4. Two programs per engine and stable
      presentation ids across programs move to M5.
    - **Diamond lane: race lost, C-D (D-M3-2; plan departure 9).** M3-T17 merged (`1a36bcc2`,
      2026-11-09 14:35Z) before M3-T5b, so the 4 Diamond nodes d384, d394, d399 and no3 ship as
      Custom owners. This departs from "every node takes Forcer's action" (§0 item 4, §9) on
      those 4 nodes only. Details and the reason are in §3.5.
    - **Owners (D-M3-3; plan departure 3).** 21 Forcer custom nodes plus the 4 Diamond nodes,
      all-rank through `generate_custom_owner`
      (`crates/rustred-app/src/application/propagator/generate.rs:245`). The LP count runs once
      per owner (M3-T15) and is stored; regeneration recounts only owners of at most 9 lines
      (§3.4).
    - **Generation safety (D-M3-4; plan departures 2 and 4).** Every generation has a time and
      an RSS budget, and there is an opt-in L2 fall-through: at least 3 permuted slot orders,
      then alternative source orders. There is no chart re-solve, no deeper search and no
      `LocusPolicy` plumbing (§3.4, §4.3).
    - **The locus ladder moves to 5L** (plan §8, DEC 2026-11-08 14:04Z (b)). This covers L0
      emptiness, L1 affine-bilinear, the L3/L4 discharge driver, `LocusPolicy::Ledger`, the
      LocusLedger and the runtime LocalLaporta overlays (former T11, T12, T14, T16). §4 stays
      the design for 5L. No scope 1-4 owner needed it: all 25 close under the default order with
      residuals = LP [M `WS/notes/rr-gen4.md`].
    - **Basis (D-M3-5).** The terminals map onto Forcer's 16 masters (`P_BASIS_MASTERS_1_TO_4`,
      `crates/rustred-core/src/propagator/basis.rs:150`). The map is a square layer of format
      `rustred.p-basis-layer.v1` with no relations between residuals (contingency C-B not
      needed). The signs and normalisations are in `docs/p_integrals.md` ("Scope 1-4: Forcer's
      16 masters").
    - **D-SIZE (D-M3-6; plan departure 8).** Outcome (B), final by user ruling (DECISIONS
      2026-11-09 01:01Z): the scope 1-4 package is loaded from vakint-external-data and is not
      vendored. The sizes are in §8.
    - **Default order (D-M3-7).** This is vakint-side, with no order code: RustRed comes first
      whenever vakint has a 4L library, and Forcer handles 4L when none is present. There is no
      silent Forcer retry after a RustRed run-time error.
    - **KO RS sets (D-M3-9, plan §0.1, departure 1).** These are not owners of the scope 1-4
      program: the 13 KO 4L-eps nodes are reached only from 5L parents. M3 uses only T075p1, as
      the oracle of T075c (§3.4). The 5L phase imports the rest.
    - **Test placement (D-M3-10).** The 4L assembly, regeneration and validation tests are
      `#[ignore]` release tests (`crates/rustred-app/tests/p_library_app/assemble4.rs`). They run
      once on the final tip and at the batch end of the M3-T8c final merge.
    - **Deferred to M5:** the Diamond lane (M3-T5b branch `p_integral-rr-diamond` `2f744d99`,
      §3.5), a blocking 4L speed gate, ablations, and cross-program step sharing. **Deferred to
      5L:** `register_family`, run-time TopologyService use, and the reserved lanes of §3.5.

## 1. Vocabulary

| Term | Meaning |
|---|---|
| top family | An L-loop p-family: one (L+1)-loop cubic vacuum parent with one line cut. Counts: 1/1/3/11/64 at L = 1..5. `C = L(L+1)/2 + L` coordinates (2/5/9/14/20). |
| presentation | An `IntegralFamily` plus the role of its coordinates. The kinds are: labelled top family, owner presentation, caller family. Identified by `PresentationId(u32)`. M2 amendment: kinds `Top`, `Induced`, `InsertionAdapted`, `CarpetAdapted`, `TriangleAdapted`, `EpsNode`, `Caller` (M3); every DAG node gets a lane-adapted node presentation; `PresentationId` is library-local. |
| labelled sector | A (presentation, positive-line mask) pair. |
| owner class | The marked-matroid class of a nonzero sector (glued graph `Gamma + e0`, `e0` = the `q` line). Identified by `OwnerClassId(u64)`. M2 amendment: key `OwnerClassKey` (`propagator/class.rs`), separate from the DAG-node key `GraphKey` (`rustred-topology`). |
| owner | The program serving one class: a presentation, a lane, a program, its terminals and a descent witness. |
| route | A verified map from a labelled sector to its owner's presentation. It is a signed affine loop map. |
| eps-line | A line whose power carries `j` eps units, `a + j eps` (Forcer's crossed line). It comes from an integrated insertion and is never pinched. |
| eps-node | A DAG node with at least one eps-line; its family has `power_shifts`. |
| Gamma tag | A sparse small-integer exponent vector over `Gamma(1 + m eps)`, `m != 0`. Tags add along a path. |
| DAG node | A topology of the discovery DAG (§2.4): eps-coloured canonical graph plus its lane. |
| terminal | A key at which reduction stops: (tag, trivial node) and (tag, custom-node residual). |
| ledger | `LocusLedger`: every generator refusal and how it was discharged. |
| library | Everything above for one release scope, plus the relations layer and version constants. M2 amendment: one library program per release scope (built-in: scope 1-3, all vakint tops with L <= 3 as DAG roots), not one library per loop count. |

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
- **Change 1.** `eligible()` admits `E = 1` families with a numeric Gram matrix (done in P0).
  - From M2 it also admits power shifts: eps-nodes are owners too. M2 amendment: the gate is at
    `crates/rustred-core/src/sector/symmetry/discovery/mod.rs:77-83` (the test at `:78`), not
    `:69`; Changes 2 and 3 landed in P0. Owner class keys are computed on the support
    `raw ∪ forced` (§3.4).
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
  - M2 amendment (plan §3.1, §4): `OwnerClassKey { loops, lines, eps_lines, canonical }`;
    `OwnerClassId` = low 64 bits of `blake3(P_CLASS_KEY_VERSION LE || bytes)`, used as a name.
    Distinct canonical bytes with an equal id are `PError::MalformedLibrary` at build and load.
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

Each class has one owner presentation, chosen deterministically.

M2 amendment (plan §3.1, D7, departure 7; critique-phys MF3 item 1): every DAG node gets a
lane-adapted node presentation (`present`, `propagator/present.rs`). The owner of a class is the
lane-adapted presentation of its min-GraphKey DAG node; the census picks exactly this owner, so
the owner does not depend on the library's presentation list. The Induced presentation (the
vakint top-family sector) is used only for integer Custom nodes. The "first labelled sector in
(t descending, family name, mask)" rule below is superseded. The list of the first draft:

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
  transverse decomposition of §3.2 applies. M2 contract (`check_insertion_contract`, plan §4):
  `k_l` occurs only in the two bubble line slots (`k_l` and `k_l - P`, `P` free of `k_l`) and in
  ISP slots `k_l.Y` (`Y` free of `k_l`).
- **Carpet-adapted** (carpet lane). The outer loop momentum is `p`; no ISP contains `p1` or `p2`
  (fa:1105).
  - M2 amendment, corrected contract (`check_carpet_contract`, plan §4; critique-phys MF1; Forcer
    restricts only dot products, fa:1103): `q` is routed only through `r = (p - q)^2`, with
    `p = mom(third)`, or `p` = the blob's external momentum when `third` is absent. Every other
    line slot is `q`-free. Every ISP is either `q`-free or `l.q` with `l` a blob loop momentum (no
    `p` component). No ISP contains `p.q`: it is expressed through `r`, `third` and `p^2`.
  - The blob is a two-point function of `p`, so `p` necessarily occurs in blob lines (for example
    `(l - p)^2`); a contract that kept `p` out of the blob lines would reject every real carpet.
- **Diamond-adapted** (diamond lane). Dots avoid the A-lines; the basis is `k_i`/`l_i`
  (Th eq. 26).
- **eps-node** presentations. Fewer coordinates, `power_shifts`, padded to `N` in `PKey`.
  M2 amendment: no padding; keys have the presentation's own arity plus the dense tag slots
  (§7.1). An eps line stores `power_shift = j (4-d)/2` on a line slot.
- **Caller** families (vakint input built from an Atom or a HEPKit dot graph with an arbitrary
  routing). They are registered at run time and routed on demand (§5.4). M2 amendment: M3; in
  M2 vakint canonicalises its inputs to its own registered families.

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
  - M2 amendment: the production DAG is `reduction_graph(tops, ChildMode::ForcerTree)` with the
    top families as roots: children = rule output plus the contraction of every integer,
    non-multi-edge line (pf:1576-1592). It is closed under the single integer-line contractions
    that run-time keys reach. The eager-roots Demand DAG is a diagnostic (§0 item 4).
  - M2 amendment: the input `p-families.v1.json` is exported from vakint's registry
    (`crates/rustred-core/data/p/p-families.v1.json`, slot order = lines then ISPs); the census
    runs over the tops and every node presentation.
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
  - M2 amendment: a sharded `RwLock<HashMap<GraphKey, Arc<NodeInfo>>>` with hit/miss counters
    (there is no dashmap dependency). In M2 only `reduction_graph` uses it, at generation; run-time
    use (key recomputation, caller routing) is M3, so the graphica skew between rustred's vendored
    symbolica and gammaloop's patched graphica cannot change run-time keys in M2.
- Cost: 5.35 us per canonisation; the 5L DAG in 3.6 s [M].

Gate: the 4L DAG equals Forcer's 438 nodes node by node, with identical actions and 21 custom;
43 nodes at 3L; 7 at 2L [M simulator]. M2 amendment: in `ChildMode::ForcerTree`, which is also
the production mode; the DAG built from vakint's families must equal it. The 3L DAG holds 10
eps-nodes (1L d4, d6, d7; 2L d11, d13, d14, d15, d17, d30, t1star05) [M cascade.md §0 item 4].

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

M2 amendment: lanes are decided per DAG node (`decide`, `propagator/decide.rs`), not per class:
the action is class-invariant at <= 4L but not at 5L. Diamond is detected in M2 (for the 4L
golden); the Diamond lane is M3. The 1L-3L DAG has no diamond node.

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

M2 amendment (plan D4, departure 4): in M2 the SeriesParallel lane IS the Insertion step applied
along the chain (`SeriesParallelStep` delegates to `InsertionStep`); every SP node has Forcer
action Insertion [M]. The transverse `SpPlan` evaluation described below is the independent test
oracle, not the production path. A single-step fold exists only behind a flag (default off); the
comparison of the two forms is an M3/M5 ablation. Kernel signs follow convention C2
(`docs/p_integrals.md`).

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

Counts [M]: 335 nodes at 4L (M2 amendment: the ForcerTree insertion count; Demand gives 324,
or 323 with another triangle choice [M cascade.md §0 item 1]), 4,838 at 5L. Insertion takes the
custom count from 1,741 to 283 overall (5L integer 1,618 -> 129; 4L 123 -> 21).

### 3.2c Carpet lane (`propagator/carpet.rs`)

Forcer's carpet rule [R Th eq. 18 p.44 = FP eq. 26 p.35; pf:1413-1448].

- Applies when a line `r` joins the two external vertices and the rest is any `(L-1)`-loop
  two-point blob; eps is allowed.
- Step: the double sum over `sigma` and `j`, with tensor reduction of the `p_i.q` numerators.
- Output: the `(L-1)`-loop blob key with its integer powers unchanged, and a relative tag from
  `G(alpha + Sum a - L_sub d/2 - sigma, beta)`.
- No exception branches [D].
- M2 amendment: the presentation satisfies the corrected carpet-adapted contract of §2.2 (`q`
  only through `r`); blob numerators `(l.q)^k` with `k >= 2` and the `third`-absent case are
  tested. Fallback if the lane slips: d121 and d60 become Custom owners (plan contingency C2).

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

M2 amendment (plan D3, departure 3): the rule is a template `PLaneStep` (`TriangleStep`,
`propagator/triangle.rs`) applied by our own step, not a SpIRed `SectorRule`.

- `TriangleTemplate::extract` builds `SourceSystem::from_family`
  (`crates/rustred-core/src/solver/source.rs:90`) on the triangle-adapted presentation and takes
  the `(d/dk).k` row, `k = mom(b)`. The coefficients come only from RustRed's own row; "no
  hand-written formula is trusted" is kept.
- Shape check: zero-shift coefficient `d + N - n_a1 - n_a2 - 2 n_b - (j1 + j2)(4-d)/2` and exactly
  four shifted terms. A mismatch is `TemplateMismatch`; the class then falls back to Custom, and
  the reason is recorded.
- The step maps `I(n)` to four children divided by the pivot. Children with `b`, `c1` or `c2` at
  0 are Route keys (subsector); the others are Apply keys of the same owner. The descent is strict
  in `n_b + n_c1 + n_c2`.
- It is exception-free (the pivot's `d` coefficient never vanishes), so it needs neither
  `finish_rule` nor exception extraction. The eps apex (d30 at 3L) works without two-sided SpIRed,
  power shifts being lifted into rows by the IBP generator
  (`crates/rustred-core/src/identity/generator/construction.rs:76-81`).
- This decouples the Triangle lane from the two-sided SpIRed work. Version constant:
  `P_TRIANGLE_TEMPLATE_VERSION`.

The construction of the first draft (superseded in M2 by the bullets above):

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
(M2 amendment: one `TriangleStep`, no candidate payload; see above.)

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
eps-nodes with `power_shifts`. At 1L-3L they are NO (integer, Induced presentation) and the eps
kite t1star05. M2 amendment: they run as `CustomStep<M>` (§7.1).

**M3 amendment: the 4L owners, with stored LP** (`WS/M3_PLAN.md` D-M3-3 and D-M3-4; M3-T15, I1,
T13, T8c). Golden: `crates/rustred-core/tests/p_library/golden/owners_1-4.tsv`, one row per
owner with its LP, residuals, rules, seconds, RSS and payload sha256.

- **Which owners.** The scope 1-4 program has 25 Custom owners, one per class (no Whitney twins
  among them):
  - 2L, eps: t1star05, t1star24, t1star34, t1star45, t1star55;
  - 3L: NO (`nostar0`, integer); eps: fastar2, fastar3, bustar5, lastar5, nostar5, nostar6;
  - 4L, integer: the 9 Forcer custom nodes bebe, cross, lala, nono, bubu, no6, no1, no2, haha;
  - the 4 Diamond nodes d384, d394, d399, no3 (C-D, §3.5).

  These are the 21 Forcer custom nodes plus the 4 Diamonds. Inventory counts:
  `node_lane_L{2,3,4}_Custom` 5 / 7 / 13 and `owners_Custom 25` in
  `crates/rustred-core/tests/p_library/golden/inventory_1-4.txt`.
- **LP and residuals.** 16 owners have LP 1 and one residual. Their residuals are the 16
  terminals that the basis layer maps onto Forcer's 16 masters. The other 9 owners have LP 0 and
  no residual: t1star34, fastar3, bustar5, lastar5, bubu (T148) and the 4 Diamonds. For every
  owner, residuals = stored LP, and for the 21 Forcer nodes the LP equals
  `rules_study/targets.tsv` `masters_lp_nosym`.
- **Generation.** Every owner except NO and t1star05 was generated once, all-rank, by
  `rustred p-generate --max-loops 4 --owner <class>` (`generate_custom_owner`,
  `crates/rustred-app/src/application/propagator/generate.rs:245`; unrestricted SpIRed,
  `SearchFinite`, two-sided eps axes). Each owner ran as one guarded job, with
  1-33 s of generation per 4L owner [M]. NO and t1star05 are the M2 payloads, reused because no
  P_* version had moved. Payloads and records: `WS/artefacts/m3/owners-1-4/`
  (`<label>.rrbin` + `<label>.json`, schema `rustred.p-custom-owner.v1`,
  `crates/rustred-app/src/cli/p/generate.rs:50`).
- **The LP count runs once and is stored** (`custom_owner_lp_count`, `generate.rs:269`). The
  count is `CustomOwnerMeta.lp_count`
  (`crates/rustred-core/src/propagator/custom.rs:483-490`), which `library.json` carries per
  owner. The hard guard at build and at load reads the stored count (`custom.rs:561-575`:
  `lp_count >= 1` with no residual is refused); load never re-derives it. The 10- and 11-line
  owners cost 176-4,740 s of LP each (no2 the longest; peak RSS up to 3.5 GB, no1) [M,
  indicative].
- **Consumption.** `rustred p-package --max-loops 4 --reuse-custom <owners-1-4> [--recount-lp]`
  (`crates/rustred-app/src/cli/p/package.rs:1-19`).
  - `read_custom_payloads` reads a directory without `library.json` as p-generate owner records
    (`read_owner_records`, `crates/rustred-app/src/application/propagator/loader.rs:299`,
    `:464`). Each record must describe its payload (arity, support, residuals, two-sided axes,
    charts) and carry this build's `P_CUSTOM_OWNER_VERSION` and `P_OWNER_LIBRARY_SEMANTICS`.
  - `--recount-lp` recounts only the owners of at most `RecountProvidedLp::DEFAULT_MAX_LINES` =
    9 lines (`crates/rustred-app/src/application/propagator/assemble.rs:159-184`). That is 18 of
    the 25. The 7 owners of 10/11 lines (no1, haha, no2, no3, no6, d394, d399) are recounted only
    with `P_LP_RECOUNT_ALL=1`, never by default. A recount different from the record is refused.
- **Budgets (M3-I1).** `GenerationBudget { max_seconds, max_rss_bytes }` (`generate.rs:200`) is
  checked in-process; over budget is `PError::BudgetExceeded { resource: "time" | "rss" }`
  (`crates/rustred-core/src/propagator/error.rs:43`). CLI: `p-generate --max-seconds <s>
  --max-rss <bytes> --lp-from <dir>`, where `--lp-from` passes a recorded LP as
  `CustomOwnerOptions.lp_given` (`generate.rs:89`) so a retry skips the count.
- **L2 fall-through (M3-T13).** This replaces "`BoundedPortfolio` with fall-through" and
  `LocusPolicy::Ledger` below for M3. It is opt-in: `CustomOwnerOptions.fallthrough`
  (`CustomOwnerFallthrough`, `generate.rs:110`) is `None` by default, and `p-generate` passes
  `None`.
  - When on, attempt 0 uses the default order. After a failed chart or `BudgetExceeded`, the
    retries are tried in this order (`fallthrough_orders`, `generate.rs:537`;
    `fallthrough_retries`, `:579`; `generate_custom_owner_on_order`, `:496`):
    - at least `MIN_FALLTHROUGH_ORDERS` = 3 permuted slot orders (`:121`). Each permutes the
      default order program's support and coordinate priorities (lines among lines, ISPs among
      ISPs), with seeds 1, 2, 3;
    - for an owner with eps axes, `FallthroughOrder::NumeratorBeforeEps` (`:147`);
    - then 3 to 6 alternative baseline source orders (`FALLTHROUGH_SOURCE_ORDERS`, `:126`).
  - Each retry is a whole regeneration. `solver/sector/selection.rs` is unchanged, so the
    recipe of a persisted `bounded-portfolio` keeps its meaning.
  - What is never retried: a guard refusal (false knock-out), an LP-count failure, or a
    support/chart mismatch.
  - A winning attempt i > 0 records "custom owner; L2 fall-through attempt i: <order>" as the
    bundle's `family_source`. The payload records its order program.
  - There is no chart re-solve and no deeper search. A 4L-DAG owner still refused would go to
    the user (contingency C-L). No scope 1-4 owner needed a retry.
  - Evidence (`crates/rustred-app/tests/p_library_app/portfolio.rs:838`, ignored; run once,
    `WS/artefacts/m3/rr-portfolio/run-t075c-7`): the 4L eps node T075c (scope 1-5) is refused
    under the default order (unsupported nonlinear equalities). It closes on the seed-1 slot
    order with 386 rules and 0 residuals = LP 0, and 600 of 600 evaluations equal the
    VERIFIED_RS T075p1 set.

Runs `family_candidates_from_family_controlled` on the induced presentation, with
`selected_sectors` = the owner masks of that top family. Settings:

- `SectorSolveOptions::default()`: unrestricted, `SearchFinite`, numerical depth 2;
- `RuleSelectionPolicy::BoundedPortfolio` with at least 3 alternative source orders and
  fall-through (§4.3);
- `LocusPolicy::Ledger` (M2 amendment: M3; M2 uses the default `Abort`, see §0 item 5).

The p generation request rejects `max_numerator_rank` and `RetainRankFinite`; only L4 uses
them, per locus.

eps-axes (M2 amendment, plan §3.4, D2, D10, departures 2 and 20; this replaces the first
draft's "excess `|n - 1|`, two charts per eps-axis" and its gate list):

- An eps line has power `n + j eps`, `n` in Z, `j` in 1..3, stored as `power_shift = j (4-d)/2`
  on a line slot of a massless one-external family.
- Two supports, never mixed: `raw(key) = {i: n_i > 0}` (the SpIRed chart; candidate dispatch is
  keyed by it) and `support(key) = raw ∪ forced`, `forced = {i: power_shift_i != 0}` (graphs,
  classes, census, routes, phases, descent, transport). The zero analyzer already uses
  `raw ∪ shifted` (`crates/rustred-core/src/sector/zero/analysis.rs:180-193`).
- Why two-sided: SpIRed cannot close the charts one-sided. The top chart lowers the eps axis
  1 -> 0 into the `n <= 0` chart, which one-sided SpIRed treats as a subsector; that chart fails
  closed with "numeric RHS index activates an inactive sector coordinate"
  (`crates/rustred-core/src/solver/exception.rs:110-116`) or does not terminate [M
  `WS/SCRATCHPAD/m2-design/rr-code.md` §0.1]. Solving only the top chart produced false
  knock-outs.
- Two-sided eps axes inside SpIRed, opt-in: `FamilyCandidatesRequest.two_sided_eps_axes`
  (default false; set only by `p-generate` and the test helper `generate_eps_family`). An axis
  qualifies only with a shift of exactly `j(4-d)/2`, `j != 0`, on a line slot of a one-external
  family; any other shift with the option set is a typed refusal. Every existing shifted-family
  test stays byte-identical.
- Order: a two-sided axis is always in the support, with the injective excess
  `e(n) = 2(n-1)` for `n >= 1` and `e(n) = 2(1-n)-1` for `n <= 0` (n = 1 -> 0 is the master
  position, n = 0 -> 1, n = 2 -> 2, n = -1 -> 3, ...). It is identical in
  `crates/rustred-core/src/solver/index.rs`, `crates/rustred-order/src/compare.rs` and
  `crates/rustred-core/src/sector/ordering.rs` (whose complexity key stays "exact, injective",
  `ordering.rs:23`). The generation order equals the run-time descent check, which requires a
  strict decrease (`crates/rustred-core/src/reduction/ordered_reduction/mod.rs:276-291`).
  `|n - 1|` is not injective and would break the exact complexity key.
- Custom eps owners: `selected_sectors` = all `2^j` raw charts of the owner support; chart+
  rules may produce chart- terms only if they descend in this order; residuals sit at `n = 1`.
- Hard false-knock-out guard: `generate_custom_owner` and `generate_eps_family` compute the
  Lee-Pomeransky count with shifts (`MasterCounter::try_new_p_shifted`); `LP >= 1` with 0
  residuals is `PError::Generation`. The payload's two-sided data must cover every forced axis
  and every raw chart must be present. `library.json` records `lp_count` and `residuals` per
  Custom owner; the descent certificate and the loader re-check `lp_count >= 1 => residuals >= 1`.
- Admission (plan D10): eps families enter only through a p-only path,
  `IntegralFamily::shifted_unit_scale_kinematics()`. Every existing unit-scale consumer keeps
  refusing shifted families. The p-path shift gates, all in `crates/rustred-core/src/`, are:
  - `family/unit_scale.rs:42` (`unit_scale_kinematics()` keeps refusing shifts);
  - `sector/symmetry/integral_transport/compile.rs:104-110` (opened only for p-admitted families);
  - `sector/masters/counter.rs:89-96` (`try_new` keeps refusing; opt-in `try_new_p_shifted`);
  - `sector/symmetry/discovery/mod.rs:77-83`.

  `foundry/artifact/install/source_port.rs:699` is vacuum-only
  (`validate_parametric_vacuum_family`) and is not a p-path gate.
- Transport treats forced axes as bijective lines at every power. Today
  `crates/rustred-core/src/sector/symmetry/integral_transport/transport.rs:236-240` treats a
  power `< 0` as a numerator factor and a power 0 as an absent line, which would be silently
  wrong for an eps line.
- The coefficient context stays `{d}`, so `Z_p(d)` replay still applies.

To measure in M3: in the support-primary SpIRed order, a tie-break that prefers right-hand sides
in rule-closed subsectors over custom subsectors.

Accelerator kept from the first draft: a triangle-first source order, which visits the
`(d/dk).k` rows of inadmissible triangles first. SpIRed already finds "d-benign" triangle-shaped
pivots there [M E2].

### 3.5 Diamond lane and reserved lanes

**Diamond** (`propagator/diamond.rs`), promoted from rung L3 to a primary lane [R Th eqs. 26-35
pp.47-52; DR eqs. 3-8; pf:1030-1061; arXiv:1504.08258]. M2 amendment: the lane is M3 (task T5b);
M2 only detects diamonds (`rustred-topology`, for the 4L golden). There is no diamond node at
1L-3L.

- Shape: `L + 1` paths T-`x_i`-B through 3-valent `x_i`, plus optional T-B spectator lines.
  `b_i` and `c_i` are integer lines; `a_i` may carry eps.
- Template: built from the `Sum (d/dk_i).k_i + Sum (d/dl_i).l_i` rows in the diamond-adapted
  presentation, validated like §3.3.
- Its pivot `E` has `d` coefficient `L + S + Sum j_a/2 + Sum j_s > 0`, so it is exception-free.
- Counts [M]: 4 nodes at 4L, including `no3`, which Forcer overrides with `SchemeZno3`; 42 at 5L.
- Carpet and diamond together remove about 60 5L integer custom classes (32 + 28 of the first
  draft's generic classes) [D].

**M3 amendment: departure C-D. The Diamond lane is not in the scope 1-4 library**
(`WS/M3_PLAN.md` D-M3-2, departure 9; DECISIONS 2026-11-09 14:35Z-14:36Z).

- **What ships.** The 4 Diamond nodes ship as Custom owners. Each is generated all-rank by
  SpIRed, has LP 0 and no residual, and is listed in §3.4:

  | Node | Owner class | DAG node | Lines |
  |---|---|---|---|
  | d384 | `L4T09-fcbb654a0b4296f8` | 132 | 9 |
  | d394 | `L4T10-d1f7ffcd2b2f3614` | 48 | 10 |
  | d399 | `L4T10-d07d0fffbe48f1f2` | 34 | 10 |
  | no3 | `L4T11-b08ea0232a8b1503` | 9 | 11 |

  - The mapping is M2's: `Action::Diamond(_)` presents on the Custom lane
    (`crates/rustred-core/src/propagator/decide.rs:37`).
  - The golden records it as `count diamond_nodes_on_custom 4` and
    `count forcer_action_L4_Diamond 4`
    (`crates/rustred-core/tests/p_library/golden/inventory_1-4.txt:33`, `:15`).
  - So 4 of the 438 nodes do not take Forcer's action (§0 item 4, §9 "the 4L DAG equals
    Forcer's ... identical actions"): the matcher action stays Diamond, but the lane is Custom.
- **Why.** The plan made the lane a race: it would enter only if M3-T5b merged before M3-T17.
  M3-T17 merged first (`1a36bcc2`), so C-D fired automatically, with no extra wait.
  - The user's ruling takes precedence over "every node takes Forcer's action" (DECISIONS
    2026-11-08 14:04Z, verbatim: "as soon as rustred backend is an option, it *must* be
    default then").
  - The Diamond lane changes which lane handles the 4 nodes, not their values. With M3-T17's
    basis layer, the Diamond-as-Custom library is already complete and can reach Forcer parity,
    so RustRed is "an option" from then on and must not wait.
- **Cost of the departure** [M `WS/notes/rr-diamond.md`, STEP (5)]. The package carries 4
  payloads it would not need under the lane: 3,539,779 B raw, about 207 KB under gzip -9. The
  census is also larger: about 0.75 MB more in `census.json` and `census.rrbin`. In total
  50,057,750 B against 45,780,441 B (-8.5%), measured without the basis layer.
- **Reduction time.** Exact reductions are slower on the SpIRed owners than they would be with
  the template [M, indicative]. For 30 keys each, d384 / d394 / d399 took 414 / 3,620 / 435 s
  on the SpIRed owner against 19 / 203 / 143 s with the template.
- **M3-T5b is parked for M5** with branch `p_integral-rr-diamond` at `2f744d99` (base
  `fd81c7ff`, before T17) and a HANDOFF.
  - What the branch has built:
    - the lane as designed above: `propagator/diamond.rs`, a `DiamondTemplate` from the summed
      `(d/dk_i).k_i` rows, exact in `d`, with an exception-free pivot of `d` slope
      `L + S + Sum j_a/2 + Sum j_s >= 1`;
    - a `DiamondStep` (a `PLaneStep`) whose children stay on the owner presentation through
      census routes, with no edge maps (a departure from the brief's wording);
    - `DiamondRoles` and `check_diamond_contract`;
    - `P_DIAMOND_TEMPLATE_VERSION = 1` in `p_versions()`.
  - What is verified on it:
    - G3 on d384, d394 and d399: the template equals the SpIRed owner exactly in `Q(d)` on 30
      keys each;
    - no3 under `SchemeZno3` equals Forcer exactly on 4 inputs.
  - Open for M5:
    - no3 against its SpIRed owner: stopped after more than 3 h of Exact reference reductions;
    - the rebase onto T17 and T8c, with re-blessed pins and goldens;
    - the identity move from `P_DIAMOND_TEMPLATE_VERSION`. This moves vakint's built-in
      `data/p/p-library-1-3` identity too: the integrator regenerates it at the next pin.
  - No second package and no version bump follow from C-D in M3.

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

M2 amendment: this whole section is M3 (tasks T11-T14, T16), including the `LocusPolicy` plumbing
(it touches about 29 `SectorSolution` literals and the bundle codec). In M2 no owner needs loci,
generation uses `Abort`, and the run-time policy is `PUncoveredPolicy::Reject` only. Contingency C3
of the plan pulls the plumbing into M2 if NO or t1star05 aborts.

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

M2 amendment: this subsection is stale after P0. P0 removed the app-level vacuum gates of
`routed_campaign/prepare.rs` (the routed-campaign and saved-application loaders have no vacuum
gate left, `docs/p_integrals.md`). What still refuses eps families on this chain is
`integral_transport/compile.rs:104-110` and the power `<= 0` branch of `transport.rs:236-240`,
both handled on the p-only path (§3.4). Route records are `SignedAffineLoopMap { loops_in,
loops_out, loop_linear, loop_external, external_sign }` (plan §4); forced axes map onto
equal-shift axes.

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

M2 amendment: `register_family` is M3 (plan D11, departure 9). In M2 vakint canonicalises its
inputs to its registered families (`P_FAMILIES`), and run time never recomputes keys.

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
   M2 amendment (2026-11-06, DECISIONS 17:35Z): only for owners without forced axes. Every
   two-sided order answers `is_support_primary() == false`, so the literal check would refuse
   t1star05. For an eps owner the persisted program's two-sided flags must equal the forced
   axes of its sector, and descent is checked per RHS term through the owner's `order_key`
   (`PLaneStep::order_key`, D-T6-1);
2. every rule is checked per RHS term by the existing strict-descent comparison, and stores a
   `DescentWitness { SameSectorStrict | Subsector }`;
3. every route and every DAG edge map is verified; a route maps `t` lines to an owner with the
   same `t`, and its outputs are Apply keys, never routed again;
4. series-parallel, insertion and carpet steps emit terminals, or keys with smaller `t` (and
   `L - 1` loops);
5. overlays satisfy check 2;
6. eps-axes are forced active in every support mask. The `power > 0` masks are fixed at
   `sector/ordering.rs:26`, routed `ordered.rs:111-112` and `layered.rs:103-110`.
   M2 amendment: the vacuum routed sites (`engine/ordered.rs:111`, `engine/layered.rs:103-110`)
   are not touched; the p adapter computes `support = raw ∪ forced` itself, and the order uses the
   injective `e(n)` of §3.4.
7. M2 amendment, hard guard: every Custom owner with `lp_count >= 1` has `residuals >= 1`, the
   two-sided data covers every forced axis, and every raw chart is present (§3.4).

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

M2 amendment (plan D9, departure 19; critique-eng MF4, critique-phys MF6): the generic sketch below
is superseded by the frozen interfaces of `WS/M2_PLAN.md` §4.

- `PLibrary`, `POwner`, `PLaneStep` and `PStepContext` are non-generic. Keys are dynamic
  `IntegralKey`s of the presentation's arity plus `P_GAMMA_TAG_WIDTH` dense tag slots; the Ordered
  engine is generic over the key type only
  (`crates/rustred-core/src/reduction/ordered_reduction/mod.rs:69-122`).
- `OwnerProgram` is `Lane(Arc<dyn PLaneStep>)` or `Missing { ledger }`; every lane (SeriesParallel,
  Insertion, Carpet, Triangle, Custom) is a `PLaneStep`.
- Custom owners run as `CustomStep<const M: usize>`, `M` = the owner presentation's arity
  (t1star05 5, NO 9), created by `custom::build_custom_step` through
  `with_candidate_runtime_arities!` (`crates/rustred-core/build.rs:98`). Preparation requires
  family arity == storage `N` without `capacity-dispatch`
  (`solver/candidate_reduction/preparation/shared.rs:102`, `arity.rs:65-70`), and vakint builds
  rustred with `default-features = false`: hence one `N` per owner, not per library.
- An arity outside `compiled_runtime_arities()` (default 1..=16, `build.rs:10-12`) is
  `PError::Unsupported` naming `RUSTRED_RUNTIME_ARITIES` (fail closed). 5L owners (`M = 20`) need
  a build with 20 in the registry (M3).
- Loading: `load_p_library(PLibrarySource, PLibraryOptions)` in the app crate, not
  `PLibrary::load`.

The first draft's sketch:

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
  M2 amendment: `PKey::{Route{presentation, view}, Apply{class, view}}`, `view` = powers
  (presentation arity) ++ `P_GAMMA_TAG_WIDTH` dense tag slots; no padding. Phases are per `t`
  (Route(t) then Apply(t)), not per node (plan departure 11).
- The coefficient context stays `{d}`; `modular_polynomial` is unchanged.
- Phases come from the DAG nodes: `t` descending, then Route and Apply per node. Children always
  land in the same or a later phase, as the engine requires (`ordered_reduction/mod.rs:1-120`).
  Loop counts mix within one `t` level: Forcer's level 8 holds 13 3L and 87 4L topologies [M].
- Step, tail and result cache identities stay tag-free; their values carry relative tags. This
  corrects the claim of `WS/notes/forcer-tricks-ordering.md` that tails and results need the tag
  in their identity [D].
  M2 amendment (plan D5, departure 5): reversed. Steps carry relative tags, children carry
  absolute tags (parent + relative), and step and tail identities include the absolute tag.
  Different tags never merge on the frontier. Tag-free identities (a step-cache rebase hook) are
  an M3 item if the measured tag multiplicity exceeds 1.5.
- Apply steps by lane:
  - series-parallel, insertion and carpet: the closed steps;
  - rule-based owners: the existing `CandidateEvaluator` dispatch;
  - `Uncovered`: the overlay, then local Laporta (M2 amendment: M3; M2 rejects, fail closed).
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
  - M2 amendment (plan D8, departure 18; critique-phys MF3): one library program per release
    scope. The identity is `PLibrary::identity() = "p-library:scope=<a>-<b>:" + p_versions()`:
    every `P_*` version constant (including `P_PRESENTATION_VERSION`, `P_FAMILIES_VERSION` and
    `P_TOPOLOGY_VERSION`) and the scope; no `L`, no digest. One cache directory serves every loop
    count of the program ("the caller keeps one cache per program",
    `crates/rustred-core/src/reduction/ordered_reduction/step_cache/mod.rs:120-128`). Records are
    meaningful only inside one program identity; golden inventories make "content changed,
    identity unchanged" a test failure. Stable cross-program ids are M3.
- **Record identity.**
  - M2 amendment: stages `ClassRoute { class: presentation index as u64 }` (wire tag 4) and
    `ClassApply { class: OwnerClassId }` (wire tag 5); tags 0-3 are taken
    (`step_cache/codec.rs:211-247`). Powers are followed by the absolute Gamma tag slots; caller
    families are M3. Shards are (stage, `t`, class).
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
- **Layers.** M2 amendment: M2 ships `steps/` and `tails/` only. There is no `results/` layer:
  vakint's depth-first completed-result caches were removed
  (gammaloop `crates/vakint/src/rustred_evaluation/cache.rs:41-44`), and the tag is in the step
  identity (D5). `results/`, `overlays/`, base/delta merge and `rustred p-cache` are M3
  performance items. The first draft's layout:

  `<root>/<library-version>/` contains:
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

M2 amendment: one library directory per program (built-in scope 1-3), not `p-library-L<k>/`.
M2 ships no `sp/plans.json` (SeriesParallel runs as the insertion chain, §3.2), no `overlays/`
and no `locus-ledger.json` (§4). `library.json`
(`rustred.p-owner-library.v1`) records `lp_count` and `residuals` per Custom owner. The version
constants are those of `WS/M2_PLAN.md` §4 (`propagator/mod.rs`): `P_OWNER_LIBRARY_SEMANTICS`,
`P_CLASS_KEY_VERSION`, `P_CENSUS_VERSION`, `P_PRESENTATION_VERSION`, `P_FAMILIES_VERSION`,
`P_GAMMA_TAG_VERSION`, `P_INSERTION_KERNEL_VERSION`, `P_CARPET_KERNEL_VERSION`,
`P_TRIANGLE_TEMPLATE_VERSION`, `P_CUSTOM_OWNER_VERSION`, `P_TOPOLOGY_VERSION`, and the schemas
`rustred.p-owner-library.v1`, `rustred.p-families.v1`, `rustred.p-basis-layer.v1`; `p_versions()`
lists them in a fixed order. `P_SP_LANE_SEMANTICS`, `P_LOCUS_LEDGER_SCHEMA` and
`P_DIAMOND_TEMPLATE_VERSION` arrive with their M3 lanes. The first draft's layout:

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

**M3 amendment: version constants in M3.** None was added and none was bumped on rustred
`p_integral` in M3 (`git diff 6e13f9ca..b7fd9f2b`). The constants stay as M2 left them
(`crates/rustred-core/src/propagator/mod.rs:52-69`, `p_versions()` at `:76`), so the identity of
the scope 1-3 program, and of vakint's built-in `data/p/p-library-1-3`, is unchanged. The scope
1-4 identity is `"p-library:scope=1-4:" + p_versions()` (`PLibrary::identity`,
`crates/rustred-core/src/propagator/library.rs:227`).

- `P_DIAMOND_TEMPLATE_VERSION = 1` exists only on the parked M3-T5b branch (`2f744d99`, §3.5).
  It enters `p_versions()` when the Diamond lane merges (M5).
- `P_SP_LANE_SEMANTICS` and `P_LOCUS_LEDGER_SCHEMA` were not introduced: SeriesParallel stays
  the insertion chain (§3.2), and the locus ledger is 5L (§0 item 11).
- Every M3 addition is behind an option or a new API, and no existing semantics changed:
  - M3-I1's budgets, `lp_given` and Provided LP recount. The record keys `lp_count_source`,
    `lp_given_from` and `budget` are additive; the schema string is unchanged.
  - M3-T13's opt-in fall-through (`None` by default).
  - M3-T17's `P_BASIS_MASTERS_1_TO_4` (a reference list chosen by scope through
    `p_basis_masters`, `basis.rs:268`; `PBasisOptions::default()` stays the 1-3 list).
  - M3-T8c's owner-record reader and scope-generic layer attachment.
  - M3-T19 STAGE 1's R1 fix: the concurrent Exact scheduler journals the adjoint tail for p
    step caches. It changes no step output, record encoding or tail semantics, so no `ORDERED_*`
    or P_* bump.
- The basis layer is not part of the identity. M3-T8c's scope 1-4 inventory pin
  (`crates/rustred-app/tests/p_library_app/assemble4.rs:608`) was therefore replaced, not
  appended, when STAGE 2 attached the layer: same identity, new inventory text.
- Other new named constants are not versions:
  - `RecountProvidedLp::DEFAULT_MAX_LINES = 9` and the variable `P_LP_RECOUNT_ALL`
    (`crates/rustred-app/src/application/propagator/assemble.rs:164-165`);
  - `MIN_FALLTHROUGH_ORDERS = 3` and `FALLTHROUGH_SOURCE_ORDERS` (`generate.rs:121`, `:126`).
- Open tasks may still bump: M3-T7d and M3-T19's final stage, both still running when this was
  written. Under `WS/M3_PLAN.md` §2 delta 6 they must report any bump, and that bump would then
  be added here.

Packaging:

- 1L-3L are embedded in vakint. The 3L built-in library has 43 DAG nodes and 2 SpIRed owners
  (NO and the 2L kite with an eps-line).
- 4L goes to vakint-external-data `p/4L/`. M3 amendment: it ships as the single scope 1-4 package
  below (D-M3-1), not as a `p-library-L4/`.

**M3 amendment: the scope 1-4 package** (M3-T8c final, `WS/artefacts/m3/p-library-1-4`, with
SHA256SUMS and a README; built by `rustred p-package --max-loops 4 --reuse-custom
WS/artefacts/m3/owners-1-4 --recount-lp` at `192e21f7`).

- **Contents.** 453 presentations; 438 DAG nodes (437 census nodes, 310 classes); 185,138 census
  sectors (163,059 zero); 21,769 verified routes; 227 edge maps.
- **Owners.** SeriesParallel 99, Insertion 113, Carpet 21, Triangle 52, Custom 25 (§3.4).
- **Terminals and layer.** 17 terminals: `Unit` and 16 residuals. The basis layer
  `layer/p-layer.json` (11,342 B) maps them one-to-one onto Forcer's 16 masters.
- **Size** (one tar under `gzip -9`, the D-SIZE number): 470 files, 50,069,108 B raw, 3,270,518 B
  compressed. The largest parts:

  | Part | Raw | gzip -9 |
  |---|---|---|
  | `census.json` (the largest file) | 22,617,822 B | 1,042,982 B |
  | `owners/` (25 payloads) | 21,092,877 B | 1,367,435 B |
  | `census.rrbin` | 2,080,494 B | 731,566 B |

  The largest payload is bebe, at 3,640,693 B. No file is near vakint-external-data's per-file
  limits (about 50 MB compress-or-split, 100 MB hard).
- **D-SIZE outcome: (B), final** (user ruling, DECISIONS 2026-11-09 01:01Z, "External data
  only").
  - vakint's `.crate` is already 12,675,558 B, approximated by M3-V8 (`cargo package` itself
    refuses the path dependencies). That is over the crates.io limit of 10,485,760 B before any
    library.
  - With the 1-4 package in place of 1-3 the `.crate` would be 15,886,364 B.
  - The package is not vendored. vakint loads it from the vakint-external-data `p_integral`
    branch through the listed-library resolver (M3-V8), and RustRed is vakint's 4L default
    whenever that library is present (D-M3-7).
- **Load times** [M, indicative, shared host]: 4.15 s without the descent certificate and about
  440-450 s with it (certificate, strong completeness, layer re-derivation). The first 1L result
  takes 3.9 s on the 1-4 package against 0.12 s on the 1-3 package (D-M3-1; a poor value is an M5
  item, not a vendoring criterion).

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

- the 4L DAG equals Forcer's: 438 nodes, identical actions, 21 custom (M2 amendment:
  `ChildMode::ForcerTree`, not an eps-on-q convention; §0 item 4);
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

M2 amendment (plan departure 16): for M2 this table is superseded by `WS/M2_PLAN.md` §5-6 (23
tasks: T7 -> T7a/T7b/T7c, T8 -> T8a/T8b, T10 -> T10a/T10b/T10c, new M2-I0 and M2-D0, vakint
V1-V5; T1b runs without T1). The `LocusPolicy` plumbing of T7 moves to M3.

M3 amendment: for M3 the M3 rows below are superseded by `WS/M3_PLAN.md` §4-5 (19 tasks;
departures 2-6 and 12 at `WS/M3_PLAN.md:1289`). Live states are in `WS/M3_STATUS.json`. How
each M3 row was done, or where it went:

| AD row | M3 | Outcome |
|---|---|---|
| T5b Diamond template | M3-T5b, race | Parked for M5: C-D, 4 Diamond nodes as Custom (§3.5) |
| T11, T12, T14, T16 (locus ladder, ledger, safety net) | - | Deferred to 5L (§0 item 11); `LocusPolicy` plumbing too |
| T13 L2 fall-through | M3-T13 | Merged `9f6696a1`. Bounded at L2: at least 3 slot orders, `NumeratorBeforeEps` for eps owners, 3-6 source orders; no chart re-solve, no deeper search; opt-in (§3.4). T075c closes and equals T075p1 |
| T15 4L census and generation | M3-T15, then M3-I1 and M3-T8c | T15 merged `f4578abd`: 25 owners generated all-rank, LP counted once and stored. I1 merged `1d9c3e1b`: budgets, `lp_given`, Provided LP recount filter. T8c merged `fd81c7ff` (STAGE 1) and `b7fd9f2b` (final): package, golden inventory, certificate (§3.4, §8) |
| T17 4L basis | M3-T17 | Merged `1a36bcc2`: `P_BASIS_MASTERS_1_TO_4`, square layer (no Kira, no relations), anchors exact under 3 arithmetics, M_MAP 64/64, no `PBasis::Residual` on random 4L inputs (`docs/p_integrals.md`) |
| T18 4L validation | M3-T18 | Running at this writing (`p-validate` at 4L, §9) |
| T19 performance | M3-T19 | STAGE 1 merged `2bb4afd1` (R1 fixed, cancellation); final stage records the 4L numbers; speed is not blocking (M5) |
| T20 canary #2 | - | Removed (plan departure 12): to the 5L plan |
| - | M3-T7d | Shared custom-payload decoder, off the critical path; running at this writing |

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

M3 amendment, settled points [M]:

- **Terminal count.** The 25 Custom owners of scope 1-4 leave 16 residuals, one per Forcer
  master: a square, permutation-shaped layer (`count layer_rows_one_master_term 16`).
- **Depth.** The 4L masters are known in `Masters.prc` only through `eps^5` (no1, no2, lala),
  `eps^6` (haha, no6, nono, cross), `eps^7` (bebe, fastar2, nostar5, nostar6, no) or `eps^9`
  (t105, t124, t145, t155). Deeper requests fail closed in vakint (`Oep`). See the depth table
  in `docs/p_integrals.md`.

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
