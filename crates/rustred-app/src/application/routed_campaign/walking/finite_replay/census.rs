//! Completed native identities for publication; no algebra or diagnostic trust.
use rustred::family::IntegralKey;
use rustred::solver::{
    CandidateRoutedCampaignReport, CandidateRoutedTraceCensus, CandidateRoutedTraceCensusLimits,
};
use std::collections::BTreeSet;

#[derive(Clone, Copy)]
pub(in crate::application::routed_campaign::walking) struct CensusLimits {
    pub rules: CandidateRoutedTraceCensusLimits,
    pub max_terminal_keys: usize,
}

pub(in crate::application::routed_campaign::walking) struct CompletedCensus {
    pub rules: CandidateRoutedTraceCensus,
    pub terminals: BTreeSet<IntegralKey>,
    pub zero_keys: usize,
}
impl CompletedCensus {
    pub(super) fn from_report<const N: usize>(
        report: &CandidateRoutedCampaignReport<N>,
        limits: CensusLimits,
    ) -> Result<Self, String> {
        let rules = report
            .complete_census()
            .ok_or("finite replay did not supply a complete native census")?;
        let applications = rules
            .selections()
            .values()
            .try_fold(0u64, |sum, &count| sum.checked_add(count))
            .ok_or("finite census application multiplicity overflow")?;
        if u64::try_from(report.snapshot().rule_applications).ok() != Some(applications) {
            return Err(
                "finite census identities do not account for every successful native application"
                    .into(),
            );
        }
        if report.trace().declared_terminals().len() > limits.max_terminal_keys {
            return Err(
                "finite replay terminal census exceeds its explicit inventory limit".into(),
            );
        }
        Ok(Self {
            rules: rules.clone(),
            terminals: report.trace().declared_terminals().clone(),
            zero_keys: report.trace().visited_zeros().len(),
        })
    }
}
