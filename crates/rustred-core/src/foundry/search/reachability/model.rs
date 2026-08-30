use crate::algebra::IndexedCoefficientContext;
use crate::family::IntegralKey;
use crate::foundry::cell::RuleCell;
use crate::sector::{OrderingPolicy, symmetry::Canonicalizer};

use super::ReachabilityLimits;

/// Caller-proved reason why discovery need not expand one concrete key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ReachabilityTerminalKind {
    Master,
    ZeroSector,
    Factorization,
    LowerSectorFeedback,
    /// An explicitly caller-owned finite boundary, useful for diagnostics.
    /// It carries no implication that the key is a valid artifact master.
    ExternalBoundary,
}

/// Stable terminal provenance retained in a concrete reachability report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ReachabilityTerminal {
    kind: ReachabilityTerminalKind,
    owner_ordinal: usize,
}

impl ReachabilityTerminal {
    pub const fn new(kind: ReachabilityTerminalKind, owner_ordinal: usize) -> Self {
        Self {
            kind,
            owner_ordinal,
        }
    }

    pub const fn kind(self) -> ReachabilityTerminalKind {
        self.kind
    }

    pub const fn owner_ordinal(self) -> usize {
        self.owner_ordinal
    }
}

/// Exact, prevalidated terminal classifier supplied by one discovery caller.
///
/// The planner calls this only with canonical keys of its authenticated
/// arity. Implementations should be deterministic and should return a
/// terminal only when some independent proof owner discharges the key.
pub trait ReachabilityTerminalProvider {
    fn classify(&self, target: &IntegralKey) -> Option<ReachabilityTerminal>;
}

impl<F> ReachabilityTerminalProvider for F
where
    F: Fn(&IntegralKey) -> Option<ReachabilityTerminal>,
{
    fn classify(&self, target: &IntegralKey) -> Option<ReachabilityTerminal> {
        self(target)
    }
}

/// One nonzero specialized RHS dependency of a selected rule cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReachabilityDependency {
    source_rhs_ordinal: usize,
    raw_child: IntegralKey,
    canonical_child: IntegralKey,
}

impl ReachabilityDependency {
    pub fn source_rhs_ordinal(&self) -> usize {
        self.source_rhs_ordinal
    }

    pub fn raw_child(&self) -> &IntegralKey {
        &self.raw_child
    }

    pub fn canonical_child(&self) -> &IntegralKey {
        &self.canonical_child
    }

    pub(super) fn new(
        source_rhs_ordinal: usize,
        raw_child: IntegralKey,
        canonical_child: IntegralKey,
    ) -> Self {
        Self {
            source_rhs_ordinal,
            raw_child,
            canonical_child,
        }
    }
}

/// Exact concrete application selected by first-applicable RuleCell order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReachabilityRuleApplication {
    cell_ordinal: usize,
    assignment: Box<[i64]>,
    dependencies: Box<[ReachabilityDependency]>,
}

impl ReachabilityRuleApplication {
    pub fn cell_ordinal(&self) -> usize {
        self.cell_ordinal
    }

    pub fn assignment(&self) -> &[i64] {
        &self.assignment
    }

    pub fn dependencies(&self) -> &[ReachabilityDependency] {
        &self.dependencies
    }

    pub(super) fn new(
        cell_ordinal: usize,
        assignment: Box<[i64]>,
        dependencies: Box<[ReachabilityDependency]>,
    ) -> Self {
        Self {
            cell_ordinal,
            assignment,
            dependencies,
        }
    }
}

/// How one visited concrete key was discharged, or why it remains exposed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReachabilityDisposition {
    Terminal(ReachabilityTerminal),
    Rule(ReachabilityRuleApplication),
    Uncovered,
}

/// One canonical concrete key and its exact bounded-discovery disposition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReachabilityNode {
    target: IntegralKey,
    disposition: ReachabilityDisposition,
}

impl ReachabilityNode {
    pub fn target(&self) -> &IntegralKey {
        &self.target
    }

    pub fn disposition(&self) -> &ReachabilityDisposition {
        &self.disposition
    }

    pub(super) fn new(target: IntegralKey, disposition: ReachabilityDisposition) -> Self {
        Self {
            target,
            disposition,
        }
    }
}

/// Exact work census for one bounded discovery run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReachabilityStatistics {
    pub(super) submitted_roots: usize,
    pub(super) canonical_roots: usize,
    pub(super) discovered_nodes: usize,
    pub(super) terminal_nodes: usize,
    pub(super) rule_applications: usize,
    pub(super) uncovered_nodes: usize,
    pub(super) dependency_edges: usize,
    pub(super) retained_lattice_coordinate_cells: usize,
    pub(super) rule_cell_probes: usize,
    pub(super) guard_specializations: usize,
    pub(super) coefficient_specializations: usize,
}

impl ReachabilityStatistics {
    pub fn submitted_roots(self) -> usize {
        self.submitted_roots
    }
    pub fn canonical_roots(self) -> usize {
        self.canonical_roots
    }
    pub fn discovered_nodes(self) -> usize {
        self.discovered_nodes
    }
    pub fn terminal_nodes(self) -> usize {
        self.terminal_nodes
    }
    pub fn rule_applications(self) -> usize {
        self.rule_applications
    }
    pub fn uncovered_nodes(self) -> usize {
        self.uncovered_nodes
    }
    pub fn dependency_edges(self) -> usize {
        self.dependency_edges
    }
    pub fn retained_lattice_coordinate_cells(self) -> usize {
        self.retained_lattice_coordinate_cells
    }
    pub fn rule_cell_probes(self) -> usize {
        self.rule_cell_probes
    }
    pub fn guard_specializations(self) -> usize {
        self.guard_specializations
    }
    pub fn coefficient_specializations(self) -> usize {
        self.coefficient_specializations
    }
}

/// Deterministic result for exactly the finite concrete graph reached under
/// the configured resource limits.
///
/// Roots and nodes are sorted by the persisted integral ordering. In
/// particular, [`Self::uncovered`] yields minimal exposed keys first. This
/// object is discovery evidence only and is never an infinite-domain closure
/// witness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReachabilityFrontier {
    canonical_roots: Box<[IntegralKey]>,
    nodes: Box<[ReachabilityNode]>,
    statistics: ReachabilityStatistics,
}

impl ReachabilityFrontier {
    pub fn canonical_roots(&self) -> &[IntegralKey] {
        &self.canonical_roots
    }

    pub fn nodes(&self) -> &[ReachabilityNode] {
        &self.nodes
    }

    pub fn uncovered(&self) -> impl DoubleEndedIterator<Item = &IntegralKey> {
        self.nodes
            .iter()
            .filter_map(|node| match node.disposition() {
                ReachabilityDisposition::Uncovered => Some(node.target()),
                _ => None,
            })
    }

    pub fn statistics(&self) -> ReachabilityStatistics {
        self.statistics
    }

    pub(super) fn new(
        canonical_roots: Box<[IntegralKey]>,
        nodes: Box<[ReachabilityNode]>,
        statistics: ReachabilityStatistics,
    ) -> Self {
        Self {
            canonical_roots,
            nodes,
            statistics,
        }
    }
}

/// Reusable, topology-neutral owner of exact concrete RuleCell semantics.
///
/// Construction validates common RuleCell arity, coefficient context, family,
/// and ordering once. [`Self::discover`] may then census different bounded
/// root sets against different independently proved terminal providers.
pub struct ReachabilityPlanner<'foundry> {
    pub(super) context: &'foundry IndexedCoefficientContext,
    pub(super) ordering: OrderingPolicy,
    pub(super) canonicalizer: Option<&'foundry Canonicalizer>,
    pub(super) cells: Box<[&'foundry RuleCell]>,
    pub(super) limits: ReachabilityLimits,
}

impl ReachabilityPlanner<'_> {
    pub fn ordering(&self) -> OrderingPolicy {
        self.ordering
    }

    pub fn arity(&self) -> usize {
        self.context.index_count()
    }

    pub fn rule_cell_count(&self) -> usize {
        self.cells.len()
    }

    pub fn limits(&self) -> ReachabilityLimits {
        self.limits
    }
}
