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
    builder.build().map(Arc::new)
}
