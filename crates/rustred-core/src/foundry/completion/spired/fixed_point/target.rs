use std::cmp::Ordering;
use std::fmt::Write as _;

use crate::family::IntegralKey;
use crate::foundry::completion::source_discovery::CanonicalExactOwnerLedger;
use crate::foundry::completion::source_discovery::leader_walk::{LeaderWalkTask, LeaderWalkWave};
use crate::sector::CoordinatePriority;

use super::SpiredFixedPointError;

const SCOPE_PREFIX: &str = "rustred.spired.fixed-point.v1;family=";
const CONTEXT_FIELD: &str = ";context=";
const SECTOR_FIELD: &str = ";sector=";
const PREDECESSOR_FIELD: &str = ";predecessor=";
const ORDERING_FIELD: &str = ";ordering=";
const REVISION_FIELD: &str = ";revision=";

pub(super) fn try_scope_key(
    ledger: &CanonicalExactOwnerLedger,
) -> Result<Box<str>, SpiredFixedPointError> {
    let family = ledger.predecessor_snapshot().family_fingerprint();
    let context = ledger.predecessor_snapshot().context_fingerprint();
    let sector = ledger.sector().active_bits();
    let predecessor = ledger.predecessor_snapshot().id().as_str();
    let ordering = ledger.ordering().stable_id();
    let ordering = ordering.as_str();
    let revision = ledger.revision().get();
    let requested = SCOPE_PREFIX
        .len()
        .checked_add(decimal_digits(family.len()))
        .and_then(|value| value.checked_add(1))
        .and_then(|value| value.checked_add(family.len()))
        .and_then(|value| value.checked_add(CONTEXT_FIELD.len()))
        .and_then(|value| value.checked_add(decimal_digits(context.len())))
        .and_then(|value| value.checked_add(1))
        .and_then(|value| value.checked_add(context.len()))
        .and_then(|value| value.checked_add(SECTOR_FIELD.len()))
        .and_then(|value| value.checked_add(sector.len()))
        .and_then(|value| value.checked_add(PREDECESSOR_FIELD.len()))
        .and_then(|value| value.checked_add(decimal_digits(predecessor.len())))
        .and_then(|value| value.checked_add(1))
        .and_then(|value| value.checked_add(predecessor.len()))
        .and_then(|value| value.checked_add(ORDERING_FIELD.len()))
        .and_then(|value| value.checked_add(decimal_digits(ordering.len())))
        .and_then(|value| value.checked_add(1))
        .and_then(|value| value.checked_add(ordering.len()))
        .and_then(|value| value.checked_add(REVISION_FIELD.len()))
        .and_then(|value| value.checked_add(decimal_digits_u64(revision)))
        .ok_or(SpiredFixedPointError::ResourceCountOverflow {
            resource: "stable scope-key bytes",
        })?;
    let mut key = String::new();
    key.try_reserve_exact(requested)
        .map_err(|_| SpiredFixedPointError::AllocationFailure {
            resource: "stable scope-key bytes",
            requested,
        })?;
    key.push_str(SCOPE_PREFIX);
    write!(&mut key, "{}#", family.len()).map_err(|_| scope_write_error())?;
    key.push_str(family);
    key.push_str(CONTEXT_FIELD);
    write!(&mut key, "{}#", context.len()).map_err(|_| scope_write_error())?;
    key.push_str(context);
    key.push_str(SECTOR_FIELD);
    key.extend(sector.iter().map(|active| if *active { '1' } else { '0' }));
    key.push_str(PREDECESSOR_FIELD);
    write!(&mut key, "{}#", predecessor.len()).map_err(|_| scope_write_error())?;
    key.push_str(predecessor);
    key.push_str(ORDERING_FIELD);
    write!(&mut key, "{}#", ordering.len()).map_err(|_| scope_write_error())?;
    key.push_str(ordering);
    key.push_str(REVISION_FIELD);
    write!(&mut key, "{revision}").map_err(|_| scope_write_error())?;
    debug_assert_eq!(key.len(), requested);
    Ok(key.into_boxed_str())
}

const fn decimal_digits(mut value: usize) -> usize {
    let mut digits = 1usize;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

const fn decimal_digits_u64(mut value: u64) -> usize {
    let mut digits = 1usize;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

const fn scope_write_error() -> SpiredFixedPointError {
    SpiredFixedPointError::Invariant {
        detail: "writing into a pre-reserved stable scope key failed",
    }
}

pub(super) fn try_target_key(
    ledger: &CanonicalExactOwnerLedger,
    task: &LeaderWalkTask,
) -> Result<IntegralKey, SpiredFixedPointError> {
    let arity = ledger.sector().arity();
    let mut powers = Vec::new();
    powers
        .try_reserve_exact(arity)
        .map_err(|_| SpiredFixedPointError::AllocationFailure {
            resource: "target integral powers",
            requested: arity,
        })?;
    for (position, (corner, shift)) in ledger
        .sector()
        .corner_indices()
        .zip(task.target_shift().values().iter().copied())
        .enumerate()
    {
        powers.push(corner.checked_add(shift).ok_or(
            SpiredFixedPointError::TargetPowerOverflow {
                position,
                corner,
                shift,
            },
        )?);
    }
    if powers.len() != arity {
        return Err(SpiredFixedPointError::Invariant {
            detail: "leader target and ledger sector have different arity",
        });
    }
    IntegralKey::try_new(powers).map_err(Into::into)
}

pub(super) fn try_order_wave(
    wave: &LeaderWalkWave,
    priority: Option<&CoordinatePriority>,
) -> Result<Vec<usize>, SpiredFixedPointError> {
    let mut order = Vec::new();
    order.try_reserve_exact(wave.tasks().len()).map_err(|_| {
        SpiredFixedPointError::AllocationFailure {
            resource: "ordered leader-task indices",
            requested: wave.tasks().len(),
        }
    })?;
    order.extend(0..wave.tasks().len());
    if let Some(priority) = priority.filter(|priority| !priority.is_natural()) {
        order.sort_unstable_by(|&left, &right| {
            compare_task(&wave.tasks()[left], &wave.tasks()[right], priority)
        });
    }
    Ok(order)
}

fn compare_task(
    left: &LeaderWalkTask,
    right: &LeaderWalkTask,
    priority: &CoordinatePriority,
) -> Ordering {
    let rank = |task: &LeaderWalkTask| {
        task.key()
            .depth_one_axis()
            .map_or(usize::MAX, |axis| priority.rank_by_slot()[axis])
    };
    rank(left)
        .cmp(&rank(right))
        .then_with(|| left.key().cmp(right.key()))
        .then_with(|| left.canonical_ordinal().cmp(&right.canonical_ordinal()))
}
