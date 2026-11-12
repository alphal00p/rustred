//! The descent certificate of a p library (owner: M2-T8a rr-assemble).
//!
//! [`check`] is the load-time certificate of architecture section 5.5, on the
//! structure of a library:
//!
//! 1. Custom owners: the payload is generated on the owner sector; its order is
//!    support-primary or rank-gated (M5 B2: pre-support rows only on numerator
//!    slots outside the support, [`is_rank_gated`]) for an owner without forced
//!    axes; for an eps owner the persisted order program is two-sided exactly on
//!    the forced axes (plan M2 amendment 2026-11-06: every two-sided order is not
//!    support-primary).
//! 2. Custom rules are checked per RHS term through the owner's `order_key`
//!    (D-T6-1) on a witness box of keys of every raw chart
//!    ([`custom_witness_keys`]): every child of a step is a
//!    [`DescentWitness::SameSectorStrict`] Apply key or a
//!    [`DescentWitness::Subsector`] Route key, every terminal a residual of
//!    the owner.
//! 3. Every census route is verified and compiled into its transport, and maps
//!    `t` lines with their eps lines onto an owner of the same `t`; every edge
//!    map is verified and belongs to exactly one owner.
//! 4. SeriesParallel, Insertion and Carpet owners end in the trivial node at one
//!    loop and otherwise have exactly one rule edge into a presentation of `L - 1`
//!    loops and fewer lines; a Triangle or Diamond owner has no edge and lowers
//!    integer lines of its own support only.
//! 5. Overlays: none in M2.
//! 6. Eps axes are forced into every support: census sectors and owner sectors.
//! 7. The hard false knock-out guard on Custom owners ([`check_custom_guard`]).
//!
//! Every owner's program is built on the way (`lane_program`): a class without a
//! lane program fails the certificate. Items 1 and 2 read the Custom payloads
//! (M2-T7c, `custom::inspect_custom_payload` and the owner's `CustomStep`).
//! Item 2 is checked on instances, not on the symbolic rules: the generated
//! rules carry their case (face and equations) as data, and the step's own
//! dispatch decides which rule applies to a key, so the box steps keys through
//! the program the engine runs. Every chart's corner is in the box.
//!
//! [`trace`] is the dynamic side, `rustred p-validate`: it follows keys through
//! the census, the routes and the owner programs without coefficients and counts
//! what a reduction under `PUncoveredPolicy::Reject` would refuse: uncovered keys,
//! missing owners and steps that do not strictly descend in the Ordered measure
//! (phase `t` descending, Route before Apply, then node rank and owner order).
//! It does not share code with the engine on purpose: it is a second reading of
//! the same contract.
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use crate::family::IntegralKey;
use crate::reduction::ReductionLimits;
use crate::sector::symmetry::integral_transport::{self, ExpansionLimits, Prepared};
use crate::sector::symmetry::{self, CoefficientMatrix, MomentumMap};
use crate::sector::{Mask, OrderingPolicy};

use super::census::{PSectorFate, SignedAffineLoopMap};
use super::custom::{CustomPayloadSummary, inspect_custom_payload};
use super::error::PError;
use super::lane::{LaneRoles, OwnerLane};
use super::library::{OwnerProgram, PLibrary, POwner, POwnerSpec, PTerminal};
use super::presentation::{EdgeKind, Presentation, PresentationKind, Slot, verify_edge_map};
use super::step::{PKeyView, PLaneStep, POrderKey, PStage, PStepContext};
use super::tag::GammaTag;
use super::{OwnerClassId, PresentationId};

/// What [`check`] verified.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DescentReport {
    /// Owners with a lane program, by lane.
    pub owners: BTreeMap<OwnerLane, usize>,
    /// Census sectors proved zero.
    pub zero_sectors: usize,
    /// Census sectors that are the sector of their class's owner.
    pub owner_sectors: usize,
    /// Census routes verified and compiled (owner sectors excluded).
    pub routes: usize,
    /// Edge maps verified.
    pub edge_maps: usize,
    /// Custom owners that passed the hard guard.
    pub guarded_owners: usize,
    /// Rules of the Custom payloads.
    pub custom_rules: usize,
    /// Keys of the Custom witness boxes stepped (item 2).
    pub witness_keys: usize,
    /// The children of those steps, by witness.
    pub witnesses: BTreeMap<DescentWitness, usize>,
    /// Overlay rules (none in M2).
    pub overlays: usize,
}

/// Why a child of a Custom step is below its key (AD section 5.5 item 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DescentWitness {
    /// An Apply key of the same owner, strictly lower in its `order_key`.
    SameSectorStrict,
    /// A Route key on a strict subsector of the owner support: fewer lines,
    /// a later phase.
    Subsector,
}

/// Total dots over the raw chart's slots of a Custom witness key.
pub const CUSTOM_WITNESS_DOTS: u32 = 2;
/// Total numerator rank over the other slots (eps axes off the chart at
/// `n <= 0`, ISPs) of a Custom witness key.
pub const CUSTOM_WITNESS_RANK: u32 = 2;

fn malformed(detail: impl Into<String>) -> PError {
    PError::MalformedLibrary(detail.into())
}

/// Line slots of `p` as a mask.
fn line_mask(p: &Presentation) -> u64 {
    p.lines().fold(0, |mask, (slot, _)| mask | 1 << slot)
}

/// `raw | forced` of `powers` on `p`.
fn support_of(p: &Presentation, powers: &[i64]) -> u64 {
    powers
        .iter()
        .enumerate()
        .filter(|&(_, &power)| power > 0)
        .fold(p.forced_axes(), |mask, (slot, _)| mask | 1 << slot)
}

/// The owner sector of `owner`'s class.
fn owner_sector(library: &PLibrary, owner: &POwnerSpec) -> Result<(PresentationId, u64), PError> {
    library
        .census()
        .classes
        .get(&owner.class)
        .map(|record| record.owner)
        .ok_or_else(|| malformed(format!("owner {:016x} outside the census", owner.class.0)))
}

/// The hard false knock-out guard of a Custom owner (plan M2 section 3.4, AD
/// section 5.5 item 7): `lp_count >= 1 => residuals >= 1`; its residual
/// terminals are `residuals` many; the two-sided axes of its program are exactly
/// the forced axes of its sector; its charts are exactly the `2^j` raw charts of
/// its support. Shared by [`check`], `lane_program` and the loader.
pub fn check_custom_guard(library: &PLibrary, owner: &POwnerSpec) -> Result<(), PError> {
    let label = format!("Custom owner {:016x}", owner.class.0);
    if owner.lane != OwnerLane::Custom {
        return Err(malformed(format!("{label} has the lane {:?}", owner.lane)));
    }
    let (Some(meta), Some(_)) = (&owner.custom, &owner.payload) else {
        return Err(malformed(format!("{label} has no payload or no meta")));
    };
    if meta.lp_count >= 1 && meta.residuals == 0 {
        return Err(malformed(format!(
            "false knock-out guard: {label} has LP = {} and 0 residuals",
            meta.lp_count
        )));
    }
    let residuals = owner
        .terminals
        .iter()
        .filter(|terminal| {
            matches!(
                library.terminals().get(terminal.0 as usize),
                Some(PTerminal::Residual { owner: class, .. }) if *class == owner.class
            )
        })
        .count();
    if residuals != meta.residuals as usize {
        return Err(malformed(format!(
            "{label} records {} residuals and names {residuals} residual terminals",
            meta.residuals
        )));
    }
    let (presentation, support) = owner_sector(library, owner)?;
    if presentation != owner.presentation {
        return Err(malformed(format!(
            "{label} is on presentation {}, its class on {}",
            owner.presentation.0, presentation.0
        )));
    }
    let forced = library.presentation(presentation).forced_axes() & support;
    if meta.two_sided_axes != forced {
        return Err(malformed(format!(
            "{label}: two-sided axes {:#x}, forced axes {forced:#x}",
            meta.two_sided_axes
        )));
    }
    let integer = support & !forced;
    let mut expected: Vec<u64> = (0..1_u64 << forced.count_ones())
        .map(|bits| integer | deposit(bits, forced))
        .collect();
    expected.sort_unstable();
    let mut charts = meta.charts.clone();
    charts.sort_unstable();
    if charts != expected {
        return Err(malformed(format!(
            "{label}: charts {charts:#x?}, the raw charts of support {support:#x} are {expected:#x?}"
        )));
    }
    Ok(())
}

/// The bits of `bits` placed on the set bits of `mask`, lowest first.
fn deposit(bits: u64, mask: u64) -> u64 {
    let mut out = 0;
    let mut rest = mask;
    let mut index = 0;
    while rest != 0 {
        let bit = rest & rest.wrapping_neg();
        if bits >> index & 1 == 1 {
            out |= bit;
        }
        rest &= rest - 1;
        index += 1;
    }
    out
}

/// The route of a census sector as an exact, verified transport onto its owner.
fn compile_route(
    source: &Presentation,
    target: &Presentation,
    map: &SignedAffineLoopMap,
    source_support: u64,
    target_support: u64,
) -> Result<Prepared, PError> {
    let context = source.family.coefficient_context();
    let (loops_in, loops_out) = (usize::from(map.loops_in), usize::from(map.loops_out));
    let refused = |error: symmetry::Error| {
        malformed(format!(
            "the route of sector {source_support:#x} of presentation {}: {error}",
            source.id.0
        ))
    };
    let matrix = |rows, columns, values: &[i64]| {
        CoefficientMatrix::try_new(
            rows,
            columns,
            values.iter().map(|&value| context.integer(value)),
        )
        .map_err(refused)
    };
    let momentum = MomentumMap::new(
        matrix(loops_in, loops_out, &map.loop_linear)?,
        matrix(loops_in, 1, &map.loop_external)?,
        matrix(1, 1, &[i64::from(map.external_sign)])?,
    );
    let verified = symmetry::verify(
        &source.family,
        &target.family,
        momentum,
        symmetry::Limits::default(),
    )
    .map_err(refused)?;
    let root = |presentation: &Presentation, support: u64| {
        Mask::try_new((0..presentation.slots.len()).map(|slot| support >> slot & 1 == 1))
            .map_err(|error| malformed(error.to_string()))
    };
    integral_transport::compile(
        &source.family,
        Arc::clone(&target.family),
        Arc::new(verified),
        root(source, source_support)?,
        root(target, target_support)?,
        ExpansionLimits::default(),
    )
    .map_err(|error| {
        malformed(format!(
            "the route of sector {source_support:#x} of presentation {} does not transport: {error:?}",
            source.id.0
        ))
    })
}

/// The load-time descent certificate of `library`, see the module docs.
pub fn check(library: &PLibrary) -> Result<DescentReport, PError> {
    let mut report = DescentReport::default();
    let census = library.census();
    let presentations = library.presentations();
    let presentation = |id: PresentationId| {
        presentations
            .get(id.0 as usize)
            .ok_or_else(|| malformed(format!("unknown presentation {}", id.0)))
    };

    // Items 3 and 6: sectors and routes.
    for (&(id, support), fate) in &census.sectors {
        let source = presentation(id)?;
        let forced = source.forced_axes();
        if support & forced != forced {
            return Err(malformed(format!(
                "sector {support:#x} of presentation {} drops forced axes {forced:#x}",
                id.0
            )));
        }
        let PSectorFate::Owner { class, map } = fate else {
            report.zero_sectors += 1;
            continue;
        };
        let record = census
            .classes
            .get(class)
            .ok_or_else(|| malformed(format!("a route to the unknown class {:016x}", class.0)))?;
        let lines = (support & line_mask(source)).count_ones();
        if lines != u32::from(record.t) || forced.count_ones() != u32::from(record.eps_lines) {
            return Err(malformed(format!(
                "sector {support:#x} of presentation {} has {lines} lines and {} eps lines, \
                 its class {} has {} and {}",
                id.0,
                forced.count_ones(),
                record.key.label(),
                record.t,
                record.eps_lines
            )));
        }
        if record.owner == (id, support) {
            report.owner_sectors += 1;
            continue;
        }
        let (owner, owner_support) = record.owner;
        compile_route(source, presentation(owner)?, map, support, owner_support)?;
        report.routes += 1;
    }

    // Items 3 and 4: edge maps, each named by one owner.
    let mut named = vec![0_usize; library.edge_maps().len()];
    for owner in library.owners() {
        for &edge in &owner.spec.edge_maps {
            *named
                .get_mut(edge)
                .ok_or_else(|| malformed(format!("owner names edge map {edge}")))? += 1;
        }
    }
    for (index, map) in library.edge_maps().iter().enumerate() {
        if named[index] != 1 {
            return Err(malformed(format!(
                "edge map {index} is named by {} owners",
                named[index]
            )));
        }
        verify_edge_map(presentation(map.parent)?, presentation(map.child)?, map)?;
        report.edge_maps += 1;
    }

    // Items 1, 2, 4, 6, 7: owners and their programs.
    for owner in library.owners() {
        let spec = &owner.spec;
        let (id, support) = owner_sector(library, spec)?;
        let parent = presentation(id)?;
        let forced = parent.forced_axes();
        if id != spec.presentation || support & forced != forced {
            return Err(malformed(format!(
                "owner {:016x} on presentation {}, its sector ({}, {support:#x})",
                spec.class.0, spec.presentation.0, id.0
            )));
        }
        let t = (support & line_mask(parent)).count_ones() as usize;
        let mut summary = None;
        match spec.lane {
            OwnerLane::SeriesParallel | OwnerLane::Insertion | OwnerLane::Carpet => {
                check_rule_owner(library, spec, parent, t)?;
            }
            OwnerLane::Triangle => check_triangle_owner(spec, parent, support)?,
            OwnerLane::Diamond => check_diamond_owner(spec, parent, support)?,
            OwnerLane::Scheme => {
                if !spec.edge_maps.is_empty() || spec.terminals.len() > 1 {
                    return Err(malformed(format!(
                        "Scheme owner {:016x} with {} edge maps and {} terminals",
                        spec.class.0,
                        spec.edge_maps.len(),
                        spec.terminals.len()
                    )));
                }
                // M5 no6: the Custom fallback (payload) of a Scheme owner:
                // its guard, its residual (= the owner's corner) and its
                // transport are checked by the build; here the item-2
                // witness box: every child of a fallback step is strictly
                // lower in the Custom order on the fallback's sector, or on
                // a strict subsector of it. The Scheme step closes the
                // fallback's sector locally, so it never returns a key of
                // its own sector: scheme and fallback cannot cycle.
                if let Some(fallback) = super::scheme::scheme_fallback_program(library, spec)? {
                    check_fallback_witnesses(library, spec, &fallback, &mut report)?;
                }
            }
            OwnerLane::Custom => {
                check_custom_guard(library, spec)?;
                report.guarded_owners += 1;
                summary = Some(check_custom_order(spec, parent, support)?);
            }
            lane => {
                return Err(malformed(format!(
                    "owner {:016x} has the lane {lane:?}",
                    spec.class.0
                )));
            }
        }
        let step = match owner.program(library)? {
            OwnerProgram::Lane(step) => step,
            OwnerProgram::Missing { ledger } => {
                return Err(PError::MissingOwner {
                    class: spec.class,
                    ledger: *ledger,
                });
            }
        };
        *report.owners.entry(spec.lane).or_default() += 1;
        if let Some(summary) = summary {
            report.custom_rules += summary.rules();
            check_custom_witnesses(library, owner, step.as_ref(), support, &mut report)?;
        }
    }
    Ok(report)
}

/// Item 1 for a Custom owner: its payload is generated on the owner sector,
/// with a support-primary order and no two-sided axis when the sector has no
/// forced axis, else with an order program two-sided exactly on the forced
/// axes. Returns the payload's summary.
fn check_custom_order(
    spec: &POwnerSpec,
    parent: &Presentation,
    support: u64,
) -> Result<CustomPayloadSummary, PError> {
    let fail = |detail: String| {
        malformed(format!(
            "Custom owner {:016x} on presentation {}: {detail}",
            spec.class.0, parent.id.0
        ))
    };
    let payload = spec
        .payload
        .as_ref()
        .ok_or_else(|| fail("no payload".to_owned()))?;
    let summary = inspect_custom_payload(&payload.bytes()?)?;
    if summary.support != support || summary.arity != parent.slots.len() {
        return Err(fail(format!(
            "the payload is generated on support {:#x} of arity {}, the owner sector is {support:#x} of arity {}",
            summary.support,
            summary.arity,
            parent.slots.len()
        )));
    }
    let policy = OrderingPolicy::try_from_stable_id(&summary.ordering)
        .map_err(|error| fail(format!("the payload's order {}: {error}", summary.ordering)))?;
    let program_axes = policy.program().map_or(0, |program| {
        (program.two_sided().iter().enumerate())
            .filter(|&(_, &two_sided)| two_sided)
            .fold(0_u64, |mask, (axis, _)| mask | 1 << axis)
    });
    let forced = parent.forced_axes() & support;
    if summary.two_sided_axes != forced || program_axes != forced {
        return Err(fail(format!(
            "the order is two-sided on {:#x} (program {program_axes:#x}), the forced axes are {forced:#x}",
            summary.two_sided_axes
        )));
    }
    if forced == 0 && !policy.is_support_primary() && !is_rank_gated(&policy, support) {
        return Err(fail(format!(
            "the order {} is neither support-primary nor rank-gated (pre-support rows only on \
             numerator slots outside the support)",
            summary.ordering
        )));
    }
    Ok(summary)
}

/// Whether `policy` is a rank-gated order of an owner on `support` (M5 B2,
/// the WP3 `globisp` recipe): one-sided, and every pre-support degree row
/// weighs only numerator powers (`inactive`) of slots outside `support`, so
/// it is zero on the support lines and on every positive power. Such a row
/// is the numerator rank on the owner's non-support slots: on the owner
/// support the order stays a well-order (the rows are nonnegative and the
/// support and degree rows follow), and a subsector child that moves a
/// support line into a numerator ties on the row and descends by support.
/// The p engine needs only same-support descent of Apply children and strict
/// subsectors for Route children, which item 2 checks per RHS term; sector-
/// first induction (the vacuum reducer's `is_support_primary` consumers) is
/// not used on the p path.
pub(super) fn is_rank_gated(policy: &OrderingPolicy, support: u64) -> bool {
    let Some(program) = policy.program() else {
        return false;
    };
    if program.has_two_sided() {
        return false;
    }
    let rows = &program.descriptor().pre_support_degree_rows;
    !rows.is_empty()
        && rows.iter().all(|row| {
            row.active.iter().all(|&weight| weight == 0)
                && (row.inactive.iter().enumerate())
                    .all(|(slot, &weight)| weight == 0 || support >> slot & 1 == 0)
        })
}

/// The keys of the witness box of a Custom owner with `support` and forced
/// axes `forced` on `arity` slots: on every raw chart, the chart's slots at
/// `1 + dots` with at most [`CUSTOM_WITNESS_DOTS`] dots in total and the other
/// slots at `-rank` with at most [`CUSTOM_WITNESS_RANK`] in total. Charts
/// ascending, keys in enumeration order.
pub fn custom_witness_keys(arity: usize, support: u64, forced: u64) -> Vec<Vec<i64>> {
    fn spread(slots: &[usize], total: u32, sign: i64, base: &[i64], out: &mut Vec<Vec<i64>>) {
        let Some((&slot, rest)) = slots.split_first() else {
            out.push(base.to_vec());
            return;
        };
        for amount in 0..=total {
            let mut powers = base.to_vec();
            powers[slot] += sign * i64::from(amount);
            spread(rest, total - amount, sign, &powers, out);
        }
    }
    let free = support & !forced;
    let mut charts: Vec<u64> = (0..1_u64 << forced.count_ones())
        .map(|bits| free | deposit(bits, forced))
        .collect();
    charts.sort_unstable();
    let mut keys = Vec::new();
    for chart in charts {
        let (positive, other): (Vec<usize>, Vec<usize>) =
            (0..arity).partition(|&slot| chart >> slot & 1 == 1);
        let corner: Vec<i64> = (0..arity)
            .map(|slot| i64::from(chart >> slot & 1 == 1))
            .collect();
        let mut dotted = Vec::new();
        spread(&positive, CUSTOM_WITNESS_DOTS, 1, &corner, &mut dotted);
        for base in dotted {
            spread(&other, CUSTOM_WITNESS_RANK, -1, &base, &mut keys);
        }
    }
    keys
}

/// Item 2 for a Custom owner: every key of its witness box is stepped; each
/// child is an Apply key of the owner support strictly lower in the owner's
/// `order_key`, or a Route key of the owner presentation on a strict subsector
/// of the support; each terminal is one of the owner's residuals.
fn check_custom_witnesses(
    library: &PLibrary,
    owner: &POwner,
    step: &dyn PLaneStep,
    support: u64,
    report: &mut DescentReport,
) -> Result<(), PError> {
    let spec = &owner.spec;
    let presentation = library.presentation(spec.presentation);
    let limits = ReductionLimits::default();
    let cx = PStepContext {
        library,
        owner,
        limits: &limits,
    };
    let fail = |key: &[i64], detail: String| {
        malformed(format!(
            "Custom owner {:016x}, key {key:?}: {detail}",
            spec.class.0
        ))
    };
    let tag = GammaTag::one();
    let keys = custom_witness_keys(
        presentation.slots.len(),
        support,
        presentation.forced_axes(),
    );
    for key in &keys {
        let order = step.order_key(&cx, key)?;
        let view = PKeyView {
            presentation: spec.presentation,
            powers: key,
            tag: &tag,
        };
        let output = step.step(&cx, &view)?;
        for child in &output.children {
            let child_support = support_of(presentation, &child.powers);
            if child.presentation != spec.presentation || child.tag != tag {
                return Err(fail(
                    key,
                    format!(
                        "a child on presentation {} with tag {:?}",
                        child.presentation.0, child.tag
                    ),
                ));
            }
            let witness = match child.stage {
                PStage::Apply
                    if child_support == support && step.order_key(&cx, &child.powers)? < order =>
                {
                    DescentWitness::SameSectorStrict
                }
                PStage::Route if child_support & !support == 0 && child_support != support => {
                    DescentWitness::Subsector
                }
                stage => {
                    return Err(fail(
                        key,
                        format!(
                            "the {stage:?} child {:?} on support {child_support:#x} does not descend",
                            child.powers
                        ),
                    ));
                }
            };
            *report.witnesses.entry(witness).or_default() += 1;
        }
        for (terminal, terminal_tag, _) in &output.terminals {
            let residual = matches!(
                library.terminals().get(terminal.0 as usize),
                Some(PTerminal::Residual { owner: class, .. }) if *class == spec.class
            );
            if !residual || !spec.terminals.contains(terminal) || *terminal_tag != tag {
                return Err(fail(key, format!("the terminal {}", terminal.0)));
            }
        }
    }
    report.witness_keys += keys.len();
    Ok(())
}

/// Item 2 on the Custom fallback of a Scheme owner (M5 no6), on the same
/// witness box as a Custom owner of the fallback's presentation and support.
fn check_fallback_witnesses(
    library: &PLibrary,
    spec: &POwnerSpec,
    fallback: &super::scheme::SchemeFallback,
    report: &mut DescentReport,
) -> Result<(), PError> {
    let (program, target, support) = fallback.program();
    let presentation = library.presentation(target);
    let limits = ReductionLimits::default();
    let residuals = program.residual_keys();
    let fail = |key: &[i64], detail: String| {
        malformed(format!(
            "Scheme owner {:016x}, fallback key {key:?}: {detail}",
            spec.class.0
        ))
    };
    let keys = custom_witness_keys(presentation.slots.len(), support, 0);
    for key in &keys {
        let order = program.order(key)?;
        match program.apply(spec.class, key, &limits)? {
            super::custom::PFallbackApplied::Residual => {
                if !residuals
                    .iter()
                    .any(|residual| residual.powers() == key.as_slice())
                {
                    return Err(fail(key, "a terminal that is no residual".to_owned()));
                }
            }
            super::custom::PFallbackApplied::Children { children, .. } => {
                for (child, _) in &children {
                    let child_support = support_of(presentation, child);
                    let witness = if child_support == support {
                        if program.order(child)? >= order {
                            return Err(fail(key, format!("the child {child:?} does not descend")));
                        }
                        DescentWitness::SameSectorStrict
                    } else if child_support & !support == 0 {
                        DescentWitness::Subsector
                    } else {
                        return Err(fail(
                            key,
                            format!("the child {child:?} on support {child_support:#x}"),
                        ));
                    };
                    *report.witnesses.entry(witness).or_default() += 1;
                }
            }
        }
    }
    report.witness_keys += keys.len();
    Ok(())
}

/// Item 4 for a rule lane: the trivial node at one loop, else one rule edge into
/// `L - 1` loops and fewer lines.
fn check_rule_owner(
    library: &PLibrary,
    spec: &POwnerSpec,
    parent: &Presentation,
    t: usize,
) -> Result<(), PError> {
    let fail = |detail: String| {
        malformed(format!(
            "{:?} owner {:016x} on presentation {}: {detail}",
            spec.lane, spec.class.0, parent.id.0
        ))
    };
    let kind = match spec.lane {
        OwnerLane::Carpet => EdgeKind::Carpet,
        _ => EdgeKind::Insertion,
    };
    match spec.edge_maps.as_slice() {
        [] if parent.loops == 1 && kind == EdgeKind::Insertion => Ok(()),
        &[edge] => {
            let map = &library.edge_maps()[edge];
            let child = library.presentation(map.child);
            // An Induced child's lines are its sector's (p5l-library: 5L Insertion
            // rule edges into integer Custom nodes); the top's other lines are slots.
            let child_lines = match child.kind {
                PresentationKind::Induced { mask, .. } => child
                    .lines()
                    .filter(|(index, _)| mask >> index & 1 == 1)
                    .count(),
                _ => child.lines().count(),
            };
            if map.kind != kind || map.parent != parent.id {
                return Err(fail(format!("edge map {edge} is {:?}", map.kind)));
            }
            if child.loops.checked_add(1) != Some(parent.loops) || child_lines >= t {
                return Err(fail(format!(
                    "the child {} has {} loops and {child_lines} lines, the owner {} and {t}",
                    child.id.0, child.loops, parent.loops
                )));
            }
            Ok(())
        }
        edges => Err(fail(format!(
            "{} edge maps at {} loops",
            edges.len(),
            parent.loops
        ))),
    }
}

/// Item 4 for the Triangle lane: no edge; `b`, `c1`, `c2` are distinct integer
/// lines of the owner's support, so every child lowers one of them.
fn check_triangle_owner(
    spec: &POwnerSpec,
    parent: &Presentation,
    support: u64,
) -> Result<(), PError> {
    let LaneRoles::Triangle(roles) = spec.roles else {
        return Err(malformed(format!(
            "Triangle owner {:016x} with roles {:?}",
            spec.class.0, spec.roles
        )));
    };
    if !spec.edge_maps.is_empty() {
        return Err(malformed(format!(
            "Triangle owner {:016x} names edge maps",
            spec.class.0
        )));
    }
    let lowered = [roles.b, roles.c[0], roles.c[1]];
    let integer = support & line_mask(parent) & !parent.forced_axes();
    let mask = lowered.iter().fold(0_u64, |mask, &slot| mask | 1 << slot);
    if mask.count_ones() != 3 || mask & integer != mask {
        return Err(malformed(format!(
            "Triangle owner {:016x}: b, c1, c2 = {lowered:?} are not three integer lines of {support:#x}",
            spec.class.0
        )));
    }
    Ok(())
}

/// Item 4 for the Diamond lane (M3-T5b): no edge; every `b[i]` and `c[i]` is a
/// distinct integer line of the owner's support, so every child lowers one.
fn check_diamond_owner(
    spec: &POwnerSpec,
    parent: &Presentation,
    support: u64,
) -> Result<(), PError> {
    let LaneRoles::Diamond(roles) = spec.roles else {
        return Err(malformed(format!(
            "Diamond owner {:016x} with roles {:?}",
            spec.class.0, spec.roles
        )));
    };
    if !spec.edge_maps.is_empty() {
        return Err(malformed(format!(
            "Diamond owner {:016x} names edge maps",
            spec.class.0
        )));
    }
    let lowered: Vec<u16> = roles.b().iter().chain(roles.c()).copied().collect();
    let integer = support & line_mask(parent) & !parent.forced_axes();
    let mask = (lowered.iter()).fold(0_u64, |mask, &slot| {
        mask | 1_u64.checked_shl(u32::from(slot)).unwrap_or(0)
    });
    if mask.count_ones() as usize != lowered.len() || mask & integer != mask {
        return Err(malformed(format!(
            "Diamond owner {:016x}: b, c = {lowered:?} are not distinct integer lines of {support:#x}",
            spec.class.0
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The support-only trace
// ---------------------------------------------------------------------------

/// Why a traced key is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TraceFailureKind {
    Uncovered,
    MissingOwner,
    /// A child that is not strictly below its parent.
    Descent,
    /// Any other error of a step, a route or the census.
    Other,
}

/// The first refusal reached from one input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceFailure {
    pub input: usize,
    pub kind: TraceFailureKind,
    pub detail: String,
}

/// What [`trace`] reached.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TraceReport {
    pub inputs: usize,
    /// Distinct inputs (presentation, powers).
    pub distinct_inputs: usize,
    /// Distinct Route and Apply keys stepped (with their tags when traced with
    /// tags).
    pub route_keys: usize,
    pub apply_keys: usize,
    /// Distinct Apply sites `(class, powers)`: `apply_keys / apply_sites` is the
    /// number of distinct tags per site.
    pub apply_sites: usize,
    /// Distinct Route keys on a zero sector.
    pub zero_keys: usize,
    /// Children and terminals of the stepped keys, summed over the steps.
    pub children: usize,
    pub terminals: usize,
    /// When traced with tags: the distinct identities (stepped key, children,
    /// terminal keys) of each step-cache shard `(stage, t, class)`, summed over
    /// the shards: what a saved cache of these steps stores once per shard.
    pub shard_identities: usize,
    /// Distinct Apply sites `(class, powers)` per owner class.
    pub applied: BTreeMap<OwnerClassId, usize>,
    /// Distinct Route keys per presentation (zero sectors included).
    pub routed: BTreeMap<PresentationId, usize>,
    /// Distinct Route keys transported through a route that flips `q`
    /// (`external_sign = -1`).
    pub flipped_route_keys: usize,
    /// Failures by kind; every failing key counts once.
    pub failures: BTreeMap<TraceFailureKind, usize>,
    /// The first failures, at most [`TRACE_FAILURE_SAMPLES`].
    pub samples: Vec<TraceFailure>,
}

impl TraceReport {
    pub fn failures_of(&self, kind: TraceFailureKind) -> usize {
        self.failures.get(&kind).copied().unwrap_or(0)
    }

    pub fn is_clean(&self) -> bool {
        self.failures.values().all(|&count| count == 0)
    }
}

/// Failures kept with their message.
pub const TRACE_FAILURE_SAMPLES: usize = 32;

/// A traced key: no coefficient, the absolute tag only when traced with tags
/// (else always one).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum TraceKey {
    Route(PresentationId, Vec<i64>, GammaTag),
    Apply(OwnerClassId, Vec<i64>, GammaTag),
}

/// What [`trace_with`] follows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TraceOptions {
    pub limits: ReductionLimits,
    /// Keep absolute Gamma tags in the keys, as the engine's identities do: the
    /// distinct tags per Apply site are then measured (plan risk 12).
    pub tags: bool,
}

/// A terminal of a step: id and tag.
type Terminal = (u32, GammaTag);

/// An identity a step-cache record names.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Identity {
    Key(TraceKey),
    Terminal(Terminal),
}

/// The Ordered position of a key: phase (larger = later), then the priority
/// inside an Apply phase (larger first).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Position {
    phase: usize,
    rank: u32,
    order: Option<POrderKey>,
}

struct Tracer<'l> {
    library: &'l PLibrary,
    limits: ReductionLimits,
    tags: bool,
    max_lines: usize,
    ranks: HashMap<OwnerClassId, u32>,
    owner_sectors: HashMap<(PresentationId, u64), OwnerClassId>,
    routes: HashMap<(PresentationId, u64), Result<Arc<Prepared>, PError>>,
}

struct Refusal(TraceFailureKind, String);

impl From<PError> for Refusal {
    fn from(error: PError) -> Self {
        let kind = match error {
            PError::Uncovered { .. } => TraceFailureKind::Uncovered,
            PError::MissingOwner { .. } => TraceFailureKind::MissingOwner,
            _ => TraceFailureKind::Other,
        };
        Self(kind, error.to_string())
    }
}

impl<'l> Tracer<'l> {
    fn new(library: &'l PLibrary, options: TraceOptions) -> Self {
        let census = library.census();
        let max_lines = library
            .presentations()
            .iter()
            .map(|p| p.lines().count())
            .max()
            .unwrap_or(0);
        // The engine's rank: parents first by DAG node, then class id.
        let mut order: Vec<_> = census.classes.iter().collect();
        order.sort_by_key(|(class, record)| (record.node.is_none(), record.node, **class));
        let count = u32::try_from(order.len()).unwrap_or(u32::MAX);
        let ranks = order
            .iter()
            .enumerate()
            .map(|(index, (class, _))| {
                let index = u32::try_from(index).unwrap_or(u32::MAX);
                (**class, count.saturating_sub(index))
            })
            .collect();
        let owner_sectors = census
            .classes
            .iter()
            .map(|(&class, record)| (record.owner, class))
            .collect();
        Self {
            library,
            limits: options.limits,
            tags: options.tags,
            max_lines,
            ranks,
            owner_sectors,
            routes: HashMap::new(),
        }
    }

    fn presentation(&self, id: PresentationId) -> Result<&'l Arc<Presentation>, PError> {
        self.library
            .presentations()
            .get(id.0 as usize)
            .ok_or_else(|| malformed(format!("unknown presentation {}", id.0)))
    }

    fn t_of(&self, id: PresentationId, powers: &[i64]) -> Result<usize, PError> {
        let p = self.presentation(id)?;
        Ok((support_of(p, powers) & line_mask(p)).count_ones() as usize)
    }

    fn class_t(&self, class: OwnerClassId) -> Result<usize, PError> {
        let record = self
            .library
            .census()
            .classes
            .get(&class)
            .ok_or_else(|| malformed(format!("unknown owner class {:016x}", class.0)))?;
        Ok(usize::from(record.t))
    }

    fn context(&self, class: OwnerClassId) -> Result<PStepContext<'_>, PError> {
        let owner = self
            .library
            .owner(class)
            .ok_or_else(|| malformed(format!("no owner of class {:016x}", class.0)))?;
        Ok(PStepContext {
            library: self.library,
            owner,
            limits: &self.limits,
        })
    }

    fn position(&self, key: &TraceKey) -> Result<Position, PError> {
        match key {
            TraceKey::Route(id, powers, _) => Ok(Position {
                phase: 2 * (self.max_lines - self.t_of(*id, powers)?),
                rank: 0,
                order: None,
            }),
            TraceKey::Apply(class, powers, _) => {
                let cx = self.context(*class)?;
                let order = match cx.owner.program(self.library)? {
                    OwnerProgram::Lane(lane) => lane.order_key(&cx, powers)?,
                    OwnerProgram::Missing { ledger } => {
                        return Err(PError::MissingOwner {
                            class: *class,
                            ledger: *ledger,
                        });
                    }
                };
                Ok(Position {
                    phase: 2 * (self.max_lines - self.class_t(*class)?) + 1,
                    rank: self.ranks.get(class).copied().unwrap_or(0),
                    order: Some(order),
                })
            }
        }
    }

    /// The Apply key of `powers` on the owner sector `(id, support)`, if any.
    fn apply_key(
        &self,
        id: PresentationId,
        powers: Vec<i64>,
        tag: GammaTag,
    ) -> Result<TraceKey, PError> {
        let support = support_of(self.presentation(id)?, &powers);
        let class = self.owner_sectors.get(&(id, support)).ok_or_else(|| {
            malformed(format!(
                "an Apply child on sector {support:#x} of presentation {}, no owner",
                id.0
            ))
        })?;
        Ok(TraceKey::Apply(*class, powers, tag))
    }

    fn route(&mut self, id: PresentationId, support: u64) -> Result<Arc<Prepared>, PError> {
        if let Some(prepared) = self.routes.get(&(id, support)) {
            return prepared.clone();
        }
        let census = self.library.census();
        let compiled = match census.sectors.get(&(id, support)) {
            Some(PSectorFate::Owner { class, map }) => {
                let (owner, owner_support) = census.classes[class].owner;
                compile_route(
                    self.presentation(id)?,
                    self.presentation(owner)?,
                    map,
                    support,
                    owner_support,
                )
                .map(Arc::new)
            }
            _ => Err(malformed(format!(
                "sector {support:#x} of presentation {} has no route",
                id.0
            ))),
        };
        self.routes.insert((id, support), compiled.clone());
        compiled
    }

    /// The children of `key` (none for a zero sector) and its terminals.
    fn children(&mut self, key: &TraceKey) -> Result<(Vec<TraceKey>, Vec<Terminal>), PError> {
        match key {
            TraceKey::Route(id, powers, tag) => {
                let p = self.presentation(*id)?;
                let support = support_of(p, powers);
                if support & !line_mask(p) != 0 {
                    return Err(PError::Unsupported(format!(
                        "key {powers:?} of presentation {} has an ISP in its denominator",
                        id.0
                    )));
                }
                if let Some(&class) = self.owner_sectors.get(&(*id, support)) {
                    return Ok((
                        vec![TraceKey::Apply(class, powers.clone(), tag.clone())],
                        Vec::new(),
                    ));
                }
                let census = self.library.census();
                let class = match census.sectors.get(&(*id, support)) {
                    Some(PSectorFate::Zero) => return Ok((Vec::new(), Vec::new())),
                    Some(PSectorFate::Owner { class, .. }) => *class,
                    None => {
                        return Err(malformed(format!(
                            "sector {support:#x} of presentation {} is not in the census",
                            id.0
                        )));
                    }
                };
                let (owner, owner_support) = census.classes[&class].owner;
                let prepared = self.route(*id, support)?;
                let key = IntegralKey::try_new(powers.iter().copied())
                    .map_err(|error| malformed(error.to_string()))?;
                let limits = ExpansionLimits {
                    diagnostics_enabled: self.limits.diagnostics_enabled,
                    exact_algebra: self.limits.exact_algebra,
                    ..Default::default()
                };
                let mapped = prepared.transport(&key, limits)?;
                let target = self.presentation(owner)?;
                let mut children = Vec::new();
                for endpoint in mapped.terms() {
                    let powers = endpoint.key().powers().to_vec();
                    children.push(if support_of(target, &powers) == owner_support {
                        TraceKey::Apply(class, powers, tag.clone())
                    } else {
                        TraceKey::Route(owner, powers, tag.clone())
                    });
                }
                Ok((children, Vec::new()))
            }
            TraceKey::Apply(class, powers, tag) => {
                let cx = self.context(*class)?;
                let (id, support) = self.library.census().classes[class].owner;
                if support_of(self.presentation(id)?, powers) != support {
                    return Err(malformed(format!(
                        "Apply key {powers:?} outside the support {support:#x} of its owner"
                    )));
                }
                let lane = match cx.owner.program(self.library)? {
                    OwnerProgram::Lane(lane) => Arc::clone(lane),
                    OwnerProgram::Missing { ledger } => {
                        return Err(PError::MissingOwner {
                            class: *class,
                            ledger: *ledger,
                        });
                    }
                };
                let view = PKeyView {
                    presentation: id,
                    powers,
                    tag,
                };
                let output = lane.step(&cx, &view)?;
                let mut children = Vec::with_capacity(output.children.len());
                for child in output.children {
                    let tag = if self.tags {
                        child.tag
                    } else {
                        GammaTag::one()
                    };
                    children.push(match child.stage {
                        PStage::Route => TraceKey::Route(child.presentation, child.powers, tag),
                        PStage::Apply => self.apply_key(child.presentation, child.powers, tag)?,
                    });
                }
                for (terminal, _, _) in &output.terminals {
                    if terminal.0 as usize >= self.library.terminals().len() {
                        return Err(malformed(format!(
                            "a step emits the unknown terminal {}",
                            terminal.0
                        )));
                    }
                }
                let terminals = output
                    .terminals
                    .into_iter()
                    .map(|(terminal, tag, _)| {
                        let tag = if self.tags { tag } else { GammaTag::one() };
                        (terminal.0, tag)
                    })
                    .collect();
                Ok((children, terminals))
            }
        }
    }

    /// Whether the Route key `powers` of presentation `id` is transported
    /// through a route with `external_sign = -1`.
    fn flips_q(&self, id: PresentationId, powers: &[i64]) -> Result<bool, PError> {
        let support = support_of(self.presentation(id)?, powers);
        Ok(matches!(
            self.library.census().sectors.get(&(id, support)),
            Some(PSectorFate::Owner { class, map })
                if map.external_sign < 0
                    && self.library.census().classes[class].owner != (id, support)
        ))
    }

    /// The step-cache shard of `key`: stage, `t`, presentation or class.
    fn shard(&self, key: &TraceKey) -> Result<(u8, usize, u64), PError> {
        Ok(match key {
            TraceKey::Route(id, powers, _) => (0, self.t_of(*id, powers)?, u64::from(id.0)),
            TraceKey::Apply(class, ..) => (1, self.class_t(*class)?, class.0),
        })
    }

    /// Step `key` and check that every child descends.
    fn expand(&mut self, key: &TraceKey) -> Result<(Vec<TraceKey>, Vec<Terminal>), Refusal> {
        let parent = self.position(key)?;
        let (children, terminals) = self.children(key)?;
        for child in &children {
            let position = self.position(child)?;
            // A Route parent sits at (even phase, 0, None): a child in its own
            // phase is a Route key at the same position, never lower, so the
            // Apply clause below is implied there (kept for clarity).
            let descends = position.phase > parent.phase
                || (position.phase == parent.phase
                    && (position.rank, &position.order) < (parent.rank, &parent.order)
                    && matches!(key, TraceKey::Apply(..)));
            if !descends {
                return Err(Refusal(
                    TraceFailureKind::Descent,
                    format!("{child:?} does not descend from {key:?}"),
                ));
            }
        }
        Ok((children, terminals))
    }
}

/// Trace `inputs` (keys of library presentations) through `library` without
/// tags, see the module docs and [`trace_with`].
pub fn trace(
    library: &PLibrary,
    inputs: &[(PresentationId, Vec<i64>)],
    limits: ReductionLimits,
) -> Result<TraceReport, PError> {
    trace_with(
        library,
        inputs,
        TraceOptions {
            limits,
            tags: false,
        },
    )
}

/// Trace `inputs`, each at tag one. Keys are deduplicated across inputs; a key
/// that fails is counted once, for the first input that reaches it, and is not
/// expanded. An input that is no key of the library is an error, not a failure.
pub fn trace_with(
    library: &PLibrary,
    inputs: &[(PresentationId, Vec<i64>)],
    options: TraceOptions,
) -> Result<TraceReport, PError> {
    let mut tracer = Tracer::new(library, options);
    let mut sites: HashSet<(OwnerClassId, Vec<i64>)> = HashSet::new();
    let mut report = TraceReport {
        inputs: inputs.len(),
        ..TraceReport::default()
    };
    let mut seen: HashSet<TraceKey> = HashSet::new();
    let mut distinct = HashSet::new();
    let mut shards: HashMap<(u8, usize, u64), HashSet<Identity>> = HashMap::new();
    for (index, (id, powers)) in inputs.iter().enumerate() {
        let p = tracer.presentation(*id).map_err(|_| {
            PError::Unsupported(format!(
                "input on presentation {}, which this library does not have",
                id.0
            ))
        })?;
        if powers.len() != p.slots.len()
            || powers
                .iter()
                .zip(&p.slots)
                .any(|(&power, slot)| power > 0 && matches!(slot, Slot::Isp(_)))
        {
            return Err(PError::Unsupported(format!(
                "input {powers:?} is no key of presentation {}",
                id.0
            )));
        }
        distinct.insert((*id, powers.clone()));
        let mut stack = vec![TraceKey::Route(*id, powers.clone(), GammaTag::one())];
        while let Some(key) = stack.pop() {
            if !seen.insert(key.clone()) {
                continue;
            }
            match tracer.expand(&key) {
                Ok((children, terminals)) => {
                    report.children += children.len();
                    report.terminals += terminals.len();
                    if options.tags {
                        let shard = tracer.shard(&key)?;
                        let identities = shards.entry(shard).or_default();
                        identities.insert(Identity::Key(key.clone()));
                        identities.extend(children.iter().cloned().map(Identity::Key));
                        identities.extend(terminals.into_iter().map(Identity::Terminal));
                    }
                    if let TraceKey::Route(id, powers, _) = &key {
                        *report.routed.entry(*id).or_default() += 1;
                        if !children.is_empty() && tracer.flips_q(*id, powers)? {
                            report.flipped_route_keys += 1;
                        }
                    }
                    match &key {
                        TraceKey::Route(..) if children.is_empty() => report.zero_keys += 1,
                        TraceKey::Route(..) => {}
                        TraceKey::Apply(class, powers, _) => {
                            if sites.insert((*class, powers.clone())) {
                                *report.applied.entry(*class).or_default() += 1;
                            }
                        }
                    }
                    match &key {
                        TraceKey::Route(..) => report.route_keys += 1,
                        TraceKey::Apply(..) => report.apply_keys += 1,
                    }
                    stack.extend(children.into_iter().filter(|child| !seen.contains(child)));
                }
                Err(Refusal(kind, detail)) => {
                    *report.failures.entry(kind).or_default() += 1;
                    if report.samples.len() < TRACE_FAILURE_SAMPLES {
                        report.samples.push(TraceFailure {
                            input: index,
                            kind,
                            detail,
                        });
                    }
                }
            }
        }
    }
    report.distinct_inputs = distinct.len();
    report.shard_identities = shards.values().map(HashSet::len).sum();
    report.apply_sites = sites.len();
    Ok(report)
}

#[cfg(test)]
mod rank_gate_tests {
    use super::*;
    use rustred_order::OrderDescriptor;
    use rustred_order::{CompiledOrder, CoordinateGroups, DegreeRow, Direction, Limits};

    /// A 5-slot order (slots 0-2 the support lines, 3-4 the ISPs) with the
    /// given pre-support rows and the default total-excess rows after the
    /// support.
    fn policy(pre: Vec<DegreeRow>, two_sided: Vec<bool>) -> OrderingPolicy {
        let n = 5;
        OrderingPolicy::try_programmed(
            CompiledOrder::compile(
                OrderDescriptor {
                    pre_support_degree_rows: pre,
                    support_weights: vec![0; n],
                    support_priority: (0..n).collect(),
                    degree_rows: vec![
                        DegreeRow {
                            active: vec![1; n],
                            inactive: vec![1; n],
                        },
                        DegreeRow {
                            active: vec![0; n],
                            inactive: vec![1; n],
                        },
                    ],
                    coordinate_priority: (0..n).collect(),
                    coordinate_groups: CoordinateGroups::ActiveFirst,
                    active_direction: Direction::Descending,
                    inactive_direction: Direction::Descending,
                    two_sided,
                },
                Limits::default(),
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn row(active: [u64; 5], inactive: [u64; 5]) -> DegreeRow {
        DegreeRow {
            active: active.to_vec(),
            inactive: inactive.to_vec(),
        }
    }

    /// The WP3 `globisp` gate (numerator rank on the non-support slots, also
    /// on a subset of them) is accepted; a row that weighs a support line or a
    /// positive power, and the support-primary order itself, are not rank
    /// gates.
    #[test]
    fn only_numerator_rows_outside_the_support_are_rank_gates() {
        let support = 0b00111;
        assert!(is_rank_gated(
            &policy(vec![row([0; 5], [0, 0, 0, 1, 1])], vec![]),
            support
        ));
        assert!(is_rank_gated(
            &policy(vec![row([0; 5], [0, 0, 0, 1, 0])], vec![]),
            support
        ));
        assert!(!is_rank_gated(
            &policy(vec![row([0; 5], [1, 0, 0, 1, 1])], vec![]),
            support
        ));
        assert!(!is_rank_gated(
            &policy(vec![row([1, 1, 1, 0, 0], [0, 0, 0, 1, 1])], vec![]),
            support
        ));
        assert!(!is_rank_gated(
            &policy(vec![row([1; 5], [1; 5])], vec![]),
            support
        ));
        let primary = policy(vec![], vec![]);
        assert!(primary.is_support_primary() && !is_rank_gated(&primary, support));
        let two_sided = policy(
            vec![row([0; 5], [0, 0, 0, 1, 1])],
            vec![true, false, false, false, false],
        );
        assert!(!is_rank_gated(&two_sided, support));
    }
}
