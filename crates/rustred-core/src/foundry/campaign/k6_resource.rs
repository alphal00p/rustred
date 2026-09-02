//! K6-specific coherent resource sizing for bounded serial campaigns.
//!
//! A task can admit at most one new exact owner. Therefore a task-report
//! ceiling is also a conservative exact-owner ceiling. The exact cover has
//! several independently enforced owner and quadratic pairing limits; this
//! profile sizes all of those coupled limits together without weakening the
//! global defaults used by other families.

use std::fmt;

use crate::foundry::completion::source_discovery::{
    ExactOwnerContentOrderKey, ExactOwnerCoverDeltaLimits, ProbeCampaignLimits,
    StagedSectorClosureLimits,
};

const K6_ARITY: usize = 6;
/// A smaller run is a diagnostic screen and should fail quickly at the
/// ordinary per-task envelope.  A campaign with enough reports to make a
/// serious closure attempt raises the *coupled* scheduler resources together;
/// widening only the first named stop was measured to expose the next one.
const K6_PROOF_REPORT_FLOOR: usize = 64;
const K6_PROOF_SCHEDULER_MULTIPLIER: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct K6CampaignResourceProfile {
    task_report_ceiling: usize,
    exact: ExactOwnerCoverDeltaLimits,
    probe_campaign: ProbeCampaignLimits,
}

impl K6CampaignResourceProfile {
    /// Size every exact-owner resource coupled to one bounded task-report run.
    ///
    /// Existing defaults remain floors. This keeps small diagnostic campaigns
    /// on the ordinary policy while ensuring a larger task budget cannot run
    /// into the old 4096-owner / `4096^2` pairing ceilings first.
    pub(crate) fn try_for_task_report_ceiling(
        task_report_ceiling: usize,
    ) -> Result<Self, K6CampaignResourceProfileError> {
        if task_report_ceiling == 0 {
            return Err(K6CampaignResourceProfileError::ZeroTaskReportCeiling);
        }
        let pairing_probe_ceiling = checked_mul(
            "K6 exact owner pairing probes",
            task_report_ceiling,
            task_report_ceiling,
        )?;
        let staged_coordinate_cells = checked_mul(
            "K6 staged owner coordinate cells",
            task_report_ceiling,
            K6_ARITY,
        )?;
        let cover_coordinate_cells = checked_mul(
            "K6 exact cover owner endpoint cells",
            staged_coordinate_cells,
            2,
        )?;
        let retained_content_key_bytes = checked_mul(
            "K6 retained compact owner-content key bytes",
            task_report_ceiling,
            std::mem::size_of::<ExactOwnerContentOrderKey>(),
        )?;

        let mut exact = ExactOwnerCoverDeltaLimits::default();
        let candidate_slots = checked_mul(
            "K6 staged executable owner candidate slots",
            task_report_ceiling,
            exact.staged.executable.max_candidates_per_owner,
        )?;

        exact.staged.max_staged_owners = exact.staged.max_staged_owners.max(task_report_ceiling);
        exact.staged.max_staged_owner_coordinate_cells = exact
            .staged
            .max_staged_owner_coordinate_cells
            .max(staged_coordinate_cells);
        exact.staged.max_staged_owner_candidate_slots = exact
            .staged
            .max_staged_owner_candidate_slots
            .max(candidate_slots);
        exact.staged.max_staged_owner_content_key_bytes = exact
            .staged
            .max_staged_owner_content_key_bytes
            .max(retained_content_key_bytes);
        exact.staged.max_owner_order_comparisons = exact
            .staged
            .max_owner_order_comparisons
            .max(pairing_probe_ceiling);
        exact.staged.max_compiled_pairing_probes = exact
            .staged
            .max_compiled_pairing_probes
            .max(pairing_probe_ceiling);

        let executable = &mut exact.staged.executable;
        executable.max_owners = executable.max_owners.max(task_report_ceiling);
        executable.max_pairing_probes = executable.max_pairing_probes.max(pairing_probe_ceiling);
        executable.cover.max_owner_inputs =
            executable.cover.max_owner_inputs.max(task_report_ceiling);
        executable.cover.max_owner_coordinate_cells = executable
            .cover
            .max_owner_coordinate_cells
            .max(cover_coordinate_cells);

        // Every guard-total owner contributes at most one structural box to
        // the exact cover. Keep this geometry preflight coherent with the
        // owner-input ceiling as well.
        let geometry = &mut executable.cover.geometry;
        geometry.max_requested_boxes = geometry.max_requested_boxes.max(task_report_ceiling);
        geometry.max_requested_box_coordinate_cells = geometry
            .max_requested_box_coordinate_cells
            .max(cover_coordinate_cells);

        let mut probe_campaign = ProbeCampaignLimits::default();
        if task_report_ceiling >= K6_PROOF_REPORT_FLOOR {
            let scheduler = &mut probe_campaign.replay.scheduler;
            scheduler.max_aggregate_residual_candidate_work = checked_mul(
                "K6 aggregate residual candidate work",
                scheduler.max_aggregate_residual_candidate_work,
                K6_PROOF_SCHEDULER_MULTIPLIER,
            )?;
            scheduler.max_aggregate_residual_source_term_work = checked_mul(
                "K6 aggregate residual source-term work",
                scheduler.max_aggregate_residual_source_term_work,
                K6_PROOF_SCHEDULER_MULTIPLIER,
            )?;
            scheduler.max_aggregate_prospective_classification_work = checked_mul(
                "K6 aggregate prospective classification work",
                scheduler.max_aggregate_prospective_classification_work,
                K6_PROOF_SCHEDULER_MULTIPLIER,
            )?;
            scheduler.max_aggregate_obstruction_block_candidate_work = checked_mul(
                "K6 aggregate obstruction-block candidate work",
                scheduler.max_aggregate_obstruction_block_candidate_work,
                K6_PROOF_SCHEDULER_MULTIPLIER,
            )?;
            scheduler.max_aggregate_obstruction_block_source_term_work = checked_mul(
                "K6 aggregate obstruction-block source-term work",
                scheduler.max_aggregate_obstruction_block_source_term_work,
                K6_PROOF_SCHEDULER_MULTIPLIER,
            )?;
            scheduler.max_aggregate_obstruction_block_signature_work = checked_mul(
                "K6 aggregate obstruction-block signature work",
                scheduler.max_aggregate_obstruction_block_signature_work,
                K6_PROOF_SCHEDULER_MULTIPLIER,
            )?;
            scheduler.max_aggregate_obstruction_block_selection_work = checked_mul(
                "K6 aggregate obstruction-block selection work",
                scheduler.max_aggregate_obstruction_block_selection_work,
                K6_PROOF_SCHEDULER_MULTIPLIER,
            )?;
        }

        Ok(Self {
            task_report_ceiling,
            exact,
            probe_campaign,
        })
    }

    pub(crate) const fn exact_limits(self) -> ExactOwnerCoverDeltaLimits {
        self.exact
    }

    /// Coherent one-task replay envelope for this campaign tier.
    pub(crate) const fn probe_campaign_limits(self) -> ProbeCampaignLimits {
        self.probe_campaign
    }

    /// Raise the aggregate publication envelope for a same-rank sibling wave.
    ///
    /// Discovery limits apply independently to each ledger, whereas the
    /// consuming publisher accounts all sealed siblings together. Scale every
    /// owner-dependent and compiled-cover aggregate by the wave width so a
    /// cover admitted under this profile cannot be rejected merely because it
    /// is published beside an equally large sibling.
    pub(crate) fn try_raise_publication_limits(
        self,
        mut limits: StagedSectorClosureLimits,
        wave_width: usize,
    ) -> Result<StagedSectorClosureLimits, K6CampaignResourceProfileError> {
        if wave_width == 0 {
            return Err(K6CampaignResourceProfileError::ZeroWaveWidth);
        }
        let exact = self.exact.staged;
        let aggregate_owners = checked_mul(
            "K6 published wave owners",
            self.task_report_ceiling,
            wave_width,
        )?;
        let aggregate_owner_coordinates = checked_mul(
            "K6 published wave owner coordinate cells",
            aggregate_owners,
            K6_ARITY,
        )?;
        let aggregate_candidate_slots = checked_mul(
            "K6 published wave owner candidate slots",
            aggregate_owners,
            exact.executable.max_candidates_per_owner,
        )?;
        let aggregate_content_key_bytes = checked_mul(
            "K6 published wave compact owner-content key bytes",
            aggregate_owners,
            std::mem::size_of::<ExactOwnerContentOrderKey>(),
        )?;
        let aggregate_pairing_probes = checked_mul(
            "K6 published wave exact owner pairing probes",
            checked_mul(
                "K6 exact owner pairing probes",
                self.task_report_ceiling,
                self.task_report_ceiling,
            )?,
            wave_width,
        )?;

        limits.max_sectors = limits.max_sectors.max(wave_width);
        limits.max_frontier_coordinate_cells =
            limits.max_frontier_coordinate_cells.max(checked_mul(
                "K6 published wave frontier coordinate cells",
                wave_width,
                K6_ARITY,
            )?);
        limits.max_staged_owners = limits.max_staged_owners.max(aggregate_owners);
        limits.max_staged_owner_coordinate_cells = limits
            .max_staged_owner_coordinate_cells
            .max(aggregate_owner_coordinates);
        limits.max_staged_owner_candidate_slots = limits
            .max_staged_owner_candidate_slots
            .max(aggregate_candidate_slots);
        limits.max_staged_owner_content_key_bytes = limits
            .max_staged_owner_content_key_bytes
            .max(aggregate_content_key_bytes);
        limits.max_compiled_pairing_probes = limits
            .max_compiled_pairing_probes
            .max(aggregate_pairing_probes);

        // The publisher also sums exact compiler telemetry from the already
        // sealed sibling covers. Give it the corresponding aggregate envelope.
        limits.max_compiled_finite_complement_points = scaled_floor(
            "K6 published finite complement points",
            limits.max_compiled_finite_complement_points,
            exact.max_compiled_finite_complement_points,
            wave_width,
        )?;
        limits.max_compiled_finite_complement_coordinate_cells = scaled_floor(
            "K6 published finite complement coordinate cells",
            limits.max_compiled_finite_complement_coordinate_cells,
            exact.max_compiled_finite_complement_coordinate_cells,
            wave_width,
        )?;
        limits.max_compiled_point_owner_probes = scaled_floor(
            "K6 published point-owner probes",
            limits.max_compiled_point_owner_probes,
            exact.max_compiled_point_owner_probes,
            wave_width,
        )?;
        limits.max_compiled_uncovered_boxes = scaled_floor(
            "K6 published uncovered boxes",
            limits.max_compiled_uncovered_boxes,
            exact.max_compiled_uncovered_boxes,
            wave_width,
        )?;
        limits.max_compiled_uncovered_box_coordinate_cells = scaled_floor(
            "K6 published uncovered-box coordinate cells",
            limits.max_compiled_uncovered_box_coordinate_cells,
            exact.max_compiled_uncovered_box_coordinate_cells,
            wave_width,
        )?;
        limits.max_compiled_split_operations = scaled_floor(
            "K6 published split operations",
            limits.max_compiled_split_operations,
            exact.max_compiled_split_operations,
            wave_width,
        )?;
        Ok(limits)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum K6CampaignResourceProfileError {
    ZeroTaskReportCeiling,
    ZeroWaveWidth,
    ResourceCountOverflow { resource: &'static str },
}

impl fmt::Display for K6CampaignResourceProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroTaskReportCeiling => {
                formatter.write_str("K6 task-report ceiling must be positive")
            }
            Self::ZeroWaveWidth => {
                formatter.write_str("K6 publication wave width must be positive")
            }
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed")
            }
        }
    }
}

impl std::error::Error for K6CampaignResourceProfileError {}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, K6CampaignResourceProfileError> {
    left.checked_mul(right)
        .ok_or(K6CampaignResourceProfileError::ResourceCountOverflow { resource })
}

fn scaled_floor(
    resource: &'static str,
    current: usize,
    per_sector: usize,
    wave_width: usize,
) -> Result<usize, K6CampaignResourceProfileError> {
    Ok(current.max(checked_mul(resource, per_sector, wave_width)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn larger_k6_task_ceiling_raises_every_coupled_exact_owner_limit() {
        let ceiling = 8_192;
        let profile = K6CampaignResourceProfile::try_for_task_report_ceiling(ceiling).unwrap();
        let exact = profile.exact_limits();
        let pairing = ceiling * ceiling;
        let staged_coordinates = ceiling * K6_ARITY;
        let cover_coordinates = staged_coordinates * 2;

        assert!(exact.staged.max_staged_owners >= ceiling);
        assert!(exact.staged.max_staged_owner_coordinate_cells >= staged_coordinates);
        assert!(
            exact.staged.max_staged_owner_candidate_slots
                >= ceiling * exact.staged.executable.max_candidates_per_owner
        );
        assert!(
            exact.staged.max_staged_owner_content_key_bytes
                >= ceiling * std::mem::size_of::<ExactOwnerContentOrderKey>()
        );
        assert!(exact.staged.max_owner_order_comparisons >= pairing);
        assert!(exact.staged.max_compiled_pairing_probes >= pairing);
        assert!(exact.staged.executable.max_owners >= ceiling);
        assert!(exact.staged.executable.max_pairing_probes >= pairing);
        assert!(exact.staged.executable.cover.max_owner_inputs >= ceiling);
        assert!(exact.staged.executable.cover.max_owner_coordinate_cells >= cover_coordinates);
        assert!(exact.staged.executable.cover.geometry.max_requested_boxes >= ceiling);
        assert!(
            exact
                .staged
                .executable
                .cover
                .geometry
                .max_requested_box_coordinate_cells
                >= cover_coordinates
        );

        let publication = profile
            .try_raise_publication_limits(StagedSectorClosureLimits::default(), 2)
            .unwrap();
        assert!(publication.max_staged_owners >= 2 * ceiling);
        assert!(publication.max_staged_owner_coordinate_cells >= 2 * staged_coordinates);
        assert!(
            publication.max_staged_owner_candidate_slots
                >= 2 * ceiling * exact.staged.executable.max_candidates_per_owner
        );
        assert!(publication.max_compiled_pairing_probes >= 2 * pairing);
        assert!(
            publication.max_compiled_uncovered_boxes
                >= 2 * exact.staged.max_compiled_uncovered_boxes
        );

        let defaults = ProbeCampaignLimits::default();
        let proof = profile.probe_campaign_limits();
        assert_eq!(
            proof.replay.scheduler.max_aggregate_residual_candidate_work,
            K6_PROOF_SCHEDULER_MULTIPLIER
                * defaults
                    .replay
                    .scheduler
                    .max_aggregate_residual_candidate_work
        );
        assert_eq!(
            proof
                .replay
                .scheduler
                .max_aggregate_residual_source_term_work,
            K6_PROOF_SCHEDULER_MULTIPLIER
                * defaults
                    .replay
                    .scheduler
                    .max_aggregate_residual_source_term_work
        );
        assert_eq!(
            proof
                .replay
                .scheduler
                .max_aggregate_prospective_classification_work,
            K6_PROOF_SCHEDULER_MULTIPLIER
                * defaults
                    .replay
                    .scheduler
                    .max_aggregate_prospective_classification_work
        );
        assert_eq!(
            proof
                .replay
                .scheduler
                .max_aggregate_obstruction_block_candidate_work,
            K6_PROOF_SCHEDULER_MULTIPLIER
                * defaults
                    .replay
                    .scheduler
                    .max_aggregate_obstruction_block_candidate_work
        );
        assert_eq!(
            proof
                .replay
                .scheduler
                .max_aggregate_obstruction_block_source_term_work,
            K6_PROOF_SCHEDULER_MULTIPLIER
                * defaults
                    .replay
                    .scheduler
                    .max_aggregate_obstruction_block_source_term_work
        );
        assert_eq!(
            proof
                .replay
                .scheduler
                .max_aggregate_obstruction_block_signature_work,
            K6_PROOF_SCHEDULER_MULTIPLIER
                * defaults
                    .replay
                    .scheduler
                    .max_aggregate_obstruction_block_signature_work
        );
        assert_eq!(
            proof
                .replay
                .scheduler
                .max_aggregate_obstruction_block_selection_work,
            K6_PROOF_SCHEDULER_MULTIPLIER
                * defaults
                    .replay
                    .scheduler
                    .max_aggregate_obstruction_block_selection_work
        );
    }

    #[test]
    fn short_k6_screens_keep_the_fast_default_probe_envelope() {
        let profile =
            K6CampaignResourceProfile::try_for_task_report_ceiling(K6_PROOF_REPORT_FLOOR - 1)
                .unwrap();
        assert_eq!(
            profile.probe_campaign_limits(),
            ProbeCampaignLimits::default()
        );
    }

    #[test]
    fn invalid_k6_resource_ceilings_fail_before_mutating_limits() {
        assert_eq!(
            K6CampaignResourceProfile::try_for_task_report_ceiling(0),
            Err(K6CampaignResourceProfileError::ZeroTaskReportCeiling)
        );
        assert!(matches!(
            K6CampaignResourceProfile::try_for_task_report_ceiling(usize::MAX),
            Err(K6CampaignResourceProfileError::ResourceCountOverflow { .. })
        ));
        let profile = K6CampaignResourceProfile::try_for_task_report_ceiling(1).unwrap();
        assert_eq!(
            profile.try_raise_publication_limits(StagedSectorClosureLimits::default(), 0),
            Err(K6CampaignResourceProfileError::ZeroWaveWidth)
        );
    }
}
