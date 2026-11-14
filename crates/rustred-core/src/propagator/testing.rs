//! Test support of the p library: fake lanes and hand-built presentations
//! (owner: M2-T6 rr-plib).
//!
//! [`one_loop_reference_library`] is the 1L program every consumer can run
//! before real library data exists: the massless bubble as an exact closed form
//! behind the real engine (census, routes, phases, tags, step cache).
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraError, ExactAlgebraLimits};
use crate::reduction::ReductionError;

use super::census::{PCensus, PClassRecord, PSectorFate, SignedAffineLoopMap};
use super::class::OwnerClassKey;
use super::error::PError;
use super::families::{P_FAMILIES_V1_JSON, PFamiliesFile};
use super::lane::{LaneRoles, OwnerLane};
use super::library::{OwnerProgram, PLibrary, POwnerSpec};
use super::presentation::{Presentation, PresentationKind, Slot, SlotLine};
use super::step::{PKeyView, PLaneStep, PStepContext, PStepOut};
use super::tag::GammaTag;
use super::{PTerminalId, PresentationId};

/// The top `P1L` of the reference library and the other routings of the bubble:
/// `[k^2, (k-q)^2]`.
pub const REFERENCE_TOP: PresentationId = PresentationId(0);
/// `[(k-q)^2, k^2]`: the slots of the top swapped.
pub const REFERENCE_SWAPPED: PresentationId = PresentationId(1);
/// `[k^2, (k+q)^2]`: the top under `q -> -q`.
pub const REFERENCE_FLIPPED: PresentationId = PresentationId(2);
/// `[(k+q)^2, k^2]`: the top under `k -> k - q`.
pub const REFERENCE_SHIFTED: PresentationId = PresentationId(3);

/// `Gamma(1 + eps) Gamma(1 - eps)^2 / Gamma(1 - 2 eps)`, the tag of every `G(a, b)`.
pub fn bubble_tag() -> Result<GammaTag, PError> {
    GammaTag::from_pairs([(1, 1), (-1, 2), (-2, -1)])
}

fn algebra(error: ExactAlgebraError) -> PError {
    ReductionError::ExactAlgebra(error).into()
}

/// `(base)_n`, the Pochhammer symbol at an integer `n` of either sign.
pub fn pochhammer(
    context: &CoefficientContext,
    base: &Coefficient,
    n: i64,
    limits: ExactAlgebraLimits,
) -> Result<Coefficient, PError> {
    let mut product = context.one();
    let shifted = |shift: i64| {
        context
            .try_add(base, &context.integer(shift), limits)
            .map_err(algebra)
    };
    if n >= 0 {
        for shift in 0..n {
            product = context
                .try_mul(&product, &shifted(shift)?, limits)
                .map_err(algebra)?;
        }
    } else {
        for shift in n..0 {
            product = context
                .try_div(&product, &shifted(shift)?, limits)
                .map_err(algebra)?;
        }
    }
    Ok(product)
}

/// `Gamma(1 + m eps) * (1 + m eps)_c = Gamma(1 + m eps + c)`: the rational part
/// of a Gamma function, `eps = (4 - d) / 2`.
pub fn gamma_ratio(
    context: &CoefficientContext,
    m: i64,
    c: i64,
    limits: ExactAlgebraLimits,
) -> Result<Coefficient, PError> {
    let d = context.parameter("d").ok_or(PError::ContextMismatch)?;
    let twice = context
        .try_sub(&context.integer(4), &d, limits)
        .map_err(algebra)?;
    let scaled = context
        .try_mul(&twice, &context.integer(m), limits)
        .map_err(algebra)?;
    let half = context
        .try_div(&scaled, &context.integer(2), limits)
        .map_err(algebra)?;
    let base = context
        .try_add(&context.one(), &half, limits)
        .map_err(algebra)?;
    pochhammer(context, &base, c, limits)
}

/// `G(a, b) = R(d) * bubble_tag()` for `a, b >= 1`: the Euclidean bubble at
/// `Q^2 = 1`, measure `d^dk / pi^(d/2)`.
pub fn euclidean_bubble(
    context: &CoefficientContext,
    a: i64,
    b: i64,
    limits: ExactAlgebraLimits,
) -> Result<Coefficient, PError> {
    if a < 1 || b < 1 {
        return Err(PError::Contract(format!(
            "the closed form of G({a}, {b}) needs a, b >= 1"
        )));
    }
    let mut value = context
        .try_mul(
            &gamma_ratio(context, -1, 1 - a, limits)?,
            &gamma_ratio(context, -1, 1 - b, limits)?,
            limits,
        )
        .map_err(algebra)?;
    value = context
        .try_mul(&value, &gamma_ratio(context, 1, a + b - 3, limits)?, limits)
        .map_err(algebra)?;
    value = context
        .try_div(
            &value,
            &gamma_ratio(context, -2, 3 - a - b, limits)?,
            limits,
        )
        .map_err(algebra)?;
    for n in (2..a).chain(2..b) {
        value = context
            .try_div(&value, &context.integer(n), limits)
            .map_err(algebra)?;
    }
    Ok(value)
}

/// The fake series-parallel lane of the reference bubble: `I_M(a, b)`, `a, b >= 1`,
/// is `(-1)^(a + b) G(a, b)` (conventions C1, C2) in one step onto the trivial
/// node. Numerators are lowered onto `(a, b)` by the caller, and `a <= 0` or
/// `b <= 0` is a scaleless sector with a zero proof in the census.
#[derive(Debug)]
struct FakeSpLane;

impl PLaneStep for FakeSpLane {
    fn lane(&self) -> OwnerLane {
        OwnerLane::SeriesParallel
    }

    fn step(&self, cx: &PStepContext<'_>, key: &PKeyView<'_>) -> Result<PStepOut, PError> {
        let context = cx.library.coefficient_context();
        let &[a, b] = key.powers else {
            return Err(PError::Contract(format!(
                "a bubble key has 2 powers, not {}",
                key.powers.len()
            )));
        };
        let limits = cx.limits.exact_algebra;
        let mut value = euclidean_bubble(context, a, b, limits)?;
        if (a + b) % 2 != 0 {
            value = context.try_neg(&value, limits).map_err(algebra)?;
        }
        Ok(PStepOut {
            terminals: vec![(PTerminalId(0), key.tag.add(&bubble_tag()?), value)],
            ..PStepOut::default()
        })
    }
}

fn line(momentum: [i64; 2]) -> Slot {
    Slot::Line(SlotLine {
        momentum: momentum.to_vec(),
        eps_units: 0,
    })
}

/// `k_in = A k_out + b q_out`, `q_in = s q_out` of one loop.
fn one_loop_map(linear: i64, external: i64, external_sign: i8) -> SignedAffineLoopMap {
    SignedAffineLoopMap {
        loops_in: 1,
        loops_out: 1,
        loop_linear: Box::new([linear]),
        loop_external: Box::new([external]),
        external_sign,
    }
}

/// The 1L reference library, scope 1: the top `P1L` of vakint's families and
/// three more routings of the bubble ([`REFERENCE_SWAPPED`],
/// [`REFERENCE_FLIPPED`], [`REFERENCE_SHIFTED`]), one owner class on the top,
/// no basis map (`Unit` is the only terminal).
#[doc(hidden)]
pub fn one_loop_reference_library() -> Result<Arc<PLibrary>, PError> {
    one_loop_reference_library_with_revision(None)
}

/// [`one_loop_reference_library`] with a package content revision (batch 6:
/// `PLibraryBuilder::set_revision`).
#[doc(hidden)]
pub fn one_loop_reference_library_with_revision(
    revision: Option<&str>,
) -> Result<Arc<PLibrary>, PError> {
    let context = CoefficientContext::try_new(["d"]).map_err(|_| PError::ContextMismatch)?;
    let file: PFamiliesFile = serde_json::from_str(P_FAMILIES_V1_JSON)
        .map_err(|error| PError::MalformedLibrary(format!("p families: {error}")))?;
    let top = file
        .families
        .iter()
        .find(|family| family.name == "P1L")
        .ok_or_else(|| PError::MalformedLibrary("no family P1L".to_owned()))?
        .presentation(REFERENCE_TOP, &context)?;

    let mut builder = PLibrary::builder(context.clone(), 1..=1);
    builder.add_presentation(top)?;
    for (id, name, slots) in [
        (REFERENCE_SWAPPED, "P1L_swapped", [[1, -1], [1, 0]]),
        (REFERENCE_FLIPPED, "P1L_flipped", [[1, 0], [1, 1]]),
        (REFERENCE_SHIFTED, "P1L_shifted", [[1, 1], [1, 0]]),
    ] {
        builder.add_presentation(Presentation::from_slots(
            id,
            PresentationKind::InsertionAdapted,
            name,
            1,
            slots.map(line).into(),
            &context,
            None,
        )?)?;
    }

    let key = OwnerClassKey {
        loops: 1,
        lines: 2,
        eps_lines: 0,
        canonical: Arc::from(&b"rustred reference bubble"[..]),
    };
    let mut census = PCensus::default();
    let class = census.insert_class(PClassRecord {
        key,
        owner: (REFERENCE_TOP, 0b11),
        node: None,
        nodes: Vec::new(),
        t: 2,
        eps_lines: 0,
    })?;
    let routes = [
        (REFERENCE_TOP, one_loop_map(1, 0, 1)),
        (REFERENCE_SWAPPED, one_loop_map(1, 0, 1)),
        (REFERENCE_FLIPPED, one_loop_map(1, 0, -1)),
        (REFERENCE_SHIFTED, one_loop_map(1, -1, 1)),
    ];
    let mut sectors = BTreeMap::new();
    for (id, map) in routes {
        sectors.insert((id, 0b11), PSectorFate::Owner { class, map });
        for support in 0b00..0b11 {
            sectors.insert((id, support), PSectorFate::Zero);
        }
    }
    census.sectors = sectors;
    builder.set_census(census)?;
    builder.add_owner_with_program(
        POwnerSpec {
            class,
            lane: OwnerLane::SeriesParallel,
            presentation: REFERENCE_TOP,
            node: None,
            roles: LaneRoles::None,
            edge_maps: Vec::new(),
            payload: None,
            custom: None,
            terminals: vec![PTerminalId(0)],
        },
        OwnerProgram::Lane(Arc::new(FakeSpLane)),
    )?;
    if let Some(name) = revision {
        builder.set_revision(name)?;
    }
    builder.build().map(Arc::new)
}

// ----- t162-export: a toy BOXED node behind a library (consumer tests) -----

/// The kite's lines in its table (`q.q = +1`), slots `D1..D5`.
const BOXED_KITE_LINES: [&str; 5] = ["(k1)^2", "(-k1-k2)^2", "(k2)^2", "(k1+k2-q)^2", "(-k2+q)^2"];
/// The kite's graph for `table_import::canonical_node_graph` (`s = 2`, `t = 3`).
pub const BOXED_KITE_GRAPH: &str = "0-1 0-2 0-3 1-2 1-3";
/// Presentation of the boxed kite's Custom owner in [`boxed_kite_library`].
pub const BOXED_KITE_OWNER: PresentationId = PresentationId(2);

fn boxed_kite_key(powers: [i64; 5]) -> String {
    let parts: Vec<String> = powers.iter().map(i64::to_string).collect();
    format!("I({})", parts.join(","))
}

/// A boxed `table_dump.v1` table of the two-loop kite `KITE` (5 lines, no
/// ISP): box `D <= 1` (6 keys), the corner `I(1,1,1,1,1)` a boxed terminal
/// ("unknown integral"), one point rule per dotted key
/// `I(1 + e_i) = (d-4)/(d-3) I(corner) + (2d-7) I(corner - e_{i+1})`.
/// The table of rustred's boxed-node import tests, for consumers.
#[doc(hidden)]
pub fn boxed_kite_table() -> String {
    boxed_kite_table_with_box(&[[0, 0], [1, 0]], 6)
}

/// vk5l WP1: [`boxed_kite_table`] with the box `layers` (`box_keys` keys);
/// with `[[0, 0]]` (the corner alone) the five point rules target keys
/// outside the box, which only a certified key set
/// ([`boxed_kite_certified_keys`]) admits.
#[doc(hidden)]
pub fn boxed_kite_table_with_box(layers: &[[u32; 2]], box_keys: u64) -> String {
    use super::table_import::{BOX_MEASURE, BOXED_NODE_SCHEMA};
    let mut source =
        String::from("I(\n  name(KITE),\n  loops(k1,k2),\n  externals(q),\n  dimension(d),\n");
    for (i, momentum) in BOXED_KITE_LINES.iter().enumerate() {
        source.push_str(&format!("  prop(D{},{momentum},1),\n", i + 1));
    }
    source.push_str("  gram(q,q,1)\n)\n");
    let header = serde_json::json!({
        "type": "header", "schema": "table_dump.v1", "family": "KITE",
        "family_file": "KITE.sym", "family_source": source,
        "power_shifts": ["0", "0", "0", "0", "0"], "mask": "11111",
        "order_name": "legacy", "order_is_program": false,
        "order": {"support_priority": [0, 1, 2, 3, 4], "coordinate_priority": [0, 1, 2, 3, 4], "two_sided": []},
        "charts": ["11111"],
        "boxed_node": {
            "schema": BOXED_NODE_SCHEMA, "node": "KITE", "chart": "11111",
            "box": {"name": if layers.len() == 1 { "d0" } else { "d1" }, "measure": BOX_MEASURE, "layers": layers, "cap": null},
            "box_keys": box_keys,
        },
    });
    let mut lines = vec![
        header,
        serde_json::json!({"type": "zero_sectors", "scope": "downset", "count": 0, "masks": []}),
        serde_json::json!({"type": "chart", "chart": "11111", "zero": false}),
    ];
    for i in 0..5 {
        let mut target = [1_i64; 5];
        target[i] = 2;
        let mut lower = [1_i64; 5];
        lower[(i + 1) % 5] = 0;
        lines.push(serde_json::json!({
            "type": "rule", "chart": "11111", "index": i,
            "case": format!("Coordinate(CoordinateCase {{ fixed: {target:?} }})"),
            "fixed": target, "equations": [], "target": boxed_kite_key(target),
            "rhs": [[boxed_kite_key([1; 5]), "(d-4)/(d-3)"], [boxed_kite_key(lower), "2*d-7"]],
            "exceptions": [], "dispatch": "Partition", "sym_rows": 0,
        }));
    }
    lines.push(
        serde_json::json!({"type": "residual", "chart": "11111", "key": boxed_kite_key([1; 5]),
        "boxed_terminal": true, "status": "unknown integral"}),
    );
    lines.push(
        serde_json::json!({"type": "chart_end", "chart": "11111", "deferred_cases": 0,
        "deferred_branches": 0, "deferred_regions": 0}),
    );
    lines.push(serde_json::json!({"type": "end", "chart_errors": 0, "state": "complete"}));
    lines.iter().map(|line| format!("{line}\n")).collect()
}

/// A unimodular map `source -> target` (entries in {-1, 0, 1}) under which
/// the lines of `source` are those of `target` up to sign, one to one.
fn line_matching_map(source: &Presentation, target: &Presentation) -> Option<SignedAffineLoopMap> {
    let lines = |presentation: &Presentation| -> Vec<Vec<i64>> {
        (presentation.slots.iter())
            .filter_map(|slot| match slot {
                Slot::Line(line) => Some(line.momentum.clone()),
                Slot::Isp(_) => None,
            })
            .collect()
    };
    let (from, to) = (lines(source), lines(target));
    let loops = usize::from(source.loops);
    if source.loops != target.loops || from.len() != to.len() {
        return None;
    }
    let entries = loops * loops + loops;
    for code in 0..3_usize.pow(entries as u32) * 2 {
        let mut digits = code;
        let mut next = || {
            let digit = (digits % 3) as i64 - 1;
            digits /= 3;
            digit
        };
        let linear: Vec<i64> = (0..loops * loops).map(|_| next()).collect();
        let external: Vec<i64> = (0..loops).map(|_| next()).collect();
        let map = SignedAffineLoopMap {
            loops_in: source.loops,
            loops_out: target.loops,
            loop_linear: linear.into(),
            loop_external: external.into(),
            external_sign: if digits % 2 == 0 { 1 } else { -1 },
        };
        if !map.is_unimodular() {
            continue;
        }
        let mut used = vec![false; to.len()];
        let all = from.iter().all(|line| {
            let Ok(image) = map.map_momentum(line) else {
                return false;
            };
            let negated: Vec<i64> = image.iter().map(|c| -c).collect();
            match (to.iter().enumerate())
                .find(|&(slot, row)| !used[slot] && (*row == image || *row == negated))
            {
                Some((slot, _)) => {
                    used[slot] = true;
                    true
                }
                None => false,
            }
        });
        if all {
            return Some(map);
        }
    }
    None
}

/// A toy library of scope 1..=2 with a BOXED node (t162-export): the tops
/// `P1L` (presentation 0) and `P2L` (1) of vakint's families, the payload of
/// [`boxed_kite_table`] as the Custom owner of P2L's full sector on its own
/// presentation [`BOXED_KITE_OWNER`] (routed from the top by a loop map), no
/// basis map; terminals `Unit` and the kite's boxed terminal (the corner).
/// TOY: only P2L's full sector is routed (a dotted key reduces onto a
/// subsector key, which is `MissingOwner` here) and P1L has no owner; it
/// exists to test a consumer on boxed terminals and `OutsideBox`.
#[doc(hidden)]
pub fn boxed_kite_library() -> Result<Arc<PLibrary>, PError> {
    boxed_kite_library_with(&boxed_kite_table(), None)
}

/// vk5l WP1: the certified key set of the kite with box `d0`: the corner
/// and the five dotted keys (its forward closure on the chart), table frame.
#[doc(hidden)]
pub fn boxed_kite_certified_keys() -> super::certified_keys::CertifiedKeySet {
    let mut keys = vec![vec![1_i64; 5]];
    for i in 0..5 {
        let mut key = vec![1_i64; 5];
        key[i] = 2;
        keys.push(key);
    }
    super::certified_keys::CertifiedKeySet::new("KITE", 5, "table", keys)
        .expect("a valid certified set")
}

/// vk5l WP1: [`boxed_kite_library`] with box `d0` and the certified key set
/// [`boxed_kite_certified_keys`] (the dotted keys lie outside the box but in
/// the set).
#[doc(hidden)]
pub fn boxed_kite_library_certified() -> Result<Arc<PLibrary>, PError> {
    boxed_kite_library_with(
        &boxed_kite_table_with_box(&[[0, 0]], 1),
        Some(boxed_kite_certified_keys()),
    )
}

fn boxed_kite_library_with(
    table: &str,
    certified_keys: Option<super::certified_keys::CertifiedKeySet>,
) -> Result<Arc<PLibrary>, PError> {
    use super::custom::inspect_custom_payload;
    use super::library::PTerminal;
    use super::programs::CustomPayloadRef;
    use super::table_import::{TableImportOptions, canonical_node_graph, import_table};
    let context = CoefficientContext::try_new(["d"]).map_err(|_| PError::ContextMismatch)?;
    let file: PFamiliesFile = serde_json::from_str(P_FAMILIES_V1_JSON)
        .map_err(|error| PError::MalformedLibrary(format!("p families: {error}")))?;
    let top = |name: &str, id: u32| -> Result<Presentation, PError> {
        file.families
            .iter()
            .find(|family| family.name == name)
            .ok_or_else(|| PError::MalformedLibrary(format!("no family {name}")))?
            .presentation(PresentationId(id), &context)
    };
    let (p1l, p2l) = (top("P1L", 0)?, top("P2L", 1)?);
    let (_, graph) = canonical_node_graph(BOXED_KITE_GRAPH, 2, 3)?;
    let imported = import_table(
        table,
        &graph,
        &TableImportOptions {
            id: BOXED_KITE_OWNER,
            node: None,
            lp_count: 1,
            census_family: None,
            order_map: None,
            partial_box: false,
            mixed_box: false,
            certified_keys,
        },
    )?;
    let route = line_matching_map(&p2l, &imported.presentation).ok_or_else(|| {
        PError::MalformedLibrary("the kite payload does not route from P2L".to_owned())
    })?;
    let full = (1_u64 << imported.presentation.slots.len()) - 1;
    let mut census = PCensus::default();
    let class = census.insert_class(PClassRecord {
        key: OwnerClassKey {
            loops: 2,
            lines: 5,
            eps_lines: 0,
            canonical: Arc::from(&b"rustred boxed kite toy"[..]),
        },
        owner: (BOXED_KITE_OWNER, full),
        node: None,
        nodes: Vec::new(),
        t: 5,
        eps_lines: 0,
    })?;
    census.sectors.insert(
        (PresentationId(1), 0b11111),
        PSectorFate::Owner { class, map: route },
    );
    census.sectors.insert(
        (BOXED_KITE_OWNER, full),
        PSectorFate::Owner {
            class,
            map: super::route_choice::identity_map(2),
        },
    );
    let residuals = inspect_custom_payload(&imported.payload)?.residual_keys();
    let mut builder = PLibrary::builder(context, 1..=2);
    builder.add_presentation(p1l)?;
    builder.add_presentation(p2l)?;
    builder.add_presentation(imported.presentation)?;
    builder.set_census(census)?;
    let mut terminals = Vec::new();
    for key in residuals {
        terminals.push(builder.add_terminal(PTerminal::Residual {
            owner: class,
            key: crate::family::IntegralKey::try_new(key).map_err(|error| {
                PError::MalformedLibrary(format!("boxed kite terminal: {error}"))
            })?,
        })?);
    }
    builder.add_owner(POwnerSpec {
        class,
        lane: OwnerLane::Custom,
        presentation: BOXED_KITE_OWNER,
        node: None,
        roles: LaneRoles::None,
        edge_maps: Vec::new(),
        payload: Some(CustomPayloadRef::Bytes(Arc::from(imported.payload))),
        custom: Some(imported.meta),
        terminals,
    })?;
    builder.build().map(Arc::new)
}

#[cfg(test)]
mod boxed_kite_tests {
    use super::*;
    use crate::family::IntegralKey;
    use crate::propagator::library::{PBasis, PReductionRequest, PTerminal, PUncoveredPolicy};
    use crate::reduction::{OrderedSchedulerOptions, OrderedWeightArithmetic, ReductionLimits};

    fn reduce(
        library: &PLibrary,
        key: [i64; 5],
    ) -> Result<super::super::library::PReduction, PError> {
        library.reduce(PReductionRequest {
            inputs: vec![(
                PresentationId(1),
                IntegralKey::try_new(key).unwrap(),
                library.coefficient_context().one(),
            )],
            arithmetic: OrderedWeightArithmetic::Exact,
            scheduler: OrderedSchedulerOptions {
                threads: 1,
                ..Default::default()
            },
            step_cache: None,
            uncovered: PUncoveredPolicy::Reject,
            onto_masters: true,
            limits: ReductionLimits::default(),
            inspector: None,
        })
    }

    /// The toy boxed library: the corner of P2L is the boxed terminal (a
    /// residual of the kite's class, coefficient 1), a key of the chart
    /// outside the box is `OutsideBox` naming the node.
    #[test]
    fn the_boxed_kite_library_returns_its_terminal_and_refuses_outside_its_box() {
        let library = boxed_kite_library().unwrap();
        assert_eq!(library.scope(), 1..=2);
        assert_eq!(library.terminals().len(), 2);
        assert!(matches!(library.terminals()[1], PTerminal::Residual { .. }));
        let corner = reduce(&library, [1; 5]).unwrap();
        assert_eq!(corner.terms.len(), 1, "{:?}", corner.terms);
        assert_eq!(corner.terms[0].basis, PBasis::Residual(PTerminalId(1)));
        let named = library.residual_terminal(PTerminalId(1)).unwrap().unwrap();
        assert_eq!((named.node.as_str(), named.boxed), ("KITE", true));
        assert_eq!(named.key.powers(), [1; 5]);
        assert_eq!(library.residual_terminal(PTerminalId(0)).unwrap(), None);
        match reduce(&library, [3, 1, 1, 1, 1]) {
            Err(PError::OutsideBox { node, .. }) => assert_eq!(node, "KITE"),
            other => panic!("expected OutsideBox, got {other:?}"),
        }
    }

    fn request_on(
        library: &PLibrary,
        presentation: PresentationId,
        key: [i64; 5],
        uncovered: PUncoveredPolicy,
    ) -> Result<super::super::library::PReduction, PError> {
        library.reduce(PReductionRequest {
            inputs: vec![(
                presentation,
                IntegralKey::try_new(key).unwrap(),
                library.coefficient_context().one(),
            )],
            arithmetic: OrderedWeightArithmetic::Exact,
            scheduler: OrderedSchedulerOptions {
                threads: 1,
                ..Default::default()
            },
            step_cache: None,
            uncovered,
            onto_masters: true,
            limits: ReductionLimits::default(),
            inspector: None,
        })
    }

    /// vk5l WP1 (a): with a certified set, a key of the chart outside it is
    /// `OutsideCertifiedSet` at dispatch, naming node, key, the violated box
    /// bound, the set's size and the original request with its reproduction
    /// line; the corner (in the set) still reduces.
    #[test]
    fn a_key_outside_the_certified_set_is_refused_at_dispatch_with_its_bound_and_request() {
        use crate::propagator::error::BoxViolation;
        let library = boxed_kite_library_certified().unwrap();
        library.set_source("/packages/kite");
        let corner = reduce(&library, [1; 5]).unwrap();
        assert_eq!(corner.terms.len(), 1);
        let error = reduce(&library, [3, 1, 1, 1, 1]).unwrap_err();
        match &error {
            PError::OutsideCertifiedSet {
                node,
                key,
                violated,
                certified_keys,
                request,
                ..
            } => {
                assert_eq!(node, "KITE");
                assert_eq!(*certified_keys, 6);
                assert_eq!(key.powers().iter().sum::<i64>(), 7);
                assert_eq!(
                    *violated,
                    Some(BoxViolation::Dots {
                        dots: 2,
                        layers: vec![0]
                    })
                );
                let request = request.as_ref().expect("the request is recorded");
                assert_eq!(request.inputs.len(), 1);
                assert_eq!(request.inputs[0].powers, [3, 1, 1, 1, 1]);
                assert_eq!(request.inputs[0].presentation, PresentationId(1));
                assert!(
                    request
                        .reproduce
                        .contains("p5l_library -- reduce '/packages/kite' p1:3,1,1,1,1")
                );
            }
            other => panic!("expected OutsideCertifiedSet, got {other:?}"),
        }
        let text = error.to_string();
        for part in [
            "boxed node KITE",
            "D = 2 is no layer",
            "certified set (6 keys)",
            "reproduce: cargo run",
        ] {
            assert!(text.contains(part), "{part:?} not in {text}");
        }
        assert_eq!(error.request().map(|r| r.inputs.len()), Some(1));
    }

    /// vk5l WP1 (b): the early check: a request input on the boxed owner's
    /// own sector outside its USER box is refused before any step, naming the
    /// violated bound, even when the key is certified (decision Q7: both
    /// checks); inside the box it reduces.
    #[test]
    fn a_direct_input_outside_the_user_box_is_refused_early_with_its_bound() {
        use crate::propagator::error::BoxViolation;
        let library = boxed_kite_library_certified().unwrap();
        match request_on(
            &library,
            BOXED_KITE_OWNER,
            [1, 2, 1, 1, 1],
            PUncoveredPolicy::Reject,
        ) {
            Err(PError::OutsideBox {
                node,
                violated,
                request,
                ..
            }) => {
                assert_eq!(node, "KITE");
                assert_eq!(
                    violated,
                    Some(BoxViolation::Dots {
                        dots: 1,
                        layers: vec![0]
                    })
                );
                assert_eq!(request.unwrap().inputs[0].powers, [1, 2, 1, 1, 1]);
            }
            other => panic!("expected an early OutsideBox, got {other:?}"),
        }
        let plain = boxed_kite_library().unwrap();
        match request_on(
            &plain,
            BOXED_KITE_OWNER,
            [1, 1, 4, 1, 1],
            PUncoveredPolicy::Reject,
        ) {
            Err(error @ PError::OutsideBox { .. }) => {
                assert!(
                    error
                        .to_string()
                        .contains("D = 3 is no layer of the box (D in [0, 1])"),
                    "{error}"
                );
            }
            other => panic!("expected an early OutsideBox, got {other:?}"),
        }
        let corner =
            request_on(&plain, BOXED_KITE_OWNER, [1; 5], PUncoveredPolicy::Reject).unwrap();
        assert_eq!(corner.terms.len(), 1);
    }

    /// vk5l WP1 (d): Retain keeps a refused key as a residual terminal named
    /// by its node, with the refusal; the default (Reject) is unchanged.
    #[test]
    fn retain_keeps_a_refused_boxed_key_as_a_named_residual() {
        let library = boxed_kite_library().unwrap();
        assert!(matches!(
            request_on(
                &library,
                PresentationId(1),
                [3, 1, 1, 1, 1],
                PUncoveredPolicy::default()
            ),
            Err(PError::OutsideBox { .. })
        ));
        let kept = request_on(
            &library,
            PresentationId(1),
            [3, 1, 1, 1, 1],
            PUncoveredPolicy::Retain,
        )
        .unwrap();
        assert_eq!(kept.terms.len(), 1, "{:?}", kept.terms);
        let PBasis::Residual(id) = kept.terms[0].basis else {
            panic!("a residual, got {:?}", kept.terms[0].basis);
        };
        assert!(id.0 as usize >= library.terminals().len());
        let named = library.residual_terminal(id).unwrap().unwrap();
        assert_eq!((named.node.as_str(), named.boxed), ("KITE", true));
        assert_eq!(named.key.powers().iter().sum::<i64>(), 7);
        assert!(matches!(named.retained, Some(PError::OutsideBox { .. })));
        // The same key again: the same id; the corner is still the library's terminal.
        let again = request_on(
            &library,
            PresentationId(1),
            [3, 1, 1, 1, 1],
            PUncoveredPolicy::Retain,
        )
        .unwrap();
        assert_eq!(again.terms[0].basis, PBasis::Residual(id));
        let corner = request_on(
            &library,
            PresentationId(1),
            [1; 5],
            PUncoveredPolicy::Retain,
        )
        .unwrap();
        assert_eq!(corner.terms[0].basis, PBasis::Residual(PTerminalId(1)));
        let certified = boxed_kite_library_certified().unwrap();
        let kept = request_on(
            &certified,
            PresentationId(1),
            [3, 1, 1, 1, 1],
            PUncoveredPolicy::Retain,
        )
        .unwrap();
        let PBasis::Residual(id) = kept.terms[0].basis else {
            panic!("a residual");
        };
        assert!(matches!(
            certified.residual_terminal(id).unwrap().unwrap().retained,
            Some(PError::OutsideCertifiedSet { .. })
        ));
    }
}
