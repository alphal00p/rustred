//! Exact signed-line discovery through a basis-invariant native graph.
//!
//! For a full-column-rank vacuum momentum matrix Q, the rational orthogonal
//! projector P = Q (Q^T Q)^-1 Q^T determines col(Q). A signed row permutation
//! S is realizable by a loop change of basis iff S P S^T = P. The graph below
//! represents this equality faithfully: every physical line has a colored
//! center and two interchangeable oriented endpoints, with signed P_ij edge
//! colors. Native graph generators therefore recover simultaneous signed
//! permutations directly, without enumerating the permutation group.
//!
//! The projector is only a discovery representation. Every recovered map
//! passes the existing complete family-map verifier, active-row bijection,
//! exact mass/scale checks, and unconditional unit-Jacobian check.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::Arc;

use symbolica::domains::{InternalOrdering, SelfRing};
use symbolica::graph::Graph;
use symbolica::prelude::{Matrix, PolyVariable, Q, Rational};

use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::IntegralFamily;
use crate::sector::Mask;
use crate::sector::symmetry::{
    self, CoefficientMatrix, DenominatorAction, Jacobian, MomentumMap, VerifiedMap,
};

use super::{
    Error, TerminalSectorSymmetryControl, TerminalSectorSymmetryLimits,
    TerminalSectorSymmetrySkipReason, algebra,
};

type NativeMatrix = Matrix<symbolica::domains::rational::RationalField>;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct MassColor(Coefficient);

impl Ord for MassColor {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.internal_cmp(&other.0)
    }
}

impl PartialOrd for MassColor {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum VertexColor {
    Center { mass: MassColor, diagonal: Rational },
    Orientation,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum EdgeColor {
    Incidence,
    Projector(Rational),
}

type ProposalGraph = Graph<VertexColor, EdgeColor>;

/// Exact native canonical graph identity; cloning shares immutable storage.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct CanonicalSupportKey(Arc<ProposalGraph>);

#[derive(Clone)]
enum Momentum {
    Unprepared,
    Unsupported(super::super::ProductSkipReason),
    Row(Vec<Rational>),
}

/// Line factors and prepared geometry shared across caller-declared supports.
/// This cache never enumerates additional target supports.
pub(super) struct DiscoveryCache {
    family: Arc<IntegralFamily>,
    momenta: Vec<Momentum>,
    supports: BTreeMap<Mask, Option<Arc<PreparedSupport>>>,
}

impl DiscoveryCache {
    pub(super) fn new(family: Arc<IntegralFamily>) -> Self {
        let momenta = vec![Momentum::Unprepared; family.denominator_count()];
        Self {
            family,
            momenta,
            supports: BTreeMap::new(),
        }
    }

    pub(super) fn prepare_support(
        &mut self,
        root: &Mask,
        limits: TerminalSectorSymmetryLimits,
        control: &TerminalSectorSymmetryControl<'_>,
    ) -> Result<Option<Arc<PreparedSupport>>, Error> {
        control.checkpoint()?;
        if !self.supports.contains_key(root) {
            let prepared =
                prepare_support_cached(&self.family, root, &mut self.momenta, limits, control)?;
            self.supports.insert(root.clone(), prepared);
        }
        Ok(self.supports[root].clone())
    }

    /// Diagnostic for an already prepared unsupported support. Successful or
    /// unprepared supports have no skip reason.
    pub(super) fn skip_reason(&self, root: &Mask) -> Option<TerminalSectorSymmetrySkipReason> {
        if !matches!(self.supports.get(root), Some(None)) {
            return None;
        }
        if self.family.loop_count() != 0 && root.active_count() >= self.family.loop_count() {
            for (slot, &active) in root.active_bits().iter().enumerate() {
                if active {
                    if let Momentum::Unsupported(reason) = &self.momenta[slot] {
                        return Some(TerminalSectorSymmetrySkipReason::MomentumGeometry(*reason));
                    }
                }
            }
        }
        Some(TerminalSectorSymmetrySkipReason::RankDeficientSupport)
    }
}

/// Immutable geometry shared by every power assignment on one support.
/// The canonical graph and its generators are computed exactly once.
pub(super) struct PreparedSupport {
    family: Arc<IntegralFamily>,
    slots: Vec<usize>,
    rows: NativeMatrix,
    basis_slots: Vec<usize>,
    inverse: NativeMatrix,
    graph: Arc<ProposalGraph>,
    vertex_map: Vec<usize>,
    inverse_vertex_map: Vec<usize>,
    generators: Vec<Vec<Vec<usize>>>,
}

impl PreparedSupport {
    pub(super) fn canonical_key(&self) -> CanonicalSupportKey {
        CanonicalSupportKey(Arc::clone(&self.graph))
    }
}

/// Prove that an active signed-line stabilizer acts through a finite loop
/// group. This gate deliberately performs no graph construction or search.
pub(super) fn verify_finite_root(
    family: &Arc<IntegralFamily>,
    root: &Mask,
    limits: TerminalSectorSymmetryLimits,
    control: &TerminalSectorSymmetryControl<'_>,
) -> Result<bool, Error> {
    super::eligible(family)?;
    control.checkpoint()?;
    if root.arity() != family.denominator_count() {
        return Err(Error::InvalidWitness(
            "sector symmetry finite root arity mismatch",
        ));
    }
    let loops = family.loop_count();
    let lines = root.active_count();
    if loops == 0 || lines < loops {
        return Ok(false);
    }
    check_shape(lines, loops, limits)?;
    let live = lines
        .checked_mul(loops)
        .and_then(|entries| entries.checked_mul(2))
        .ok_or(Error::InvalidWitness(
            "sector symmetry finite root live entry count overflow",
        ))?;
    if live > limits.verification.max_symbolica_live_matrix_entries {
        return Err(Error::InvalidWitness(
            "sector symmetry finite root exceeds caller live entry limit",
        ));
    }
    let variables = Arc::new((0..loops).map(PolyVariable::Temporary).collect());
    let mut momenta = vec![Momentum::Unprepared; family.denominator_count()];
    let mut rows = Vec::new();
    rows.try_reserve_exact(lines).map_err(algebra)?;
    for (slot, &active) in root.active_bits().iter().enumerate() {
        if !active {
            continue;
        }
        control.checkpoint()?;
        prepare_momentum(family, slot, &variables, &mut momenta, limits)?;
        let Momentum::Row(row) = &momenta[slot] else {
            return Ok(false);
        };
        rows.push(row.clone());
    }
    let rank = matrix(&rows, limits)?.rank();
    control.checkpoint()?;
    Ok(rank == loops)
}

pub(super) fn discover(
    family: &Arc<IntegralFamily>,
    root: &Mask,
    limits: TerminalSectorSymmetryLimits,
    control: &TerminalSectorSymmetryControl<'_>,
) -> Result<Vec<Arc<VerifiedMap>>, Error> {
    let Some(prepared) = prepare_support(family, root, limits, control)? else {
        return Ok(Vec::new());
    };
    discover_prepared(&prepared, limits, control)
}

/// Unsupported or rank-deficient active momentum geometry supplies no maps.
/// Inactive ISP rows are deliberately not factored or restricted here.
/// Active factors use the existing exact integer quadratic/square extractor;
/// the projector theorem does not broaden that extraction boundary.
pub(super) fn prepare_support(
    family: &Arc<IntegralFamily>,
    root: &Mask,
    limits: TerminalSectorSymmetryLimits,
    control: &TerminalSectorSymmetryControl<'_>,
) -> Result<Option<Arc<PreparedSupport>>, Error> {
    let mut momenta = vec![Momentum::Unprepared; family.denominator_count()];
    prepare_support_cached(family, root, &mut momenta, limits, control)
}

fn prepare_support_cached(
    family: &Arc<IntegralFamily>,
    root: &Mask,
    momenta: &mut [Momentum],
    limits: TerminalSectorSymmetryLimits,
    control: &TerminalSectorSymmetryControl<'_>,
) -> Result<Option<Arc<PreparedSupport>>, Error> {
    super::eligible(family)?;
    control.checkpoint()?;
    if root.arity() != family.denominator_count() {
        return Err(Error::InvalidWitness(
            "sector symmetry support arity mismatch",
        ));
    }
    let slots: Vec<_> = root
        .active_bits()
        .iter()
        .enumerate()
        .filter_map(|(slot, &active)| active.then_some(slot))
        .collect();
    let loops = family.loop_count();
    if loops == 0 || slots.len() < loops {
        return Ok(None);
    }
    check_shape(slots.len(), loops, limits)?;
    check_shape(slots.len(), slots.len(), limits)?;
    check_geometry_live_entries(slots.len(), loops, limits)?;
    let variables = Arc::new((0..loops).map(PolyVariable::Temporary).collect());
    let mut rows = Vec::new();
    rows.try_reserve_exact(slots.len()).map_err(algebra)?;
    for &slot in &slots {
        control.checkpoint()?;
        prepare_momentum(family, slot, &variables, momenta, limits)?;
        let Momentum::Row(row) = &momenta[slot] else {
            return Ok(None);
        };
        rows.push(row.clone());
    }
    let all = matrix(&rows, limits)?;
    control.checkpoint()?;
    let mut transpose = all.transpose();
    if transpose.row_reduce(u32::try_from(rows.len()).map_err(algebra)?) != loops {
        return Ok(None);
    }
    // Native RREF pivot columns identify independent ORIGINAL momentum rows.
    let basis_slots: Vec<_> = transpose
        .row_iter()
        .filter_map(|row| row.iter().position(|value| !value.is_zero()))
        .collect();
    drop(transpose);
    let basis = matrix(
        &basis_slots
            .iter()
            .map(|&slot| rows[slot].clone())
            .collect::<Vec<_>>(),
        limits,
    )?;
    let inverse = basis.inv().map_err(algebra)?;
    drop(basis);
    control.checkpoint()?;
    let projector = projector(&all)?;
    let masses = slots
        .iter()
        .map(|&slot| family.denominators()[slot].constant().clone())
        .collect::<Vec<_>>();
    let graph = signed_graph(&projector, &masses, control)?;
    control.checkpoint()?;
    let canonical = graph.canonize();
    control.checkpoint()?;
    let inverse_vertex_map = invert_vertex_map(&canonical.vertex_map)?;
    Ok(Some(Arc::new(PreparedSupport {
        family: Arc::clone(family),
        slots,
        rows: all,
        basis_slots,
        inverse,
        graph: Arc::new(canonical.graph),
        vertex_map: canonical.vertex_map,
        inverse_vertex_map,
        generators: canonical.orbit_generators,
    })))
}

fn prepare_momentum(
    family: &IntegralFamily,
    slot: usize,
    variables: &Arc<Vec<PolyVariable>>,
    momenta: &mut [Momentum],
    limits: TerminalSectorSymmetryLimits,
) -> Result<(), Error> {
    if !matches!(momenta[slot], Momentum::Unprepared) {
        return Ok(());
    }
    momenta[slot] = match super::super::products::momentum_quadratic(family, slot, variables)? {
        Ok(row) => Momentum::Row(
            row.iter()
                .map(|value| rational(value, family.coefficient_context(), limits))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Err(reason) => Momentum::Unsupported(reason),
    };
    Ok(())
}

pub(super) fn discover_prepared(
    support: &PreparedSupport,
    limits: TerminalSectorSymmetryLimits,
    control: &TerminalSectorSymmetryControl<'_>,
) -> Result<Vec<Arc<VerifiedMap>>, Error> {
    control.checkpoint()?;
    let mut maps = Vec::new();
    for cycles in &support.generators {
        control.checkpoint()?;
        // Graphica returns disjoint cycles in CANONICAL vertex numbering.
        let mut canonical_action: Vec<_> = (0..support.vertex_map.len()).collect();
        for cycle in cycles {
            for position in 0..cycle.len() {
                canonical_action[cycle[position]] = cycle[(position + 1) % cycle.len()];
            }
        }
        let action = support
            .vertex_map
            .iter()
            .map(|&canonical| support.inverse_vertex_map[canonical_action[canonical]])
            .collect::<Vec<_>>();
        if let Some(map) = recover(support, support, &action, limits, control)? {
            if !maps
                .iter()
                .any(|old: &Arc<VerifiedMap>| old.momentum() == map.momentum())
            {
                maps.push(map);
            }
        }
    }
    Ok(maps)
}

/// One native canonical isomorphism between two support geometries. Together
/// with each support's self generators it represents the complete signed-map
/// torsor; no factorial enumeration of graph isomorphisms is needed.
pub(super) fn match_supports(
    source: &PreparedSupport,
    target: &PreparedSupport,
    limits: TerminalSectorSymmetryLimits,
    control: &TerminalSectorSymmetryControl<'_>,
) -> Result<Option<Arc<VerifiedMap>>, Error> {
    control.checkpoint()?;
    // Native coefficient ordering assumes the same declared variable map.
    if !source
        .family
        .coefficient_context()
        .has_same_variable_map(target.family.coefficient_context())
        || source.family.loop_count() != target.family.loop_count()
        || source.slots.len() != target.slots.len()
        || source.graph != target.graph
    {
        return Ok(None);
    }
    let action = source
        .vertex_map
        .iter()
        .map(|&canonical| target.inverse_vertex_map[canonical])
        .collect::<Vec<_>>();
    recover(source, target, &action, limits, control)
}

fn projector(rows: &NativeMatrix) -> Result<NativeMatrix, Error> {
    let transpose = rows.transpose();
    let gram = &transpose * rows;
    let inverse = gram.inv().map_err(algebra)?;
    Ok(&(rows * &inverse) * &transpose)
}

fn signed_graph(
    projector: &NativeMatrix,
    masses: &[Coefficient],
    control: &TerminalSectorSymmetryControl<'_>,
) -> Result<ProposalGraph, Error> {
    let count = masses.len();
    count.checked_mul(3).ok_or(Error::InvalidWitness(
        "sector symmetry graph vertex count overflow",
    ))?;
    count
        .checked_mul(count)
        .and_then(|square| square.checked_mul(2))
        .ok_or(Error::InvalidWitness(
            "sector symmetry graph edge count overflow",
        ))?;
    let mut graph = Graph::new();
    for (line, mass) in masses.iter().enumerate() {
        let index = u32::try_from(line).map_err(algebra)?;
        graph.add_node(VertexColor::Center {
            mass: MassColor(mass.clone()),
            diagonal: projector[(index, index)].clone(),
        });
        graph.add_node(VertexColor::Orientation);
        graph.add_node(VertexColor::Orientation);
        for sign in [1, 2] {
            graph
                .add_edge(3 * line, 3 * line + sign, false, EdgeColor::Incidence)
                .map_err(algebra)?;
        }
    }
    for left in 0..count {
        control.checkpoint()?;
        for right in left + 1..count {
            let value = &projector[(
                u32::try_from(left).map_err(algebra)?,
                u32::try_from(right).map_err(algebra)?,
            )];
            if value.is_zero() {
                continue;
            }
            for left_orientation in 0..2 {
                for right_orientation in 0..2 {
                    let color = if left_orientation == right_orientation {
                        value.clone()
                    } else {
                        -value.clone()
                    };
                    graph
                        .add_edge(
                            3 * left + 1 + left_orientation,
                            3 * right + 1 + right_orientation,
                            false,
                            EdgeColor::Projector(color),
                        )
                        .map_err(algebra)?;
                }
            }
        }
    }
    Ok(graph)
}

fn recover(
    source: &PreparedSupport,
    target: &PreparedSupport,
    action: &[usize],
    limits: TerminalSectorSymmetryLimits,
    control: &TerminalSectorSymmetryControl<'_>,
) -> Result<Option<Arc<VerifiedMap>>, Error> {
    control.checkpoint()?;
    let target_rows: Vec<_> = target.rows.row_iter().map(<[_]>::to_vec).collect();
    let mut permuted_rows = Vec::new();
    permuted_rows
        .try_reserve_exact(source.slots.len())
        .map_err(algebra)?;
    for line in 0..source.slots.len() {
        let center = action[3 * line];
        let plus = action[3 * line + 1];
        let minus = action[3 * line + 2];
        if center % 3 != 0
            || plus / 3 != center / 3
            || minus / 3 != center / 3
            || plus % 3 == 0
            || minus % 3 == 0
            || plus == minus
        {
            return Err(Error::InvalidWitness(
                "native signed graph action broke line pairing",
            ));
        }
        let row = &target_rows[center / 3];
        permuted_rows.push(
            row.iter()
                .map(|value| {
                    if plus % 3 == 1 {
                        value.clone()
                    } else {
                        -value.clone()
                    }
                })
                .collect(),
        );
    }
    let targets = matrix(&permuted_rows, limits)?;
    let target_basis = matrix(
        &source
            .basis_slots
            .iter()
            .map(|&slot| permuted_rows[slot].clone())
            .collect::<Vec<_>>(),
        limits,
    )?;
    let linear = &source.inverse * &target_basis;
    // Dependent rows are replayed natively before the full family verifier.
    if &source.rows * &linear != targets {
        return Err(Error::InvalidWitness(
            "native signed projector action failed momentum replay",
        ));
    }
    let loops = source.family.loop_count();
    let context = source.family.coefficient_context();
    let entries = linear
        .into_vec()
        .iter()
        .map(|value| coefficient(value, context, limits))
        .collect::<Result<Vec<_>, _>>()?;
    let map = MomentumMap::new(
        CoefficientMatrix::try_new_with_max_entries(
            loops,
            loops,
            entries,
            limits.verification.max_matrix_entries,
        )
        .map_err(algebra)?,
        CoefficientMatrix::try_new_with_max_entries(
            loops,
            0,
            [],
            limits.verification.max_matrix_entries,
        )
        .map_err(algebra)?,
        CoefficientMatrix::try_new_with_max_entries(
            0,
            0,
            [],
            limits.verification.max_matrix_entries,
        )
        .map_err(algebra)?,
    );
    let verified = Arc::new(
        symmetry::verify(&source.family, &target.family, map, limits.verification)
            .map_err(algebra)?,
    );
    if !matches!(verified.jacobian(), Jacobian::Unit { .. })
        || verified.nonzero_conditions().iter().any(|condition| {
            !condition.polynomial().is_constant() || condition.polynomial().is_zero()
        })
    {
        return Ok(None);
    }
    let mut images = Vec::new();
    for &slot in &source.slots {
        let DenominatorAction::Monomial {
            target: image,
            scale,
        } = &verified.row_actions()[slot]
        else {
            return Ok(None);
        };
        if !scale.is_one() || target.slots.binary_search(image).is_err() || images.contains(image) {
            return Ok(None);
        }
        images.push(*image);
    }
    control.checkpoint()?;
    Ok(Some(verified))
}

fn invert_vertex_map(map: &[usize]) -> Result<Vec<usize>, Error> {
    let mut inverse = vec![usize::MAX; map.len()];
    for (original, &canonical) in map.iter().enumerate() {
        let Some(entry) = inverse.get_mut(canonical) else {
            return Err(Error::InvalidWitness(
                "native graph vertex map out of range",
            ));
        };
        if *entry != usize::MAX {
            return Err(Error::InvalidWitness(
                "native graph vertex map is not bijective",
            ));
        }
        *entry = original;
    }
    Ok(inverse)
}

fn check_shape(
    rows: usize,
    columns: usize,
    limits: TerminalSectorSymmetryLimits,
) -> Result<(), Error> {
    u32::try_from(rows).map_err(algebra)?;
    u32::try_from(columns).map_err(algebra)?;
    let entries = rows.checked_mul(columns).ok_or(Error::InvalidWitness(
        "sector symmetry matrix entry count overflow",
    ))?;
    if entries > limits.verification.max_matrix_entries
        || entries > limits.verification.max_symbolica_single_matrix_entries
    {
        return Err(Error::InvalidWitness(
            "sector symmetry native matrix exceeds caller entry limit",
        ));
    }
    Ok(())
}

fn check_geometry_live_entries(
    lines: usize,
    loops: usize,
    limits: TerminalSectorSymmetryLimits,
) -> Result<(), Error> {
    // Q, transpose/work/product copies, Gram/inverse work, retained basis
    // inverse and the line-space projector. Native graph storage is separate.
    let entries = lines
        .checked_mul(loops)
        .and_then(|value| value.checked_mul(4))
        .and_then(|value| {
            loops
                .checked_mul(loops)
                .and_then(|square| square.checked_mul(4))
                .and_then(|square| value.checked_add(square))
        })
        .and_then(|value| {
            lines
                .checked_mul(lines)
                .and_then(|square| value.checked_add(square))
        })
        .ok_or(Error::InvalidWitness(
            "sector symmetry native live matrix entry count overflow",
        ))?;
    if entries > limits.verification.max_symbolica_live_matrix_entries {
        return Err(Error::InvalidWitness(
            "sector symmetry native geometry exceeds caller live entry limit",
        ));
    }
    Ok(())
}

fn matrix(
    rows: &[Vec<Rational>],
    limits: TerminalSectorSymmetryLimits,
) -> Result<NativeMatrix, Error> {
    let columns = rows
        .first()
        .ok_or(Error::InvalidWitness(
            "empty sector symmetry momentum matrix",
        ))?
        .len();
    if rows.iter().any(|row| row.len() != columns) {
        return Err(Error::InvalidWitness(
            "ragged sector symmetry momentum matrix",
        ));
    }
    check_shape(rows.len(), columns, limits)?;
    Matrix::from_linear(
        rows.iter().flatten().cloned().collect(),
        u32::try_from(rows.len()).map_err(algebra)?,
        u32::try_from(columns).map_err(algebra)?,
        Q,
    )
    .map_err(algebra)
}

fn rational(
    value: &Coefficient,
    context: &CoefficientContext,
    limits: TerminalSectorSymmetryLimits,
) -> Result<Rational, Error> {
    context
        .validate_with_limits(value, limits.verification.exact_algebra)
        .map_err(algebra)?;
    if !value.numerator.is_constant()
        || !value.denominator.is_constant()
        || value.denominator.is_zero()
    {
        return Err(Error::InvalidWitness(
            "sector symmetry momentum coefficient is not constant rational",
        ));
    }
    Ok(Rational::from((
        value.numerator.get_constant(),
        value.denominator.get_constant(),
    )))
}

fn coefficient(
    value: &Rational,
    context: &CoefficientContext,
    limits: TerminalSectorSymmetryLimits,
) -> Result<Coefficient, Error> {
    let numerator: Coefficient = context
        .zero()
        .numerator
        .constant(value.numerator_ref().clone())
        .into();
    let denominator: Coefficient = context
        .zero()
        .numerator
        .constant(value.denominator_ref().clone())
        .into();
    context
        .try_div(&numerator, &denominator, limits.verification.exact_algebra)
        .map_err(algebra)
}

#[cfg(test)]
mod tests;
