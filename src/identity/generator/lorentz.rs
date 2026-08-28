use crate::algebra::Coefficient;
use crate::family::ScalarProductCoordinate;

use super::super::relation::{IndexShift, ParametricRelation};
use super::super::row::RowId;
use super::counts::checked_row_counts;
use super::error::ParametricIbpError;
use super::model::{
    CompletedIbpSourceRows, ParametricIbpGenerator, PreparedLorentzInvarianceBatch,
};

impl PreparedLorentzInvarianceBatch<'_, '_, '_> {
    pub fn len(&self) -> usize {
        self.external_pairs.len()
    }

    /// Generate one LI row at its lexicographic external-pair ordinal.
    pub fn generate(&self, ordinal: usize) -> Result<ParametricRelation, ParametricIbpError> {
        let &(first_external, second_external) =
            self.external_pairs
                .get(ordinal)
                .ok_or(ParametricIbpError::RowOrdinalOutOfRange {
                    batch: "Lorentz-invariance",
                    ordinal,
                    rows: self.external_pairs.len(),
                })?;
        self.generator.generate_li_row(
            self.ordinary,
            self.source_offset,
            first_external,
            second_external,
        )
    }
}

impl<'family> ParametricIbpGenerator<'family> {
    /// Prepare LI rows from one completed ordinary or external-only source
    /// barrier. Completion already authenticated every row, so this boundary
    /// compares the semantic family/context scope once and does not replay the
    /// relation slice.
    pub fn prepare_lorentz_invariance<'generator, 'ordinary>(
        &'generator self,
        sources: &'ordinary CompletedIbpSourceRows,
    ) -> Result<PreparedLorentzInvarianceBatch<'generator, 'family, 'ordinary>, ParametricIbpError>
    {
        if sources.scope != self.source_scope {
            return Err(ParametricIbpError::CompletedSourceScopeMismatch);
        }
        let loops = self.family.loop_count();
        let externals = self.family.external_count();
        let source_offset = sources
            .layout
            .source_offset(loops)
            .ok_or(ParametricIbpError::RowCountOverflow { loops, externals })?;
        let (_, li_count) = checked_row_counts(loops, externals)?;
        let mut pairs = Vec::with_capacity(li_count);
        for first_external in 0..externals {
            for second_external in first_external + 1..externals {
                pairs.push((first_external, second_external));
            }
        }
        debug_assert_eq!(pairs.len(), li_count);
        Ok(PreparedLorentzInvarianceBatch {
            generator: self,
            ordinary: &sources.relations,
            source_offset,
            external_pairs: pairs,
        })
    }

    fn generate_li_row(
        &self,
        ordinary: &[ParametricRelation],
        source_offset: usize,
        first_external: usize,
        second_external: usize,
    ) -> Result<ParametricRelation, ParametricIbpError> {
        let row_id = RowId::LorentzInvariance {
            first_external,
            second_external,
        };
        let mut row = self.empty_relation(row_id.clone())?;
        for differentiated_loop in 0..self.family.loop_count() {
            // M_ba: X_{i b} B_{a i}
            let source_a = self.external_ordinary_row(
                ordinary,
                source_offset,
                first_external,
                differentiated_loop,
            )?;
            let coordinate_b =
                self.family
                    .coordinate_index(ScalarProductCoordinate::LoopExternal {
                        loop_index: differentiated_loop,
                        external_index: second_external,
                    })?;
            let multiplier_b = self.family.scalar_product_expansion(coordinate_b)?;
            self.add_weighted_translation(
                &mut row,
                source_a,
                multiplier_b.constant(),
                multiplier_b.denominator_coefficients(),
                false,
                &row_id,
            )?;

            // -M_ab: -X_{i a} B_{b i}
            let source_b = self.external_ordinary_row(
                ordinary,
                source_offset,
                second_external,
                differentiated_loop,
            )?;
            let coordinate_a =
                self.family
                    .coordinate_index(ScalarProductCoordinate::LoopExternal {
                        loop_index: differentiated_loop,
                        external_index: first_external,
                    })?;
            let multiplier_a = self.family.scalar_product_expansion(coordinate_a)?;
            self.add_weighted_translation(
                &mut row,
                source_b,
                multiplier_a.constant(),
                multiplier_a.denominator_coefficients(),
                true,
                &row_id,
            )?;
        }
        Ok(row)
    }

    fn external_ordinary_row<'rows>(
        &self,
        ordinary: &'rows [ParametricRelation],
        source_offset: usize,
        external: usize,
        differentiated_loop: usize,
    ) -> Result<&'rows ParametricRelation, ParametricIbpError> {
        let row = external
            .checked_mul(self.family.loop_count())
            .and_then(|offset| source_offset.checked_add(offset))
            .and_then(|offset| offset.checked_add(differentiated_loop))
            .and_then(|position| ordinary.get(position))
            .ok_or(ParametricIbpError::RowCountOverflow {
                loops: self.family.loop_count(),
                externals: self.family.external_count(),
            })?;
        Ok(row)
    }

    #[allow(clippy::too_many_arguments)]
    fn add_weighted_translation(
        &self,
        target: &mut ParametricRelation,
        source: &ParametricRelation,
        constant: &Coefficient,
        denominator_coefficients: &[Coefficient],
        negate: bool,
        row_id: &RowId,
    ) -> Result<(), ParametricIbpError> {
        self.add_one_weighted_translation(
            target,
            source,
            self.zero_shift.clone(),
            constant,
            negate,
            row_id,
        )?;
        for (denominator, coefficient) in denominator_coefficients.iter().enumerate() {
            self.add_one_weighted_translation(
                target,
                source,
                self.negative_units[denominator].clone(),
                coefficient,
                negate,
                row_id,
            )?;
        }
        Ok(())
    }

    fn add_one_weighted_translation(
        &self,
        target: &mut ParametricRelation,
        source: &ParametricRelation,
        translation: IndexShift,
        base_factor: &Coefficient,
        negate: bool,
        row_id: &RowId,
    ) -> Result<(), ParametricIbpError> {
        if base_factor.is_zero() {
            return Ok(());
        }
        let translated = source.translated(
            &self.context,
            &translation,
            row_id.clone(),
            self.config.relation_limits,
        )?;
        let mut factor = self.context.lift(base_factor)?;
        if negate {
            factor = self.context.neg_with_limits(
                &factor,
                self.config.relation_limits.arithmetic.exact_algebra,
            )?;
        }
        target.add_scaled_with_limits(
            &self.context,
            &translated,
            &factor,
            self.config.relation_limits,
        )?;
        Ok(())
    }
}
