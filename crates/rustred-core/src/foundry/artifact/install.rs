use std::collections::BTreeSet;

use crate::algebra::IndexedCoefficientContext;
use crate::family::{IntegralFamily, IntegralKey};
use crate::foundry::parametric::ParametricRule;
use crate::identity::ParametricRelation;
use crate::sector::OrderingPolicy;

use super::error::ArtifactError;
use super::model::{
    ArtifactSchemaVersion, ArtifactValidationWitness, ClosedArtifact, CommonMassHomogeneityProof,
    ZeroSectorTerminal, ZeroTerminalProof,
};

pub(super) struct ClosingArtifactCandidate {
    pub schema: ArtifactSchemaVersion,
    pub algorithm_id: &'static str,
    pub arity: usize,
    pub family: IntegralFamily,
    pub context: IndexedCoefficientContext,
    pub source_relations: Vec<ParametricRelation>,
    pub rules: Vec<ParametricRule>,
    pub masters: BTreeSet<IntegralKey>,
    pub zero_sectors: Vec<ZeroSectorTerminal>,
    pub common_mass_homogeneity: Option<CommonMassHomogeneityProof>,
}

/// Seal a candidate only after a registered closure verifier discharges its
/// complete lattice partition. The candidate/runtime representation is
/// generic; the sole registered verifier in this first slice recognizes the
/// generated one-loop unit-mass vacuum partition.
pub(super) fn install(
    candidate: ClosingArtifactCandidate,
) -> Result<ClosedArtifact, ArtifactError> {
    validate_generic_bindings(&candidate)?;
    validate_generated_one_loop_partition(candidate)
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
    Ok(())
}

fn validate_generated_one_loop_partition(
    candidate: ClosingArtifactCandidate,
) -> Result<ClosedArtifact, ArtifactError> {
    if candidate.arity != 1
        || candidate.algorithm_id != "rustred.generated.one-loop-unit-mass-tadpole.v1"
        || candidate.source_relations.len() != 1
        || candidate.rules.len() != 1
        || candidate.masters.len() != 1
        || candidate.zero_sectors.len() != 1
        || candidate.common_mass_homogeneity
            != Some(CommonMassHomogeneityProof::UniformVacuumMassSquared)
    {
        return Err(ArtifactError::UnsupportedClosureShape);
    }
    validate_canonical_one_loop_family(&candidate.family)?;
    let master = candidate
        .masters
        .first()
        .expect("the generic validator requires a nonempty master manifest");
    if master.powers() != [1] {
        return Err(ArtifactError::InvalidMasterManifest);
    }
    let zero = &candidate.zero_sectors[0];
    if zero.sector().active_bits() != [false]
        || zero.proof() != ZeroTerminalProof::ScalelessVacuumPolynomial
    {
        return Err(ArtifactError::InvalidZeroTerminal);
    }
    let rule = &candidate.rules[0];
    validate_complete_one_loop_rule(rule)?;
    validate_replay(&candidate.source_relations, rule)?;

    let guard_count = rule.nonzero_guards().len();
    for (guard_ordinal, guard) in rule.nonzero_guards().iter().enumerate() {
        candidate
            .context
            .validate_polynomial_context(guard.polynomial())?;
        if !guard_is_nonzero_on_positive_one_loop_domain(
            &candidate.context,
            guard.polynomial().raw(),
        ) {
            return Err(ArtifactError::UnprovedGuardApplicability { guard_ordinal });
        }
    }

    let replay = rule.replay();
    let validation = ArtifactValidationWitness::new(
        candidate.source_relations.len(),
        replay.source_rows_used(),
        replay.shift_columns_checked(),
        candidate.rules.len(),
        guard_count,
        candidate.masters.len(),
        candidate.zero_sectors.len(),
    );
    Ok(ClosedArtifact {
        schema: candidate.schema,
        algorithm_id: candidate.algorithm_id,
        arity: candidate.arity,
        family_fingerprint: candidate.family.fingerprint_owner(),
        family: candidate.family,
        context: candidate.context,
        source_relations: candidate.source_relations,
        rules: candidate.rules,
        masters: candidate.masters,
        zero_sectors: candidate.zero_sectors,
        common_mass_homogeneity: candidate.common_mass_homogeneity,
        validation,
    })
}

fn validate_canonical_one_loop_family(family: &IntegralFamily) -> Result<(), ArtifactError> {
    let context = family.coefficient_context();
    let one = context.one();
    let minus_one = context
        .try_neg(&one, Default::default())
        .map_err(crate::family::IntegralFamilyError::from)?;
    let dimension = context
        .parameter("d")
        .ok_or(ArtifactError::UnsupportedClosureShape)?;
    let canonical = family.loop_count() == 1
        && family.external_count() == 0
        && family.denominator_count() == 1
        && context.parameter_names() == ["d"]
        && family.dimension() == &dimension
        && family.denominators()[0].constant() == &minus_one
        && family.denominators()[0].coefficients() == [one]
        && family.external_gram().is_empty()
        && family.power_shifts() == [context.zero()];
    if canonical {
        Ok(())
    } else {
        Err(ArtifactError::UnsupportedClosureShape)
    }
}

fn validate_complete_one_loop_rule(rule: &ParametricRule) -> Result<(), ArtifactError> {
    if rule.ordering() != OrderingPolicy::default() {
        return Err(ArtifactError::InvalidRuleShape {
            detail: "the rule uses a foreign integral ordering",
        });
    }
    if rule.sector().active_bits() != [true] {
        return Err(ArtifactError::InvalidRuleShape {
            detail: "the recurrence does not own the positive one-line sector",
        });
    }
    let bounds = rule.domain().bounds()[0];
    if bounds.lower() != 1 || bounds.upper() != i64::MAX - 1 {
        return Err(ArtifactError::InvalidRuleShape {
            detail: "the free-index domain does not cover every target power n >= 2",
        });
    }
    if rule.pivot().values() != [1] {
        return Err(ArtifactError::InvalidRuleShape {
            detail: "the positive-sector rule pivot is not I(n+1)",
        });
    }
    if rule.right_hand_side().len() != 1 || rule.right_hand_side()[0].shift().values() != [0] {
        return Err(ArtifactError::InvalidRuleShape {
            detail: "the generated rule does not descend directly from I(n+1) to I(n)",
        });
    }
    for (right_hand_side_ordinal, term) in rule.right_hand_side().iter().enumerate() {
        if !term.descent().verify() {
            return Err(ArtifactError::InvalidDescentWitness {
                right_hand_side_ordinal,
            });
        }
    }
    if rule.anchor().powers() != [1] {
        return Err(ArtifactError::InvalidRuleShape {
            detail: "the independently replayed agreement anchor is not the sector corner",
        });
    }
    Ok(())
}

fn validate_replay(
    source_relations: &[ParametricRelation],
    rule: &ParametricRule,
) -> Result<(), ArtifactError> {
    let replay = rule.replay();
    if replay.source_rows_used() == 0
        || replay.source_rows_used() != rule.source_combination().len()
    {
        return Err(ArtifactError::InvalidReplayEvidence {
            detail: "the replayed source-row count differs from retained provenance",
        });
    }
    if replay.shift_columns_checked() != 2 || replay.exact_operations() == 0 {
        return Err(ArtifactError::InvalidReplayEvidence {
            detail: "the one-loop replay did not check both physical shift columns",
        });
    }
    for contribution in rule.source_combination() {
        let source = source_relations.get(contribution.source_ordinal()).ok_or(
            ArtifactError::InvalidReplayEvidence {
                detail: "a source contribution ordinal is outside the retained source set",
            },
        )?;
        if source.row_id() != contribution.row_id() {
            return Err(ArtifactError::InvalidReplayEvidence {
                detail: "a source contribution row identity differs from retained provenance",
            });
        }
    }
    if rule.anchor_agreement().specialized_source_terms() == 0
        || rule.anchor_agreement().specialized_right_hand_side_terms() != 1
        || rule.anchor_agreement().nonzero_guards_checked() != rule.nonzero_guards().len()
    {
        return Err(ArtifactError::InvalidReplayEvidence {
            detail: "the independent anchored agreement is incomplete",
        });
    }
    Ok(())
}

/// Prove the only guard shape admitted by the first closure verifier: one
/// nonzero integer monomial in the sole positive index. Base-parameter factors
/// and polynomial sums are deliberately rejected; proving those belongs to
/// the exceptional-locus engine.
fn guard_is_nonzero_on_positive_one_loop_domain(
    context: &IndexedCoefficientContext,
    polynomial: &crate::algebra::CoefficientPolynomial,
) -> bool {
    if polynomial.nterms() != 1 {
        return false;
    }
    let Some(exponents) = polynomial.exponents_iter().next() else {
        return false;
    };
    let base_count = context.base().parameter_names().len();
    if exponents.len() != base_count + 1
        || exponents[..base_count]
            .iter()
            .any(|exponent| *exponent != 0)
    {
        return false;
    }
    !polynomial.coefficients[0].is_zero()
}
