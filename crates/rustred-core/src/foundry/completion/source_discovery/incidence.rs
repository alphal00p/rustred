use std::sync::Arc;

use crate::identity::{
    CompletedIbpSourceRows, IndexShift, TranslatedSource, TranslatedSourceBatch,
};

use super::nominate::{check_limit, checked_add, try_vec};
use super::{SourceDiscoveryError, SourceDiscoveryLimits};

pub(super) const SOURCE_ROWS: &str = "source-discovery ordinary source rows";
pub(super) const SOURCE_TERMS: &str = "source-discovery ordinary term occurrences";
pub(super) const DISTINCT_SHIFTS: &str = "source-discovery distinct ordinary shifts";

/// Borrowed exact support index for one complete ordinary source module.
///
/// The input must be the complete rectangular translation at the unique zero
/// offset.  Coefficients and gates remain owned by that sealed batch; this
/// index records only checked cardinalities and borrows its exact term keys.
#[derive(Debug)]
pub(crate) struct OrdinarySourceIncidenceIndex<'sources> {
    identity: Arc<()>,
    sources: &'sources TranslatedSourceBatch,
    arity: usize,
    term_occurrences: usize,
    distinct_shift_count: usize,
}

impl<'sources> OrdinarySourceIncidenceIndex<'sources> {
    pub(crate) fn try_new(
        sources: &'sources TranslatedSourceBatch,
        limits: SourceDiscoveryLimits,
    ) -> Result<Self, SourceDiscoveryError> {
        if !sources.is_complete_ordinary() {
            return Err(SourceDiscoveryError::WrongSourceLayout {
                actual: sources.source_layout_name(),
            });
        }
        if sources.family_fingerprint().is_empty() || sources.context_fingerprint().is_empty() {
            return Err(SourceDiscoveryError::ScopeMismatch {
                detail: "zero-offset source batch has an empty family or context identity",
            });
        }
        if sources.source_row_count() == 0 || sources.sources().is_empty() {
            return Err(SourceDiscoveryError::Invariant {
                detail: "zero-offset incidence input has no ordinary sources",
            });
        }
        check_limit(
            SOURCE_ROWS,
            sources.source_row_count(),
            limits.max_source_rows,
        )?;
        if sources.offsets().len() != 1 {
            return Err(SourceDiscoveryError::Invariant {
                detail: "ordinary incidence input is not a single-offset source batch",
            });
        }
        let zero = &sources.offsets()[0];
        let arity = zero.len();
        if arity == 0 || zero.values().iter().any(|&component| component != 0) {
            return Err(SourceDiscoveryError::Invariant {
                detail: "ordinary incidence input offset is not the nonempty zero shift",
            });
        }
        check_limit("source-discovery arity", arity, limits.max_arity)?;
        if sources.sources().len() != sources.source_row_count() {
            return Err(SourceDiscoveryError::Invariant {
                detail: "zero-offset batch does not contain every ordinary source exactly once",
            });
        }

        let mut term_occurrences = 0usize;
        for (source_ordinal, source) in sources.sources().iter().enumerate() {
            validate_source(source, source_ordinal, zero, arity)?;
            term_occurrences = checked_add(SOURCE_TERMS, term_occurrences, source.terms().len())?;
            check_limit(
                SOURCE_TERMS,
                term_occurrences,
                limits.max_source_term_occurrences,
            )?;
        }
        if term_occurrences == 0 {
            return Err(SourceDiscoveryError::Invariant {
                detail: "complete ordinary source module has empty exact support",
            });
        }

        let mut distinct: Vec<&IndexShift> = try_vec(DISTINCT_SHIFTS, term_occurrences)?;
        for source in sources.sources() {
            distinct.extend(source.terms().keys());
        }
        distinct.sort_unstable_by(|left, right| left.values().cmp(right.values()));
        distinct.dedup_by(|left, right| left.values() == right.values());
        check_limit(
            DISTINCT_SHIFTS,
            distinct.len(),
            limits.max_distinct_source_shifts,
        )?;
        let distinct_shift_count = distinct.len();

        Ok(Self {
            identity: Arc::new(()),
            sources,
            arity,
            term_occurrences,
            distinct_shift_count,
        })
    }

    pub(crate) const fn arity(&self) -> usize {
        self.arity
    }

    pub(super) fn identity_owner(&self) -> Arc<()> {
        self.identity.clone()
    }

    pub(super) fn owns_identity(&self, identity: &Arc<()>) -> bool {
        Arc::ptr_eq(&self.identity, identity)
    }

    pub(crate) fn family_fingerprint(&self) -> &str {
        self.sources.family_fingerprint()
    }

    pub(crate) fn context_fingerprint(&self) -> &str {
        self.sources.context_fingerprint()
    }

    pub(crate) fn source_count(&self) -> usize {
        self.sources.source_row_count()
    }

    pub(crate) const fn term_occurrences(&self) -> usize {
        self.term_occurrences
    }

    pub(crate) const fn distinct_shift_count(&self) -> usize {
        self.distinct_shift_count
    }

    /// Require this exact zero-offset translation to replay one completed
    /// ordinary source barrier row for row.
    ///
    /// Both inputs are already sealed.  This cold, allocation-free equality
    /// join prevents a same-scope, same-cardinality translation of a different
    /// completed chronology from being paired with the campaign adapter.  No
    /// digest is introduced and the scan is never repeated in the task path.
    pub(crate) fn exactly_replays_completed(&self, completed: &CompletedIbpSourceRows) -> bool {
        completed.is_complete_ordinary()
            && self.family_fingerprint() == completed.family_fingerprint()
            && self.context_fingerprint() == completed.context_fingerprint()
            && self.source_count() == completed.source_row_count()
            && self
                .sources()
                .iter()
                .enumerate()
                .all(|(ordinal, translated)| {
                    let Some(source) = completed.source_relation(ordinal) else {
                        return false;
                    };
                    translated.row_id() == source.row_id()
                        && translated.terms() == source.terms()
                        && translated.nonzero_conditions() == source.nonzero_conditions()
                })
    }

    /// Reapply a caller's current resource policy before this previously
    /// admitted index enters a new cold evidence boundary.
    ///
    /// The exact source batch is immutable and borrowed, so no content can
    /// change after construction. Counts still have to be checked again:
    /// admission under one permissive policy must not bypass a later tighter
    /// sampled-dual policy.
    pub(crate) fn try_verify_limits(
        &self,
        limits: SourceDiscoveryLimits,
    ) -> Result<(), SourceDiscoveryError> {
        check_limit("source-discovery arity", self.arity, limits.max_arity)?;
        check_limit(SOURCE_ROWS, self.source_count(), limits.max_source_rows)?;
        check_limit(
            SOURCE_TERMS,
            self.term_occurrences,
            limits.max_source_term_occurrences,
        )?;
        check_limit(
            DISTINCT_SHIFTS,
            self.distinct_shift_count,
            limits.max_distinct_source_shifts,
        )?;
        Ok(())
    }

    pub(super) fn sources(&self) -> &'sources [TranslatedSource] {
        self.sources.sources()
    }
}

fn validate_source(
    source: &TranslatedSource,
    expected_ordinal: usize,
    zero: &crate::identity::IntegralShift,
    arity: usize,
) -> Result<(), SourceDiscoveryError> {
    if source.provenance().source_ordinal() != expected_ordinal {
        return Err(SourceDiscoveryError::Invariant {
            detail: "zero-offset source chronology is not complete and ordinal-stable",
        });
    }
    if source.provenance().offset() != zero {
        return Err(SourceDiscoveryError::Invariant {
            detail: "zero-offset source provenance disagrees with its batch offset",
        });
    }
    if source.terms().is_empty() {
        return Err(SourceDiscoveryError::Invariant {
            detail: "complete ordinary source module contains an empty source row",
        });
    }
    if source
        .terms()
        .keys()
        .any(|shift| shift.values().len() != arity)
    {
        return Err(SourceDiscoveryError::Invariant {
            detail: "ordinary source term has the wrong integral-shift arity",
        });
    }
    if source
        .terms()
        .values()
        .any(|coefficient| coefficient.is_zero())
    {
        return Err(SourceDiscoveryError::Invariant {
            detail: "ordinary source support contains an explicit zero coefficient",
        });
    }
    Ok(())
}
