use super::counts::checked_row_counts;
use super::error::{ParametricIbpError, try_preallocate_vec};
use super::model::{
    CompletedIbpSourceRows, IbpSourceRow, ParametricIbpGenerator, PreparedIbpSource,
    PreparedIbpSourceBatch,
};

impl CompletedIbpSourceRows {
    pub fn into_relations(self) -> Vec<super::super::relation::ParametricRelation> {
        self.relations
    }
}

impl PreparedIbpSourceBatch<'_, '_> {
    pub const fn len(&self) -> usize {
        self.rows
    }

    /// Generate one row at its stable layout-specific ordinal.
    pub fn generate(&self, ordinal: usize) -> Result<IbpSourceRow, ParametricIbpError> {
        let layout = self.source.layout();
        if ordinal >= self.rows {
            return Err(ParametricIbpError::RowOrdinalOutOfRange {
                batch: layout.name(),
                ordinal,
                rows: self.rows,
            });
        }
        let relation = match &self.source {
            PreparedIbpSource::CompleteOrdinary { dimension } => self
                .generator
                .generate_ordinary_row(ordinal, dimension, &self.powers)?,
            PreparedIbpSource::ExternalOnly => self
                .generator
                .generate_external_source_row(ordinal, &self.powers)?,
        };
        Ok(IbpSourceRow {
            scope: self.scope.clone(),
            layout,
            ordinal,
            relation,
        })
    }

    /// Validate one concrete ordered execution transcript and seal its source
    /// relations for LI preparation. A real `Vec` length is checked before
    /// consuming results, whose order selects the lowest-ordinal failure.
    pub fn complete(
        self,
        rows: Vec<Result<IbpSourceRow, ParametricIbpError>>,
    ) -> Result<CompletedIbpSourceRows, ParametricIbpError> {
        let layout = self.source.layout();
        if rows.len() != self.rows {
            return Err(ParametricIbpError::WrongSourceRowCount {
                batch: layout.name(),
                expected: self.rows,
                actual: rows.len(),
            });
        }
        let mut relations = try_preallocate_vec("completed IBP source relations", self.rows)?;
        for (position, row) in rows.into_iter().enumerate() {
            let row = row?;
            if row.layout != layout {
                return Err(ParametricIbpError::SourceRowLayoutMismatch {
                    position,
                    expected: layout.name(),
                    actual: row.layout.name(),
                });
            }
            if row.scope != self.scope {
                return Err(ParametricIbpError::SourceRowScopeMismatch {
                    batch: layout.name(),
                    position,
                });
            }
            if row.ordinal != position {
                return Err(ParametricIbpError::SourceRowOrdinalMismatch {
                    batch: layout.name(),
                    position,
                    actual: row.ordinal,
                });
            }
            relations.push(row.relation);
        }
        Ok(CompletedIbpSourceRows {
            scope: self.scope,
            layout,
            relations,
        })
    }
}

impl<'family> ParametricIbpGenerator<'family> {
    /// Prepare the `L*(L+E)` independent ordinary rows for deterministic
    /// ordinal execution. The returned batch owns every shared coefficient
    /// translation needed by its rows and performs no scheduling itself.
    pub fn prepare_ordinary_ibp(
        &self,
    ) -> Result<PreparedIbpSourceBatch<'_, 'family>, ParametricIbpError> {
        let (ordinary_count, _) =
            checked_row_counts(self.family.loop_count(), self.family.external_count())?;
        let (dimension, powers) = self.prepare_ordinary_coefficients()?;
        Ok(PreparedIbpSourceBatch {
            generator: self,
            scope: self.source_scope.clone(),
            source: PreparedIbpSource::CompleteOrdinary { dimension },
            powers,
            rows: ordinary_count,
        })
    }

    /// Prepare only the `L*E` external-contraction ordinary rows needed as
    /// sources for LI-only generation.
    pub fn prepare_external_ibp_sources(
        &self,
    ) -> Result<PreparedIbpSourceBatch<'_, 'family>, ParametricIbpError> {
        let loops = self.family.loop_count();
        let externals = self.family.external_count();
        let rows = loops
            .checked_mul(externals)
            .ok_or(ParametricIbpError::RowCountOverflow { loops, externals })?;
        let powers = self.prepare_ordinary_powers()?;
        Ok(PreparedIbpSourceBatch {
            generator: self,
            scope: self.source_scope.clone(),
            source: PreparedIbpSource::ExternalOnly,
            powers,
            rows,
        })
    }
}
