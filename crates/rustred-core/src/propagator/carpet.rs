//! The Carpet lane (owner: M2-T4 rr-kernel): integrate the outer loop of a
//! one-loop graph around a blob, Forcer's carpet rule (architecture doc section
//! 3.2c).
//!
//! The kernel is [`CarpetKernel`], in native Minkowski form (`kernel` module
//! docs, convention C2). The child is the blob as an `(L - 1)`-loop p-integral
//! with external momentum `p -> q`: blob lines keep their powers, the `l.q` ISPs
//! are the numerators of the double sum, the `q`-free ISPs count in the blob's
//! exponent sum `A` and are rewritten like every other scalar product through
//! `outer_map` and `express_scalar_product` of the child. Lines `r` and `third`
//! go away with `(-1)^{n_r + n_3}`. Children are [`PStage::Route`] keys with
//! absolute tags, as in [`PStepOut`].
//!
//! [`P_CARPET_KERNEL_VERSION`] names this meaning of a recorded step. Bump it on
//! any semantic change.
//!
//! [`P_CARPET_KERNEL_VERSION`]: super::P_CARPET_KERNEL_VERSION
use std::sync::Arc;

use crate::algebra::CoefficientContext;
use crate::reduction::ReductionLimits;

use super::error::PError;
use super::kernel::bubble::LinePower;
use super::kernel::carpet::{CarpetKernel, carpet_tag};
use super::kernel::{
    ChildKeys, MemoCounters, NumeratorRewrite, OwnerEdge, absolute_tag, child_row,
    numerator_powers, owner_edges,
};
use super::lane::{CarpetRoles, LaneRoles, OwnerLane};
use super::library::{PLibrary, POwnerSpec};
use super::presentation::{EdgeKind, Presentation, Slot, check_carpet_contract, verify_edge_map};
use super::step::{PKeyView, PLaneStep, PStepContext, PStepOut};
use super::tag::GammaTag;

/// The carpet step of one owner, see the module docs.
pub struct CarpetStep {
    parent: Arc<Presentation>,
    child: Arc<Presentation>,
    roles: CarpetRoles,
    context: CoefficientContext,
    kernel: CarpetKernel,
    tag: GammaTag,
    r_units: i64,
    /// `j_3 + j_blob + L_sub`.
    inner_units: i64,
    /// `(parent line slot, child line slot)` of the blob lines.
    kept: Vec<(usize, usize)>,
    /// ISP slots `l.q`, the kernel's numerators, then the `q`-free ISPs.
    box_slots: Vec<usize>,
    free_slots: Vec<usize>,
    rewrite: NumeratorRewrite,
}

impl CarpetStep {
    /// The step of `(parent, roles)`; `edges` holds exactly the owner's one map
    /// with its child presentation.
    ///
    /// The lane contract is checked here, once per owner, and with it what no
    /// other check sees: that the edge removes `r` and `third` of the role loop.
    pub fn new(
        parent: Arc<Presentation>,
        roles: CarpetRoles,
        edges: &[OwnerEdge],
    ) -> Result<Self, PError> {
        check_carpet_contract(&parent, &roles)?;
        let fail =
            |detail: String| PError::Contract(format!("carpet step on {}: {detail}", parent.id.0));
        let [(edge, child)] = edges else {
            return Err(fail(format!("{} edge maps, not 1", edges.len())));
        };
        verify_edge_map(&parent, child, edge)?;
        if edge.kind != EdgeKind::Carpet || edge.integrated_loop != roles.outer_loop {
            return Err(fail(format!(
                "the edge is {:?} of loop {}, the roles integrate loop {}",
                edge.kind, edge.integrated_loop, roles.outer_loop
            )));
        }
        for line in std::iter::once(roles.r).chain(roles.third) {
            if edge.line_slots[usize::from(line)].is_some() {
                return Err(fail(format!("the edge keeps the role line {line}")));
            }
        }

        let outer = usize::from(roles.outer_loop);
        let width = usize::from(parent.loops) + 1;
        let external = width - 1;
        let map = |row: &[i64]| child_row(&edge.outer_map, EdgeKind::Carpet, outer, row);
        let mut p = vec![0_i64; width];
        p[outer] = 1;
        let p = map(&p)?;

        let mut kept = Vec::new();
        let mut blob_units = 0_i64;
        let mut third_units = 0_i64;
        let mut r_units = 0_i64;
        let mut box_slots = Vec::new();
        let mut free_slots = Vec::new();
        let mut variables = Vec::new();
        let mut free = Vec::new();
        for (index, (slot, target)) in parent.slots.iter().zip(&edge.line_slots).enumerate() {
            match slot {
                Slot::Line(line) => {
                    let units = i64::from(line.eps_units);
                    match *target {
                        Some(target) => {
                            kept.push((index, usize::from(target)));
                            blob_units += units;
                        }
                        None if index == usize::from(roles.r) => r_units = units,
                        None if Some(index) == roles.third.map(usize::from) => third_units = units,
                        None => return Err(fail(format!("the edge removes line {index}"))),
                    }
                }
                Slot::Isp(isp) => match (isp.left[external], isp.right[external]) {
                    (0, 0) => {
                        free_slots.push(index);
                        free.push((map(&isp.left)?, map(&isp.right)?));
                    }
                    (sign, 0) => {
                        box_slots.push(index);
                        let blob: Vec<i64> = isp.right.iter().map(|c| sign * c).collect();
                        variables.push(map(&blob)?);
                    }
                    (0, sign) => {
                        box_slots.push(index);
                        let blob: Vec<i64> = isp.left.iter().map(|c| sign * c).collect();
                        variables.push(map(&blob)?);
                    }
                    _ => return Err(fail(format!("ISP slot {index} is quadratic in q"))),
                },
            }
        }
        let rewrite = NumeratorRewrite::new(child, &variables, &p, &free, None)?;
        let inner_units = third_units + blob_units + i64::from(child.loops);
        let context = parent.family.coefficient_context().clone();
        Ok(Self {
            tag: carpet_tag(LinePower::new(1, inner_units), LinePower::new(1, r_units))?,
            kernel: CarpetKernel::new(&context),
            context,
            child: child.clone(),
            parent,
            roles,
            r_units,
            inner_units,
            kept,
            box_slots,
            free_slots,
            rewrite,
        })
    }

    /// [`Self::new`] on an owner of a library.
    pub fn from_owner(library: &PLibrary, owner: &POwnerSpec) -> Result<Self, PError> {
        let LaneRoles::Carpet(roles) = owner.roles else {
            return Err(PError::Contract(format!(
                "a carpet step with roles {:?}",
                owner.roles
            )));
        };
        let (parent, edges) = owner_edges(library, owner)?;
        Self::new(parent, roles, &edges)
    }

    pub fn presentation(&self) -> &Arc<Presentation> {
        &self.parent
    }

    pub fn child(&self) -> &Arc<Presentation> {
        &self.child
    }

    /// Memo counters: order weights, then numerator tables.
    pub fn memo_counters(&self) -> (MemoCounters, MemoCounters) {
        (self.kernel.counters(), self.rewrite.counters())
    }

    /// The step on `key`, without a library context.
    pub fn apply(&self, key: &PKeyView<'_>, limits: &ReductionLimits) -> Result<PStepOut, PError> {
        let powers = key.powers;
        if key.presentation != self.parent.id || powers.len() != self.parent.slots.len() {
            return Err(PError::Contract(format!(
                "key of presentation {} with {} powers on carpet step {}",
                key.presentation.0,
                powers.len(),
                self.parent.id.0
            )));
        }
        let box_powers = numerator_powers(powers, &self.box_slots)?;
        let free_powers = numerator_powers(powers, &self.free_slots)?;

        let limits = limits.exact_algebra;
        let r = LinePower::new(powers[usize::from(self.roles.r)], self.r_units);
        let mut output = PStepOut::default();
        if r.vanishes() {
            return Ok(output);
        }
        let third = self.roles.third.map_or(0, |slot| powers[usize::from(slot)]);
        let blob: i64 = self
            .kept
            .iter()
            .map(|&(slot, _)| powers[slot])
            .chain(self.free_slots.iter().map(|&slot| powers[slot]))
            .sum();
        let inner = LinePower::new(
            third + blob - 2 * i64::from(self.child.loops),
            self.inner_units,
        );
        let tag = absolute_tag(key.tag, &self.tag)?;
        let rank: u32 = box_powers.iter().sum();
        let table = self.rewrite.table(&box_powers, &free_powers)?;
        let weights = (0..=rank / 2)
            .map(|order| {
                self.kernel
                    .order_weight(inner, r, i64::from(rank), i64::from(order), limits)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut base = vec![0_i64; self.child.slots.len()];
        for &(slot, target) in &self.kept {
            base[target] += powers[slot];
        }
        let keys = ChildKeys {
            context: &self.context,
            child: &self.child,
            base,
            negate: (r.integer + third) % 2 != 0,
            tag: &tag,
        };
        keys.emit(&table, &weights, limits, &mut output)?;
        Ok(output)
    }
}

impl std::fmt::Debug for CarpetStep {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CarpetStep")
            .field("presentation", &self.parent.id)
            .field("roles", &self.roles)
            .finish_non_exhaustive()
    }
}

impl PLaneStep for CarpetStep {
    fn lane(&self) -> OwnerLane {
        OwnerLane::Carpet
    }

    fn step(&self, cx: &PStepContext<'_>, key: &PKeyView<'_>) -> Result<PStepOut, PError> {
        self.apply(key, cx.limits)
    }
}
