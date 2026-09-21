use rustred::family::IntegralKey;
use rustred::solver::{CandidateRoutedFrontierReason, CandidateRoutedTraceReport, CoordinateCase};
use std::sync::atomic::{AtomicBool, Ordering};

/// Exact native coordinate-ray identity under one session's fixed source policy.
/// Active powers stay symbolic; only inactive coordinates are stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Ray<const N: usize> {
    pub owner: [bool; N],
    pub fixed: [Option<i16>; N],
    pub rank: u32,
}

impl<const N: usize> Ray<N> {
    pub fn from_target(owner: [bool; N], target: &IntegralKey) -> Result<Self, &'static str> {
        if target.powers().len() != N {
            return Err("frontier target arity differs from owner");
        }
        let mut fixed = [None; N];
        let mut rank = 0u32;
        for (axis, (&active, &power)) in owner.iter().zip(target.powers()).enumerate() {
            if (power > 0) != active {
                return Err("MissingRule target support differs from its owner");
            }
            if !active {
                let degree = u32::try_from(power.unsigned_abs())
                    .map_err(|_| "actual successor numerator rank exceeds u32")?;
                rank = rank.checked_add(degree).ok_or("successor rank overflow")?;
                fixed[axis] = Some(i16::try_from(power).map_err(
                    |_| "fixed inactive coordinate is outside native compact admission",
                )?);
            }
        }
        CoordinateCase::new(fixed)
            .map_err(|_| "fixed inactive coordinate is outside native compact admission")?;
        Ok(Self { owner, fixed, rank })
    }

    pub fn case(&self) -> CoordinateCase<N> {
        CoordinateCase::new(self.fixed).expect("nomination admitted native fixed powers")
    }
}

pub(super) struct Nominations<const N: usize> {
    pub jobs: Vec<Ray<N>>,
    pub duplicate_entries: usize,
    pub already_installed_entries: usize,
    /// First reason why the whole observed frontier could not be nominated.
    /// Previously admitted jobs can still make progress; no ignored master.
    pub incomplete: Option<(&'static str, Vec<i64>)>,
}

pub(super) fn nominate<const N: usize>(
    trace: &CandidateRoutedTraceReport<N>,
    installed: &[Ray<N>],
    max_jobs: usize,
    max_bytes: usize,
    cancellation: &AtomicBool,
) -> Nominations<N> {
    let mut out = Nominations {
        jobs: Vec::new(),
        duplicate_entries: 0,
        already_installed_entries: 0,
        incomplete: None,
    };
    for item in trace.frontier() {
        if cancellation.load(Ordering::Relaxed) {
            out.incomplete = Some(("cancelled during nomination", Vec::new()));
            break;
        }
        let CandidateRoutedFrontierReason::MissingRule { owner_sector } = item.reason else {
            continue;
        };
        let ray = match Ray::from_target(owner_sector, &item.target) {
            Ok(ray) => ray,
            Err(reason) => {
                out.incomplete = Some((reason, item.target.powers().to_vec()));
                break;
            }
        };
        if installed.contains(&ray) {
            out.already_installed_entries += 1;
            continue;
        }
        if out.jobs.contains(&ray) {
            out.duplicate_entries += 1;
            continue;
        }
        let next = out.jobs.len().checked_add(1);
        let bytes = next.and_then(|n| n.checked_mul(std::mem::size_of::<Ray<N>>()));
        if next.is_none_or(|n| n > max_jobs) || bytes.is_none_or(|n| n > max_bytes) {
            out.incomplete = Some(("nomination resource limit", item.target.powers().to_vec()));
            break;
        }
        if out.jobs.try_reserve_exact(1).is_err() {
            out.incomplete = Some((
                "nomination allocation failed",
                item.target.powers().to_vec(),
            ));
            break;
        }
        out.jobs.push(ray);
    }
    // Native frontier iteration and this compact exact key make selection and
    // publication deterministic. No Debug-string identity or domain algebra.
    out.jobs.sort();
    out
}
