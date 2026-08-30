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
mod two_loop;

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
    factorization::validate_and_compile(&mut candidate)?;
    match candidate.algorithm_id {
        super::one_loop::ALGORITHM_ID => one_loop::validate(candidate),
        super::two_loop::ALGORITHM_ID => two_loop::validate(candidate),
        _ => Err(ArtifactError::UnsupportedClosureShape),
    }
}

fn validate_generic_bindings(candidate: &ClosingArtifactCandidate) -> Result<(), ArtifactError> {
    if candidate.schema != ArtifactSchemaVersion::CURRENT {
        return Err(ArtifactError::UnsupportedSchema {
            actual: candidate.schema.as_u32(),
        });
    }
    if candidate.arity == 0 || candidate.context.index_count() != candidate.arity {
        return Err(ArtifactError::WrongArity {
            expected: candidate.context.index_count(),
            actual: candidate.arity,
        });
    }
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
    if candidate.family.denominator_count() != candidate.arity
        || !candidate
            .family
            .coefficient_context()
            .has_same_variable_map(candidate.context.base())
    {
        return Err(ArtifactError::WrongCoefficientContext);
    }
    if candidate.masters.is_empty()
        || candidate
            .masters
            .iter()
            .any(|master| master.powers().len() != candidate.arity)
    {
        return Err(ArtifactError::InvalidMasterManifest);
    }
    if candidate.zero_sectors.iter().any(|terminal| {
        terminal.sector().arity() != candidate.arity
            || candidate.masters.iter().any(|master| {
                terminal
                    .sector()
                    .active_bits()
                    .iter()
                    .zip(master.powers())
                    .all(|(&active, &power)| active == (power >= 1))
            })
    }) {
        return Err(ArtifactError::InvalidZeroTerminal);
    }
    validate_zero_terminal_proofs(candidate)?;
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
    if let Some(canonicalizer) = &candidate.canonicalizer {
        if canonicalizer.arity() != candidate.arity
            || canonicalizer.ordering() != OrderingPolicy::default()
        {
            return Err(ArtifactError::InvalidCanonicalizer);
        }
    }
    Ok(())
}

fn validate_zero_terminal_proofs(
    candidate: &ClosingArtifactCandidate,
) -> Result<(), ArtifactError> {
    let needs_rank_analysis = candidate
        .zero_sectors
        .iter()
        .any(|terminal| terminal.proof() == ZeroTerminalProof::LeePomeranskyRankDeficiency);
    let analyzer = needs_rank_analysis
        .then(|| crate::sector::zero::Analyzer::try_unrestricted(&candidate.family))
        .transpose()?;
    for terminal in &candidate.zero_sectors {
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
    let agreement = rule.anchor_agreement();
    if agreement.specialized_source_terms() == 0
        || agreement.specialized_right_hand_side_terms() != rule.right_hand_side().len()
        || agreement.nonzero_guards_checked() != rule.nonzero_guards().len()
    {
        return Err(ArtifactError::InvalidReplayEvidence {
            detail: "a rule cell has incomplete independent anchored agreement",
        });
    }
    Ok(())
}
