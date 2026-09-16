# Four-loop Vakint matcher census

Frozen on 2026-09-16 against GammaLoop `a3d26dab9eaa4d141196b302ec95e2b8c888c2de`
on `vakint_rustred`. This is a matcher/input audit, **not four-loop IBP closure
or native numerical acceptance**.

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

## Frozen inputs and diagnostic boundary

| Input under `examples/input/` | SHA256 |
| --- | --- |
| four_loop_h.toml | f1abef4559e4a5f045abe610a7b25450ebdfefad70859e9401680346f1c4d344 |
| four_loop_x.toml | ce66a6964742b9e39483d6e5e8d2508205b12881d723e41e033a84282d3ab9ff |
| four_loop_bmw.toml | 9dedb6767d5350745deae547ddc28de46b90fbada86e11f2f6cd66d010fac65c |
| four_loop_fg.toml | 60210409f39ecd7d0ed62594c7a490388f5012eee3c8c91cdac670cd5edc842a |

The diagnostic links the frozen migrated Vakint library, SHA256
`cb237cee0c1743ff3cb90f8cbc60ced070dec65c72b776340aa7c71871cafcde`,
using published RustRed `09cef8e3` and Symbolica `953e26e2`. No GammaLoop
production code, artifacts or dependencies were changed. The run used an
invalid FORM path and passed in 0.57 seconds wall; this is a debug correctness
diagnostic, not a solver benchmark.

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
