use std::collections::{BTreeMap, BTreeSet};

use crate::algebra::IndexedCoefficientContext;
use crate::family::IntegralKey;
use crate::foundry::cell::RuleCell;
use crate::sector::{ComplexityKey, OrderingPolicy, symmetry::Canonicalizer};

use super::{
    ReachabilityDependency, ReachabilityDisposition, ReachabilityError, ReachabilityFrontier,
    ReachabilityLimits, ReachabilityNode, ReachabilityPlanner, ReachabilityRuleApplication,
    ReachabilityStatistics, ReachabilityTerminalProvider,
};

const RULE_CELLS: &str = "rule cells";
const ROOTS: &str = "submitted roots";
const NODES: &str = "discovered nodes";
const PENDING: &str = "pending nodes";
const COORDINATE_CELLS: &str = "retained lattice coordinate cells";
const EDGES: &str = "dependency edges";
const PROBES: &str = "rule-cell probes";
const GUARDS: &str = "guard specializations";
const COEFFICIENTS: &str = "coefficient specializations";

impl<'foundry> ReachabilityPlanner<'foundry> {
    /// Bind one ordered RuleCell collection to its exact indexed context and
    /// optional family symmetry action.
    ///
    /// Every cell is checked for common arity, ordering, context, and family
    /// fingerprint. `Canonicalizer` does not retain a family fingerprint, so
    /// the caller must supply the action authenticated for that same family;
    /// its arity and ordering are checked here.
    pub fn try_new(
        context: &'foundry IndexedCoefficientContext,
        ordering: OrderingPolicy,
        canonicalizer: Option<&'foundry Canonicalizer>,
        cells: impl IntoIterator<Item = &'foundry RuleCell>,
        limits: ReachabilityLimits,
    ) -> Result<Self, ReachabilityError> {
        let arity = context.index_count();
        if let Some(canonicalizer) = canonicalizer {
            if canonicalizer.arity() != arity {
                return Err(ReachabilityError::CanonicalizerArity {
                    expected: arity,
                    actual: canonicalizer.arity(),
                });
            }
            if canonicalizer.ordering() != ordering {
                return Err(ReachabilityError::CanonicalizerOrdering {
                    expected: ordering,
                    actual: canonicalizer.ordering(),
                });
            }
        }

        let mut retained = Vec::new();
        let mut family_fingerprint = None;
        for cell in cells {
            let ordinal = retained.len();
            let requested = checked_add(RULE_CELLS, ordinal, 1)?;
            check_limit(RULE_CELLS, requested, limits.max_rule_cells)?;
            if cell.application_domain().arity() != arity {
                return Err(ReachabilityError::RuleCellArity {
                    cell_ordinal: ordinal,
                    expected: arity,
                    actual: cell.application_domain().arity(),
                });
            }
            if cell.rule().ordering() != ordering {
                return Err(ReachabilityError::RuleCellOrdering {
                    cell_ordinal: ordinal,
                    expected: ordering,
                    actual: cell.rule().ordering(),
                });
            }
            if !cell.indexed_context_matches(context) {
                return Err(ReachabilityError::ForeignRuleCellContext {
                    cell_ordinal: ordinal,
                });
            }
            match family_fingerprint {
                None => family_fingerprint = Some(cell.rule().family_fingerprint()),
                Some(expected) if expected != cell.rule().family_fingerprint() => {
                    return Err(ReachabilityError::ForeignRuleCellFamily {
                        cell_ordinal: ordinal,
                    });
                }
                Some(_) => {}
            }
            retained
                .try_reserve_exact(1)
                .map_err(|_| ReachabilityError::AllocationFailure {
                    resource: RULE_CELLS,
                    requested,
                })?;
            retained.push(cell);
        }
        Ok(Self {
            context,
            ordering,
            canonicalizer,
            cells: retained.into_boxed_slice(),
            limits,
        })
    }

    /// Discover the finite concrete dependency graph reachable from `roots`.
    ///
    /// Every root and nonzero RHS child is canonicalized. Cells are probed in
    /// caller order; domain assignment and every retained guard are
    /// specialized exactly. The first applicable cell owns the key. Each
    /// retained RHS coefficient is then specialized exactly, identically zero
    /// branches are omitted, and raw strict descent is proved before symmetry
    /// routing. None of these concrete checks certifies an unvisited key.
    pub fn discover<'root>(
        &self,
        roots: impl IntoIterator<Item = &'root IntegralKey>,
        terminals: &impl ReachabilityTerminalProvider,
    ) -> Result<ReachabilityFrontier, ReachabilityError> {
        let mut statistics = ReachabilityStatistics::default();
        let mut canonical_roots = BTreeMap::<ComplexityKey, IntegralKey>::new();
        let mut known = BTreeSet::<IntegralKey>::new();
        let mut pending = BTreeSet::<(ComplexityKey, IntegralKey)>::new();

        for root in roots {
            let root_ordinal = statistics.submitted_roots;
            statistics.submitted_roots = checked_add(ROOTS, statistics.submitted_roots, 1)?;
            check_limit(ROOTS, statistics.submitted_roots, self.limits.max_roots)?;
            if root.powers().len() != self.arity() {
                return Err(ReachabilityError::RootArity {
                    root_ordinal,
                    expected: self.arity(),
                    actual: root.powers().len(),
                });
            }
            let canonical = self.canonicalize_root(root)?;
            let complexity = self.ordering.complexity_key(canonical.powers())?;
            if !canonical_roots.contains_key(&complexity) {
                // One retained root key and its persisted complexity key.
                retain_coordinate_cells(
                    &mut statistics,
                    checked_mul(COORDINATE_CELLS, self.arity(), 2)?,
                    self.limits,
                )?;
                canonical_roots.insert(complexity, try_clone_key(&canonical, "canonical roots")?);
            }
            schedule(
                canonical,
                &mut known,
                &mut pending,
                &mut statistics,
                self.ordering,
                self.limits,
            )?;
        }
        statistics.canonical_roots = canonical_roots.len();

        let mut nodes = BTreeMap::<ComplexityKey, ReachabilityNode>::new();
        while let Some((complexity, target)) = pending.pop_last() {
            let disposition = if let Some(terminal) = terminals.classify(&target) {
                statistics.terminal_nodes =
                    checked_add("terminal nodes", statistics.terminal_nodes, 1)?;
                ReachabilityDisposition::Terminal(terminal)
            } else if let Some(selected) = self.select_first_cell(&target, &mut statistics)? {
                // The free-index assignment is retained independently of the
                // target key in the concrete application report.
                retain_coordinate_cells(&mut statistics, self.arity(), self.limits)?;
                let application = self.apply_cell(&target, selected, &mut statistics)?;
                for dependency in application.dependencies() {
                    schedule(
                        try_clone_key(dependency.canonical_child(), "scheduled child keys")?,
                        &mut known,
                        &mut pending,
                        &mut statistics,
                        self.ordering,
                        self.limits,
                    )?;
                }
                statistics.rule_applications =
                    checked_add("rule applications", statistics.rule_applications, 1)?;
                ReachabilityDisposition::Rule(application)
            } else {
                statistics.uncovered_nodes =
                    checked_add("uncovered nodes", statistics.uncovered_nodes, 1)?;
                ReachabilityDisposition::Uncovered
            };
            if nodes
                .insert(complexity, ReachabilityNode::new(target, disposition))
                .is_some()
            {
                return Err(ReachabilityError::Invariant {
                    detail: "two visited keys collided under the injective complexity order",
                });
            }
        }
        if nodes.len() != known.len() {
            return Err(ReachabilityError::Invariant {
                detail: "not every scheduled concrete key was visited exactly once",
            });
        }

        Ok(ReachabilityFrontier::new(
            try_collect_values(canonical_roots, "canonical roots")?,
            try_collect_values(nodes, "reachability nodes")?,
            statistics,
        ))
    }

    fn canonicalize_root(&self, root: &IntegralKey) -> Result<IntegralKey, ReachabilityError> {
        match self.canonicalizer {
            Some(canonicalizer) => try_clone_key(
                canonicalizer.canonicalize(root)?.canonical(),
                "canonical root powers",
            ),
            None => try_clone_key(root, "canonical root powers"),
        }
    }

    fn select_first_cell<'cell>(
        &'cell self,
        target: &IntegralKey,
        statistics: &mut ReachabilityStatistics,
    ) -> Result<Option<SelectedCell<'cell>>, ReachabilityError> {
        for (cell_ordinal, &cell) in self.cells.iter().enumerate() {
            statistics.rule_cell_probes = bounded_increment(
                PROBES,
                statistics.rule_cell_probes,
                self.limits.max_rule_cell_probes,
            )?;
            let Some(assignment) = cell.assignment_for_target(target)? else {
                continue;
            };
            let mut applicable = true;
            for guard in cell.guards() {
                statistics.guard_specializations = bounded_increment(
                    GUARDS,
                    statistics.guard_specializations,
                    self.limits.max_guard_specializations,
                )?;
                let specialized = self.context.specialize_polynomial_sealed(
                    guard.polynomial(),
                    &assignment,
                    self.limits.indexed_algebra,
                )?;
                if specialized.is_zero() {
                    applicable = false;
                    break;
                }
            }
            if applicable {
                return Ok(Some(SelectedCell {
                    ordinal: cell_ordinal,
                    cell,
                    assignment,
                }));
            }
        }
        Ok(None)
    }

    fn apply_cell(
        &self,
        target: &IntegralKey,
        selected: SelectedCell<'_>,
        statistics: &mut ReachabilityStatistics,
    ) -> Result<ReachabilityRuleApplication, ReachabilityError> {
        let mut dependencies = Vec::new();
        for retained in selected.cell.terms() {
            let ordinal = retained.source_rhs_ordinal();
            let term = selected.cell.rule().right_hand_side().get(ordinal).ok_or(
                ReachabilityError::Invariant {
                    detail: "a retained RuleCell term has no source RHS term",
                },
            )?;
            statistics.coefficient_specializations = bounded_increment(
                COEFFICIENTS,
                statistics.coefficient_specializations,
                self.limits.max_coefficient_specializations,
            )?;
            let (coefficient, _) = self.context.specialize_sealed(
                term.coefficient(),
                &selected.assignment,
                self.limits.indexed_algebra,
            )?;
            if coefficient.is_zero() {
                continue;
            }

            let edge_count = bounded_increment(
                EDGES,
                statistics.dependency_edges,
                self.limits.max_dependency_edges,
            )?;
            // One raw rule child and one canonical orbit representative are
            // retained for every exact nonzero dependency.
            retain_coordinate_cells(
                statistics,
                checked_mul(COORDINATE_CELLS, self.arity(), 2)?,
                self.limits,
            )?;

            let mut powers = try_i64_buffer(self.arity(), "raw child powers")?;
            for (position, (&index, &shift)) in selected
                .assignment
                .iter()
                .zip(term.shift().values())
                .enumerate()
            {
                powers.push(
                    index
                        .checked_add(shift)
                        .ok_or(ReachabilityError::IndexOverflow { position })?,
                );
            }
            let raw_child = IntegralKey::try_from_preallocated(powers)?;
            let canonical_child = match self.canonicalizer {
                Some(canonicalizer) => {
                    let descending =
                        canonicalizer.canonicalize_descending_child(target, &raw_child)?;
                    if !descending.verify() {
                        return Err(ReachabilityError::Invariant {
                            detail: "a descending canonicalization witness did not replay",
                        });
                    }
                    try_clone_key(descending.child().canonical(), "canonical child powers")?
                }
                None => {
                    selected
                        .cell
                        .rule()
                        .ordering()
                        .prove_strict_descent(target.powers(), raw_child.powers())?;
                    try_clone_key(&raw_child, "canonical child powers")?
                }
            };
            dependencies.try_reserve_exact(1).map_err(|_| {
                ReachabilityError::AllocationFailure {
                    resource: EDGES,
                    requested: edge_count,
                }
            })?;
            dependencies.push(ReachabilityDependency::new(
                ordinal,
                raw_child,
                canonical_child,
            ));
            statistics.dependency_edges = edge_count;
        }
        Ok(ReachabilityRuleApplication::new(
            selected.ordinal,
            selected.assignment.into_boxed_slice(),
            dependencies.into_boxed_slice(),
        ))
    }
}

struct SelectedCell<'cell> {
    ordinal: usize,
    cell: &'cell RuleCell,
    assignment: Vec<i64>,
}

fn schedule(
    target: IntegralKey,
    known: &mut BTreeSet<IntegralKey>,
    pending: &mut BTreeSet<(ComplexityKey, IntegralKey)>,
    statistics: &mut ReachabilityStatistics,
    ordering: OrderingPolicy,
    limits: ReachabilityLimits,
) -> Result<(), ReachabilityError> {
    if known.contains(&target) {
        return Ok(());
    }
    let requested = checked_add(NODES, known.len(), 1)?;
    check_limit(NODES, requested, limits.max_discovered_nodes)?;
    let pending_requested = checked_add(PENDING, pending.len(), 1)?;
    check_limit(PENDING, pending_requested, limits.max_pending_nodes)?;
    // One known-set key, one pending/eventual report key, and that key's
    // persisted complexity coordinates are retained per unique node.
    retain_coordinate_cells(
        statistics,
        checked_mul(COORDINATE_CELLS, target.powers().len(), 3)?,
        limits,
    )?;
    let complexity = ordering.complexity_key(target.powers())?;
    let known_key = try_clone_key(&target, "known concrete keys")?;
    if !known.insert(known_key) || !pending.insert((complexity, target)) {
        return Err(ReachabilityError::Invariant {
            detail: "one newly scheduled key was already retained",
        });
    }
    statistics.discovered_nodes = requested;
    Ok(())
}

fn bounded_increment(
    resource: &'static str,
    current: usize,
    limit: usize,
) -> Result<usize, ReachabilityError> {
    let requested = checked_add(resource, current, 1)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn retain_coordinate_cells(
    statistics: &mut ReachabilityStatistics,
    additional: usize,
    limits: ReachabilityLimits,
) -> Result<(), ReachabilityError> {
    let requested = checked_add(
        COORDINATE_CELLS,
        statistics.retained_lattice_coordinate_cells,
        additional,
    )?;
    check_limit(
        COORDINATE_CELLS,
        requested,
        limits.max_retained_lattice_coordinate_cells,
    )?;
    statistics.retained_lattice_coordinate_cells = requested;
    Ok(())
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ReachabilityError> {
    if requested > limit {
        Err(ReachabilityError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ReachabilityError> {
    left.checked_add(right)
        .ok_or(ReachabilityError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ReachabilityError> {
    left.checked_mul(right)
        .ok_or(ReachabilityError::ResourceCountOverflow { resource })
}

fn try_clone_key(
    source: &IntegralKey,
    resource: &'static str,
) -> Result<IntegralKey, ReachabilityError> {
    let mut powers = try_i64_buffer(source.powers().len(), resource)?;
    powers.extend_from_slice(source.powers());
    Ok(IntegralKey::try_from_preallocated(powers)?)
}

fn try_i64_buffer(capacity: usize, resource: &'static str) -> Result<Vec<i64>, ReachabilityError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| ReachabilityError::AllocationFailure {
            resource,
            requested: capacity,
        })?;
    Ok(values)
}

fn try_collect_values<K: Ord, V>(
    values: BTreeMap<K, V>,
    resource: &'static str,
) -> Result<Box<[V]>, ReachabilityError> {
    let mut retained = Vec::new();
    retained
        .try_reserve_exact(values.len())
        .map_err(|_| ReachabilityError::AllocationFailure {
            resource,
            requested: values.len(),
        })?;
    retained.extend(values.into_values());
    Ok(retained.into_boxed_slice())
}
