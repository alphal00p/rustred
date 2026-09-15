//! Registered in-memory source-directed producer for the existing artifact.
//! Its input rules were atomically constructed from full original-source
//! replay. This gate checks their bindings and a complete unbounded cover;
//! it never accepts old wave metadata or substitutes finite sampling.
use std::collections::{BTreeMap, BTreeSet};

use crate::foundry::cell::RuleCell;
use crate::foundry::completion::{BoxCover, CompletionGeometryLimits, LatticeBox};
use crate::sector::Mask;

use super::super::error::ArtifactError;
use super::super::model::{ArtifactValidationWitness, ClosedArtifact, CommonMassHomogeneityProof};
use super::{ClosingArtifactCandidate, ReplayProducer};

pub(in crate::foundry::artifact) const ALGORITHM_ID: &str =
    "rustred.source-port-original-domain.v1";

pub(super) fn validate_combined_rule(cell: &RuleCell) -> Result<(), ArtifactError> {
    let rule = cell.rule();
    let fail = || ArtifactError::InvalidReplayEvidence {
        detail: "combined original-domain replay is absent or not bound to its rule/cell",
    };
    let evidence = rule
        .replay_evidence()
        .combined_original_domain()
        .ok_or_else(fail)?;
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

pub(in crate::foundry::artifact) fn install_source_port(
    candidate: ClosingArtifactCandidate,
) -> Result<ClosedArtifact, ArtifactError> {
    install_source_port_with_limits(candidate, Default::default())
}

pub(in crate::foundry::artifact) fn install_source_port_with_limits(
    candidate: ClosingArtifactCandidate,
    geometry: CompletionGeometryLimits,
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
    let mut covers: BTreeMap<Mask, Vec<LatticeBox>> = BTreeMap::new();
    let zeros: BTreeSet<_> = candidate
        .zero_sectors
        .iter()
        .map(|zero| zero.sector().clone())
        .collect();
    let mut replayed_rows = 0usize;
    let mut replayed_columns = 0usize;
    let mut guards = 0usize;
    for cell in &candidate.rule_cells {
        let evidence = cell
            .rule()
            .replay_evidence()
            .combined_original_domain()
            .ok_or(ArtifactError::UnsupportedClosureShape)?;
        if zeros.contains(cell.rule().sector()) {
            return Err(ArtifactError::InvalidZeroTerminal);
        }
        for piece in evidence.application_boxes() {
            covers
                .entry(cell.rule().sector().clone())
                .or_default()
                .push(
                    LatticeBox::try_new(piece.lower().to_vec(), piece.upper().to_vec())
                        .map_err(|_| ArtifactError::UnsupportedClosureShape)?,
                );
        }
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
        covers.entry(sector).or_default().push(
            LatticeBox::try_new(local, upper).map_err(|_| ArtifactError::InvalidMasterManifest)?,
        );
    }
    let expected = 1usize
        .checked_shl(
            u32::try_from(candidate.arity).map_err(|_| ArtifactError::UnsupportedClosureShape)?,
        )
        .ok_or(ArtifactError::UnsupportedClosureShape)?;
    if covers.len().checked_add(zeros.len()) != Some(expected) {
        return Err(ArtifactError::UnsupportedClosureShape);
    }
    for (sector, pieces) in covers {
        if zeros.contains(&sector) {
            return Err(ArtifactError::InvalidZeroTerminal);
        }
        let cover = BoxCover::try_new(candidate.arity, pieces, geometry)
            .map_err(|_| ArtifactError::UnsupportedClosureShape)?;
        if !cover
            .uncovered_partition()
            .map_err(|_| ArtifactError::UnsupportedClosureShape)?
            .boxes()
            .is_empty()
        {
            return Err(ArtifactError::UnsupportedClosureShape);
        }
    }
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

fn validate_unit_mass(candidate: &ClosingArtifactCandidate) -> Result<(), ArtifactError> {
    let base = candidate.family.coefficient_context();
    let minus_one = base
        .try_neg(&base.one(), Default::default())
        .map_err(crate::family::IntegralFamilyError::from)?;
    if candidate.family.external_count() != 0
        || base.parameter_names() != ["d"]
        || candidate
            .family
            .denominators()
            .iter()
            .any(|denominator| denominator.constant() != &minus_one)
        || candidate
            .family
            .power_shifts()
            .iter()
            .any(|shift| !shift.is_zero())
        || base.parameter("d").as_ref() != Some(candidate.family.dimension())
    {
        return Err(ArtifactError::UnsupportedClosureShape);
    }
    Ok(())
}
