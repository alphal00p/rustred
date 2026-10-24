use super::super::super::relation::RelationLimits;

/// Optional aggregate count diagnostics for one complete translated-source batch.
///
/// Count templates apply only when diagnostics are enabled. The explicit
/// builder also enables child coefficient/condition diagnostics. Shared
/// resource budgets remain active independently of these count settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TranslatedSourceLimits {
    pub diagnostics_enabled: bool,
    pub relation: RelationLimits,
    pub max_requested_offsets: usize,
    /// Maximum raw selected `(source ordinal, offset)` requests admitted
    /// before canonical sorting and exact-pair deduplication.
    pub max_requested_source_translations: usize,
    pub max_translated_sources: usize,
    pub max_translated_term_entries: usize,
    pub max_translated_condition_entries: usize,
    /// Maximum aggregate typed provenance entries retained across every
    /// translated nonzero condition in the output batch.
    pub max_retained_condition_source_entries: usize,
    pub max_retained_index_coordinate_cells: usize,
}

impl Default for TranslatedSourceLimits {
    fn default() -> Self {
        Self {
            diagnostics_enabled: false,
            relation: RelationLimits::default(),
            max_requested_offsets: 65_536,
            max_requested_source_translations: 1_000_000,
            max_translated_sources: 1_000_000,
            max_translated_term_entries: 16_000_000,
            max_translated_condition_entries: 4_000_000,
            max_retained_condition_source_entries: 16_000_000,
            max_retained_index_coordinate_cells: 64_000_000,
        }
    }
}

impl TranslatedSourceLimits {
    pub fn with_diagnostics(mut self) -> Self {
        self.diagnostics_enabled = true;
        self.relation.arithmetic = self.relation.arithmetic.with_diagnostics();
        self.relation.identity_conditions = self.relation.identity_conditions.with_diagnostics();
        self
    }

    pub(super) fn check_limit(
        self,
        resource: &'static str,
        requested: usize,
        limit: usize,
    ) -> Result<(), super::error::TranslatedSourceError> {
        symbolica::utils::resources::checkpoint()?;
        if self.diagnostics_enabled {
            super::construction::check_limit(resource, requested, limit)?;
        }
        Ok(())
    }
}
