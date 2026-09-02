//! Cold compilation and exact finite-complement replay.

use std::cmp::Ordering;
use std::sync::Arc;

use crate::algebra::{IndexedCoefficientContext, IndexedGuardLimits};
use crate::family::IntegralKey;
use crate::foundry::completion::frame::admission::semantic::compare_exact_circuit_content;
use crate::foundry::completion::{
    BoxCover, CompletionGeometryError, LatticeBox, LatticeCardinality, LatticePoint, SectorChart,
    UncoveredPartition,
};
use crate::sector::{Mask, OrderingPolicy};

use super::super::semantic::{ExactCircuitSemanticCandidate, ExactCircuitSemanticSelection};
use super::model::ExactFiniteTerminalOwnerId;
use super::{
    ExactCircuitOwner, ExactCircuitOwnerCover, ExactCircuitOwnerCoverError,
    ExactCircuitOwnerCoverLimits, ExactCircuitOwnerId, ExactCircuitOwnerInput,
    ExactFinitePointOwner, ExactFiniteTerminalOwner, ExactOwnerCoverObstructionKind,
    ExactOwnerCoverStatus,
};

const OWNER_INPUTS: &str = "exact owner-cover semantic inputs";
const OWNER_REGION_ENDPOINT_CELLS: &str = "exact owner-cover region endpoint cells";
const EXPLICIT_TERMINALS: &str = "exact owner-cover explicit terminals";
const TERMINAL_COORDINATE_CELLS: &str = "exact owner-cover terminal-coordinate cells";
const FINITE_POINTS: &str = "exact owner-cover finite complement points";
const FINITE_POINT_COORDINATE_CELLS: &str = "exact owner-cover finite-complement coordinate cells";
const POINT_OWNER_PROBES: &str = "exact owner-cover finite point-owner probes";

struct PreparedOwner {
    input_ordinal: usize,
    family_fingerprint: Arc<String>,
    context_fingerprint: Arc<String>,
    sector: Mask,
    ordering: OrderingPolicy,
    owner_snapshot_id: crate::foundry::completion::stratum::ImmutableOwnerSnapshotId,
    leading: LatticePoint,
    region: crate::foundry::completion::LatticeBox,
    semantic: Arc<super::super::ExactCircuitSemanticDag>,
    guard_total: bool,
}

impl ExactCircuitOwnerCover {
    /// Materialize the exact proof shell for a carrier already authenticated
    /// as completely owned by the retained immutable predecessor.
    ///
    /// The caller must establish that ownership through the installed
    /// predecessor snapshot, not from serialized bounds or a detached domain.
    /// Consequently this proof has no ordinary circuit owner and no fabricated
    /// terminal: its empty uncovered partition records predecessor closure.
    pub(crate) fn predecessor_closed(
        family_fingerprint: &str,
        context_fingerprint: &str,
        sector: Mask,
        ordering: OrderingPolicy,
        owner_snapshot_id: crate::foundry::completion::stratum::ImmutableOwnerSnapshotId,
        closure_carrier: LatticeBox,
    ) -> Self {
        Self {
            family_fingerprint: Arc::new(family_fingerprint.to_owned()),
            context_fingerprint: Arc::new(context_fingerprint.to_owned()),
            sector,
            ordering,
            owner_snapshot_id,
            closure_carrier,
            owners: Box::new([]),
            terminals: Box::new([]),
            finite_point_owners: Box::new([]),
            uncovered: UncoveredPartition::new(Vec::new(), 0),
            missing_terminals: Box::new([]),
            guard_incomplete_owners: Box::new([]),
            finite_complement_points: 0,
            point_owner_probes: 0,
            compiled_uncovered_boxes: 0,
            compiled_uncovered_box_coordinate_cells: 0,
            compiled_split_operations: 0,
            status: ExactOwnerCoverStatus::Closed,
        }
    }

    pub(crate) fn try_compile<'partition, 'frame: 'partition>(
        context: &IndexedCoefficientContext,
        inputs: impl IntoIterator<Item = ExactCircuitOwnerInput<'partition, 'frame>>,
        explicit_terminals: impl IntoIterator<Item = IntegralKey>,
        limits: ExactCircuitOwnerCoverLimits,
    ) -> Result<Self, ExactCircuitOwnerCoverError> {
        Self::try_compile_in_carrier(context, inputs, explicit_terminals, None, limits)
    }

    /// Compile relative to one explicit, finite root carrier.
    ///
    /// This is the bounded diagnostic-preview seam. The retained carrier is
    /// part of the resulting proof object, so a `Closed` verdict cannot be
    /// mistaken for closure over the complete machine-index sector.
    pub(crate) fn try_compile_with_carrier<'partition, 'frame: 'partition>(
        context: &IndexedCoefficientContext,
        inputs: impl IntoIterator<Item = ExactCircuitOwnerInput<'partition, 'frame>>,
        explicit_terminals: impl IntoIterator<Item = IntegralKey>,
        carrier: &LatticeBox,
        limits: ExactCircuitOwnerCoverLimits,
    ) -> Result<Self, ExactCircuitOwnerCoverError> {
        Self::try_compile_in_carrier(context, inputs, explicit_terminals, Some(carrier), limits)
    }

    fn try_compile_in_carrier<'partition, 'frame: 'partition>(
        context: &IndexedCoefficientContext,
        inputs: impl IntoIterator<Item = ExactCircuitOwnerInput<'partition, 'frame>>,
        explicit_terminals: impl IntoIterator<Item = IntegralKey>,
        requested_carrier: Option<&LatticeBox>,
        limits: ExactCircuitOwnerCoverLimits,
    ) -> Result<Self, ExactCircuitOwnerCoverError> {
        let mut prepared = try_vec(0, OWNER_INPUTS)?;
        for (input_ordinal, input) in inputs.into_iter().enumerate() {
            let requested = checked_add(OWNER_INPUTS, prepared.len(), 1)?;
            check_limit(OWNER_INPUTS, requested, limits.max_owner_inputs)?;
            prepared.try_reserve_exact(1).map_err(|_| {
                ExactCircuitOwnerCoverError::AllocationFailure {
                    resource: OWNER_INPUTS,
                    requested,
                }
            })?;
            prepared.push(prepare_owner(context, input, input_ordinal, limits)?);
        }
        let Some(scope) = prepared.first() else {
            return Err(ExactCircuitOwnerCoverError::EmptyOwnerInputs);
        };
        for owner in prepared.iter().skip(1) {
            validate_common_scope(scope, owner)?;
        }
        let family_fingerprint = scope.family_fingerprint.clone();
        let context_fingerprint = scope.context_fingerprint.clone();
        let sector = scope.sector.clone();
        let ordering = scope.ordering;
        let owner_snapshot_id = scope.owner_snapshot_id.clone();

        prepared.sort_unstable_by(compare_prepared_owners);
        if prepared
            .windows(2)
            .any(|pair| compare_prepared_owners(&pair[0], &pair[1]) == Ordering::Equal)
        {
            return Err(ExactCircuitOwnerCoverError::DuplicateOwnerContent);
        }

        let arity = sector.arity();
        let mut owners = try_vec(prepared.len(), OWNER_INPUTS)?;
        for (ordinal, owner) in prepared.into_iter().enumerate() {
            owners.push(ExactCircuitOwner {
                id: ExactCircuitOwnerId(ordinal),
                leading: owner.leading,
                region: owner.region,
                semantic: owner.semantic,
                guard_total: owner.guard_total,
            });
        }

        let chart = SectorChart::new(sector.clone());
        let full_carrier = chart.carrier_box()?;
        let carrier = requested_carrier.unwrap_or(&full_carrier);
        validate_closure_carrier(carrier, &full_carrier, arity)?;
        let terminals = prepare_terminals(&chart, explicit_terminals, limits)?;
        for (terminal, retained) in terminals.iter().enumerate() {
            if !carrier.contains(retained.point()) {
                return Err(ExactCircuitOwnerCoverError::TerminalOutsideClosureCarrier {
                    terminal,
                });
            }
        }
        let uncovered = uncovered_for_owners(
            carrier,
            arity,
            owners.iter().filter(|owner| owner.guard_total),
            limits,
        )?;
        let (mut compiled_uncovered_boxes, mut compiled_uncovered_box_coordinate_cells) =
            partition_storage(&uncovered, arity)?;
        let mut compiled_split_operations = uncovered.split_operations();
        let mut probes = 0usize;
        validate_terminal_disjointness(context, &owners, &terminals, limits, &mut probes)?;

        let mut finite_point_owners = try_vec(0, FINITE_POINTS)?;
        let mut missing_terminals = try_vec(0, FINITE_POINTS)?;
        let mut guard_incomplete_owners = try_vec(0, OWNER_INPUTS)?;
        let mut finite_complement_points = 0usize;

        let finite_partition = uncovered.is_finite();
        let enumerable_point_count = if finite_partition {
            match uncovered.try_cardinality(limits.max_finite_complement_points) {
                Ok(LatticeCardinality::Finite(point_count)) => Some(point_count),
                Ok(LatticeCardinality::Infinite) => {
                    return Err(ExactCircuitOwnerCoverError::Invariant(
                        "a finite uncovered partition reported infinite cardinality",
                    ));
                }
                Err(
                    CompletionGeometryError::ResourceLimit { .. }
                    | CompletionGeometryError::ResourceCountOverflow { .. },
                ) => None,
                Err(error) => return Err(error.into()),
            }
        } else {
            None
        };
        let status = if let Some(point_count) = enumerable_point_count {
            check_limit(
                FINITE_POINTS,
                point_count,
                limits.max_finite_complement_points,
            )?;
            finite_complement_points = point_count;
            let coordinate_cells = checked_mul(FINITE_POINT_COORDINATE_CELLS, point_count, arity)?;
            check_limit(
                FINITE_POINT_COORDINATE_CELLS,
                coordinate_cells,
                limits.max_finite_complement_coordinate_cells,
            )?;
            finite_point_owners = try_vec(point_count, FINITE_POINTS)?;
            missing_terminals = try_vec(point_count, FINITE_POINTS)?;
            for point in enumerate_finite_partition(&uncovered, point_count)? {
                let target = chart.to_integral(&point)?;
                if let Some((owner, candidate)) =
                    try_select_owner(context, &owners, &point, &target, limits, &mut probes)?
                {
                    finite_point_owners.push(ExactFinitePointOwner {
                        point,
                        owner: owner.id,
                        candidate_ordinal: candidate.id().ordinal(),
                        circuit: candidate.circuit().clone(),
                    });
                } else if terminals
                    .binary_search_by(|terminal| terminal.point.cmp(&point))
                    .is_err()
                {
                    missing_terminals.push(point);
                }
            }
            if missing_terminals.is_empty() {
                ExactOwnerCoverStatus::Closed
            } else {
                ExactOwnerCoverStatus::Incomplete(
                    ExactOwnerCoverObstructionKind::FiniteTerminalOwnership,
                )
            }
        } else if finite_partition {
            // Carrier normalization may turn a thin multi-axis tail into a
            // finite box whose product is still intentionally too large to
            // enumerate.  This is conservative incomplete structural state,
            // not a compiler failure and never closure authority.
            ExactOwnerCoverStatus::Incomplete(ExactOwnerCoverObstructionKind::NonFinite)
        } else {
            let mut structural_limits = limits;
            structural_limits.geometry.max_uncovered_boxes = structural_limits
                .geometry
                .max_uncovered_boxes
                .checked_sub(compiled_uncovered_boxes)
                .ok_or(ExactCircuitOwnerCoverError::ResourceCountOverflow {
                    resource: "compiled uncovered lattice boxes",
                })?;
            structural_limits
                .geometry
                .max_uncovered_box_coordinate_cells = structural_limits
                .geometry
                .max_uncovered_box_coordinate_cells
                .checked_sub(compiled_uncovered_box_coordinate_cells)
                .ok_or(ExactCircuitOwnerCoverError::ResourceCountOverflow {
                    resource: "compiled uncovered lattice-box coordinate cells",
                })?;
            structural_limits.geometry.max_split_operations = structural_limits
                .geometry
                .max_split_operations
                .checked_sub(compiled_split_operations)
                .ok_or(ExactCircuitOwnerCoverError::ResourceCountOverflow {
                    resource: "compiled box-union split operations",
                })?;
            let structural =
                uncovered_for_owners(carrier, arity, owners.iter(), structural_limits)?;
            let (structural_boxes, structural_coordinate_cells) =
                partition_storage(&structural, arity)?;
            compiled_uncovered_boxes = checked_add(
                "compiled uncovered lattice boxes",
                compiled_uncovered_boxes,
                structural_boxes,
            )?;
            compiled_uncovered_box_coordinate_cells = checked_add(
                "compiled uncovered lattice-box coordinate cells",
                compiled_uncovered_box_coordinate_cells,
                structural_coordinate_cells,
            )?;
            compiled_split_operations = checked_add(
                "compiled box-union split operations",
                compiled_split_operations,
                structural.split_operations(),
            )?;
            if structural.is_finite() {
                guard_incomplete_owners = effective_partial_owners(&owners, &uncovered)?;
                ExactOwnerCoverStatus::Incomplete(ExactOwnerCoverObstructionKind::GuardIncomplete)
            } else {
                ExactOwnerCoverStatus::Incomplete(ExactOwnerCoverObstructionKind::NonFinite)
            }
        };

        Ok(Self {
            family_fingerprint,
            context_fingerprint,
            sector,
            ordering,
            owner_snapshot_id,
            closure_carrier: carrier.try_clone_fallible()?,
            owners: owners.into_boxed_slice(),
            terminals: terminals.into_boxed_slice(),
            finite_point_owners: finite_point_owners.into_boxed_slice(),
            uncovered,
            missing_terminals: missing_terminals.into_boxed_slice(),
            guard_incomplete_owners: guard_incomplete_owners.into_boxed_slice(),
            finite_complement_points,
            point_owner_probes: probes,
            compiled_uncovered_boxes,
            compiled_uncovered_box_coordinate_cells,
            compiled_split_operations,
            status,
        })
    }
}

fn partition_storage(
    partition: &crate::foundry::completion::UncoveredPartition,
    arity: usize,
) -> Result<(usize, usize), ExactCircuitOwnerCoverError> {
    let boxes = partition.boxes().len();
    let coordinate_cells = checked_mul(
        "compiled uncovered lattice-box coordinate cells",
        checked_mul(
            "compiled uncovered lattice-box coordinate cells",
            boxes,
            arity,
        )?,
        2,
    )?;
    Ok((boxes, coordinate_cells))
}

fn prepare_owner(
    context: &IndexedCoefficientContext,
    input: ExactCircuitOwnerInput<'_, '_>,
    input_ordinal: usize,
    limits: ExactCircuitOwnerCoverLimits,
) -> Result<PreparedOwner, ExactCircuitOwnerCoverError> {
    let partition = input.partition;
    let outer = input.outer_extension;
    if outer.context_fingerprint.as_str() != context.fingerprint()
        || partition.frame().context_fingerprint() != context.fingerprint()
    {
        return Err(ExactCircuitOwnerCoverError::WrongContext);
    }
    if outer.semantic.candidates().is_empty() {
        return Err(ExactCircuitOwnerCoverError::OwnerJoin {
            owner: input_ordinal,
            detail: "semantic DAG contains no exact candidates",
        });
    }
    if !std::ptr::eq(outer.plan, partition.frame())
        || !outer.semantic.is_bound_to(partition.frame())
        || outer.family_fingerprint.as_str() != partition.frame().family_fingerprint()
        || outer.context_fingerprint.as_str() != partition.frame().context_fingerprint()
        || outer.sector != *partition.frame().sector()
        || outer.ordering != partition.ordering()
        || outer.stratum_id != *partition.stratum_id()
        || outer.owner_snapshot_id != *partition.snapshot_id()
        || outer.target_column != partition.target_column()
    {
        return Err(ExactCircuitOwnerCoverError::OwnerJoin {
            owner: input_ordinal,
            detail: "outer-extension witness differs from its exact physical plan or target partition",
        });
    }
    if outer.leading.coordinates() != outer.region.lower() {
        return Err(ExactCircuitOwnerCoverError::OwnerJoin {
            owner: input_ordinal,
            detail: "outer-extension leading point differs from its region lower endpoint",
        });
    }
    let arity = partition.frame().sector().arity();
    let owner_count = checked_add(OWNER_INPUTS, input_ordinal, 1)?;
    let coordinate_cells = checked_mul(
        OWNER_REGION_ENDPOINT_CELLS,
        checked_mul(OWNER_REGION_ENDPOINT_CELLS, owner_count, arity)?,
        2,
    )?;
    check_limit(
        OWNER_REGION_ENDPOINT_CELLS,
        coordinate_cells,
        limits.max_owner_coordinate_cells,
    )?;
    let guard_total = guard_total_on_region(
        context,
        &outer.semantic,
        &outer.region,
        partition.frame().sector(),
        input_ordinal,
        limits.guard_locus,
    )?;
    Ok(PreparedOwner {
        input_ordinal,
        family_fingerprint: Arc::new(partition.frame().family_fingerprint().to_owned()),
        context_fingerprint: context.fingerprint_owner(),
        sector: partition.frame().sector().clone(),
        ordering: partition.ordering(),
        owner_snapshot_id: partition.snapshot_id().clone(),
        leading: outer.leading,
        region: outer.region,
        guard_total,
        semantic: outer.semantic,
    })
}

fn guard_total_on_region(
    context: &IndexedCoefficientContext,
    semantic: &super::super::ExactCircuitSemanticDag,
    region: &crate::foundry::completion::LatticeBox,
    sector: &Mask,
    owner: usize,
    limits: IndexedGuardLimits,
) -> Result<bool, ExactCircuitOwnerCoverError> {
    if semantic.guard_dag().is_abstractly_total() {
        return Ok(true);
    }
    for (candidate_ordinal, candidate) in semantic.candidates().iter().enumerate() {
        let mut everywhere_applicable = true;
        for (guard_ordinal, atom) in candidate.guard_atoms().iter().enumerate() {
            let misses_region = context
                .integer_zero_locus_misses_domain(
                    atom.coefficient_system(),
                    limits,
                    |position, root| integer_root_belongs_to_region(position, root, region, sector),
                )
                .map_err(|error| ExactCircuitOwnerCoverError::GuardLocus {
                    owner,
                    candidate: candidate_ordinal,
                    guard: guard_ordinal,
                    error,
                })?;
            if !misses_region {
                everywhere_applicable = false;
                break;
            }
        }
        if everywhere_applicable {
            return Ok(true);
        }
    }
    // Deliberately conservative: several individually partial candidates may
    // jointly cover an orthant (for example guards `n` and `n - 1`). Proving
    // the integer-emptiness of every reachable Incomplete-path conjunction is
    // a separate exact-locus extension. Until then this returns `false`, so
    // such a region remains a typed GuardIncomplete obstruction and can never
    // acquire closure authority from sampling.
    Ok(false)
}

fn integer_root_belongs_to_region(
    position: usize,
    root: &symbolica::prelude::Integer,
    region: &crate::foundry::completion::LatticeBox,
    sector: &Mask,
) -> bool {
    let Some((&lower, &upper)) = region
        .lower()
        .get(position)
        .zip(region.upper().get(position))
    else {
        // A malformed domain callback must never mint a nonvanishing proof.
        return true;
    };
    let Some(&active) = sector.active_bits().get(position) else {
        return true;
    };
    let Some(root) = root.to_i64() else {
        // The exact completion lattice can be unbounded even though runtime
        // integral keys use i64. Retain a possible out-of-range exceptional
        // root rather than silently certifying a mathematical ray.
        return true;
    };
    let coordinate = if active {
        if root < 1 {
            return false;
        }
        u64::try_from(i128::from(root) - 1).ok()
    } else {
        if root > 0 {
            return false;
        }
        u64::try_from(-i128::from(root)).ok()
    };
    let Some(coordinate) = coordinate else {
        // As above, conversion uncertainty withholds the certificate.
        return true;
    };
    coordinate >= lower && upper.is_none_or(|upper| coordinate <= upper)
}

fn effective_partial_owners(
    owners: &[ExactCircuitOwner],
    uncovered: &crate::foundry::completion::UncoveredPartition,
) -> Result<Vec<ExactCircuitOwnerId>, ExactCircuitOwnerCoverError> {
    let mut effective = try_vec(owners.len(), OWNER_INPUTS)?;
    for owner in owners.iter().filter(|owner| !owner.guard_total) {
        if !uncovered
            .boxes()
            .iter()
            .any(|cell| cell.intersects_box(owner.region()))
        {
            continue;
        }
        effective.push(owner.id);
    }
    Ok(effective)
}

fn validate_common_scope(
    scope: &PreparedOwner,
    owner: &PreparedOwner,
) -> Result<(), ExactCircuitOwnerCoverError> {
    let detail = if owner.family_fingerprint != scope.family_fingerprint {
        Some("family fingerprint differs")
    } else if owner.context_fingerprint != scope.context_fingerprint {
        Some("coefficient context differs")
    } else if owner.sector != scope.sector {
        Some("sector differs")
    } else if owner.ordering != scope.ordering {
        Some("ordering policy differs")
    } else if owner.owner_snapshot_id != scope.owner_snapshot_id {
        Some("immutable lower-sector owner snapshot differs")
    } else {
        None
    };
    match detail {
        Some(detail) => Err(ExactCircuitOwnerCoverError::MixedOwnerScope {
            owner: owner.input_ordinal,
            detail,
        }),
        None => Ok(()),
    }
}

fn compare_prepared_owners(left: &PreparedOwner, right: &PreparedOwner) -> Ordering {
    left.region.cmp(&right.region).then_with(|| {
        for (left, right) in left
            .semantic
            .candidates()
            .iter()
            .zip(right.semantic.candidates())
        {
            let ordering = compare_exact_circuit_content(left.circuit(), right.circuit());
            if ordering != Ordering::Equal {
                return ordering;
            }
        }
        left.semantic
            .candidates()
            .len()
            .cmp(&right.semantic.candidates().len())
    })
}

fn prepare_terminals(
    chart: &SectorChart,
    terminals: impl IntoIterator<Item = IntegralKey>,
    limits: ExactCircuitOwnerCoverLimits,
) -> Result<Vec<ExactFiniteTerminalOwner>, ExactCircuitOwnerCoverError> {
    let mut prepared = try_vec(0, EXPLICIT_TERMINALS)?;
    for integral in terminals {
        let requested = checked_add(EXPLICIT_TERMINALS, prepared.len(), 1)?;
        check_limit(EXPLICIT_TERMINALS, requested, limits.max_explicit_terminals)?;
        let coordinate_cells =
            checked_mul(TERMINAL_COORDINATE_CELLS, requested, chart.sector().arity())?;
        check_limit(
            TERMINAL_COORDINATE_CELLS,
            coordinate_cells,
            limits.max_terminal_coordinate_cells,
        )?;
        prepared.try_reserve_exact(1).map_err(|_| {
            ExactCircuitOwnerCoverError::AllocationFailure {
                resource: EXPLICIT_TERMINALS,
                requested,
            }
        })?;
        prepared.push(ExactFiniteTerminalOwner {
            id: ExactFiniteTerminalOwnerId(0),
            point: chart.to_lattice(&integral)?,
            integral,
        });
    }
    prepared.sort_unstable_by(|left, right| left.point.cmp(&right.point));
    if prepared
        .windows(2)
        .any(|pair| pair[0].point == pair[1].point)
    {
        return Err(ExactCircuitOwnerCoverError::DuplicateTerminal);
    }
    for (ordinal, terminal) in prepared.iter_mut().enumerate() {
        terminal.id = ExactFiniteTerminalOwnerId(ordinal);
    }
    Ok(prepared)
}

fn uncovered_for_owners<'a>(
    carrier: &LatticeBox,
    arity: usize,
    owners: impl IntoIterator<Item = &'a ExactCircuitOwner>,
    limits: ExactCircuitOwnerCoverLimits,
) -> Result<UncoveredPartition, ExactCircuitOwnerCoverError> {
    let owners = owners.into_iter();
    let (lower, upper) = owners.size_hint();
    let capacity = upper.unwrap_or(lower).min(limits.max_owner_inputs);
    let mut boxes = try_vec(capacity, OWNER_INPUTS)?;
    for owner in owners {
        let requested = checked_add(OWNER_INPUTS, boxes.len(), 1)?;
        check_limit(OWNER_INPUTS, requested, limits.max_owner_inputs)?;
        if boxes.len() == boxes.capacity() {
            boxes.try_reserve_exact(1).map_err(|_| {
                ExactCircuitOwnerCoverError::AllocationFailure {
                    resource: OWNER_INPUTS,
                    requested,
                }
            })?;
        }
        boxes.push(owner.region().try_clone_fallible()?);
    }
    let abstract_partition =
        BoxCover::try_new(arity, boxes, limits.geometry)?.uncovered_partition()?;
    normalize_uncovered_to_carrier(
        abstract_partition,
        carrier,
        limits.max_finite_complement_points.max(1),
    )
}

fn validate_closure_carrier(
    carrier: &LatticeBox,
    full_carrier: &LatticeBox,
    arity: usize,
) -> Result<(), ExactCircuitOwnerCoverError> {
    if carrier.arity() != arity
        || full_carrier.arity() != arity
        || carrier.lower().iter().any(|&lower| lower != 0)
        || carrier
            .upper()
            .iter()
            .zip(full_carrier.upper())
            .any(|(&upper, &full_upper)| match (upper, full_upper) {
                (Some(upper), Some(full_upper)) => upper > full_upper,
                _ => true,
            })
    {
        return Err(ExactCircuitOwnerCoverError::Invariant(
            "closure carrier is not a finite origin-anchored subbox of the sector carrier",
        ));
    }
    Ok(())
}

/// Normalize the abstract `N^r` complement to the representable sector
/// carrier without destroying the symbolic free-axis representation used by
/// the boundary planner.
///
/// A box wholly beyond the carrier is discarded. Finite endpoints are
/// clamped. An abstractly unbounded endpoint is materialized when the
/// remaining carrier tail is small enough for the configured finite-point
/// budget; larger carrier-reaching intervals retain `None` as a symbolic
/// endpoint. Recompiling after an endpoint owner is inserted therefore drops
/// the now-purely-outside-carrier tail instead of manufacturing an impossible
/// closure obligation.
pub(super) fn normalize_uncovered_to_carrier(
    partition: UncoveredPartition,
    carrier: &LatticeBox,
    max_materialized_axis_width: usize,
) -> Result<UncoveredPartition, ExactCircuitOwnerCoverError> {
    let arity = carrier.arity();
    let mut normalized = try_vec(
        partition.boxes().len(),
        "carrier-normalized uncovered boxes",
    )?;
    for cell in partition.boxes() {
        let outside = cell
            .lower()
            .iter()
            .zip(carrier.upper())
            .any(|(&lower, &carrier_upper)| carrier_upper.is_some_and(|upper| lower > upper));
        if outside {
            continue;
        }
        let mut lower = try_vec(arity, "carrier-normalized lower endpoints")?;
        lower.extend_from_slice(cell.lower());
        let mut upper = try_vec(arity, "carrier-normalized upper endpoints")?;
        for ((&lower, &endpoint), &carrier_upper) in
            cell.lower().iter().zip(cell.upper()).zip(carrier.upper())
        {
            let carrier_upper = carrier_upper.ok_or(ExactCircuitOwnerCoverError::Invariant(
                "sector carrier unexpectedly has an unbounded endpoint",
            ))?;
            let clipped = endpoint.map_or(carrier_upper, |endpoint| endpoint.min(carrier_upper));
            let reaches_carrier = endpoint.is_none_or(|endpoint| endpoint >= carrier_upper);
            let width = u128::from(clipped) - u128::from(lower) + 1;
            upper.push(
                if reaches_carrier && width > max_materialized_axis_width as u128 {
                    // `None` is contextual here: the retained proof carrier, not
                    // mathematical infinity, is its authenticated endpoint. This
                    // preserves a scalable symbolic axis for the boundary
                    // planner even when an abstract finite owner endpoint lies
                    // beyond a deliberately inset source-safe carrier.
                    None
                } else {
                    Some(clipped)
                },
            );
        }
        normalized.push(LatticeBox::try_from_preallocated(lower, upper)?);
    }
    Ok(UncoveredPartition::new(
        normalized,
        partition.split_operations(),
    ))
}

fn validate_terminal_disjointness(
    context: &IndexedCoefficientContext,
    owners: &[ExactCircuitOwner],
    terminals: &[ExactFiniteTerminalOwner],
    limits: ExactCircuitOwnerCoverLimits,
    probes: &mut usize,
) -> Result<(), ExactCircuitOwnerCoverError> {
    for (terminal_ordinal, terminal) in terminals.iter().enumerate() {
        if let Some((owner, _)) = try_select_owner(
            context,
            owners,
            terminal.point(),
            terminal.integral(),
            limits,
            probes,
        )? {
            return Err(
                ExactCircuitOwnerCoverError::TerminalOverlapsDescendingOwner {
                    terminal: terminal_ordinal,
                    owner: owner.id.ordinal(),
                },
            );
        }
    }
    Ok(())
}

fn try_select_owner<'owner>(
    context: &IndexedCoefficientContext,
    owners: &'owner [ExactCircuitOwner],
    point: &LatticePoint,
    target: &IntegralKey,
    limits: ExactCircuitOwnerCoverLimits,
    probes: &mut usize,
) -> Result<
    Option<(
        &'owner ExactCircuitOwner,
        &'owner ExactCircuitSemanticCandidate,
    )>,
    ExactCircuitOwnerCoverError,
> {
    for owner in owners {
        if !owner.region().contains(point) {
            continue;
        }
        *probes = checked_add(POINT_OWNER_PROBES, *probes, 1)?;
        check_limit(POINT_OWNER_PROBES, *probes, limits.max_point_owner_probes)?;
        match owner
            .semantic
            .try_select_at(context, target.powers(), limits.guard_evaluation)
            .map_err(|error| ExactCircuitOwnerCoverError::SemanticSelection {
                owner: owner.id.ordinal(),
                error,
            })? {
            ExactCircuitSemanticSelection::Selected(candidate) => {
                return Ok(Some((owner, candidate)));
            }
            ExactCircuitSemanticSelection::Incomplete => {}
        }
    }
    Ok(None)
}

fn enumerate_finite_partition(
    partition: &crate::foundry::completion::UncoveredPartition,
    expected: usize,
) -> Result<Vec<LatticePoint>, ExactCircuitOwnerCoverError> {
    let mut points = try_vec(expected, FINITE_POINTS)?;
    for cell in partition.boxes() {
        let mut coordinates = try_vec(cell.arity(), FINITE_POINT_COORDINATE_CELLS)?;
        coordinates.extend_from_slice(cell.lower());
        let mut upper = try_vec(cell.arity(), FINITE_POINT_COORDINATE_CELLS)?;
        for &upper_endpoint in cell.upper() {
            upper.push(upper_endpoint.ok_or(ExactCircuitOwnerCoverError::Invariant(
                "finite complement enumeration reached an unbounded box",
            ))?);
        }
        loop {
            points.push(LatticePoint::try_new(coordinates.iter().copied())?);
            let mut advanced = false;
            for position in (0..coordinates.len()).rev() {
                if coordinates[position] < upper[position] {
                    coordinates[position] += 1;
                    coordinates[position + 1..].copy_from_slice(&cell.lower()[position + 1..]);
                    advanced = true;
                    break;
                }
            }
            if !advanced {
                break;
            }
        }
    }
    if points.len() != expected {
        return Err(ExactCircuitOwnerCoverError::Invariant(
            "finite complement enumeration changed its exact cardinality",
        ));
    }
    Ok(points)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ExactCircuitOwnerCoverError> {
    if requested > limit {
        Err(ExactCircuitOwnerCoverError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ExactCircuitOwnerCoverError> {
    left.checked_add(right)
        .ok_or(ExactCircuitOwnerCoverError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ExactCircuitOwnerCoverError> {
    left.checked_mul(right)
        .ok_or(ExactCircuitOwnerCoverError::ResourceCountOverflow { resource })
}

fn try_vec<T>(
    capacity: usize,
    resource: &'static str,
) -> Result<Vec<T>, ExactCircuitOwnerCoverError> {
    let mut values = Vec::new();
    values.try_reserve_exact(capacity).map_err(|_| {
        ExactCircuitOwnerCoverError::AllocationFailure {
            resource,
            requested: capacity,
        }
    })?;
    Ok(values)
}
