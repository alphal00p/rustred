//! Explicit finite terminal normalization using the existing exact native plan.
//! Structural inspection stays lazy; this operation alone performs algebra.

use super::{
    CandidateArtifact, CandidateArtifactPage, CandidateCoefficientInspection, codec, order, view,
};
use crate::AppError;
use rustred::family::{IntegralFamily, IntegralKey};
use rustred::persistence::BinaryIoLimits;
use rustred::reduction::terminal_normalization::{
    TerminalNormalizationError, TerminalNormalizationPlan,
};
use rustred::sector::OrderingPolicy;
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub use rustred::reduction::terminal_normalization::TerminalNormalizationLimits as CandidateTerminalNormalizationLimits;

pub struct CandidateTerminalNormalization {
    creator_pid: u32,
    plan: TerminalNormalizationPlan,
    raw_records: usize,
    io: BinaryIoLimits,
}

impl CandidateArtifact {
    /// Verify against the caller's existing family and normalize all declared
    /// residual keys. No rule coefficient is imported or deserialized. This
    /// does not install a rule, consult a value catalog, or prove closure.
    pub fn normalize_terminals(
        &self,
        family: &IntegralFamily,
        limits: CandidateTerminalNormalizationLimits,
    ) -> Result<CandidateTerminalNormalization, AppError> {
        self.check_process()?;
        if family.fingerprint() != self.records.family_fingerprint
            || family.denominator_count() != self.records.root_sector.len()
        {
            return Err(AppError::input(
                "terminal normalization family does not match candidate artifact",
            ));
        }
        let raw_records = self
            .records
            .sectors
            .iter()
            .map(|s| s.finite_residuals.len())
            .sum::<usize>();
        if raw_records > limits.max_terminals {
            return Err(AppError::limit(format!(
                "terminal records require {raw_records}; limit is {}",
                limits.max_terminals
            )));
        }
        let raw = self
            .records
            .sectors
            .iter()
            .flat_map(|s| &s.finite_residuals)
            .map(|key| {
                IntegralKey::try_new(key.values.iter().copied().map(i64::from))
                    .map_err(|e| AppError::input(e.to_string()))
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        let mut normalized = CandidateTerminalNormalization::from_keys(
            family,
            &raw,
            order::saved_policy(&self.records)?,
            limits,
        )?;
        normalized.raw_records = raw_records;
        normalized.io = self.limits.binary_limits();
        Ok(normalized)
    }
}

impl CandidateTerminalNormalization {
    /// Normalize an explicitly selected finite set in one existing family.
    ///
    /// The caller binds the keys to that family (for example, through an
    /// authenticated common-family owner registry). This constructor neither
    /// imports owner programs nor broadens the selected set to all saved
    /// terminals. Arity and algebraic admissibility are checked by the existing
    /// exact native plan; unsupported keys remain declared outputs.
    ///
    /// `ordering` is an output convention for selecting representatives, not
    /// the owners' execution order. `SpiredUncutV1` provides a stable generic
    /// convention when selected owners use different execution orders. No
    /// equality across distinct family fingerprints, master minimality,
    /// general product decomposition or closure is claimed. Sidecar encoding
    /// uses the ordinary bounded binary-I/O defaults.
    pub fn from_keys(
        family: &IntegralFamily,
        keys: &BTreeSet<IntegralKey>,
        ordering: OrderingPolicy,
        limits: CandidateTerminalNormalizationLimits,
    ) -> Result<Self, AppError> {
        let plan =
            TerminalNormalizationPlan::vacuum_quadratic_numerators(family, keys, ordering, limits)
                .map_err(|e| match e {
                    TerminalNormalizationError::Limit { .. } => AppError::limit(e.to_string()),
                    _ => AppError::execution(e.to_string()),
                })?;
        Ok(Self {
            creator_pid: std::process::id(),
            plan,
            raw_records: keys.len(),
            io: BinaryIoLimits::default(),
        })
    }

    pub fn check_process(&self) -> Result<(), AppError> {
        if self.creator_pid != std::process::id() {
            return Err(AppError::execution(
                "terminal normalization cannot be reused after fork; recreate in a fresh process",
            ));
        }
        Ok(())
    }
    pub fn metadata(&self) -> Result<Value, AppError> {
        self.check_process()?;
        let stats = self.plan.statistics();
        Ok(json!({
            "schema":"rustred.terminal-normalization-view.v1",
            "family_fingerprint":self.plan.family_fingerprint(),
            "integral_order":self.plan.ordering().stable_id().to_string(),
            "raw_terminal_records":self.raw_records,
            "unique_raw_terminals":stats.raw_terminals,
            // positive_aliases() later includes provisional projection outputs;
            // only the original raw aliases define this intermediate count.
            "after_unit_aliases":stats.raw_terminals-stats.unit_aliases,
            "canonical_terminals":stats.canonical_terminals,
            "unit_aliases":stats.unit_aliases,
            "projected_numerators":stats.projected_numerators,
            "verified_generators":stats.verified_generators,
            "analyzed_supports":stats.analyzed_supports,
            "zero_certificates":self.plan.zero_certificates().len(),
            "total_relations":self.plan.terms().len(),
            "total_coefficients":self.plan.terms().values().map(|row|row.len()).sum::<usize>(),
            "skipped":stats.skipped.iter().map(|(reason,count)|json!({"reason":format!("{reason:?}"),"count":count})).collect::<Vec<_>>(),
            "exact_within_family":true,"closure_claim":false,"master_minimality_claim":false,
            "original_ibp_source_replay_claim":false,
            "authority":"native verified U/permutation and quadratic-numerator identities; unsupported shapes retained; no external master catalog",
        }))
    }
    pub fn terminals(
        &self,
        start: usize,
        limit: usize,
    ) -> Result<CandidateArtifactPage<Vec<i64>>, AppError> {
        self.check_process()?;
        let total = self.plan.canonical_terminals().len();
        let range = view::page(total, start, limit)?;
        Ok(CandidateArtifactPage {
            total,
            start,
            items: self
                .plan
                .canonical_terminals()
                .iter()
                .skip(range.start)
                .take(range.len())
                .map(|key| key.powers().to_vec())
                .collect(),
        })
    }
    pub fn relations(
        &self,
        start: usize,
        limit: usize,
    ) -> Result<CandidateArtifactPage<Value>, AppError> {
        self.check_process()?;
        let total = self.plan.terms().len();
        let range = view::page(total, start, limit)?;
        Ok(CandidateArtifactPage { total, start, items:self.plan.terms().iter().enumerate()
            .skip(range.start).take(range.len()).map(|(ordinal,(key,row))|
                json!({"ordinal":ordinal,"integral":key.powers(),"terms":row.len()})).collect() })
    }
    pub fn relation(&self, ordinal: usize, max_output_bytes: usize) -> Result<Value, AppError> {
        self.check_process()?;
        view::output_budget(max_output_bytes)?;
        let (key, row) = self
            .plan
            .terms()
            .iter()
            .nth(ordinal)
            .ok_or_else(|| AppError::input("terminal relation ordinal is outside plan"))?;
        // Even the shortest JSON power vector needs one digit and a separator
        // per coordinate. Reject a known-too-large selected row before cloning
        // keys into its JSON view; final encoding is checked separately.
        let minimum = row
            .len()
            .checked_mul(key.powers().len().saturating_mul(2).saturating_add(40))
            .ok_or_else(|| AppError::limit("terminal relation detail size overflow"))?;
        if minimum > max_output_bytes {
            return Err(AppError::limit(
                "terminal relation exceeds detail output budget",
            ));
        }
        let first_id = self
            .plan
            .terms()
            .values()
            .take(ordinal)
            .map(|row| row.len())
            .sum::<usize>();
        let result = json!({"ordinal":ordinal,"integral":key.powers(),
            "rhs":row.keys().enumerate().map(|(index,target)| json!({"integral":target.powers(),"coefficient_id":first_id+index})).collect::<Vec<_>>(),
            "coefficient_ids":"local to this normalization result; not candidate artifact IDs"});
        view::bounded(&result, max_output_bytes)?;
        Ok(result)
    }
    pub fn coefficient(
        &self,
        id: usize,
        max_output_bytes: usize,
    ) -> Result<CandidateCoefficientInspection, AppError> {
        self.check_process()?;
        let coefficient = self
            .plan
            .terms()
            .values()
            .flat_map(|row| row.values())
            .nth(id)
            .ok_or_else(|| AppError::input("normalization coefficient ID is outside plan"))?;
        view::coefficient_view(coefficient, id, max_output_bytes)
    }
    /// Exact native sidecar, bounded by the caller's retained transport policy.
    /// Loading it through decode_generated independently rebuilds the plan.
    pub fn sidecar(&self) -> Result<Vec<u8>, AppError> {
        self.check_process()?;
        self.plan
            .encode_native(self.io)
            .map_err(codec::binary_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_loop_family() -> IntegralFamily {
        let source = r#"schema = "rustred.project.toml.v1"
[family]
name = "selected_terminal_normalization"
loop_momenta = ["k0","k1"]
external_momenta = []
dimension = "d"
[[family.denominators]]
id="a"
expression="k0^2-1"
[[family.denominators]]
id="b"
expression="k1^2-1"
[[family.denominators]]
id="c"
expression="(k0+k1)^2-1"
[target]
powers=[1,1,1]
"#;
        super::super::preparation::family(source, crate::InputFormat::Auto).unwrap()
    }

    #[test]
    fn selected_keys_normalize_together_without_owner_or_catalog_expansion() {
        let family = two_loop_family();
        let raw = [
            [1, 1, 0],
            [1, 0, 1],
            [0, 1, 1],
            [2, 1, 1],
            [1, 2, 1],
            [1, 1, 2],
        ]
        .into_iter()
        .map(|powers| IntegralKey::try_new(powers).unwrap())
        .collect::<BTreeSet<_>>();
        let result = CandidateTerminalNormalization::from_keys(
            &family,
            &raw,
            OrderingPolicy::SpiredUncutV1,
            Default::default(),
        )
        .unwrap();
        let metadata = result.metadata().unwrap();
        assert_eq!(metadata["raw_terminal_records"], 6);
        assert_eq!(metadata["unique_raw_terminals"], 6);
        assert_eq!(metadata["unit_aliases"], 4);
        assert_eq!(metadata["canonical_terminals"], 2);
        assert_eq!(metadata["master_minimality_claim"], false);
        assert_eq!(metadata["closure_claim"], false);
        assert!(result.plan.canonical_terminals().is_subset(&raw));
        assert_eq!(result.plan.terms().len(), raw.len());
        assert!(result.plan.terms().values().all(|terms| {
            terms.len() == 1
                && terms
                    .values()
                    .all(|coefficient| coefficient == &family.coefficient_context().one())
        }));
        assert_eq!(result.terminals(0, 1).unwrap().total, 2);
        assert_eq!(result.terminals(0, 1).unwrap().items.len(), 1);
    }

    #[test]
    fn selected_keys_retain_unsupported_numerators_and_accept_empty_inventory() {
        let family = two_loop_family();
        let key = IntegralKey::try_new([1, 1, -2]).unwrap();
        let result = CandidateTerminalNormalization::from_keys(
            &family,
            &BTreeSet::from([key.clone()]),
            OrderingPolicy::SpiredUncutV1,
            Default::default(),
        )
        .unwrap();
        assert_eq!(result.plan.canonical_terminals(), &BTreeSet::from([key]));
        let empty = CandidateTerminalNormalization::from_keys(
            &family,
            &BTreeSet::new(),
            OrderingPolicy::SpiredUncutV1,
            Default::default(),
        )
        .unwrap();
        assert_eq!(empty.metadata().unwrap()["unique_raw_terminals"], 0);
        assert_eq!(empty.metadata().unwrap()["canonical_terminals"], 0);
    }

    #[test]
    fn selected_keys_reject_foreign_arity_and_honor_preparation_limits() {
        let family = two_loop_family();
        let wrong = BTreeSet::from([IntegralKey::try_new([1, 1]).unwrap()]);
        let error = CandidateTerminalNormalization::from_keys(
            &family,
            &wrong,
            OrderingPolicy::SpiredUncutV1,
            Default::default(),
        );
        assert!(error.is_err());
        let raw = BTreeSet::from([IntegralKey::try_new([1, 1, 0]).unwrap()]);
        let error = CandidateTerminalNormalization::from_keys(
            &family,
            &raw,
            OrderingPolicy::SpiredUncutV1,
            CandidateTerminalNormalizationLimits {
                max_terminals: 0,
                ..Default::default()
            },
        );
        assert!(error.is_err());
    }

    #[test]
    fn alias_stage_count_does_not_include_unbound_projection_outputs() {
        let source = r#"schema = "rustred.project.toml.v1"
[family]
name = "normalization_count"
loop_momenta = ["k0","k1","k2"]
external_momenta = []
dimension = "d"
[[family.denominators]]
id="a"
expression="k0^2-1"
[[family.denominators]]
id="b"
expression="k1^2-1"
[[family.denominators]]
id="c"
expression="k2^2-1"
[[family.denominators]]
id="den_d"
expression="(k0+k1+k2)^2-1"
[[family.denominators]]
id="e"
expression="(k0+k2)^2-1"
[[family.denominators]]
id="f"
expression="(k1+k2)^2-1"
[target]
powers=[1,1,1,1,-1,0]
"#;
        let family = super::super::preparation::family(source, crate::InputFormat::Auto).unwrap();
        let raw = BTreeSet::from([IntegralKey::try_new([1, 1, 1, 1, -1, 0]).unwrap()]);
        let plan = TerminalNormalizationPlan::vacuum_quadratic_numerators(
            &family,
            &raw,
            rustred::sector::OrderingPolicy::SpiredUncutV1,
            Default::default(),
        )
        .unwrap();
        assert!(plan.positive_aliases().raw_terminals().len() > raw.len());
        let result = CandidateTerminalNormalization {
            creator_pid: std::process::id(),
            plan,
            raw_records: 1,
            io: Default::default(),
        };
        assert_eq!(result.metadata().unwrap()["after_unit_aliases"], 1);
        assert_eq!(result.metadata().unwrap()["canonical_terminals"], 1);
    }
}
