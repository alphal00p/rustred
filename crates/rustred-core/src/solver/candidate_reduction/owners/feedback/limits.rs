//! Cumulative overlay admission, not a native transient-allocation/RSS bound.
use super::{OwnerFeedbackError, PreparedOwnerBatch};
use crate::algebra::{
    Coefficient, CoefficientPolynomial, coefficient_clone_owned_retained_byte_bound,
    polynomial_clone_owned_heap_byte_bound,
};
use crate::solver::{Case, SectorDomainSolution};

/// Limits on all overlays retained by one snapshot (base records are unchanged).
/// Native bytes count polynomial/coefficient buffers and complete retained
/// affine-chart payloads (occupied matrix elements and exposed limb capacities).
/// Shared variable maps, hidden Matrix spare capacity, allocator overhead, old
/// live snapshots and source-search temporaries are not included. Structural
/// collection limits remain independently enforced.
#[derive(Clone, Copy, Debug)]
pub struct OwnerOverlayLimits {
    pub max_batches: usize,
    pub max_domains: usize,
    pub max_rules: usize,
    pub max_rhs_terms: usize,
    pub max_terminals: usize,
    pub max_native_bytes: usize,
}
impl Default for OwnerOverlayLimits {
    fn default() -> Self {
        Self {
            max_batches: 1024,
            max_domains: 10_000,
            max_rules: 100_000,
            max_rhs_terms: 1_000_000,
            max_terminals: 1_000_000,
            max_native_bytes: 512 * 1024 * 1024,
        }
    }
}
/// Borrowed-payload census using the native ownership convention above.
/// A raw source result and its prepared batch may have different byte counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OwnerOverlayUsage {
    pub batches: usize,
    pub domains: usize,
    pub rules: usize,
    pub rhs_terms: usize,
    pub terminals: usize,
    pub native_bytes: usize,
}
pub(super) fn check<const N: usize>(
    requested: usize,
    limit: usize,
    resource: &'static str,
) -> Result<(), OwnerFeedbackError<N>> {
    if requested > limit {
        Err(OwnerFeedbackError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
fn add<const N: usize>(
    value: &mut usize,
    amount: usize,
    limit: usize,
    resource: &'static str,
) -> Result<(), OwnerFeedbackError<N>> {
    let requested = value
        .checked_add(amount)
        .ok_or(OwnerFeedbackError::ResourceLimit {
            resource,
            requested: usize::MAX,
            limit,
        })?;
    check(requested, limit, resource)?;
    *value = requested;
    Ok(())
}
impl OwnerOverlayUsage {
    fn affine<const N: usize>(
        &mut self,
        case: &crate::solver::AffineCase<N>,
        limits: OwnerOverlayLimits,
    ) -> Result<(), OwnerFeedbackError<N>> {
        let bytes = case
            .native_payload_bytes()
            .ok_or(OwnerFeedbackError::ResourceLimit {
                resource: "native bytes",
                requested: usize::MAX,
                limit: limits.max_native_bytes,
            })?;
        add(
            &mut self.native_bytes,
            bytes,
            limits.max_native_bytes,
            "native bytes",
        )
    }
    pub(super) fn validate<const N: usize>(
        &self,
        limits: OwnerOverlayLimits,
    ) -> Result<(), OwnerFeedbackError<N>> {
        for (value, limit, name) in [
            (self.batches, limits.max_batches, "overlay batches"),
            (self.domains, limits.max_domains, "overlay domains"),
            (self.rules, limits.max_rules, "overlay rules"),
            (self.rhs_terms, limits.max_rhs_terms, "overlay RHS terms"),
            (self.terminals, limits.max_terminals, "overlay terminals"),
            (self.native_bytes, limits.max_native_bytes, "native bytes"),
        ] {
            check(value, limit, name)?;
        }
        Ok(())
    }
    fn polynomial<const N: usize>(
        &mut self,
        value: &CoefficientPolynomial,
        limits: OwnerOverlayLimits,
    ) -> Result<(), OwnerFeedbackError<N>> {
        let bytes = polynomial_clone_owned_heap_byte_bound(value)
            .and_then(|bytes| bytes.checked_add(size_of::<CoefficientPolynomial>()))
            .ok_or(OwnerFeedbackError::ResourceLimit {
                resource: "native bytes",
                requested: usize::MAX,
                limit: limits.max_native_bytes,
            })?;
        add(
            &mut self.native_bytes,
            bytes,
            limits.max_native_bytes,
            "native bytes",
        )
    }
    fn coefficient<const N: usize>(
        &mut self,
        value: &Coefficient,
        limits: OwnerOverlayLimits,
    ) -> Result<(), OwnerFeedbackError<N>> {
        let bytes = coefficient_clone_owned_retained_byte_bound(value).ok_or(
            OwnerFeedbackError::ResourceLimit {
                resource: "native bytes",
                requested: usize::MAX,
                limit: limits.max_native_bytes,
            },
        )?;
        add(
            &mut self.native_bytes,
            bytes,
            limits.max_native_bytes,
            "native bytes",
        )
    }
    fn domains<const N: usize>(
        &mut self,
        cases: &[Case<N>],
        limits: OwnerOverlayLimits,
    ) -> Result<(), OwnerFeedbackError<N>> {
        add(&mut self.batches, 1, limits.max_batches, "overlay batches")?;
        add(
            &mut self.domains,
            cases.len(),
            limits.max_domains,
            "overlay domains",
        )?;
        for case in cases {
            if let Some(affine) = case.affine() {
                self.affine(affine, limits)?;
            }
        }
        Ok(())
    }
    pub(super) fn admit_solution<const N: usize>(
        &mut self,
        result: &SectorDomainSolution<N>,
        limits: OwnerOverlayLimits,
    ) -> Result<(), OwnerFeedbackError<N>> {
        self.domains(&result.requested_cases, limits)?;
        add(
            &mut self.rules,
            result.rules.len(),
            limits.max_rules,
            "overlay rules",
        )?;
        add(
            &mut self.terminals,
            result.finite_residuals.len(),
            limits.max_terminals,
            "overlay terminals",
        )?;
        for rule in &result.rules {
            if let Some(affine) = rule.candidate.case.affine() {
                self.affine(affine, limits)?;
            }
            for branch in &rule.exceptions.branches {
                for equation in branch {
                    self.polynomial(equation, limits)?;
                }
            }
            add(
                &mut self.rhs_terms,
                rule.candidate.rhs.len(),
                limits.max_rhs_terms,
                "overlay RHS terms",
            )?;
            for term in &rule.candidate.rhs {
                self.coefficient(&term.coefficient, limits)?;
            }
        }
        Ok(())
    }
    pub(super) fn admit_batch<const N: usize>(
        &mut self,
        batch: &PreparedOwnerBatch<N>,
        limits: OwnerOverlayLimits,
    ) -> Result<(), OwnerFeedbackError<N>> {
        self.domains(
            &batch
                .overlay
                .as_ref()
                .expect("new overlay metadata")
                .requested_cases,
            limits,
        )?;
        add(
            &mut self.rules,
            batch.rules.len(),
            limits.max_rules,
            "overlay rules",
        )?;
        add(
            &mut self.terminals,
            batch.terminals.len(),
            limits.max_terminals,
            "overlay terminals",
        )?;
        for rule in &batch.rules {
            for equality in &rule.equalities {
                self.polynomial(equality.raw(), limits)?;
            }
            for branch in &rule.exceptions {
                for equation in branch {
                    self.polynomial(equation.raw(), limits)?;
                }
            }
            add(
                &mut self.rhs_terms,
                rule.rhs.len(),
                limits.max_rhs_terms,
                "overlay RHS terms",
            )?;
            for term in &rule.rhs {
                self.coefficient(term.coefficient.raw(), limits)?;
                self.polynomial(term.denominator.raw(), limits)?;
            }
        }
        Ok(())
    }
}
