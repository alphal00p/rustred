use crate::identity::TranslatedSourceRequest;

/// One structurally present forbidden coefficient at the current modular
/// sample.
///
/// A zero residue is significant: it registers the column before numeric
/// zero filtering. Allowed columns have no representation in this input and
/// are therefore omitted without entering either reducer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredForbiddenTerm<Column> {
    pub(super) column: Column,
    pub(super) residue: u64,
}

impl<Column> SpiredForbiddenTerm<Column> {
    pub(in crate::foundry::completion::spired) const fn new(column: Column, residue: u64) -> Self {
        Self { column, residue }
    }

    pub(crate) const fn column(&self) -> &Column {
        &self.column
    }

    pub(crate) const fn residue(&self) -> u64 {
        self.residue
    }
}

/// One already-classified source row for the streaming numeric kernel.
///
/// The caller supplies every structurally present forbidden term, including
/// terms whose current residue is zero, and one target residue. Allowed terms
/// are intentionally absent. Residues are canonical integers in `[0, p)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredModularRow<Column> {
    pub(super) source: TranslatedSourceRequest,
    pub(super) forbidden_terms: Vec<SpiredForbiddenTerm<Column>>,
    pub(super) target_residue: u64,
}

impl<Column> SpiredModularRow<Column> {
    pub(in crate::foundry::completion::spired) const fn new(
        source: TranslatedSourceRequest,
        forbidden_terms: Vec<SpiredForbiddenTerm<Column>>,
        target_residue: u64,
    ) -> Self {
        Self {
            source,
            forbidden_terms,
            target_residue,
        }
    }

    pub(crate) const fn source(&self) -> &TranslatedSourceRequest {
        &self.source
    }

    pub(crate) fn forbidden_terms(&self) -> &[SpiredForbiddenTerm<Column>] {
        self.forbidden_terms.as_slice()
    }

    pub(crate) const fn target_residue(&self) -> u64 {
        self.target_residue
    }
}

/// A checked first target-rank gain and its source support.
///
/// `direct_dependencies` is the canonical request image of the target row's
/// direct Symbolica `L`-pattern predecessors. `support` is the canonical,
/// duplicate-free transitive closure obtained by DFS and includes the target
/// row itself. It is discovery evidence only; exact coefficients and replay
/// remain separate promotion boundaries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredModularHit {
    pub(super) rows_consumed: usize,
    pub(super) forbidden_rank: usize,
    pub(super) augmented_rank: usize,
    pub(super) target_logical_column: usize,
    pub(super) direct_dependencies: Box<[TranslatedSourceRequest]>,
    pub(super) support: Box<[TranslatedSourceRequest]>,
}

impl SpiredModularHit {
    pub(crate) const fn rows_consumed(&self) -> usize {
        self.rows_consumed
    }

    pub(crate) const fn forbidden_rank(&self) -> usize {
        self.forbidden_rank
    }

    pub(crate) const fn augmented_rank(&self) -> usize {
        self.augmented_rank
    }

    pub(crate) const fn target_logical_column(&self) -> usize {
        self.target_logical_column
    }

    pub(crate) fn direct_dependencies(&self) -> &[TranslatedSourceRequest] {
        &self.direct_dependencies
    }

    pub(crate) fn support(&self) -> &[TranslatedSourceRequest] {
        &self.support
    }
}
