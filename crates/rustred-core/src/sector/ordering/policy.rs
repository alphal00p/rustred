use std::fmt;
use std::ops::Deref;

use crate::sector::error::{Error, try_copy_string};

use super::coordinate_priority::{CoordinatePriority, CoordinatePriorityLimits};

/// Stable identifier of RustRed's first deterministic integral order.
pub(crate) const RUSTRED_UNSHIFTED_ORDER_V1_ID: &str = "rustred.unshifted-sector-order.v1";
const COORDINATE_PRIORITY_ORDER_V1_PREFIX: &str = "rustred.unshifted-sector-order.v1;priority=";
#[cfg(test)]
const TEST_ONLY_DISTINCT_ORDER_ID: &str = "rustred.test-only-distinct-sector-order";

/// The largest permutation that has an injective factorial-rank encoding in
/// one `u128`. Since `34! < 2^128 < 35!`, this covers every family through
/// the anticipated six-loop `K=21` pressure target without putting a heap
/// allocation inside the pervasive, copyable ordering policy.
pub const MAX_PACKED_ORDERING_PRIORITY_ARITY: usize = 34;

/// Fixed upper bound for the canonical identity of a packed ordering policy.
///
/// The longest supported identity is the coordinate-priority identity at
/// arity 34 and occupies fewer than 256 bytes. Keeping the representation on
/// the stack makes identity rendering infallible and allocation-free.
const ORDERING_POLICY_STABLE_ID_CAPACITY: usize = 256;

/// Persisted choice of integral-ordering semantics.
///
/// The coordinate-priority variant changes only the last, per-coordinate
/// excess tie-break of the unshifted v1 key. `rank_by_slot[slot] == 0` means
/// that slot is compared first. Its permutation is retained injectively as a
/// factorial rank and rendered back into a full-vector semantic identity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OrderingPolicy {
    #[default]
    RustRedUnshiftedV1,
    RustRedUnshiftedCoordinatePriorityV1(CoordinatePriorityOrderingV1),
    /// Test-only distinct identity with the same arithmetic order. It exists
    /// solely to exercise exact owner-ordering rejection and cannot enter a
    /// production build or persisted artifact.
    #[cfg(test)]
    TestOnlyDistinct,
}

/// Validated packed payload of the coordinate-priority v1 ordering.
///
/// Its fields are deliberately private: policies can only be constructed
/// through [`OrderingPolicy::try_with_coordinate_priority`] or exact stable
/// identity parsing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CoordinatePriorityOrderingV1 {
    arity: u8,
    permutation_rank: u128,
}

impl OrderingPolicy {
    /// Static identity for policies whose schema contains no payload.
    /// Payload-bearing policies use [`Self::stable_id`] instead.
    pub const fn static_stable_id(self) -> Option<&'static str> {
        match self {
            Self::RustRedUnshiftedV1 => Some(RUSTRED_UNSHIFTED_ORDER_V1_ID),
            Self::RustRedUnshiftedCoordinatePriorityV1(_) => None,
            #[cfg(test)]
            Self::TestOnlyDistinct => Some(TEST_ONLY_DISTINCT_ORDER_ID),
        }
    }

    /// Construct the exact coordinate-priority order. A natural priority is
    /// canonicalized to the original v1 identity, avoiding two persisted
    /// names for identical semantics.
    pub fn try_with_coordinate_priority(priority: &CoordinatePriority) -> Result<Self, Error> {
        if priority.is_natural() {
            return Ok(Self::RustRedUnshiftedV1);
        }
        if priority.arity() > MAX_PACKED_ORDERING_PRIORITY_ARITY {
            return Err(Error::OrderingPriorityArityLimit {
                actual: priority.arity(),
                limit: MAX_PACKED_ORDERING_PRIORITY_ARITY,
            });
        }
        let arity = u8::try_from(priority.arity()).expect("packed ordering arity fits u8");
        let permutation_rank = encode_permutation(priority.rank_by_slot());
        Ok(Self::RustRedUnshiftedCoordinatePriorityV1(
            CoordinatePriorityOrderingV1 {
                arity,
                permutation_rank,
            },
        ))
    }

    pub fn try_from_stable_id(id: &str) -> Result<Self, Error> {
        if id == RUSTRED_UNSHIFTED_ORDER_V1_ID {
            return Ok(Self::RustRedUnshiftedV1);
        }
        #[cfg(test)]
        if id == TEST_ONLY_DISTINCT_ORDER_ID {
            return Ok(Self::TestOnlyDistinct);
        }
        if let Some(priority_id) = id.strip_prefix(COORDINATE_PRIORITY_ORDER_V1_PREFIX) {
            let limits = CoordinatePriorityLimits {
                max_arity: MAX_PACKED_ORDERING_PRIORITY_ARITY,
                max_stable_id_bytes: ORDERING_POLICY_STABLE_ID_CAPACITY,
            };
            if let Ok(priority) = CoordinatePriority::try_from_stable_id(priority_id, limits)
                && let Ok(policy) = Self::try_with_coordinate_priority(&priority)
                && policy.stable_id().as_str() == id
            {
                return Ok(policy);
            }
        }
        Err(Error::UnknownOrderingPolicy {
            id: try_copy_string(id, "ordering policy identifier")?,
        })
    }

    /// Render the exact canonical semantic identity without allocating.
    pub fn stable_id(self) -> OrderingPolicyStableId {
        let mut id = OrderingPolicyStableId::new();
        match self {
            Self::RustRedUnshiftedV1 => id.push_str(RUSTRED_UNSHIFTED_ORDER_V1_ID),
            Self::RustRedUnshiftedCoordinatePriorityV1(_) => {
                id.push_str(COORDINATE_PRIORITY_ORDER_V1_PREFIX);
                id.push_str(super::coordinate_priority::COORDINATE_PRIORITY_V1_PREFIX);
                let (ranks, arity) = self.decoded_rank_by_slot();
                id.push_decimal(arity);
                id.push_str(";rank-by-slot=");
                for (slot, rank) in ranks[..arity].iter().copied().enumerate() {
                    if slot != 0 {
                        id.push_byte(b',');
                    }
                    id.push_decimal(usize::from(rank));
                }
            }
            #[cfg(test)]
            Self::TestOnlyDistinct => id.push_str(TEST_ONLY_DISTINCT_ORDER_ID),
        }
        id
    }

    /// Return the exact coordinate priority when this policy has a custom
    /// final tie-break. The original v1 and test-only policies return `None`.
    pub fn try_coordinate_priority(self) -> Result<Option<CoordinatePriority>, Error> {
        let Self::RustRedUnshiftedCoordinatePriorityV1(_) = self else {
            return Ok(None);
        };
        let (ranks, arity) = self.decoded_rank_by_slot();
        let mut retained = Vec::new();
        retained
            .try_reserve_exact(arity)
            .map_err(|_| Error::AllocationFailure {
                resource: "ordering coordinate priority",
                requested: arity,
            })?;
        retained.extend(ranks[..arity].iter().map(|&rank| usize::from(rank)));
        Ok(Some(CoordinatePriority::from_validated_rank_by_slot(
            retained,
        )))
    }

    /// Arity fixed by a coordinate-priority payload, if present.
    pub const fn coordinate_priority_arity(self) -> Option<usize> {
        match self {
            Self::RustRedUnshiftedCoordinatePriorityV1(payload) => Some(payload.arity as usize),
            Self::RustRedUnshiftedV1 => None,
            #[cfg(test)]
            Self::TestOnlyDistinct => None,
        }
    }

    pub(crate) fn require_arity(self, actual: usize) -> Result<(), Error> {
        if let Self::RustRedUnshiftedCoordinatePriorityV1(payload) = self {
            let expected = usize::from(payload.arity);
            if actual != expected {
                return Err(Error::WrongArity { expected, actual });
            }
        }
        Ok(())
    }

    /// Decode rank-by-slot into a fixed stack buffer. Unused entries are zero.
    pub(crate) fn decoded_rank_by_slot(self) -> ([u8; MAX_PACKED_ORDERING_PRIORITY_ARITY], usize) {
        match self {
            Self::RustRedUnshiftedV1 => ([0; MAX_PACKED_ORDERING_PRIORITY_ARITY], 0),
            Self::RustRedUnshiftedCoordinatePriorityV1(payload) => {
                decode_permutation(usize::from(payload.arity), payload.permutation_rank)
            }
            #[cfg(test)]
            Self::TestOnlyDistinct => ([0; MAX_PACKED_ORDERING_PRIORITY_ARITY], 0),
        }
    }

    /// Visit coordinate slots in the exact final tie-break order.
    pub(crate) fn compare_coordinate_slices<T: Ord>(
        self,
        left: &[T],
        right: &[T],
    ) -> std::cmp::Ordering {
        debug_assert_eq!(left.len(), right.len());
        if let Self::RustRedUnshiftedCoordinatePriorityV1(_) = self {
            let (rank_by_slot, arity) = self.decoded_rank_by_slot();
            debug_assert_eq!(left.len(), arity);
            for rank in 0..arity {
                let slot = rank_by_slot[..arity]
                    .iter()
                    .position(|&candidate| usize::from(candidate) == rank)
                    .expect("decoded priority is a bijection");
                let comparison = left[slot].cmp(&right[slot]);
                if comparison != std::cmp::Ordering::Equal {
                    return comparison;
                }
            }
            std::cmp::Ordering::Equal
        } else {
            left.cmp(right)
        }
    }

    pub(crate) fn first_differing_coordinate<T: Eq>(
        self,
        left: &[T],
        right: &[T],
    ) -> Option<usize> {
        debug_assert_eq!(left.len(), right.len());
        if let Self::RustRedUnshiftedCoordinatePriorityV1(_) = self {
            let (rank_by_slot, arity) = self.decoded_rank_by_slot();
            debug_assert_eq!(left.len(), arity);
            (0..arity).find_map(|rank| {
                let slot = rank_by_slot[..arity]
                    .iter()
                    .position(|&candidate| usize::from(candidate) == rank)
                    .expect("decoded priority is a bijection");
                (left[slot] != right[slot]).then_some(slot)
            })
        } else {
            left.iter()
                .zip(right)
                .position(|(left, right)| left != right)
        }
    }
}

/// Stack-backed canonical identity returned by [`OrderingPolicy::stable_id`].
#[derive(Clone, Copy)]
pub struct OrderingPolicyStableId {
    bytes: [u8; ORDERING_POLICY_STABLE_ID_CAPACITY],
    len: u16,
}

impl OrderingPolicyStableId {
    fn new() -> Self {
        Self {
            bytes: [0; ORDERING_POLICY_STABLE_ID_CAPACITY],
            len: 0,
        }
    }

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..usize::from(self.len)])
            .expect("ordering-policy identities contain ASCII only")
    }

    fn push_str(&mut self, value: &str) {
        let start = usize::from(self.len);
        let end = start
            .checked_add(value.len())
            .expect("bounded ordering-policy identity length cannot overflow");
        assert!(end <= self.bytes.len(), "ordering-policy identity bound");
        self.bytes[start..end].copy_from_slice(value.as_bytes());
        self.len = u16::try_from(end).expect("ordering-policy identity fits u16");
    }

    fn push_byte(&mut self, value: u8) {
        let position = usize::from(self.len);
        assert!(
            position < self.bytes.len(),
            "ordering-policy identity bound"
        );
        self.bytes[position] = value;
        self.len += 1;
    }

    fn push_decimal(&mut self, mut value: usize) {
        let mut digits = [0_u8; 20];
        let mut count = 0;
        loop {
            digits[count] = b'0' + u8::try_from(value % 10).expect("decimal digit fits u8");
            count += 1;
            value /= 10;
            if value == 0 {
                break;
            }
        }
        for &digit in digits[..count].iter().rev() {
            self.push_byte(digit);
        }
    }
}

impl AsRef<str> for OrderingPolicyStableId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Deref for OrderingPolicyStableId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl fmt::Display for OrderingPolicyStableId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Debug for OrderingPolicyStableId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), formatter)
    }
}

impl PartialEq for OrderingPolicyStableId {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for OrderingPolicyStableId {}

impl PartialEq<&str> for OrderingPolicyStableId {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

fn encode_permutation(rank_by_slot: &[usize]) -> u128 {
    let mut code = 0_u128;
    for (slot, &rank) in rank_by_slot.iter().enumerate() {
        let digit = rank_by_slot[slot + 1..]
            .iter()
            .filter(|&&later| later < rank)
            .count();
        let radix = rank_by_slot.len() - slot;
        code = code
            .checked_mul(radix as u128)
            .and_then(|value| value.checked_add(digit as u128))
            .expect("factorial rank fits u128 through arity 34");
    }
    code
}

fn decode_permutation(
    arity: usize,
    mut code: u128,
) -> ([u8; MAX_PACKED_ORDERING_PRIORITY_ARITY], usize) {
    debug_assert!((1..=MAX_PACKED_ORDERING_PRIORITY_ARITY).contains(&arity));
    let mut digits = [0_u8; MAX_PACKED_ORDERING_PRIORITY_ARITY];
    for slot in (0..arity).rev() {
        let radix = (arity - slot) as u128;
        digits[slot] = u8::try_from(code % radix).expect("factoradic digit fits u8");
        code /= radix;
    }
    debug_assert_eq!(code, 0);

    let mut rank_by_slot = [0_u8; MAX_PACKED_ORDERING_PRIORITY_ARITY];
    let mut used = [false; MAX_PACKED_ORDERING_PRIORITY_ARITY];
    for slot in 0..arity {
        let mut remaining = usize::from(digits[slot]);
        for (rank, is_used) in used[..arity].iter_mut().enumerate() {
            if *is_used {
                continue;
            }
            if remaining == 0 {
                rank_by_slot[slot] = u8::try_from(rank).expect("packed rank fits u8");
                *is_used = true;
                break;
            }
            remaining -= 1;
        }
    }
    (rank_by_slot, arity)
}

#[cfg(test)]
mod tests {
    use crate::sector::{CoordinatePriority, CoordinatePriorityLimits, OrderingPolicy};

    use super::{MAX_PACKED_ORDERING_PRIORITY_ARITY, decode_permutation, encode_permutation};

    #[test]
    fn factorial_rank_round_trips_all_small_permutations() {
        fn visit(values: &mut [usize], offset: usize) {
            if offset == values.len() {
                let code = encode_permutation(values);
                let (decoded, arity) = decode_permutation(values.len(), code);
                assert_eq!(
                    decoded[..arity]
                        .iter()
                        .map(|&rank| usize::from(rank))
                        .collect::<Vec<_>>(),
                    values
                );
                return;
            }
            for position in offset..values.len() {
                values.swap(offset, position);
                visit(values, offset + 1);
                values.swap(offset, position);
            }
        }
        for arity in 1..=7 {
            visit(&mut (0..arity).collect::<Vec<_>>(), 0);
        }
    }

    #[test]
    fn maximum_packed_permutation_has_a_deterministic_full_vector_identity() {
        let ranks = (0..MAX_PACKED_ORDERING_PRIORITY_ARITY)
            .rev()
            .collect::<Vec<_>>();
        let priority = CoordinatePriority::try_new(
            MAX_PACKED_ORDERING_PRIORITY_ARITY,
            &ranks,
            CoordinatePriorityLimits::default(),
        )
        .unwrap();
        let policy = OrderingPolicy::try_with_coordinate_priority(&priority).unwrap();
        let stable = policy.stable_id();
        assert_eq!(OrderingPolicy::try_from_stable_id(&stable).unwrap(), policy);
        assert_eq!(
            policy
                .try_coordinate_priority()
                .unwrap()
                .unwrap()
                .rank_by_slot(),
            ranks
        );
    }
}
