use super::{DomainPowerBounds, DomainPowerError, project};

/// Exact, cached fixed-template domain geometry, independent of raw diagnostic
/// identity, program snapshot, scheduling phase and coverage authority.
///
/// Build once for a fixed-support box intersected with R and A/D bounds, then
/// reuse [`Self::contains`] without allocation, projection or algebra. An empty
/// domain is represented explicitly and cannot be confused with an unbounded
/// domain. Malformed or unrepresentable input fails before a summary exists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DomainPowerSummary<const N: usize> {
    owner: [bool; N],
    extrema: Option<DomainPowerExtrema<N>>,
}

/// Tight extrema of every linear form in the fixed domain vocabulary.
///
/// Coordinate extrema are local (n=x+1 on active axes and n=-x otherwise).
/// Aggregate extrema are physical A, R and D=A-R. Upper None means +infinity;
/// only the signed D lower endpoint can be None, meaning -infinity. Finite
/// aggregates may exceed the public u64/i64 input caps and the u32 rank cap.
/// Fields are private: only checked domain projection can construct a value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DomainPowerExtrema<const N: usize> {
    lower: [u64; N],
    upper: [Option<u64>; N],
    positive_lower: u128,
    positive_upper: Option<u128>,
    numerator_lower: u128,
    numerator_upper: Option<u128>,
    difference_lower: Option<i128>,
    difference_upper: Option<i128>,
}

impl<const N: usize> DomainPowerSummary<N> {
    /// Uses the existing O(N) checked disjoint-sum projection. This is discrete
    /// endpoint bookkeeping, not a new polynomial or polyhedral solver.
    pub fn try_new(
        owner: [bool; N],
        lower: &[u64],
        upper: &[Option<u64>],
        rank: Option<u32>,
        powers: DomainPowerBounds,
    ) -> Result<Self, DomainPowerError> {
        let extrema = project(&owner, lower, upper, rank, powers)?.map(|p| DomainPowerExtrema {
            lower: p.lower,
            upper: p.upper,
            positive_lower: p.positive_lower,
            positive_upper: p.positive_upper,
            numerator_lower: p.numerator_lower,
            numerator_upper: p.numerator_upper,
            difference_lower: p.difference_lower,
            difference_upper: p.difference_upper,
        });
        Ok(Self { owner, extrema })
    }

    pub fn owner(&self) -> &[bool; N] {
        &self.owner
    }

    pub fn is_empty(&self) -> bool {
        self.extrema.is_none()
    }

    pub fn extrema(&self) -> Option<&DomainPowerExtrema<N>> {
        self.extrema.as_ref()
    }

    /// Exact integer-set inclusion for the admitted fixed-template domain class.
    /// Does not prove that either domain has been processed or covered by rules.
    /// Callers must separately check program identity and scheduling phase.
    ///
    /// A domain equals the conjunction of its own tight coordinate/A/R/D
    /// extrema: all its points satisfy them, and they imply every original
    /// defining inequality. Thus candidate extrema inside container extrema
    /// are necessary and sufficient, even when different original inequalities
    /// describe the same set. This does not claim that arbitrary correlated
    /// sets are determined by their coordinate or aggregate extrema.
    ///
    /// Empty candidates are contained even across supports. Nonempty domains
    /// of different supports are disjoint. Both inputs were fully validated by
    /// construction; malformed bounds can never become an empty-set shortcut.
    pub fn contains(&self, candidate: &Self) -> bool {
        let Some(other) = candidate.extrema.as_ref() else {
            return true;
        };
        let Some(this) = self.extrema.as_ref() else {
            return false;
        };
        self.owner == candidate.owner
            && this.positive_lower <= other.positive_lower
            && upper_contains(this.positive_upper, other.positive_upper)
            && this.numerator_lower <= other.numerator_lower
            && upper_contains(this.numerator_upper, other.numerator_upper)
            && lower_contains(this.difference_lower, other.difference_lower)
            && upper_contains(this.difference_upper, other.difference_upper)
            && this.lower.iter().zip(&other.lower).all(|(a, b)| a <= b)
            && this
                .upper
                .iter()
                .zip(&other.upper)
                .all(|(&a, &b)| upper_contains(a, b))
    }
}

impl<const N: usize> DomainPowerExtrema<N> {
    pub fn lower(&self) -> &[u64; N] {
        &self.lower
    }

    pub fn upper(&self) -> &[Option<u64>; N] {
        &self.upper
    }

    pub fn positive_power(&self) -> (u128, Option<u128>) {
        (self.positive_lower, self.positive_upper)
    }

    pub fn numerator_rank(&self) -> (u128, Option<u128>) {
        (self.numerator_lower, self.numerator_upper)
    }

    pub fn power_difference(&self) -> (Option<i128>, Option<i128>) {
        (self.difference_lower, self.difference_upper)
    }
}

fn upper_contains<T: Ord>(container: Option<T>, candidate: Option<T>) -> bool {
    container.is_none_or(|a| candidate.is_some_and(|b| b <= a))
}

fn lower_contains<T: Ord>(container: Option<T>, candidate: Option<T>) -> bool {
    container.is_none_or(|a| candidate.is_some_and(|b| b >= a))
}

#[cfg(test)]
mod tests;
