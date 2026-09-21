# Sparse routing among already-loaded saved owners

Status: **metadata and source inspection only**, 21 September 2026. No new
native jobs, builds, polynomial powers, IBP generation or performance measurement.
The proposal is an input-selection policy, not a closure/certification service.

## Why routing choice matters

The original selection minimizes **encoded owner bytes**, then routes every
label in a class to that owner
([selector](../../TMP/routed-all-saved-owners.Hw5g0s/select.py), lines 102–134).
The app composes `source_to_representative * inverse(owner_to_representative)`
and natively verifies/compiles that map
([prepare.rs](../../crates/rustred-app/src/application/routed_campaign/prepare.rs),
lines 127–162). Neither selection step minimizes numerator-row width.

The inspected SpIReD path instead solves in the requested literal sector:
[`solver.tpp`](../../vendor/spired/src/solver.tpp), lines 11–63, reorders and
row-reduces the source IBPs there;
[`reducer.tpp`](../../vendor/spired/src/reducer.tpp), lines 393–408, loads that
literal sector's rule file. Its special sector settings choose index priorities
([`family.tpp`](../../vendor/spired/src/family.tpp), lines 696–726), not our
representative affine routing. This does not establish that SpIReD has a better
map optimizer, or that generating every literal sector is cheap. It establishes
that representative remapping is not inherent to parametric rule generation.

Changing index priority alone cannot sparsify the affine map. Changing the
actual family/ISP basis would change saved-program identity and is not a safe
retagging. Prefer compatible saved owners and verified witnesses in the current
family. Dense numerator expansion may remain unavoidable for a particular map;
no sparse alternative has yet been established for the remaining costly labels.

## Concrete zero-additional-byte opportunity

The current 75-owner selection has SHA256
`85d4664085b58344e0bb42431ecc7c181747964b223759bb788271c528ebe6ad`.
Its eight added owners bypass their literal routes. They also provide alternate
destinations for **1,828 still-nonliteral routes**, without loading any more
owner bytes:

| Class | Already-loaded published owner labels | Nonliteral routes |
| --- | --- | ---: |
| 31744 | 1225, 6440 | 972 |
| 32274 | 29199, 9759 | 223 |
| 32288 | 4841, 8555 | 226 |
| 32576 | 12651, 26985, 19305 | 407 |

These are input metadata counts, not calls, hit rates or demonstrated savings.
The four loaded class 32737 labels already exhaust that class's routes.
For example, old hot label 9571 could try already-loaded 8555 instead of 4841;
its row sparsity is not known. Labels 12303/4623 share class 32256, with 891 saved
label alternatives but only 10608 currently loaded; they are not part of this
zero-added-bytes opportunity.

The selection-bound frozen inventory has literal files for 6,147 of the 8,171
nonidentity labels, but their cheapest copies total 63,745,839,674 encoded bytes.
Blindly loading them would exceed the current 2-GiB ingress limit, and encoded
size is not native RSS. Even the 2,650 files individually ≤1 MiB total 1,751,001,663
bytes, above the remaining ingress headroom. Any future additions should be
chosen from actual census needs, not an unrestricted preload.

## Minimal generic offline policy

After the domain census identifies relevant successor supports and actual rank
or negative-axis scopes, consider **all compatible already-loaded owners** of
each such support's class. Keep the existing route as a fallback. The number of
candidate compositions/verified-map entries and scratch memory must be bounded;
an exhausted selector leaves the old route intact and reports incomplete
optimization, not unavailable reduction.

1. Compose each candidate witness with existing Symbolica Matrix operations.
   Use existing `symmetry::verify` and `integral_transport::compile` for common
   family/context, conditions, unit Jacobian and active-row bijection admission.
   Graph or class metadata alone is not map authority.
2. Inspect the **verified denominator-affine rows**, not momentum-matrix
   sparsity. Public APIs already suffice:
   `Prepared::verified_map()` → `VerifiedMap::denominators()` →
   `constant()`/`linear().get(row,column)` and `Coefficient::is_zero()`.
   `source_root()`/`target_root()` identify active rows; `row_actions()` exposes
   monomial versus affine action. This requires no powering or new core API.
3. Score relevant inactive row widths, variable-union size, constant presence,
   coefficient size and active-column/pinch incidence. For a negative power p
   and w nonzero affine terms, C(p+w−1,w−1) bounds one row's support. Shared
   variable incidence and homogeneous total degree improve the product bound.
   These are checked combinatorial policy estimates, **not exact cancellation
   counts or runtime budget admission**. The current exact prospective planner
   is private; its implementation remains authoritative and unchanged
   ([`support.rs`](../../crates/rustred-core/src/family/numerator_expansion/support.rs),
   lines 31–80; [`expand.rs`](../../crates/rustred-core/src/family/numerator_expansion/expand.rs),
   lines 304–347). Pair-operation cost must not be replaced by output support.
4. Use actual census rank bounds, including above-entry-R successors. For an
   unresolved coupled domain, retain its predicates and label the rectangular
   cost estimate conservative; do not call it attained. Infinite/unsupported
   scopes remain explicitly unscored. Positive dots translate only the base,
   so do not enumerate them to score numerator shape.
5. Freeze one deterministic winner per source support, favouring lower
   worst/weighted support and work estimates. Preserve literal owners, all
   saved programs, scope and limits. Replace only the selected route's owner
   label/mask and owner witness in a separate selection file. The existing
   schema already expresses this; no runtime portfolio, scheduler or algebra
   change is required.

This is a routing experiment, not a matched same-graph speed benchmark. A
smaller immediate expansion can lead to more downstream rules, different
frontiers or less merging. Compare actual transport endpoints, joined and
distinct nodes, rule work, frontiers and resource usage after native admission.
The reported 1.23B duplicate joins alone cannot predict the avoidable fraction.

## Follow-ups only if this small portfolio helps

For unsupported choices, bounded additional saved owners can be considered
under explicit ingress limits. A bounded alternate-witness search is another
option: the old census returns the first valid signed basis lift
([witness producer](../../TMP/tide-routing-census.f3acKs/main.rs), lines 56–84),
while Symbolica's `CanonicalForm::orbit_generators` exposes graph automorphism
proposals. Each lifted map still needs native exact verification. A pure target
denominator permutation merely relabels polynomial monomials; it cannot reduce
the fixed map's support cardinality. The existing permutation Canonicalizer is
not an affine sparse-basis optimizer.

The support visitor saves coefficient-wrapper work, and a bounded
negative-pattern cache could save repeated powers. Neither removes distinct
translated descendants or their joins. Sparse routing can reduce that fanout
before those stages, but it does not replace parametric domain reuse or actual
missing-domain source feedback. No complete-rank coverage or master-count claim
follows from this proposal.

Audit inputs: the 75-owner selection above and its bound inventory SHA256
`5819f1254063d29c810bb3e0be77479d0cb01763c8b147ea70ee823265f4c98e`.
The metadata calculations and broader notes are recorded in
`TMP/sparse-saved-owner-routing.hwdRCH/PLAN.md`.
