//! Complete bounded seed admission, using the existing Symbolica-backed
//! finite-entry enumerator. This is not a clipped or sampled starting domain.
use super::{Domain, IntegralKey, Outcome, OwnerDomainWalkFiniteReplayLimits};
use crate::{AppError, AppErrorKind, EntryPowerBudget, FiniteEntryDomain};
use rustred::solver::{DomainPowerSummary, FiniteRootAdmission, RootRegionInput};
use serde::Serialize;
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Default, Serialize)]
pub(super) struct Work {
    mode: &'static str,
    count_calls: usize,
    /// Conservative count work admission, not measured binomial calls.
    positive_layer_allowance: usize,
    expected_seed_points: Option<usize>,
    seed_results: usize,
    retained_seed_points: usize,
    seed_buffer_bytes: usize,
    exhausted: bool,
}

pub(super) struct Prepared<const N: usize> {
    pub targets: Vec<IntegralKey>,
    pub admission: FiniteRootAdmission<N>,
    pub work: Work,
}

#[derive(Debug)]
enum Reason {
    Declined(&'static str),
    Hard(String),
    Cancelled,
}

#[derive(Debug)]
pub(super) struct Failure {
    reason: Reason,
    work: Work,
}
impl Failure {
    pub fn outcome(self, cancel: &AtomicBool) -> Outcome {
        let reason = match self.reason {
            Reason::Declined(_) if cancel.load(Ordering::Acquire) => Reason::Cancelled,
            other => other,
        };
        let (status, error, cancelled) = match reason {
            Reason::Declined(status) => (status, None, false),
            Reason::Hard(error) => ("enumeration_error", Some(error), false),
            Reason::Cancelled => (
                "cancelled",
                Some("finite seed admission cancelled".into()),
                true,
            ),
        };
        let work = json!({"status":status,"attempted":false,"enumeration":self.work});
        match error {
            Some(error) => Outcome::Failed {
                error,
                cancelled,
                work,
            },
            None => Outcome::Declined { work },
        }
    }
}

fn failed(work: &Work, reason: Reason) -> Failure {
    Failure {
        reason,
        work: work.clone(),
    }
}
fn hard(work: &Work, error: impl ToString) -> Failure {
    failed(work, Reason::Hard(error.to_string()))
}
fn check(cancel: &AtomicBool, work: &Work) -> Result<(), Failure> {
    if cancel.load(Ordering::Acquire) {
        Err(failed(work, Reason::Cancelled))
    } else {
        Ok(())
    }
}

/// All keys must be native members of a finite set of independently counted
/// cardinality `expected`. The live kernel deduplicates initial keys before
/// setting requested_targets; equality proves the absence of duplicates and omissions.
pub(super) fn complete_cardinality(input: usize, unique: usize, expected: usize) -> bool {
    expected > 0 && input == expected && unique == expected
}

fn buffer_bytes<const N: usize>(count: usize) -> Option<usize> {
    // Buffer payload only. Native BTreeSet/membership storage and temporary
    // composition vectors remain under native node caps and the outer RSS cap.
    count
        .checked_mul(
            std::mem::size_of::<IntegralKey>()
                .checked_add(N.checked_mul(std::mem::size_of::<i64>())?)?,
        )?
        .checked_add(std::mem::size_of::<Vec<IntegralKey>>())
}

fn admit_buffer<const N: usize>(
    count: usize,
    limits: OwnerDomainWalkFiniteReplayLimits,
    native_seed_cap: usize,
    work: &mut Work,
) -> Result<Vec<IntegralKey>, Failure> {
    work.expected_seed_points = Some(count);
    if count == 0 {
        return Err(failed(work, Reason::Declined("empty_capped_domain")));
    }
    if count > native_seed_cap {
        return Err(failed(work, Reason::Declined("aggregate_budget")));
    }
    if count > limits.max_seed_points {
        return Err(failed(work, Reason::Declined("seed_point_budget")));
    }
    let bytes =
        buffer_bytes::<N>(count).ok_or_else(|| hard(work, "seed storage count overflow"))?;
    if bytes > limits.max_seed_bytes {
        return Err(failed(work, Reason::Declined("seed_byte_budget")));
    }
    let mut targets = Vec::new();
    targets
        .try_reserve_exact(count)
        .map_err(|error| hard(work, error))?;
    work.seed_buffer_bytes = bytes;
    Ok(targets)
}

fn collect<const N: usize>(
    mut iterator: impl Iterator<Item = Result<IntegralKey, AppError>>,
    mut targets: Vec<IntegralKey>,
    expected: usize,
    admission: &FiniteRootAdmission<N>,
    cancel: &AtomicBool,
    work: &mut Work,
) -> Result<Vec<IntegralKey>, Failure> {
    loop {
        check(cancel, work)?;
        let item = iterator.next();
        let Some(item) = item else {
            check(cancel, work)?;
            break;
        };
        work.seed_results = work
            .seed_results
            .checked_add(1)
            .ok_or_else(|| hard(work, "seed result count overflow"))?;
        let target = item.map_err(|error| hard(work, error))?;
        check(cancel, work)?;
        if targets.len() >= expected {
            return Err(hard(work, "seed iterator exceeded exact count"));
        }
        admission
            .validate_entry(&target)
            .map_err(|error| hard(work, error))?;
        targets.push(target);
        work.retained_seed_points = targets.len();
    }
    work.exhausted = true;
    if targets.len() != expected {
        return Err(hard(work, "seed iterator exhausted before exact count"));
    }
    Ok(targets)
}

/// A native count is bounded but not internally interruptible. Check before
/// and after it; its explicit layer allowance applies before each binomial.
fn count(
    envelope: &FiniteEntryDomain,
    limits: OwnerDomainWalkFiniteReplayLimits,
    native_seed_cap: usize,
    cancel: &AtomicBool,
    work: &mut Work,
    after_count: impl FnOnce(),
) -> Result<usize, Failure> {
    check(cancel, work)?;
    work.count_calls = 1;
    work.positive_layer_allowance = limits.max_positive_layers;
    let counted = envelope.target_count(limits.max_positive_layers);
    after_count();
    check(cancel, work)?;
    let counted = counted.map_err(|error| {
        if error.kind() == AppErrorKind::Limit {
            failed(work, Reason::Declined("enumeration_layer_budget"))
        } else {
            hard(work, error)
        }
    })?;
    // Huge but valid finite sets are budget declines, not arithmetic errors.
    // Native arbitrary-precision comparisons precede machine-size conversion.
    use symbolica::domains::integer::Integer;
    if counted > Integer::from(native_seed_cap as u64) {
        return Err(failed(work, Reason::Declined("aggregate_budget")));
    }
    if counted > Integer::from(limits.max_seed_points as u64) {
        return Err(failed(work, Reason::Declined("seed_point_budget")));
    }
    usize::try_from(counted).map_err(|error| hard(work, error))
}

pub(super) fn prepare<const N: usize>(
    domain: &Domain<N>,
    limits: OwnerDomainWalkFiniteReplayLimits,
    native_seed_cap: usize,
    cancel: &AtomicBool,
) -> Result<Prepared<N>, Failure> {
    prepare_with_arity(domain, N, limits, native_seed_cap, cancel)
}

pub(super) fn prepare_with_arity<const N: usize>(
    domain: &Domain<N>,
    physical_arity: usize,
    limits: OwnerDomainWalkFiniteReplayLimits,
    native_seed_cap: usize,
    cancel: &AtomicBool,
) -> Result<Prepared<N>, Failure> {
    let mut work = Work::default();
    check(cancel, &work)?;
    let summary = DomainPowerSummary::try_new(
        domain.owner,
        &domain.lower,
        &domain.upper,
        domain.rank,
        domain.powers,
    )
    .map_err(|error| hard(&work, error))?;
    if summary.is_empty() {
        return Err(failed(&work, Reason::Declined("empty_capped_domain")));
    }
    let singleton = domain
        .lower
        .iter()
        .zip(&domain.upper)
        .all(|(&lo, &hi)| hi == Some(lo));
    let envelope = if singleton {
        work.mode = "singleton";
        None
    } else {
        work.mode = "whole_entry_envelope";
        if domain.lower.iter().any(|&value| value != 0) {
            return Err(failed(
                &work,
                Reason::Declined("unsupported_envelope_geometry"),
            ));
        }
        let Some(max_positive_power) = domain
            .powers
            .max_positive_power
            .and_then(|value| u32::try_from(value).ok())
        else {
            return Err(failed(
                &work,
                Reason::Declined("unsupported_envelope_budget"),
            ));
        };
        let Some(max_numerator_rank) = domain.rank else {
            return Err(failed(
                &work,
                Reason::Declined("unsupported_envelope_budget"),
            ));
        };
        let convert = |bound: Option<i64>| bound.map(i32::try_from).transpose();
        let (Ok(min_power_difference), Ok(max_power_difference)) = (
            convert(domain.powers.min_power_difference),
            convert(domain.powers.max_power_difference),
        ) else {
            return Err(failed(
                &work,
                Reason::Declined("unsupported_envelope_budget"),
            ));
        };
        // Do NOT intersect the envelope with the original coordinate box and
        // thereby silently narrow it. Authenticate equality of both full sets.
        let full = DomainPowerSummary::try_new(
            domain.owner,
            &[0; N],
            &std::array::from_fn::<_, N, _>(|axis| (axis >= physical_arity).then_some(0)),
            domain.rank,
            domain.powers,
        )
        .map_err(|error| hard(&work, error))?;
        if !summary.contains(&full) || !full.contains(&summary) {
            return Err(failed(
                &work,
                Reason::Declined("unsupported_envelope_geometry"),
            ));
        }
        Some(
            FiniteEntryDomain::new(
                domain.owner[..physical_arity].to_vec(),
                EntryPowerBudget {
                    max_positive_power,
                    max_numerator_rank,
                    min_power_difference,
                    max_power_difference,
                },
            )
            .map_err(|error| hard(&work, error))?,
        )
    };
    let admission = FiniteRootAdmission::try_new(
        [RootRegionInput {
            support: domain.owner,
            lower: domain.lower.clone(),
            upper: domain.upper.clone(),
            rank: domain.rank,
            powers: domain.powers,
        }],
        1,
    )
    .map_err(|error| hard(&work, error))?;
    let expected = match &envelope {
        Some(envelope) => count(envelope, limits, native_seed_cap, cancel, &mut work, || {})?,
        None => 1,
    };
    let targets = admit_buffer::<N>(expected, limits, native_seed_cap, &mut work)?;
    let targets = if let Some(envelope) = &envelope {
        collect(
            envelope.targets(),
            targets,
            expected,
            &admission,
            cancel,
            &mut work,
        )?
    } else {
        let point = super::point(domain, physical_arity)
            .map_err(|error| hard(&work, error))?
            .ok_or_else(|| hard(&work, "singleton shape changed during admission"))?;
        collect(
            std::iter::once(Ok(point)),
            targets,
            expected,
            &admission,
            cancel,
            &mut work,
        )?
    };
    Ok(Prepared {
        targets,
        admission,
        work,
    })
}
