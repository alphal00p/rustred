use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

use crate::sector::error::{Error, try_copy_string};

use super::coordinate_priority::{CoordinatePriority, CoordinatePriorityLimits};

/// Stable identifier of RustRed's first deterministic integral order.
pub(crate) const RUSTRED_UNSHIFTED_ORDER_V1_ID: &str = "rustred.unshifted-sector-order.v1";
const COORDINATE_PRIORITY_ORDER_V1_PREFIX: &str = "rustred.unshifted-sector-order.v1;priority=";
const SPIRED_UNCUT_ORDER_V1_ID: &str = "rustred.spired-uncut-sector-order.v1";
const SPIRED_PRIORITY_ORDER_V1_PREFIX: &str = "rustred.spired-uncut-sector-order.v1;priority=";
const PROGRAMMED_UNCUT_ORDER_V1_PREFIX: &str = "rustred.programmed-uncut-sector-order.v1;data=";
#[cfg(test)]
const TEST_ONLY_DISTINCT_ORDER_ID: &str = "rustred.test-only-distinct-sector-order";

/// The largest permutation that has an injective factorial-rank encoding in
/// one `u128`. Since `34! < 2^128 < 35!`, this covers every family through
/// the anticipated six-loop `K=21` pressure target without putting a heap
/// allocation for these legacy coordinate-priority variants. Programmed
/// orders instead share an immutable descriptor and have no packed-arity cap.
pub const MAX_PACKED_ORDERING_PRIORITY_ARITY: usize = 34;

/// Fixed upper bound for the canonical identity of a legacy packed policy.
///
/// The longest supported identity is the coordinate-priority identity at
/// arity 34 and occupies fewer than 256 bytes. Keeping the representation on
/// the stack makes its identity rendering infallible and allocation-free.
/// Programmed policies borrow their cached, separately bounded identity.
const ORDERING_POLICY_STABLE_ID_CAPACITY: usize = 256;

/// Persisted choice of integral-ordering semantics.
///
/// Each coordinate-priority variant changes only its coordinate tie-break
/// priority, not its aggregate ordering. `rank_by_slot[slot] == 0` means
/// that slot is compared first. Its permutation is retained injectively as a
/// factorial rank and rendered back into a full-vector semantic identity.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OrderingPolicy {
    #[default]
    RustRedUnshiftedV1,
    RustRedUnshiftedCoordinatePriorityV1(CoordinatePriorityOrderingV1),
    /// The uncut SpIRed source-port order, expressed simpler-first like all
    /// artifact orders: sector, absolute degree, numerator degree, then
    /// reversed coordinate excess, with active coordinates compared first.
    /// The bounded absolute-degree level sets make the reversed final ties
    /// well-founded. This variant does not encode cut-index priorities.
    SpiredUncutV1,
    SpiredUncutCoordinatePriorityV1(CoordinatePriorityOrderingV1),
    /// One immutable validated runtime program. Concrete, symbolic and shift
    /// comparisons all consume this payload; it is not a discovery-only hint.
    ProgrammedUncutV1(Arc<ProgrammedOrdering>),
    /// Test-only distinct identity with the same arithmetic order. It exists
    /// solely to exercise exact owner-ordering rejection and cannot enter a
    /// production build or persisted artifact.
    #[cfg(test)]
    TestOnlyDistinct,
}

/// Cached canonical metadata belongs to the same shared payload as its order.
/// Identity is content-based; no process-local interning handle is persisted.
#[derive(Debug)]
pub struct ProgrammedOrdering {
    compiled: rustred_order::CompiledOrder,
    stable_id: Box<str>,
}

impl PartialEq for ProgrammedOrdering {
    fn eq(&self, other: &Self) -> bool {
        self.compiled == other.compiled
    }
}
impl Eq for ProgrammedOrdering {}
impl PartialOrd for ProgrammedOrdering {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for ProgrammedOrdering {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.compiled.cmp(&other.compiled)
    }
}
impl std::hash::Hash for ProgrammedOrdering {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.compiled, state);
    }
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
    pub fn try_programmed(compiled: rustred_order::CompiledOrder) -> Result<Self, Error> {
        let length = compiled
            .canonical_bytes()
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(PROGRAMMED_UNCUT_ORDER_V1_PREFIX.len()))
            .ok_or(Error::OrderProgram(rustred_order::Error::DimensionOverflow))?;
        let mut id = String::new();
        id.try_reserve_exact(length)
            .map_err(|_| Error::AllocationFailure {
                resource: "programmable order identity",
                requested: length,
            })?;
        id.push_str(PROGRAMMED_UNCUT_ORDER_V1_PREFIX);
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for &byte in compiled.canonical_bytes() {
            id.push(char::from(HEX[usize::from(byte >> 4)]));
            id.push(char::from(HEX[usize::from(byte & 15)]));
        }
        Ok(Self::ProgrammedUncutV1(Arc::new(ProgrammedOrdering {
            compiled,
            stable_id: id.into_boxed_str(),
        })))
    }

    pub fn program(&self) -> Option<&rustred_order::CompiledOrder> {
        match self {
            Self::ProgrammedUncutV1(payload) => Some(&payload.compiled),
            _ => None,
        }
    }

    /// Solver/source-port capability is distinct from a total-excess envelope.
    pub fn is_source_port_uncut(&self) -> bool {
        self.is_spired() || self.program().is_some()
    }

    pub fn has_total_excess_primary(&self) -> bool {
        self.program()
            .map_or(true, |program| program.has_total_excess_primary())
    }

    /// Whether support count precedes every physical-degree comparison.
    /// Sector-first induction and unconditional proper-pinch witnesses need
    /// this stronger property, not merely a fixed-support degree comparison.
    pub fn is_support_primary(&self) -> bool {
        self.program()
            .is_none_or(|program| program.is_support_primary())
    }

    /// Static identity for policies whose schema contains no payload.
    /// Payload-bearing policies use [`Self::stable_id`] instead.
    pub const fn static_stable_id(&self) -> Option<&'static str> {
        match self {
            Self::RustRedUnshiftedV1 => Some(RUSTRED_UNSHIFTED_ORDER_V1_ID),
            Self::SpiredUncutV1 => Some(SPIRED_UNCUT_ORDER_V1_ID),
            Self::RustRedUnshiftedCoordinatePriorityV1(_)
            | Self::SpiredUncutCoordinatePriorityV1(_)
            | Self::ProgrammedUncutV1(_) => None,
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

    /// Construct the uncut source-port order with this exact coordinate
    /// priority. Natural priority has the payload-free canonical identity.
    pub fn try_spired_with_coordinate_priority(
        priority: &CoordinatePriority,
    ) -> Result<Self, Error> {
        match Self::try_with_coordinate_priority(priority)? {
            Self::RustRedUnshiftedV1 => Ok(Self::SpiredUncutV1),
            Self::RustRedUnshiftedCoordinatePriorityV1(payload) => {
                Ok(Self::SpiredUncutCoordinatePriorityV1(payload))
            }
            _ => {
                unreachable!("the original-order constructor returns only original-order variants")
            }
        }
    }

    pub(crate) const fn is_spired(&self) -> bool {
        matches!(
            self,
            Self::SpiredUncutV1 | Self::SpiredUncutCoordinatePriorityV1(_)
        )
    }

    pub fn try_from_stable_id(id: &str) -> Result<Self, Error> {
        if let Some(hex) = id.strip_prefix(PROGRAMMED_UNCUT_ORDER_V1_PREFIX) {
            let limits = rustred_order::Limits::default();
            if hex.len() % 2 != 0 || hex.len() / 2 > limits.max_encoded_bytes {
                return Err(Error::OrderProgram(rustred_order::Error::InvalidEncoding));
            }
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(hex.len() / 2)
                .map_err(|_| Error::AllocationFailure {
                    resource: "programmable order identity",
                    requested: hex.len() / 2,
                })?;
            let digit = |value| match value {
                b'0'..=b'9' => Some(value - b'0'),
                b'a'..=b'f' => Some(value - b'a' + 10),
                _ => None,
            };
            for pair in hex.as_bytes().chunks_exact(2) {
                let a = digit(pair[0])
                    .ok_or(Error::OrderProgram(rustred_order::Error::InvalidEncoding))?;
                let b = digit(pair[1])
                    .ok_or(Error::OrderProgram(rustred_order::Error::InvalidEncoding))?;
                bytes.push((a << 4) | b);
            }
            return Self::try_programmed(
                rustred_order::CompiledOrder::from_canonical_bytes(&bytes, limits)
                    .map_err(Error::OrderProgram)?,
            );
        }
        if id == RUSTRED_UNSHIFTED_ORDER_V1_ID {
            return Ok(Self::RustRedUnshiftedV1);
        }
        if id == SPIRED_UNCUT_ORDER_V1_ID {
            return Ok(Self::SpiredUncutV1);
        }
        #[cfg(test)]
        if id == TEST_ONLY_DISTINCT_ORDER_ID {
            return Ok(Self::TestOnlyDistinct);
        }
        for (prefix, spired) in [
            (COORDINATE_PRIORITY_ORDER_V1_PREFIX, false),
            (SPIRED_PRIORITY_ORDER_V1_PREFIX, true),
        ] {
            let Some(priority_id) = id.strip_prefix(prefix) else {
                continue;
            };
            let limits = CoordinatePriorityLimits {
                max_arity: MAX_PACKED_ORDERING_PRIORITY_ARITY,
                max_stable_id_bytes: ORDERING_POLICY_STABLE_ID_CAPACITY,
            };
            if let Ok(priority) = CoordinatePriority::try_from_stable_id(priority_id, limits)
                && let Ok(policy) = if spired {
                    Self::try_spired_with_coordinate_priority(&priority)
                } else {
                    Self::try_with_coordinate_priority(&priority)
                }
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
    pub fn stable_id(&self) -> OrderingPolicyStableId<'_> {
        if let Self::ProgrammedUncutV1(payload) = self {
            return OrderingPolicyStableId {
                bytes: [0; ORDERING_POLICY_STABLE_ID_CAPACITY],
                len: 0,
                borrowed: Some(&payload.stable_id),
            };
        }
        let mut id = OrderingPolicyStableId::new();
        match self {
            Self::RustRedUnshiftedV1 => id.push_str(RUSTRED_UNSHIFTED_ORDER_V1_ID),
            Self::SpiredUncutV1 => id.push_str(SPIRED_UNCUT_ORDER_V1_ID),
            Self::ProgrammedUncutV1(_) => unreachable!("handled shared program identity"),
            Self::RustRedUnshiftedCoordinatePriorityV1(_)
            | Self::SpiredUncutCoordinatePriorityV1(_) => {
                id.push_str(if self.is_spired() {
                    SPIRED_PRIORITY_ORDER_V1_PREFIX
                } else {
                    COORDINATE_PRIORITY_ORDER_V1_PREFIX
                });
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
    /// final tie-break. Policies with natural priority return `None`.
    pub fn try_coordinate_priority(&self) -> Result<Option<CoordinatePriority>, Error> {
        if self.program().is_some() {
            return Err(Error::OrderProgramNotCoordinatePriority);
        }
        if self.coordinate_priority_arity().is_none() {
            return Ok(None);
        }
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
    pub const fn coordinate_priority_arity(&self) -> Option<usize> {
        match self {
            Self::RustRedUnshiftedCoordinatePriorityV1(payload)
            | Self::SpiredUncutCoordinatePriorityV1(payload) => Some(payload.arity as usize),
            Self::RustRedUnshiftedV1 | Self::SpiredUncutV1 | Self::ProgrammedUncutV1(_) => None,
            #[cfg(test)]
            Self::TestOnlyDistinct => None,
        }
    }

    pub(crate) fn require_arity(&self, actual: usize) -> Result<(), Error> {
        if let Some(expected) = self
            .program()
            .map(|program| program.arity())
            .or(self.coordinate_priority_arity())
        {
            if actual != expected {
                return Err(Error::WrongArity { expected, actual });
            }
        }
        Ok(())
    }

    /// Decode rank-by-slot into a fixed stack buffer. Unused entries are zero.
    pub(crate) fn decoded_rank_by_slot(&self) -> ([u8; MAX_PACKED_ORDERING_PRIORITY_ARITY], usize) {
        match self {
            Self::RustRedUnshiftedV1 | Self::SpiredUncutV1 => {
                ([0; MAX_PACKED_ORDERING_PRIORITY_ARITY], 0)
            }
            Self::ProgrammedUncutV1(_) => {
                unreachable!("a full program is not a legacy coordinate priority")
            }
            Self::RustRedUnshiftedCoordinatePriorityV1(payload)
            | Self::SpiredUncutCoordinatePriorityV1(payload) => {
                decode_permutation(usize::from(payload.arity), payload.permutation_rank)
            }
            #[cfg(test)]
            Self::TestOnlyDistinct => ([0; MAX_PACKED_ORDERING_PRIORITY_ARITY], 0),
        }
    }
}

/// Stack-backed canonical identity returned by [`OrderingPolicy::stable_id`].
#[derive(Clone, Copy)]
pub struct OrderingPolicyStableId<'a> {
    bytes: [u8; ORDERING_POLICY_STABLE_ID_CAPACITY],
    len: u16,
    borrowed: Option<&'a str>,
}

impl OrderingPolicyStableId<'_> {
    fn new() -> Self {
        Self {
            bytes: [0; ORDERING_POLICY_STABLE_ID_CAPACITY],
            len: 0,
            borrowed: None,
        }
    }

    pub fn as_str(&self) -> &str {
        if let Some(value) = self.borrowed {
            return value;
        }
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

impl AsRef<str> for OrderingPolicyStableId<'_> {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Deref for OrderingPolicyStableId<'_> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl fmt::Display for OrderingPolicyStableId<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Debug for OrderingPolicyStableId<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), formatter)
    }
}

impl PartialEq for OrderingPolicyStableId<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for OrderingPolicyStableId<'_> {}

impl PartialEq<&str> for OrderingPolicyStableId<'_> {
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
