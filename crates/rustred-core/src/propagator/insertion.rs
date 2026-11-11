//! The Insertion lane (owner: M2-T4 rr-kernel): integrate one one-loop bubble,
//! Forcer's insertion rule (architecture doc section 3.2b).
//!
//! The kernel is [`BubbleKernel`], in native Minkowski form (`kernel` module docs,
//! convention C2). A key of the owner presentation goes to keys of the child
//! presentation of the rule edge, or to [`PTerminal::Unit`] at one loop, where
//! the rule leaves the trivial node and no edge map exists.
//!
//! - Merged line: eps units `j1 + j2 + 1`, integer power `n1 + n2 - 2 - sigma`
//!   plus whatever the numerators and parallel kept lines shift, on
//!   `merged_slot`. Without a slot it is parallel to `q` and contributes
//!   `(-1)^{n1 + n2}` in front of the constant `(P^2)^sigma = (-1)^sigma`.
//! - Kept lines add their integer powers on their child slot.
//! - ISPs `k.Y` are the numerators of the kernel; every scalar product of its
//!   output and the ISPs free of `k` are rewritten through `outer_map` and
//!   `express_scalar_product` of the child, tabulated at construction.
//! - Children are [`PStage::Route`] keys: they may sit on any support of the
//!   child presentation. Absolute tags, as in [`PStepOut`].
//!
//! [`P_INSERTION_KERNEL_VERSION`] names this meaning of a recorded step: the
//! formula, its signs and tags, and the rewrite. Bump it on any semantic change.
//!
//! [`P_INSERTION_KERNEL_VERSION`]: super::P_INSERTION_KERNEL_VERSION
//! [`PTerminal::Unit`]: super::library::PTerminal::Unit
use std::sync::Arc;

use crate::algebra::CoefficientContext;
use crate::reduction::ReductionLimits;

use super::PTerminalId;
use super::error::PError;
use super::kernel::bubble::{BubbleKernel, LinePower, relative_tag};
use super::kernel::gamma::algebra;
use super::kernel::{
    ChildKeys, MemoCounters, NumeratorRewrite, OwnerEdge, absolute_tag, child_row,
    numerator_powers, owner_edges,
};
use super::lane::{BubbleRoles, LaneRoles, OwnerLane};
use super::library::{PLibrary, POwnerSpec};
use super::presentation::{
    EdgeKind, Presentation, Slot, check_insertion_contract, verify_edge_map,
};
use super::step::{PKeyView, PLaneStep, PStepContext, PStepOut};
use super::step_memo::{self, MemoKey, MemoMode};
use super::tag::GammaTag;

/// What the step produces: keys of the child, or the Unit terminal at one loop.
enum Target {
    Terminal,
    Child(Box<ChildRule>),
}

struct ChildRule {
    child: Arc<Presentation>,
    /// `(parent line slot, child line slot)` of the kept lines.
    kept: Vec<(usize, usize)>,
    merged_slot: Option<usize>,
    rewrite: NumeratorRewrite,
}

/// The bubble step of one owner, see the module docs.
pub struct InsertionStep {
    parent: Arc<Presentation>,
    roles: BubbleRoles,
    context: CoefficientContext,
    kernel: BubbleKernel,
    tag: GammaTag,
    eps_units: [i64; 2],
    /// ISP slots `k.Y`, the kernel's numerators, then the ISPs free of `k`.
    box_slots: Vec<usize>,
    free_slots: Vec<usize>,
    target: Target,
    /// rr-alloc: the id of this step in the per-thread memos ([`step_memo`]).
    uid: u64,
}

fn line_units(parent: &Presentation, slot: u16) -> Result<i64, PError> {
    match parent.slots.get(usize::from(slot)) {
        Some(Slot::Line(line)) => Ok(i64::from(line.eps_units)),
        _ => Err(PError::Contract(format!("slot {slot} is not a line"))),
    }
}

/// `true` when `row` is `+-q`.
fn is_external(row: &[i64]) -> bool {
    let (&last, rest) = row.split_last().expect("a momentum row has a q entry");
    last.abs() == 1 && rest.iter().all(|&c| c == 0)
}

impl InsertionStep {
    /// The step of `(parent, roles)`. `edges` holds the maps of the owner with
    /// their child presentations: none at one loop, exactly one above.
    ///
    /// The lane contract is checked here, once per owner, and with it what no
    /// other check sees: that the edge removes the two role lines of the role
    /// loop.
    pub fn new(
        parent: Arc<Presentation>,
        roles: BubbleRoles,
        edges: &[OwnerEdge],
    ) -> Result<Self, PError> {
        check_insertion_contract(&parent, &roles)?;
        let fail = |detail: String| {
            PError::Contract(format!("insertion step on {}: {detail}", parent.id.0))
        };
        let l = usize::from(roles.loop_index);
        let second = match &parent.slots[usize::from(roles.lines[1])] {
            Slot::Line(line) => &line.momentum,
            Slot::Isp(_) => return Err(fail("the second bubble line is an ISP".to_owned())),
        };
        let s1 = second[l];
        // k - P = s1 mom(lines[1]).
        let bubble: Vec<i64> = second
            .iter()
            .enumerate()
            .map(|(column, &c)| i64::from(column == l) - s1 * c)
            .collect();

        let mut box_slots = Vec::new();
        let mut free_slots = Vec::new();
        let mut box_rows = Vec::new();
        let mut free_rows = Vec::new();
        for (index, slot) in parent.slots.iter().enumerate() {
            let Slot::Isp(isp) = slot else { continue };
            match (isp.left[l], isp.right[l]) {
                (0, 0) => {
                    free_slots.push(index);
                    free_rows.push((isp.left.clone(), isp.right.clone()));
                }
                (sign, 0) => {
                    box_slots.push(index);
                    box_rows.push(isp.right.iter().map(|c| sign * c).collect::<Vec<i64>>());
                }
                (0, sign) => {
                    box_slots.push(index);
                    box_rows.push(isp.left.iter().map(|c| sign * c).collect::<Vec<i64>>());
                }
                _ => return Err(fail(format!("ISP slot {index} is not linear in the loop"))),
            }
        }

        let target = match edges {
            [] if parent.loops == 1 => {
                if !is_external(&bubble) {
                    return Err(fail(format!(
                        "at one loop the bubble momentum {bubble:?} must be +-q"
                    )));
                }
                Target::Terminal
            }
            [(edge, child)] if parent.loops > 1 => {
                verify_edge_map(&parent, child, edge)?;
                if edge.kind != EdgeKind::Insertion || edge.integrated_loop != roles.loop_index {
                    return Err(fail(format!(
                        "the edge is {:?} of loop {}, the roles integrate loop {}",
                        edge.kind, edge.integrated_loop, roles.loop_index
                    )));
                }
                for line in roles.lines {
                    if edge.line_slots[usize::from(line)].is_some() {
                        return Err(fail(format!("the edge keeps the role line {line}")));
                    }
                }
                let kept = edge
                    .line_slots
                    .iter()
                    .enumerate()
                    .filter_map(|(slot, target)| Some((slot, usize::from((*target)?))))
                    .collect();
                let map = |row: &[i64]| child_row(&edge.outer_map, EdgeKind::Insertion, l, row);
                let merged = map(&bubble)?;
                let variables = box_rows
                    .iter()
                    .map(|row| map(row))
                    .collect::<Result<Vec<_>, _>>()?;
                let free = free_rows
                    .iter()
                    .map(|(left, right)| Ok((map(left)?, map(right)?)))
                    .collect::<Result<Vec<_>, PError>>()?;
                let rewrite = NumeratorRewrite::new(
                    child,
                    &variables,
                    &merged,
                    &free,
                    Some((&merged, &merged)),
                )?;
                Target::Child(Box::new(ChildRule {
                    child: child.clone(),
                    kept,
                    merged_slot: edge.merged_slot.map(usize::from),
                    rewrite,
                }))
            }
            _ => {
                return Err(fail(format!(
                    "{} edge maps at {} loops",
                    edges.len(),
                    parent.loops
                )));
            }
        };

        let eps_units = [
            line_units(&parent, roles.lines[0])?,
            line_units(&parent, roles.lines[1])?,
        ];
        let context = parent.family.coefficient_context().clone();
        Ok(Self {
            tag: relative_tag(eps_units[0], eps_units[1])?,
            kernel: BubbleKernel::new(&context),
            context,
            parent,
            roles,
            eps_units,
            box_slots,
            free_slots,
            target,
            uid: step_memo::next_uid(),
        })
    }

    /// [`Self::new`] on an owner of a library.
    pub fn from_owner(library: &PLibrary, owner: &POwnerSpec) -> Result<Self, PError> {
        let LaneRoles::Insertion(roles) = owner.roles else {
            return Err(PError::Contract(format!(
                "an insertion step with roles {:?}",
                owner.roles
            )));
        };
        let (parent, edges) = owner_edges(library, owner)?;
        Self::new(parent, roles, &edges)
    }

    pub fn presentation(&self) -> &Arc<Presentation> {
        &self.parent
    }

    /// The child presentation, `None` at one loop.
    pub fn child(&self) -> Option<&Arc<Presentation>> {
        match &self.target {
            Target::Terminal => None,
            Target::Child(rule) => Some(&rule.child),
        }
    }

    /// Memo counters: weights by `(j1, j2, n1, n2, n, sigma)`, then numerator tables.
    pub fn memo_counters(&self) -> (MemoCounters, MemoCounters) {
        let tables = match &self.target {
            Target::Terminal => MemoCounters::default(),
            Target::Child(rule) => rule.rewrite.counters(),
        };
        (self.kernel.counters(), tables)
    }

    /// The step on `key`, without a library context.
    pub fn apply(&self, key: &PKeyView<'_>, limits: &ReductionLimits) -> Result<PStepOut, PError> {
        let powers = key.powers;
        if key.presentation != self.parent.id || powers.len() != self.parent.slots.len() {
            return Err(PError::Contract(format!(
                "key of presentation {} with {} powers on insertion step {}",
                key.presentation.0,
                powers.len(),
                self.parent.id.0
            )));
        }
        let box_powers = numerator_powers(powers, &self.box_slots)?;
        let free_powers = numerator_powers(powers, &self.free_slots)?;

        let limits = limits.exact_algebra;
        let alpha = LinePower::new(powers[usize::from(self.roles.lines[0])], self.eps_units[0]);
        let beta = LinePower::new(powers[usize::from(self.roles.lines[1])], self.eps_units[1]);
        let mut output = PStepOut::default();
        if alpha.vanishes() || beta.vanishes() {
            return Ok(output);
        }
        let tag = absolute_tag(key.tag, &self.tag)?;
        let rank: u32 = box_powers.iter().sum();
        match &self.target {
            Target::Terminal => {
                let mut weight = self.kernel.ratio(alpha, beta, 0, 0, limits)?;
                if (alpha.integer + beta.integer) % 2 != 0 {
                    weight = self.context.try_neg(&weight, limits).map_err(algebra)?;
                }
                output.terminals.push((PTerminalId(0), tag, weight));
            }
            Target::Child(rule) => {
                let mut base = vec![0_i64; rule.child.slots.len()];
                for &(slot, target) in &rule.kept {
                    base[target] += powers[slot];
                }
                let negate = match rule.merged_slot {
                    Some(slot) => {
                        base[slot] += alpha.integer + beta.integer - 2;
                        false
                    }
                    None => (alpha.integer + beta.integer) % 2 != 0,
                };
                let keys = ChildKeys {
                    context: &self.context,
                    child: &rule.child,
                    base,
                    negate,
                    tag: &tag,
                };
                let direct = |output: &mut PStepOut| -> Result<(), PError> {
                    let table = rule.rewrite.table(&box_powers, &free_powers)?;
                    let weights = self.kernel_weights(alpha, beta, rank, limits)?;
                    keys.emit(&table, &weights, limits, output)
                };
                let mode = step_memo::mode();
                if mode == MemoMode::Off {
                    direct(&mut output)?;
                } else {
                    // The weights are a function of the two line powers (the
                    // sign of `negate` included) and the numerators.
                    let mut key = MemoKey::new();
                    key.push(self.uid as i64);
                    key.push(alpha.integer);
                    key.push(beta.integer);
                    key.extend(box_powers.iter().map(|&p| i64::from(p)));
                    key.extend(free_powers.iter().map(|&p| i64::from(p)));
                    let memo = step_memo::insertion(&key, limits, || {
                        let table = rule.rewrite.table(&box_powers, &free_powers)?;
                        let weights = self.kernel_weights(alpha, beta, rank, limits)?;
                        let weights = keys.weights(&table, &weights, limits)?;
                        Ok((table, weights))
                    })?;
                    match memo {
                        Some(memo) => step_memo::coefficients_of(&memo, |values| {
                            keys.emit_weights(&memo.table, &memo.weights, values, &mut output);
                        }),
                        // The shared memo is full: compute directly.
                        None => direct(&mut output)?,
                    }
                    if mode == MemoMode::Check {
                        let mut reference = PStepOut::default();
                        direct(&mut reference)?;
                        if reference != output {
                            return Err(step_memo::mismatch("insertion step"));
                        }
                    }
                }
            }
        }
        Ok(output)
    }
}

impl InsertionStep {
    /// The bubble weights `W_sigma`, `sigma = 0..=rank/2`.
    fn kernel_weights(
        &self,
        alpha: LinePower,
        beta: LinePower,
        rank: u32,
        limits: crate::algebra::ExactAlgebraLimits,
    ) -> Result<Vec<crate::algebra::Coefficient>, PError> {
        (0..=rank / 2)
            .map(|sigma| {
                self.kernel
                    .ratio(alpha, beta, i64::from(rank), i64::from(sigma), limits)
            })
            .collect()
    }
}

impl std::fmt::Debug for InsertionStep {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InsertionStep")
            .field("presentation", &self.parent.id)
            .field("roles", &self.roles)
            .finish_non_exhaustive()
    }
}

impl PLaneStep for InsertionStep {
    fn lane(&self) -> OwnerLane {
        OwnerLane::Insertion
    }

    fn step(&self, cx: &PStepContext<'_>, key: &PKeyView<'_>) -> Result<PStepOut, PError> {
        self.apply(key, cx.limits)
    }
}
