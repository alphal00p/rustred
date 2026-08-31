//! Deterministic bounded compilation of an exhaustive residual census into a
//! non-authoritative next-epoch proposal batch.

use crate::foundry::completion::source_discovery::{
    NonzeroIncidentTranslationResiduals, ResidualProposalScore,
};
use crate::identity::TranslatedSourceRequest;

use super::super::super::ProbeLocalSchedulerError;
use super::super::budget::try_vec;

const RESIDUAL_PROPOSAL_ORDER: &str = "probe-local ranked residual proposal ordinals";
const RESIDUAL_PROPOSALS: &str = "probe-local ranked residual proposals";

/// Rank a bounded proposal batch without changing the exhaustive residual
/// census which authorized the candidate pool.
///
/// Preference is structural and total: minimize newly introduced forbidden
/// shifts, then all newly introduced physical shifts, maximize contact with
/// the current obstruction support, minimize exact row width, and finally use
/// the canonical request identity. None of these heuristics carries
/// mathematical authority; a later epoch may nominate any request omitted
/// here again.
pub(super) fn try_rank_residual_proposals(
    residuals: &NonzeroIncidentTranslationResiduals,
    maximum: usize,
) -> Result<Vec<TranslatedSourceRequest>, ProbeLocalSchedulerError> {
    let order = try_rank_residual_proposal_ordinals(
        residuals.requests(),
        residuals.proposal_scores(),
        maximum,
    )?;
    let mut proposals = try_vec(RESIDUAL_PROPOSALS, order.len())?;
    proposals.extend(
        order
            .into_iter()
            .map(|ordinal| residuals.requests()[ordinal].clone()),
    );
    Ok(proposals)
}

fn try_rank_residual_proposal_ordinals(
    requests: &[TranslatedSourceRequest],
    scores: &[ResidualProposalScore],
    maximum: usize,
) -> Result<Vec<usize>, ProbeLocalSchedulerError> {
    if requests.len() != scores.len() {
        return Err(ProbeLocalSchedulerError::Invariant {
            detail: "nonzero residual requests and proposal scores have different cardinalities",
        });
    }
    let retained = requests.len().min(maximum);
    let mut order = try_vec(RESIDUAL_PROPOSAL_ORDER, requests.len())?;
    order.extend(0..requests.len());
    order.sort_unstable_by(|&left, &right| {
        scores[left]
            .compare_proposal_preference(scores[right])
            .then_with(|| requests[left].cmp(&requests[right]))
    });
    order.truncate(retained);
    Ok(order)
}

#[cfg(test)]
mod tests {
    use crate::foundry::completion::source_discovery::ResidualProposalScore;
    use crate::identity::{IntegralShift, TranslatedSourceRequest};

    use super::{ProbeLocalSchedulerError, try_rank_residual_proposal_ordinals};

    fn request(offset: i64) -> TranslatedSourceRequest {
        TranslatedSourceRequest::new(0, IntegralShift::try_new([offset]).unwrap())
    }

    fn ranked_requests(
        requests: &[TranslatedSourceRequest],
        scores: &[ResidualProposalScore],
        maximum: usize,
    ) -> Vec<TranslatedSourceRequest> {
        try_rank_residual_proposal_ordinals(requests, scores, maximum)
            .unwrap()
            .into_iter()
            .map(|ordinal| requests[ordinal].clone())
            .collect()
    }

    #[test]
    fn proposal_order_uses_every_score_level_and_is_input_order_independent() {
        let new_forbidden = request(0);
        let new_allowed = request(1);
        let low_overlap = request(2);
        let wider = request(3);
        let canonical_first = request(4);
        let canonical_second = request(5);
        let score_new_forbidden = ResidualProposalScore::new(1, 1, 100, 1);
        let score_new_allowed = ResidualProposalScore::new(0, 2, 100, 1);
        let score_low_overlap = ResidualProposalScore::new(0, 0, 1, 1);
        let score_wider = ResidualProposalScore::new(0, 0, 2, 9);
        let score_narrow = ResidualProposalScore::new(0, 0, 2, 5);

        let first_requests = vec![
            canonical_second.clone(),
            low_overlap.clone(),
            new_forbidden.clone(),
            new_allowed.clone(),
            canonical_first.clone(),
            wider.clone(),
        ];
        let first_scores = vec![
            score_narrow,
            score_low_overlap,
            score_new_forbidden,
            score_new_allowed,
            score_narrow,
            score_wider,
        ];
        let expected = vec![
            canonical_first.clone(),
            canonical_second.clone(),
            wider.clone(),
            low_overlap.clone(),
            new_allowed.clone(),
            new_forbidden.clone(),
        ];
        assert_eq!(
            ranked_requests(&first_requests, &first_scores, usize::MAX),
            expected
        );
        assert_eq!(
            ranked_requests(&first_requests, &first_scores, 3),
            expected[..3]
        );

        let permuted_requests = vec![
            wider,
            new_forbidden,
            canonical_second,
            low_overlap,
            new_allowed,
            canonical_first,
        ];
        let permuted_scores = vec![
            score_wider,
            score_new_forbidden,
            score_narrow,
            score_low_overlap,
            score_new_allowed,
            score_narrow,
        ];
        assert_eq!(
            ranked_requests(&permuted_requests, &permuted_scores, usize::MAX),
            expected
        );
        assert!(matches!(
            try_rank_residual_proposal_ordinals(&permuted_requests, &permuted_scores[..4], 3),
            Err(ProbeLocalSchedulerError::Invariant { .. })
        ));
    }
}
