//! Owned identity for one independently schedulable campaign work unit.
//!
//! [`CampaignJobKey`](crate::campaign::CampaignJobKey) identifies a static planned
//! family/sector job. A live campaign may schedule more than one independent
//! unit for that job, so runtime resource admission must use the finer
//! [`CampaignWorkKey`]. This key is only deterministic scheduling and retry
//! identity. It does not authenticate mathematical provenance, certify a
//! context, or prove that an epoch is closed.

use std::sync::Arc;

use super::CampaignJobKey;

/// Phase-local discriminator for one campaign work unit.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CampaignWorkUnitKey {
    /// A generic, caller-defined lane within a planned job.
    JobLane { lane_ordinal: u64 },
}

impl CampaignWorkUnitKey {
    pub const fn job_lane(lane_ordinal: u64) -> Self {
        Self::JobLane { lane_ordinal }
    }
}

/// Complete owned scheduling/retry identity for one runtime work unit.
///
/// The context fingerprint is copied into an `Arc<str>` so reservations and
/// resident tokens never borrow an epoch owner. Its spelling is treated as an
/// ordering discriminator only; mathematical context ownership and
/// authentication remain separate contracts.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CampaignWorkKey {
    job: CampaignJobKey,
    context_fingerprint: Arc<str>,
    unit: CampaignWorkUnitKey,
}

impl CampaignWorkKey {
    pub fn new(
        job: CampaignJobKey,
        context_fingerprint: impl Into<Arc<str>>,
        unit: CampaignWorkUnitKey,
    ) -> Self {
        Self {
            job,
            context_fingerprint: context_fingerprint.into(),
            unit,
        }
    }

    pub fn job_lane(
        job: CampaignJobKey,
        context_fingerprint: impl Into<Arc<str>>,
        lane_ordinal: u64,
    ) -> Self {
        Self::new(
            job,
            context_fingerprint,
            CampaignWorkUnitKey::job_lane(lane_ordinal),
        )
    }

    /// Static planned job shared by every runtime unit in that job.
    pub const fn job(&self) -> &CampaignJobKey {
        &self.job
    }

    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }

    pub const fn context_fingerprint_arc(&self) -> &Arc<str> {
        &self.context_fingerprint
    }

    pub const fn unit(&self) -> &CampaignWorkUnitKey {
        &self.unit
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use crate::campaign::{CampaignPlan, CampaignPlanLimits, CampaignRootSpec};
    use crate::{
        AffineDenominator, IntegralFamily, IntegralOrderingPolicy, SectorMask,
        algebra::CoefficientContext,
    };

    use super::{CampaignWorkKey, CampaignWorkUnitKey};

    fn job() -> super::CampaignJobKey {
        let coefficients = CoefficientContext::new(["d"]);
        let family = Arc::new(
            IntegralFamily::new(
                "campaign-work-family",
                vec!["k".to_owned()],
                Vec::new(),
                coefficients.clone(),
                coefficients.parameter("d").unwrap(),
                vec![AffineDenominator::new(
                    coefficients.zero(),
                    vec![coefficients.one()],
                )],
                Vec::new(),
                vec![coefficients.zero()],
            )
            .unwrap(),
        );
        let plan = CampaignPlan::compile(
            vec![
                CampaignRootSpec::try_new(
                    "root",
                    family,
                    SectorMask::try_from_bit_string("1").unwrap(),
                )
                .unwrap(),
            ],
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            CampaignPlanLimits::default(),
        )
        .unwrap();
        plan.intrinsic_jobs().next().unwrap().clone()
    }

    #[test]
    fn same_job_supports_distinct_owned_runtime_units() {
        let job = job();
        let context = "campaign-work-context".to_owned();
        let lane_zero = CampaignWorkKey::job_lane(job.clone(), context.clone(), 0);
        let lane_one = CampaignWorkKey::job_lane(job.clone(), context.clone(), 1);
        let other_context = CampaignWorkKey::job_lane(job.clone(), "other-context", 1);

        let keys = BTreeSet::from([lane_zero.clone(), lane_one.clone(), other_context]);
        assert_eq!(keys.len(), 3);
        assert_eq!(lane_zero.job(), &job);
        assert_eq!(lane_one.job(), &job);
        assert_eq!(lane_one.context_fingerprint(), context);
        assert_eq!(
            lane_one.unit(),
            &CampaignWorkUnitKey::JobLane { lane_ordinal: 1 }
        );
    }
}
