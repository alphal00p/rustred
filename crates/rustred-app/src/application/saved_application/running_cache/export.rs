//! Explicit immutable completed-record scans; never a publication barrier.
use super::*;

fn add_references(total: usize, additional: usize) -> Result<usize, AppError> {
    total
        .checked_add(additional)
        .ok_or_else(|| AppError::limit("completed cache reference count overflow"))
}

impl SavedApplicationRunningCacheBase {
    /// Visit all completed references in this immutable base. Pack versions are
    /// retained, including differing ordered guards for the same exact key.
    /// File keys are inherently unique. Forked readers preserve captured indexes
    /// and local lookup counters; lazy immutable tables remain shared as usual.
    /// The caller owns semantic validation and duplicate reconciliation.
    pub fn visit_all_completed_records(
        &self,
        context: &CoefficientContext,
        mut visit: impl FnMut(ApplicationCacheRecord) -> Result<(), AppError>,
    ) -> Result<usize, AppError> {
        checkpoint().map_err(AppError::resource)?;
        match self {
            Self::File(file) => {
                let mut reader = file.fork_reader();
                let mut emitted = 0usize;
                for key in file.keys() {
                    checkpoint().map_err(AppError::resource)?;
                    let record = reader.lookup(key, context)?.ok_or_else(|| {
                        AppError::input("completed cache file index references an absent record")
                    })?;
                    let next = add_references(emitted, 1)?;
                    visit(record)?;
                    emitted = next;
                }
                Ok(emitted)
            }
            Self::Pack(pack) => pack
                .fork_reader()
                .visit_all_completed_records(context, visit),
        }
    }
}

impl SavedApplicationReadOnlyCache {
    /// Visit every completed reference from every explicitly attached immutable
    /// base, in base order. There is no lookup precedence or latest-key filter.
    /// Captured directory metadata is never refreshed and no writer lease is
    /// acquired. Normal lazy native state/coefficient import may occur.
    pub fn visit_all_completed_records(
        &self,
        context: &CoefficientContext,
        mut visit: impl FnMut(ApplicationCacheRecord) -> Result<(), AppError>,
    ) -> Result<usize, AppError> {
        checkpoint().map_err(AppError::resource)?;
        check_context(&self.context, context)?;
        if self
            .bases
            .iter()
            .any(|base| base.binding() != &self.binding)
        {
            return Err(AppError::input("read-only cache base binding mismatch"));
        }
        let mut emitted = 0usize;
        for base in &self.bases {
            checkpoint().map_err(AppError::resource)?;
            emitted = add_references(
                emitted,
                base.visit_all_completed_records(context, &mut visit)?,
            )?;
        }
        Ok(emitted)
    }
}

impl SavedApplicationRunningCache {
    /// Visit every completed delta/base reference ONLY after an explicit,
    /// successful close has drained the writer. This method does not close,
    /// flush, freeze or otherwise command the worker. A live/failed/pending
    /// session is rejected before any callback or payload scan.
    ///
    /// A fork refreshes the delta's committed index and must match the closed
    /// report's acknowledged generation exactly. The resulting immutable scan
    /// includes all versions, even when backing reads were disabled during
    /// capture. Captured immutable bases are then scanned in their supplied order.
    /// The caller validates semantic bindings and resolves duplicate records.
    pub fn visit_closed_completed_records(
        &self,
        context: &CoefficientContext,
        mut visit: impl FnMut(ApplicationCacheRecord) -> Result<(), AppError>,
    ) -> Result<usize, AppError> {
        checkpoint().map_err(AppError::resource)?;
        check_context(&self.context, context)?;
        if self.delta.binding() != &self.binding
            || self
                .bases
                .iter()
                .any(|base| base.binding() != &self.binding)
        {
            return Err(AppError::input(
                "closed running cache base binding mismatch",
            ));
        }
        let report = self.control.report()?;
        if !report.closed {
            return Err(AppError::execution(
                "completed cache export requires an explicitly closed running writer",
            ));
        }
        if let Some(error) = report.writer_error {
            return Err(error);
        }
        if report.pending_records != 0 {
            return Err(AppError::execution(
                "closed running cache has uncommitted records",
            ));
        }
        let mut delta = self.delta.fork_reader();
        delta.refresh()?;
        if delta.generation() != report.generation {
            return Err(AppError::input(
                "closed running cache generation differs from its acknowledged checkpoint",
            ));
        }
        let mut emitted = delta.visit_all_completed_records(context, &mut visit)?;
        for base in &self.bases {
            checkpoint().map_err(AppError::resource)?;
            emitted = add_references(
                emitted,
                base.visit_all_completed_records(context, &mut visit)?,
            )?;
        }
        Ok(emitted)
    }
}

#[cfg(test)]
mod tests;
