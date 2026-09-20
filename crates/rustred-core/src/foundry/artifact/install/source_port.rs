//! Registered in-memory source-directed producer for the existing artifact.
//! Its input rules were atomically constructed from full original-source
//! replay. This gate checks their bindings and either complete unbounded cover
//! or exact total-excess cover with a closed, immutable successor envelope;
//! it never accepts old wave metadata or substitutes finite sampling.
use std::collections::{BTreeMap, BTreeSet};

use crate::foundry::cell::{RuleCell, RuleCellGuardDomainProof};
use crate::foundry::completion::{CompletionGeometryLimits, LatticeBox};
use crate::sector::Mask;

use super::super::error::ArtifactError;
use super::super::model::{ArtifactValidationWitness, ClosedArtifact, CommonMassHomogeneityProof};
use super::super::source_port::predicate_cover::{
    PredicateCoverError, PredicateCoverLimits, PredicateCoveragePiece, certify_predicate_cover,
    certify_predicate_cover_up_to_degree,
};
use super::super::source_port::scope;
use super::super::source_port::{
    EnvelopeBudget, SourcePortAuditError, SourcePortSuccessorSnapshot, SourcePortSuccessorStage,
    visit_successor_degrees,
};
use super::{ClosingArtifactCandidate, ReplayProducer};

pub(in crate::foundry::artifact) const ALGORITHM_ID: &str =
    super::super::COMPLETE_VACUUM_SOURCE_PORT_ALGORITHM_ID;

struct TotalExcessProposal {
    entry: scope::EntryScope,
    degrees: Vec<(Mask, u64)>,
}

/// Minted only after actual-cell coverage and all successor obligations pass.
pub(in crate::foundry::artifact) struct VerifiedTotalExcessScope {
    entry: scope::EntryScope,
    degrees: BTreeMap<Mask, u64>,
}

impl VerifiedTotalExcessScope {
    pub(in crate::foundry::artifact) fn into_parts(
        self,
    ) -> (scope::EntryScope, BTreeMap<Mask, u64>) {
        (self.entry, self.degrees)
    }
}

pub(super) fn validate_combined_rule(cell: &RuleCell) -> Result<(), ArtifactError> {
    let rule = cell.rule();
    let fail = || ArtifactError::InvalidReplayEvidence {
        detail: "combined original-domain replay is absent or not bound to its rule/cell",
    };
    let evidence = rule
        .replay_evidence()
        .combined_original_domain()
        .ok_or_else(fail)?;
    let predicated =
        evidence.affine_application_domain().is_some() || !evidence.affine_exclusions().is_empty();
    if predicated
        && cell.guard_domain_proof() != RuleCellGuardDomainProof::ReplayAuthorizedOriginalPredicate
    {
        return Err(fail());
    }
    for domain in evidence.affine_application_domain().into_iter().chain(
        evidence
            .affine_exclusions()
            .iter()
            .map(|domain| domain.as_ref()),
    ) {
        if !domain.is_authenticated()
            || domain.sector() != rule.sector().active_bits()
            || domain.fixed().len() != rule.sector().arity()
        {
            return Err(fail());
        }
    }
    if let Some(target) = evidence.affine_application_domain() {
        for (axis, value) in target.fixed().iter().enumerate() {
            if value.map(i64::from)
                != cell
                    .fixed_restrictions()
                    .iter()
                    .find(|item| item.position() == axis)
                    .map(|item| item.value())
            {
                return Err(fail());
            }
        }
    }
    if rule.anchor().is_some()
        || rule.replay().is_some()
        || !rule.elimination_pivot_guards().is_empty()
        || rule.pivot().values().iter().any(|value| *value != 0)
        || evidence.sector() != rule.sector()
        || evidence.fixed_restrictions() != cell.fixed_restrictions()
        || evidence.application_boxes().len() != 1
        || evidence.source_rows_used() == 0
        || evidence.source_rows_used() != rule.source_combination().len()
        || evidence.shift_columns_checked() == 0
        || rule
            .sector_monotone_admission()
            .is_none_or(|admission| admission.domain() != cell.application_domain())
        || !cell.pruned_rhs_ordinals().is_empty()
        || cell.terms().len() != rule.right_hand_side().len()
    {
        return Err(fail());
    }
    let mut seen = BTreeSet::new();
    for contribution in rule.source_combination() {
        let ordinal = contribution.source_ordinal();
        let source = cell.sources().relations().get(ordinal).ok_or_else(fail)?;
        if source.row_id() != contribution.row_id()
            || contribution.coefficient().is_zero()
            || !seen.insert(ordinal)
        {
            return Err(fail());
        }
    }
    let piece = &evidence.application_boxes()[0];
    if piece.arity() != rule.sector().arity() {
        return Err(fail());
    }
    for (axis, bounds) in cell.application_domain().bounds().iter().enumerate() {
        let (lower, upper) = if rule.sector().active_bits()[axis] {
            (
                i128::from(bounds.lower()) - 1,
                i128::from(bounds.upper()) - 1,
            )
        } else {
            (-i128::from(bounds.upper()), -i128::from(bounds.lower()))
        };
        if lower < i128::from(piece.lower()[axis])
            || piece.upper()[axis].is_some_and(|end| upper > i128::from(end))
        {
            return Err(fail());
        }
    }
    super::validate_descent_payload(rule)
}

#[cfg(test)]
pub(in crate::foundry::artifact) fn install_source_port(
    candidate: ClosingArtifactCandidate,
) -> Result<ClosedArtifact, ArtifactError> {
    install_source_port_with_limits(
        candidate,
        Default::default(),
        super::super::source_port::DEFAULT_PREDICATE_CONSISTENCY_WORK,
        super::super::source_port::DEFAULT_PREDICATE_ATOMS,
    )
}

pub(in crate::foundry::artifact) fn install_source_port_with_limits(
    candidate: ClosingArtifactCandidate,
    geometry: CompletionGeometryLimits,
    max_predicate_consistency_work: usize,
    max_predicate_atoms: usize,
) -> Result<ClosedArtifact, ArtifactError> {
    install_source_port_impl(
        candidate,
        None,
        geometry,
        max_predicate_consistency_work,
        max_predicate_atoms,
        &mut |_, _| {},
    )
}

pub(in crate::foundry::artifact) fn install_source_port_through_total_excess_with_limits(
    candidate: ClosingArtifactCandidate,
    entry: scope::EntryScope,
    degrees: Vec<(Mask, u64)>,
    geometry: CompletionGeometryLimits,
    max_predicate_consistency_work: usize,
    max_predicate_atoms: usize,
) -> Result<ClosedArtifact, ArtifactError> {
    install_source_port_through_total_excess_with_observer(
        candidate,
        entry,
        degrees,
        geometry,
        max_predicate_consistency_work,
        max_predicate_atoms,
        &mut |_, _| {},
    )
}

pub(in crate::foundry::artifact) fn install_source_port_through_total_excess_with_observer(
    candidate: ClosingArtifactCandidate,
    entry: scope::EntryScope,
    degrees: Vec<(Mask, u64)>,
    geometry: CompletionGeometryLimits,
    max_predicate_consistency_work: usize,
    max_predicate_atoms: usize,
    observe: &mut dyn FnMut(Option<&[bool]>, &SourcePortSuccessorSnapshot),
) -> Result<ClosedArtifact, ArtifactError> {
    install_source_port_impl(
        candidate,
        Some(TotalExcessProposal { entry, degrees }),
        geometry,
        max_predicate_consistency_work,
        max_predicate_atoms,
        observe,
    )
}

fn install_source_port_impl(
    candidate: ClosingArtifactCandidate,
    proposal: Option<TotalExcessProposal>,
    geometry: CompletionGeometryLimits,
    max_predicate_consistency_work: usize,
    max_predicate_atoms: usize,
    observe: &mut dyn FnMut(Option<&[bool]>, &SourcePortSuccessorSnapshot),
) -> Result<ClosedArtifact, ArtifactError> {
    let check = |resource: &'static str, requested: usize, limit: usize| {
        if requested > limit {
            Err(ArtifactError::ResourceLimit {
                resource,
                requested,
                limit,
            })
        } else {
            Ok(())
        }
    };
    check(
        "supported predicate atom policy",
        max_predicate_atoms,
        super::super::source_port::SourcePortLimits::MAX_PREDICATE_ATOMS,
    )?;
    check("combined cover arity", candidate.arity, geometry.max_arity)?;
    let requested =
        candidate
            .rule_cells
            .iter()
            .try_fold(candidate.masters.len(), |count, cell| {
                let evidence = cell
                    .rule()
                    .replay_evidence()
                    .combined_original_domain()
                    .ok_or(ArtifactError::UnsupportedClosureShape)?;
                count.checked_add(evidence.application_boxes().len()).ok_or(
                    ArtifactError::ResourceCountOverflow {
                        resource: "combined cover boxes",
                    },
                )
            })?;
    check(
        "combined cover boxes",
        requested,
        geometry.max_requested_boxes,
    )?;
    let coordinates = requested
        .checked_mul(candidate.arity)
        .and_then(|value| value.checked_mul(2))
        .ok_or(ArtifactError::ResourceCountOverflow {
            resource: "combined cover coordinate cells",
        })?;
    check(
        "combined cover coordinate cells",
        coordinates,
        geometry.max_requested_box_coordinate_cells,
    )?;
    if candidate.algorithm_id != ALGORITHM_ID
        || !candidate.rules.is_empty()
        || candidate.canonicalizer.is_some()
        || !candidate.dependencies.is_empty()
        || !candidate.factorization_rules.is_empty()
        || candidate.common_mass_homogeneity
            != Some(CommonMassHomogeneityProof::UniformVacuumMassSquared)
    {
        return Err(ArtifactError::UnsupportedClosureShape);
    }
    super::validate_generic_bindings_for(&candidate, ReplayProducer::CombinedOriginalDomain)?;
    validate_unit_mass(&candidate)?;
    let root = scope::root_from_bounds(&candidate.supported_root_power_bounds, candidate.arity)?;
    let mut covers: BTreeMap<Mask, (Vec<PredicateCoveragePiece<'_>>, Vec<LatticeBox>)> =
        BTreeMap::new();
    let zeros: BTreeSet<_> = candidate
        .zero_sectors
        .iter()
        .map(|zero| zero.sector().clone())
        .collect();
    let proposed = proposal
        .map(|proposal| {
            proposal.entry.validate_binding(&candidate.family, &root)?;
            if !candidate.ordering.is_spired()
                || !matches!(
                    proposal.entry.bound(),
                    scope::EntryDegreeBound::MaxTotalExcessDegree(_)
                )
            {
                return Err(ArtifactError::UnsupportedClosureShape);
            }
            let census = scope::sector_count(&root)?;
            check(
                "bounded degree census",
                census,
                geometry.max_requested_boxes,
            )?;
            let cells = census.checked_mul(candidate.arity).ok_or(
                ArtifactError::ResourceCountOverflow {
                    resource: "bounded degree census coordinates",
                },
            )?;
            check(
                "bounded degree census coordinates",
                cells,
                geometry.max_requested_box_coordinate_cells,
            )?;
            let mut degrees = BTreeMap::new();
            for (sector, degree) in proposal.degrees {
                proposal.entry.validate_sector(sector.active_bits())?;
                if zeros.contains(&sector)
                    || degree < proposal.entry.bound().limit()
                    || degrees.len() >= census
                    || degrees.insert(sector, degree).is_some()
                {
                    return Err(ArtifactError::InvalidRuleShape {
                        detail: "bounded degree map has duplicate, zero or insufficient entries",
                    });
                }
            }
            let scoped_zeros = zeros.iter().try_fold(0usize, |count, sector| {
                Ok::<_, ArtifactError>(count + usize::from(sector.is_subsector_of(&root)?))
            })?;
            if degrees.len().checked_add(scoped_zeros) != Some(census) {
                return Err(ArtifactError::InvalidRuleShape {
                    detail: "bounded degree map does not exhaust the nonzero root census",
                });
            }
            Ok((proposal.entry, degrees))
        })
        .transpose()?;
    // The bounded promise is about executable keys, not the potentially larger
    // mathematical replay boxes. These endpoint buffers were precharged above.
    let actual_boxes = if proposed.is_some() {
        let mut boxes = Vec::new();
        boxes
            .try_reserve_exact(candidate.rule_cells.len())
            .map_err(|_| ArtifactError::ResourceBudgetExhausted {
                resource: "bounded actual-cell box allocation",
            })?;
        for cell in &candidate.rule_cells {
            boxes.push(actual_application_box(cell)?);
        }
        boxes
    } else {
        Vec::new()
    };
    let mut replayed_rows = 0usize;
    let mut replayed_columns = 0usize;
    let mut guards = 0usize;
    for (cell_ordinal, cell) in candidate.rule_cells.iter().enumerate() {
        if !cell.rule().sector().is_subsector_of(&root)? {
            return Err(ArtifactError::InvalidRuleShape {
                detail: "source-port rule sector is outside the declared root scope",
            });
        }
        let evidence = cell
            .rule()
            .replay_evidence()
            .combined_original_domain()
            .ok_or(ArtifactError::UnsupportedClosureShape)?;
        if zeros.contains(cell.rule().sector()) {
            return Err(ArtifactError::InvalidZeroTerminal);
        }
        covers
            .entry(cell.rule().sector().clone())
            .or_default()
            .0
            .push(PredicateCoveragePiece {
                boxes: if proposed.is_some() {
                    std::slice::from_ref(&actual_boxes[cell_ordinal])
                } else {
                    evidence.application_boxes()
                },
                affine_target: evidence.affine_application_domain(),
                affine_exclusions: evidence.affine_exclusions(),
            });
        replayed_rows = replayed_rows
            .checked_add(evidence.source_rows_used())
            .ok_or(ArtifactError::UnsupportedClosureShape)?;
        replayed_columns = replayed_columns
            .checked_add(evidence.shift_columns_checked())
            .ok_or(ArtifactError::UnsupportedClosureShape)?;
        guards = guards
            .checked_add(cell.guards().len())
            .ok_or(ArtifactError::UnsupportedClosureShape)?;
    }
    for terminal in &candidate.masters {
        let sector = Mask::try_from_indices(terminal.powers()).map_err(ArtifactError::from)?;
        if !sector.is_subsector_of(&root)? {
            return Err(ArtifactError::InvalidMasterManifest);
        }
        let local: Vec<_> = terminal
            .powers()
            .iter()
            .map(|value| {
                if *value > 0 {
                    u64::try_from(i128::from(*value) - 1)
                } else {
                    u64::try_from(-i128::from(*value))
                }
            })
            .collect::<Result<_, _>>()
            .map_err(|_| ArtifactError::InvalidMasterManifest)?;
        let upper: Vec<_> = local.iter().copied().map(Some).collect();
        covers.entry(sector).or_default().1.push(
            LatticeBox::try_new(local, upper).map_err(|_| ArtifactError::InvalidMasterManifest)?,
        );
    }
    let expected = scope::sector_count(&root)?;
    // Global zero certificates may discharge translated source columns outside
    // the requested scope; only their intersection with the root downset counts
    // towards this artifact's promised ownership cover.
    let scoped_zeros = zeros.iter().try_fold(0usize, |count, sector| {
        Ok::<_, ArtifactError>(count + usize::from(sector.is_subsector_of(&root)?))
    })?;
    if covers.len().checked_add(scoped_zeros) != Some(expected) {
        return Err(ArtifactError::UnsupportedClosureShape);
    }
    for (sector, (owners, terminals)) in covers {
        if zeros.contains(&sector) {
            return Err(ArtifactError::InvalidZeroTerminal);
        }
        let limits = PredicateCoverLimits {
            geometry,
            max_predicates: max_predicate_atoms,
            max_consistency_work: max_predicate_consistency_work,
            ..Default::default()
        };
        match &proposed {
            None => certify_predicate_cover(sector.active_bits(), &owners, &terminals, limits),
            Some((_, degrees)) => {
                let degree = *degrees
                    .get(&sector)
                    .ok_or(ArtifactError::InvalidRuleShape {
                        detail: "covered sector is missing from bounded degree map",
                    })?;
                certify_predicate_cover_up_to_degree(
                    sector.active_bits(),
                    &owners,
                    &terminals,
                    scope::EntryDegreeBound::MaxTotalExcessDegree(degree),
                    limits,
                )
            }
        }
        .map_err(|issue| match issue {
            PredicateCoverError::Budget(resource) => {
                ArtifactError::ResourceBudgetExhausted { resource }
            }
            _ => ArtifactError::UnsupportedClosureShape,
        })?;
    }
    let proof_scope = if let Some((entry, degrees)) = proposed {
        let mut budget =
            EnvelopeBudget::new_for_stage(geometry, SourcePortSuccessorStage::ActualCells);
        let mut current_sector = None;
        // Coverage and binding failures above are not mislabeled as successor
        // failures. This observation covers only the actual-cell RHS pass.
        let successor_result = (|| {
            budget
                .charge(0, candidate.arity, candidate.arity)
                .map_err(successor_error)?;
            let first_index = candidate.context.base().variables().len();
            let mut indices = Vec::new();
            indices.try_reserve_exact(candidate.arity).map_err(|_| {
                ArtifactError::ResourceBudgetExhausted {
                    resource: "bounded index map allocation",
                }
            })?;
            for axis in 0..candidate.arity {
                indices.push(first_index.checked_add(axis).ok_or(
                    ArtifactError::ResourceCountOverflow {
                        resource: "bounded index map",
                    },
                )?);
            }
            for (cell_ordinal, (cell, application)) in
                candidate.rule_cells.iter().zip(&actual_boxes).enumerate()
            {
                let sector = cell.rule().sector();
                current_sector = Some(sector.active_bits());
                budget.set_rule(cell_ordinal);
                let degree = *degrees.get(sector).ok_or(ArtifactError::InvalidRuleShape {
                    detail: "executable sector has no bounded degree",
                })?;
                let evidence = cell
                    .rule()
                    .replay_evidence()
                    .combined_original_domain()
                    .ok_or(ArtifactError::UnsupportedClosureShape)?;
                for (rhs_ordinal, term) in cell.rule().right_hand_side().iter().enumerate() {
                    budget.set_rhs(rhs_ordinal);
                    visit_successor_degrees(
                        &entry,
                        candidate.ordering,
                        sector.active_bits(),
                        degree,
                        std::slice::from_ref(application),
                        term.shift().values(),
                        term.coefficient().raw(),
                        evidence.affine_application_domain(),
                        evidence.affine_exclusions(),
                        |child| zeros.iter().any(|zero| zero.active_bits() == child),
                        &indices,
                        &mut budget,
                        |child, required| {
                            let destination = Mask::try_new(child.iter().copied())
                                .map_err(|e| SourcePortAuditError::message(e.to_string()))?;
                            if degrees
                                .get(&destination)
                                .is_some_and(|&bound| required <= bound)
                            {
                                Ok(())
                            } else {
                                Err(SourcePortAuditError::message(
                                    "actual cell successor exceeds its immutable degree envelope",
                                ))
                            }
                        },
                    )
                    .map_err(successor_error)?;
                }
                budget.completed_cell();
            }
            Ok::<_, ArtifactError>(())
        })();
        if let Err(issue) = successor_result {
            observe(current_sector, &budget.snapshot(true, false));
            return Err(issue);
        }
        observe(None, &budget.snapshot(false, true));
        super::super::scope::ArtifactProofScope::from_verified_total_excess(
            VerifiedTotalExcessScope { entry, degrees },
        )
    } else {
        super::super::scope::ArtifactProofScope::Unrestricted
    };
    let validation = ArtifactValidationWitness::new(
        candidate.source_relations.len(),
        replayed_rows,
        replayed_columns,
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
        proof_scope,
        family_fingerprint: candidate.family.fingerprint_owner(),
        family: candidate.family,
        context: candidate.context,
        source_relations: candidate.source_relations,
        rules: candidate.rules,
        rule_cells: candidate.rule_cells,
        canonicalizer: None,
        dependencies: Vec::new(),
        factorization_rules: Vec::new(),
        factorized_product_programs: Vec::new(),
        masters: candidate.masters,
        zero_sectors: candidate.zero_sectors,
        common_mass_homogeneity: candidate.common_mass_homogeneity,
        validation,
    })
}

fn actual_application_box(cell: &RuleCell) -> Result<LatticeBox, ArtifactError> {
    let endpoints = || {
        cell.rule()
            .sector()
            .active_bits()
            .iter()
            .zip(cell.application_domain().bounds())
            .map(|(&active, bounds)| {
                if active {
                    (
                        i128::from(bounds.lower()) - 1,
                        i128::from(bounds.upper()) - 1,
                    )
                } else {
                    (-i128::from(bounds.upper()), -i128::from(bounds.lower()))
                }
            })
    };
    for (lo, hi) in endpoints() {
        if u64::try_from(lo).is_err() || u64::try_from(hi).is_err() {
            return Err(ArtifactError::InvalidRuleShape {
                detail: "actual cell endpoint is outside its sector",
            });
        }
    }
    // Endpoints were checked in widened arithmetic. Let LatticeBox perform the
    // fallible allocations directly, without duplicate temporary buffers.
    LatticeBox::try_new(
        endpoints().map(|(lo, _)| lo as u64),
        endpoints().map(|(_, hi)| Some(hi as u64)),
    )
    .map_err(|issue| match issue {
        crate::foundry::completion::CompletionGeometryError::AllocationFailure { .. } => {
            ArtifactError::ResourceBudgetExhausted {
                resource: "bounded actual-cell endpoint allocation",
            }
        }
        _ => ArtifactError::InvalidRuleShape {
            detail: "actual cell has invalid local bounds",
        },
    })
}

fn successor_error(issue: SourcePortAuditError) -> ArtifactError {
    match issue {
        SourcePortAuditError::ResourceBudgetExhausted { resource } => {
            ArtifactError::ResourceBudgetExhausted { resource }
        }
        SourcePortAuditError::UnsupportedResourcePolicy {
            resource,
            requested,
            supported_max,
        } => ArtifactError::ResourceLimit {
            resource,
            requested,
            limit: supported_max,
        },
        _ => ArtifactError::InvalidRuleShape {
            detail: "bounded actual-cell successor envelope is not proved",
        },
    }
}

fn validate_unit_mass(candidate: &ClosingArtifactCandidate) -> Result<(), ArtifactError> {
    validate_unit_mass_family(&candidate.family)
}

#[cfg(test)]
mod total_excess_tests;

/// Shared early admission and final-installation check. Keeping the exact
/// family predicate here prevents frontends from duplicating algebra policy.
pub(crate) fn validate_unit_mass_family(
    family: &crate::family::IntegralFamily,
) -> Result<(), ArtifactError> {
    let base = family.coefficient_context();
    let minus_one = base
        .try_neg(&base.one(), Default::default())
        .map_err(crate::family::IntegralFamilyError::from)?;
    if family.external_count() != 0
        || base.parameter_names() != ["d"]
        || family
            .denominators()
            .iter()
            .any(|denominator| denominator.constant() != &minus_one)
        || family.power_shifts().iter().any(|shift| !shift.is_zero())
        || base.parameter("d").as_ref() != Some(family.dimension())
    {
        return Err(ArtifactError::UnsupportedClosureShape);
    }
    Ok(())
}
