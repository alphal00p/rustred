use super::{DomainPowerBounds, DomainPowerError};

/// Exact coordinate projections plus the indispensable retained predicates.
/// Aggregate endpoints are wider than public coordinate/cap endpoints because
/// sums over several coordinates may exceed their individual machine ranges.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DomainPowerProjection<const N: usize> {
    pub lower: [u64; N],
    pub upper: [Option<u64>; N],
    pub powers: DomainPowerBounds,
    pub effective_rank: Option<u32>,
    pub positive_lower: u128,
    pub positive_upper: Option<u128>,
    pub numerator_lower: u128,
    pub numerator_upper: Option<u128>,
    pub difference_lower: Option<i128>,
    pub difference_upper: Option<i128>,
}

#[derive(Clone, Copy, Default)]
struct Sum {
    lower: i128,
    finite_upper: i128,
    unbounded: usize,
}
impl Sum {
    fn push(&mut self, lower: u64, upper: Option<u64>) -> Result<(), DomainPowerError> {
        self.lower = add(self.lower, i128::from(lower), "coordinate lower sum")?;
        if let Some(upper) = upper {
            self.finite_upper = add(self.finite_upper, i128::from(upper), "coordinate upper sum")?;
        } else {
            self.unbounded = self
                .unbounded
                .checked_add(1)
                .ok_or(DomainPowerError::ArithmeticOverflow("unbounded axis count"))?;
        }
        Ok(())
    }
    fn upper(self) -> Option<i128> {
        (self.unbounded == 0).then_some(self.finite_upper)
    }
    fn other_upper(self, upper: Option<u64>) -> Result<Option<i128>, DomainPowerError> {
        let remaining_unbounded = self.unbounded - usize::from(upper.is_none());
        if remaining_unbounded != 0 {
            Ok(None)
        } else {
            upper
                .map_or(Ok(self.finite_upper), |upper| {
                    sub(
                        self.finite_upper,
                        i128::from(upper),
                        "other coordinate upper sum",
                    )
                })
                .map(Some)
        }
    }
}

#[derive(Clone, Copy)]
struct Interval {
    lower: i128,
    upper: Option<i128>,
}
impl Interval {
    fn empty(self) -> bool {
        self.upper.is_some_and(|upper| upper < self.lower)
    }
}

fn add(a: i128, b: i128, context: &'static str) -> Result<i128, DomainPowerError> {
    a.checked_add(b)
        .ok_or(DomainPowerError::ArithmeticOverflow(context))
}
fn sub(a: i128, b: i128, context: &'static str) -> Result<i128, DomainPowerError> {
    a.checked_sub(b)
        .ok_or(DomainPowerError::ArithmeticOverflow(context))
}
fn min_upper(a: Option<i128>, b: Option<i128>) -> Option<i128> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}
fn max_lower(a: Option<i128>, b: Option<i128>) -> Option<i128> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    }
}
fn unsigned(value: i128) -> Result<u128, DomainPowerError> {
    u128::try_from(value).map_err(|_| DomainPowerError::OutOfRange("negative aggregate power"))
}

/// Project a fixed-support box intersected with rank and total-power bounds.
/// Exact because the two groups are disjoint sums of integer intervals, with
/// no holes. This is O(N), and never enumerates points or polynomial guards.
pub(crate) fn project<const N: usize>(
    owner: &[bool; N],
    lower: &[u64],
    upper: &[Option<u64>],
    rank: Option<u32>,
    powers: DomainPowerBounds,
) -> Result<Option<DomainPowerProjection<N>>, DomainPowerError> {
    if N == 0 || N > 4096 || lower.len() != N || upper.len() != N {
        return Err(DomainPowerError::InvalidArity);
    }
    powers.validate()?;
    let mut groups = [Sum::default(); 2];
    let mut active = 0i128;
    for axis in 0..N {
        if upper[axis].is_some_and(|upper| upper < lower[axis]) {
            return Err(DomainPowerError::InvertedCoordinate { axis });
        }
        groups[usize::from(owner[axis])].push(lower[axis], upper[axis])?;
        active += i128::from(owner[axis]);
    }
    let a = Interval {
        lower: add(groups[1].lower, active, "positive lower offset")?,
        upper: min_upper(
            groups[1]
                .upper()
                .map(|value| add(value, active, "positive upper offset"))
                .transpose()?,
            powers.max_positive_power.map(i128::from),
        ),
    };
    let r = Interval {
        lower: groups[0].lower,
        upper: min_upper(groups[0].upper(), rank.map(i128::from)),
    };
    if a.empty() || r.empty() {
        return Ok(None);
    }
    let difference_lower = max_lower(
        powers.min_power_difference.map(i128::from),
        r.upper
            .map(|upper| sub(a.lower, upper, "minimum difference"))
            .transpose()?,
    );
    let difference_upper = min_upper(
        powers.max_power_difference.map(i128::from),
        a.upper
            .map(|upper| sub(upper, r.lower, "maximum difference"))
            .transpose()?,
    );
    if let (Some(lower), Some(upper)) = (difference_lower, difference_upper)
        && lower > upper
    {
        return Ok(None);
    }
    let projected_a = Interval {
        lower: difference_lower.map_or(Ok(a.lower), |d| {
            add(r.lower, d, "projected positive lower").map(|bound| a.lower.max(bound))
        })?,
        upper: min_upper(
            a.upper,
            r.upper
                .zip(difference_upper)
                .map(|(r, d)| add(r, d, "projected positive upper"))
                .transpose()?,
        ),
    };
    let projected_r = Interval {
        lower: difference_upper.map_or(Ok(r.lower), |d| {
            sub(a.lower, d, "projected numerator lower").map(|bound| r.lower.max(bound))
        })?,
        upper: min_upper(
            r.upper,
            a.upper
                .zip(difference_lower)
                .map(|(a, d)| sub(a, d, "projected numerator upper"))
                .transpose()?,
        ),
    };
    if projected_a.empty() || projected_r.empty() {
        return Ok(None);
    }
    let local_a = Interval {
        lower: sub(projected_a.lower, active, "positive local lower")?,
        upper: projected_a
            .upper
            .map(|upper| sub(upper, active, "positive local upper"))
            .transpose()?,
    };
    let intervals = [projected_r, local_a];
    let mut projected_lower = [0; N];
    let mut projected_upper = [None; N];
    for axis in 0..N {
        let group = usize::from(owner[axis]);
        let sum = groups[group];
        let interval = intervals[group];
        let other_lower = sub(sum.lower, i128::from(lower[axis]), "other lower sum")?;
        let lo = sum
            .other_upper(upper[axis])?
            .map_or(Ok(i128::from(lower[axis])), |other| {
                sub(interval.lower, other, "coordinate lower projection")
                    .map(|bound| i128::from(lower[axis]).max(bound))
            })?;
        let hi = min_upper(
            upper[axis].map(i128::from),
            interval
                .upper
                .map(|upper| sub(upper, other_lower, "coordinate upper projection"))
                .transpose()?,
        );
        projected_lower[axis] = u64::try_from(lo)
            .map_err(|_| DomainPowerError::OutOfRange("projected coordinate lower"))?;
        projected_upper[axis] = hi
            .map(|upper| {
                u64::try_from(upper)
                    .map_err(|_| DomainPowerError::OutOfRange("projected coordinate upper"))
            })
            .transpose()?;
    }
    // A finite implied upper wider than u32 is still encoded exactly by the
    // retained predicates/box. None here must not become Some(u32::MAX).
    let effective_rank = projected_r
        .upper
        .and_then(|upper| u32::try_from(upper).ok());
    Ok(Some(DomainPowerProjection {
        lower: projected_lower,
        upper: projected_upper,
        powers,
        effective_rank,
        positive_lower: unsigned(projected_a.lower)?,
        positive_upper: projected_a.upper.map(unsigned).transpose()?,
        numerator_lower: unsigned(projected_r.lower)?,
        numerator_upper: projected_r.upper.map(unsigned).transpose()?,
        difference_lower,
        difference_upper,
    }))
}
