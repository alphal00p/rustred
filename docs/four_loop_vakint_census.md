# Four-loop Vakint matcher census

The original census was frozen on 2026-09-16 against GammaLoop
`a3d26dab9eaa4d141196b302ec95e2b8c888c2de` on `vakint_rustred`. The typed
routing-validation milestone is now pushed as
`30705f12303ae41275fd61ce3ee4bbb8349dd002`. Both establish matcher/input
compatibility, **not four-loop IBP closure or native numerical acceptance**.

## Shipped typed routing validation

The existing K6 parent checks now share the crate-private
`Integral::validate_parent_routing` validator. It verifies every defining
parent slot, including contracted-away slots, and transports surviving
canonical momenta with Symbolica's simultaneous replacement. Comparisons
retain exact momentum signs. K6 mass/power handling is unchanged; no graph
rematching, topology-name dispatch, new momentum solve or FORM call is added.

Typed tests use the matcher's internal records directly, without the Debug
inspection used by the original external diagnostic below. They validate all
**19 registered classes, 123 surviving physical slots and six nonidentity
parent-routing witnesses**, with structural parent counts `[1,1,1,16]`.
Renaming every topology to an opaque label leaves the checks valid. Negative
tests reject missing or malformed routing, corrupted coordinates or momenta,
a changed pinched-out parent slot, invalid descriptors and sign flips.
Every four-loop class still explicitly rejects RustRed scalar admission,
including when master substitution is disabled.

The milestone passed formatting, `cargo check`, scoped Clippy with
`-D warnings`, and independent routing review. Its fresh focused gate has
**39 passes: 23 library tests and 16 K6 pipeline tests**, with **one existing
offline FORM/MATAD terminal-catalog comparison ignored**. Native library tests
use an invalid FORM path; the pipeline retains invalid paths for its strict
native lanes and FORM5 only for independent legacy oracle lanes. The full
**83-test acceptance inventory passed at the prior milestone but was not
rerun for this routing-only change**. These debug runs are correctness checks,
not performance benchmarks or four-loop numerical acceptance.

Commands, executable hashes, test names and results are retained at
`/tmp/vakint-four-loop-routing.dkHXAz/RESULTS.md` and adjacent logs. No
dependency pin, shipped artifact, terminal catalog, public API or default
evaluation policy changed in this milestone.

## Remaining integration work after routing

A read-only audit of `30705f12` identifies the following gates. Routing tests
alone do not mean the adapter or offline master producer already supports
four-loop evaluation.

- Replace the single-family-per-loop-count asset selection with unique
  structural parent-descriptor selection using the existing witness validator.
  Append auxiliary zero powers to the authenticated artifact arity and retain
  simultaneous numerator routing for every parent. No graph rematch or
  topology-name dispatch is needed. RustRed already owns scalar numerator
  lowering, recursive IBP application and common-mass homogeneity.
- Produce terminal catalogs offline from each authentic artifact's master
  keys. The current K6 producer fixes six indices and MATAD input; it is not
  yet a generic four-loop producer. Negative physical and auxiliary powers
  must be transported as polynomial numerator factors. Prefer exact FMFT
  PR-basis projections, using numerical Laurent records only with truthful
  precision and tail metadata. Ordinary evaluation must never run the oracle.
- Select the existing pure-Symbolica
  `FMFT::finalize_native_reduced_masters` for PR catalogs. The current RustRed
  materializer always invokes the MATAD finalizer. Preserve symbolic-master
  mode, custom epsilon conventions and normalization exactly once.
- Qualify new numerical fallback master identities by family: equal
  ten-index tuples from distinct parent families are not the same integral.
  Existing `RustRedMaster(powers)` and its tail label use only the tuple.
  Exact shared PR-basis projections do not have this ambiguity. Preserve
  existing three-loop symbolic output and Vakint conventions when extending
  the new catalog identity.
- Separate arithmetic working precision from source-master accuracy. Native
  FMFT finalization currently requires available constant precision, while
  legacy FMFT does not. Default working precision is 32 digits; some stored
  PR9d/PR11d coefficients have only 28/26 digits. More floating-point precision
  cannot supply missing digits, and this checkout has no accurate 20,000-digit
  four-loop catalog. The user's selected policy is to **warn and continue**
  when the request exceeds known master-source precision, retaining the
  requested arithmetic precision and identifying the available source digits.
  Unknown Laurent orders must still be rejected. Implementation/validation of
  that policy is separate from the completed read-only routing audit.

Reuse the existing comparative harness with a FORM5 FMFT oracle and
FeynKit/RustRed native lanes using invalid FORM paths. The required numerical
inventory has **15 deterministic reference entrypoints**, including the
four-loop decorated clover whose test name says `1l`; add all **19 registered
classes** as structural/native canaries. Keep the **five optional PySecDec
cases** separate: one requests an epsilon order beyond the present FMFT
tables. The eight native PR-finalizer tests are boundary tests, not native
four-loop reduction acceptance. Rerun the complete recorded 83-test lower-loop
inventory after integration without changing inputs, tolerances or defaults.

Exact code locations and test names are recorded in
`/tmp/vakint-four-loop-routing.dkHXAz/READINESS_NEXT.md`. This audit performed
no build or test and changed no GammaLoop code; it adds no acceptance claim.

## Coverage target

The existing matcher registers **19 four-loop graph classes**: the H, X and
BMW parents, plus FG and 15 inequivalent contractions. This differs from the
15 deterministic numerical reference entrypoints: several tests exercise the
same graph with different powers, masses or numerators.

All 19 records retain four simultaneous parent-routing coordinates. An
external diagnostic verified all **123 surviving physical propagator slots**
with native Symbolica. Every transported momentum matched the parent with its
exact sign; there were no sign-only discrepancies or failed slots. Each
parent's physical denominators independently matched exactly one external
RustRed input after squaring its momentum and subtracting unit mass.

H/X have nine physical slots and auxiliary D10. BMW/FG have eight physical
slots and auxiliary D9/D10. Ten-coordinate initial powers keep each surviving
physical power and zero-fill all contractions and auxiliary slots. Numerator
lowering may subsequently produce nonpositive auxiliary powers. All algebra
and IBP application remain RustRed responsibilities.

## Registered classes

Slots are one-based. Mask bit `i-1` corresponds to Di; auxiliary bits are zero.
Names below are diagnostic labels, never production dispatch keys.

| Matcher label | Input parent | Surviving physical slots | Mask |
| --- | --- | --- | ---: |
| I4L_H | H | 1,2,3,4,5,6,7,8,9 | 511 |
| I4L_X | X | 1,2,3,4,5,6,7,8,9 | 511 |
| I4L_BMW | BMW | 1,2,3,4,5,6,7,8 | 255 |
| I4L_FG | FG | 1,2,3,4,5,6,7,8 | 255 |
| I4L_FG_pinch_2 | FG | 1,3,4,5,6,7,8 | 253 |
| I4L_FG_pinch_3 | FG | 1,2,4,5,6,7,8 | 251 |
| I4L_FG_pinch_4 | FG | 1,2,3,5,6,7,8 | 247 |
| I4L_FG_pinch_7 | FG | 1,2,3,4,5,6,8 | 191 |
| I4L_FG_pinch_8 | FG | 1,2,3,4,5,6,7 | 127 |
| I4L_FG_pinch_1_8 | FG | 2,3,4,5,6,7 | 126 |
| I4L_FG_pinch_2_4 | FG | 1,3,5,6,7,8 | 245 |
| I4L_FG_pinch_4_7 | FG | 1,2,3,5,6,8 | 183 |
| I4L_FG_pinch_4_8 | FG | 1,2,3,5,6,7 | 119 |
| I4L_FG_pinch_7_8 | FG | 1,2,3,4,5,6 | 63 |
| I4L_FG_pinch_2_4_7 | FG | 1,3,5,6,8 | 181 |
| I4L_FG_pinch_3_4_8 | FG | 1,2,5,6,7 | 115 |
| I4L_FG_pinch_4_7_8 | FG | 1,2,3,5,6 | 55 |
| I4L_FG_pinch_6_7_8 | FG | 1,2,3,4,5 | 31 |
| I4L_FG_pinch_4_6_7_8 | FG | 1,2,3,5 | 23 |

The last class is the four-tadpole clover. The physical-line histogram is:
nine lines: 2 classes; eight: 2; seven: 5; six: 5; five: 4; four: 1.

Six FG contractions have nonidentity stored coordinates:

| Pinched slots | New loop coordinates in the retained FG parent |
| --- | --- |
| 2 | `(k1-k3, k4, k1, k2-k3)` |
| 3 | `(k1-k3, k2-k3, k1, k4)` |
| 1,8 | `(k2, k3, k1-k3, k4)` |
| 2,4 | `(k1, k3, k4, k2-k3)` |
| 2,4,7 | `(k1, k3, k2-k3, k4)` |
| 3,4,8 | `(k1, k2, k4, k2-k3)` |

For example, pinch 2 has canonical D8 momentum `k_new1-k_new4`.
Its retained witness gives `(k1-k3)-(k2-k3)=k1-k2`, the FG parent's D8.
The initial powers are `(a1,0,a3,a4,a5,a6,a7,a8,0,0)`. The same simultaneous
witness must transport the scalar numerator; sequential substitution is not
equivalent. The production adapter must use the existing typed witness, with
no second graph match or momentum-basis solve.

## Original frozen inputs and diagnostic boundary

| Input under `examples/input/` | SHA256 |
| --- | --- |
| four_loop_h.toml | f1abef4559e4a5f045abe610a7b25450ebdfefad70859e9401680346f1c4d344 |
| four_loop_x.toml | ce66a6964742b9e39483d6e5e8d2508205b12881d723e41e033a84282d3ab9ff |
| four_loop_bmw.toml | 9dedb6767d5350745deae547ddc28de46b90fbada86e11f2f6cd66d010fac65c |
| four_loop_fg.toml | 60210409f39ecd7d0ed62594c7a490388f5012eee3c8c91cdac670cd5edc842a |

The diagnostic links the frozen migrated Vakint library, SHA256
`cb237cee0c1743ff3cb90f8cbc60ced070dec65c72b776340aa7c71871cafcde`,
using published RustRed `09cef8e3` and Symbolica `953e26e2`. That original
diagnostic changed no GammaLoop production code, artifacts or dependencies.
The run used an invalid FORM path and passed in 0.57 seconds wall; this is a
debug correctness diagnostic, not a solver benchmark.

To inspect private fields without changing visibility, the external diagnostic
reads its own native Debug records and passes complete Atom byte arrays to
Symbolica's public `AtomView::from` in the **same live process and symbol
state**. This is diagnostic-only inspection, not a durable reader or an adapter
design. It aborts on unexpected rendering, missing fields, ambiguous parents
or failed transport. Production integration must read internal typed
`parent_routing` directly, never reproduce this Debug inspection mechanism.

Raw census, exact checked slot inventory, zero-filled powers, source, build
command and hashes are retained at `/tmp/vakint-four-loop-census.Piiwga/`.
The source registration is in Vakint `topologies.rs`: H/X/BMW register their
parents; FG requests automatic contractions using native graph canonicalization.

This establishes compatibility of the four external inputs with every
**currently registered** four-loop class. It does not prove that the matcher
enumerates every mathematically possible four-loop topology. Four closed
artifacts, terminal catalogs and complete native/FMFT comparison remain open
delivery gates; see [the integration plan](spired_vakint_artifact_plan.md).
