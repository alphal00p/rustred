use super::error::ProbeError;

/// Non-raiseable ceiling for either phase of the recursive proof fixture.
///
/// An affine transition lowers its remaining degree, and an angular
/// transition lowers its cross degree. Consequently the call stack is at
/// most twice this value (plus constant frames), regardless of caller-supplied
/// [`ProbeLimits`]. This is a fixture safety bound, not a production claim.
pub(super) const HARD_MAX_PHASE_DEGREE: u64 = 64;
pub(super) const HARD_MAX_TRANSITIONS: usize = 100_000;
pub(super) const HARD_MAX_CACHE_ENTRIES: usize = 100_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ProbeLimits {
    pub(super) max_affine_degree: u64,
    pub(super) max_angular_degree: u64,
    pub(super) max_affine_transitions: usize,
    pub(super) max_angular_transitions: usize,
    pub(super) max_affine_cache_entries: usize,
    pub(super) max_angular_cache_entries: usize,
}

impl Default for ProbeLimits {
    fn default() -> Self {
        Self {
            max_affine_degree: HARD_MAX_PHASE_DEGREE,
            max_angular_degree: HARD_MAX_PHASE_DEGREE,
            max_affine_transitions: HARD_MAX_TRANSITIONS,
            max_angular_transitions: HARD_MAX_TRANSITIONS,
            max_affine_cache_entries: HARD_MAX_CACHE_ENTRIES,
            max_angular_cache_entries: HARD_MAX_CACHE_ENTRIES,
        }
    }
}

impl ProbeLimits {
    /// Reject configurations that could raise the fixture's structural stack
    /// or work bounds. The constructor and entry points are test-private, but
    /// the hard ceiling makes that safety property explicit and replayable.
    pub(super) fn validate(self) -> Result<Self, ProbeError> {
        admit_degree(
            "configured affine recursion ceiling",
            self.max_affine_degree,
            HARD_MAX_PHASE_DEGREE,
        )?;
        admit_degree(
            "configured angular recursion ceiling",
            self.max_angular_degree,
            HARD_MAX_PHASE_DEGREE,
        )?;
        admit_count_limit(
            "configured affine transition ceiling",
            self.max_affine_transitions,
            HARD_MAX_TRANSITIONS,
        )?;
        admit_count_limit(
            "configured angular transition ceiling",
            self.max_angular_transitions,
            HARD_MAX_TRANSITIONS,
        )?;
        admit_count_limit(
            "configured affine cache ceiling",
            self.max_affine_cache_entries,
            HARD_MAX_CACHE_ENTRIES,
        )?;
        admit_count_limit(
            "configured angular cache ceiling",
            self.max_angular_cache_entries,
            HARD_MAX_CACHE_ENTRIES,
        )?;
        Ok(self)
    }
}

fn admit_count_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ProbeError> {
    if requested <= limit {
        Ok(())
    } else {
        Err(ProbeError::CountLimit {
            resource,
            requested,
            limit,
        })
    }
}

pub(super) fn admit_degree(
    resource: &'static str,
    requested: u64,
    limit: u64,
) -> Result<(), ProbeError> {
    if requested <= limit {
        Ok(())
    } else {
        Err(ProbeError::DegreeLimit {
            resource,
            requested,
            limit,
        })
    }
}

pub(super) fn checked_total<const N: usize>(
    resource: &'static str,
    powers: &[u64; N],
) -> Result<u64, ProbeError> {
    powers.iter().try_fold(0_u64, |total, &power| {
        total
            .checked_add(power)
            .ok_or(ProbeError::DegreeOverflow { resource })
    })
}

pub(super) fn record_count(
    resource: &'static str,
    count: &mut usize,
    limit: usize,
) -> Result<(), ProbeError> {
    let requested = count
        .checked_add(1)
        .ok_or(ProbeError::CountOverflow { resource })?;
    if requested > limit {
        return Err(ProbeError::CountLimit {
            resource,
            requested,
            limit,
        });
    }
    *count = requested;
    Ok(())
}

pub(super) fn admit_new_cache_entry(
    resource: &'static str,
    current: usize,
    limit: usize,
) -> Result<(), ProbeError> {
    let requested = current
        .checked_add(1)
        .ok_or(ProbeError::CountOverflow { resource })?;
    if requested <= limit {
        Ok(())
    } else {
        Err(ProbeError::CountLimit {
            resource,
            requested,
            limit,
        })
    }
}
