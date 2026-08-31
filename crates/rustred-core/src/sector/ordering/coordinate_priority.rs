use std::borrow::Borrow;
use std::fmt;
use std::fmt::Write as _;

/// Stable prefix of the exact coordinate-priority identity.
pub const COORDINATE_PRIORITY_V1_PREFIX: &str = "rustred.coordinate-priority.v1;k=";
const RANKS_FIELD: &str = ";rank-by-slot=";

pub const DEFAULT_MAX_COORDINATE_PRIORITY_ARITY: usize = 4_096;
pub const DEFAULT_MAX_COORDINATE_PRIORITY_STABLE_ID_BYTES: usize = 1_048_576;

/// Hard construction and parsing limits for one coordinate priority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoordinatePriorityLimits {
    pub max_arity: usize,
    /// Every successfully constructed descriptor is guaranteed to have a
    /// canonical v1 identity no longer than this bound.
    pub max_stable_id_bytes: usize,
}

impl Default for CoordinatePriorityLimits {
    fn default() -> Self {
        Self {
            max_arity: DEFAULT_MAX_COORDINATE_PRIORITY_ARITY,
            max_stable_id_bytes: DEFAULT_MAX_COORDINATE_PRIORITY_STABLE_ID_BYTES,
        }
    }
}

/// Typed failures while constructing or parsing an exact coordinate priority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoordinatePriorityError {
    Empty,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    RankOutOfRange {
        slot: usize,
        rank: usize,
        arity: usize,
    },
    DuplicateRank {
        slot: usize,
        rank: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    MalformedStableId {
        detail: &'static str,
    },
}

impl fmt::Display for CoordinatePriorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("a coordinate priority needs at least one slot"),
            Self::WrongArity { expected, actual } => write!(
                formatter,
                "coordinate priority has {actual} ranks, expected {expected}"
            ),
            Self::RankOutOfRange { slot, rank, arity } => write!(
                formatter,
                "coordinate-priority slot {slot} has rank {rank}, outside arity {arity}"
            ),
            Self::DuplicateRank { slot, rank } => write!(
                formatter,
                "coordinate-priority slot {slot} repeats rank {rank}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "coordinate-priority {resource} overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "coordinate-priority {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for coordinate-priority {resource}"
            ),
            Self::MalformedStableId { detail } => {
                write!(
                    formatter,
                    "malformed coordinate-priority stable identity: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for CoordinatePriorityError {}

/// A complete priority assignment in the `rank_by_slot` convention.
///
/// `rank_by_slot[slot]` is the unique rank of that coordinate, with rank zero
/// considered first. The vector is a bijection of `0..arity`; retaining the
/// complete vector makes equality, transport, persistence, and tie-breaking
/// exact rather than dependent on a lossy hash or portfolio ordinal.
#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CoordinatePriority {
    rank_by_slot: Vec<usize>,
}

impl CoordinatePriority {
    pub fn try_new(
        arity: usize,
        rank_by_slot: &[usize],
        limits: CoordinatePriorityLimits,
    ) -> Result<Self, CoordinatePriorityError> {
        admit_descriptor(arity, limits)?;
        if rank_by_slot.len() != arity {
            return Err(CoordinatePriorityError::WrongArity {
                expected: arity,
                actual: rank_by_slot.len(),
            });
        }
        let mut retained = Vec::new();
        retained.try_reserve_exact(arity).map_err(|_| {
            CoordinatePriorityError::AllocationFailure {
                resource: "rank vector",
                requested: arity,
            }
        })?;
        retained.extend_from_slice(rank_by_slot);
        Self::try_from_owned(arity, retained)
    }

    pub fn try_natural(
        arity: usize,
        limits: CoordinatePriorityLimits,
    ) -> Result<Self, CoordinatePriorityError> {
        admit_descriptor(arity, limits)?;
        let mut rank_by_slot = Vec::new();
        rank_by_slot.try_reserve_exact(arity).map_err(|_| {
            CoordinatePriorityError::AllocationFailure {
                resource: "natural rank vector",
                requested: arity,
            }
        })?;
        rank_by_slot.extend(0..arity);
        Ok(Self::from_validated_rank_by_slot(rank_by_slot))
    }

    /// Parse the exact canonical v1 identity. Whitespace, leading zeroes,
    /// omitted fields, and reordered fields are rejected rather than
    /// normalized silently.
    pub fn try_from_stable_id(
        id: &str,
        limits: CoordinatePriorityLimits,
    ) -> Result<Self, CoordinatePriorityError> {
        admit_limit(
            "stable identity bytes",
            id.len(),
            limits.max_stable_id_bytes,
        )?;
        let remainder = id.strip_prefix(COORDINATE_PRIORITY_V1_PREFIX).ok_or(
            CoordinatePriorityError::MalformedStableId {
                detail: "missing exact v1 prefix",
            },
        )?;
        let (arity_text, ranks_text) = remainder.split_once(RANKS_FIELD).ok_or(
            CoordinatePriorityError::MalformedStableId {
                detail: "missing rank-by-slot field",
            },
        )?;
        let arity = parse_canonical_usize(arity_text, "invalid arity")?;
        admit_descriptor(arity, limits)?;

        let actual = if ranks_text.is_empty() {
            0
        } else {
            ranks_text.split(',').count()
        };
        if actual != arity {
            return Err(CoordinatePriorityError::WrongArity {
                expected: arity,
                actual,
            });
        }

        let mut ranks = Vec::new();
        ranks
            .try_reserve_exact(arity)
            .map_err(|_| CoordinatePriorityError::AllocationFailure {
                resource: "parsed rank vector",
                requested: arity,
            })?;
        for rank in ranks_text.split(',') {
            ranks.push(parse_canonical_usize(rank, "invalid rank")?);
        }
        let priority = Self::try_from_owned(arity, ranks)?;
        // This is an exact schema boundary: accepting an alternate spelling
        // would make the supposedly stable identity non-unique.
        if priority.try_stable_id(limits)? != id {
            return Err(CoordinatePriorityError::MalformedStableId {
                detail: "identity is not in canonical v1 form",
            });
        }
        Ok(priority)
    }

    pub fn arity(&self) -> usize {
        self.rank_by_slot.len()
    }

    pub fn rank_by_slot(&self) -> &[usize] {
        &self.rank_by_slot
    }

    /// Materialize the exact full-vector identity through a bounded,
    /// fallible allocation seam.
    pub fn try_stable_id(
        &self,
        limits: CoordinatePriorityLimits,
    ) -> Result<String, CoordinatePriorityError> {
        admit_descriptor(self.arity(), limits)?;
        let requested = stable_id_bytes(self.arity())?;

        let mut id = String::new();
        id.try_reserve_exact(requested).map_err(|_| {
            CoordinatePriorityError::AllocationFailure {
                resource: "stable identity bytes",
                requested,
            }
        })?;
        id.push_str(COORDINATE_PRIORITY_V1_PREFIX);
        write!(&mut id, "{}", self.arity()).expect("writing to String cannot fail");
        id.push_str(RANKS_FIELD);
        for (slot, rank) in self.rank_by_slot.iter().enumerate() {
            if slot != 0 {
                id.push(',');
            }
            write!(&mut id, "{rank}").expect("writing to String cannot fail");
        }
        debug_assert_eq!(id.len(), requested);
        Ok(id)
    }

    fn try_from_owned(
        arity: usize,
        rank_by_slot: Vec<usize>,
    ) -> Result<Self, CoordinatePriorityError> {
        debug_assert_eq!(rank_by_slot.len(), arity);
        let mut seen = Vec::new();
        seen.try_reserve_exact(arity)
            .map_err(|_| CoordinatePriorityError::AllocationFailure {
                resource: "bijection replay",
                requested: arity,
            })?;
        seen.resize(arity, false);
        for (slot, &rank) in rank_by_slot.iter().enumerate() {
            let Some(seen_rank) = seen.get_mut(rank) else {
                return Err(CoordinatePriorityError::RankOutOfRange { slot, rank, arity });
            };
            if std::mem::replace(seen_rank, true) {
                return Err(CoordinatePriorityError::DuplicateRank { slot, rank });
            }
        }
        Ok(Self::from_validated_rank_by_slot(rank_by_slot))
    }

    pub(crate) fn from_validated_rank_by_slot(rank_by_slot: Vec<usize>) -> Self {
        debug_assert!(!rank_by_slot.is_empty());
        debug_assert!(rank_by_slot.iter().enumerate().all(|(slot, &rank)| rank
            < rank_by_slot.len()
            && !rank_by_slot[..slot].contains(&rank)));
        Self { rank_by_slot }
    }
}

impl AsRef<[usize]> for CoordinatePriority {
    fn as_ref(&self) -> &[usize] {
        self.rank_by_slot()
    }
}

impl Borrow<[usize]> for CoordinatePriority {
    fn borrow(&self) -> &[usize] {
        self.rank_by_slot()
    }
}

impl fmt::Display for CoordinatePriority {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(COORDINATE_PRIORITY_V1_PREFIX)?;
        write!(formatter, "{}", self.arity())?;
        formatter.write_str(RANKS_FIELD)?;
        for (slot, rank) in self.rank_by_slot.iter().enumerate() {
            if slot != 0 {
                formatter.write_str(",")?;
            }
            write!(formatter, "{rank}")?;
        }
        Ok(())
    }
}

fn admit_arity(
    arity: usize,
    limits: CoordinatePriorityLimits,
) -> Result<(), CoordinatePriorityError> {
    if arity == 0 {
        return Err(CoordinatePriorityError::Empty);
    }
    admit_limit("arity", arity, limits.max_arity)
}

fn admit_descriptor(
    arity: usize,
    limits: CoordinatePriorityLimits,
) -> Result<(), CoordinatePriorityError> {
    admit_arity(arity, limits)?;
    admit_limit(
        "stable identity bytes",
        stable_id_bytes(arity)?,
        limits.max_stable_id_bytes,
    )
}

fn stable_id_bytes(arity: usize) -> Result<usize, CoordinatePriorityError> {
    let mut requested = COORDINATE_PRIORITY_V1_PREFIX
        .len()
        .checked_add(decimal_digits(arity))
        .and_then(|length| length.checked_add(RANKS_FIELD.len()))
        .ok_or(CoordinatePriorityError::ResourceCountOverflow {
            resource: "stable identity bytes",
        })?;
    for rank in 0..arity {
        requested = requested
            .checked_add(usize::from(rank != 0))
            .and_then(|length| length.checked_add(decimal_digits(rank)))
            .ok_or(CoordinatePriorityError::ResourceCountOverflow {
                resource: "stable identity bytes",
            })?;
    }
    Ok(requested)
}

fn admit_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), CoordinatePriorityError> {
    if requested <= limit {
        Ok(())
    } else {
        Err(CoordinatePriorityError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    }
}

fn decimal_digits(mut value: usize) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

fn parse_canonical_usize(
    text: &str,
    detail: &'static str,
) -> Result<usize, CoordinatePriorityError> {
    if text.is_empty()
        || (text.len() > 1 && text.starts_with('0'))
        || !text.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(CoordinatePriorityError::MalformedStableId { detail });
    }
    text.parse()
        .map_err(|_| CoordinatePriorityError::MalformedStableId { detail })
}
