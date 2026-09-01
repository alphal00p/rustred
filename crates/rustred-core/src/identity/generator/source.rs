use super::counts::checked_row_counts;
use super::error::{ParametricIbpError, try_preallocate_vec};
use super::model::{
    CompletedIbpSourceRows, IbpSourceRow, ParametricIbpGenerator, PreparedIbpSource,
    PreparedIbpSourceBatch,
};
use super::scope::IbpSourceLayout;

impl CompletedIbpSourceRows {
    pub fn into_relations(self) -> Vec<super::super::relation::ParametricRelation> {
        self.relations
    }

    /// Whether this barrier owns the complete `L * (L + E)` ordinary span.
    ///
    /// Consumers that make rank or completeness claims must reject the
    /// intentionally smaller external-contraction-only source layout.
    #[allow(dead_code)] // Consumed by the staged crate-private foundry driver.
    pub(crate) const fn is_complete_ordinary(&self) -> bool {
        matches!(self.layout, IbpSourceLayout::CompleteOrdinary)
    }

    #[allow(dead_code)] // Consumed by the staged crate-private foundry driver.
    pub(crate) const fn layout_name(&self) -> &'static str {
        self.layout.name()
    }

    #[allow(dead_code)] // Consumed by the staged crate-private foundry driver.
    pub(crate) fn source_row_count(&self) -> usize {
        self.relations.len()
    }

    #[allow(dead_code)] // Consumed by the staged crate-private foundry driver.
    pub(crate) fn source_row_id(&self, ordinal: usize) -> Option<&crate::identity::RowId> {
        self.relations
            .get(ordinal)
            .map(|relation| relation.row_id())
    }

    /// Borrow one sealed source relation for an exact crate-internal join.
    ///
    /// This does not expose mutation or unrestricted cloning.  In particular,
    /// source-discovery uses it once when binding a complete zero-offset
    /// translation to the completed source barrier from which it must have
    /// been derived.
    pub(crate) fn source_relation(
        &self,
        ordinal: usize,
    ) -> Option<&super::super::relation::ParametricRelation> {
        self.relations.get(ordinal)
    }

    /// Crate-test-only chronology mutant. Prepared-batch completion never
    /// permits this ordering in production.
    #[cfg(test)]
    pub(crate) fn swap_source_rows_for_test(&mut self, left: usize, right: usize) -> bool {
        if left >= self.relations.len() || right >= self.relations.len() {
            return false;
        }
        self.relations.swap(left, right);
        true
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
