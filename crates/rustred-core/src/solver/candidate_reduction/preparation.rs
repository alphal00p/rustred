pub(super) mod shared;

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::family::IntegralFamily;
use crate::reduction::{CacheWeight, ReductionLimits, ReductionStatistics, SharedCacheBudget};
use crate::sector::{OrderingPolicy, zero};
use crate::solver::{FiniteCasePolicy, SectorSolution};

use super::model::{CandidateCacheRepresentation, CandidateReductionError};
use super::reducer::CandidateReducer;

impl<const N: usize> CandidateReducer<N> {
    /// Prepare an explicitly experimental, owned formula applier. The family
    /// admission is the same unit-mass vacuum shape check as publication, but
    /// this constructor does NOT perform original-source replay or cover proof.
    /// `finite_residuals` must contain only concrete fixed integral keys.
    pub fn try_new(
        family: &IntegralFamily,
        root_sector: [bool; N],
        ordering: OrderingPolicy,
        sectors: impl IntoIterator<Item = ([bool; N], SectorSolution<N>)>,
        zero_certificates: Vec<zero::Certificate>,
        limits: ReductionLimits,
    ) -> Result<Self, CandidateReductionError> {
        Self::prepare(
            family,
            root_sector,
            ordering,
            sectors,
            zero_certificates,
            limits,
            None,
        )
    }

    /// Prepare with an explicit entry-domain scope, including when no nonzero
    /// sector records exist. Every supplied solution must have exactly this
    /// scope. This is metadata/admission only, not a coverage certificate.
    pub fn try_new_with_numerator_rank(
        family: &IntegralFamily,
        root_sector: [bool; N],
        ordering: OrderingPolicy,
        sectors: impl IntoIterator<Item = ([bool; N], SectorSolution<N>)>,
        zero_certificates: Vec<zero::Certificate>,
        limits: ReductionLimits,
        max_numerator_rank: Option<u32>,
    ) -> Result<Self, CandidateReductionError> {
        Self::prepare(
            family,
            root_sector,
            ordering,
            sectors,
            zero_certificates,
            limits,
            Some(max_numerator_rank),
        )
    }

    fn prepare(
        family: &IntegralFamily,
        root_sector: [bool; N],
        ordering: OrderingPolicy,
        sectors: impl IntoIterator<Item = ([bool; N], SectorSolution<N>)>,
        zero_certificates: Vec<zero::Certificate>,
        limits: ReductionLimits,
        mut scope: Option<Option<u32>>,
    ) -> Result<Self, CandidateReductionError> {
        // Preserve validation of an ordering even for an empty/zero request.
        ordering
            .compare(&[0; N], &[0; N])
            .map_err(crate::reduction::ReductionError::Ordering)?;
        let shared = shared::prepare_family(family, zero_certificates, limits)?;
        let mut records = BTreeMap::new();
        let mut finite_case_policy = None;
        for (sector, solution) in sectors {
            if solution.finite_case_policy == FiniteCasePolicy::RetainRankFinite
                && solution.max_numerator_rank.is_none()
            {
                return Err(CandidateReductionError::InvalidInput(
                    "retain-rank-finite candidate solution requires an explicit numerator rank"
                        .into(),
                ));
            }
            if let Some(expected) = finite_case_policy {
                if solution.finite_case_policy != expected {
                    return Err(CandidateReductionError::InconsistentFiniteCasePolicy {
                        expected,
                        actual: solution.finite_case_policy,
                    });
                }
            } else {
                finite_case_policy = Some(solution.finite_case_policy);
            }
            if let Some(expected) = scope {
                if solution.max_numerator_rank != expected {
                    return Err(CandidateReductionError::InconsistentNumeratorRank {
                        expected,
                        actual: solution.max_numerator_rank,
                    });
                }
            } else {
                scope = Some(solution.max_numerator_rank);
            }
            if sector.iter().zip(root_sector).any(|(&s, root)| s && !root) {
                return Err(CandidateReductionError::InvalidInput(
                    "candidate sector lies outside the supplied root".into(),
                ));
            }
            if records.insert(sector, solution).is_some() {
                return Err(CandidateReductionError::InvalidInput(
                    "duplicate candidate sector record".into(),
                ));
            }
        }
        let shared::PreparedRecords { rules, terminals } =
            shared::prepare_records(&shared, records, limits)?;
        let shared::PreparedFamily {
            context,
            source_conditions,
            zero_sectors,
            zero_certificates,
            ..
        } = shared;
        Ok(Self {
            family_fingerprint: Arc::new(family.fingerprint().to_owned()),
            context,
            root_sector,
            max_numerator_rank: scope.flatten(),
            ordering,
            rules,
            terminals,
            terminal_aliases: None,
            terminal_normalization: None,
            zero_sectors,
            _zero_certificates: zero_certificates,
            source_conditions,
            limits,
            cache: BTreeMap::new(),
            cache_representation: CandidateCacheRepresentation::Sparse,
            cache_weight: CacheWeight::default(),
            cache_budget: SharedCacheBudget::default(),
            statistics: ReductionStatistics::default(),
        })
    }

    /// The exact family-owned indexed coefficient context, useful for
    /// diagnostics and pure-Rust adapters. It has no closure authority.
    pub fn coefficient_context(&self) -> &IndexedCoefficientContext {
        &self.context
    }
}
