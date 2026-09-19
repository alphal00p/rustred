//! Load saved formulas into the explicitly uncertified concrete applier.

use rustred::family::IntegralFamily;
use rustred::identity::ParametricIbpGenerator;
use rustred::reduction::ReductionLimits;
use rustred::sector::{CoordinatePriority, CoordinatePriorityLimits, Mask, OrderingPolicy, zero};
use rustred::solver::CandidateReducer;

use crate::application::AppError;

use super::{CandidateBundleLimits, codec, model::CandidateBundleInspection, preparation};

/// Inspect candidate structure without importing Symbolica state or coefficients.
/// Counts describe the saved payload; they do not authenticate algebra, replay
/// sources, validate every native frame, or establish closure.
pub fn inspect_generated_candidate_bundle(
    bytes: &[u8],
    input_limits: CandidateBundleLimits,
) -> Result<CandidateBundleInspection, AppError> {
    use rustred::persistence::SectionTag;
    let (envelope, record, _) = codec::read_structure(bytes, input_limits)?;
    let table = envelope
        .section(SectionTag::COEFFICIENTS)
        .expect("checked section");
    let count_bytes = table
        .get(..8)
        .ok_or_else(|| AppError::schema("truncated coefficient count"))?;
    let unique_coefficients = usize::try_from(u64::from_le_bytes(count_bytes.try_into().unwrap()))
        .map_err(|_| AppError::limit("coefficient count exceeds host width"))?;
    if unique_coefficients > input_limits.max_collection_entries {
        return Err(AppError::limit("coefficient count exceeds input limit"));
    }
    Ok(CandidateBundleInspection {
        schema: record.schema,
        status: record.status,
        family_fingerprint: record.family_fingerprint,
        arity: record.root_sector.len(),
        solved_sectors: record.sectors.len(),
        generated_rules: record.sectors.iter().map(|s| s.rules.len()).sum(),
        finite_residuals: record
            .sectors
            .iter()
            .map(|s| s.finite_residuals.len())
            .sum(),
        unique_coefficients,
        symbolica_state_bytes: envelope
            .section(SectionTag::SYMBOLICA_STATE)
            .expect("checked section")
            .len(),
        coefficient_table_bytes: table.len(),
    })
}

/// Decode a saved candidate bundle for experimental concrete application.
///
/// Re-establishes the family binding, zero-sector evidence and the saved
/// ordering, but performs no sector search, source-identity replay or closure
/// certification. The returned [`CandidateReducer`] checks guards and descent
/// at actual integer targets; it is never a `ClosedArtifact`. The family and
/// reducer use the original denominator coordinates, not priority-order slots.
///
/// Only load programs produced by the trusted matching RustRed/Symbolica stack.
/// Native Symbolica state and Atom decoding is not a hostile-input parser.
/// Frame and structural limits do not bound every allocation in malformed
/// native data. Loaded coefficients must have their generated normalized form.
pub fn load_generated_candidate_bundle<const N: usize>(
    bytes: &[u8],
    input_limits: CandidateBundleLimits,
    reduction_limits: ReductionLimits,
) -> Result<(IntegralFamily, CandidateReducer<N>), AppError> {
    let bundle = codec::read(bytes, input_limits)?;
    if !(1..=16).contains(&N) || bundle.root_sector.len() != N {
        return Err(AppError::input("candidate bundle/reducer arity mismatch"));
    }
    let family = bundle
        .family
        .to_family(
            &bundle.coefficients,
            input_limits.family_limits(),
            input_limits.binary_limits(),
        )
        .map_err(codec::binary_error)?;
    if family.fingerprint() != bundle.family_fingerprint || family.denominator_count() != N {
        return Err(AppError::input(
            "candidate family binding differs from reconstructed family",
        ));
    }
    let prepared =
        preparation::prepare::<N>(family, &bundle.root_sector, bundle.permutation.as_deref())?;
    let context = ParametricIbpGenerator::try_new(&prepared.family)
        .map_err(|error| AppError::execution(error.to_string()))?
        .context()
        .clone();
    let solutions = codec::solutions::<N>(
        &bundle,
        &context,
        prepared.sources.index_variables(),
        input_limits,
    )?;
    let ordering = candidate_ordering(N, bundle.permutation.as_deref())?;
    // Native transport ownership ends here: the unchanged applier consumes
    // the reconstructed solutions. Do not retain the dictionary alongside its
    // prepared coefficient owners while building the reducer.
    drop(bundle);
    // Preparation keeps a compact zero-mask census. The experimental reducer
    // takes native proof owners, not caller-asserted zero masks.
    let analyzer = zero::Analyzer::try_unrestricted(&prepared.family)
        .map_err(|error| AppError::execution(error.to_string()))?;
    let mut certificates = Vec::with_capacity(prepared.zeros.len());
    for sector in prepared.zeros.iter() {
        let mask = Mask::try_new(*sector).map_err(|error| AppError::input(error.to_string()))?;
        match analyzer
            .analyze(&mask)
            .map_err(|error| AppError::execution(error.to_string()))?
        {
            zero::Decision::ProvedZero(certificate) => certificates.push(certificate),
            _ => {
                return Err(AppError::internal_invariant(
                    "prepared zero-sector proof was lost",
                ));
            }
        }
    }
    drop(analyzer);
    let reducer = CandidateReducer::try_new(
        &prepared.family,
        prepared.root,
        ordering,
        solutions,
        certificates,
        reduction_limits,
    )
    .map_err(|error| AppError::execution(error.to_string()))?;
    Ok((prepared.family, reducer))
}

fn candidate_ordering(
    arity: usize,
    permutation: Option<&[usize]>,
) -> Result<OrderingPolicy, AppError> {
    preparation::validate_permutation(arity, permutation)?;
    let Some(slots) = permutation else {
        return Ok(OrderingPolicy::SpiredUncutV1);
    };
    // Solver input lists slots by priority. The application descriptor stores
    // the inverse: each physical slot's rank. Never permute the integral key.
    let mut ranks = vec![0; arity];
    for (rank, &slot) in slots.iter().enumerate() {
        ranks[slot] = rank;
    }
    let priority = CoordinatePriority::try_new(arity, &ranks, CoordinatePriorityLimits::default())
        .map_err(|error| AppError::input(error.to_string()))?;
    OrderingPolicy::try_spired_with_coordinate_priority(&priority)
        .map_err(|error| AppError::input(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_non_involutive_priority_is_inverted_without_rerouting_keys() {
        let expected =
            CoordinatePriority::try_new(3, &[1, 2, 0], CoordinatePriorityLimits::default())
                .unwrap();
        assert_eq!(
            candidate_ordering(3, Some(&[2, 0, 1])).unwrap(),
            OrderingPolicy::try_spired_with_coordinate_priority(&expected).unwrap()
        );
        assert!(candidate_ordering(3, Some(&[0, 0, 2])).is_err());
    }
}
