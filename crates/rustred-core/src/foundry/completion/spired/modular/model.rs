use crate::identity::TranslatedSourceRequest;

/// One request-bearing node of the compact GPLU dependency trace.
///
/// Nodes are stored in dependency-topological order. Every predecessor ordinal
/// is therefore strictly smaller than the ordinal of this node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredDependencyTraceNode {
    pub(super) source: TranslatedSourceRequest,
    pub(super) direct_predecessors: Box<[usize]>,
}

impl SpiredDependencyTraceNode {
    pub(crate) const fn source(&self) -> &TranslatedSourceRequest {
        &self.source
    }

    pub(crate) fn direct_predecessors(&self) -> &[usize] {
        &self.direct_predecessors
    }
}

/// Immutable compact ancestor DAG for one target-producing GPLU row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredDependencyTrace {
    pub(super) nodes: Box<[SpiredDependencyTraceNode]>,
    pub(super) root: usize,
    pub(super) edge_count: usize,
}

/// A later streamed row whose modular dependency closure reaches the basis
/// row which produced the first target pivot.
///
/// The later source is the designated root of `dependency_trace`.  This is
/// proposal evidence only: exact materialization, replay, and promotion still
/// decide whether the candidate can own a rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredPostHitCandidate {
    pub(super) rows_consumed: usize,
    pub(super) forbidden_rank: usize,
    pub(super) augmented_rank: usize,
    pub(super) target_logical_column: usize,
    pub(super) dependency_trace: SpiredDependencyTrace,
}

impl SpiredPostHitCandidate {
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

    pub(crate) const fn dependency_trace(&self) -> &SpiredDependencyTrace {
        &self.dependency_trace
    }

    pub(crate) fn root_source(&self) -> &TranslatedSourceRequest {
        self.dependency_trace.nodes[self.dependency_trace.root].source()
    }
}

/// Result of admitting one row through the opt-in post-hit streaming path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpiredModularStreamOutcome {
    Pending,
    FirstHit(SpiredModularHit),
    PostHitCandidate(SpiredPostHitCandidate),
}

impl SpiredDependencyTrace {
    pub(crate) fn nodes(&self) -> &[SpiredDependencyTraceNode] {
        &self.nodes
    }

    pub(crate) const fn root(&self) -> usize {
        self.root
    }

    pub(crate) const fn edge_count(&self) -> usize {
        self.edge_count
    }
}

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
/// duplicate-free transitive closure used for identity and exclusion.
/// `dependency_order` contains exactly the same request set in the original
/// GPLU dependency-topological chronology: every dependency precedes its
/// consumer and the target-producing root is last. `dependency_trace` retains
/// the compact direct-predecessor DAG from which that projection is derived.
/// Both are discovery/scheduling evidence only; exact coefficients and replay
/// remain separate promotion boundaries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredModularHit {
    pub(super) rows_consumed: usize,
    pub(super) forbidden_rank: usize,
    pub(super) augmented_rank: usize,
    pub(super) target_logical_column: usize,
    pub(super) direct_dependencies: Box<[TranslatedSourceRequest]>,
    pub(super) support: Box<[TranslatedSourceRequest]>,
    pub(super) dependency_order: Box<[TranslatedSourceRequest]>,
    pub(super) dependency_trace: SpiredDependencyTrace,
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

    pub(crate) fn dependency_order(&self) -> &[TranslatedSourceRequest] {
        &self.dependency_order
    }

    pub(crate) const fn dependency_trace(&self) -> &SpiredDependencyTrace {
        &self.dependency_trace
    }

    /// Test-only control for proving that an oversized discovery proposal is
    /// rejected before exact materialization without terminating the bounded
    /// candidate portfolio.
    #[cfg(test)]
    pub(crate) fn replace_dependency_order_for_budget_regression(
        &mut self,
        dependency_order: Vec<TranslatedSourceRequest>,
    ) {
        self.dependency_order = dependency_order.into_boxed_slice();
    }
}
