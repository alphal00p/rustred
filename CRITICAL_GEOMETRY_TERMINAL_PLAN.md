# Critical geometry, oracle-guided terminal reduction, and campaign feedback

## Goal

Find and implement substantially useful, cost-effective generic relations that
compress RustRed's nonminimal terminal inventories and, where justified by
measurements, improve the actual campaign's rule selection or application.
Use four-loop Vakint/FMFT results as an offline diagnostic oracle, then test
autonomous RustRed discoveries on four-loop controls and the frozen five-loop
inventory. Exact minimality is not required. Numerical oracle projections or
geometric counts alone must never become reduction authority.

This is the active October 8 follow-up to the completed 829-to-608 delivery.
It supersedes that delivery's stopping instruction, without reopening every
historical optimization or altering any user-owned running campaign.

## User directive

> While doing so, also consider if the same type of rules you used here to find additional relations may be useful or can be incorporated during the actual campaign itself.
> Also, note that you can use FMFT itself  (by running it in vakint on your terminals) to see in practice the relations you may be missing, which may guide you towards what's missing for that in RustRed.
>
> Continue with the above, and  once you find a successful approach implement it. However be mindful of performance, it is not *that* important to land on an exact minimal basis, we just want to be closer to that ballpartk.
>
> Assign yourself a plan related to the above.

## Starting evidence and interpretation

- Main has the completed profile-aware refinement, ordinary sources, weighted
  circuit identities, containing-sector seeds, and optional saved-rule assistance.
- Frozen five-loop R<=2,D<=9 inventory: 829 raw ->651 normalized ->608 remaining;
  ordinary-only stops at613. These are labels, not independent masters.
- Four packaged parent inventories: H22->19, FG16->15, BMW17->15, X19->16;
  the sum74->65 is family-local, not a globally deduplicated basis.
- FMFT has19 retained formal basis symbols (PR0..PR15 plus three dotted
  integrals), not16. Existing catalogue projections are useful diagnostic
  evidence; regenerate selected terminal projections through Vakint/FMFT.
- Published five-loop110-master results suggest substantial headroom, but are
  not an unconditionally matched dimension for our bounded terminal inventory.
- Shallow containing-sector expansion, one-dot lowering and broad saved-rule
  assistance had negative or poor cost results. Reopen only with a specific
  mechanism and a discriminating control, not a larger blind seed radius.

## Delegated work and sequencing

Root orchestrates, integrates, coordinates CPU/build resources, measures and
verifies. Use separate agents for each initial lane:

1. **Oracle/census lane.** Map current four-loop remaining keys to the existing
   generic-d FMFT basis using Vakint. Classify equal/proportional combinations,
   cross-family aliases, dotted relations, numerator transport, factorization,
   and higher-sector cancellations. Use cached audited projections for the
   census, but confirm representative missing identities with fresh FMFT calls.
   Keep FORM strictly in offline oracle tests, never production RustRed.
2. **Critical-geometry lane.** Read the primary Lee-Pomeransky, critical-syzygy,
   and magic-relation papers. Audit Symbolica's pinned/current public APIs and
   existing RustRed geometry. Design the smallest useful isolated-point/count
   diagnostic and geometry-guided exact relation experiment. Exploit generic
   single-scale-vacuum structure, not topology names or loop-specific rules.
3. **Independent audit/campaign lane.** Audit mathematical assumptions and
   identify safe, economical insertion points for weighted symmetries,
   projected terminal equations, or new syzygies in campaign generation and
   application. Distinguish a finite fixed-index relation from a parametric
   recurrence; do not generalize by sampling. Review each implementation
   independently before publication.

After the diagnostic results, choose and record a bounded portfolio of concrete
experiments. Each needs a mechanism, expected benefit, falsifier, workload,
cost boundary, and exact authority path before implementation. Candidates are:

- Global cross-parent canonicalization and terminal-projected identities,
  concentrating on specific redundancies exposed by FMFT.
- Critical-point diagnostics to prioritize sectors with excess terminals;
  count only under the correct assumptions and symmetry conventions.
- Critical-syzygy-guided source selection and construction of exact uncut IBPs.
- Geometry-selected higher-sector cancellations (magic relations), retaining
  all auxiliary columns and lower-sector contributions.
- Selective modular discovery or row pruning using Symbolica, followed by
  exact materialization/replay, if measured exact elimination costs warrant it.

Do not build an independent Groebner, syzygy, reconstruction or CAS kernel.
Use Symbolica primitives; document any narrowly scoped missing service before
implementing orchestration. Positive-dimensional critical varieties are not
an infinite-master claim, and geometric counts are not coefficients. General
claims linking magic relations and critical varieties remain conjectural where
the literature states them as such. Preserve d=4-2epsilon symbolically.

## Promotion and campaign feedback

Implement successful practical strategies in cohesive generic modules, reuse
the existing finite relation session/provider and native persistence, and
expose opt-in Rust/CLI/Python controls where needed. Prefer data-driven steering
over repeated backend rebuilds. Never import FMFT rules into autonomous solves.

For every positive relation class, assess campaign use explicitly:

- Can exact normal forms prevent duplicate terminal admission or shrink
  memoized decompositions without hiding unfinished obligations?
- Can the relation provide a replayable, guarded descending rule, or is it
  valid only for a finite terminal set?
- Would an immutable, precomputed relation bank reduce campaign work, and
  does preparing/applying it cost less than the work it saves?

Run isolated campaign/application pilots if there is a justified insertion
point. Do not rewrite live checkpoints or inflate closure claims. A negative
campaign result is acceptable: retain a useful postprocessor without forcing
it into the hot path. Any introduced dependency graph must remain sound and
application must terminate or fail explicitly.

## Validation, resources and delivery

- All inputs/evidence under local untracked TMP; no changes to user campaigns.
- Preserve unrelated HEPKit/notebook/reference edits. Use existing build/heavy
  job locks and nonproduction CPU affinity; record contention and limits.
- Compare optimized frozen binaries on identical inputs and timing boundaries;
  report compilation separately, wall/CPU/RSS, row/column counts, raw/canonical/
  remaining counts, and preparation/search/application/cold-load costs.
- Start with cheap four-loop experiments, then frozen five-loop controls.
  No long blind search: stop unproductive experiments based on recorded cost.
- Validate every admitted relation exactly, with denominator/guard conditions,
  normalization factors, family/physical-arity identity, and source provenance.
  Do not drop auxiliary terms or use modular evidence as a proof.
- Check all original terminal coefficient maps, cold loading, interruption and
  resume, scope extension and policy changes when affected. Keep ordinary mode
  as the matched baseline and test flag-off behavior.
- Independent code and mathematical audit; no implementer signs off its own
  changes. Refresh numerical Vakint checks if packaged artifacts are changed;
  never change shipped catalogues merely to match an expected count.
- Continuously update CODEX_PROGRESS.md with lane ownership, decisions,
  commands, evidence, negative results and measured versus inferred findings.
- Commit/push coherent tested milestones on main using ValentinHirschi
  <valentin.hirschi@gmail.com>, preserving existing coauthorship conventions.
  Never commit licenses, reference material or campaign outputs.

Completion requires a measured successful strategy integrated and audited,
honest before/after controls, and an explicit conclusion on campaign reuse
(implemented/tested if worthwhile, otherwise evidence-backed deferral).
Do not claim minimality or completion of the broader five-loop physics goal.

## Delivered implementation and follow-up boundary

The measured successful strategy is implemented as the generic Rust library
services `VacuumFamilyAliasPlan` and `VacuumDiagonalCollectionPlan`. Exact
cross-family parameter equivalences followed by native diagonal ordinary IBP
sums reduce the four-loop collection65→22→20; the same implementation reduces
the frozen five-loop scalar subset355→354, with253 numerator keys unchanged
(608→607 combined). Native Symbolica owns the algebra. All auxiliary columns,
original source rows, guarded flat maps and mass-power differences are retained.
Cold reconstruction and independent four-loop FMFT comparisons pass.

The library interface is available now; this milestone does not add a new
CLI, persisted collection codec, or automatic Vakint/campaign rewrite. Existing
saved-session interruption/resume and packaged artifacts are unchanged.
Campaign reuse is supported at the coefficient-application boundary; there is
no measured generation/reachability improvement from the new collection.
Source-preconditioning inspection identified a future diagonal-prefix/priority
experiment, but that is not grounds to change the user's running campaign.
Broad circuit-support expansion and larger all-ordinary collection did not
justify their additional costs on these controls. Further magic/syzygy searches
need a concrete diagnostic trigger rather than an exact-minimality target.

Results, authority limits, preparation costs, API usage, negative controls and
follow-up conditions are in
`docs/research/cross_family_terminal_collection_2026-10-08.md` and
`CODEX_PROGRESS.md`. This completes the finite strategy-delivery scope, not
the broader five-loop closure or master-evaluation programme.
