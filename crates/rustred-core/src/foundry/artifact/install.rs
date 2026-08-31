use std::collections::BTreeSet;

use crate::algebra::IndexedCoefficientContext;
use crate::family::{IntegralFamily, IntegralKey};
use crate::foundry::cell::{RuleCell, SourceViewConstruction};
use crate::foundry::parametric::ParametricRule;
use crate::identity::ParametricRelation;
use crate::sector::{InteriorBounds, OrderingPolicy, symmetry::Canonicalizer};

use super::error::ArtifactError;
use super::factorization::FactorizationRule;
use super::model::{
    ArtifactSchemaVersion, ClosedArtifact, CommonMassHomogeneityProof, ZeroSectorTerminal,
    ZeroTerminalProof,
};

mod factorization;
mod one_loop;
#[cfg(test)]
mod terminal;
mod two_loop;

#[cfg(test)]
pub(crate) use terminal::{TerminalAuthorityCandidate, install_terminal_authority};

pub(super) struct ClosingArtifactCandidate {
    pub schema: ArtifactSchemaVersion,
    pub algorithm_id: &'static str,
    pub arity: usize,
    pub supported_root_power_bounds: Box<[InteriorBounds]>,
    pub family: IntegralFamily,
    pub context: IndexedCoefficientContext,
    pub source_relations: Vec<ParametricRelation>,
    pub rules: Vec<ParametricRule>,
    pub rule_cells: Vec<RuleCell>,
    pub canonicalizer: Option<Canonicalizer>,
    pub dependencies: Vec<Box<ClosedArtifact>>,
    pub factorization_rules: Vec<FactorizationRule>,
    pub masters: BTreeSet<IntegralKey>,
    pub zero_sectors: Vec<ZeroSectorTerminal>,
    pub common_mass_homogeneity: Option<CommonMassHomogeneityProof>,
}

/// Seal a candidate only after a registered closure verifier discharges its
/// complete lattice partition. The candidate/runtime representation is
/// generic; the registered verifiers currently recognize the generated
/// one-loop tadpole and equal-mass two-loop sunset unit-mass partitions.
pub(super) fn install(
    mut candidate: ClosingArtifactCandidate,
) -> Result<ClosedArtifact, ArtifactError> {
    validate_generic_bindings(&candidate)?;
    factorization::validate_and_compile(
        factorization::InstallContext::new(
            candidate.arity,
            &candidate.family,
            candidate.canonicalizer.as_ref(),
            &candidate.dependencies,
            &candidate.masters,
            &candidate.zero_sectors,
        ),
        &mut candidate.factorization_rules,
    )?;
    match candidate.algorithm_id {
        super::one_loop::ALGORITHM_ID => one_loop::validate(candidate),
        super::two_loop::ALGORITHM_ID => two_loop::validate(candidate),
        _ => Err(ArtifactError::UnsupportedClosureShape),
    }
}

fn validate_generic_bindings(candidate: &ClosingArtifactCandidate) -> Result<(), ArtifactError> {
    validate_terminal_bindings(TerminalBindings {
        schema: candidate.schema,
        arity: candidate.arity,
        family: &candidate.family,
        context: &candidate.context,
        canonicalizer: candidate.canonicalizer.as_ref(),
        parent_terminals: &candidate.masters,
        zero_sectors: &candidate.zero_sectors,
        require_parent_terminals: true,
    })?;
    if candidate.supported_root_power_bounds.len() != candidate.arity
        || candidate
            .supported_root_power_bounds
            .iter()
            .any(|bounds| bounds.lower() > bounds.upper())
    {
        return Err(ArtifactError::InvalidRuleShape {
            detail: "the certified root-power box has invalid bounds or arity",
        });
    }
    for source in &candidate.source_relations {
        source.validate_context(&candidate.context)?;
        if source.family_fingerprint_owner().as_str() != candidate.family.fingerprint() {
            return Err(ArtifactError::WrongFamily);
        }
    }
    for rule in &candidate.rules {
        if rule.family_fingerprint() != candidate.family.fingerprint() {
            return Err(ArtifactError::WrongFamily);
        }
        if rule.context_fingerprint() != candidate.context.fingerprint() {
            return Err(ArtifactError::WrongCoefficientContext);
        }
        if rule.domain().arity() != candidate.arity {
            return Err(ArtifactError::WrongArity {
                expected: candidate.arity,
                actual: rule.domain().arity(),
            });
        }
    }
    for cell in &candidate.rule_cells {
        if cell.rule().family_fingerprint() != candidate.family.fingerprint() {
            return Err(ArtifactError::WrongFamily);
        }
        if !cell.indexed_context_matches(&candidate.context) {
            return Err(ArtifactError::WrongCoefficientContext);
        }
        if cell.application_domain().arity() != candidate.arity {
            return Err(ArtifactError::WrongArity {
                expected: candidate.arity,
                actual: cell.application_domain().arity(),
            });
        }
        if cell.terms().iter().any(|term| !term.descent().verify()) {
            return Err(ArtifactError::InvalidRuleShape {
                detail: "a rule cell has invalid sector-monotone descent evidence",
            });
        }
        if cell.sources().family_fingerprint() != candidate.family.fingerprint()
            || cell.sources().context_fingerprint() != candidate.context.fingerprint()
            || cell.sources().len() != cell.sources().provenance().len()
        {
            return Err(ArtifactError::InvalidReplayEvidence {
                detail: "a rule cell has foreign or incomplete immutable source views",
            });
        }
        for source in cell.sources().relations() {
            source.validate_context(&candidate.context)?;
            if source.family_fingerprint_owner().as_str() != candidate.family.fingerprint() {
                return Err(ArtifactError::WrongFamily);
            }
        }
        validate_source_view_construction(cell, candidate)?;
        validate_cell_replay(cell)?;
    }
    Ok(())
}

pub(super) struct TerminalBindings<'input> {
    pub(super) schema: ArtifactSchemaVersion,
    pub(super) arity: usize,
    pub(super) family: &'input IntegralFamily,
    pub(super) context: &'input IndexedCoefficientContext,
    pub(super) canonicalizer: Option<&'input Canonicalizer>,
    pub(super) parent_terminals: &'input BTreeSet<IntegralKey>,
    pub(super) zero_sectors: &'input [ZeroSectorTerminal],
    pub(super) require_parent_terminals: bool,
}

pub(super) fn validate_terminal_bindings(
    bindings: TerminalBindings<'_>,
) -> Result<(), ArtifactError> {
    if bindings.schema != ArtifactSchemaVersion::CURRENT {
        return Err(ArtifactError::UnsupportedSchema {
            actual: bindings.schema.as_u32(),
        });
    }
    if bindings.arity == 0 || bindings.context.index_count() != bindings.arity {
        return Err(ArtifactError::WrongArity {
            expected: bindings.context.index_count(),
            actual: bindings.arity,
        });
    }
    if bindings.family.denominator_count() != bindings.arity
        || !bindings
            .family
            .coefficient_context()
            .has_same_variable_map(bindings.context.base())
    {
        return Err(ArtifactError::WrongCoefficientContext);
    }
    if (bindings.require_parent_terminals && bindings.parent_terminals.is_empty())
        || bindings
            .parent_terminals
            .iter()
            .any(|master| master.powers().len() != bindings.arity)
    {
        return Err(ArtifactError::InvalidMasterManifest);
    }
    let mut distinct_zero_sectors = BTreeSet::new();
    for terminal in bindings.zero_sectors {
        if terminal.sector().arity() != bindings.arity
            || !distinct_zero_sectors.insert(terminal.sector().clone())
            || bindings.parent_terminals.iter().any(|master| {
                terminal
                    .sector()
                    .active_bits()
                    .iter()
                    .zip(master.powers())
                    .all(|(&active, &power)| active == (power >= 1))
            })
        {
            return Err(ArtifactError::InvalidZeroTerminal);
        }
    }
    validate_zero_terminal_proofs(bindings.family, bindings.zero_sectors)?;
    if let Some(canonicalizer) = bindings.canonicalizer {
        if canonicalizer.arity() != bindings.arity
            || canonicalizer.family_fingerprint() != bindings.family.fingerprint()
            || canonicalizer.ordering() != OrderingPolicy::default()
        {
            return Err(ArtifactError::InvalidCanonicalizer);
        }
    }
    Ok(())
}

fn validate_zero_terminal_proofs(
    family: &IntegralFamily,
    zero_sectors: &[ZeroSectorTerminal],
) -> Result<(), ArtifactError> {
    let needs_rank_analysis = zero_sectors
        .iter()
        .any(|terminal| terminal.proof() == ZeroTerminalProof::LeePomeranskyRankDeficiency);
    let analyzer = needs_rank_analysis
        .then(|| crate::sector::zero::Analyzer::try_unrestricted(family))
        .transpose()?;
    for terminal in zero_sectors {
        match terminal.proof() {
            ZeroTerminalProof::ScalelessVacuumPolynomial => {
                if terminal.sector().active_bits().iter().any(|&active| active) {
                    return Err(ArtifactError::InvalidZeroTerminal);
                }
            }
            ZeroTerminalProof::LeePomeranskyRankDeficiency => {
                let Some(analyzer) = analyzer.as_ref() else {
                    return Err(ArtifactError::InvalidZeroTerminal);
                };
                if !matches!(
                    analyzer.analyze(terminal.sector())?,
                    crate::sector::zero::Decision::ProvedZero(_)
                ) {
                    return Err(ArtifactError::InvalidZeroTerminal);
                }
            }
        }
    }
    Ok(())
}

fn validate_source_view_construction(
    cell: &RuleCell,
    candidate: &ClosingArtifactCandidate,
) -> Result<(), ArtifactError> {
    let SourceViewConstruction::ResidualProjection(evidence) = cell.sources().construction() else {
        return Ok(());
    };
    if evidence.original_relations().len() != cell.sources().len()
        || evidence.term_projections().len() != cell.sources().len()
        || evidence.stabilizer_group_elements().is_empty()
        || evidence.domain().arity() != cell.application_domain().arity()
        || evidence.domain().sector() != cell.application_domain().sector()
        || cell
            .application_domain()
            .bounds()
            .iter()
            .zip(evidence.domain().bounds())
            .any(|(application, proof)| {
                application.lower() < proof.lower() || application.upper() > proof.upper()
            })
    {
        return Err(ArtifactError::InvalidReplayEvidence {
            detail: "a residual source projection has an incomplete proof payload",
        });
    }
    for ((original, terms), projected) in evidence
        .original_relations()
        .iter()
        .zip(evidence.term_projections())
        .zip(cell.sources().relations())
    {
        if original.row_id() != projected.row_id() || terms.len() != original.terms().len() {
            return Err(ArtifactError::InvalidReplayEvidence {
                detail: "a residual source projection does not own one disposition per input term",
            });
        }
    }
    let canonicalizer = candidate
        .canonicalizer
        .as_ref()
        .ok_or(ArtifactError::InvalidCanonicalizer)?;
    let zero_sectors = candidate
        .zero_sectors
        .iter()
        .map(|terminal| terminal.sector().clone())
        .collect::<Vec<_>>();
    if !cell.sources().verify_residual_projection(
        &candidate.context,
        canonicalizer,
        &zero_sectors,
        Default::default(),
    )? {
        return Err(ArtifactError::InvalidReplayEvidence {
            detail: "a residual source projection does not exactly replay its owned source terms",
        });
    }
    Ok(())
}

fn validate_cell_replay(cell: &RuleCell) -> Result<(), ArtifactError> {
    let rule = cell.rule();
    let replay = rule.replay();
    if replay.source_rows_used() == 0
        || replay.source_rows_used() != rule.source_combination().len()
        || replay.shift_columns_checked() == 0
        || replay.exact_operations() == 0
    {
        return Err(ArtifactError::InvalidReplayEvidence {
            detail: "a rule cell has incomplete exact indexed replay counts",
        });
    }
    for contribution in rule.source_combination() {
        let source = cell
            .sources()
            .relations()
            .get(contribution.source_ordinal())
            .ok_or(ArtifactError::InvalidReplayEvidence {
                detail: "a rule-cell source contribution is outside its immutable source span",
            })?;
        if source.row_id() != contribution.row_id() {
            return Err(ArtifactError::InvalidReplayEvidence {
                detail: "a rule-cell source contribution has foreign row provenance",
            });
        }
    }
    let concrete = rule.concrete_replay();
    if concrete.source_contributions_checked() == 0
        || concrete.source_contributions_checked() != rule.source_combination().len()
        || concrete.source_terms_checked() == 0
        || concrete.right_hand_side_terms_checked() != rule.right_hand_side().len()
        || concrete.integral_keys_checked()
            != concrete
                .source_terms_checked()
                .checked_add(rule.right_hand_side().len())
                .and_then(|count| count.checked_add(1))
                .ok_or(ArtifactError::InvalidReplayEvidence {
                    detail: "a rule cell concrete replay key count overflowed",
                })?
        || concrete.nonzero_guards_checked() != rule.nonzero_guards().len()
        || concrete.exact_operations() == 0
        || concrete.peak_retained_coefficient_terms() == 0
    {
        return Err(ArtifactError::InvalidReplayEvidence {
            detail: "a rule cell has incomplete concrete specialization replay",
        });
    }
    Ok(())
}
