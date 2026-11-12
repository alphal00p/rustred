//! The interface every lane implements (owner: M2-I0 rr-iface).
use crate::algebra::Coefficient;
use crate::reduction::ReductionLimits;
use crate::sector::ComplexityKey;

use super::error::PError;
use super::lane::{LaneRoles, OwnerLane};
use super::library::{PLibrary, POwner, POwnerSpec};
use super::tag::GammaTag;
use super::{PTerminalId, PresentationId};

/// One lane program. Pure: the same key gives the same output, so a step can
/// be recorded and replayed.
pub trait PLaneStep: Send + Sync {
    fn lane(&self) -> OwnerLane;
    fn step(&self, cx: &PStepContext<'_>, key: &PKeyView<'_>) -> Result<PStepOut, PError>;
    /// The order of this owner's keys, a pure function of `powers`: a same-owner
    /// Apply child must lie strictly below its parent, or the Ordered engine
    /// refuses the step. The default fits every lane without same-owner
    /// children and the Triangle lane; a Custom owner returns the order its
    /// rules were generated in.
    fn order_key(&self, cx: &PStepContext<'_>, powers: &[i64]) -> Result<POrderKey, PError> {
        default_order_key(&cx.owner.spec, powers)
    }
}

/// The entries of [`POrderKey::Lexicographic`].
pub type OrderEntries = smallvec::SmallVec<[i64; 4]>;

/// Position of a key among the keys of one owner, larger first.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum POrderKey {
    /// Compared entry by entry (rr-alloc: inline up to 4 entries, no heap
    /// allocation per priority; the order is that of the entries).
    Lexicographic(OrderEntries),
    /// The order of a generated rule set.
    Complexity(ComplexityKey),
}

/// [`PLaneStep::order_key`] of a lane without an order of its own: on a Triangle
/// owner `n_b + n_c1 + n_c2`, which every step lowers, then the distance
/// `Sum_{n > 0} (n - 1) + Sum_{n <= 0} |n|` from the corner of the sector.
pub fn default_order_key(owner: &POwnerSpec, powers: &[i64]) -> Result<POrderKey, PError> {
    let mut key = OrderEntries::new();
    if let LaneRoles::Triangle(roles) = owner.roles {
        let mut lowered = 0_i64;
        for slot in [roles.b, roles.c[0], roles.c[1]] {
            let power = powers.get(usize::from(slot)).ok_or_else(|| {
                PError::Contract(format!(
                    "triangle slot {slot} on a key of {} slots",
                    powers.len()
                ))
            })?;
            lowered = lowered.saturating_add(*power);
        }
        key.push(lowered);
    }
    key.push(powers.iter().fold(0_i64, |distance, &power| {
        distance.saturating_add(if power > 0 {
            power - 1
        } else {
            power.saturating_neg()
        })
    }));
    Ok(POrderKey::Lexicographic(key))
}

pub struct PStepContext<'a> {
    pub library: &'a PLibrary,
    pub owner: &'a POwner,
    pub limits: &'a ReductionLimits,
}

/// The key of one step: the powers of the presentation (its arity) and the
/// absolute tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PKeyView<'a> {
    pub presentation: PresentationId,
    pub powers: &'a [i64],
    pub tag: &'a GammaTag,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PStage {
    Route,
    Apply,
}

/// One child key with its absolute tag (parent tag + relative tag of the step)
/// and its weight in `Q(d)`, context `{d}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PChild {
    pub stage: PStage,
    pub presentation: PresentationId,
    pub powers: Vec<i64>,
    pub tag: GammaTag,
    pub coefficient: Coefficient,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PStepOut {
    pub children: Vec<PChild>,
    /// A terminal, its absolute tag and its weight, as for a [`PChild`].
    pub terminals: Vec<(PTerminalId, GammaTag, Coefficient)>,
    pub conditions: Vec<Coefficient>,
}
