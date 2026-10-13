//! Small deterministic boundary samples of exact saved entry domains.
//!
//! These are test inputs, not an enumeration or a proof of domain closure.
//! Native disjoint-sum projection owns feasibility; projected coordinate
//! extrema must be fixed sequentially rather than combined into a false corner.

use std::sync::atomic::AtomicBool;

use rustred::family::IntegralKey;
use rustred::solver::{
    DomainPowerSummary, EntryWitnessLimits, EntryWitnessOutcome, FiniteRootAdmission,
    RootRegionInput, pick_entry_intersection_witness,
};

use crate::AppError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SavedApplicationWitnessKind {
    Representative,
    MaximumNumeratorRank,
    MaximumPositivePower,
}

#[derive(Clone, Debug)]
pub struct SavedApplicationWitness {
    pub kind: SavedApplicationWitnessKind,
    pub integral: IntegralKey,
}

/// Sample a representable point and two aggregate-boundary points.
///
/// Equal points deliberately retain their distinct labels: the caller may
/// deduplicate execution while retaining every domain/requirement association.
/// In this fixed-template geometry constraints couple axes only through the
/// positive and negative sums. Maximizing one coordinate in a group and
/// reprojecting the remainder therefore preserves an achievable maximum of
/// that group's sum. Every proposed point is independently admitted afterward.
pub fn saved_application_boundary_witnesses<const N: usize>(
    region: &RootRegionInput<N>,
) -> Result<Vec<SavedApplicationWitness>, AppError> {
    let policy = FiniteRootAdmission::try_new([region.clone()], 1)
        .map_err(|error| AppError::input(error.to_string()))?;
    let physical_arity = region.lower.len();
    if region.support[physical_arity..]
        .iter()
        .any(|&active| active)
    {
        return Err(AppError::input(
            "storage padding cannot be a physical denominator",
        ));
    }
    // The generic witness picker takes full storage coordinates. Persisted
    // inputs and returned integral keys retain their physical arity instead.
    let mut padded = region.clone();
    padded.lower.resize(N, 0);
    padded.upper.resize(N, Some(0));
    let region = &padded;
    let summary = &policy.regions()[0];
    let extrema = summary
        .extrema()
        .ok_or_else(|| AppError::input("cannot sample an empty entry region"))?;
    let mut output = Vec::with_capacity(3);
    for (kind, group) in [
        (SavedApplicationWitnessKind::Representative, None),
        (
            SavedApplicationWitnessKind::MaximumNumeratorRank,
            Some(false),
        ),
        (
            SavedApplicationWitnessKind::MaximumPositivePower,
            Some(true),
        ),
    ] {
        let mut proposal = region.clone();
        if let Some(active) = group {
            for axis in 0..N {
                if region.support[axis] != active {
                    continue;
                }
                let current = DomainPowerSummary::try_new(
                    proposal.support,
                    &proposal.lower,
                    &proposal.upper,
                    proposal.rank,
                    proposal.powers,
                )
                .map_err(|error| AppError::input(error.to_string()))?;
                let upper = current
                    .extrema()
                    .and_then(|bounds| bounds.upper()[axis])
                    .ok_or_else(|| AppError::input("entry boundary is empty or unbounded"))?;
                proposal.lower[axis] = upper;
                proposal.upper[axis] = Some(upper);
            }
        }
        let witness = pick_entry_intersection_witness(
            region,
            &proposal,
            EntryWitnessLimits {
                max_projections: N.saturating_add(4),
            },
            &AtomicBool::new(false),
        )
        .map_err(|error| AppError::input(error.to_string()))?;
        let EntryWitnessOutcome::Point { key, .. } = witness else {
            return Err(AppError::input("nonempty entry boundary had no witness"));
        };
        let key = IntegralKey::try_new(key.powers()[..physical_arity].iter().copied())
            .map_err(|error| AppError::input(error.to_string()))?;
        policy
            .validate_entry(&key)
            .map_err(|error| AppError::input(error.to_string()))?;
        let measure = |positive: bool| {
            key.powers()
                .iter()
                .filter(|&&power| (power > 0) == positive)
                .map(|power| u128::from(power.unsigned_abs()))
                .sum::<u128>()
        };
        let expected = match kind {
            SavedApplicationWitnessKind::Representative => None,
            SavedApplicationWitnessKind::MaximumNumeratorRank => {
                Some((measure(false), extrema.numerator_rank().1))
            }
            SavedApplicationWitnessKind::MaximumPositivePower => {
                Some((measure(true), extrema.positive_power().1))
            }
        };
        if expected.is_some_and(|(actual, expected)| expected != Some(actual)) {
            return Err(AppError::input(
                "sample failed to attain native aggregate extremum",
            ));
        }
        output.push(SavedApplicationWitness {
            kind,
            integral: key,
        });
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustred::solver::DomainPowerBounds;

    #[test]
    fn correlated_boundaries_are_valid_and_hit_aggregate_maxima() {
        let region = RootRegionInput {
            support: [true, false, true, false],
            lower: vec![0, 0, 0, 0],
            upper: vec![Some(5), Some(4), Some(3), Some(4)],
            rank: Some(5),
            powers: DomainPowerBounds {
                max_positive_power: Some(7),
                min_power_difference: Some(3),
                max_power_difference: Some(3),
            },
        };
        let witnesses = saved_application_boundary_witnesses(&region).unwrap();
        assert_eq!(witnesses.len(), 3);
        for witness in &witnesses {
            assert_eq!(witness.integral.powers().iter().sum::<i64>(), 3);
        }
        assert_eq!(witnesses[1].integral.powers(), &[3, -4, 4, 0]);
    }

    #[test]
    fn small_box_boundaries_match_exhaustive_points() {
        for support in [[true, false, false], [true, true, false], [false; 3]] {
            for rank in 0..=4 {
                for min_d in -3..=3 {
                    let region = RootRegionInput {
                        support,
                        lower: vec![0, 0, 0],
                        upper: vec![Some(2), Some(2), Some(2)],
                        rank: Some(rank),
                        powers: DomainPowerBounds {
                            max_positive_power: Some(5),
                            min_power_difference: Some(min_d),
                            max_power_difference: Some(min_d + 1),
                        },
                    };
                    let mut expected = Vec::new();
                    for a in 0..=2 {
                        for b in 0..=2 {
                            for c in 0..=2 {
                                let powers: Vec<i64> = [a, b, c]
                                    .into_iter()
                                    .zip(support)
                                    .map(|(x, active)| if active { x + 1 } else { -x })
                                    .collect();
                                let positive = powers.iter().filter(|&&n| n > 0).sum::<i64>();
                                let numerator = -powers.iter().filter(|&&n| n < 0).sum::<i64>();
                                let d = positive - numerator;
                                if positive <= 5
                                    && numerator <= i64::from(rank)
                                    && d >= min_d
                                    && d <= min_d + 1
                                {
                                    expected.push((positive, numerator));
                                }
                            }
                        }
                    }
                    if expected.is_empty() {
                        assert!(saved_application_boundary_witnesses(&region).is_err());
                        continue;
                    }
                    let sampled = saved_application_boundary_witnesses(&region).unwrap();
                    let maximum_rank = -sampled[1]
                        .integral
                        .powers()
                        .iter()
                        .filter(|&&n| n < 0)
                        .sum::<i64>();
                    let maximum_positive = sampled[2]
                        .integral
                        .powers()
                        .iter()
                        .filter(|&&n| n > 0)
                        .sum::<i64>();
                    assert_eq!(maximum_rank, expected.iter().map(|x| x.1).max().unwrap());
                    assert_eq!(
                        maximum_positive,
                        expected.iter().map(|x| x.0).max().unwrap()
                    );
                }
            }
        }
    }

    #[cfg(feature = "capacity-dispatch")]
    #[test]
    fn padded_storage_never_leaks_into_witness_keys() {
        let region = RootRegionInput {
            support: [true, false, false, false],
            lower: vec![0, 0, 0],
            upper: vec![Some(3), Some(2), Some(2)],
            rank: Some(2),
            powers: DomainPowerBounds {
                max_positive_power: Some(4),
                min_power_difference: Some(2),
                max_power_difference: Some(2),
            },
        };
        for witness in saved_application_boundary_witnesses(&region).unwrap() {
            assert_eq!(witness.integral.powers().len(), 3);
            assert_eq!(witness.integral.powers().iter().sum::<i64>(), 2);
        }
    }
}
