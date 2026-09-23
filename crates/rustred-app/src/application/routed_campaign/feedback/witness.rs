//! Bounded geometric proposals enter the existing exact feedback path.
//! No point sample discharges its surrounding symbolic region.
use super::{RoutedFeedbackNomination, RoutedFeedbackRoundResult, RoutedFeedbackSession};
use crate::AppError;
use rustred::family::IntegralKey;
use rustred::solver::{
    CandidateOwnerPrograms, EntryWitnessError, EntryWitnessLimits, EntryWitnessOutcome,
    FiniteCasePolicy, OwnerDomainMatchDisposition, OwnerDomainMatchPiece, RootRegionInput,
    pick_entry_intersection_witness,
};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// A typed local-match proposal tied to one immutable program snapshot.
/// Its disposition is diagnostic, never proof of an actual MissingRule.
#[derive(Clone, Debug)]
pub struct RoutedEntryWitnessProposal<const N: usize> {
    programs: Arc<CandidateOwnerPrograms<N>>,
    region: RootRegionInput<N>,
    disposition: OwnerDomainMatchDisposition,
}
impl<const N: usize> RoutedEntryWitnessProposal<N> {
    /// Copy while the native matching callback is alive. The caller supplies
    /// the same programs used for that visit; no Debug/JSON parsing is involved.
    pub fn from_match(
        programs: &Arc<CandidateOwnerPrograms<N>>,
        piece: &OwnerDomainMatchPiece<N>,
    ) -> Result<Self, AppError> {
        let mut lower = Vec::new();
        let mut upper = Vec::new();
        lower
            .try_reserve_exact(piece.lower().len())
            .map_err(|_| AppError::limit("witness proposal lower-bound allocation failed"))?;
        upper
            .try_reserve_exact(piece.upper().len())
            .map_err(|_| AppError::limit("witness proposal upper-bound allocation failed"))?;
        lower.extend_from_slice(piece.lower());
        upper.extend_from_slice(piece.upper());
        Ok(Self {
            programs: programs.clone(),
            region: RootRegionInput {
                support: *piece.owner(),
                lower,
                upper,
                rank: piece.max_numerator_rank(),
                powers: piece.power_bounds(),
            },
            disposition: piece.disposition(),
        })
    }

    pub fn region(&self) -> &RootRegionInput<N> {
        &self.region
    }
    pub fn disposition(&self) -> OwnerDomainMatchDisposition {
        self.disposition
    }
}

/// Aggregate selection allowances. Exhaustion is an error, not a truncated
/// batch implicitly treated as complete. Native trace/search limits still apply.
#[derive(Clone, Copy, Debug)]
pub struct RoutedEntryWitnessLimits {
    pub max_proposals: usize,
    pub max_intersections: usize,
    pub max_projection_calls: usize,
    pub max_roots: usize,
}
impl Default for RoutedEntryWitnessLimits {
    fn default() -> Self {
        Self {
            max_proposals: 128,
            max_intersections: 4096,
            max_projection_calls: 100_000,
            max_roots: 128,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RoutedEntryWitnessStats {
    pub proposals: usize,
    pub intersections: usize,
    pub empty_intersections: usize,
    pub projection_calls: usize,
    pub points: usize,
    pub duplicate_roots: usize,
}

/// One bounded probe/feedback round, not exhaustion of a region or entry union.
#[derive(Clone, Debug)]
pub struct RoutedEntryWitnessRoundResult {
    pub selected_roots: Vec<IntegralKey>,
    pub statistics: RoutedEntryWitnessStats,
    /// None means no root was selected: the previous batch was not retraced.
    pub feedback: Option<RoutedFeedbackRoundResult>,
    /// Always true for this point-proposal interface, even on successful traces.
    pub more_region_work_required: bool,
}

fn check_cancel(cancellation: &AtomicBool) -> Result<(), AppError> {
    if cancellation.load(Ordering::Acquire) {
        Err(AppError::execution("entry-witness selection cancelled"))
    } else {
        Ok(())
    }
}
fn witness_error(error: EntryWitnessError) -> AppError {
    match &error {
        EntryWitnessError::ProjectionAllowance { .. } | EntryWitnessError::IntegralKey(_) => {
            AppError::limit(error.to_string())
        }
        EntryWitnessError::Cancelled => AppError::execution(error.to_string()),
        EntryWitnessError::Invariant(_) => AppError::internal_invariant(error.to_string()),
        EntryWitnessError::Geometry(_) | EntryWitnessError::UnrepresentableIntegralKey => {
            AppError::input(error.to_string())
        }
    }
}

impl<const N: usize> RoutedFeedbackSession<N> {
    /// Intersect typed local-domain proposals with each original finite entry
    /// member separately, then run the existing exact fixed-target feedback.
    ///
    /// Selection is atomic: all proposals, limits and original-policy checks
    /// pass before replacing the old batch. Empty intersections leave the old
    /// batch untouched and never run it accidentally. A stale/foreign snapshot
    /// is rejected; recollect typed proposals after an overlay publication.
    ///
    /// Only the subsequent native trace can nominate an actual MissingRule.
    /// It retains source/pole/descent checks and above-entry descendants. The
    /// typed symbolic disposition does not authorize searches or terminals.
    /// No outer retry loop or region-exhaustion claim is supplied here.
    pub fn run_entry_witness_round(
        &mut self,
        proposals: &[RoutedEntryWitnessProposal<N>],
        limits: RoutedEntryWitnessLimits,
        cancellation: &AtomicBool,
        observer: impl Fn(Value),
    ) -> Result<RoutedEntryWitnessRoundResult, AppError> {
        self.options.validate::<N>()?;
        if self.options.nomination != RoutedFeedbackNomination::FixedTargets
            || self.options.finite_case_policy != FiniteCasePolicy::SearchFinite
        {
            return Err(AppError::input(
                "entry-witness feedback requires FixedTargets and SearchFinite",
            ));
        }
        if limits.max_proposals == 0
            || limits.max_intersections == 0
            || limits.max_projection_calls == 0
            || limits.max_roots == 0
        {
            return Err(AppError::input(
                "entry-witness limits must admit nonzero work",
            ));
        }
        if proposals.len() > limits.max_proposals {
            return Err(AppError::limit("entry-witness proposal allowance exceeded"));
        }
        let original = self.entry_domain.as_ref().ok_or_else(|| {
            AppError::input("entry-witness feedback requires an explicit finite starting domain")
        })?;
        let mut stats = RoutedEntryWitnessStats {
            proposals: proposals.len(),
            ..Default::default()
        };
        let mut roots = Vec::new();
        check_cancel(cancellation)?;
        for proposal in proposals {
            check_cancel(cancellation)?;
            if !Arc::ptr_eq(&proposal.programs, self.programs()) {
                return Err(AppError::input(
                    "entry-witness proposal belongs to a different program snapshot",
                ));
            }
            for region in original.regions() {
                check_cancel(cancellation)?;
                // The native helper validates both inputs before disjointness,
                // including selected supports different from this proposal.
                if stats.intersections >= limits.max_intersections {
                    return Err(AppError::limit(
                        "entry-witness intersection allowance exceeded",
                    ));
                }
                stats.intersections += 1;
                let remaining = limits
                    .max_projection_calls
                    .checked_sub(stats.projection_calls)
                    .ok_or_else(|| {
                        AppError::internal_invariant(
                            "entry-witness projection counter exceeded its allowance",
                        )
                    })?;
                let selected = pick_entry_intersection_witness(
                    region,
                    &proposal.region,
                    EntryWitnessLimits {
                        max_projections: remaining,
                    },
                    cancellation,
                )
                .map_err(witness_error)?;
                let (key, calls) = match selected {
                    EntryWitnessOutcome::Empty { projection_calls } => {
                        stats.empty_intersections += 1;
                        (None, projection_calls)
                    }
                    EntryWitnessOutcome::Point {
                        key,
                        projection_calls,
                    } => {
                        stats.points += 1;
                        (Some(key), projection_calls)
                    }
                };
                stats.projection_calls = stats
                    .projection_calls
                    .checked_add(calls)
                    .filter(|&n| n <= limits.max_projection_calls)
                    .ok_or_else(|| {
                        AppError::internal_invariant("entry-witness projection accounting overflow")
                    })?;
                let Some(key) = key else { continue };
                // No projection rectangle, weaker envelope or serialized JSON
                // is allowed to stand in for the original requested union.
                original.validate(&key)?;
                match roots.binary_search(&key) {
                    Ok(_) => stats.duplicate_roots += 1,
                    Err(index) => {
                        if roots.len() >= limits.max_roots
                            || roots.len() >= self.reducer.limits().max_input_targets
                        {
                            return Err(AppError::limit("entry-witness root allowance exceeded"));
                        }
                        roots
                            .try_reserve_exact(1)
                            .map_err(|_| AppError::limit("entry-witness root allocation failed"))?;
                        roots.insert(index, key);
                    }
                }
            }
        }
        check_cancel(cancellation)?;
        observer(
            json!({"event":"entry_witness_selected", "proposals":stats.proposals,
            "intersections":stats.intersections, "empty_intersections":stats.empty_intersections,
            "projection_calls":stats.projection_calls, "points":stats.points,
            "duplicate_roots":stats.duplicate_roots, "selected_roots":roots.len(),
            "more_region_work_required":true, "family_closure_claim":false,
            "symbolic_disposition_is_missing_rule_authority":false}),
        );
        check_cancel(cancellation)?;
        let feedback = if roots.is_empty() {
            None
        } else {
            self.replace_targets(roots.iter().cloned())?;
            Some(self.run_round(cancellation, observer))
        };
        Ok(RoutedEntryWitnessRoundResult {
            selected_roots: roots,
            statistics: stats,
            feedback,
            more_region_work_required: true,
        })
    }
}

#[cfg(test)]
mod tests;
