//! Zp DAG over the saved candidate rules, with `CandidateEvaluator` semantics.
//!
//! The DAG *structure* (which rule a node selects, which RHS terms survive,
//! which children coalesce) is decided once with exact semantics: every Zp
//! image that is zero is confirmed by the exact specialization before it is
//! acted on, and a nonzero image is an exact nonzero. The structure is thus
//! independent of the prime and of the sample point. Edge *values* are images
//! of the exact coefficients at one sample; other samples re-evaluate the same
//! edges without repeating selection.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};

use crate::algebra::{Coefficient, ExactAlgebraLimits, IndexedPolynomial};
use crate::family::IntegralKey;
use crate::reduction::ReductionError;
use crate::solver::RuleDispatchPolicy;
use crate::solver::candidate_reduction::{CandidateProgramView, PreparedRule};

use super::LiteRedError;
use super::zp::{ZpIndexPolynomial, ZpSample};

/// Node classification in the application DAG.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    /// Expanded by the rule in flattened program slot `slot`.
    Rule { slot: u32, ordinal: usize },
    /// A declared finite candidate terminal (column id).
    Terminal(u32),
    /// Lies in a proved-zero sector; only possible for an entry target.
    Zero,
    /// No applicable rule (a candidate-program gap, never a master).
    Uncovered,
}

#[derive(Clone, Debug)]
pub(crate) struct Edge {
    pub child: u32,
    /// RHS term positions of the selected rule that coalesce onto `child`.
    pub terms: Vec<u16>,
}

#[derive(Clone, Debug)]
pub(crate) struct Node<const N: usize> {
    pub key: [i64; N],
    pub kind: NodeKind,
    pub edges: Vec<Edge>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VisitState {
    Unvisited,
    InProgress,
    Done,
}

/// Counters of one sample program (exact confirmations and point zeros).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SampleProgramStatistics {
    pub compiled_rules: usize,
    pub compiled_monomials: usize,
    /// Zp zero images confirmed exactly zero.
    pub exact_zero_confirmations: usize,
    /// Zp zero images of exactly nonzero values (generic semantics kept).
    pub point_zeros: usize,
}

struct CompiledTerm {
    numerator: ZpIndexPolynomial,
    denominator: ZpIndexPolynomial,
    /// `None` when the applicability guard equals the coefficient denominator.
    guard: Option<ZpIndexPolynomial>,
}

struct CompiledRule {
    equalities: Vec<ZpIndexPolynomial>,
    exceptions: Vec<Vec<ZpIndexPolynomial>>,
    terms: Vec<CompiledTerm>,
}

/// The candidate program compiled lazily at one modular sample. It owns only
/// compiled images; the exact program is passed in as a view on every call.
pub(crate) struct SampleProgram<const N: usize> {
    sample: ZpSample,
    index_count: usize,
    /// `(sector, position)` of every flattened rule slot.
    slots: Vec<([bool; N], u32)>,
    sector_rules: HashMap<[bool; N], (u32, u32)>,
    compiled: Vec<Option<CompiledRule>>,
    source_conditions: Vec<ZpIndexPolynomial>,
    terminal_index: HashMap<[i64; N], u32>,
    statistics: SampleProgramStatistics,
}

pub(crate) struct Classified<const N: usize> {
    pub kind: NodeKind,
    /// `(child, coalesced term positions, value at this sample)`.
    pub children: Vec<([i64; N], Vec<u16>, u64)>,
}

impl<const N: usize> SampleProgram<N> {
    pub fn new(view: CandidateProgramView<'_, N>, sample: ZpSample) -> Result<Self, LiteRedError> {
        let index_count = view.context().index_count();
        if index_count > N {
            return Err(LiteRedError::InvalidInput(
                "candidate program arity exceeds storage".into(),
            ));
        }
        if sample.base.len() != view.context().base().parameter_names().len() {
            return Err(LiteRedError::InvalidInput(format!(
                "sample has {} base values, family has parameters {:?}",
                sample.base.len(),
                view.context().base().parameter_names()
            )));
        }
        let mut slots = Vec::new();
        let mut sector_rules = HashMap::new();
        for (sector, sector_rule_list) in view.rules() {
            let start = slots.len() as u32;
            slots.extend((0..sector_rule_list.len() as u32).map(|position| (*sector, position)));
            sector_rules.insert(*sector, (start, sector_rule_list.len() as u32));
        }
        let source_conditions = view
            .source_conditions()
            .iter()
            .map(|condition| ZpIndexPolynomial::compile(condition.raw(), &sample, index_count))
            .collect::<Result<Vec<_>, _>>()?;
        let terminal_index = view
            .terminals()
            .iter()
            .enumerate()
            .map(|(position, key)| (pad::<N>(key.powers()), position as u32))
            .collect();
        let compiled = (0..slots.len()).map(|_| None).collect();
        Ok(Self {
            sample,
            index_count,
            slots,
            sector_rules,
            compiled,
            source_conditions,
            terminal_index,
            statistics: SampleProgramStatistics::default(),
        })
    }

    pub fn sample(&self) -> &ZpSample {
        &self.sample
    }
    pub fn statistics(&self) -> SampleProgramStatistics {
        self.statistics
    }

    fn rule<'v>(&self, view: CandidateProgramView<'v, N>, slot: usize) -> &'v PreparedRule<N> {
        let (sector, position) = &self.slots[slot];
        &view.rules()[sector][*position as usize]
    }

    fn residues(&self, key: &[i64; N]) -> Vec<u64> {
        key[..self.index_count]
            .iter()
            .map(|&n| self.sample.field.from_i64(n))
            .collect()
    }

    pub(crate) fn integral_key(&self, key: &[i64; N]) -> Result<IntegralKey, LiteRedError> {
        IntegralKey::try_new(key[..self.index_count].iter().copied())
            .map_err(|error| LiteRedError::InvalidInput(error.to_string()))
    }

    fn exact_polynomial_is_zero(
        &self,
        view: CandidateProgramView<'_, N>,
        polynomial: &IndexedPolynomial,
        key: &[i64; N],
    ) -> Result<bool, LiteRedError> {
        Ok(view
            .context()
            .specialize_polynomial_sealed(
                polynomial,
                &key[..self.index_count],
                view.limits().indexed_algebra,
            )
            .map_err(LiteRedError::from_indexed)?
            .is_zero())
    }

    fn exact_coefficient(
        &self,
        view: CandidateProgramView<'_, N>,
        rule: &PreparedRule<N>,
        term: usize,
        key: &[i64; N],
    ) -> Result<Coefficient, LiteRedError> {
        Ok(view
            .context()
            .specialize_sealed(
                &rule.rhs[term].coefficient,
                &key[..self.index_count],
                view.limits().indexed_algebra,
            )
            .map_err(LiteRedError::from_indexed)?
            .0)
    }

    fn compile(
        &mut self,
        view: CandidateProgramView<'_, N>,
        slot: usize,
    ) -> Result<(), LiteRedError> {
        if self.compiled[slot].is_some() {
            return Ok(());
        }
        let rule = self.rule(view, slot);
        let sample = &self.sample;
        let count = self.index_count;
        let compile = |p: &IndexedPolynomial| ZpIndexPolynomial::compile(p.raw(), sample, count);
        let equalities = rule
            .equalities
            .iter()
            .map(compile)
            .collect::<Result<Vec<_>, _>>()?;
        let exceptions = rule
            .exceptions
            .iter()
            .map(|branch| branch.iter().map(compile).collect::<Result<Vec<_>, _>>())
            .collect::<Result<Vec<_>, _>>()?;
        let mut terms = Vec::with_capacity(rule.rhs.len());
        for term in &rule.rhs {
            let raw = term.coefficient.raw();
            let numerator = ZpIndexPolynomial::compile(&raw.numerator, sample, count)?;
            let denominator = ZpIndexPolynomial::compile(&raw.denominator, sample, count)?;
            let guard = if term.denominator.raw() == &raw.denominator {
                None
            } else {
                Some(compile(&term.denominator)?)
            };
            terms.push(CompiledTerm {
                numerator,
                denominator,
                guard,
            });
        }
        let compiled = CompiledRule {
            equalities,
            exceptions,
            terms,
        };
        self.statistics.compiled_rules += 1;
        self.statistics.compiled_monomials += compiled
            .equalities
            .iter()
            .chain(compiled.exceptions.iter().flatten())
            .map(ZpIndexPolynomial::monomials)
            .sum::<usize>()
            + compiled
                .terms
                .iter()
                .map(|t| {
                    t.numerator.monomials()
                        + t.denominator.monomials()
                        + t.guard.as_ref().map_or(0, ZpIndexPolynomial::monomials)
                })
                .sum::<usize>();
        self.compiled[slot] = Some(compiled);
        Ok(())
    }

    /// The evaluator's target validation: root support and source conditions.
    fn validate(
        &mut self,
        view: CandidateProgramView<'_, N>,
        key: &[i64; N],
        residues: &[u64],
    ) -> Result<(), LiteRedError> {
        let root = view.root_sector();
        if key.iter().zip(root).any(|(&n, active)| n > 0 && !active) {
            return Err(LiteRedError::Candidate(
                crate::solver::CandidateReductionError::OutsideRoot {
                    target: self.integral_key(key)?,
                },
            ));
        }
        for ordinal in 0..self.source_conditions.len() {
            if self.source_conditions[ordinal].evaluate(residues, &self.sample.field) != 0 {
                continue;
            }
            if self.exact_polynomial_is_zero(view, &view.source_conditions()[ordinal], key)? {
                return Err(LiteRedError::Candidate(
                    crate::solver::CandidateReductionError::SourceConditionVanished {
                        target: self.integral_key(key)?,
                        ordinal,
                    },
                ));
            }
            self.statistics.point_zeros += 1;
        }
        Ok(())
    }

    fn is_zero_sector(&self, view: CandidateProgramView<'_, N>, key: &[i64; N]) -> bool {
        view.zero_sectors()
            .contains(&std::array::from_fn(|i| key[i] > 0))
    }

    /// Exact applicability of one rule (fixed coordinates, affine equalities,
    /// AND-branch exclusions and every RHS denominator), decided through Zp
    /// images with exact confirmation of every zero image.
    fn applicable(
        &mut self,
        view: CandidateProgramView<'_, N>,
        slot: usize,
        key: &[i64; N],
        residues: &[u64],
    ) -> Result<bool, LiteRedError> {
        let rule = self.rule(view, slot);
        if rule
            .fixed
            .iter()
            .zip(key)
            .any(|(fixed, &n)| fixed.is_some_and(|v| i64::from(v) != n))
        {
            return Ok(false);
        }
        self.compile(view, slot)?;
        let field = self.sample.field.clone();
        let compiled = self.compiled[slot].as_ref().expect("compiled");
        // Collect zero images first; exact confirmation needs `&mut self`.
        let equality_images: Vec<u64> = compiled
            .equalities
            .iter()
            .map(|p| p.evaluate(residues, &field))
            .collect();
        for (position, image) in equality_images.into_iter().enumerate() {
            if image != 0 {
                return Ok(false);
            }
            if !self.exact_polynomial_is_zero(view, &rule.equalities[position], key)? {
                self.statistics.point_zeros += 1;
                return Ok(false);
            }
            self.statistics.exact_zero_confirmations += 1;
        }
        let compiled = self.compiled[slot].as_ref().expect("compiled");
        let exception_images: Vec<Vec<u64>> = compiled
            .exceptions
            .iter()
            .map(|branch| {
                branch
                    .iter()
                    .map(|p| p.evaluate(residues, &field))
                    .collect()
            })
            .collect();
        for (branch, images) in exception_images.into_iter().enumerate() {
            let mut all_zero = true;
            for (position, image) in images.into_iter().enumerate() {
                if image != 0 {
                    all_zero = false;
                    break;
                }
                if !self.exact_polynomial_is_zero(view, &rule.exceptions[branch][position], key)? {
                    self.statistics.point_zeros += 1;
                    all_zero = false;
                    break;
                }
                self.statistics.exact_zero_confirmations += 1;
            }
            if all_zero {
                return Ok(false);
            }
        }
        let compiled = self.compiled[slot].as_ref().expect("compiled");
        let guard_images: Vec<u64> = compiled
            .terms
            .iter()
            .map(|t| {
                t.guard
                    .as_ref()
                    .unwrap_or(&t.denominator)
                    .evaluate(residues, &field)
            })
            .collect();
        for (term, image) in guard_images.into_iter().enumerate() {
            if image != 0 {
                continue;
            }
            if self.exact_polynomial_is_zero(view, &rule.rhs[term].denominator, key)? {
                self.statistics.exact_zero_confirmations += 1;
                return Ok(false);
            }
            return Err(LiteRedError::UnluckySample {
                phase: "rule denominator",
                detail: format!("rule {} at {:?}", rule.ordinal, &key[..self.index_count]),
            });
        }
        Ok(true)
    }

    /// The evaluator's ordered dispatch: the first applicable `Partition`
    /// rule, then the first applicable whole-piece alternative.
    fn select(
        &mut self,
        view: CandidateProgramView<'_, N>,
        key: &[i64; N],
        residues: &[u64],
    ) -> Result<Option<usize>, LiteRedError> {
        let sector: [bool; N] = std::array::from_fn(|i| key[i] > 0);
        let Some(&(start, len)) = self.sector_rules.get(&sector) else {
            return Ok(None);
        };
        let (start, len) = (start as usize, len as usize);
        let mut baseline = None;
        for slot in start..start + len {
            if self.rule(view, slot).dispatch_policy == RuleDispatchPolicy::Partition
                && self.applicable(view, slot, key, residues)?
            {
                baseline = Some(slot);
                break;
            }
        }
        let Some(mut selected) = baseline else {
            return Ok(None);
        };
        for &position in view.whole_piece_alternatives(&sector) {
            let slot = start + position;
            if self.applicable(view, slot, key, residues)? {
                selected = slot;
                break;
            }
        }
        Ok(Some(selected))
    }

    /// Classify one integral exactly and, for a rule node, return its
    /// coalesced children with values at this sample.
    pub fn classify(
        &mut self,
        view: CandidateProgramView<'_, N>,
        key: &[i64; N],
    ) -> Result<Classified<N>, LiteRedError> {
        let residues = self.residues(key);
        self.validate(view, key, &residues)?;
        if self.is_zero_sector(view, key) {
            return Ok(Classified {
                kind: NodeKind::Zero,
                children: Vec::new(),
            });
        }
        if let Some(&terminal) = self.terminal_index.get(key) {
            return Ok(Classified {
                kind: NodeKind::Terminal(terminal),
                children: Vec::new(),
            });
        }
        let Some(slot) = self.select(view, key, &residues)? else {
            return Ok(Classified {
                kind: NodeKind::Uncovered,
                children: Vec::new(),
            });
        };
        let rule = self.rule(view, slot);
        let field = self.sample.field.clone();
        let images: Vec<(u64, u64)> = {
            let compiled = self.compiled[slot].as_ref().expect("compiled by selection");
            compiled
                .terms
                .iter()
                .map(|t| {
                    (
                        t.numerator.evaluate(&residues, &field),
                        t.denominator.evaluate(&residues, &field),
                    )
                })
                .collect()
        };
        let mut coalesced: BTreeMap<[i64; N], (Vec<u16>, u64)> = BTreeMap::new();
        for (term, (numerator, denominator)) in images.into_iter().enumerate() {
            let Some(inverse) = field.inv(denominator) else {
                return Err(LiteRedError::UnluckySample {
                    phase: "coefficient denominator",
                    detail: format!("rule {} term {term}", rule.ordinal),
                });
            };
            if numerator == 0 {
                if self.exact_coefficient(view, rule, term, key)?.is_zero() {
                    self.statistics.exact_zero_confirmations += 1;
                    continue;
                }
                self.statistics.point_zeros += 1;
            }
            let value = field.mul(numerator, inverse);
            let mut child = [0_i64; N];
            for axis in 0..N {
                child[axis] = key[axis]
                    .checked_add(rule.rhs[term].shift[axis])
                    .ok_or_else(|| {
                        LiteRedError::InvalidInput(format!(
                            "index overflow in rule {} at axis {axis}",
                            rule.ordinal
                        ))
                    })?;
            }
            let child_residues = self.residues(&child);
            self.validate(view, &child, &child_residues)?;
            if self.is_zero_sector(view, &child) {
                continue;
            }
            let order = view
                .ordering()
                .compare(&child[..self.index_count], &key[..self.index_count])
                .map_err(|e| LiteRedError::Candidate(ReductionError::Ordering(e).into()))?;
            if order != Ordering::Less {
                return Err(LiteRedError::Candidate(
                    crate::solver::CandidateReductionError::NonDescending {
                        target: self.integral_key(key)?,
                        child: self.integral_key(&child)?,
                        rule: rule.ordinal,
                    },
                ));
            }
            let entry = coalesced.entry(child).or_insert_with(|| (Vec::new(), 0));
            entry.0.push(term as u16);
            entry.1 = field.add(entry.1, value);
        }
        let mut children = Vec::with_capacity(coalesced.len());
        for (child, (terms, value)) in coalesced {
            if value == 0 && terms.len() > 1 {
                // Confirm a coalesced cancellation exactly before dropping it.
                let base = view.context().base();
                let mut sum = base.zero();
                for &term in &terms {
                    let coefficient = self.exact_coefficient(view, rule, term as usize, key)?;
                    sum = base
                        .try_add(&sum, &coefficient, ExactAlgebraLimits::default())
                        .map_err(|e| LiteRedError::Algebra(e.to_string()))?;
                }
                if sum.is_zero() {
                    self.statistics.exact_zero_confirmations += 1;
                    continue;
                }
                self.statistics.point_zeros += 1;
            }
            children.push((child, terms, value));
        }
        Ok(Classified {
            kind: NodeKind::Rule {
                slot: slot as u32,
                ordinal: rule.ordinal,
            },
            children,
        })
    }

    /// Value of one structural edge at this sample.
    pub fn edge_value(
        &mut self,
        view: CandidateProgramView<'_, N>,
        slot: usize,
        key: &[i64; N],
        terms: &[u16],
    ) -> Result<u64, LiteRedError> {
        self.compile(view, slot)?;
        let residues = self.residues(key);
        let field = &self.sample.field;
        let compiled = self.compiled[slot].as_ref().expect("compiled");
        let mut total = 0;
        for &term in terms {
            let term = &compiled.terms[term as usize];
            let numerator = term.numerator.evaluate(&residues, field);
            let denominator = term.denominator.evaluate(&residues, field);
            let inverse = field.inv(denominator).ok_or(LiteRedError::UnluckySample {
                phase: "edge denominator",
                detail: format!("rule slot {slot} at {:?}", &key[..self.index_count]),
            })?;
            total = field.mul_add(total, numerator, inverse);
        }
        Ok(total)
    }
}

pub(crate) fn pad<const N: usize>(powers: &[i64]) -> [i64; N] {
    std::array::from_fn(|i| powers.get(i).copied().unwrap_or(0))
}

/// Structure statistics of the sub-DAG reachable from a target set. The
/// counters follow the design prototype (`litered-design/scripts/sim.py`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DagStatistics {
    pub targets: usize,
    pub nodes: usize,
    pub rule_nodes: usize,
    pub edges: usize,
    pub terminals_reached: usize,
    pub uncovered: usize,
    pub zero_nodes: usize,
    pub sectors_touched: usize,
    pub max_rank_visited: u64,
    pub max_dots_visited: u64,
    /// Sum over rule nodes of the RHS term multiplicities (pre-coalescing).
    pub term_applications: usize,
}

/// The prime- and point-independent application DAG.
#[derive(Clone, Debug, Default)]
pub struct DagStructure<const N: usize> {
    pub(crate) nodes: Vec<Node<N>>,
    index: HashMap<[i64; N], u32>,
    state: Vec<VisitState>,
    /// Children before parents.
    pub(crate) postorder: Vec<u32>,
    terminal_count: usize,
}

/// Edge values of a [`DagStructure`] at one sample, parallel to its edges.
#[derive(Clone, Debug, Default)]
pub struct DagValues {
    pub(crate) values: Vec<Vec<u64>>,
}

impl<const N: usize> DagStructure<N> {
    pub fn new(terminal_count: usize) -> Self {
        Self {
            terminal_count,
            ..Default::default()
        }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
    pub fn terminal_count(&self) -> usize {
        self.terminal_count
    }
    pub fn node_of(&self, key: &[i64; N]) -> Option<u32> {
        self.index.get(key).copied()
    }
    pub fn key(&self, node: u32) -> &[i64; N] {
        &self.nodes[node as usize].key
    }
    pub fn kind(&self, node: u32) -> NodeKind {
        self.nodes[node as usize].kind
    }

    fn intern(&mut self, key: [i64; N]) -> u32 {
        if let Some(&id) = self.index.get(&key) {
            return id;
        }
        let id = self.nodes.len() as u32;
        self.nodes.push(Node {
            key,
            kind: NodeKind::Uncovered,
            edges: Vec::new(),
        });
        self.state.push(VisitState::Unvisited);
        self.index.insert(key, id);
        id
    }

    /// Expand every new target (iterative DFS); values of the build sample
    /// are appended to `values`. Returns the target node ids.
    pub(crate) fn extend(
        &mut self,
        view: CandidateProgramView<'_, N>,
        program: &mut SampleProgram<N>,
        values: &mut DagValues,
        targets: &[[i64; N]],
        max_nodes: usize,
    ) -> Result<Vec<u32>, LiteRedError> {
        let ids: Vec<u32> = targets.iter().map(|key| self.intern(*key)).collect();
        let mut stack: Vec<(u32, bool)> = ids
            .iter()
            .rev()
            .filter(|&&id| self.state[id as usize] == VisitState::Unvisited)
            .map(|&id| (id, false))
            .collect();
        while let Some((id, expanded)) = stack.pop() {
            let position = id as usize;
            if expanded {
                self.state[position] = VisitState::Done;
                self.postorder.push(id);
                continue;
            }
            if self.state[position] != VisitState::Unvisited {
                continue;
            }
            if self.nodes.len() > max_nodes {
                return Err(LiteRedError::Limit {
                    resource: "LiteRed DAG nodes",
                    limit: max_nodes,
                });
            }
            self.state[position] = VisitState::InProgress;
            let key = self.nodes[position].key;
            let classified = program.classify(view, &key)?;
            let mut edges = Vec::with_capacity(classified.children.len());
            let mut edge_values = Vec::with_capacity(classified.children.len());
            for (child, terms, value) in classified.children {
                let child = self.intern(child);
                edges.push(Edge { child, terms });
                edge_values.push(value);
            }
            self.nodes[position].kind = classified.kind;
            stack.push((id, true));
            for edge in edges.iter().rev() {
                match self.state[edge.child as usize] {
                    VisitState::Unvisited => stack.push((edge.child, false)),
                    VisitState::InProgress => {
                        return Err(LiteRedError::Candidate(
                            ReductionError::CycleDetected {
                                target: program
                                    .integral_key(&self.nodes[edge.child as usize].key)?,
                            }
                            .into(),
                        ));
                    }
                    VisitState::Done => {}
                }
            }
            self.nodes[position].edges = edges;
            if values.values.len() < self.nodes.len() {
                values.values.resize(self.nodes.len(), Vec::new());
            }
            values.values[position] = edge_values;
        }
        if values.values.len() < self.nodes.len() {
            values.values.resize(self.nodes.len(), Vec::new());
        }
        Ok(ids)
    }

    /// Re-evaluate every edge at another sample (no selection is repeated).
    pub(crate) fn evaluate(
        &self,
        view: CandidateProgramView<'_, N>,
        program: &mut SampleProgram<N>,
    ) -> Result<DagValues, LiteRedError> {
        let mut values = Vec::with_capacity(self.nodes.len());
        for node in &self.nodes {
            let mut edge_values = Vec::with_capacity(node.edges.len());
            if let NodeKind::Rule { slot, .. } = node.kind {
                for edge in &node.edges {
                    edge_values.push(program.edge_value(
                        view,
                        slot as usize,
                        &node.key,
                        &edge.terms,
                    )?);
                }
            }
            values.push(edge_values);
        }
        Ok(DagValues { values })
    }

    /// Statistics of the sub-DAG reachable from `targets`.
    pub fn statistics(&self, targets: &[u32]) -> DagStatistics {
        let mut seen = vec![false; self.nodes.len()];
        let mut stack: Vec<u32> = Vec::new();
        for &target in targets {
            if !seen[target as usize] {
                seen[target as usize] = true;
                stack.push(target);
            }
        }
        let mut stats = DagStatistics {
            targets: targets.len(),
            ..Default::default()
        };
        let mut sectors = std::collections::HashSet::new();
        while let Some(id) = stack.pop() {
            let node = &self.nodes[id as usize];
            stats.nodes += 1;
            match node.kind {
                NodeKind::Rule { .. } => {
                    stats.rule_nodes += 1;
                    stats.edges += node.edges.len();
                    stats.term_applications += node
                        .edges
                        .iter()
                        .map(|edge| edge.terms.len())
                        .sum::<usize>();
                }
                NodeKind::Terminal(_) => stats.terminals_reached += 1,
                NodeKind::Zero => stats.zero_nodes += 1,
                NodeKind::Uncovered => stats.uncovered += 1,
            }
            if node.kind != NodeKind::Zero {
                let sector: [bool; N] = std::array::from_fn(|i| node.key[i] > 0);
                sectors.insert(sector);
            }
            let rank: u64 = node
                .key
                .iter()
                .filter(|&&n| n < 0)
                .map(|&n| n.unsigned_abs())
                .sum();
            let dots: u64 = node
                .key
                .iter()
                .filter(|&&n| n > 1)
                .map(|&n| (n - 1) as u64)
                .sum();
            stats.max_rank_visited = stats.max_rank_visited.max(rank);
            stats.max_dots_visited = stats.max_dots_visited.max(dots);
            for edge in &node.edges {
                if !seen[edge.child as usize] {
                    seen[edge.child as usize] = true;
                    stack.push(edge.child);
                }
            }
        }
        stats.sectors_touched = sectors.len();
        stats
    }
}
