use super::super::super::relation::RelationLimits;

/// Aggregate resource policy for one complete translated-source batch.
///
/// `relation` bounds every Symbolica coefficient/condition translation.
/// The remaining limits cover caller-sized retained Rust containers, typed
/// condition provenance, and coordinate storage across the whole batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TranslatedSourceLimits {
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
