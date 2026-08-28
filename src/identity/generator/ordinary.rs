use crate::algebra::{Coefficient, IndexedCoefficient};
use crate::family::ContractionMomentum;

use super::super::relation::{IndexShift, ParametricRelation};
use super::super::row::RowId;
use super::error::ParametricIbpError;
use super::model::ParametricIbpGenerator;

impl ParametricIbpGenerator<'_> {
    pub(super) fn generate_ordinary_row(
        &self,
        ordinal: usize,
        dimension: &IndexedCoefficient,
        powers: &[IndexedCoefficient],
    ) -> Result<ParametricRelation, ParametricIbpError> {
        let loops = self.family.loop_count();
        debug_assert!(loops > 0);
        let contraction_index = ordinal / loops;
        let differentiated_loop = ordinal % loops;
        let contraction = self.family.contraction_momenta()[contraction_index];
        let row_id = RowId::OrdinaryIbp {
            contraction_momentum: contraction_index,
            differentiated_loop,
        };
        let mut row = self.empty_relation(row_id)?;
        if contraction == ContractionMomentum::Loop(differentiated_loop) {
            row.add_term_with_limits(
                &self.context,
                self.zero_shift.clone(),
                dimension.clone(),
                self.config.relation_limits,
            )?;
        }

        self.add_ordinary_derivative_terms(&mut row, differentiated_loop, contraction, powers)?;
        Ok(row)
    }

    pub(super) fn generate_external_source_row(
        &self,
        ordinal: usize,
        powers: &[IndexedCoefficient],
    ) -> Result<ParametricRelation, ParametricIbpError> {
        let loops = self.family.loop_count();
        debug_assert!(loops > 0);
        let external = ordinal / loops;
        let differentiated_loop = ordinal % loops;
        let contraction_index =
            loops
                .checked_add(external)
                .ok_or(ParametricIbpError::RowCountOverflow {
                    loops,
                    externals: self.family.external_count(),
                })?;
        let mut row = self.empty_relation(RowId::OrdinaryIbp {
            contraction_momentum: contraction_index,
            differentiated_loop,
        })?;
        self.add_ordinary_derivative_terms(
            &mut row,
            differentiated_loop,
            ContractionMomentum::External(external),
            powers,
        )?;
        Ok(row)
    }

    fn add_ordinary_derivative_terms(
        &self,
        row: &mut ParametricRelation,
        differentiated_loop: usize,
        contraction: ContractionMomentum,
        powers: &[IndexedCoefficient],
    ) -> Result<(), ParametricIbpError> {
        for (denominator, power) in powers.iter().enumerate() {
            let derivative = self.family.derivative_contraction(
                denominator,
                differentiated_loop,
                contraction,
            )?;
            self.add_negative_derivative_term(
                row,
                self.positive_units[denominator].clone(),
                power,
                derivative.constant(),
            )?;
            for (target, coefficient) in derivative.denominator_coefficients().iter().enumerate() {
                let shift =
                    self.positive_units[denominator].checked_add(&self.negative_units[target])?;
                self.add_negative_derivative_term(row, shift, power, coefficient)?;
            }
        }
        Ok(())
    }

    fn add_negative_derivative_term(
        &self,
        row: &mut ParametricRelation,
        shift: IndexShift,
        power: &IndexedCoefficient,
        derivative_coefficient: &Coefficient,
    ) -> Result<(), ParametricIbpError> {
        if derivative_coefficient.is_zero() {
            return Ok(());
        }
        let derivative = self.context.lift(derivative_coefficient)?;
        let product = self.context.mul_with_limits(
            power,
            &derivative,
            self.config.relation_limits.arithmetic.exact_algebra,
        )?;
        let coefficient = self.context.neg_with_limits(
            &product,
            self.config.relation_limits.arithmetic.exact_algebra,
        )?;
        row.add_term_with_limits(
            &self.context,
            shift,
            coefficient,
            self.config.relation_limits,
        )?;
        Ok(())
    }
}
