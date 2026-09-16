//! Internal profiling only; none of these counters carries proof authority.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Stage {
    Classification,
    Admission,
    TrueMaterialization,
    FalseMaterialization,
    BaseReduction,
    ExtensionReduction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LocalCap {
    Equations,
    PolynomialCells,
    MatrixCells,
    Allocation,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Shape {
    pub rows: usize,
    pub columns: usize,
    pub true_count: usize,
    pub false_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Exhaustion {
    pub stage: Stage,
    pub requested: Option<usize>,
    pub remaining: usize,
    pub shape: Shape,
    pub local_cap: Option<LocalCap>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct CacheStatistics {
    pub hits: usize,
    pub misses: usize,
    /// Accepted work units, not a count of operations or rejected requests.
    pub native_work: usize,
    pub uncached_results: usize,
    pub atom_slots: usize,
    pub coordinate_cells: usize,
    pub implication_calls: usize,
    pub skipped_too_few: usize,
    pub skipped_no_true: usize,
    pub unsupported: usize,
    pub classification_materializations: usize,
    pub classification_replaces: usize,
    pub true_materializations: usize,
    pub false_materializations: usize,
    pub implication_replaces: usize,
    pub base_calls: usize,
    pub extension_calls: usize,
    pub matrix_work: usize,
    pub matrix_rows: usize,
    pub matrix_cells: usize,
    pub base_hits: usize,
    pub base_misses: usize,
    pub base_fallbacks: usize,
    pub extension_hits: usize,
    pub extension_misses: usize,
    pub extension_fallbacks: usize,
    pub inconsistent_bases: usize,
    pub equal_rank_contradictions: usize,
    pub inconsistent_extensions: usize,
    pub native_errors: usize,
    pub native_panics: usize,
    pub exhaustion: Option<Exhaustion>,
}

/// Overflow of an observational count cannot affect native work or proofs.
pub(super) fn add(counter: &mut usize, value: usize) {
    *counter = counter.checked_add(value).unwrap_or(usize::MAX);
}
