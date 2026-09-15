//! Consume the exact payload checked by original-source domain replay.
use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::foundry::artifact::{ReplayedOriginalDomain, SourcePortAuditError};
use crate::foundry::cell::{
    FixedIndexRestriction, RuleCellLimits, SourceViewBatch, SourceViewConstruction,
};
use crate::identity::IndexShift;
use crate::sector::{SectorInteriorDomain, SectorMonotoneDomain};

use super::boundary::build_supplied_sector_monotone_admission;
use super::evidence::{CombinedOriginalDomainEvidence, ParametricReplayEvidence};
use super::model::{
    ParametricNonZeroGuard, ParametricRule, ParametricRuleTerm, ParametricSourceRowContribution,
};

impl ParametricRule {
    pub(crate) fn from_replayed_original_domain(
        context: &IndexedCoefficientContext,
        checked: ReplayedOriginalDomain,
    ) -> Result<
        (
            Self,
            Arc<SourceViewBatch>,
            SectorMonotoneDomain,
            Vec<FixedIndexRestriction>,
            RuleCellLimits,
        ),
        SourcePortAuditError,
    > {
        let parts = checked.into_parts();
        let error = |message: String| SourcePortAuditError(message);
        if parts.sources.context_fingerprint() != context.fingerprint()
            || !matches!(parts.sources.construction(), SourceViewConstruction::Direct)
            || parts.source_rows_used == 0
            || parts.source_rows_used != parts.contributions.len()
            || parts.shift_columns_checked == 0
        {
            return Err(error(
                "original replay payload has incompatible source/context/count bindings".into(),
            ));
        }
        let pivot = IndexShift::try_new(vec![0; context.index_count()], context.index_count())
            .map_err(|e| error(e.to_string()))?;
        let domain = SectorInteriorDomain::try_new(
            parts.application.sector().clone(),
            parts.application.bounds().iter().copied(),
        )
        .map_err(|e| error(e.to_string()))?;
        let mut rhs = Vec::with_capacity(parts.rhs.len());
        for (shift, coefficient) in parts.rhs {
            let descent = parts
                .ordering
                .prove_sector_monotone_shift_descent(
                    &parts.application,
                    pivot.values(),
                    shift.values(),
                )
                .map_err(|e| error(e.to_string()))?;
            rhs.push(ParametricRuleTerm::new(shift, coefficient, descent));
        }
        let admission = build_supplied_sector_monotone_admission(
            parts.application.clone(),
            &pivot,
            &rhs,
            parts.ordering,
            parts.limits.rule,
        )
        .map_err(|e| error(e.to_string()))?;
        let source_combination = parts
            .contributions
            .into_iter()
            .map(|(ordinal, row, weight)| {
                ParametricSourceRowContribution::new(ordinal, row, weight)
            })
            .collect();
        let evidence = CombinedOriginalDomainEvidence {
            sector: parts.application.sector().clone(),
            fixed: parts.fixed.clone().into_boxed_slice(),
            application: vec![parts.mathematical_application].into(),
            source_rows_used: parts.source_rows_used,
            shift_columns_checked: parts.shift_columns_checked,
        };
        let rule = Self {
            family_fingerprint: Arc::new(parts.sources.family_fingerprint().to_owned()),
            context_fingerprint: Arc::new(context.fingerprint().to_owned()),
            domain,
            ordering: parts.ordering,
            pivot,
            right_hand_side: rhs,
            pivot_guards: Vec::new(),
            nonzero_guards: parts
                .guards
                .into_iter()
                .map(|(polynomial, origins)| ParametricNonZeroGuard {
                    polynomial,
                    origins,
                })
                .collect(),
            source_combination,
            replay_evidence: ParametricReplayEvidence::CombinedOriginalDomain(Arc::new(evidence)),
            sector_monotone_admission: Some(admission),
        };
        Ok((
            rule,
            parts.sources,
            parts.application,
            parts.fixed,
            parts.limits.cell,
        ))
    }
}
