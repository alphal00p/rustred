use rustred::family::IntegralFamily;

use crate::application::MAX_OUTPUT_BYTES;
use crate::application::error::AppError;
use crate::application::options::RelationSelection;

use super::derivation_bound_overflow;

const MAX_DERIVATION_TERM_ATTEMPTS: usize = 2_000_000;

/// Exact result counts for every ordered batch selected by one derivation.
///
/// These counts are established without allocating generated rows. They are
/// carried to the execution boundary so no batch can silently exceed or fall
/// short of the result buffer admitted by structural preflight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::application::derive) struct DerivationStructureCensus {
    pub(in crate::application::derive) ordinary_source_rows: usize,
    pub(in crate::application::derive) external_source_rows: usize,
    pub(in crate::application::derive) lorentz_invariance_rows: usize,
    pub(in crate::application::derive) emitted_relation_rows: usize,
    term_attempts: usize,
}

impl DerivationStructureCensus {
    pub(in crate::application::derive) fn ordered_result_ceiling(self) -> usize {
        self.ordinary_source_rows
            .max(self.external_source_rows)
            .max(self.lorentz_invariance_rows)
            .max(self.emitted_relation_rows)
    }
}

/// Bound the generator's raw addition work before it constructs a single
/// parametric coefficient. For one ordinary row there is at most one dimension
/// term and `N * (N + 1)` derivative-contraction attempts. One LI row combines
/// two external-contraction ordinary rows per loop through `N + 1` affine
/// weights. Each weight first translates every source term into a temporary
/// relation and then inserts every scaled temporary term into the target, so
/// both insertion phases are charged. These are topology-independent
/// worst-case counts; exact zeroes and equal shifts can only reduce the actual
/// retained support.
pub(in crate::application::derive) fn preflight_derivation_structure(
    family: &IntegralFamily,
    selection: RelationSelection,
) -> Result<DerivationStructureCensus, AppError> {
    let loops = family.loop_count();
    let externals = family.external_count();
    let denominators = family.denominator_count();
    let census = derivation_structure_census(loops, externals, denominators, selection)?;
    if census.term_attempts > MAX_DERIVATION_TERM_ATTEMPTS {
        return Err(AppError::limit(format!(
            "the selected generic derivation has a conservative {}-term-attempt bound (L={loops}, E={externals}, N={denominators}), exceeding the application limit {MAX_DERIVATION_TERM_ATTEMPTS}",
            census.term_attempts,
        )));
    }

    let minimum_render_bound = census
        .emitted_relation_rows
        .checked_mul(4_096)
        .ok_or_else(derivation_bound_overflow)?;
    if minimum_render_bound > MAX_OUTPUT_BYTES {
        return Err(AppError::output_limit(format!(
            "the selected derivation has {} rows whose minimum conservative render bound is {minimum_render_bound} bytes, exceeding the {MAX_OUTPUT_BYTES}-byte application output limit",
            census.emitted_relation_rows,
        )));
    }
    Ok(census)
}

fn derivation_structure_census(
    loops: usize,
    externals: usize,
    denominators: usize,
    selection: RelationSelection,
) -> Result<DerivationStructureCensus, AppError> {
    let external_predecessor = externals.saturating_sub(1);
    let li_rows = if externals % 2 == 0 {
        (externals / 2)
            .checked_mul(external_predecessor)
            .ok_or_else(derivation_bound_overflow)?
    } else {
        externals
            .checked_mul(external_predecessor / 2)
            .ok_or_else(derivation_bound_overflow)?
    };
    // No source barrier or LI row exists for fewer than two external momenta.
    // Return before even evaluating bounds for work that will not run.
    if matches!(selection, RelationSelection::LorentzInvariance) && li_rows == 0 {
        return Ok(DerivationStructureCensus {
            ordinary_source_rows: 0,
            external_source_rows: 0,
            lorentz_invariance_rows: 0,
            emitted_relation_rows: 0,
            term_attempts: 0,
        });
    }
    let contractions = loops
        .checked_add(externals)
        .ok_or_else(derivation_bound_overflow)?;
    let ordinary_rows = loops
        .checked_mul(contractions)
        .ok_or_else(derivation_bound_overflow)?;
    let external_source_rows = loops
        .checked_mul(externals)
        .ok_or_else(derivation_bound_overflow)?;
    let denominator_successor = denominators
        .checked_add(1)
        .ok_or_else(derivation_bound_overflow)?;
    let ordinary_attempts_per_row = denominators
        .checked_mul(denominator_successor)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(derivation_bound_overflow)?;
    let ordinary_attempts = ordinary_rows
        .checked_mul(ordinary_attempts_per_row)
        .ok_or_else(derivation_bound_overflow)?;
    let external_source_attempts = external_source_rows
        .checked_mul(ordinary_attempts_per_row)
        .ok_or_else(derivation_bound_overflow)?;
    let li_attempts_per_row = loops
        .checked_mul(2)
        .and_then(|value| value.checked_mul(denominator_successor))
        .and_then(|value| value.checked_mul(ordinary_attempts_per_row))
        // One Builder insertion while translating the source and another
        // while scaling the translated relation into the LI target.
        .and_then(|value| value.checked_mul(2))
        .ok_or_else(derivation_bound_overflow)?;
    let li_attempts = li_rows
        .checked_mul(li_attempts_per_row)
        .ok_or_else(derivation_bound_overflow)?;
    let census = match selection {
        RelationSelection::Ordinary => DerivationStructureCensus {
            ordinary_source_rows: ordinary_rows,
            external_source_rows: 0,
            lorentz_invariance_rows: 0,
            emitted_relation_rows: ordinary_rows,
            term_attempts: ordinary_attempts,
        },
        RelationSelection::All => DerivationStructureCensus {
            ordinary_source_rows: ordinary_rows,
            external_source_rows: 0,
            lorentz_invariance_rows: li_rows,
            emitted_relation_rows: ordinary_rows
                .checked_add(li_rows)
                .ok_or_else(derivation_bound_overflow)?,
            term_attempts: ordinary_attempts
                .checked_add(li_attempts)
                .ok_or_else(derivation_bound_overflow)?,
        },
        // LI-only construction prepares exactly the L*E external-contraction
        // source rows; loop-contraction ordinary rows are neither generated nor
        // charged.
        RelationSelection::LorentzInvariance => DerivationStructureCensus {
            ordinary_source_rows: 0,
            external_source_rows,
            lorentz_invariance_rows: li_rows,
            emitted_relation_rows: li_rows,
            term_attempts: external_source_attempts
                .checked_add(li_attempts)
                .ok_or_else(derivation_bound_overflow)?,
        },
    };
    Ok(census)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn li_with_zero_or_one_external_charges_no_generation_work() {
        for externals in [0, 1] {
            assert_eq!(
                derivation_structure_census(
                    usize::MAX,
                    externals,
                    usize::MAX,
                    RelationSelection::LorentzInvariance,
                )
                .unwrap(),
                DerivationStructureCensus {
                    ordinary_source_rows: 0,
                    external_source_rows: 0,
                    lorentz_invariance_rows: 0,
                    emitted_relation_rows: 0,
                    term_attempts: 0,
                },
            );
        }
    }

    #[test]
    fn li_charges_translation_and_scaling_term_insertions() {
        // L=1, E=2, N=3 gives one LI row. Each ordinary source row has at
        // most 3*(3+1)+1 = 13 insertion attempts. The LI row combines two
        // source rows through four weights and performs two insertion phases:
        // 1*2*4*13*2 = 208 attempts.
        let li =
            derivation_structure_census(1, 2, 3, RelationSelection::LorentzInvariance).unwrap();
        assert_eq!(li.external_source_rows, 2);
        assert_eq!(li.lorentz_invariance_rows, 1);
        assert_eq!(li.emitted_relation_rows, 1);
        assert_eq!(li.term_attempts, 2 * 13 + 208);

        let all = derivation_structure_census(1, 2, 3, RelationSelection::All).unwrap();
        assert_eq!(all.ordinary_source_rows, 3);
        assert_eq!(all.lorentz_invariance_rows, 1);
        assert_eq!(all.emitted_relation_rows, 4);
        assert_eq!(all.term_attempts, 3 * 13 + 208);
    }

    #[test]
    fn every_selection_censuses_only_its_exact_execution_batches() {
        let ordinary = derivation_structure_census(2, 3, 5, RelationSelection::Ordinary).unwrap();
        assert_eq!(ordinary.ordinary_source_rows, 10);
        assert_eq!(ordinary.external_source_rows, 0);
        assert_eq!(ordinary.lorentz_invariance_rows, 0);
        assert_eq!(ordinary.emitted_relation_rows, 10);
        assert_eq!(ordinary.ordered_result_ceiling(), 10);

        let all = derivation_structure_census(2, 3, 5, RelationSelection::All).unwrap();
        assert_eq!(all.ordinary_source_rows, 10);
        assert_eq!(all.external_source_rows, 0);
        assert_eq!(all.lorentz_invariance_rows, 3);
        assert_eq!(all.emitted_relation_rows, 13);
        assert_eq!(all.ordered_result_ceiling(), 13);

        let li =
            derivation_structure_census(2, 3, 5, RelationSelection::LorentzInvariance).unwrap();
        assert_eq!(li.ordinary_source_rows, 0);
        assert_eq!(li.external_source_rows, 6);
        assert_eq!(li.lorentz_invariance_rows, 3);
        assert_eq!(li.emitted_relation_rows, 3);
        assert_eq!(li.ordered_result_ceiling(), 6);
    }
}
