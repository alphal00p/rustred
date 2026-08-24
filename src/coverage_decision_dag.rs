//! Bounded reduced ordered multi-terminal Boolean decision DAGs.
//!
//! This crate-private core knows nothing about integral topologies, Symbolica
//! polynomials, or coverage policy. Callers supply a stable atom order and
//! shared, exactly metered terminal payloads. Nodes are reduced and structurally
//! interned by `(atom, false_child, true_child)`.
//!
//! Compound operations use explicit work stacks, one operation-wide Boolean
//! validation set, and operation-wide ITE/apply memo tables. Logical mutations
//! are append-only and rolled back on typed failure. Standard `Vec` and
//! `HashMap` capacity retained after a rollback is deliberately not hidden:
//! [`CoverageDecisionDagCapacityStats`] reports it exactly. Operation-local
//! caches are dropped in full on return.
//!
//! The fixed-hasher maps below are never iterated to choose IDs or traversal
//! order. Their only role is membership lookup, so stable IDs depend solely on
//! deterministic false-before-true construction order.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::{BuildHasherDefault, Hash};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

type FixedBuildHasher = BuildHasherDefault<DefaultHasher>;
type FixedHashMap<K, V> = HashMap<K, V, FixedBuildHasher>;
type FixedHashSet<K> = HashSet<K, FixedBuildHasher>;

fn fixed_map<K, V>() -> FixedHashMap<K, V> {
    HashMap::with_hasher(FixedBuildHasher::default())
}

fn fixed_set<K>() -> FixedHashSet<K> {
    HashSet::with_hasher(FixedBuildHasher::default())
}

static NEXT_COVERAGE_DECISION_MANAGER_ID: AtomicU64 = AtomicU64::new(1);

/// Process-local identity of one live arena.
///
/// This nonce is deliberately absent from persisted rooted views and source
/// hashes. It exists only to reject accidental cross-arena handle reuse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct CoverageDecisionManagerId(u64);

impl CoverageDecisionManagerId {
    fn fresh() -> Result<Self, CoverageDecisionDagError> {
        NEXT_COVERAGE_DECISION_MANAGER_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                next.checked_add(1)
            })
            .map(Self)
            .map_err(|_| CoverageDecisionDagError::ManagerIdentityExhausted)
    }
}

/// Stable total-order key chosen by the caller for one Boolean atom.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct CoverageDecisionAtomId(usize);

impl CoverageDecisionAtomId {
    pub(crate) const fn new(ordinal: usize) -> Self {
        Self(ordinal)
    }

    pub(crate) const fn ordinal(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct CoverageDecisionTerminalId(usize);

impl CoverageDecisionTerminalId {
    pub(crate) const fn new(ordinal: usize) -> Self {
        Self(ordinal)
    }

    pub(crate) const fn ordinal(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct CoverageDecisionNodeId(usize);

impl CoverageDecisionNodeId {
    pub(crate) const fn new(ordinal: usize) -> Self {
        Self(ordinal)
    }

    pub(crate) const fn ordinal(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum CoverageDecisionRefTarget {
    Terminal(CoverageDecisionTerminalId),
    Node(CoverageDecisionNodeId),
}

/// Copyable live handle branded with its owning arena.
///
/// The target kind occupies the high bit of one packed word. Consequently a
/// branded handle remains two words on 64-bit targets; storing it in nodes does
/// not add the extra enum-discriminant word that a naive branded enum would.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct CoverageDecisionRef {
    manager: CoverageDecisionManagerId,
    encoded_target: u64,
}

impl CoverageDecisionRef {
    const NODE_TAG: u64 = 1 << 63;

    const fn max_encodable_ordinal() -> usize {
        if usize::BITS < u64::BITS {
            usize::MAX
        } else {
            (Self::NODE_TAG - 1) as usize
        }
    }

    const fn terminal(manager: CoverageDecisionManagerId, id: CoverageDecisionTerminalId) -> Self {
        Self {
            manager,
            encoded_target: id.ordinal() as u64,
        }
    }

    const fn node(manager: CoverageDecisionManagerId, id: CoverageDecisionNodeId) -> Self {
        Self {
            manager,
            encoded_target: Self::NODE_TAG | id.ordinal() as u64,
        }
    }

    pub(crate) const fn manager(self) -> CoverageDecisionManagerId {
        self.manager
    }

    pub(crate) const fn as_terminal(self) -> Option<CoverageDecisionTerminalId> {
        match self.target() {
            CoverageDecisionRefTarget::Terminal(id) => Some(id),
            CoverageDecisionRefTarget::Node(_) => None,
        }
    }

    pub(crate) const fn as_node(self) -> Option<CoverageDecisionNodeId> {
        match self.target() {
            CoverageDecisionRefTarget::Terminal(_) => None,
            CoverageDecisionRefTarget::Node(id) => Some(id),
        }
    }

    const fn target(self) -> CoverageDecisionRefTarget {
        let ordinal = (self.encoded_target & !Self::NODE_TAG) as usize;
        if self.encoded_target & Self::NODE_TAG == 0 {
            CoverageDecisionRefTarget::Terminal(CoverageDecisionTerminalId(ordinal))
        } else {
            CoverageDecisionRefTarget::Node(CoverageDecisionNodeId(ordinal))
        }
    }
}

impl fmt::Debug for CoverageDecisionRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CoverageDecisionRef")
            .field("manager", &self.manager)
            .field("target", &self.target())
            .finish()
    }
}

/// Stable, manager-independent reference stored in a rooted view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum CoverageDecisionPersistedRef {
    Terminal(CoverageDecisionTerminalId),
    Node(CoverageDecisionNodeId),
}

impl CoverageDecisionPersistedRef {
    fn try_into_live(
        self,
        manager: CoverageDecisionManagerId,
    ) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
        let ordinal = match self {
            Self::Terminal(id) => id.ordinal(),
            Self::Node(id) => id.ordinal(),
        };
        let encoded = u64::try_from(ordinal).map_err(|_| {
            CoverageDecisionDagError::PersistedReferenceOrdinalUnencodable {
                reference: self,
                max_ordinal: CoverageDecisionRef::max_encodable_ordinal(),
            }
        })?;
        if encoded >= CoverageDecisionRef::NODE_TAG {
            return Err(
                CoverageDecisionDagError::PersistedReferenceOrdinalUnencodable {
                    reference: self,
                    max_ordinal: CoverageDecisionRef::max_encodable_ordinal(),
                },
            );
        }
        Ok(match self {
            Self::Terminal(id) => CoverageDecisionRef::terminal(manager, id),
            Self::Node(id) => CoverageDecisionRef::node(manager, id),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct CoverageDecisionNode {
    atom: CoverageDecisionAtomId,
    when_false: CoverageDecisionRef,
    when_true: CoverageDecisionRef,
}

impl CoverageDecisionNode {
    const fn new(
        atom: CoverageDecisionAtomId,
        when_false: CoverageDecisionRef,
        when_true: CoverageDecisionRef,
    ) -> Self {
        Self {
            atom,
            when_false,
            when_true,
        }
    }
}

/// Canonical manager-independent node stored in a rooted view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct CoverageDecisionPersistedNode {
    atom: CoverageDecisionAtomId,
    when_false: CoverageDecisionPersistedRef,
    when_true: CoverageDecisionPersistedRef,
}

impl CoverageDecisionPersistedNode {
    pub(crate) const fn new(
        atom: CoverageDecisionAtomId,
        when_false: CoverageDecisionPersistedRef,
        when_true: CoverageDecisionPersistedRef,
    ) -> Self {
        Self {
            atom,
            when_false,
            when_true,
        }
    }

    pub(crate) const fn atom(self) -> CoverageDecisionAtomId {
        self.atom
    }

    pub(crate) const fn when_false(self) -> CoverageDecisionPersistedRef {
        self.when_false
    }

    pub(crate) const fn when_true(self) -> CoverageDecisionPersistedRef {
        self.when_true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct CoverageDecisionBooleanTerminals {
    when_false: CoverageDecisionRef,
    when_true: CoverageDecisionRef,
}

impl CoverageDecisionBooleanTerminals {
    pub(crate) const fn when_false(self) -> CoverageDecisionRef {
        self.when_false
    }

    pub(crate) const fn when_true(self) -> CoverageDecisionRef {
        self.when_true
    }
}

/// Four terminal outputs of an exact binary Boolean truth table.
///
/// Terminal-only outputs make direct two-root Shannon apply possible. General
/// MTBDD sub-DAG selection remains available through [`CoverageDecisionDag::if_then_else`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct CoverageDecisionBinaryTruthTable {
    false_false: CoverageDecisionRef,
    false_true: CoverageDecisionRef,
    true_false: CoverageDecisionRef,
    true_true: CoverageDecisionRef,
}

impl CoverageDecisionBinaryTruthTable {
    pub(crate) const fn new(
        false_false: CoverageDecisionRef,
        false_true: CoverageDecisionRef,
        true_false: CoverageDecisionRef,
        true_true: CoverageDecisionRef,
    ) -> Self {
        Self {
            false_false,
            false_true,
            true_false,
            true_true,
        }
    }

    const fn outputs(self) -> [CoverageDecisionRef; 4] {
        [
            self.false_false,
            self.false_true,
            self.true_false,
            self.true_true,
        ]
    }

    const fn select(self, left: bool, right: bool) -> CoverageDecisionRef {
        match (left, right) {
            (false, false) => self.false_false,
            (false, true) => self.false_true,
            (true, false) => self.true_false,
            (true, true) => self.true_true,
        }
    }
}

/// Exact caller-defined retention census for one terminal payload.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CoverageDecisionTerminalPayloadCensus {
    pub(crate) units: usize,
    pub(crate) references: usize,
}

impl CoverageDecisionTerminalPayloadCensus {
    pub(crate) const fn new(units: usize, references: usize) -> Self {
        Self { units, references }
    }
}

/// Mandatory exact metering contract for terminal payloads.
///
/// `units` is the caller's stable retained-size unit (normally bytes), while
/// `references` counts nested retained proof/source references. The payload is
/// supplied as `Arc<Self>`: the underlying `Self` is never cloned, while exactly
/// two internal `Arc` handles are retained for each canonical value (vector and
/// index). The value's equality/hash and census must remain immutable while it
/// is interned, and equal values must report identical censuses.
pub(crate) trait CoverageDecisionTerminalPayload: Eq + Hash {
    fn coverage_decision_retained_census(&self) -> CoverageDecisionTerminalPayloadCensus;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CoverageDecisionDagLimits {
    pub(crate) max_atoms: usize,
    pub(crate) max_persisted_roots: usize,
    pub(crate) max_terminals: usize,
    pub(crate) max_terminal_index_entries: usize,
    pub(crate) max_retained_terminal_payload_units: usize,
    pub(crate) max_retained_terminal_payload_references: usize,
    pub(crate) max_retained_terminal_payload_handles: usize,
    pub(crate) max_nodes: usize,
    pub(crate) max_unique_table_entries: usize,
    pub(crate) max_retained_child_references: usize,
    pub(crate) max_ite_cache_entries: usize,
    pub(crate) max_apply_cache_entries: usize,
    pub(crate) max_total_operation_cache_entries: usize,
    pub(crate) max_boolean_validation_entries: usize,
    pub(crate) max_reachability_entries: usize,
    pub(crate) max_work_stack_entries: usize,
    pub(crate) max_work_stack_pushes: usize,
    pub(crate) max_operation_steps: usize,
    pub(crate) max_ite_calls: usize,
    pub(crate) max_binary_apply_calls: usize,
    pub(crate) max_unique_table_lookups: usize,
    pub(crate) max_unique_table_comparisons: usize,
    pub(crate) max_terminal_index_lookups: usize,
    pub(crate) max_priority_candidates: usize,
    pub(crate) max_exported_roots: usize,
    pub(crate) max_exported_terminals: usize,
    pub(crate) max_exported_nodes: usize,
    pub(crate) max_exported_edges: usize,
    pub(crate) max_exported_atom_ordinals: usize,
    pub(crate) max_export_remap_entries: usize,
}

impl Default for CoverageDecisionDagLimits {
    fn default() -> Self {
        Self {
            max_atoms: 16_000_000,
            max_persisted_roots: 16_000_000,
            max_terminals: 1_000_000,
            max_terminal_index_entries: 1_000_000,
            max_retained_terminal_payload_units: 4 * 1024 * 1024 * 1024,
            max_retained_terminal_payload_references: 64_000_000,
            max_retained_terminal_payload_handles: 2_000_000,
            max_nodes: 16_000_000,
            max_unique_table_entries: 16_000_000,
            max_retained_child_references: 32_000_000,
            max_ite_cache_entries: 16_000_000,
            max_apply_cache_entries: 16_000_000,
            max_total_operation_cache_entries: 16_000_000,
            max_boolean_validation_entries: 16_000_000,
            max_reachability_entries: 17_000_000,
            max_work_stack_entries: 16_000_000,
            max_work_stack_pushes: 256_000_000,
            max_operation_steps: 256_000_000,
            max_ite_calls: 16_000_000,
            max_binary_apply_calls: 16_000_000,
            max_unique_table_lookups: 256_000_000,
            max_unique_table_comparisons: 256_000_000,
            max_terminal_index_lookups: 16_000_000,
            max_priority_candidates: 1_000_000,
            max_exported_roots: 16_000_000,
            max_exported_terminals: 1_000_000,
            max_exported_nodes: 16_000_000,
            max_exported_edges: 32_000_000,
            max_exported_atom_ordinals: 16_000_000,
            max_export_remap_entries: 17_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CoverageDecisionDagRetainedStats {
    pub(crate) terminals: usize,
    pub(crate) terminal_payload_units: usize,
    pub(crate) terminal_payload_references: usize,
    pub(crate) terminal_payload_handles: usize,
    pub(crate) nodes: usize,
    pub(crate) child_references: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CoverageDecisionDagCapacityStats {
    pub(crate) terminal_vector_entries: usize,
    pub(crate) terminal_index_entries: usize,
    pub(crate) node_vector_entries: usize,
    pub(crate) unique_table_hash_buckets: usize,
    pub(crate) unique_table_entries: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CoverageDecisionDagWorkStats {
    pub(crate) public_operations: usize,
    pub(crate) operation_steps: usize,
    pub(crate) work_stack_pushes: usize,
    pub(crate) work_stack_peak: usize,
    pub(crate) terminal_index_lookups: usize,
    pub(crate) terminal_reuses: usize,
    pub(crate) unique_table_lookups: usize,
    pub(crate) unique_table_comparisons: usize,
    pub(crate) node_reduction_hits: usize,
    pub(crate) unique_table_reuses: usize,
    pub(crate) boolean_validation_insertions: usize,
    pub(crate) boolean_validation_hits: usize,
    pub(crate) ite_calls: usize,
    pub(crate) ite_cache_insertions: usize,
    pub(crate) ite_cache_hits: usize,
    pub(crate) binary_apply_calls: usize,
    pub(crate) apply_cache_insertions: usize,
    pub(crate) apply_cache_hits: usize,
    pub(crate) priority_candidates: usize,
    pub(crate) reachability_insertions: usize,
    pub(crate) reachability_hits: usize,
    pub(crate) rooted_exports: usize,
    pub(crate) exported_terminals: usize,
    pub(crate) exported_nodes: usize,
    pub(crate) exported_roots: usize,
    pub(crate) exported_edges: usize,
    pub(crate) exported_atom_ordinals: usize,
    pub(crate) export_remap_insertions: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CoverageDecisionDagStats {
    pub(crate) retained: CoverageDecisionDagRetainedStats,
    pub(crate) capacity: CoverageDecisionDagCapacityStats,
    pub(crate) work: CoverageDecisionDagWorkStats,
}

/// Self-contained deterministic projection reachable from persisted roots.
///
/// Runtime manager brands are intentionally absent. Terminal and node order is
/// the source arena's canonical insertion order filtered to reachable entries,
/// so child node ordinals remain strictly earlier than parent ordinals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CoverageDecisionDagRootedView<T: CoverageDecisionTerminalPayload> {
    terminal_payloads: Vec<Arc<T>>,
    nodes: Vec<CoverageDecisionPersistedNode>,
    roots: Vec<CoverageDecisionPersistedRef>,
    boolean_false: CoverageDecisionPersistedRef,
    boolean_true: CoverageDecisionPersistedRef,
    atom_count: usize,
    retained: CoverageDecisionDagRetainedStats,
}

impl<T: CoverageDecisionTerminalPayload> CoverageDecisionDagRootedView<T> {
    pub(crate) fn terminal_payloads(&self) -> &[Arc<T>] {
        &self.terminal_payloads
    }

    pub(crate) fn nodes(&self) -> &[CoverageDecisionPersistedNode] {
        &self.nodes
    }

    pub(crate) fn roots(&self) -> &[CoverageDecisionPersistedRef] {
        &self.roots
    }

    pub(crate) const fn boolean_false(&self) -> CoverageDecisionPersistedRef {
        self.boolean_false
    }

    pub(crate) const fn boolean_true(&self) -> CoverageDecisionPersistedRef {
        self.boolean_true
    }

    pub(crate) const fn atom_count(&self) -> usize {
        self.atom_count
    }

    pub(crate) const fn retained_stats(&self) -> CoverageDecisionDagRetainedStats {
        self.retained
    }
}

/// A replayed arena together with roots/endpoints branded for that new arena.
pub(crate) struct CoverageDecisionDagRebuild<T: CoverageDecisionTerminalPayload> {
    dag: CoverageDecisionDag<T>,
    roots: Vec<CoverageDecisionRef>,
    boolean: CoverageDecisionBooleanTerminals,
}

impl<T: CoverageDecisionTerminalPayload> CoverageDecisionDagRebuild<T> {
    pub(crate) fn dag(&self) -> &CoverageDecisionDag<T> {
        &self.dag
    }

    pub(crate) fn dag_mut(&mut self) -> &mut CoverageDecisionDag<T> {
        &mut self.dag
    }

    pub(crate) fn roots(&self) -> &[CoverageDecisionRef] {
        &self.roots
    }

    pub(crate) const fn boolean(&self) -> CoverageDecisionBooleanTerminals {
        self.boolean
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        CoverageDecisionDag<T>,
        Vec<CoverageDecisionRef>,
        CoverageDecisionBooleanTerminals,
    ) {
        (self.dag, self.roots, self.boolean)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CoverageDecisionDagError {
    ManagerIdentityExhausted,
    ForeignManagerReference {
        expected: CoverageDecisionManagerId,
        actual: CoverageDecisionManagerId,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    TerminalReferenceOutOfRange {
        ordinal: usize,
        terminal_count: usize,
    },
    NodeReferenceOutOfRange {
        ordinal: usize,
        node_count: usize,
    },
    PersistedRootOutOfRange {
        root_ordinal: usize,
        reference: CoverageDecisionPersistedRef,
    },
    PersistedReferenceOrdinalUnencodable {
        reference: CoverageDecisionPersistedRef,
        max_ordinal: usize,
    },
    BooleanTerminalExpected {
        reference: CoverageDecisionRef,
    },
    TruthTableOutputTerminalExpected {
        reference: CoverageDecisionRef,
    },
    EqualBooleanTerminals,
    NonBooleanConditionTerminal {
        terminal: CoverageDecisionTerminalId,
    },
    VariableOrderViolation {
        parent: CoverageDecisionAtomId,
        child: CoverageDecisionAtomId,
    },
    AtomOutOfRange {
        atom: CoverageDecisionAtomId,
        atom_count: usize,
    },
    InternalVariableOrderMismatch,
    MissingAtomAssignment {
        atom: CoverageDecisionAtomId,
    },
    NonCanonicalTerminalView {
        ordinal: usize,
        existing: CoverageDecisionTerminalId,
    },
    NonCanonicalNodeView {
        ordinal: usize,
        resolved: CoverageDecisionRef,
    },
    UnreachableTerminal {
        terminal: CoverageDecisionTerminalId,
    },
    UnreachableNode {
        node: CoverageDecisionNodeId,
    },
    RetainedStatsMismatch {
        expected: CoverageDecisionDagRetainedStats,
        actual: CoverageDecisionDagRetainedStats,
    },
}

impl fmt::Display for CoverageDecisionDagError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManagerIdentityExhausted => {
                formatter.write_str("coverage decision live-manager identity space is exhausted")
            }
            Self::ForeignManagerReference { expected, actual } => write!(
                formatter,
                "coverage decision reference belongs to manager {actual:?}, expected {expected:?}"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "coverage decision DAG {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "coverage decision DAG {resource} overflowed usize"
                )
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "coverage decision DAG could not reserve {requested} {resource}"
            ),
            Self::TerminalReferenceOutOfRange {
                ordinal,
                terminal_count,
            } => write!(
                formatter,
                "coverage decision terminal {ordinal} is outside {terminal_count} retained terminals"
            ),
            Self::NodeReferenceOutOfRange {
                ordinal,
                node_count,
            } => write!(
                formatter,
                "coverage decision node {ordinal} is outside {node_count} retained nodes"
            ),
            Self::PersistedRootOutOfRange {
                root_ordinal,
                reference,
            } => write!(
                formatter,
                "coverage decision persisted root {root_ordinal} has invalid reference {reference:?}"
            ),
            Self::PersistedReferenceOrdinalUnencodable {
                reference,
                max_ordinal,
            } => write!(
                formatter,
                "coverage decision persisted reference {reference:?} exceeds the packed live-reference ordinal limit {max_ordinal}"
            ),
            Self::BooleanTerminalExpected { reference } => write!(
                formatter,
                "coverage decision Boolean endpoint {reference:?} is not a terminal"
            ),
            Self::TruthTableOutputTerminalExpected { reference } => write!(
                formatter,
                "coverage decision truth-table output {reference:?} is not a terminal"
            ),
            Self::EqualBooleanTerminals => formatter
                .write_str("coverage decision Boolean false and true terminals are identical"),
            Self::NonBooleanConditionTerminal { terminal } => write!(
                formatter,
                "coverage decision condition reaches non-Boolean terminal {}",
                terminal.ordinal()
            ),
            Self::VariableOrderViolation { parent, child } => write!(
                formatter,
                "coverage decision atom {} has non-increasing child atom {}",
                parent.ordinal(),
                child.ordinal()
            ),
            Self::AtomOutOfRange { atom, atom_count } => write!(
                formatter,
                "coverage decision atom {} is outside persisted atom count {atom_count}",
                atom.ordinal()
            ),
            Self::InternalVariableOrderMismatch => formatter.write_str(
                "coverage decision operation encountered an impossible variable-order mismatch",
            ),
            Self::MissingAtomAssignment { atom } => write!(
                formatter,
                "coverage decision evaluation has no value for atom {}",
                atom.ordinal()
            ),
            Self::NonCanonicalTerminalView { ordinal, existing } => write!(
                formatter,
                "coverage decision terminal view entry {ordinal} duplicates terminal {}",
                existing.ordinal()
            ),
            Self::NonCanonicalNodeView { ordinal, resolved } => write!(
                formatter,
                "coverage decision node view entry {ordinal} reduces or duplicates to {resolved:?}"
            ),
            Self::UnreachableTerminal { terminal } => write!(
                formatter,
                "coverage decision terminal {} is unreachable from every persisted root and Boolean endpoint",
                terminal.ordinal()
            ),
            Self::UnreachableNode { node } => write!(
                formatter,
                "coverage decision node {} is unreachable from every persisted root",
                node.ordinal()
            ),
            Self::RetainedStatsMismatch { expected, actual } => write!(
                formatter,
                "coverage decision retained census differs: expected {expected:?}, rebuilt {actual:?}"
            ),
        }
    }
}

impl std::error::Error for CoverageDecisionDagError {}

#[derive(Clone, Copy)]
struct DagCheckpoint {
    terminals: usize,
    nodes: usize,
    payload_units: usize,
    payload_references: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct IteKey {
    condition: CoverageDecisionRef,
    when_true: CoverageDecisionRef,
    when_false: CoverageDecisionRef,
    boolean: CoverageDecisionBooleanTerminals,
}

#[derive(Clone, Copy, Debug)]
enum IteFrame {
    Enter(IteKey),
    Exit {
        key: IteKey,
        atom: CoverageDecisionAtomId,
        low: IteKey,
        high: IteKey,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct ApplyKey {
    left: CoverageDecisionRef,
    right: CoverageDecisionRef,
    boolean: CoverageDecisionBooleanTerminals,
    table: CoverageDecisionBinaryTruthTable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct ValidationKey {
    node: CoverageDecisionNodeId,
    boolean: CoverageDecisionBooleanTerminals,
}

#[derive(Clone, Copy, Debug)]
enum ApplyFrame {
    Enter(ApplyKey),
    Exit {
        key: ApplyKey,
        atom: CoverageDecisionAtomId,
        low: ApplyKey,
        high: ApplyKey,
    },
}

struct OperationContext {
    limits: CoverageDecisionDagLimits,
    first_failure: Option<CoverageDecisionDagError>,
    work: CoverageDecisionDagWorkStats,
    validated_nodes: FixedHashSet<ValidationKey>,
    ite_cache: FixedHashMap<IteKey, CoverageDecisionRef>,
    apply_cache: FixedHashMap<ApplyKey, CoverageDecisionRef>,
}

impl OperationContext {
    fn new(limits: CoverageDecisionDagLimits) -> Self {
        Self {
            limits,
            first_failure: None,
            work: CoverageDecisionDagWorkStats::default(),
            validated_nodes: fixed_set(),
            ite_cache: fixed_map(),
            apply_cache: fixed_map(),
        }
    }
}

pub(crate) struct CoverageDecisionDag<T: CoverageDecisionTerminalPayload> {
    manager: CoverageDecisionManagerId,
    atom_count: usize,
    terminals: Vec<Arc<T>>,
    terminal_index: FixedHashMap<Arc<T>, CoverageDecisionTerminalId>,
    nodes: Vec<CoverageDecisionNode>,
    unique_table: FixedHashMap<u64, Vec<(CoverageDecisionNode, CoverageDecisionNodeId)>>,
    retained_payload_units: usize,
    retained_payload_references: usize,
    work_stats: CoverageDecisionDagWorkStats,
    limits: CoverageDecisionDagLimits,
}

impl<T: CoverageDecisionTerminalPayload> CoverageDecisionDag<T> {
    /// Create one live arena for exactly `atom_count` ordered atoms.
    pub(crate) fn new(
        atom_count: usize,
        limits: CoverageDecisionDagLimits,
    ) -> Result<Self, CoverageDecisionDagError> {
        check_limit("coverage atoms", atom_count, limits.max_atoms)?;
        Ok(Self {
            manager: CoverageDecisionManagerId::fresh()?,
            atom_count,
            terminals: Vec::new(),
            terminal_index: fixed_map(),
            nodes: Vec::new(),
            unique_table: fixed_map(),
            retained_payload_units: 0,
            retained_payload_references: 0,
            work_stats: CoverageDecisionDagWorkStats::default(),
            limits,
        })
    }

    pub(crate) const fn manager_id(&self) -> CoverageDecisionManagerId {
        self.manager
    }

    pub(crate) const fn atom_count(&self) -> usize {
        self.atom_count
    }

    pub(crate) const fn limits(&self) -> CoverageDecisionDagLimits {
        self.limits
    }

    pub(crate) fn retained_stats(&self) -> CoverageDecisionDagRetainedStats {
        CoverageDecisionDagRetainedStats {
            terminals: self.terminals.len(),
            terminal_payload_units: self.retained_payload_units,
            terminal_payload_references: self.retained_payload_references,
            terminal_payload_handles: self
                .terminals
                .len()
                .checked_mul(2)
                .expect("terminal insertion preflights payload-handle overflow"),
            nodes: self.nodes.len(),
            child_references: self
                .nodes
                .len()
                .checked_mul(2)
                .expect("node insertion preflights child-reference overflow"),
        }
    }

    pub(crate) fn stats(&self) -> CoverageDecisionDagStats {
        CoverageDecisionDagStats {
            retained: self.retained_stats(),
            capacity: CoverageDecisionDagCapacityStats {
                terminal_vector_entries: self.terminals.capacity(),
                terminal_index_entries: self.terminal_index.capacity(),
                node_vector_entries: self.nodes.capacity(),
                unique_table_hash_buckets: self.unique_table.capacity(),
                unique_table_entries: self
                    .unique_table
                    .values()
                    .try_fold(0usize, |total, bucket| total.checked_add(bucket.capacity()))
                    .expect("unique-table capacity preflights usize overflow"),
            },
            work: self.work_stats,
        }
    }

    pub(crate) fn terminal_payloads(&self) -> &[Arc<T>] {
        &self.terminals
    }

    fn nodes(&self) -> &[CoverageDecisionNode] {
        &self.nodes
    }

    fn terminal_payload(&self, id: CoverageDecisionTerminalId) -> Option<&T> {
        self.terminals.get(id.ordinal()).map(Arc::as_ref)
    }

    fn node(&self, id: CoverageDecisionNodeId) -> Option<CoverageDecisionNode> {
        self.nodes.get(id.ordinal()).copied()
    }

    pub(crate) fn find_terminal(&self, payload: &Arc<T>) -> Option<CoverageDecisionRef> {
        self.terminal_index
            .get(payload)
            .copied()
            .map(|id| CoverageDecisionRef::terminal(self.manager, id))
    }

    /// Run a complete logical compilation/replay unit with one cumulative
    /// checkpoint, validation set, memo-table budget, step budget, and call
    /// budget. Production formula construction should use this scope; the
    /// single-operation convenience methods below each open one such scope.
    pub(crate) fn with_operation<R>(
        &mut self,
        build: impl FnOnce(
            &mut CoverageDecisionDagOperation<'_, T>,
        ) -> Result<R, CoverageDecisionDagError>,
    ) -> Result<R, CoverageDecisionDagError> {
        self.transactional_operation(|dag, context| {
            let returned = {
                let mut operation = CoverageDecisionDagOperation { dag, context };
                build(&mut operation)
            };
            match context.first_failure.clone() {
                Some(error) => Err(error),
                None => returned,
            }
        })
    }

    pub(crate) fn intern_terminal(
        &mut self,
        payload: Arc<T>,
    ) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
        self.transactional_operation(|dag, operation| {
            dag.intern_terminal_internal(payload, operation)
        })
    }

    pub(crate) fn branch(
        &mut self,
        atom: CoverageDecisionAtomId,
        when_false: CoverageDecisionRef,
        when_true: CoverageDecisionRef,
    ) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
        self.transactional_operation(|dag, operation| {
            dag.make_node_internal(atom, when_false, when_true, operation)
        })
    }

    pub(crate) fn boolean_terminals(
        &self,
        when_false: CoverageDecisionRef,
        when_true: CoverageDecisionRef,
    ) -> Result<CoverageDecisionBooleanTerminals, CoverageDecisionDagError> {
        self.validate_reference(when_false)?;
        self.validate_reference(when_true)?;
        if when_false.as_terminal().is_none() {
            return Err(CoverageDecisionDagError::BooleanTerminalExpected {
                reference: when_false,
            });
        }
        if when_true.as_terminal().is_none() {
            return Err(CoverageDecisionDagError::BooleanTerminalExpected {
                reference: when_true,
            });
        }
        if when_false == when_true {
            return Err(CoverageDecisionDagError::EqualBooleanTerminals);
        }
        Ok(CoverageDecisionBooleanTerminals {
            when_false,
            when_true,
        })
    }

    pub(crate) fn boolean_variable(
        &mut self,
        atom: CoverageDecisionAtomId,
        boolean: CoverageDecisionBooleanTerminals,
    ) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
        self.validate_boolean_terminals(boolean)?;
        self.branch(atom, boolean.when_false, boolean.when_true)
    }

    pub(crate) fn if_then_else(
        &mut self,
        condition: CoverageDecisionRef,
        boolean: CoverageDecisionBooleanTerminals,
        when_true: CoverageDecisionRef,
        when_false: CoverageDecisionRef,
    ) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
        self.transactional_operation(|dag, operation| {
            dag.if_then_else_internal(condition, boolean, when_true, when_false, operation)
        })
    }

    /// Direct iterative Shannon apply. Truth-table outputs must be terminals;
    /// correlated roots therefore never evaluate impossible table rows.
    pub(crate) fn apply_binary_truth_table(
        &mut self,
        left: CoverageDecisionRef,
        right: CoverageDecisionRef,
        boolean: CoverageDecisionBooleanTerminals,
        table: CoverageDecisionBinaryTruthTable,
    ) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
        self.transactional_operation(|dag, operation| {
            dag.apply_binary_truth_table_internal(left, right, boolean, table, operation)
        })
    }

    pub(crate) fn boolean_not(
        &mut self,
        root: CoverageDecisionRef,
        boolean: CoverageDecisionBooleanTerminals,
    ) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
        self.if_then_else(root, boolean, boolean.when_false, boolean.when_true)
    }

    pub(crate) fn boolean_and(
        &mut self,
        left: CoverageDecisionRef,
        right: CoverageDecisionRef,
        boolean: CoverageDecisionBooleanTerminals,
    ) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
        self.apply_binary_truth_table(
            left,
            right,
            boolean,
            CoverageDecisionBinaryTruthTable::new(
                boolean.when_false,
                boolean.when_false,
                boolean.when_false,
                boolean.when_true,
            ),
        )
    }

    pub(crate) fn boolean_or(
        &mut self,
        left: CoverageDecisionRef,
        right: CoverageDecisionRef,
        boolean: CoverageDecisionBooleanTerminals,
    ) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
        self.apply_binary_truth_table(
            left,
            right,
            boolean,
            CoverageDecisionBinaryTruthTable::new(
                boolean.when_false,
                boolean.when_true,
                boolean.when_true,
                boolean.when_true,
            ),
        )
    }

    pub(crate) fn compose_candidate_applicability(
        &mut self,
        bad_formula: CoverageDecisionRef,
        boolean: CoverageDecisionBooleanTerminals,
        applies: CoverageDecisionRef,
        continuation: CoverageDecisionRef,
    ) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
        self.if_then_else(bad_formula, boolean, continuation, applies)
    }

    /// Compose candidates in semantic priority order.
    ///
    /// Constant-true bad formulas are skipped. The first constant-false bad
    /// formula replaces the fallback and truncates every later continuation
    /// before any node, validation-cache, or operation-cache work is performed.
    pub(crate) fn compose_candidate_priority(
        &mut self,
        candidates: &[(CoverageDecisionRef, CoverageDecisionRef)],
        boolean: CoverageDecisionBooleanTerminals,
        fallback: CoverageDecisionRef,
    ) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
        self.with_operation(|operation| {
            operation.compose_candidate_priority(candidates, boolean, fallback)
        })
    }

    pub(crate) fn evaluate(
        &self,
        mut root: CoverageDecisionRef,
        mut atom_value: impl FnMut(CoverageDecisionAtomId) -> Option<bool>,
    ) -> Result<&T, CoverageDecisionDagError> {
        self.validate_reference(root)?;
        let mut steps = 0usize;
        loop {
            steps = checked_add("evaluation steps", steps, 1)?;
            check_limit("operation steps", steps, self.limits.max_operation_steps)?;
            match root.target() {
                CoverageDecisionRefTarget::Terminal(id) => {
                    return self.terminal_payload(id).ok_or(
                        CoverageDecisionDagError::TerminalReferenceOutOfRange {
                            ordinal: id.ordinal(),
                            terminal_count: self.terminals.len(),
                        },
                    );
                }
                CoverageDecisionRefTarget::Node(id) => {
                    let node =
                        self.node(id)
                            .ok_or(CoverageDecisionDagError::NodeReferenceOutOfRange {
                                ordinal: id.ordinal(),
                                node_count: self.nodes.len(),
                            })?;
                    root = match atom_value(node.atom) {
                        Some(false) => node.when_false,
                        Some(true) => node.when_true,
                        None => {
                            return Err(CoverageDecisionDagError::MissingAtomAssignment {
                                atom: node.atom,
                            });
                        }
                    };
                }
            }
        }
    }

    /// Project the live arena to the canonical sub-DAG reachable from `roots`
    /// and the two Boolean endpoints.
    ///
    /// Dead construction intermediates are omitted. The output uses only
    /// manager-independent ordinals and is therefore suitable for persistence
    /// and outer source-identity hashing. Root order is preserved verbatim.
    pub(crate) fn export_rooted(
        &mut self,
        roots: &[CoverageDecisionRef],
        boolean: CoverageDecisionBooleanTerminals,
    ) -> Result<CoverageDecisionDagRootedView<T>, CoverageDecisionDagError> {
        self.with_operation(|operation| operation.export_rooted(roots, boolean))
    }

    fn export_rooted_internal(
        &mut self,
        roots: &[CoverageDecisionRef],
        boolean: CoverageDecisionBooleanTerminals,
        operation: &mut OperationContext,
    ) -> Result<CoverageDecisionDagRootedView<T>, CoverageDecisionDagError> {
        let dag = self;
        check_limit(
            "persisted roots",
            roots.len(),
            dag.limits.max_persisted_roots,
        )?;
        charge_bounded_delta(
            "exported roots",
            &mut operation.work.exported_roots,
            roots.len(),
            dag.limits.max_exported_roots,
        )?;
        dag.validate_boolean_terminals(boolean)?;
        for &root in roots {
            dag.validate_reference(root)?;
        }

        let mut reachable = fixed_set::<CoverageDecisionRef>();
        let mut stack = Vec::new();
        push_work(
            operation,
            &mut stack,
            boolean.when_true,
            dag.limits.max_work_stack_entries,
        )?;
        push_work(
            operation,
            &mut stack,
            boolean.when_false,
            dag.limits.max_work_stack_entries,
        )?;
        for &root in roots.iter().rev() {
            push_work(
                operation,
                &mut stack,
                root,
                dag.limits.max_work_stack_entries,
            )?;
        }
        while let Some(reference) = stack.pop() {
            charge_step(operation, dag.limits)?;
            if reachable.contains(&reference) {
                increment_counter("reachability hits", &mut operation.work.reachability_hits)?;
                continue;
            }
            match reference.target() {
                CoverageDecisionRefTarget::Terminal(_) => {
                    charge_bounded_counter(
                        "exported terminals",
                        &mut operation.work.exported_terminals,
                        dag.limits.max_exported_terminals,
                    )?;
                    charge_bounded_counter(
                        "export remap entries",
                        &mut operation.work.export_remap_insertions,
                        dag.limits.max_export_remap_entries,
                    )?;
                }
                CoverageDecisionRefTarget::Node(_) => {
                    charge_bounded_counter(
                        "exported nodes",
                        &mut operation.work.exported_nodes,
                        dag.limits.max_exported_nodes,
                    )?;
                    charge_bounded_delta(
                        "exported edges",
                        &mut operation.work.exported_edges,
                        2,
                        dag.limits.max_exported_edges,
                    )?;
                    charge_bounded_counter(
                        "exported atom ordinals",
                        &mut operation.work.exported_atom_ordinals,
                        dag.limits.max_exported_atom_ordinals,
                    )?;
                    charge_bounded_counter(
                        "export remap entries",
                        &mut operation.work.export_remap_insertions,
                        dag.limits.max_export_remap_entries,
                    )?;
                }
            }
            let requested = checked_add("reachability entries", reachable.len(), 1)?;
            check_limit(
                "reachability entries",
                requested,
                dag.limits.max_reachability_entries,
            )?;
            try_reserve_set_one("reachability entries", &mut reachable)?;
            let inserted = reachable.insert(reference);
            debug_assert!(inserted);
            increment_counter(
                "reachability insertions",
                &mut operation.work.reachability_insertions,
            )?;
            if let CoverageDecisionRefTarget::Node(id) = reference.target() {
                let node =
                    dag.node(id)
                        .ok_or(CoverageDecisionDagError::NodeReferenceOutOfRange {
                            ordinal: id.ordinal(),
                            node_count: dag.nodes.len(),
                        })?;
                push_work(
                    operation,
                    &mut stack,
                    node.when_true,
                    dag.limits.max_work_stack_entries,
                )?;
                push_work(
                    operation,
                    &mut stack,
                    node.when_false,
                    dag.limits.max_work_stack_entries,
                )?;
            }
        }

        let mut terminal_remap =
            fixed_map::<CoverageDecisionTerminalId, CoverageDecisionTerminalId>();
        let mut node_remap = fixed_map::<CoverageDecisionNodeId, CoverageDecisionNodeId>();
        let mut terminal_payloads = Vec::new();
        for (ordinal, payload) in dag.terminals.iter().enumerate() {
            let old = CoverageDecisionTerminalId(ordinal);
            if !reachable.contains(&CoverageDecisionRef::terminal(dag.manager, old)) {
                continue;
            }
            let new = CoverageDecisionTerminalId(terminal_payloads.len());
            try_reserve_map_one("rooted terminal remap entries", &mut terminal_remap)?;
            let previous = terminal_remap.insert(old, new);
            debug_assert!(previous.is_none());
            try_reserve_vec_one("rooted terminal payload entries", &mut terminal_payloads)?;
            terminal_payloads.push(payload.clone());
        }

        let mut nodes = Vec::new();
        for (ordinal, node) in dag.nodes.iter().copied().enumerate() {
            let old = CoverageDecisionNodeId(ordinal);
            if !reachable.contains(&CoverageDecisionRef::node(dag.manager, old)) {
                continue;
            }
            let persisted = CoverageDecisionPersistedNode::new(
                node.atom,
                remap_persisted_reference(node.when_false, &terminal_remap, &node_remap)?,
                remap_persisted_reference(node.when_true, &terminal_remap, &node_remap)?,
            );
            let new = CoverageDecisionNodeId(nodes.len());
            try_reserve_map_one("rooted node remap entries", &mut node_remap)?;
            let previous = node_remap.insert(old, new);
            debug_assert!(previous.is_none());
            try_reserve_vec_one("rooted node entries", &mut nodes)?;
            nodes.push(persisted);
        }

        let mut persisted_roots = Vec::new();
        for &root in roots {
            try_reserve_vec_one("rooted persisted roots", &mut persisted_roots)?;
            persisted_roots.push(remap_persisted_reference(
                root,
                &terminal_remap,
                &node_remap,
            )?);
        }
        let boolean_false =
            remap_persisted_reference(boolean.when_false, &terminal_remap, &node_remap)?;
        let boolean_true =
            remap_persisted_reference(boolean.when_true, &terminal_remap, &node_remap)?;

        let mut payload_units = 0usize;
        let mut payload_references = 0usize;
        for payload in &terminal_payloads {
            let census = payload.coverage_decision_retained_census();
            payload_units = checked_add(
                "retained terminal payload units",
                payload_units,
                census.units,
            )?;
            payload_references = checked_add(
                "retained terminal payload references",
                payload_references,
                census.references,
            )?;
        }
        let retained = CoverageDecisionDagRetainedStats {
            terminals: terminal_payloads.len(),
            terminal_payload_units: payload_units,
            terminal_payload_references: payload_references,
            terminal_payload_handles: checked_mul(
                "retained terminal payload handles",
                terminal_payloads.len(),
                2,
            )?,
            nodes: nodes.len(),
            child_references: checked_mul("retained child references", nodes.len(), 2)?,
        };
        increment_counter("rooted exports", &mut operation.work.rooted_exports)?;
        Ok(CoverageDecisionDagRootedView {
            terminal_payloads,
            nodes,
            roots: persisted_roots,
            boolean_false,
            boolean_true,
            atom_count: dag.atom_count,
            retained,
        })
    }

    /// Rebuild and authenticate one complete, manager-independent rooted view.
    ///
    /// The returned roots and Boolean endpoints receive the rebuilt arena's new
    /// process-local brand. Source identity remains the responsibility of the
    /// outer certificate and hashes only the persisted view, never that brand.
    pub(crate) fn rebuild_rooted(
        view: &CoverageDecisionDagRootedView<T>,
        limits: CoverageDecisionDagLimits,
    ) -> Result<CoverageDecisionDagRebuild<T>, CoverageDecisionDagError> {
        Self::rebuild_rooted_from_views(
            view.terminal_payloads(),
            view.nodes(),
            view.roots(),
            view.boolean_false(),
            view.boolean_true(),
            view.atom_count(),
            view.retained_stats(),
            limits,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn rebuild_rooted_from_views(
        terminal_payloads: &[Arc<T>],
        nodes: &[CoverageDecisionPersistedNode],
        roots: &[CoverageDecisionPersistedRef],
        boolean_false: CoverageDecisionPersistedRef,
        boolean_true: CoverageDecisionPersistedRef,
        atom_count: usize,
        expected_retained: CoverageDecisionDagRetainedStats,
        limits: CoverageDecisionDagLimits,
    ) -> Result<CoverageDecisionDagRebuild<T>, CoverageDecisionDagError> {
        check_limit("coverage atoms", atom_count, limits.max_atoms)?;
        check_limit("persisted roots", roots.len(), limits.max_persisted_roots)?;
        check_limit("exported roots", roots.len(), limits.max_exported_roots)?;
        check_limit(
            "exported terminals",
            terminal_payloads.len(),
            limits.max_exported_terminals,
        )?;
        check_limit("exported nodes", nodes.len(), limits.max_exported_nodes)?;
        let exported_edges = checked_mul("exported edges", nodes.len(), 2)?;
        check_limit("exported edges", exported_edges, limits.max_exported_edges)?;
        check_limit(
            "exported atom ordinals",
            nodes.len(),
            limits.max_exported_atom_ordinals,
        )?;
        let export_remap_entries =
            checked_add("export remap entries", terminal_payloads.len(), nodes.len())?;
        check_limit(
            "export remap entries",
            export_remap_entries,
            limits.max_export_remap_entries,
        )?;
        check_limit("terminals", terminal_payloads.len(), limits.max_terminals)?;
        check_limit(
            "terminal index entries",
            terminal_payloads.len(),
            limits.max_terminal_index_entries,
        )?;
        let retained_payload_handles = checked_mul(
            "retained terminal payload handles",
            terminal_payloads.len(),
            2,
        )?;
        check_limit(
            "retained terminal payload handles",
            retained_payload_handles,
            limits.max_retained_terminal_payload_handles,
        )?;
        let mut retained_payload_units = 0usize;
        let mut retained_payload_references = 0usize;
        for payload in terminal_payloads {
            let census = payload.coverage_decision_retained_census();
            retained_payload_units = checked_add(
                "retained terminal payload units",
                retained_payload_units,
                census.units,
            )?;
            retained_payload_references = checked_add(
                "retained terminal payload references",
                retained_payload_references,
                census.references,
            )?;
        }
        check_limit(
            "retained terminal payload units",
            retained_payload_units,
            limits.max_retained_terminal_payload_units,
        )?;
        check_limit(
            "retained terminal payload references",
            retained_payload_references,
            limits.max_retained_terminal_payload_references,
        )?;
        check_limit("nodes", nodes.len(), limits.max_nodes)?;
        check_limit(
            "unique table entries",
            nodes.len(),
            limits.max_unique_table_entries,
        )?;
        let retained_child_references = checked_mul("retained child references", nodes.len(), 2)?;
        check_limit(
            "retained child references",
            retained_child_references,
            limits.max_retained_child_references,
        )?;
        check_limit(
            "terminal index lookups",
            terminal_payloads.len(),
            limits.max_terminal_index_lookups,
        )?;
        check_limit(
            "unique table lookups",
            nodes.len(),
            limits.max_unique_table_lookups,
        )?;
        let reachability_entries =
            checked_add("reachability entries", terminal_payloads.len(), nodes.len())?;
        check_limit(
            "reachability entries",
            reachability_entries,
            limits.max_reachability_entries,
        )?;
        let reachability_child_pushes = checked_mul("work stack pushes", nodes.len(), 2)?;
        let work_stack_pushes = checked_add(
            "work stack pushes",
            checked_add("work stack pushes", roots.len(), 2)?,
            reachability_child_pushes,
        )?;
        check_limit(
            "work stack pushes",
            work_stack_pushes,
            limits.max_work_stack_pushes,
        )?;
        check_limit(
            "operation steps",
            work_stack_pushes,
            limits.max_operation_steps,
        )?;
        let mut rebuilt = Self::new(atom_count, limits)?;
        let mut operation = OperationContext::new(limits);

        for (ordinal, payload) in terminal_payloads.iter().enumerate() {
            let resolved = rebuilt.intern_terminal_internal(payload.clone(), &mut operation)?;
            let expected =
                CoverageDecisionRef::terminal(rebuilt.manager, CoverageDecisionTerminalId(ordinal));
            if resolved != expected {
                return Err(CoverageDecisionDagError::NonCanonicalTerminalView {
                    ordinal,
                    existing: resolved
                        .as_terminal()
                        .expect("terminal interning returns a terminal"),
                });
            }
        }
        for (ordinal, node) in nodes.iter().copied().enumerate() {
            if node.atom.ordinal() >= atom_count {
                return Err(CoverageDecisionDagError::AtomOutOfRange {
                    atom: node.atom,
                    atom_count,
                });
            }
            let resolved = rebuilt.make_node_internal(
                node.atom,
                node.when_false.try_into_live(rebuilt.manager)?,
                node.when_true.try_into_live(rebuilt.manager)?,
                &mut operation,
            )?;
            let expected =
                CoverageDecisionRef::node(rebuilt.manager, CoverageDecisionNodeId(ordinal));
            if resolved != expected {
                return Err(CoverageDecisionDagError::NonCanonicalNodeView { ordinal, resolved });
            }
        }

        let boolean = rebuilt.boolean_terminals(
            boolean_false.try_into_live(rebuilt.manager)?,
            boolean_true.try_into_live(rebuilt.manager)?,
        )?;
        let mut rebuilt_roots = Vec::new();
        for (root_ordinal, &persisted) in roots.iter().enumerate() {
            let root = persisted.try_into_live(rebuilt.manager).map_err(|_| {
                CoverageDecisionDagError::PersistedRootOutOfRange {
                    root_ordinal,
                    reference: persisted,
                }
            })?;
            if rebuilt.validate_reference(root).is_err() {
                return Err(CoverageDecisionDagError::PersistedRootOutOfRange {
                    root_ordinal,
                    reference: persisted,
                });
            }
            try_reserve_vec_one("rebuilt persisted roots", &mut rebuilt_roots)?;
            rebuilt_roots.push(root);
        }
        rebuilt.validate_complete_reachability(&rebuilt_roots, boolean, &mut operation)?;
        let actual = rebuilt.retained_stats();
        if actual != expected_retained {
            return Err(CoverageDecisionDagError::RetainedStatsMismatch {
                expected: expected_retained,
                actual,
            });
        }
        rebuilt.commit_work(operation.work)?;
        Ok(CoverageDecisionDagRebuild {
            dag: rebuilt,
            roots: rebuilt_roots,
            boolean,
        })
    }

    fn intern_terminal_internal(
        &mut self,
        payload: Arc<T>,
        operation: &mut OperationContext,
    ) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
        charge_bounded_counter(
            "terminal index lookups",
            &mut operation.work.terminal_index_lookups,
            self.limits.max_terminal_index_lookups,
        )?;
        if let Some(&id) = self.terminal_index.get(&payload) {
            increment_counter("terminal reuse hits", &mut operation.work.terminal_reuses)?;
            return Ok(CoverageDecisionRef::terminal(self.manager, id));
        }

        let census = payload.coverage_decision_retained_census();
        let requested = checked_add("terminals", self.terminals.len(), 1)?;
        check_limit("terminals", requested, self.limits.max_terminals)?;
        check_limit(
            "terminal index entries",
            requested,
            self.limits.max_terminal_index_entries,
        )?;
        let payload_units = checked_add(
            "retained terminal payload units",
            self.retained_payload_units,
            census.units,
        )?;
        check_limit(
            "retained terminal payload units",
            payload_units,
            self.limits.max_retained_terminal_payload_units,
        )?;
        let payload_references = checked_add(
            "retained terminal payload references",
            self.retained_payload_references,
            census.references,
        )?;
        check_limit(
            "retained terminal payload references",
            payload_references,
            self.limits.max_retained_terminal_payload_references,
        )?;
        let payload_handles = checked_mul("retained terminal payload handles", requested, 2)?;
        check_limit(
            "retained terminal payload handles",
            payload_handles,
            self.limits.max_retained_terminal_payload_handles,
        )?;

        let ordinal = u64::try_from(self.terminals.len()).map_err(|_| {
            CoverageDecisionDagError::ResourceCountOverflow {
                resource: "encodable terminal ordinals",
            }
        })?;
        if ordinal & CoverageDecisionRef::NODE_TAG != 0 {
            return Err(CoverageDecisionDagError::ResourceLimit {
                resource: "encodable terminal ordinals",
                requested: self.terminals.len(),
                limit: (CoverageDecisionRef::NODE_TAG - 1) as usize,
            });
        }
        try_reserve_vec_one("terminal payload entries", &mut self.terminals)?;
        try_reserve_map_one("terminal index entries", &mut self.terminal_index)?;
        let id = CoverageDecisionTerminalId(self.terminals.len());
        self.terminals.push(payload.clone());
        let previous = self.terminal_index.insert(payload, id);
        debug_assert!(previous.is_none());
        self.retained_payload_units = payload_units;
        self.retained_payload_references = payload_references;
        Ok(CoverageDecisionRef::terminal(self.manager, id))
    }

    fn make_node_internal(
        &mut self,
        atom: CoverageDecisionAtomId,
        when_false: CoverageDecisionRef,
        when_true: CoverageDecisionRef,
        operation: &mut OperationContext,
    ) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
        if atom.ordinal() >= self.atom_count {
            return Err(CoverageDecisionDagError::AtomOutOfRange {
                atom,
                atom_count: self.atom_count,
            });
        }
        self.validate_reference(when_false)?;
        self.validate_reference(when_true)?;
        if when_false == when_true {
            increment_counter(
                "node reduction hits",
                &mut operation.work.node_reduction_hits,
            )?;
            return Ok(when_false);
        }
        for child in [when_false, when_true] {
            if let Some(child_atom) = self.top_atom(child)?
                && child_atom <= atom
            {
                return Err(CoverageDecisionDagError::VariableOrderViolation {
                    parent: atom,
                    child: child_atom,
                });
            }
        }
        let node = CoverageDecisionNode::new(atom, when_false, when_true);
        charge_bounded_counter(
            "unique table lookups",
            &mut operation.work.unique_table_lookups,
            self.limits.max_unique_table_lookups,
        )?;
        let node_hash = coverage_decision_node_hash(&node);
        if let Some(bucket) = self.unique_table.get(&node_hash) {
            for &(existing, id) in bucket {
                charge_bounded_counter(
                    "unique table comparisons",
                    &mut operation.work.unique_table_comparisons,
                    self.limits.max_unique_table_comparisons,
                )?;
                if existing == node {
                    increment_counter(
                        "unique table reuse hits",
                        &mut operation.work.unique_table_reuses,
                    )?;
                    return Ok(CoverageDecisionRef::node(self.manager, id));
                }
            }
        }

        let requested_nodes = checked_add("nodes", self.nodes.len(), 1)?;
        check_limit("nodes", requested_nodes, self.limits.max_nodes)?;
        check_limit(
            "unique table entries",
            requested_nodes,
            self.limits.max_unique_table_entries,
        )?;
        let references = checked_mul("retained child references", requested_nodes, 2)?;
        check_limit(
            "retained child references",
            references,
            self.limits.max_retained_child_references,
        )?;
        let ordinal = u64::try_from(self.nodes.len()).map_err(|_| {
            CoverageDecisionDagError::ResourceCountOverflow {
                resource: "encodable node ordinals",
            }
        })?;
        if ordinal & CoverageDecisionRef::NODE_TAG != 0 {
            return Err(CoverageDecisionDagError::ResourceLimit {
                resource: "encodable node ordinals",
                requested: self.nodes.len(),
                limit: (CoverageDecisionRef::NODE_TAG - 1) as usize,
            });
        }
        let id = CoverageDecisionNodeId(self.nodes.len());
        if let Some(bucket) = self.unique_table.get_mut(&node_hash) {
            try_reserve_vec_one("unique-table collision entries", bucket)?;
            try_reserve_vec_one("branch node entries", &mut self.nodes)?;
            self.nodes.push(node);
            bucket.push((node, id));
        } else {
            let mut bucket = Vec::new();
            try_reserve_vec_one("unique-table collision entries", &mut bucket)?;
            try_reserve_map_one("unique-table hash buckets", &mut self.unique_table)?;
            try_reserve_vec_one("branch node entries", &mut self.nodes)?;
            self.nodes.push(node);
            bucket.push((node, id));
            let previous = self.unique_table.insert(node_hash, bucket);
            debug_assert!(previous.is_none());
        }
        Ok(CoverageDecisionRef::node(self.manager, id))
    }

    fn validate_reference(
        &self,
        reference: CoverageDecisionRef,
    ) -> Result<(), CoverageDecisionDagError> {
        if reference.manager != self.manager {
            return Err(CoverageDecisionDagError::ForeignManagerReference {
                expected: self.manager,
                actual: reference.manager,
            });
        }
        match reference.target() {
            CoverageDecisionRefTarget::Terminal(id) if id.ordinal() >= self.terminals.len() => {
                Err(CoverageDecisionDagError::TerminalReferenceOutOfRange {
                    ordinal: id.ordinal(),
                    terminal_count: self.terminals.len(),
                })
            }
            CoverageDecisionRefTarget::Node(id) if id.ordinal() >= self.nodes.len() => {
                Err(CoverageDecisionDagError::NodeReferenceOutOfRange {
                    ordinal: id.ordinal(),
                    node_count: self.nodes.len(),
                })
            }
            _ => Ok(()),
        }
    }

    fn validate_boolean_terminals(
        &self,
        boolean: CoverageDecisionBooleanTerminals,
    ) -> Result<(), CoverageDecisionDagError> {
        let rebuilt = self.boolean_terminals(boolean.when_false, boolean.when_true)?;
        debug_assert_eq!(rebuilt, boolean);
        Ok(())
    }

    fn validate_boolean_function(
        &self,
        root: CoverageDecisionRef,
        boolean: CoverageDecisionBooleanTerminals,
        operation: &mut OperationContext,
    ) -> Result<(), CoverageDecisionDagError> {
        self.validate_boolean_terminals(boolean)?;
        self.validate_reference(root)?;
        if let CoverageDecisionRefTarget::Terminal(id) = root.target() {
            if root != boolean.when_false && root != boolean.when_true {
                return Err(CoverageDecisionDagError::NonBooleanConditionTerminal { terminal: id });
            }
            return Ok(());
        }

        let mut stack = Vec::new();
        push_work(
            operation,
            &mut stack,
            root,
            self.limits.max_work_stack_entries,
        )?;
        while let Some(reference) = stack.pop() {
            charge_step(operation, self.limits)?;
            match reference.target() {
                CoverageDecisionRefTarget::Terminal(id) => {
                    if reference != boolean.when_false && reference != boolean.when_true {
                        return Err(CoverageDecisionDagError::NonBooleanConditionTerminal {
                            terminal: id,
                        });
                    }
                }
                CoverageDecisionRefTarget::Node(id) => {
                    let validation = ValidationKey { node: id, boolean };
                    if operation.validated_nodes.contains(&validation) {
                        increment_counter(
                            "Boolean validation hits",
                            &mut operation.work.boolean_validation_hits,
                        )?;
                        continue;
                    }
                    let requested = checked_add(
                        "Boolean validation entries",
                        operation.validated_nodes.len(),
                        1,
                    )?;
                    check_limit(
                        "Boolean validation entries",
                        requested,
                        self.limits.max_boolean_validation_entries,
                    )?;
                    try_reserve_set_one(
                        "Boolean validation entries",
                        &mut operation.validated_nodes,
                    )?;
                    let inserted = operation.validated_nodes.insert(validation);
                    debug_assert!(inserted);
                    increment_counter(
                        "Boolean validation insertions",
                        &mut operation.work.boolean_validation_insertions,
                    )?;
                    let node =
                        self.node(id)
                            .ok_or(CoverageDecisionDagError::NodeReferenceOutOfRange {
                                ordinal: id.ordinal(),
                                node_count: self.nodes.len(),
                            })?;
                    push_work(
                        operation,
                        &mut stack,
                        node.when_true,
                        self.limits.max_work_stack_entries,
                    )?;
                    push_work(
                        operation,
                        &mut stack,
                        node.when_false,
                        self.limits.max_work_stack_entries,
                    )?;
                }
            }
        }
        Ok(())
    }

    fn if_then_else_internal(
        &mut self,
        condition: CoverageDecisionRef,
        boolean: CoverageDecisionBooleanTerminals,
        when_true: CoverageDecisionRef,
        when_false: CoverageDecisionRef,
        operation: &mut OperationContext,
    ) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
        self.validate_boolean_function(condition, boolean, operation)?;
        self.validate_reference(when_true)?;
        self.validate_reference(when_false)?;
        self.if_then_else_prevalidated(condition, boolean, when_true, when_false, operation)
    }

    fn if_then_else_prevalidated(
        &mut self,
        condition: CoverageDecisionRef,
        boolean: CoverageDecisionBooleanTerminals,
        when_true: CoverageDecisionRef,
        when_false: CoverageDecisionRef,
        operation: &mut OperationContext,
    ) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
        let root = IteKey {
            condition,
            when_true,
            when_false,
            boolean,
        };
        if let Some(result) = terminal_ite_result(root, boolean)? {
            return Ok(result);
        }
        charge_bounded_counter(
            "ITE calls",
            &mut operation.work.ite_calls,
            self.limits.max_ite_calls,
        )?;
        let mut stack = Vec::<IteFrame>::new();
        push_work(
            operation,
            &mut stack,
            IteFrame::Enter(root),
            self.limits.max_work_stack_entries,
        )?;

        while let Some(frame) = stack.pop() {
            charge_step(operation, self.limits)?;
            match frame {
                IteFrame::Enter(key) => {
                    if ite_cache_get(operation, &key)?.is_some() {
                        continue;
                    }
                    if let Some(result) = terminal_ite_result(key, boolean)? {
                        insert_ite_cache(operation, key, result, self.limits)?;
                        continue;
                    }
                    let atom = self.minimum_top_atom_ite(key)?;
                    let low = IteKey {
                        condition: self.cofactor(key.condition, atom, false)?,
                        when_true: self.cofactor(key.when_true, atom, false)?,
                        when_false: self.cofactor(key.when_false, atom, false)?,
                        boolean: key.boolean,
                    };
                    let high = IteKey {
                        condition: self.cofactor(key.condition, atom, true)?,
                        when_true: self.cofactor(key.when_true, atom, true)?,
                        when_false: self.cofactor(key.when_false, atom, true)?,
                        boolean: key.boolean,
                    };
                    push_work(
                        operation,
                        &mut stack,
                        IteFrame::Exit {
                            key,
                            atom,
                            low,
                            high,
                        },
                        self.limits.max_work_stack_entries,
                    )?;
                    if !operation.ite_cache.contains_key(&high) {
                        push_work(
                            operation,
                            &mut stack,
                            IteFrame::Enter(high),
                            self.limits.max_work_stack_entries,
                        )?;
                    }
                    if !operation.ite_cache.contains_key(&low) {
                        push_work(
                            operation,
                            &mut stack,
                            IteFrame::Enter(low),
                            self.limits.max_work_stack_entries,
                        )?;
                    }
                }
                IteFrame::Exit {
                    key,
                    atom,
                    low,
                    high,
                } => {
                    if operation.ite_cache.contains_key(&key) {
                        continue;
                    }
                    let low_result = operation
                        .ite_cache
                        .get(&low)
                        .copied()
                        .ok_or(CoverageDecisionDagError::InternalVariableOrderMismatch)?;
                    let high_result = operation
                        .ite_cache
                        .get(&high)
                        .copied()
                        .ok_or(CoverageDecisionDagError::InternalVariableOrderMismatch)?;
                    let result =
                        self.make_node_internal(atom, low_result, high_result, operation)?;
                    insert_ite_cache(operation, key, result, self.limits)?;
                }
            }
        }
        operation
            .ite_cache
            .get(&root)
            .copied()
            .ok_or(CoverageDecisionDagError::InternalVariableOrderMismatch)
    }

    fn apply_binary_truth_table_internal(
        &mut self,
        left: CoverageDecisionRef,
        right: CoverageDecisionRef,
        boolean: CoverageDecisionBooleanTerminals,
        table: CoverageDecisionBinaryTruthTable,
        operation: &mut OperationContext,
    ) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
        self.validate_boolean_function(left, boolean, operation)?;
        self.validate_boolean_function(right, boolean, operation)?;
        for output in table.outputs() {
            self.validate_reference(output)?;
            if output.as_terminal().is_none() {
                return Err(CoverageDecisionDagError::TruthTableOutputTerminalExpected {
                    reference: output,
                });
            }
        }
        let root = ApplyKey {
            left,
            right,
            boolean,
            table,
        };
        if let Some(result) = terminal_apply_result(root, boolean, table)? {
            return Ok(result);
        }
        charge_bounded_counter(
            "binary apply calls",
            &mut operation.work.binary_apply_calls,
            self.limits.max_binary_apply_calls,
        )?;
        let mut stack = Vec::<ApplyFrame>::new();
        push_work(
            operation,
            &mut stack,
            ApplyFrame::Enter(root),
            self.limits.max_work_stack_entries,
        )?;
        while let Some(frame) = stack.pop() {
            charge_step(operation, self.limits)?;
            match frame {
                ApplyFrame::Enter(key) => {
                    if apply_cache_get(operation, &key)?.is_some() {
                        continue;
                    }
                    if let Some(result) = terminal_apply_result(key, boolean, table)? {
                        insert_apply_cache(operation, key, result, self.limits)?;
                        continue;
                    }
                    let atom = self.minimum_top_atom_apply(key)?;
                    let low = ApplyKey {
                        left: self.cofactor(key.left, atom, false)?,
                        right: self.cofactor(key.right, atom, false)?,
                        boolean: key.boolean,
                        table: key.table,
                    };
                    let high = ApplyKey {
                        left: self.cofactor(key.left, atom, true)?,
                        right: self.cofactor(key.right, atom, true)?,
                        boolean: key.boolean,
                        table: key.table,
                    };
                    push_work(
                        operation,
                        &mut stack,
                        ApplyFrame::Exit {
                            key,
                            atom,
                            low,
                            high,
                        },
                        self.limits.max_work_stack_entries,
                    )?;
                    if !operation.apply_cache.contains_key(&high) {
                        push_work(
                            operation,
                            &mut stack,
                            ApplyFrame::Enter(high),
                            self.limits.max_work_stack_entries,
                        )?;
                    }
                    if !operation.apply_cache.contains_key(&low) {
                        push_work(
                            operation,
                            &mut stack,
                            ApplyFrame::Enter(low),
                            self.limits.max_work_stack_entries,
                        )?;
                    }
                }
                ApplyFrame::Exit {
                    key,
                    atom,
                    low,
                    high,
                } => {
                    if operation.apply_cache.contains_key(&key) {
                        continue;
                    }
                    let low_result = operation
                        .apply_cache
                        .get(&low)
                        .copied()
                        .ok_or(CoverageDecisionDagError::InternalVariableOrderMismatch)?;
                    let high_result = operation
                        .apply_cache
                        .get(&high)
                        .copied()
                        .ok_or(CoverageDecisionDagError::InternalVariableOrderMismatch)?;
                    let result =
                        self.make_node_internal(atom, low_result, high_result, operation)?;
                    insert_apply_cache(operation, key, result, self.limits)?;
                }
            }
        }
        operation
            .apply_cache
            .get(&root)
            .copied()
            .ok_or(CoverageDecisionDagError::InternalVariableOrderMismatch)
    }

    fn minimum_top_atom_ite(
        &self,
        key: IteKey,
    ) -> Result<CoverageDecisionAtomId, CoverageDecisionDagError> {
        self.minimum_top_atom([key.condition, key.when_true, key.when_false])
    }

    fn minimum_top_atom_apply(
        &self,
        key: ApplyKey,
    ) -> Result<CoverageDecisionAtomId, CoverageDecisionDagError> {
        self.minimum_top_atom([key.left, key.right])
    }

    fn minimum_top_atom<const N: usize>(
        &self,
        references: [CoverageDecisionRef; N],
    ) -> Result<CoverageDecisionAtomId, CoverageDecisionDagError> {
        let mut minimum = None;
        for reference in references {
            if let Some(atom) = self.top_atom(reference)? {
                minimum =
                    Some(minimum.map_or(atom, |current: CoverageDecisionAtomId| current.min(atom)));
            }
        }
        minimum.ok_or(CoverageDecisionDagError::InternalVariableOrderMismatch)
    }

    fn top_atom(
        &self,
        reference: CoverageDecisionRef,
    ) -> Result<Option<CoverageDecisionAtomId>, CoverageDecisionDagError> {
        self.validate_reference(reference)?;
        Ok(match reference.target() {
            CoverageDecisionRefTarget::Terminal(_) => None,
            CoverageDecisionRefTarget::Node(id) => Some(
                self.node(id)
                    .ok_or(CoverageDecisionDagError::NodeReferenceOutOfRange {
                        ordinal: id.ordinal(),
                        node_count: self.nodes.len(),
                    })?
                    .atom,
            ),
        })
    }

    fn cofactor(
        &self,
        reference: CoverageDecisionRef,
        atom: CoverageDecisionAtomId,
        value: bool,
    ) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
        let Some(top) = self.top_atom(reference)? else {
            return Ok(reference);
        };
        if top > atom {
            return Ok(reference);
        }
        if top < atom {
            return Err(CoverageDecisionDagError::InternalVariableOrderMismatch);
        }
        let node = self
            .node(
                reference
                    .as_node()
                    .ok_or(CoverageDecisionDagError::InternalVariableOrderMismatch)?,
            )
            .ok_or(CoverageDecisionDagError::InternalVariableOrderMismatch)?;
        Ok(if value {
            node.when_true
        } else {
            node.when_false
        })
    }

    fn validate_complete_reachability(
        &self,
        roots: &[CoverageDecisionRef],
        boolean: CoverageDecisionBooleanTerminals,
        operation: &mut OperationContext,
    ) -> Result<(), CoverageDecisionDagError> {
        let mut reachable = fixed_set::<CoverageDecisionRef>();
        let mut stack = Vec::new();
        push_work(
            operation,
            &mut stack,
            boolean.when_true,
            self.limits.max_work_stack_entries,
        )?;
        push_work(
            operation,
            &mut stack,
            boolean.when_false,
            self.limits.max_work_stack_entries,
        )?;
        for &root in roots.iter().rev() {
            push_work(
                operation,
                &mut stack,
                root,
                self.limits.max_work_stack_entries,
            )?;
        }
        while let Some(reference) = stack.pop() {
            charge_step(operation, self.limits)?;
            if reachable.contains(&reference) {
                increment_counter("reachability hits", &mut operation.work.reachability_hits)?;
                continue;
            }
            let requested = checked_add("reachability entries", reachable.len(), 1)?;
            check_limit(
                "reachability entries",
                requested,
                self.limits.max_reachability_entries,
            )?;
            try_reserve_set_one("reachability entries", &mut reachable)?;
            let inserted = reachable.insert(reference);
            debug_assert!(inserted);
            increment_counter(
                "reachability insertions",
                &mut operation.work.reachability_insertions,
            )?;
            if let CoverageDecisionRefTarget::Node(id) = reference.target() {
                let node =
                    self.node(id)
                        .ok_or(CoverageDecisionDagError::NodeReferenceOutOfRange {
                            ordinal: id.ordinal(),
                            node_count: self.nodes.len(),
                        })?;
                push_work(
                    operation,
                    &mut stack,
                    node.when_true,
                    self.limits.max_work_stack_entries,
                )?;
                push_work(
                    operation,
                    &mut stack,
                    node.when_false,
                    self.limits.max_work_stack_entries,
                )?;
            }
        }
        for ordinal in 0..self.terminals.len() {
            let terminal = CoverageDecisionTerminalId(ordinal);
            if !reachable.contains(&CoverageDecisionRef::terminal(self.manager, terminal)) {
                return Err(CoverageDecisionDagError::UnreachableTerminal { terminal });
            }
        }
        for ordinal in 0..self.nodes.len() {
            let node = CoverageDecisionNodeId(ordinal);
            if !reachable.contains(&CoverageDecisionRef::node(self.manager, node)) {
                return Err(CoverageDecisionDagError::UnreachableNode { node });
            }
        }
        Ok(())
    }

    fn transactional_operation<R>(
        &mut self,
        operation: impl FnOnce(&mut Self, &mut OperationContext) -> Result<R, CoverageDecisionDagError>,
    ) -> Result<R, CoverageDecisionDagError> {
        let checkpoint = self.checkpoint();
        let mut context = OperationContext::new(self.limits);
        match operation(self, &mut context) {
            Ok(value) => match self.commit_work(context.work) {
                Ok(()) => Ok(value),
                Err(error) => {
                    self.rollback(checkpoint);
                    Err(error)
                }
            },
            Err(error) => {
                self.rollback(checkpoint);
                Err(error)
            }
        }
    }

    fn checkpoint(&self) -> DagCheckpoint {
        DagCheckpoint {
            terminals: self.terminals.len(),
            nodes: self.nodes.len(),
            payload_units: self.retained_payload_units,
            payload_references: self.retained_payload_references,
        }
    }

    fn rollback(&mut self, checkpoint: DagCheckpoint) {
        while self.nodes.len() > checkpoint.nodes {
            let ordinal = self.nodes.len() - 1;
            let node = self.nodes.pop().expect("checkpoint retains a node");
            let node_hash = coverage_decision_node_hash(&node);
            let remove_bucket = {
                let bucket = self
                    .unique_table
                    .get_mut(&node_hash)
                    .expect("checkpoint retains a unique-table bucket");
                let removed = bucket
                    .pop()
                    .expect("checkpoint retains a unique-table entry");
                debug_assert_eq!(removed, (node, CoverageDecisionNodeId(ordinal)));
                bucket.is_empty()
            };
            if remove_bucket {
                let removed = self.unique_table.remove(&node_hash);
                debug_assert!(removed.is_some());
            }
        }
        while self.terminals.len() > checkpoint.terminals {
            let ordinal = self.terminals.len() - 1;
            let payload = self.terminals.pop().expect("checkpoint retains a terminal");
            let removed = self.terminal_index.remove(&payload);
            debug_assert_eq!(removed, Some(CoverageDecisionTerminalId(ordinal)));
        }
        self.retained_payload_units = checkpoint.payload_units;
        self.retained_payload_references = checkpoint.payload_references;
    }

    fn commit_work(
        &mut self,
        delta: CoverageDecisionDagWorkStats,
    ) -> Result<(), CoverageDecisionDagError> {
        let mut next = self.work_stats;
        next.public_operations =
            checked_add("committed public operations", next.public_operations, 1)?;
        macro_rules! add_fields {
            ($($field:ident),+ $(,)?) => {
                $(next.$field = checked_add(
                    stringify!($field),
                    next.$field,
                    delta.$field,
                )?;)+
            };
        }
        add_fields!(
            operation_steps,
            work_stack_pushes,
            terminal_index_lookups,
            terminal_reuses,
            unique_table_lookups,
            unique_table_comparisons,
            node_reduction_hits,
            unique_table_reuses,
            boolean_validation_insertions,
            boolean_validation_hits,
            ite_calls,
            ite_cache_insertions,
            ite_cache_hits,
            binary_apply_calls,
            apply_cache_insertions,
            apply_cache_hits,
            priority_candidates,
            reachability_insertions,
            reachability_hits,
            rooted_exports,
            exported_terminals,
            exported_nodes,
            exported_roots,
            exported_edges,
            exported_atom_ordinals,
            export_remap_insertions,
        );
        next.work_stack_peak = next.work_stack_peak.max(delta.work_stack_peak);
        self.work_stats = next;
        Ok(())
    }
}

/// Mutable façade sharing one [`OperationContext`] across a complete compile.
///
/// Every method below contributes to the same cumulative resource counters and
/// all logical appends roll back together if the closure passed to
/// [`CoverageDecisionDag::with_operation`] returns an error. The first method
/// error also poisons the scope: catching it inside the closure cannot commit a
/// partially validated or partially appended arena.
pub(crate) struct CoverageDecisionDagOperation<'a, T: CoverageDecisionTerminalPayload> {
    dag: &'a mut CoverageDecisionDag<T>,
    context: &'a mut OperationContext,
}

impl<T: CoverageDecisionTerminalPayload> CoverageDecisionDagOperation<'_, T> {
    fn ensure_active(&self) -> Result<(), CoverageDecisionDagError> {
        match self.context.first_failure.clone() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn record<R>(
        &mut self,
        result: Result<R, CoverageDecisionDagError>,
    ) -> Result<R, CoverageDecisionDagError> {
        if let Some(error) = self.context.first_failure.clone() {
            return Err(error);
        }
        if let Err(error) = &result {
            self.context.first_failure = Some(error.clone());
        }
        result
    }

    pub(crate) fn intern_terminal(
        &mut self,
        payload: Arc<T>,
    ) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
        self.ensure_active()?;
        let result = self.dag.intern_terminal_internal(payload, self.context);
        self.record(result)
    }

    pub(crate) fn branch(
        &mut self,
        atom: CoverageDecisionAtomId,
        when_false: CoverageDecisionRef,
        when_true: CoverageDecisionRef,
    ) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
        self.ensure_active()?;
        let result = self
            .dag
            .make_node_internal(atom, when_false, when_true, self.context);
        self.record(result)
    }

    pub(crate) fn boolean_terminals(
        &mut self,
        when_false: CoverageDecisionRef,
        when_true: CoverageDecisionRef,
    ) -> Result<CoverageDecisionBooleanTerminals, CoverageDecisionDagError> {
        self.ensure_active()?;
        let result = self.dag.boolean_terminals(when_false, when_true);
        self.record(result)
    }

    pub(crate) fn boolean_variable(
        &mut self,
        atom: CoverageDecisionAtomId,
        boolean: CoverageDecisionBooleanTerminals,
    ) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
        self.ensure_active()?;
        let result = (|| {
            self.dag.validate_boolean_terminals(boolean)?;
            self.dag
                .make_node_internal(atom, boolean.when_false, boolean.when_true, self.context)
        })();
        self.record(result)
    }

    pub(crate) fn if_then_else(
        &mut self,
        condition: CoverageDecisionRef,
        boolean: CoverageDecisionBooleanTerminals,
        when_true: CoverageDecisionRef,
        when_false: CoverageDecisionRef,
    ) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
        self.ensure_active()?;
        let result =
            self.dag
                .if_then_else_internal(condition, boolean, when_true, when_false, self.context);
        self.record(result)
    }

    pub(crate) fn apply_binary_truth_table(
        &mut self,
        left: CoverageDecisionRef,
        right: CoverageDecisionRef,
        boolean: CoverageDecisionBooleanTerminals,
        table: CoverageDecisionBinaryTruthTable,
    ) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
        self.ensure_active()?;
        let result =
            self.dag
                .apply_binary_truth_table_internal(left, right, boolean, table, self.context);
        self.record(result)
    }

    pub(crate) fn boolean_not(
        &mut self,
        root: CoverageDecisionRef,
        boolean: CoverageDecisionBooleanTerminals,
    ) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
        self.if_then_else(root, boolean, boolean.when_false, boolean.when_true)
    }

    pub(crate) fn boolean_and(
        &mut self,
        left: CoverageDecisionRef,
        right: CoverageDecisionRef,
        boolean: CoverageDecisionBooleanTerminals,
    ) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
        self.apply_binary_truth_table(
            left,
            right,
            boolean,
            CoverageDecisionBinaryTruthTable::new(
                boolean.when_false,
                boolean.when_false,
                boolean.when_false,
                boolean.when_true,
            ),
        )
    }

    pub(crate) fn boolean_or(
        &mut self,
        left: CoverageDecisionRef,
        right: CoverageDecisionRef,
        boolean: CoverageDecisionBooleanTerminals,
    ) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
        self.apply_binary_truth_table(
            left,
            right,
            boolean,
            CoverageDecisionBinaryTruthTable::new(
                boolean.when_false,
                boolean.when_true,
                boolean.when_true,
                boolean.when_true,
            ),
        )
    }

    pub(crate) fn compose_candidate_applicability(
        &mut self,
        bad_formula: CoverageDecisionRef,
        boolean: CoverageDecisionBooleanTerminals,
        applies: CoverageDecisionRef,
        continuation: CoverageDecisionRef,
    ) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
        self.if_then_else(bad_formula, boolean, continuation, applies)
    }

    pub(crate) fn compose_candidate_priority(
        &mut self,
        candidates: &[(CoverageDecisionRef, CoverageDecisionRef)],
        boolean: CoverageDecisionBooleanTerminals,
        fallback: CoverageDecisionRef,
    ) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
        self.ensure_active()?;
        let result = (|| {
            self.dag.validate_boolean_terminals(boolean)?;
            let mut cutoff = None;
            for (ordinal, &(bad_formula, applies)) in candidates.iter().enumerate() {
                charge_bounded_counter(
                    "priority candidates",
                    &mut self.context.work.priority_candidates,
                    self.dag.limits.max_priority_candidates,
                )?;
                self.dag.validate_reference(bad_formula)?;
                if bad_formula == boolean.when_false {
                    self.dag.validate_reference(applies)?;
                    cutoff = Some(ordinal);
                    break;
                }
                if bad_formula != boolean.when_true {
                    self.dag.validate_reference(applies)?;
                }
            }

            let mut continuation = match cutoff {
                Some(ordinal) => candidates[ordinal].1,
                None => {
                    self.dag.validate_reference(fallback)?;
                    fallback
                }
            };
            let end = cutoff.unwrap_or(candidates.len());
            for &(bad_formula, applies) in candidates[..end].iter().rev() {
                if bad_formula == boolean.when_true {
                    continue;
                }
                continuation = self.dag.if_then_else_internal(
                    bad_formula,
                    boolean,
                    continuation,
                    applies,
                    self.context,
                )?;
            }
            Ok(continuation)
        })();
        self.record(result)
    }

    /// Export within this same checkpoint and cumulative work budget. If export
    /// fails, all construction performed earlier in the surrounding scope is
    /// rolled back by [`CoverageDecisionDag::with_operation`].
    pub(crate) fn export_rooted(
        &mut self,
        roots: &[CoverageDecisionRef],
        boolean: CoverageDecisionBooleanTerminals,
    ) -> Result<CoverageDecisionDagRootedView<T>, CoverageDecisionDagError> {
        self.ensure_active()?;
        let result = self
            .dag
            .export_rooted_internal(roots, boolean, self.context);
        self.record(result)
    }
}

fn coverage_decision_node_hash(node: &CoverageDecisionNode) -> u64 {
    // Explicit FNV-1a over the persisted triple. The live manager nonce is not
    // included, so collision buckets and exact comparison charges reproduce
    // across arenas and Rust toolchains.
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mix = |state: u64, value: u64| (state ^ value).wrapping_mul(PRIME);
    let state = mix(OFFSET, node.atom.ordinal() as u64);
    let state = mix(state, node.when_false.encoded_target);
    mix(state, node.when_true.encoded_target)
}

fn remap_persisted_reference(
    reference: CoverageDecisionRef,
    terminal_remap: &FixedHashMap<CoverageDecisionTerminalId, CoverageDecisionTerminalId>,
    node_remap: &FixedHashMap<CoverageDecisionNodeId, CoverageDecisionNodeId>,
) -> Result<CoverageDecisionPersistedRef, CoverageDecisionDagError> {
    match reference.target() {
        CoverageDecisionRefTarget::Terminal(id) => terminal_remap
            .get(&id)
            .copied()
            .map(CoverageDecisionPersistedRef::Terminal)
            .ok_or(CoverageDecisionDagError::InternalVariableOrderMismatch),
        CoverageDecisionRefTarget::Node(id) => node_remap
            .get(&id)
            .copied()
            .map(CoverageDecisionPersistedRef::Node)
            .ok_or(CoverageDecisionDagError::InternalVariableOrderMismatch),
    }
}

fn terminal_ite_result(
    key: IteKey,
    boolean: CoverageDecisionBooleanTerminals,
) -> Result<Option<CoverageDecisionRef>, CoverageDecisionDagError> {
    if key.when_true == key.when_false {
        return Ok(Some(key.when_true));
    }
    match key.condition.target() {
        _ if key.condition == boolean.when_false => Ok(Some(key.when_false)),
        _ if key.condition == boolean.when_true => Ok(Some(key.when_true)),
        CoverageDecisionRefTarget::Terminal(terminal) => {
            Err(CoverageDecisionDagError::NonBooleanConditionTerminal { terminal })
        }
        CoverageDecisionRefTarget::Node(_) => Ok(None),
    }
}

fn terminal_apply_result(
    key: ApplyKey,
    boolean: CoverageDecisionBooleanTerminals,
    table: CoverageDecisionBinaryTruthTable,
) -> Result<Option<CoverageDecisionRef>, CoverageDecisionDagError> {
    let [ff, ft, tf, tt] = table.outputs();
    if ff == ft && ff == tf && ff == tt {
        return Ok(Some(ff));
    }
    if key.left == key.right && ff == tt {
        return Ok(Some(ff));
    }
    if key.left == boolean.when_false && ff == ft {
        return Ok(Some(ff));
    }
    if key.left == boolean.when_true && tf == tt {
        return Ok(Some(tf));
    }
    if key.right == boolean.when_false && ff == tf {
        return Ok(Some(ff));
    }
    if key.right == boolean.when_true && ft == tt {
        return Ok(Some(ft));
    }
    let left = match key.left.target() {
        _ if key.left == boolean.when_false => Some(false),
        _ if key.left == boolean.when_true => Some(true),
        CoverageDecisionRefTarget::Terminal(terminal) => {
            return Err(CoverageDecisionDagError::NonBooleanConditionTerminal { terminal });
        }
        CoverageDecisionRefTarget::Node(_) => None,
    };
    let right = match key.right.target() {
        _ if key.right == boolean.when_false => Some(false),
        _ if key.right == boolean.when_true => Some(true),
        CoverageDecisionRefTarget::Terminal(terminal) => {
            return Err(CoverageDecisionDagError::NonBooleanConditionTerminal { terminal });
        }
        CoverageDecisionRefTarget::Node(_) => None,
    };
    Ok(match (left, right) {
        (Some(left), Some(right)) => Some(table.select(left, right)),
        _ => None,
    })
}

fn ite_cache_get(
    operation: &mut OperationContext,
    key: &IteKey,
) -> Result<Option<CoverageDecisionRef>, CoverageDecisionDagError> {
    let result = operation.ite_cache.get(key).copied();
    if result.is_some() {
        increment_counter("ITE cache hits", &mut operation.work.ite_cache_hits)?;
    }
    Ok(result)
}

fn apply_cache_get(
    operation: &mut OperationContext,
    key: &ApplyKey,
) -> Result<Option<CoverageDecisionRef>, CoverageDecisionDagError> {
    let result = operation.apply_cache.get(key).copied();
    if result.is_some() {
        increment_counter("apply cache hits", &mut operation.work.apply_cache_hits)?;
    }
    Ok(result)
}

fn insert_ite_cache(
    operation: &mut OperationContext,
    key: IteKey,
    value: CoverageDecisionRef,
    limits: CoverageDecisionDagLimits,
) -> Result<(), CoverageDecisionDagError> {
    if operation.ite_cache.contains_key(&key) {
        return Ok(());
    }
    let requested = checked_add("ITE cache entries", operation.ite_cache.len(), 1)?;
    check_limit("ITE cache entries", requested, limits.max_ite_cache_entries)?;
    check_total_cache_limit(operation, limits, 1)?;
    try_reserve_map_one("ITE cache entries", &mut operation.ite_cache)?;
    operation.ite_cache.insert(key, value);
    increment_counter(
        "ITE cache insertions",
        &mut operation.work.ite_cache_insertions,
    )
}

fn insert_apply_cache(
    operation: &mut OperationContext,
    key: ApplyKey,
    value: CoverageDecisionRef,
    limits: CoverageDecisionDagLimits,
) -> Result<(), CoverageDecisionDagError> {
    if operation.apply_cache.contains_key(&key) {
        return Ok(());
    }
    let requested = checked_add("apply cache entries", operation.apply_cache.len(), 1)?;
    check_limit(
        "apply cache entries",
        requested,
        limits.max_apply_cache_entries,
    )?;
    check_total_cache_limit(operation, limits, 1)?;
    try_reserve_map_one("apply cache entries", &mut operation.apply_cache)?;
    operation.apply_cache.insert(key, value);
    increment_counter(
        "apply cache insertions",
        &mut operation.work.apply_cache_insertions,
    )
}

fn check_total_cache_limit(
    operation: &OperationContext,
    limits: CoverageDecisionDagLimits,
    delta: usize,
) -> Result<(), CoverageDecisionDagError> {
    let existing = checked_add(
        "total operation cache entries",
        operation.ite_cache.len(),
        operation.apply_cache.len(),
    )?;
    let requested = checked_add("total operation cache entries", existing, delta)?;
    check_limit(
        "total operation cache entries",
        requested,
        limits.max_total_operation_cache_entries,
    )
}

fn charge_step(
    operation: &mut OperationContext,
    limits: CoverageDecisionDagLimits,
) -> Result<(), CoverageDecisionDagError> {
    charge_bounded_counter(
        "operation steps",
        &mut operation.work.operation_steps,
        limits.max_operation_steps,
    )
}

fn charge_bounded_counter(
    resource: &'static str,
    counter: &mut usize,
    limit: usize,
) -> Result<(), CoverageDecisionDagError> {
    let requested = checked_add(resource, *counter, 1)?;
    check_limit(resource, requested, limit)?;
    *counter = requested;
    Ok(())
}

fn charge_bounded_delta(
    resource: &'static str,
    counter: &mut usize,
    delta: usize,
    limit: usize,
) -> Result<(), CoverageDecisionDagError> {
    let requested = checked_add(resource, *counter, delta)?;
    check_limit(resource, requested, limit)?;
    *counter = requested;
    Ok(())
}

fn increment_counter(
    resource: &'static str,
    counter: &mut usize,
) -> Result<(), CoverageDecisionDagError> {
    *counter = checked_add(resource, *counter, 1)?;
    Ok(())
}

fn push_work<T>(
    operation: &mut OperationContext,
    work: &mut Vec<T>,
    value: T,
    limit: usize,
) -> Result<(), CoverageDecisionDagError> {
    let requested = checked_add("work stack entries", work.len(), 1)?;
    check_limit("work stack entries", requested, limit)?;
    let requested_pushes = checked_add("work stack pushes", operation.work.work_stack_pushes, 1)?;
    check_limit(
        "work stack pushes",
        requested_pushes,
        operation.limits.max_work_stack_pushes,
    )?;
    if work.len() == work.capacity() {
        work.try_reserve_exact(1)
            .map_err(|_| CoverageDecisionDagError::AllocationFailure {
                resource: "work stack entries",
                requested: 1,
            })?;
    }
    work.push(value);
    operation.work.work_stack_pushes = requested_pushes;
    operation.work.work_stack_peak = operation.work.work_stack_peak.max(work.len());
    Ok(())
}

fn try_reserve_vec_one<T>(
    resource: &'static str,
    values: &mut Vec<T>,
) -> Result<(), CoverageDecisionDagError> {
    if values.len() == values.capacity() {
        values
            .try_reserve_exact(1)
            .map_err(|_| CoverageDecisionDagError::AllocationFailure {
                resource,
                requested: 1,
            })?;
    }
    Ok(())
}

fn try_reserve_map_one<K: Eq + Hash, V>(
    resource: &'static str,
    values: &mut FixedHashMap<K, V>,
) -> Result<(), CoverageDecisionDagError> {
    values
        .try_reserve(1)
        .map_err(|_| CoverageDecisionDagError::AllocationFailure {
            resource,
            requested: 1,
        })
}

fn try_reserve_set_one<K: Eq + Hash>(
    resource: &'static str,
    values: &mut FixedHashSet<K>,
) -> Result<(), CoverageDecisionDagError> {
    values
        .try_reserve(1)
        .map_err(|_| CoverageDecisionDagError::AllocationFailure {
            resource,
            requested: 1,
        })
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, CoverageDecisionDagError> {
    left.checked_add(right)
        .ok_or(CoverageDecisionDagError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, CoverageDecisionDagError> {
    left.checked_mul(right)
        .ok_or(CoverageDecisionDagError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), CoverageDecisionDagError> {
    if requested > limit {
        Err(CoverageDecisionDagError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    static COUNTING_CENSUS_CALLS: AtomicUsize = AtomicUsize::new(0);

    #[derive(Debug, PartialEq, Eq, Hash)]
    struct Terminal {
        label: u16,
        units: usize,
        references: usize,
    }

    #[derive(Debug, PartialEq, Eq, Hash)]
    struct CountingTerminal {
        label: u8,
        units: usize,
        references: usize,
    }

    impl CoverageDecisionTerminalPayload for CountingTerminal {
        fn coverage_decision_retained_census(&self) -> CoverageDecisionTerminalPayloadCensus {
            COUNTING_CENSUS_CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            CoverageDecisionTerminalPayloadCensus::new(self.units, self.references)
        }
    }

    impl CoverageDecisionTerminalPayload for Terminal {
        fn coverage_decision_retained_census(&self) -> CoverageDecisionTerminalPayloadCensus {
            CoverageDecisionTerminalPayloadCensus::new(self.units, self.references)
        }
    }

    fn payload(label: u16) -> Arc<Terminal> {
        Arc::new(Terminal {
            label,
            units: usize::from(label) + 2,
            references: usize::from(label % 3),
        })
    }

    fn manager(
        limits: CoverageDecisionDagLimits,
    ) -> (
        CoverageDecisionDag<Terminal>,
        CoverageDecisionBooleanTerminals,
    ) {
        let mut dag = CoverageDecisionDag::new(limits.max_atoms, limits).unwrap();
        let when_false = dag.intern_terminal(payload(0)).unwrap();
        let when_true = dag.intern_terminal(payload(1)).unwrap();
        let boolean = dag.boolean_terminals(when_false, when_true).unwrap();
        (dag, boolean)
    }

    fn manager_with_atoms(
        atom_count: usize,
        limits: CoverageDecisionDagLimits,
    ) -> (
        CoverageDecisionDag<Terminal>,
        CoverageDecisionBooleanTerminals,
    ) {
        let mut dag = CoverageDecisionDag::new(atom_count, limits).unwrap();
        let when_false = dag.intern_terminal(payload(0)).unwrap();
        let when_true = dag.intern_terminal(payload(1)).unwrap();
        let boolean = dag.boolean_terminals(when_false, when_true).unwrap();
        (dag, boolean)
    }

    fn intern(dag: &mut CoverageDecisionDag<Terminal>, label: u16) -> CoverageDecisionRef {
        dag.intern_terminal(payload(label)).unwrap()
    }

    fn label(
        dag: &CoverageDecisionDag<Terminal>,
        root: CoverageDecisionRef,
        values: &[bool],
    ) -> u16 {
        dag.evaluate(root, |atom| values.get(atom.ordinal()).copied())
            .unwrap()
            .label
    }

    fn boolean_table(
        boolean: CoverageDecisionBooleanTerminals,
        mask: u8,
    ) -> CoverageDecisionBinaryTruthTable {
        let output = |bit: u8| {
            if mask & (1 << bit) == 0 {
                boolean.when_false()
            } else {
                boolean.when_true()
            }
        };
        CoverageDecisionBinaryTruthTable::new(output(0), output(1), output(2), output(3))
    }

    #[test]
    fn terminal_payload_is_single_owned_and_exactly_metered() {
        let mut limits = CoverageDecisionDagLimits::default();
        limits.max_retained_terminal_payload_units = 17;
        limits.max_retained_terminal_payload_references = 2;
        limits.max_retained_terminal_payload_handles = 2;
        let mut dag = CoverageDecisionDag::new(0, limits).unwrap();
        let terminal = Arc::new(Terminal {
            label: 90,
            units: 17,
            references: 2,
        });
        let root = dag.intern_terminal(terminal.clone()).unwrap();
        assert_eq!(Arc::strong_count(&terminal), 3); // caller, vector, index
        assert_eq!(dag.find_terminal(&terminal), Some(root));
        assert_eq!(dag.intern_terminal(terminal.clone()).unwrap(), root);
        let equal_but_distinct = Arc::new(Terminal {
            label: 90,
            units: 17,
            references: 2,
        });
        assert!(!Arc::ptr_eq(&terminal, &equal_but_distinct));
        assert_eq!(
            dag.intern_terminal(equal_but_distinct.clone()).unwrap(),
            root
        );
        assert_eq!(Arc::strong_count(&equal_but_distinct), 1);
        assert!(Arc::ptr_eq(&dag.terminal_payloads()[0], &terminal));
        assert_eq!(dag.retained_stats().terminal_payload_units, 17);
        assert_eq!(dag.retained_stats().terminal_payload_references, 2);
        assert_eq!(dag.retained_stats().terminal_payload_handles, 2);
        assert_eq!(dag.stats().work.terminal_reuses, 2);

        let mut one_below_handles = CoverageDecisionDagLimits::default();
        one_below_handles.max_retained_terminal_payload_handles = 1;
        let mut rejected_handles = CoverageDecisionDag::new(0, one_below_handles).unwrap();
        let handle_payload = payload(7);
        assert_eq!(Arc::strong_count(&handle_payload), 1);
        assert!(matches!(
            rejected_handles.intern_terminal(handle_payload.clone()),
            Err(CoverageDecisionDagError::ResourceLimit {
                resource: "retained terminal payload handles",
                requested: 2,
                limit: 1,
            })
        ));
        assert_eq!(Arc::strong_count(&handle_payload), 1);
        assert_eq!(
            rejected_handles.retained_stats(),
            CoverageDecisionDagRetainedStats::default()
        );

        let mut one_below_units = limits;
        one_below_units.max_retained_terminal_payload_units = 16;
        let mut rejected = CoverageDecisionDag::new(0, one_below_units).unwrap();
        assert!(matches!(
            rejected.intern_terminal(terminal.clone()),
            Err(CoverageDecisionDagError::ResourceLimit {
                resource: "retained terminal payload units",
                requested: 17,
                limit: 16,
            })
        ));
        assert_eq!(
            rejected.retained_stats(),
            CoverageDecisionDagRetainedStats::default()
        );

        let mut one_below_references = limits;
        one_below_references.max_retained_terminal_payload_references = 1;
        let mut rejected = CoverageDecisionDag::new(0, one_below_references).unwrap();
        assert!(matches!(
            rejected.intern_terminal(terminal),
            Err(CoverageDecisionDagError::ResourceLimit {
                resource: "retained terminal payload references",
                requested: 2,
                limit: 1,
            })
        ));
    }

    #[test]
    fn live_handles_are_manager_branded_and_declared_atom_count_is_exact() {
        assert_eq!(
            std::mem::size_of::<CoverageDecisionRef>(),
            2 * std::mem::size_of::<u64>()
        );
        let mut atom_limits = CoverageDecisionDagLimits::default();
        atom_limits.max_atoms = 2;
        assert!(CoverageDecisionDag::<Terminal>::new(2, atom_limits).is_ok());
        assert!(matches!(
            CoverageDecisionDag::<Terminal>::new(3, atom_limits),
            Err(CoverageDecisionDagError::ResourceLimit {
                resource: "coverage atoms",
                requested: 3,
                limit: 2,
            })
        ));

        let (mut first, first_boolean) = manager_with_atoms(2, atom_limits);
        let (mut second, second_boolean) = manager_with_atoms(2, atom_limits);
        let first_x = first
            .boolean_variable(CoverageDecisionAtomId::new(0), first_boolean)
            .unwrap();
        let second_x = second
            .boolean_variable(CoverageDecisionAtomId::new(0), second_boolean)
            .unwrap();
        assert_eq!(first_x.as_node(), second_x.as_node());
        assert_ne!(first_x, second_x);
        assert_ne!(first.manager_id(), second.manager_id());

        assert!(matches!(
            second.evaluate(first_x, |_| Some(false)),
            Err(CoverageDecisionDagError::ForeignManagerReference { .. })
        ));
        assert!(matches!(
            second.branch(
                CoverageDecisionAtomId::new(1),
                first_boolean.when_false(),
                second_boolean.when_true(),
            ),
            Err(CoverageDecisionDagError::ForeignManagerReference { .. })
        ));
        assert!(matches!(
            second.boolean_terminals(first_boolean.when_false(), first_boolean.when_true(),),
            Err(CoverageDecisionDagError::ForeignManagerReference { .. })
        ));
        assert!(matches!(
            second.apply_binary_truth_table(
                second_x,
                second_x,
                second_boolean,
                CoverageDecisionBinaryTruthTable::new(
                    second_boolean.when_false(),
                    first_boolean.when_true(),
                    second_boolean.when_true(),
                    second_boolean.when_false(),
                ),
            ),
            Err(CoverageDecisionDagError::ForeignManagerReference { .. })
        ));
        assert!(matches!(
            second.compose_candidate_priority(
                &[(first_x, second_boolean.when_true())],
                second_boolean,
                second_boolean.when_false(),
            ),
            Err(CoverageDecisionDagError::ForeignManagerReference { .. })
        ));

        let selected = intern(&mut second, 8);
        assert_eq!(
            second
                .compose_candidate_priority(
                    &[
                        (second_boolean.when_false(), selected),
                        (first_x, first_boolean.when_true()),
                    ],
                    second_boolean,
                    second_boolean.when_true(),
                )
                .unwrap(),
            selected
        );
        let before_nodes = second.retained_stats().nodes;
        assert!(matches!(
            second.branch(
                CoverageDecisionAtomId::new(2),
                second_boolean.when_false(),
                second_boolean.when_true(),
            ),
            Err(CoverageDecisionDagError::AtomOutOfRange { atom_count: 2, .. })
        ));
        assert_eq!(second.retained_stats().nodes, before_nodes);
    }

    #[test]
    fn reduction_hash_consing_and_order_are_exact() {
        let (mut dag, boolean) = manager(CoverageDecisionDagLimits::default());
        let x = dag
            .boolean_variable(CoverageDecisionAtomId::new(0), boolean)
            .unwrap();
        assert_eq!(
            dag.boolean_variable(CoverageDecisionAtomId::new(0), boolean)
                .unwrap(),
            x
        );
        assert_eq!(
            dag.branch(
                CoverageDecisionAtomId::new(10),
                boolean.when_false(),
                boolean.when_false(),
            )
            .unwrap(),
            boolean.when_false()
        );
        assert!(matches!(
            dag.branch(CoverageDecisionAtomId::new(1), boolean.when_false(), x),
            Err(CoverageDecisionDagError::VariableOrderViolation { .. })
        ));
        assert_eq!(dag.retained_stats().nodes, 1);
        assert_eq!(dag.stats().work.unique_table_reuses, 1);
        assert_eq!(dag.stats().work.node_reduction_hits, 1);
    }

    #[test]
    fn all_sixteen_truth_tables_handle_shared_correlated_and_mtbdd_outputs() {
        let (mut dag, boolean) = manager(CoverageDecisionDagLimits::default());
        let x = dag
            .boolean_variable(CoverageDecisionAtomId::new(0), boolean)
            .unwrap();
        let y = dag
            .boolean_variable(CoverageDecisionAtomId::new(1), boolean)
            .unwrap();
        let shared = dag.boolean_and(x, y, boolean).unwrap();

        for mask in 0u8..16 {
            let root = dag
                .apply_binary_truth_table(x, y, boolean, boolean_table(boolean, mask))
                .unwrap();
            let correlated = dag
                .apply_binary_truth_table(x, x, boolean, boolean_table(boolean, mask))
                .unwrap();
            let shared_root = dag
                .apply_binary_truth_table(shared, x, boolean, boolean_table(boolean, mask))
                .unwrap();
            for xv in [false, true] {
                for yv in [false, true] {
                    let values = [xv, yv];
                    let bit = u8::from(xv) * 2 + u8::from(yv);
                    let expected = u16::from((mask >> bit) & 1);
                    assert_eq!(label(&dag, root, &values), expected);
                    let correlated_bit = if xv { 3 } else { 0 };
                    assert_eq!(
                        label(&dag, correlated, &values),
                        u16::from((mask >> correlated_bit) & 1)
                    );
                    let left = xv && yv;
                    let shared_bit = u8::from(left) * 2 + u8::from(xv);
                    assert_eq!(
                        label(&dag, shared_root, &values),
                        u16::from((mask >> shared_bit) & 1)
                    );
                }
            }
        }

        let first = intern(&mut dag, 2);
        let second = intern(&mut dag, 3);
        let fallback = intern(&mut dag, 4);
        let fourth = intern(&mut dag, 5);
        let multi = dag
            .apply_binary_truth_table(
                x,
                y,
                boolean,
                CoverageDecisionBinaryTruthTable::new(first, second, fallback, fourth),
            )
            .unwrap();
        assert_eq!(label(&dag, multi, &[false, false]), 2);
        assert_eq!(label(&dag, multi, &[false, true]), 3);
        assert_eq!(label(&dag, multi, &[true, false]), 4);
        assert_eq!(label(&dag, multi, &[true, true]), 5);
    }

    #[test]
    fn shared_operation_caches_bind_truth_table_and_oriented_boolean_pair() {
        let (mut dag, boolean) = manager_with_atoms(2, CoverageDecisionDagLimits::default());
        let x = dag
            .boolean_variable(CoverageDecisionAtomId::new(0), boolean)
            .unwrap();
        let y = dag
            .boolean_variable(CoverageDecisionAtomId::new(1), boolean)
            .unwrap();
        let selected_true = intern(&mut dag, 2);
        let selected_false = intern(&mut dag, 3);
        let alternate_true = intern(&mut dag, 4);
        let alternate = dag
            .boolean_terminals(boolean.when_false(), alternate_true)
            .unwrap();
        let mixed_condition = dag
            .branch(
                CoverageDecisionAtomId::new(0),
                boolean.when_false(),
                alternate_true,
            )
            .unwrap();
        let before = dag.stats().work;

        let (and_root, or_root, repeated_and, normal_selection, swapped_selection) = dag
            .with_operation(|operation| {
                let and_root = operation.boolean_and(x, y, boolean)?;
                let or_root = operation.boolean_or(x, y, boolean)?;
                let repeated_and = operation.boolean_and(x, y, boolean)?;

                let normal_selection =
                    operation.if_then_else(x, boolean, selected_true, selected_false)?;
                let swapped =
                    operation.boolean_terminals(boolean.when_true(), boolean.when_false())?;
                let swapped_selection =
                    operation.if_then_else(x, swapped, selected_true, selected_false)?;

                Ok((
                    and_root,
                    or_root,
                    repeated_and,
                    normal_selection,
                    swapped_selection,
                ))
            })
            .unwrap();

        assert_eq!(and_root, repeated_and);
        assert_ne!(and_root, or_root);
        assert_eq!(label(&dag, and_root, &[false, true]), 0);
        assert_eq!(label(&dag, or_root, &[false, true]), 1);
        assert_eq!(label(&dag, normal_selection, &[false, false]), 3);
        assert_eq!(label(&dag, normal_selection, &[true, false]), 2);
        assert_eq!(label(&dag, swapped_selection, &[false, false]), 2);
        assert_eq!(label(&dag, swapped_selection, &[true, false]), 3);
        let after = dag.stats().work;
        assert!(after.apply_cache_hits > before.apply_cache_hits);
        assert!(after.ite_cache_insertions > before.ite_cache_insertions);
        assert!(after.boolean_validation_insertions > before.boolean_validation_insertions);

        let before_failure = dag.stats();
        let invalid = dag
            .with_operation(|operation| {
                operation.if_then_else(
                    mixed_condition,
                    alternate,
                    selected_true,
                    selected_false,
                )?;
                let caught = operation
                    .if_then_else(mixed_condition, boolean, selected_true, selected_false)
                    .expect_err("validation must not leak across endpoint pairs");
                let retry = operation
                    .if_then_else(mixed_condition, boolean, selected_true, selected_true)
                    .expect_err("a caught method error must poison the operation");
                assert_eq!(retry, caught);
                Ok(())
            })
            .expect_err("a poisoned operation must roll back even if its closure returns Ok");
        assert!(matches!(
            invalid,
            CoverageDecisionDagError::NonBooleanConditionTerminal { .. }
        ));
        assert_eq!(dag.retained_stats(), before_failure.retained);
        assert_eq!(dag.stats().work, before_failure.work);
    }

    #[test]
    fn constant_apply_row_does_not_build_or_charge_an_unreachable_row() {
        let mut limits = CoverageDecisionDagLimits::default();
        limits.max_nodes = 1;
        limits.max_unique_table_entries = 1;
        limits.max_retained_child_references = 2;
        limits.max_apply_cache_entries = 0;
        limits.max_total_operation_cache_entries = 0;
        limits.max_binary_apply_calls = 0;
        let (mut dag, boolean) = manager(limits);
        let x = dag
            .boolean_variable(CoverageDecisionAtomId::new(0), boolean)
            .unwrap();
        let before = dag.stats();
        let root = dag
            .apply_binary_truth_table(
                boolean.when_false(),
                x,
                boolean,
                CoverageDecisionBinaryTruthTable::new(
                    boolean.when_false(),
                    boolean.when_false(),
                    boolean.when_true(),
                    boolean.when_false(),
                ),
            )
            .unwrap();
        assert_eq!(root, boolean.when_false());
        assert_eq!(dag.retained_stats(), before.retained);
        assert_eq!(
            dag.stats().work.binary_apply_calls,
            before.work.binary_apply_calls
        );
        assert_eq!(
            dag.stats().work.apply_cache_insertions,
            before.work.apply_cache_insertions
        );
    }

    #[test]
    fn constant_priority_skips_true_and_truncates_at_first_false_without_work() {
        let mut limits = CoverageDecisionDagLimits::default();
        limits.max_operation_steps = 0;
        limits.max_boolean_validation_entries = 0;
        limits.max_ite_calls = 0;
        limits.max_ite_cache_entries = 0;
        limits.max_total_operation_cache_entries = 0;
        let (mut dag, boolean) = manager(limits);
        let later_bad = dag
            .boolean_variable(CoverageDecisionAtomId::new(0), boolean)
            .unwrap();
        let skipped = intern(&mut dag, 2);
        let selected = intern(&mut dag, 3);
        let unreachable = intern(&mut dag, 4);
        let fallback = intern(&mut dag, 5);
        let before = dag.stats();
        let root = dag
            .compose_candidate_priority(
                &[
                    (boolean.when_true(), skipped),
                    (boolean.when_false(), selected),
                    (later_bad, unreachable),
                ],
                boolean,
                fallback,
            )
            .unwrap();
        assert_eq!(root, selected);
        assert_eq!(dag.retained_stats(), before.retained);
        let after = dag.stats().work;
        assert_eq!(after.operation_steps, before.work.operation_steps);
        assert_eq!(
            after.boolean_validation_insertions,
            before.work.boolean_validation_insertions
        );
        assert_eq!(after.ite_calls, before.work.ite_calls);
        assert_eq!(after.ite_cache_insertions, before.work.ite_cache_insertions);
    }

    #[test]
    fn independent_two_disjunct_candidates_remain_linear() {
        const CANDIDATES: usize = 12;
        let (mut dag, boolean) = manager(CoverageDecisionDagLimits::default());
        let mut bad = Vec::new();
        let mut applies = Vec::new();
        for candidate in 0..CANDIDATES {
            let left = dag
                .boolean_variable(CoverageDecisionAtomId::new(candidate * 2), boolean)
                .unwrap();
            let right = dag
                .boolean_variable(CoverageDecisionAtomId::new(candidate * 2 + 1), boolean)
                .unwrap();
            bad.push(dag.boolean_or(left, right, boolean).unwrap());
            applies.push(intern(&mut dag, u16::try_from(candidate + 10).unwrap()));
        }
        let fallback = intern(&mut dag, 100);
        let candidates = bad
            .iter()
            .copied()
            .zip(applies.iter().copied())
            .collect::<Vec<_>>();
        let before = dag.retained_stats().nodes;
        let root = dag
            .compose_candidate_priority(&candidates, boolean, fallback)
            .unwrap();
        let priority_nodes = dag.retained_stats().nodes - before;
        assert!(priority_nodes <= 2 * CANDIDATES);

        for selected in 0..CANDIDATES {
            let mut values = vec![false; 2 * CANDIDATES];
            for earlier in 0..selected {
                values[earlier * 2] = true;
            }
            assert_eq!(
                label(&dag, root, &values),
                u16::try_from(selected + 10).unwrap()
            );
        }
        assert!(2usize.pow(u32::try_from(CANDIDATES).unwrap()) > priority_nodes);
    }

    #[test]
    fn deep_priority_chain_is_iterative_and_preserves_first_applicable_candidate() {
        const CANDIDATES: usize = 4_096;
        let mut limits = CoverageDecisionDagLimits::default();
        limits.max_nodes = 3 * CANDIDATES;
        limits.max_unique_table_entries = 3 * CANDIDATES;
        limits.max_retained_child_references = 6 * CANDIDATES;
        let (mut dag, boolean) = manager_with_atoms(CANDIDATES, limits);

        let mut candidates = Vec::with_capacity(CANDIDATES);
        for ordinal in 0..CANDIDATES {
            let bad = dag
                .boolean_variable(CoverageDecisionAtomId::new(ordinal), boolean)
                .unwrap();
            let applies = intern(&mut dag, u16::try_from(ordinal + 2).unwrap());
            candidates.push((bad, applies));
        }
        let fallback = intern(&mut dag, u16::try_from(CANDIDATES + 2).unwrap());

        let before_nodes = dag.retained_stats().nodes;
        let before_priority = dag.stats().work.priority_candidates;
        let root = dag
            .compose_candidate_priority(&candidates, boolean, fallback)
            .unwrap();
        assert_eq!(
            dag.stats().work.priority_candidates - before_priority,
            CANDIDATES
        );
        assert_eq!(dag.retained_stats().nodes - before_nodes, CANDIDATES);

        let mut values = vec![false; CANDIDATES];
        assert_eq!(label(&dag, root, &values), 2);
        for selected in [1, CANDIDATES / 2, CANDIDATES - 1] {
            values[..selected].fill(true);
            values[selected..].fill(false);
            assert_eq!(
                label(&dag, root, &values),
                u16::try_from(selected + 2).unwrap()
            );
        }
        values.fill(true);
        assert_eq!(
            label(&dag, root, &values),
            u16::try_from(CANDIDATES + 2).unwrap()
        );
    }

    #[test]
    fn caught_append_then_fail_rolls_back_and_retries_ids() {
        let mut limits = CoverageDecisionDagLimits::default();
        limits.max_nodes = 1;
        limits.max_unique_table_entries = 1;
        limits.max_retained_child_references = 2;
        let (mut dag, boolean) = manager_with_atoms(2, limits);
        let before = dag.stats();

        let error = dag
            .with_operation(|operation| {
                let selected = operation.intern_terminal(payload(2))?;
                let appended = operation.branch(
                    CoverageDecisionAtomId::new(0),
                    boolean.when_false(),
                    selected,
                )?;
                assert_eq!(appended.as_node(), Some(CoverageDecisionNodeId::new(0)));
                let caught = operation
                    .branch(
                        CoverageDecisionAtomId::new(1),
                        boolean.when_true(),
                        selected,
                    )
                    .expect_err("the second node must exceed the configured limit");
                assert!(matches!(
                    caught,
                    CoverageDecisionDagError::ResourceLimit {
                        resource: "nodes",
                        requested: 2,
                        limit: 1,
                    }
                ));
                assert_eq!(
                    operation
                        .intern_terminal(payload(3))
                        .expect_err("the operation remains poisoned after a caught error"),
                    caught
                );
                Ok(())
            })
            .expect_err("a caught failure must still abort and roll back the operation");
        assert!(matches!(
            error,
            CoverageDecisionDagError::ResourceLimit {
                resource: "nodes",
                requested: 2,
                limit: 1,
            }
        ));
        assert_eq!(dag.retained_stats(), before.retained);
        assert_eq!(dag.stats().work, before.work);

        let selected = intern(&mut dag, 2);
        assert_eq!(
            selected.as_terminal(),
            Some(CoverageDecisionTerminalId::new(2))
        );
        let retried = dag
            .branch(
                CoverageDecisionAtomId::new(0),
                boolean.when_false(),
                selected,
            )
            .unwrap();
        assert_eq!(retried.as_node(), Some(CoverageDecisionNodeId::new(0)));
    }

    #[test]
    fn append_then_fail_rolls_back_logically_exposes_capacity_and_retries_ids() {
        let mut limits = CoverageDecisionDagLimits::default();
        limits.max_nodes = 3;
        limits.max_unique_table_entries = 3;
        limits.max_retained_child_references = 6;
        let (mut dag, boolean) = manager(limits);
        let x = dag
            .boolean_variable(CoverageDecisionAtomId::new(0), boolean)
            .unwrap();
        let y = dag
            .boolean_variable(CoverageDecisionAtomId::new(1), boolean)
            .unwrap();
        let before_nodes = dag.nodes().to_vec();
        let before_capacity = dag.stats().capacity;
        assert!(matches!(
            dag.apply_binary_truth_table(
                x,
                y,
                boolean,
                CoverageDecisionBinaryTruthTable::new(
                    boolean.when_false(),
                    boolean.when_true(),
                    boolean.when_true(),
                    boolean.when_false(),
                ),
            ),
            Err(CoverageDecisionDagError::ResourceLimit {
                resource: "nodes",
                requested: 4,
                limit: 3,
            })
        ));
        assert_eq!(dag.nodes(), before_nodes);
        assert!(dag.stats().capacity.node_vector_entries >= before_capacity.node_vector_entries);

        let retried = dag
            .branch(
                CoverageDecisionAtomId::new(1),
                boolean.when_true(),
                boolean.when_false(),
            )
            .unwrap();
        assert_eq!(retried.as_node(), Some(CoverageDecisionNodeId::new(2)));

        let (mut fresh, fresh_boolean) = manager(limits);
        fresh
            .boolean_variable(CoverageDecisionAtomId::new(0), fresh_boolean)
            .unwrap();
        fresh
            .boolean_variable(CoverageDecisionAtomId::new(1), fresh_boolean)
            .unwrap();
        let fresh_retried = fresh
            .branch(
                CoverageDecisionAtomId::new(1),
                fresh_boolean.when_true(),
                fresh_boolean.when_false(),
            )
            .unwrap();
        assert_eq!(fresh_retried.as_node(), retried.as_node());
        assert_ne!(fresh_retried, retried);
        let original_view = dag.export_rooted(&[retried], boolean).unwrap();
        let fresh_view = fresh
            .export_rooted(&[fresh_retried], fresh_boolean)
            .unwrap();
        assert_eq!(fresh_view, original_view);
        assert_eq!(fresh.stats().work, dag.stats().work);
    }

    #[test]
    fn rooted_export_prunes_rebuilds_and_checks_atoms_roots_endpoints_and_stats() {
        let limits = CoverageDecisionDagLimits::default();
        let (mut dag, boolean) = manager_with_atoms(1, limits);
        let x = dag
            .boolean_variable(CoverageDecisionAtomId::new(0), boolean)
            .unwrap();
        let selected = intern(&mut dag, 2);
        let mtbdd = dag
            .if_then_else(x, boolean, selected, boolean.when_false())
            .unwrap();
        let dead_payload = intern(&mut dag, 9);
        let _dead_node = dag
            .branch(
                CoverageDecisionAtomId::new(0),
                boolean.when_true(),
                dead_payload,
            )
            .unwrap();

        let view = dag.export_rooted(&[mtbdd], boolean).unwrap();
        let repeated = dag.export_rooted(&[mtbdd], boolean).unwrap();
        assert_eq!(view, repeated);
        assert_eq!(view.atom_count(), 1);
        assert_eq!(view.terminal_payloads().len(), 3);
        assert_eq!(view.nodes().len(), 1);
        assert!(
            !view
                .terminal_payloads()
                .iter()
                .any(|payload| payload.label == 9)
        );

        let rebuilt = CoverageDecisionDag::rebuild_rooted(&view, limits).unwrap();
        assert_ne!(rebuilt.dag().manager_id(), dag.manager_id());
        assert_ne!(rebuilt.roots()[0], mtbdd);
        assert_eq!(rebuilt.dag().retained_stats(), view.retained_stats());
        assert_eq!(label(rebuilt.dag(), rebuilt.roots()[0], &[false]), 0);
        assert_eq!(label(rebuilt.dag(), rebuilt.roots()[0], &[true]), 2);
        for (left, right) in rebuilt
            .dag()
            .terminal_payloads()
            .iter()
            .zip(view.terminal_payloads())
        {
            assert!(Arc::ptr_eq(left, right));
        }

        assert!(matches!(
            CoverageDecisionDag::rebuild_rooted_from_views(
                view.terminal_payloads(),
                view.nodes(),
                &[CoverageDecisionPersistedRef::Node(
                    CoverageDecisionNodeId::new(99)
                )],
                view.boolean_false(),
                view.boolean_true(),
                view.atom_count(),
                view.retained_stats(),
                limits,
            ),
            Err(CoverageDecisionDagError::PersistedRootOutOfRange { .. })
        ));
        assert!(matches!(
            CoverageDecisionDag::rebuild_rooted_from_views(
                view.terminal_payloads(),
                view.nodes(),
                view.roots(),
                CoverageDecisionPersistedRef::Node(CoverageDecisionNodeId::new(0)),
                view.boolean_true(),
                view.atom_count(),
                view.retained_stats(),
                limits,
            ),
            Err(CoverageDecisionDagError::BooleanTerminalExpected { .. })
        ));
        assert!(matches!(
            CoverageDecisionDag::rebuild_rooted_from_views(
                view.terminal_payloads(),
                view.nodes(),
                view.roots(),
                view.boolean_false(),
                view.boolean_true(),
                0,
                view.retained_stats(),
                limits,
            ),
            Err(CoverageDecisionDagError::AtomOutOfRange { .. })
        ));
        let mut wrong_stats = view.retained_stats();
        wrong_stats.child_references += 1;
        assert!(matches!(
            CoverageDecisionDag::rebuild_rooted_from_views(
                view.terminal_payloads(),
                view.nodes(),
                view.roots(),
                view.boolean_false(),
                view.boolean_true(),
                view.atom_count(),
                wrong_stats,
                limits,
            ),
            Err(CoverageDecisionDagError::RetainedStatsMismatch { .. })
        ));

        let mut nodes_with_garbage = view.nodes().to_vec();
        nodes_with_garbage.push(CoverageDecisionPersistedNode::new(
            CoverageDecisionAtomId::new(0),
            view.boolean_false(),
            view.boolean_true(),
        ));
        assert!(matches!(
            CoverageDecisionDag::rebuild_rooted_from_views(
                view.terminal_payloads(),
                &nodes_with_garbage,
                view.roots(),
                view.boolean_false(),
                view.boolean_true(),
                view.atom_count(),
                view.retained_stats(),
                limits,
            ),
            Err(CoverageDecisionDagError::UnreachableNode { node })
                if node == CoverageDecisionNodeId::new(1)
        ));
    }

    #[test]
    fn export_representation_limits_refuse_before_reachability_allocation() {
        let mut node_limits = CoverageDecisionDagLimits::default();
        node_limits.max_exported_nodes = 0;
        node_limits.max_reachability_entries = 0;
        let (mut node_dag, node_boolean) = manager_with_atoms(1, node_limits);
        let node_root = node_dag
            .boolean_variable(CoverageDecisionAtomId::new(0), node_boolean)
            .unwrap();
        let before = node_dag.stats();
        assert!(matches!(
            node_dag.export_rooted(&[node_root], node_boolean),
            Err(CoverageDecisionDagError::ResourceLimit {
                resource: "exported nodes",
                requested: 1,
                limit: 0,
            })
        ));
        assert_eq!(node_dag.retained_stats(), before.retained);
        assert_eq!(node_dag.stats().work, before.work);

        let mut terminal_limits = CoverageDecisionDagLimits::default();
        terminal_limits.max_exported_terminals = 0;
        terminal_limits.max_reachability_entries = 0;
        let (mut terminal_dag, terminal_boolean) = manager_with_atoms(0, terminal_limits);
        let before = terminal_dag.stats();
        assert!(matches!(
            terminal_dag.export_rooted(&[terminal_boolean.when_false()], terminal_boolean),
            Err(CoverageDecisionDagError::ResourceLimit {
                resource: "exported terminals",
                requested: 1,
                limit: 0,
            })
        ));
        assert_eq!(terminal_dag.retained_stats(), before.retained);
        assert_eq!(terminal_dag.stats().work, before.work);

        let mut remap_limits = CoverageDecisionDagLimits::default();
        remap_limits.max_export_remap_entries = 0;
        remap_limits.max_reachability_entries = 0;
        let (mut remap_dag, remap_boolean) = manager_with_atoms(0, remap_limits);
        let before = remap_dag.stats();
        assert!(matches!(
            remap_dag.export_rooted(&[remap_boolean.when_false()], remap_boolean),
            Err(CoverageDecisionDagError::ResourceLimit {
                resource: "export remap entries",
                requested: 1,
                limit: 0,
            })
        ));
        assert_eq!(remap_dag.retained_stats(), before.retained);
        assert_eq!(remap_dag.stats().work, before.work);
    }

    #[test]
    fn rooted_rebuild_representation_limits_are_exact_and_one_below_is_rejected() {
        let limits = CoverageDecisionDagLimits::default();
        let (mut dag, boolean) = manager_with_atoms(1, limits);
        let x = dag
            .boolean_variable(CoverageDecisionAtomId::new(0), boolean)
            .unwrap();
        let selected = intern(&mut dag, 2);
        let root = dag
            .if_then_else(x, boolean, selected, boolean.when_false())
            .unwrap();
        let view = dag.export_rooted(&[root], boolean).unwrap();

        let mut exact = limits;
        exact.max_atoms = view.atom_count();
        exact.max_persisted_roots = view.roots().len();
        exact.max_exported_roots = view.roots().len();
        exact.max_exported_terminals = view.terminal_payloads().len();
        exact.max_exported_nodes = view.nodes().len();
        exact.max_exported_edges = view.nodes().len() * 2;
        exact.max_exported_atom_ordinals = view.nodes().len();
        exact.max_export_remap_entries = view.terminal_payloads().len() + view.nodes().len();
        assert!(CoverageDecisionDag::rebuild_rooted(&view, exact).is_ok());

        let mut cases = Vec::new();
        let mut below = exact;
        below.max_persisted_roots -= 1;
        cases.push((below, "persisted roots"));
        below = exact;
        below.max_exported_roots -= 1;
        cases.push((below, "exported roots"));
        below = exact;
        below.max_exported_terminals -= 1;
        cases.push((below, "exported terminals"));
        below = exact;
        below.max_exported_nodes -= 1;
        cases.push((below, "exported nodes"));
        below = exact;
        below.max_exported_edges -= 1;
        cases.push((below, "exported edges"));
        below = exact;
        below.max_exported_atom_ordinals -= 1;
        cases.push((below, "exported atom ordinals"));
        below = exact;
        below.max_export_remap_entries -= 1;
        cases.push((below, "export remap entries"));

        for (limits, resource) in cases {
            assert!(matches!(
                CoverageDecisionDag::rebuild_rooted(&view, limits),
                Err(CoverageDecisionDagError::ResourceLimit {
                    resource: actual,
                    ..
                }) if actual == resource
            ));
        }
    }

    #[test]
    fn rebuild_bulk_preflights_retained_census_before_manager_allocation() {
        let boolean_false =
            CoverageDecisionPersistedRef::Terminal(CoverageDecisionTerminalId::new(0));
        let boolean_true =
            CoverageDecisionPersistedRef::Terminal(CoverageDecisionTerminalId::new(1));

        let single = [Arc::new(CountingTerminal {
            label: 0,
            units: 7,
            references: 1,
        })];
        let mut zero_terminals = CoverageDecisionDagLimits::default();
        zero_terminals.max_terminals = 0;
        COUNTING_CENSUS_CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
        assert!(matches!(
            CoverageDecisionDag::<CountingTerminal>::rebuild_rooted_from_views(
                &single,
                &[],
                &[boolean_false],
                boolean_false,
                boolean_false,
                0,
                CoverageDecisionDagRetainedStats::default(),
                zero_terminals,
            ),
            Err(CoverageDecisionDagError::ResourceLimit {
                resource: "terminals",
                requested: 1,
                limit: 0,
            })
        ));
        assert_eq!(
            COUNTING_CENSUS_CALLS.load(std::sync::atomic::Ordering::SeqCst),
            0
        );

        let payloads = [
            Arc::new(CountingTerminal {
                label: 0,
                units: 7,
                references: 1,
            }),
            Arc::new(CountingTerminal {
                label: 1,
                units: 11,
                references: 2,
            }),
        ];
        let retained = CoverageDecisionDagRetainedStats {
            terminals: 2,
            terminal_payload_units: 18,
            terminal_payload_references: 3,
            terminal_payload_handles: 4,
            nodes: 0,
            child_references: 0,
        };
        let mut exact = CoverageDecisionDagLimits::default();
        exact.max_terminals = 2;
        exact.max_terminal_index_entries = 2;
        exact.max_retained_terminal_payload_units = 18;
        exact.max_retained_terminal_payload_references = 3;
        exact.max_retained_terminal_payload_handles = 4;
        exact.max_terminal_index_lookups = 2;
        exact.max_reachability_entries = 2;
        exact.max_work_stack_pushes = 3;
        exact.max_operation_steps = 3;
        COUNTING_CENSUS_CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
        let rebuilt = match CoverageDecisionDag::<CountingTerminal>::rebuild_rooted_from_views(
            &payloads,
            &[],
            &[boolean_false],
            boolean_false,
            boolean_true,
            0,
            retained,
            exact,
        ) {
            Ok(rebuilt) => rebuilt,
            Err(error) => panic!("exact retained replay preflight failed: {error}"),
        };
        assert_eq!(
            COUNTING_CENSUS_CALLS.load(std::sync::atomic::Ordering::SeqCst),
            4
        );
        assert_eq!(rebuilt.dag().retained_stats(), retained);
        assert_eq!(rebuilt.dag().stats().work.rooted_exports, 0);
        assert_eq!(rebuilt.dag().stats().work.exported_roots, 0);
        assert_eq!(rebuilt.dag().stats().work.exported_terminals, 0);
        assert_eq!(rebuilt.dag().stats().work.exported_nodes, 0);
        assert_eq!(rebuilt.dag().stats().work.export_remap_insertions, 0);

        let mut one_below_units = exact;
        one_below_units.max_retained_terminal_payload_units = 17;
        COUNTING_CENSUS_CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
        assert!(matches!(
            CoverageDecisionDag::<CountingTerminal>::rebuild_rooted_from_views(
                &payloads,
                &[],
                &[boolean_false],
                boolean_false,
                boolean_true,
                0,
                retained,
                one_below_units,
            ),
            Err(CoverageDecisionDagError::ResourceLimit {
                resource: "retained terminal payload units",
                requested: 18,
                limit: 17,
            })
        ));
        assert_eq!(
            COUNTING_CENSUS_CALLS.load(std::sync::atomic::Ordering::SeqCst),
            2
        );

        let mut one_below_references = exact;
        one_below_references.max_retained_terminal_payload_references = 2;
        COUNTING_CENSUS_CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
        assert!(matches!(
            CoverageDecisionDag::<CountingTerminal>::rebuild_rooted_from_views(
                &payloads,
                &[],
                &[boolean_false],
                boolean_false,
                boolean_true,
                0,
                retained,
                one_below_references,
            ),
            Err(CoverageDecisionDagError::ResourceLimit {
                resource: "retained terminal payload references",
                requested: 3,
                limit: 2,
            })
        ));
        assert_eq!(
            COUNTING_CENSUS_CALLS.load(std::sync::atomic::Ordering::SeqCst),
            2
        );

        let mut one_below_handles = exact;
        one_below_handles.max_retained_terminal_payload_handles = 3;
        COUNTING_CENSUS_CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
        assert!(matches!(
            CoverageDecisionDag::<CountingTerminal>::rebuild_rooted_from_views(
                &payloads,
                &[],
                &[boolean_false],
                boolean_false,
                boolean_true,
                0,
                retained,
                one_below_handles,
            ),
            Err(CoverageDecisionDagError::ResourceLimit {
                resource: "retained terminal payload handles",
                requested: 4,
                limit: 3,
            })
        ));
        assert_eq!(
            COUNTING_CENSUS_CALLS.load(std::sync::atomic::Ordering::SeqCst),
            0
        );

        let overflowing = [
            Arc::new(CountingTerminal {
                label: 0,
                units: usize::MAX,
                references: 0,
            }),
            Arc::new(CountingTerminal {
                label: 1,
                units: 1,
                references: 0,
            }),
        ];
        COUNTING_CENSUS_CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
        assert!(matches!(
            CoverageDecisionDag::<CountingTerminal>::rebuild_rooted_from_views(
                &overflowing,
                &[],
                &[boolean_false],
                boolean_false,
                boolean_true,
                0,
                retained,
                CoverageDecisionDagLimits::default(),
            ),
            Err(CoverageDecisionDagError::ResourceCountOverflow {
                resource: "retained terminal payload units",
            })
        ));
        assert_eq!(
            COUNTING_CENSUS_CALLS.load(std::sync::atomic::Ordering::SeqCst),
            2
        );
    }

    #[test]
    fn malformed_views_are_rejected() {
        let limits = CoverageDecisionDagLimits::default();
        let false_payload = payload(0);
        let true_payload = payload(1);
        let terminals = [false_payload.clone(), true_payload.clone()];
        let boolean_false =
            CoverageDecisionPersistedRef::Terminal(CoverageDecisionTerminalId::new(0));
        let boolean_true =
            CoverageDecisionPersistedRef::Terminal(CoverageDecisionTerminalId::new(1));

        #[cfg(target_pointer_width = "64")]
        {
            let high_tag = usize::try_from(CoverageDecisionRef::NODE_TAG).unwrap();
            let valid_node = CoverageDecisionPersistedNode::new(
                CoverageDecisionAtomId::new(0),
                boolean_false,
                boolean_true,
            );
            for forged in [
                CoverageDecisionPersistedRef::Node(CoverageDecisionNodeId::new(high_tag)),
                CoverageDecisionPersistedRef::Terminal(CoverageDecisionTerminalId::new(high_tag)),
            ] {
                assert!(matches!(
                    CoverageDecisionDag::rebuild_rooted_from_views(
                        &terminals,
                        &[valid_node],
                        &[forged],
                        boolean_false,
                        boolean_true,
                        1,
                        CoverageDecisionDagRetainedStats::default(),
                        limits,
                    ),
                    Err(CoverageDecisionDagError::PersistedRootOutOfRange {
                        root_ordinal: 0,
                        reference,
                    }) if reference == forged
                ));
            }

            let first = CoverageDecisionPersistedNode::new(
                CoverageDecisionAtomId::new(1),
                boolean_false,
                boolean_true,
            );
            let forged_child =
                CoverageDecisionPersistedRef::Node(CoverageDecisionNodeId::new(high_tag));
            let second = CoverageDecisionPersistedNode::new(
                CoverageDecisionAtomId::new(0),
                forged_child,
                boolean_false,
            );
            assert!(matches!(
                CoverageDecisionDag::rebuild_rooted_from_views(
                    &terminals,
                    &[first, second],
                    &[CoverageDecisionPersistedRef::Node(
                        CoverageDecisionNodeId::new(1),
                    )],
                    boolean_false,
                    boolean_true,
                    2,
                    CoverageDecisionDagRetainedStats::default(),
                    limits,
                ),
                Err(CoverageDecisionDagError::PersistedReferenceOrdinalUnencodable {
                    reference,
                    ..
                }) if reference == forged_child
            ));
        }

        assert!(matches!(
            CoverageDecisionDag::rebuild_rooted_from_views(
                &[false_payload.clone(), false_payload],
                &[],
                &[boolean_false],
                boolean_false,
                boolean_true,
                0,
                CoverageDecisionDagRetainedStats {
                    terminals: 2,
                    ..CoverageDecisionDagRetainedStats::default()
                },
                limits,
            ),
            Err(CoverageDecisionDagError::NonCanonicalTerminalView { .. })
        ));

        let reduced = CoverageDecisionPersistedNode::new(
            CoverageDecisionAtomId::new(0),
            boolean_false,
            boolean_false,
        );
        assert!(matches!(
            CoverageDecisionDag::rebuild_rooted_from_views(
                &terminals,
                &[reduced],
                &[boolean_false],
                boolean_false,
                boolean_true,
                1,
                CoverageDecisionDagRetainedStats::default(),
                limits,
            ),
            Err(CoverageDecisionDagError::NonCanonicalNodeView { .. })
        ));

        let forward = CoverageDecisionPersistedNode::new(
            CoverageDecisionAtomId::new(0),
            boolean_false,
            CoverageDecisionPersistedRef::Node(CoverageDecisionNodeId::new(0)),
        );
        assert!(matches!(
            CoverageDecisionDag::rebuild_rooted_from_views(
                &terminals,
                &[forward],
                &[boolean_false],
                boolean_false,
                boolean_true,
                1,
                CoverageDecisionDagRetainedStats::default(),
                limits,
            ),
            Err(CoverageDecisionDagError::NodeReferenceOutOfRange { .. })
        ));

        let first_node = CoverageDecisionPersistedNode::new(
            CoverageDecisionAtomId::new(0),
            boolean_false,
            boolean_true,
        );
        assert!(matches!(
            CoverageDecisionDag::rebuild_rooted_from_views(
                &terminals,
                &[first_node, first_node],
                &[CoverageDecisionPersistedRef::Node(
                    CoverageDecisionNodeId::new(1)
                )],
                boolean_false,
                boolean_true,
                2,
                CoverageDecisionDagRetainedStats::default(),
                limits,
            ),
            Err(CoverageDecisionDagError::NonCanonicalNodeView { ordinal: 1, .. })
        ));

        let bad_order = CoverageDecisionPersistedNode::new(
            CoverageDecisionAtomId::new(1),
            CoverageDecisionPersistedRef::Node(CoverageDecisionNodeId::new(0)),
            boolean_false,
        );
        assert!(matches!(
            CoverageDecisionDag::rebuild_rooted_from_views(
                &terminals,
                &[first_node, bad_order],
                &[CoverageDecisionPersistedRef::Node(
                    CoverageDecisionNodeId::new(1)
                )],
                boolean_false,
                boolean_true,
                2,
                CoverageDecisionDagRetainedStats::default(),
                limits,
            ),
            Err(CoverageDecisionDagError::VariableOrderViolation { .. })
        ));

        let extra = payload(2);
        assert!(matches!(
            CoverageDecisionDag::rebuild_rooted_from_views(
                &[terminals[0].clone(), terminals[1].clone(), extra],
                &[],
                &[boolean_false],
                boolean_false,
                boolean_true,
                0,
                CoverageDecisionDagRetainedStats::default(),
                limits,
            ),
            Err(CoverageDecisionDagError::UnreachableTerminal { terminal })
                if terminal == CoverageDecisionTerminalId::new(2)
        ));
    }

    #[test]
    fn cache_and_boolean_validation_limits_are_exact_and_cumulative() {
        fn build(
            mut limits: CoverageDecisionDagLimits,
        ) -> (
            Result<CoverageDecisionRef, CoverageDecisionDagError>,
            CoverageDecisionDag<Terminal>,
        ) {
            limits.max_nodes = 32;
            limits.max_unique_table_entries = 32;
            limits.max_retained_child_references = 64;
            let (mut dag, boolean) = manager(limits);
            let x = dag
                .boolean_variable(CoverageDecisionAtomId::new(0), boolean)
                .unwrap();
            let y = dag
                .boolean_variable(CoverageDecisionAtomId::new(1), boolean)
                .unwrap();
            let result = dag.apply_binary_truth_table(
                x,
                y,
                boolean,
                CoverageDecisionBinaryTruthTable::new(
                    boolean.when_false(),
                    boolean.when_true(),
                    boolean.when_true(),
                    boolean.when_false(),
                ),
            );
            (result, dag)
        }

        let (result, measured) = build(CoverageDecisionDagLimits::default());
        result.unwrap();
        let cache_entries = measured.stats().work.apply_cache_insertions;
        assert!(cache_entries > 0);
        let validation_entries = measured.stats().work.boolean_validation_insertions;
        assert_eq!(validation_entries, 2);

        let mut exact = CoverageDecisionDagLimits::default();
        exact.max_apply_cache_entries = cache_entries;
        exact.max_total_operation_cache_entries = cache_entries;
        exact.max_boolean_validation_entries = validation_entries;
        assert!(build(exact).0.is_ok());

        let mut one_below_cache = exact;
        one_below_cache.max_apply_cache_entries = cache_entries - 1;
        assert!(matches!(
            build(one_below_cache).0,
            Err(CoverageDecisionDagError::ResourceLimit {
                resource: "apply cache entries",
                ..
            })
        ));

        let mut one_below_validation = exact;
        one_below_validation.max_boolean_validation_entries = validation_entries - 1;
        assert!(matches!(
            build(one_below_validation).0,
            Err(CoverageDecisionDagError::ResourceLimit {
                resource: "Boolean validation entries",
                requested: 2,
                limit: 1,
            })
        ));
    }

    #[test]
    fn terminal_retention_and_lookup_limits_are_exact_and_transactional() {
        fn build(
            limits: CoverageDecisionDagLimits,
        ) -> (
            Result<(), CoverageDecisionDagError>,
            CoverageDecisionDag<Terminal>,
            CoverageDecisionDagStats,
        ) {
            let mut dag = CoverageDecisionDag::new(0, limits).unwrap();
            let before = dag.stats();
            let result = dag.with_operation(|operation| {
                operation.intern_terminal(payload(0))?;
                operation.intern_terminal(payload(1))?;
                Ok(())
            });
            (result, dag, before)
        }

        let mut exact = CoverageDecisionDagLimits::default();
        exact.max_terminals = 2;
        exact.max_terminal_index_entries = 2;
        exact.max_terminal_index_lookups = 2;
        let (result, dag, before) = build(exact);
        result.unwrap();
        assert_eq!(dag.retained_stats().terminals, 2);
        assert_eq!(dag.stats().work.terminal_index_lookups, 2);

        let mut cases = Vec::new();
        let mut below = exact;
        below.max_terminals = 1;
        cases.push((below, "terminals"));
        below = exact;
        below.max_terminal_index_entries = 1;
        cases.push((below, "terminal index entries"));
        below = exact;
        below.max_terminal_index_lookups = 1;
        cases.push((below, "terminal index lookups"));

        for (limits, resource) in cases {
            let (result, dag, checkpoint) = build(limits);
            assert!(matches!(
                result,
                Err(CoverageDecisionDagError::ResourceLimit {
                    resource: actual,
                    requested: 2,
                    limit: 1,
                }) if actual == resource
            ));
            assert_eq!(dag.retained_stats(), checkpoint.retained);
            assert_eq!(dag.stats().work, checkpoint.work);
            assert_eq!(checkpoint.retained, before.retained);
        }
    }

    #[test]
    fn unique_table_child_reference_and_lookup_limits_are_exact_and_transactional() {
        fn build(
            limits: CoverageDecisionDagLimits,
        ) -> (
            Result<(), CoverageDecisionDagError>,
            CoverageDecisionDag<Terminal>,
            CoverageDecisionDagStats,
        ) {
            let (mut dag, boolean) = manager_with_atoms(2, limits);
            let before = dag.stats();
            let result = dag.with_operation(|operation| {
                operation.boolean_variable(CoverageDecisionAtomId::new(0), boolean)?;
                operation.boolean_variable(CoverageDecisionAtomId::new(1), boolean)?;
                Ok(())
            });
            (result, dag, before)
        }

        let mut exact = CoverageDecisionDagLimits::default();
        exact.max_nodes = 2;
        exact.max_unique_table_entries = 2;
        exact.max_retained_child_references = 4;
        exact.max_unique_table_lookups = 2;
        let (result, dag, before) = build(exact);
        result.unwrap();
        assert_eq!(dag.retained_stats().nodes - before.retained.nodes, 2);
        assert_eq!(
            dag.stats().work.unique_table_lookups - before.work.unique_table_lookups,
            2
        );

        let mut cases = Vec::new();
        let mut below = exact;
        below.max_unique_table_entries = 1;
        cases.push((below, "unique table entries", 2, 1));
        below = exact;
        below.max_retained_child_references = 3;
        cases.push((below, "retained child references", 4, 3));
        below = exact;
        below.max_unique_table_lookups = 1;
        cases.push((below, "unique table lookups", 2, 1));

        for (limits, resource, requested, limit) in cases {
            let (result, dag, checkpoint) = build(limits);
            assert!(matches!(
                result,
                Err(CoverageDecisionDagError::ResourceLimit {
                    resource: actual,
                    requested: actual_requested,
                    limit: actual_limit,
                }) if actual == resource
                    && actual_requested == requested
                    && actual_limit == limit
            ));
            assert_eq!(dag.retained_stats(), checkpoint.retained);
            assert_eq!(dag.stats().work, checkpoint.work);
        }
    }

    #[test]
    fn live_ite_and_binary_apply_call_limits_are_exact_and_transactional() {
        fn ite(
            limits: CoverageDecisionDagLimits,
        ) -> (
            Result<CoverageDecisionRef, CoverageDecisionDagError>,
            CoverageDecisionDag<Terminal>,
            CoverageDecisionDagStats,
        ) {
            let (mut dag, boolean) = manager_with_atoms(1, limits);
            let condition = dag
                .boolean_variable(CoverageDecisionAtomId::new(0), boolean)
                .unwrap();
            let selected = intern(&mut dag, 2);
            let fallback = intern(&mut dag, 3);
            let before = dag.stats();
            let result = dag.with_operation(|operation| {
                operation.if_then_else(condition, boolean, selected, fallback)
            });
            (result, dag, before)
        }

        fn apply(
            limits: CoverageDecisionDagLimits,
        ) -> (
            Result<CoverageDecisionRef, CoverageDecisionDagError>,
            CoverageDecisionDag<Terminal>,
            CoverageDecisionDagStats,
        ) {
            let (mut dag, boolean) = manager_with_atoms(2, limits);
            let left = dag
                .boolean_variable(CoverageDecisionAtomId::new(0), boolean)
                .unwrap();
            let right = dag
                .boolean_variable(CoverageDecisionAtomId::new(1), boolean)
                .unwrap();
            let before = dag.stats();
            let result =
                dag.with_operation(|operation| operation.boolean_and(left, right, boolean));
            (result, dag, before)
        }

        let mut exact_ite = CoverageDecisionDagLimits::default();
        exact_ite.max_ite_calls = 1;
        let (result, dag, before) = ite(exact_ite);
        result.unwrap();
        assert_eq!(dag.stats().work.ite_calls - before.work.ite_calls, 1);

        let mut below_ite = exact_ite;
        below_ite.max_ite_calls = 0;
        let (result, dag, checkpoint) = ite(below_ite);
        assert!(matches!(
            result,
            Err(CoverageDecisionDagError::ResourceLimit {
                resource: "ITE calls",
                requested: 1,
                limit: 0,
            })
        ));
        assert_eq!(dag.retained_stats(), checkpoint.retained);
        assert_eq!(dag.stats().work, checkpoint.work);

        let mut exact_apply = CoverageDecisionDagLimits::default();
        exact_apply.max_binary_apply_calls = 1;
        let (result, dag, before) = apply(exact_apply);
        result.unwrap();
        assert_eq!(
            dag.stats().work.binary_apply_calls - before.work.binary_apply_calls,
            1
        );

        let mut below_apply = exact_apply;
        below_apply.max_binary_apply_calls = 0;
        let (result, dag, checkpoint) = apply(below_apply);
        assert!(matches!(
            result,
            Err(CoverageDecisionDagError::ResourceLimit {
                resource: "binary apply calls",
                requested: 1,
                limit: 0,
            })
        ));
        assert_eq!(dag.retained_stats(), checkpoint.retained);
        assert_eq!(dag.stats().work, checkpoint.work);
    }

    #[test]
    fn scoped_build_and_export_budgets_are_cumulative_exact_and_rollback_together() {
        fn build(
            limits: CoverageDecisionDagLimits,
        ) -> (
            Result<CoverageDecisionRef, CoverageDecisionDagError>,
            CoverageDecisionDag<Terminal>,
            CoverageDecisionDagWorkStats,
        ) {
            let (mut dag, boolean) = manager_with_atoms(2, limits);
            let x = dag
                .boolean_variable(CoverageDecisionAtomId::new(0), boolean)
                .unwrap();
            let y = dag
                .boolean_variable(CoverageDecisionAtomId::new(1), boolean)
                .unwrap();
            let selected = intern(&mut dag, 2);
            let fallback = intern(&mut dag, 3);
            let before = dag.stats().work;
            let result = dag.with_operation(|operation| {
                assert_eq!(operation.intern_terminal(payload(2))?, selected);
                assert_eq!(
                    operation.branch(
                        CoverageDecisionAtomId::new(0),
                        boolean.when_false(),
                        boolean.when_true(),
                    )?,
                    x
                );
                assert_eq!(
                    operation.branch(
                        CoverageDecisionAtomId::new(0),
                        boolean.when_false(),
                        boolean.when_false(),
                    )?,
                    boolean.when_false()
                );
                let conjunction = operation.boolean_and(x, y, boolean)?;
                let disjunction = operation.boolean_or(x, y, boolean)?;
                let selected_root =
                    operation.if_then_else(conjunction, boolean, selected, disjunction)?;
                let prioritized = operation.compose_candidate_priority(
                    &[(x, selected_root), (y, fallback)],
                    boolean,
                    disjunction,
                )?;
                let view = operation.export_rooted(&[prioritized], boolean)?;
                assert_eq!(view.roots().len(), 1);
                Ok(prioritized)
            });
            (result, dag, before)
        }

        let (measured_result, measured, before) = build(CoverageDecisionDagLimits::default());
        measured_result.unwrap();
        let after = measured.stats().work;
        let delta = |after: usize, before: usize| after.checked_sub(before).unwrap();
        let apply_entries = delta(after.apply_cache_insertions, before.apply_cache_insertions);
        let ite_entries = delta(after.ite_cache_insertions, before.ite_cache_insertions);
        let validation_entries = delta(
            after.boolean_validation_insertions,
            before.boolean_validation_insertions,
        );
        let pushes = delta(after.work_stack_pushes, before.work_stack_pushes);
        let steps = delta(after.operation_steps, before.operation_steps);
        let comparisons = delta(
            after.unique_table_comparisons,
            before.unique_table_comparisons,
        );
        let priority = delta(after.priority_candidates, before.priority_candidates);
        let reachability = delta(
            after.reachability_insertions,
            before.reachability_insertions,
        );
        let exported_roots = delta(after.exported_roots, before.exported_roots);
        let exported_terminals = delta(after.exported_terminals, before.exported_terminals);
        let exported_nodes = delta(after.exported_nodes, before.exported_nodes);
        let exported_edges = delta(after.exported_edges, before.exported_edges);
        let exported_atoms = delta(after.exported_atom_ordinals, before.exported_atom_ordinals);
        let remaps = delta(
            after.export_remap_insertions,
            before.export_remap_insertions,
        );
        assert!(apply_entries > 0);
        assert!(ite_entries > 0);
        assert!(validation_entries > 0);
        assert!(pushes > after.work_stack_peak);
        assert!(steps > 0);
        assert!(comparisons > 0);
        assert_eq!(priority, 2);
        assert!(reachability > 0);
        assert_eq!(exported_roots, 1);
        assert!(exported_terminals >= 2);
        assert!(exported_nodes > 0);
        assert_eq!(exported_edges, exported_nodes * 2);
        assert_eq!(exported_atoms, exported_nodes);
        assert!(remaps >= exported_nodes + 2);
        assert_eq!(delta(after.terminal_reuses, before.terminal_reuses), 1);
        assert!(delta(after.node_reduction_hits, before.node_reduction_hits) >= 1);
        assert!(delta(after.unique_table_reuses, before.unique_table_reuses) >= 1);

        let mut exact = CoverageDecisionDagLimits::default();
        exact.max_apply_cache_entries = apply_entries;
        exact.max_ite_cache_entries = ite_entries;
        exact.max_total_operation_cache_entries = apply_entries + ite_entries;
        exact.max_boolean_validation_entries = validation_entries;
        exact.max_work_stack_entries = after.work_stack_peak;
        exact.max_work_stack_pushes = pushes;
        exact.max_operation_steps = steps;
        exact.max_unique_table_comparisons = comparisons;
        exact.max_priority_candidates = priority;
        exact.max_reachability_entries = reachability;
        exact.max_persisted_roots = exported_roots;
        exact.max_exported_roots = exported_roots;
        exact.max_exported_terminals = exported_terminals;
        exact.max_exported_nodes = exported_nodes;
        exact.max_exported_edges = exported_edges;
        exact.max_exported_atom_ordinals = exported_atoms;
        exact.max_export_remap_entries = remaps;
        assert!(build(exact).0.is_ok());

        let mut cases = Vec::new();
        let mut below = exact;
        below.max_apply_cache_entries = apply_entries - 1;
        cases.push((below, "apply cache entries"));
        below = exact;
        below.max_ite_cache_entries = ite_entries - 1;
        cases.push((below, "ITE cache entries"));
        below = exact;
        below.max_total_operation_cache_entries = apply_entries + ite_entries - 1;
        cases.push((below, "total operation cache entries"));
        below = exact;
        below.max_boolean_validation_entries = validation_entries - 1;
        cases.push((below, "Boolean validation entries"));
        below = exact;
        below.max_work_stack_entries = after.work_stack_peak - 1;
        cases.push((below, "work stack entries"));
        below = exact;
        below.max_work_stack_pushes = pushes - 1;
        cases.push((below, "work stack pushes"));
        below = exact;
        below.max_operation_steps = steps - 1;
        cases.push((below, "operation steps"));
        below = exact;
        below.max_unique_table_comparisons = comparisons - 1;
        cases.push((below, "unique table comparisons"));
        below = exact;
        below.max_priority_candidates = priority - 1;
        cases.push((below, "priority candidates"));
        below = exact;
        below.max_reachability_entries = reachability - 1;
        cases.push((below, "reachability entries"));
        below = exact;
        below.max_persisted_roots = exported_roots - 1;
        cases.push((below, "persisted roots"));
        below = exact;
        below.max_exported_roots = exported_roots - 1;
        cases.push((below, "exported roots"));
        below = exact;
        below.max_exported_terminals = exported_terminals - 1;
        cases.push((below, "exported terminals"));
        below = exact;
        below.max_exported_nodes = exported_nodes - 1;
        cases.push((below, "exported nodes"));
        below = exact;
        below.max_exported_edges = exported_edges - 1;
        cases.push((below, "exported edges"));
        below = exact;
        below.max_exported_atom_ordinals = exported_atoms - 1;
        cases.push((below, "exported atom ordinals"));
        below = exact;
        below.max_export_remap_entries = remaps - 1;
        cases.push((below, "export remap entries"));

        for (limits, resource) in cases {
            let (result, dag, checkpoint_work) = build(limits);
            assert!(matches!(
                result,
                Err(CoverageDecisionDagError::ResourceLimit {
                    resource: actual,
                    ..
                }) if actual == resource
            ));
            assert_eq!(dag.stats().work, checkpoint_work);
            assert_eq!(dag.retained_stats().nodes, 2);
        }
    }

    #[test]
    fn deep_iterative_ite_and_apply_do_not_use_call_stack_depth() {
        const DEPTH: usize = 1_024;
        let mut limits = CoverageDecisionDagLimits::default();
        limits.max_nodes = 8_192;
        limits.max_unique_table_entries = 8_192;
        limits.max_retained_child_references = 16_384;
        let (mut dag, boolean) = manager(limits);
        let mut conjunction = boolean.when_true();
        let mut disjunction = boolean.when_false();
        for ordinal in (0..DEPTH).rev() {
            conjunction = dag
                .branch(
                    CoverageDecisionAtomId::new(ordinal),
                    boolean.when_false(),
                    conjunction,
                )
                .unwrap();
            disjunction = dag
                .branch(
                    CoverageDecisionAtomId::new(ordinal),
                    disjunction,
                    boolean.when_true(),
                )
                .unwrap();
        }
        let first = intern(&mut dag, 2);
        let fallback = intern(&mut dag, 3);
        let selected = dag
            .if_then_else(conjunction, boolean, first, fallback)
            .unwrap();
        let combined = dag.boolean_and(conjunction, disjunction, boolean).unwrap();
        assert_eq!(label(&dag, selected, &vec![true; DEPTH]), 2);
        assert_eq!(label(&dag, combined, &vec![true; DEPTH]), 1);
        let mut one_false = vec![true; DEPTH];
        one_false[DEPTH / 2] = false;
        assert_eq!(label(&dag, selected, &one_false), 3);
        assert_eq!(label(&dag, combined, &one_false), 0);

        let view = dag.export_rooted(&[selected, combined], boolean).unwrap();
        let rebuilt = CoverageDecisionDag::rebuild_rooted(&view, limits).unwrap();
        assert_eq!(
            label(rebuilt.dag(), rebuilt.roots()[0], &vec![true; DEPTH]),
            2
        );
        assert_eq!(label(rebuilt.dag(), rebuilt.roots()[1], &one_false), 0);
    }

    #[test]
    fn malformed_boolean_root_is_rejected_transactionally() {
        let (mut dag, boolean) = manager(CoverageDecisionDagLimits::default());
        let foreign = intern(&mut dag, 8);
        let condition = dag
            .branch(
                CoverageDecisionAtomId::new(0),
                boolean.when_false(),
                foreign,
            )
            .unwrap();
        let before = dag.nodes().to_vec();
        assert!(matches!(
            dag.if_then_else(
                condition,
                boolean,
                boolean.when_true(),
                boolean.when_false(),
            ),
            Err(CoverageDecisionDagError::NonBooleanConditionTerminal { .. })
        ));
        assert_eq!(dag.nodes(), before);
    }
}
