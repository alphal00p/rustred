//! Registered closure verifier for a fully published K6 sector-wave chain.

use std::sync::Arc;

use crate::family::IntegralKey;
use crate::foundry::artifact::{
    ArtifactCoverReplayLimits, FULL_RANK_ORBITS, K6_ARITY, K6_MASTER_TERMINAL_COUNT,
    MAX_PUBLISHED_K6_RULE_CELLS,
};
use crate::foundry::completion::frame::admission::ExactOwnerCoverStatus;
use crate::foundry::completion::source_discovery::ClosedSectorClosureWave;
use crate::foundry::completion::stratum::ImmutableOwnerSnapshot;
use crate::sector::{InteriorBounds, Mask};

use super::super::error::ArtifactError;
use super::super::model::{ArtifactValidationWitness, ClosedArtifact, CommonMassHomogeneityProof};
use super::ClosingArtifactCandidate;

mod source_authentication;

pub(super) use source_authentication::authenticate_canonical_source_views;
pub(super) use source_authentication::authenticate_canonical_source_views_with_limits;
#[cfg(test)]
pub(crate) use source_authentication::authenticate_rule_cell_source_views;

const WAVE_WIDTHS: [usize; 4] = [2, 2, 1, 1];
/// Recompile the persisted rule-cell/terminal geometry without trusting a
/// campaign wave transcript.  Algebraic replay and guard validity are checked
/// by the generic installer immediately afterwards; this pass establishes
/// that their target boxes plus explicit full-rank terminals leave no point
/// uncovered in any registered orbit representative.
pub(super) fn validate_persisted_cover(
    candidate: &ClosingArtifactCandidate,
    factorized_product_programs: &[Option<
        crate::foundry::artifact::factorized_product_moments::FactorizedProductMomentProgram,
    >],
    cover_limits: ArtifactCoverReplayLimits,
) -> Result<(), ArtifactError> {
    validate_candidate_shell(candidate)?;
    if candidate.supported_root_power_bounds.len() != candidate.arity {
        return Err(invalid(
            "persisted K6 root-power bounds have the wrong arity",
        ));
    }
    // Fail a caller-tightened arity policy before constructing even the fixed
    // six-coordinate universe or any per-sector box container.
    preflight_persisted_cover_inputs(candidate.arity, 0, cover_limits)?;
    use crate::foundry::completion::{BoxCover, LatticeBox, SectorChart};
    for orbit in FULL_RANK_ORBITS {
        let sector = Mask::try_from_indices(&orbit.representative)?;
        let chart = SectorChart::new(sector.clone());
        let universe = power_box_to_lattice(
            &chart,
            &sector,
            &candidate.supported_root_power_bounds,
            None,
        )?;
        let requested_boxes = candidate
            .rule_cells
            .iter()
            .filter(|cell| cell.application_domain().sector() == &sector)
            .count()
            .checked_add(
                candidate
                    .masters
                    .iter()
                    .filter(|master| {
                        sector
                            .active_bits()
                            .iter()
                            .zip(master.powers())
                            .all(|(&active, &power)| active == (power >= 1))
                    })
                    .count(),
            )
            .ok_or(ArtifactError::ResourceCountOverflow {
                resource: "persisted K6 cover boxes",
            })?;
        preflight_persisted_cover_inputs(candidate.arity, requested_boxes, cover_limits)?;
        let mut boxes = Vec::new();
        boxes
            .try_reserve_exact(requested_boxes)
            .map_err(|_| ArtifactError::AllocationFailure {
                resource: "persisted K6 cover boxes",
                requested: requested_boxes,
            })?;
        for cell in &candidate.rule_cells {
            if cell.application_domain().sector() == &sector {
                boxes.push(power_box_to_lattice(
                    &chart,
                    &sector,
                    cell.application_domain().bounds(),
                    Some(cell.rule().pivot().values()),
                )?);
            }
        }
        if factorized_product_programs.iter().any(Option::is_none) {
            return Err(invalid(
                "persisted K6 factorization lacks an authenticated product program",
            ));
        }
        for master in &candidate.masters {
            if sector
                .active_bits()
                .iter()
                .zip(master.powers())
                .all(|(&active, &power)| active == (power >= 1))
            {
                let point = chart
                    .to_lattice(master)
                    .map_err(|_| invalid("persisted K6 master is outside its sector chart"))?;
                boxes.push(
                    LatticeBox::try_new(
                        point.coordinates().iter().copied(),
                        point.coordinates().iter().copied().map(Some),
                    )
                    .map_err(|_| invalid("persisted K6 master box is invalid"))?,
                );
            }
        }
        debug_assert_eq!(boxes.len(), requested_boxes);
        let uncovered = BoxCover::try_new(candidate.arity, boxes, cover_limits.geometry())
            .and_then(|cover| cover.uncovered_within(universe))
            .map_err(map_cover_geometry_error)?;
        // K6 completeness of this any-one-route test is stronger than its
        // deliberately conservative generic shape.  Within each admitted K6
        // product-sector orbit, every transported sparse preimage is an
        // upward-closed intersection of lower affine inequalities and every
        // stabilizer route has the same rectangular upper carrier (the
        // K3-times-K1 stabilizer fixes its unique singleton factor).  Hence,
        // if a rectangular ordinary-rule remainder is covered by the route
        // union, its power-space lower corner belongs to one route and that
        // same route covers the complete rectangle.  Future product shapes
        // with route-dependent upper carriers may be safely rejected here
        // until a bounded exact union-discharge algorithm is registered.
        for uncovered_box in uncovered.boxes() {
            let domain = lattice_box_to_power_domain(&sector, uncovered_box)?;
            let product_covers_remainder = factorized_product_programs
                .iter()
                .filter_map(Option::as_ref)
                .any(|program| {
                    program.exact_application_domain().covers_domain(&domain)
                        || candidate
                            .canonicalizer
                            .as_ref()
                            .is_some_and(|canonicalizer| {
                                canonicalizer.routing_witnesses().any(|route| {
                                    program
                                        .exact_application_domain()
                                        .covers_transported_domain(
                                            &domain,
                                            route.source_for_target(),
                                        )
                                })
                            })
                });
            if !product_covers_remainder {
                return Err(invalid(
                    "persisted K6 rule cells do not close a registered full-rank orbit",
                ));
            }
        }
    }
    Ok(())
}

fn preflight_persisted_cover_inputs(
    arity: usize,
    requested_boxes: usize,
    limits: ArtifactCoverReplayLimits,
) -> Result<(), ArtifactError> {
    if arity > limits.max_arity {
        return Err(ArtifactError::ResourceLimit {
            resource: "completion coordinate arity",
            requested: arity,
            limit: limits.max_arity,
        });
    }
    if requested_boxes > limits.max_requested_boxes {
        return Err(ArtifactError::ResourceLimit {
            resource: "requested structural cover boxes",
            requested: requested_boxes,
            limit: limits.max_requested_boxes,
        });
    }
    let coordinate_cells = requested_boxes
        .checked_mul(arity)
        .and_then(|cells| cells.checked_mul(2))
        .ok_or(ArtifactError::ResourceCountOverflow {
            resource: "requested structural-cover coordinate cells",
        })?;
    if coordinate_cells > limits.max_requested_box_coordinate_cells {
        return Err(ArtifactError::ResourceLimit {
            resource: "requested structural-cover coordinate cells",
            requested: coordinate_cells,
            limit: limits.max_requested_box_coordinate_cells,
        });
    }
    Ok(())
}

fn map_cover_geometry_error(
    error: crate::foundry::completion::CompletionGeometryError,
) -> ArtifactError {
    use crate::foundry::completion::CompletionGeometryError;
    match error {
        CompletionGeometryError::ResourceCountOverflow { resource } => {
            ArtifactError::ResourceCountOverflow { resource }
        }
        CompletionGeometryError::ResourceLimit {
            resource,
            requested,
            limit,
        } => ArtifactError::ResourceLimit {
            resource,
            requested,
            limit,
        },
        CompletionGeometryError::AllocationFailure {
            resource,
            requested,
        } => ArtifactError::AllocationFailure {
            resource,
            requested,
        },
        _ => invalid("persisted K6 exact cover geometry is invalid"),
    }
}

fn lattice_box_to_power_domain(
    sector: &Mask,
    lattice: &crate::foundry::completion::LatticeBox,
) -> Result<crate::sector::SectorInteriorDomain, ArtifactError> {
    if lattice.arity() != sector.arity() {
        return Err(invalid("persisted K6 uncovered box has the wrong arity"));
    }
    let mut bounds = Vec::new();
    bounds
        .try_reserve_exact(sector.arity())
        .map_err(|_| ArtifactError::AllocationFailure {
            resource: "persisted K6 uncovered-domain bounds",
            requested: sector.arity(),
        })?;
    for (position, &active) in sector.active_bits().iter().enumerate() {
        let lower = lattice.lower()[position];
        let upper = lattice.upper()[position].ok_or(invalid(
            "persisted K6 uncovered box escaped its finite root",
        ))?;
        let (power_lower, power_upper) = if active {
            (
                chart_coordinate_to_power(true, lower)?,
                chart_coordinate_to_power(true, upper)?,
            )
        } else {
            (
                chart_coordinate_to_power(false, upper)?,
                chart_coordinate_to_power(false, lower)?,
            )
        };
        bounds.push(InteriorBounds::new(power_lower, power_upper));
    }
    crate::sector::SectorInteriorDomain::try_new(sector.clone(), bounds)
        .map_err(ArtifactError::from)
}

fn chart_coordinate_to_power(active: bool, coordinate: u64) -> Result<i64, ArtifactError> {
    if active {
        i64::try_from(coordinate)
            .ok()
            .and_then(|coordinate| coordinate.checked_add(1))
            .ok_or(invalid(
                "persisted K6 active chart endpoint is not representable",
            ))
    } else if coordinate == 1_u64 << 63 {
        Ok(i64::MIN)
    } else {
        i64::try_from(coordinate)
            .ok()
            .map(|coordinate| -coordinate)
            .ok_or(invalid(
                "persisted K6 inactive chart endpoint is not representable",
            ))
    }
}

fn power_box_to_lattice(
    chart: &crate::foundry::completion::SectorChart,
    sector: &Mask,
    bounds: &[InteriorBounds],
    shift: Option<&[i64]>,
) -> Result<crate::foundry::completion::LatticeBox, ArtifactError> {
    if bounds.len() != sector.arity() || shift.is_some_and(|shift| shift.len() != sector.arity()) {
        return Err(invalid("persisted K6 target box has the wrong arity"));
    }
    let mut lower = Vec::new();
    let mut upper = Vec::new();
    lower
        .try_reserve_exact(bounds.len())
        .map_err(|_| ArtifactError::AllocationFailure {
            resource: "persisted K6 lattice-lower coordinates",
            requested: bounds.len(),
        })?;
    upper
        .try_reserve_exact(bounds.len())
        .map_err(|_| ArtifactError::AllocationFailure {
            resource: "persisted K6 lattice-upper coordinates",
            requested: bounds.len(),
        })?;
    for (position, (&active, bound)) in sector.active_bits().iter().zip(bounds).enumerate() {
        let displacement = shift.map_or(0, |shift| shift[position]);
        let mut low_power = bound
            .lower()
            .checked_add(displacement)
            .ok_or(invalid("persisted K6 target lower endpoint overflowed"))?;
        let mut high_power = bound
            .upper()
            .checked_add(displacement)
            .ok_or(invalid("persisted K6 target upper endpoint overflowed"))?;
        if shift.is_none() {
            if active {
                low_power = low_power.max(1);
            } else {
                high_power = high_power.min(0);
            }
        }
        if low_power > high_power {
            return Err(invalid("persisted K6 root box misses a registered sector"));
        }
        let low_key = IntegralKey::try_new((0..sector.arity()).map(|slot| {
            if slot == position {
                low_power
            } else if sector.active_bits()[slot] {
                1
            } else {
                0
            }
        }))?;
        let high_key = IntegralKey::try_new((0..sector.arity()).map(|slot| {
            if slot == position {
                high_power
            } else if sector.active_bits()[slot] {
                1
            } else {
                0
            }
        }))?;
        let low_coordinate = chart
            .to_lattice(&low_key)
            .map_err(|_| invalid("persisted K6 target lower endpoint leaves its sector"))?
            .coordinates()[position];
        let high_coordinate = chart
            .to_lattice(&high_key)
            .map_err(|_| invalid("persisted K6 target upper endpoint leaves its sector"))?
            .coordinates()[position];
        if active {
            lower.push(low_coordinate);
            upper.push(Some(high_coordinate));
        } else {
            lower.push(high_coordinate);
            upper.push(Some(low_coordinate));
        }
    }
    crate::foundry::completion::LatticeBox::try_new(lower, upper)
        .map_err(|_| invalid("persisted K6 target box is invalid"))
}

/// Validate proof-chain topology and transfer every retained executable cell
/// into the generic candidate before generic source/descent replay runs.
pub(super) fn prepare(
    candidate: &mut ClosingArtifactCandidate,
    waves: &[ClosedSectorClosureWave],
) -> Result<(), ArtifactError> {
    validate_candidate_shell(candidate)?;
    validate_wave_chain_root(candidate, waves)?;

    let mut lower = vec![None; candidate.arity];
    let mut upper = vec![None; candidate.arity];
    let mut cell_count = 0usize;
    let mut orbit_start = 0usize;
    let mut predecessor = waves
        .first()
        .ok_or(invalid("published K6 wave chain is empty"))?
        .predecessor();

    for (wave_ordinal, (wave, &expected_width)) in waves.iter().zip(WAVE_WIDTHS.iter()).enumerate()
    {
        if !wave.predecessor().same_authority_as(predecessor)
            || wave.layers().len() != expected_width
            || wave.successor().closed_layer_count()
                != wave
                    .predecessor()
                    .closed_layer_count()
                    .checked_add(expected_width)
                    .ok_or(invalid("published K6 predecessor layer count overflowed"))?
        {
            return Err(invalid(
                "published K6 wave has a foreign predecessor or wrong width",
            ));
        }
        let expected_end = orbit_start
            .checked_add(expected_width)
            .ok_or(invalid("published K6 orbit manifest overflowed"))?;
        let mut expected_sectors = FULL_RANK_ORBITS[orbit_start..expected_end]
            .iter()
            .map(|orbit| Mask::try_from_indices(&orbit.representative))
            .collect::<Result<Vec<_>, _>>()?;
        expected_sectors.sort();

        for (layer_ordinal, (layer, expected_sector)) in wave
            .layers()
            .iter()
            .zip(expected_sectors.iter())
            .enumerate()
        {
            if layer.sector() != expected_sector
                || layer.ordering() != candidate.ordering
                || !layer
                    .predecessor_snapshot()
                    .same_authority_as(wave.predecessor())
                || layer.family_fingerprint() != candidate.family.fingerprint()
                || layer.context_fingerprint() != candidate.context.fingerprint()
            {
                return Err(ArtifactError::InvalidOrderingAuthority {
                    detail: "a published K6 layer differs from its manifest or wave authority",
                    ordinal: wave_ordinal * 2 + layer_ordinal,
                });
            }
            let executable = layer.executable_cover().executable_cover();
            let proof = executable.proof_cover();
            if proof.status() != ExactOwnerCoverStatus::Closed
                || !proof.uncovered_partition().boxes().is_empty()
                || !proof.missing_terminals().is_empty()
                || !proof.guard_incomplete_owners().is_empty()
                || proof.owner_snapshot_id() != wave.predecessor().id()
            {
                return Err(invalid(
                    "a published K6 layer no longer proves an exact zero-uncovered cover",
                ));
            }
            validate_layer_domain(
                layer.sector(),
                layer.proven_domain().bounds(),
                &mut lower,
                &mut upper,
            )?;
            if executable.owners().is_empty() && executable.terminals().is_empty() {
                // A factorized carrier needs no ordinary rule cell or master.
                // Re-authenticate the exact published domain against the
                // strongly retained predecessor whose compiled product
                // programs supplied the zero-uncovered proof.
                if !wave
                    .predecessor()
                    .authenticates_same_sector_domain(candidate.ordering, layer.proven_domain())
                {
                    return Err(invalid(
                        "a zero-cell K6 layer is not completely owned by its retained predecessor",
                    ));
                }
            } else {
                validate_terminal_ownership(candidate, expected_sector, executable.terminals())?;
            }
            for owner in executable.owners() {
                cell_count = cell_count
                    .checked_add(owner.executable_candidates().len())
                    .ok_or(invalid("published K6 rule-cell count overflowed"))?;
                if cell_count > MAX_PUBLISHED_K6_RULE_CELLS {
                    return Err(invalid(
                        "published K6 rule-cell count exceeds the installation limit",
                    ));
                }
            }
        }
        predecessor = wave.successor();
        orbit_start = expected_end;
    }
    if waves.len() != WAVE_WIDTHS.len()
        || orbit_start != FULL_RANK_ORBITS.len()
        || predecessor.closed_layer_count() != FULL_RANK_ORBITS.len()
    {
        return Err(invalid(
            "published K6 waves do not consume the complete six-orbit manifest",
        ));
    }

    validate_coordinate_transitivity(candidate)?;
    let common_lower = common_symmetric_endpoint(&lower)?;
    let common_upper = common_symmetric_endpoint(&upper)?;
    if common_lower > 0 || common_upper < 1 {
        return Err(ArtifactError::InvalidRuleShape {
            detail: "published K6 root-power bounds do not span sector boundaries",
        });
    }
    let mut bounds = Vec::new();
    bounds
        .try_reserve_exact(candidate.arity)
        .map_err(|_| invalid("could not reserve published K6 root-power bounds"))?;
    bounds.resize(
        candidate.arity,
        InteriorBounds::new(common_lower, common_upper),
    );

    let mut cells = Vec::new();
    cells
        .try_reserve_exact(cell_count)
        .map_err(|_| invalid("could not reserve published K6 rule-cell owners"))?;
    for wave in waves {
        for layer in wave.layers() {
            for owner in layer.executable_cover().executable_cover().owners() {
                cells.extend(
                    owner
                        .executable_candidates()
                        .iter()
                        .map(|candidate| candidate.cell_owner().clone()),
                );
            }
        }
    }
    if cells.len() != cell_count || cells.is_empty() {
        return Err(invalid(
            "published K6 executable-cell ownership census changed during assembly",
        ));
    }
    candidate.supported_root_power_bounds = bounds.into_boxed_slice();
    candidate.rule_cells = cells;
    Ok(())
}

/// Seal with product programs compiled once at the untrusted load boundary.
/// Persisted-cover validation consumes these exact authenticated domains, and
/// the hot artifact retains the same programs without a second compilation.
pub(super) fn seal_with_programs(
    candidate: ClosingArtifactCandidate,
    factorized_product_programs: Vec<
        Option<
            crate::foundry::artifact::factorized_product_moments::FactorizedProductMomentProgram,
        >,
    >,
) -> Result<ClosedArtifact, ArtifactError> {
    validate_candidate_shell(&candidate)?;
    if candidate.supported_root_power_bounds.len() != 6 || candidate.rule_cells.is_empty() {
        return Err(invalid("prepared K6 artifact payload is incomplete"));
    }
    let expected_rows = [
        "ordinary-ibp:0:0",
        "ordinary-ibp:0:1",
        "ordinary-ibp:0:2",
        "ordinary-ibp:1:0",
        "ordinary-ibp:1:1",
        "ordinary-ibp:1:2",
        "ordinary-ibp:2:0",
        "ordinary-ibp:2:1",
        "ordinary-ibp:2:2",
    ];
    if candidate
        .source_relations
        .iter()
        .map(|row| row.row_id().stable_string())
        .ne(expected_rows.map(str::to_owned))
    {
        return Err(ArtifactError::InvalidReplayEvidence {
            detail: "the K6 artifact does not retain the canonical nine-row ordinary source manifest",
        });
    }

    let replayed_source_rows =
        checked_cell_sum(&candidate, |cell| cell.rule().replay().source_rows_used())?;
    let replayed_shift_columns = checked_cell_sum(&candidate, |cell| {
        cell.rule().replay().shift_columns_checked()
    })?;
    let guards = checked_cell_sum(&candidate, |cell| cell.guards().len())?;
    if factorized_product_programs.len() != candidate.factorization_rules.len()
        || factorized_product_programs.iter().any(Option::is_none)
    {
        return Err(invalid(
            "the K6 artifact lacks an exact dependency-root preimage product-moment executor",
        ));
    }
    let validation = ArtifactValidationWitness::new(
        candidate.source_relations.len(),
        replayed_source_rows,
        replayed_shift_columns,
        candidate.rule_cells.len(),
        guards,
        candidate.masters.len(),
        candidate.zero_sectors.len(),
    );
    Ok(ClosedArtifact {
        schema: candidate.schema,
        algorithm_id: candidate.algorithm_id,
        arity: candidate.arity,
        ordering: candidate.ordering,
        supported_root_power_bounds: candidate.supported_root_power_bounds,
        family_fingerprint: candidate.family.fingerprint_owner(),
        family: candidate.family,
        context: candidate.context,
        source_relations: candidate.source_relations,
        rules: candidate.rules,
        rule_cells: candidate.rule_cells,
        canonicalizer: candidate.canonicalizer,
        dependencies: candidate.dependencies,
        factorization_rules: candidate.factorization_rules,
        factorized_product_programs,
        masters: candidate.masters,
        zero_sectors: candidate.zero_sectors,
        common_mass_homogeneity: candidate.common_mass_homogeneity,
        validation,
    })
}

fn validate_candidate_shell(candidate: &ClosingArtifactCandidate) -> Result<(), ArtifactError> {
    if candidate.algorithm_id != super::super::three_loop::ALGORITHM_ID
        || candidate.arity != K6_ARITY
        || candidate.source_relations.len() != 9
        || !candidate.rules.is_empty()
        || candidate.dependencies.len() != 2
        || candidate.factorization_rules.len() != 3
        || candidate.masters.len() != K6_MASTER_TERMINAL_COUNT
        || candidate.common_mass_homogeneity
            != Some(CommonMassHomogeneityProof::UniformVacuumMassSquared)
    {
        return Err(ArtifactError::UnsupportedClosureShape);
    }
    Ok(())
}

fn validate_wave_chain_root(
    candidate: &ClosingArtifactCandidate,
    waves: &[ClosedSectorClosureWave],
) -> Result<(), ArtifactError> {
    if waves.len() != WAVE_WIDTHS.len() {
        return Err(invalid(
            "published K6 wave count differs from the registered manifest",
        ));
    }
    let authority = Arc::new(
        super::super::three_loop::derive_k6_terminal_authority_with_ordering(candidate.ordering)?,
    );
    let expected =
        ImmutableOwnerSnapshot::try_from_terminal_authority(authority, Default::default())
            .map_err(|_| invalid("could not reconstruct the authenticated K6 root predecessor"))?;
    let actual = waves
        .first()
        .ok_or(invalid("published K6 wave chain is empty"))?
        .predecessor();
    if actual != &expected
        || actual.closed_layer_count() != 0
        || actual.family_fingerprint() != candidate.family.fingerprint()
        || actual.context_fingerprint() != candidate.context.fingerprint()
        || actual.canonicalizer_ordering() != Some(candidate.ordering)
    {
        return Err(invalid(
            "published K6 waves do not descend from the registered terminal root",
        ));
    }
    Ok(())
}

fn validate_layer_domain(
    sector: &Mask,
    bounds: &[InteriorBounds],
    lower: &mut [Option<i64>],
    upper: &mut [Option<i64>],
) -> Result<(), ArtifactError> {
    if bounds.len() != sector.arity() {
        return Err(invalid("published K6 layer domain has the wrong arity"));
    }
    for (coordinate, (&active, bound)) in sector.active_bits().iter().zip(bounds).enumerate() {
        if active {
            if bound.lower() != 1 {
                return Err(invalid(
                    "published K6 active-sector domain does not start at one",
                ));
            }
            merge_endpoint(&mut upper[coordinate], bound.upper())?;
        } else {
            if bound.upper() != 0 {
                return Err(invalid(
                    "published K6 inactive-sector domain does not end at zero",
                ));
            }
            merge_endpoint(&mut lower[coordinate], bound.lower())?;
        }
    }
    Ok(())
}

fn merge_endpoint(slot: &mut Option<i64>, endpoint: i64) -> Result<(), ArtifactError> {
    if slot.is_some_and(|retained| retained != endpoint) {
        return Err(invalid(
            "published K6 layers disagree on a shared root-power endpoint",
        ));
    }
    *slot = Some(endpoint);
    Ok(())
}

fn common_symmetric_endpoint(endpoints: &[Option<i64>]) -> Result<i64, ArtifactError> {
    let common = endpoints.iter().flatten().next().copied().ok_or(invalid(
        "published K6 layers provide no source-safe power endpoint",
    ))?;
    if endpoints
        .iter()
        .flatten()
        .any(|&endpoint| endpoint != common)
    {
        return Err(invalid(
            "coordinate-symmetric K6 layers disagree on a source-safe power endpoint",
        ));
    }
    Ok(common)
}

fn validate_coordinate_transitivity(
    candidate: &ClosingArtifactCandidate,
) -> Result<(), ArtifactError> {
    let canonicalizer = candidate
        .canonicalizer
        .as_ref()
        .ok_or(ArtifactError::InvalidCanonicalizer)?;
    let mut reached = vec![false; candidate.arity];
    for permutation in canonicalizer.group_elements() {
        let Some(&image) = permutation.first() else {
            return Err(ArtifactError::InvalidCanonicalizer);
        };
        let Some(slot) = reached.get_mut(image) else {
            return Err(ArtifactError::InvalidCanonicalizer);
        };
        *slot = true;
    }
    if reached.iter().all(|&is_reached| is_reached) {
        Ok(())
    } else {
        Err(ArtifactError::InvalidCanonicalizer)
    }
}

fn validate_terminal_ownership(
    candidate: &ClosingArtifactCandidate,
    sector: &Mask,
    terminals: &[IntegralKey],
) -> Result<(), ArtifactError> {
    let representative =
        IntegralKey::try_new(sector.active_bits().iter().map(|&active| i64::from(active)))?;
    let canonicalizer = candidate
        .canonicalizer
        .as_ref()
        .ok_or(ArtifactError::InvalidCanonicalizer)?;
    let expected = canonicalizer
        .canonicalize(&representative)?
        .canonical()
        .clone();
    if !candidate.masters.contains(&expected)
        || !terminals.iter().any(|terminal| {
            canonicalizer
                .canonicalize(terminal)
                .is_ok_and(|canonical| canonical.canonical() == &expected)
        })
        || terminals.iter().any(|terminal| {
            canonicalizer
                .canonicalize(terminal)
                .map_or(true, |canonical| {
                    !candidate.masters.contains(canonical.canonical())
                })
        })
    {
        return Err(ArtifactError::InvalidMasterManifest);
    }
    Ok(())
}

fn checked_cell_sum(
    candidate: &ClosingArtifactCandidate,
    value: impl Fn(&crate::foundry::cell::RuleCell) -> usize,
) -> Result<usize, ArtifactError> {
    candidate.rule_cells.iter().try_fold(0usize, |sum, cell| {
        sum.checked_add(value(cell))
            .ok_or(ArtifactError::InvalidReplayEvidence {
                detail: "K6 artifact validation census overflowed",
            })
    })
}

const fn invalid(detail: &'static str) -> ArtifactError {
    ArtifactError::InvalidClosurePublication { detail }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::algebra::IndexedCoefficientContext;
    use crate::foundry::artifact::derive_two_loop_unit_mass_sunset;
    use crate::foundry::artifact::model::{ArtifactSchemaVersion, CommonMassHomogeneityProof};
    use crate::foundry::artifact::{ArtifactCoverReplayLimits, K6_ARITY};
    use crate::foundry::cell::SourceViewConstruction;
    use crate::foundry::completion::CompletionGeometryError;
    use crate::identity::{
        IdentityConditionSource, IntegralShift, ParametricIbpConfig, ParametricIbpGenerator,
        ParametricNonZeroCondition, ParametricRelation, RelationBuilder, RelationLimits,
    };
    use crate::sector::{InteriorBounds, OrderingPolicy};

    use super::super::{
        ClosingArtifactCandidate, compile_factorized_product_programs, validate_generic_bindings,
    };
    use super::{
        WAVE_WIDTHS, authenticate_rule_cell_source_views, common_symmetric_endpoint,
        map_cover_geometry_error, merge_endpoint, preflight_persisted_cover_inputs,
        seal_with_programs,
    };

    #[test]
    fn persisted_cover_preflight_accepts_more_than_the_legacy_box_ceiling() {
        let former_ceiling_plus_one = 65_537;
        assert!(
            preflight_persisted_cover_inputs(
                K6_ARITY,
                former_ceiling_plus_one,
                ArtifactCoverReplayLimits::default(),
            )
            .is_ok()
        );
        let mut tightened = ArtifactCoverReplayLimits::default();
        tightened.max_requested_boxes = former_ceiling_plus_one - 1;
        assert!(
            preflight_persisted_cover_inputs(K6_ARITY, former_ceiling_plus_one, tightened).is_err()
        );

        let mut overflow = ArtifactCoverReplayLimits::default();
        overflow.max_requested_boxes = usize::MAX;
        overflow.max_requested_box_coordinate_cells = usize::MAX;
        assert_eq!(
            preflight_persisted_cover_inputs(K6_ARITY, usize::MAX, overflow),
            Err(
                crate::foundry::artifact::ArtifactError::ResourceCountOverflow {
                    resource: "requested structural-cover coordinate cells",
                }
            )
        );
    }

    #[test]
    fn persisted_cover_geometry_resource_errors_keep_typed_payloads() {
        assert_eq!(
            map_cover_geometry_error(CompletionGeometryError::ResourceCountOverflow {
                resource: "synthetic cover count",
            }),
            crate::foundry::artifact::ArtifactError::ResourceCountOverflow {
                resource: "synthetic cover count",
            }
        );
        assert_eq!(
            map_cover_geometry_error(CompletionGeometryError::AllocationFailure {
                resource: "synthetic cover allocation",
                requested: 17,
            }),
            crate::foundry::artifact::ArtifactError::AllocationFailure {
                resource: "synthetic cover allocation",
                requested: 17,
            }
        );
    }

    #[test]
    fn registered_wave_widths_cover_every_k6_orbit_once() {
        assert_eq!(WAVE_WIDTHS.iter().sum::<usize>(), 6);
        assert_eq!(
            WAVE_WIDTHS,
            crate::foundry::campaign::K6_FULL_RANK_WAVE_WIDTHS
        );
    }

    #[test]
    fn shared_domain_endpoint_merge_is_exact_and_fail_closed() {
        let mut endpoint = None;
        merge_endpoint(&mut endpoint, 17).unwrap();
        merge_endpoint(&mut endpoint, 17).unwrap();
        assert_eq!(endpoint, Some(17));
        assert!(merge_endpoint(&mut endpoint, 18).is_err());
        assert_eq!(
            common_symmetric_endpoint(&[Some(-17), None, Some(-17)]).unwrap(),
            -17,
        );
        assert!(common_symmetric_endpoint(&[Some(-17), None, Some(-18)]).is_err());
    }

    #[test]
    fn derived_root_bounds_precede_generic_validation_without_false_publication() {
        let ordering = OrderingPolicy::default();
        let authority =
            crate::foundry::artifact::derive_k6_terminal_authority_with_ordering(ordering).unwrap();
        let parts = authority.into_artifact_parts();
        let generator = ParametricIbpGenerator::try_new_with_config(
            &parts.family,
            ParametricIbpConfig::default(),
        )
        .unwrap();
        let prepared = generator.prepare_ordinary_ibp().unwrap();
        let rows = (0..prepared.len())
            .map(|ordinal| prepared.generate(ordinal))
            .collect();
        let source_relations = prepared.complete(rows).unwrap().into_relations();
        drop(generator);
        let mut candidate = ClosingArtifactCandidate {
            schema: ArtifactSchemaVersion::CURRENT,
            algorithm_id: crate::foundry::artifact::three_loop::ALGORITHM_ID,
            arity: parts.arity,
            ordering,
            supported_root_power_bounds: Vec::new().into_boxed_slice(),
            family: parts.family,
            context: parts.context,
            source_relations,
            rules: Vec::new(),
            rule_cells: Vec::new(),
            canonicalizer: parts.canonicalizer,
            dependencies: parts.dependencies,
            factorization_rules: parts.factorization_rules,
            masters: parts.masters,
            zero_sectors: parts.zero_sectors,
            common_mass_homogeneity: Some(CommonMassHomogeneityProof::UniformVacuumMassSquared),
        };

        assert!(validate_generic_bindings(&candidate).is_err());
        candidate.supported_root_power_bounds =
            vec![InteriorBounds::new(i64::MIN, i64::MAX - 1); 6].into_boxed_slice();
        validate_generic_bindings(&candidate).unwrap();
        assert!(
            compile_factorized_product_programs(&candidate)
                .and_then(|programs| seal_with_programs(candidate, programs))
                .is_err(),
            "a validated shell without published executable owners must never seal",
        );
    }

    #[test]
    fn canonical_source_join_rejects_forged_source_ordinal_row_offset_and_symmetry() {
        assert_sunset_source_mutant_rejected(|artifact, cell_ordinal| {
            let source = artifact.rule_cells[cell_ordinal].sources().provenance()[0]
                .translated()
                .source_ordinal();
            Arc::get_mut(&mut artifact.rule_cells[cell_ordinal])
                .unwrap()
                .replace_translated_source_ordinal_for_artifact_test(0, (source + 1) % 4);
        });
        assert_sunset_source_mutant_rejected(|artifact, cell_ordinal| {
            let foreign_row = artifact.source_relations[1].row_id().clone();
            Arc::get_mut(&mut artifact.rule_cells[cell_ordinal])
                .unwrap()
                .replace_translated_source_row_for_artifact_test(0, foreign_row);
        });
        assert_sunset_source_mutant_rejected(|artifact, cell_ordinal| {
            let arity = artifact.arity;
            Arc::get_mut(&mut artifact.rule_cells[cell_ordinal])
                .unwrap()
                .replace_translated_source_offset_for_artifact_test(
                    0,
                    IntegralShift::try_new(
                        std::iter::once(37).chain(std::iter::repeat_n(0, arity - 1)),
                    )
                    .unwrap(),
                );
        });
        assert_sunset_source_mutant_rejected(|artifact, cell_ordinal| {
            Arc::get_mut(&mut artifact.rule_cells[cell_ordinal])
                .unwrap()
                .attach_unregistered_source_symmetry_for_artifact_test(0, 0);
        });
    }

    #[test]
    fn canonical_source_join_rejects_mutated_coefficients_and_conditions() {
        assert_sunset_source_mutant_rejected(|artifact, cell_ordinal| {
            let forged = rebuild_relation(
                &artifact.rule_cells[cell_ordinal].sources().relations()[0],
                &artifact.context,
                RelationMutation::FirstCoefficient,
            );
            Arc::get_mut(&mut artifact.rule_cells[cell_ordinal])
                .unwrap()
                .replace_source_relation_for_artifact_test(0, forged);
        });
        assert_sunset_source_mutant_rejected(|artifact, cell_ordinal| {
            let forged = rebuild_relation(
                &artifact.rule_cells[cell_ordinal].sources().relations()[0],
                &artifact.context,
                RelationMutation::AdditionalCondition,
            );
            Arc::get_mut(&mut artifact.rule_cells[cell_ordinal])
                .unwrap()
                .replace_source_relation_for_artifact_test(0, forged);
        });
    }

    #[test]
    fn canonical_source_join_authenticates_residual_original_before_projection_replay() {
        let mut artifact = derive_two_loop_unit_mass_sunset().unwrap();
        let cell_ordinal = artifact
            .rule_cells
            .iter()
            .position(|cell| {
                matches!(
                    cell.sources().construction(),
                    SourceViewConstruction::ResidualProjection(_)
                )
            })
            .expect("the registered sunset contains a residual source projection");
        let SourceViewConstruction::ResidualProjection(evidence) =
            artifact.rule_cells[cell_ordinal].sources().construction()
        else {
            unreachable!();
        };
        let forged = rebuild_relation(
            &evidence.original_relations()[0],
            &artifact.context,
            RelationMutation::FirstCoefficient,
        );
        Arc::get_mut(&mut artifact.rule_cells[cell_ordinal])
            .unwrap()
            .replace_residual_original_relation_for_artifact_test(0, forged);

        assert_sunset_source_authentication_fails(&artifact);
    }

    fn assert_sunset_source_mutant_rejected(
        mutate: impl FnOnce(&mut crate::foundry::artifact::ClosedArtifact, usize),
    ) {
        let mut artifact = derive_two_loop_unit_mass_sunset().unwrap();
        let cell_ordinal = artifact
            .rule_cells
            .iter()
            .position(|cell| {
                !matches!(
                    cell.sources().construction(),
                    SourceViewConstruction::ResidualProjection(_)
                )
            })
            .expect("the registered sunset contains a direct source view");
        mutate(&mut artifact, cell_ordinal);
        assert_sunset_source_authentication_fails(&artifact);
    }

    fn assert_sunset_source_authentication_fails(
        artifact: &crate::foundry::artifact::ClosedArtifact,
    ) {
        let generator = ParametricIbpGenerator::try_new_with_config(
            &artifact.family,
            ParametricIbpConfig::default(),
        )
        .unwrap();
        let prepared = generator.prepare_ordinary_ibp().unwrap();
        let rows = (0..prepared.len())
            .map(|ordinal| prepared.generate(ordinal))
            .collect();
        let completed = prepared.complete(rows).unwrap();
        assert!(
            authenticate_rule_cell_source_views(&generator, &completed, &artifact.rule_cells,)
                .is_err()
        );
    }

    #[derive(Clone, Copy)]
    enum RelationMutation {
        FirstCoefficient,
        AdditionalCondition,
    }

    fn rebuild_relation(
        original: &ParametricRelation,
        context: &IndexedCoefficientContext,
        mutation: RelationMutation,
    ) -> ParametricRelation {
        let limits = RelationLimits::default();
        let mut builder = RelationBuilder::new(
            original.family_fingerprint_owner(),
            original.row_id().clone(),
            context,
        );
        for condition in original.nonzero_conditions() {
            builder
                .add_sealed_nonzero_condition(context, condition.clone(), limits)
                .unwrap();
        }
        for (ordinal, (shift, coefficient)) in original.terms().iter().enumerate() {
            let coefficient =
                if ordinal == 0 && matches!(mutation, RelationMutation::FirstCoefficient) {
                    context
                        .neg_with_limits(coefficient, limits.arithmetic.exact_algebra)
                        .unwrap()
                } else {
                    coefficient.clone()
                };
            builder
                .add_sealed_term(context, shift.clone(), coefficient, limits)
                .unwrap();
        }
        if matches!(mutation, RelationMutation::AdditionalCondition) {
            let index = context.index(0).unwrap();
            let polynomial = context
                .numerator_condition_with_limits(&index, limits.arithmetic.exact_algebra)
                .unwrap();
            let condition = ParametricNonZeroCondition::from_authenticated_with_limits(
                polynomial,
                [IdentityConditionSource::IndexTranslation {
                    offset: vec![97; context.index_count()].into_boxed_slice(),
                }],
                limits.identity_conditions,
            )
            .unwrap();
            builder
                .add_sealed_nonzero_condition(context, condition, limits)
                .unwrap();
        }
        let rebuilt = builder.finish();
        match mutation {
            RelationMutation::FirstCoefficient => {
                assert_ne!(rebuilt.terms(), original.terms());
                assert_eq!(rebuilt.nonzero_conditions(), original.nonzero_conditions());
            }
            RelationMutation::AdditionalCondition => {
                assert_eq!(rebuilt.terms(), original.terms());
                assert_ne!(rebuilt.nonzero_conditions(), original.nonzero_conditions());
            }
        }
        rebuilt
    }
}
