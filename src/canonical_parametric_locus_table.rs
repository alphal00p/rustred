//! Opaque authority for a first-seen canonical table of parametric loci.
//!
//! Exact polynomial equality is structural.  Equality up to a nonzero element
//! of the coefficient field is proved only by Symbolica through
//! `ParametricCoefficientContext::polynomial_loci_are_associates_with_census`.
//! The sealed owner is deliberately non-`Clone`: possession of the table is
//! the authority that the bounded pairwise proof has already run.

use std::fmt;
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::parametric_coefficient::{
    ParametricPolynomialAssociateLimits, ParametricPolynomialAssociateStats,
};
use crate::{
    ExactAlgebraLimits, ParametricCoefficientContext, ParametricCoefficientError,
    ParametricPolynomial,
};

pub(crate) const CANONICAL_PARAMETRIC_LOCUS_TABLE_V1_SCHEMA: &str =
    "rustred-canonical-parametric-locus-table-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CanonicalLocusTableLimits {
    pub(crate) exact_algebra: ExactAlgebraLimits,
    pub(crate) associate: ParametricPolynomialAssociateLimits,
    pub(crate) max_context_fingerprint_bytes: usize,
    pub(crate) max_structural_loci: usize,
    pub(crate) max_equality_comparisons: usize,
    pub(crate) max_equality_term_pairs: usize,
    pub(crate) max_associate_comparisons: usize,
    pub(crate) max_associate_term_pairs: usize,
    pub(crate) max_associate_native_cross_term_pairs: usize,
    pub(crate) max_retained_polynomial_terms: usize,
    pub(crate) max_retained_polynomial_exponent_entries: usize,
    pub(crate) max_retained_polynomial_integer_bits: usize,
    pub(crate) max_retained_owned_logical_bytes: usize,
    pub(crate) max_construction_owned_logical_peak_upper_bound: usize,
}

impl Default for CanonicalLocusTableLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            associate: ParametricPolynomialAssociateLimits::default(),
            max_context_fingerprint_bytes: usize::MAX,
            max_structural_loci: usize::MAX,
            max_equality_comparisons: usize::MAX,
            max_equality_term_pairs: usize::MAX,
            max_associate_comparisons: usize::MAX,
            max_associate_term_pairs: usize::MAX,
            max_associate_native_cross_term_pairs: usize::MAX,
            max_retained_polynomial_terms: usize::MAX,
            max_retained_polynomial_exponent_entries: usize::MAX,
            max_retained_polynomial_integer_bits: usize::MAX,
            max_retained_owned_logical_bytes: usize::MAX,
            max_construction_owned_logical_peak_upper_bound: usize::MAX,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CanonicalLocusTableCopyLimits {
    pub(crate) exact_algebra: ExactAlgebraLimits,
    pub(crate) max_context_fingerprint_bytes: usize,
    pub(crate) max_structural_loci: usize,
    pub(crate) max_retained_polynomial_terms: usize,
    pub(crate) max_retained_polynomial_exponent_entries: usize,
    pub(crate) max_retained_polynomial_integer_bits: usize,
    pub(crate) max_retained_owned_logical_bytes: usize,
    pub(crate) max_copy_owned_logical_peak_upper_bound: usize,
}

impl Default for CanonicalLocusTableCopyLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            max_context_fingerprint_bytes: usize::MAX,
            max_structural_loci: usize::MAX,
            max_retained_polynomial_terms: usize::MAX,
            max_retained_polynomial_exponent_entries: usize::MAX,
            max_retained_polynomial_integer_bits: usize::MAX,
            max_retained_owned_logical_bytes: usize::MAX,
            max_copy_owned_logical_peak_upper_bound: usize::MAX,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CanonicalLocusTableStats {
    structural_loci: usize,
    equality_comparisons: usize,
    equality_term_pairs: usize,
    associate_comparisons: usize,
    associate_term_pairs: usize,
    associate_native_cross_term_pairs: usize,
    associate_rustred_visible_temporary_byte_peak: usize,
    associate_native_workspace_byte_peak: usize,
    associate_combined_temporary_byte_peak: usize,
    retained_polynomial_terms: usize,
    retained_polynomial_exponent_entries: usize,
    retained_polynomial_integer_bits: usize,
    retained_owned_logical_bytes: usize,
    construction_owned_logical_peak_upper_bound: usize,
}

impl CanonicalLocusTableStats {
    pub(crate) const fn structural_loci(self) -> usize {
        self.structural_loci
    }

    pub(crate) const fn equality_comparisons(self) -> usize {
        self.equality_comparisons
    }

    pub(crate) const fn equality_term_pairs(self) -> usize {
        self.equality_term_pairs
    }

    pub(crate) const fn associate_comparisons(self) -> usize {
        self.associate_comparisons
    }

    pub(crate) const fn associate_term_pairs(self) -> usize {
        self.associate_term_pairs
    }

    pub(crate) const fn associate_native_cross_term_pairs(self) -> usize {
        self.associate_native_cross_term_pairs
    }

    pub(crate) const fn associate_rustred_visible_temporary_byte_peak(self) -> usize {
        self.associate_rustred_visible_temporary_byte_peak
    }

    pub(crate) const fn associate_native_workspace_byte_peak(self) -> usize {
        self.associate_native_workspace_byte_peak
    }

    pub(crate) const fn associate_combined_temporary_byte_peak(self) -> usize {
        self.associate_combined_temporary_byte_peak
    }

    pub(crate) const fn retained_polynomial_terms(self) -> usize {
        self.retained_polynomial_terms
    }

    pub(crate) const fn retained_polynomial_exponent_entries(self) -> usize {
        self.retained_polynomial_exponent_entries
    }

    pub(crate) const fn retained_polynomial_integer_bits(self) -> usize {
        self.retained_polynomial_integer_bits
    }

    pub(crate) const fn retained_owned_logical_bytes(self) -> usize {
        self.retained_owned_logical_bytes
    }

    pub(crate) const fn construction_owned_logical_peak_upper_bound(self) -> usize {
        self.construction_owned_logical_peak_upper_bound
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CanonicalLocusInternDisposition {
    Inserted,
    ExactDuplicate,
    AssociateDuplicate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CanonicalLocusInternOutcome {
    locus_ordinal: usize,
    disposition: CanonicalLocusInternDisposition,
}

impl CanonicalLocusInternOutcome {
    pub(crate) const fn locus_ordinal(self) -> usize {
        self.locus_ordinal
    }

    pub(crate) const fn disposition(self) -> CanonicalLocusInternDisposition {
        self.disposition
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CanonicalLocusTableError {
    SchemaMismatch,
    ContextMismatch,
    IdenticallyZeroLocus,
    CoefficientFieldLocus,
    ReservedCapacityExhausted {
        requested: usize,
        reserved: usize,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    RetainedByteEnvelopeExceeded {
        observed: usize,
        admitted: usize,
    },
    SymbolicaPanic {
        stage: &'static str,
    },
    ParametricCoefficient(ParametricCoefficientError),
}

impl fmt::Display for CanonicalLocusTableError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => write!(formatter, "canonical locus table schema mismatch"),
            Self::ContextMismatch => write!(formatter, "canonical locus table context mismatch"),
            Self::IdenticallyZeroLocus => write!(formatter, "canonical locus is identically zero"),
            Self::CoefficientFieldLocus => {
                write!(
                    formatter,
                    "canonical locus belongs to the coefficient field"
                )
            }
            Self::ReservedCapacityExhausted {
                requested,
                reserved,
            } => write!(
                formatter,
                "canonical locus table requested {requested} entries but reserved {reserved}"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requested {requested}, configured limit is {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "{resource} allocation of {requested} entries failed after bounded preflight"
            ),
            Self::RetainedByteEnvelopeExceeded { observed, admitted } => write!(
                formatter,
                "canonical locus table retained-byte envelope {observed} exceeded admission {admitted}"
            ),
            Self::SymbolicaPanic { stage } => {
                write!(formatter, "Symbolica panicked during {stage}")
            }
            Self::ParametricCoefficient(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CanonicalLocusTableError {}

impl From<ParametricCoefficientError> for CanonicalLocusTableError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::ParametricCoefficient(value)
    }
}

/// Mutable first-seen construction authority. It cannot be cloned.
pub(crate) struct CanonicalLocusTableBuilder {
    schema: &'static str,
    context_fingerprint: String,
    loci: Vec<ParametricPolynomial>,
    limits: CanonicalLocusTableLimits,
    stats: CanonicalLocusTableStats,
}

impl CanonicalLocusTableBuilder {
    pub(crate) fn try_new(
        context: &ParametricCoefficientContext,
        schema: &'static str,
        reserved_slots: usize,
        limits: CanonicalLocusTableLimits,
    ) -> Result<Self, CanonicalLocusTableError> {
        check_limit(
            "canonical locus table context fingerprint bytes",
            context.fingerprint().len(),
            limits.max_context_fingerprint_bytes,
        )?;
        check_limit(
            "canonical locus table structural loci",
            reserved_slots,
            limits.max_structural_loci,
        )?;
        let admitted = initial_retained_byte_bound(context.fingerprint().len(), reserved_slots)?;
        check_limit(
            "canonical locus table retained owned logical bytes",
            admitted,
            limits.max_retained_owned_logical_bytes,
        )?;
        check_limit(
            "canonical locus table construction owned logical peak upper bound",
            admitted,
            limits.max_construction_owned_logical_peak_upper_bound,
        )?;

        let context_fingerprint = try_copy_string(
            context.fingerprint(),
            "canonical locus table context fingerprint",
        )?;
        let mut loci = Vec::new();
        loci.try_reserve_exact(reserved_slots).map_err(|_| {
            CanonicalLocusTableError::AllocationFailure {
                resource: "canonical locus table structural loci",
                requested: reserved_slots,
            }
        })?;
        if size_of::<ParametricPolynomial>() != 0 && loci.capacity() != reserved_slots {
            return Err(CanonicalLocusTableError::AllocationFailure {
                resource: "canonical locus table structural loci",
                requested: reserved_slots,
            });
        }
        let observed = builder_retained_byte_bound(&context_fingerprint, &loci)?;
        if observed > admitted {
            return Err(CanonicalLocusTableError::RetainedByteEnvelopeExceeded {
                observed,
                admitted,
            });
        }
        Ok(Self {
            schema,
            context_fingerprint,
            loci,
            limits,
            stats: CanonicalLocusTableStats {
                retained_owned_logical_bytes: observed,
                construction_owned_logical_peak_upper_bound: observed,
                ..CanonicalLocusTableStats::default()
            },
        })
    }

    pub(crate) fn loci(&self) -> &[ParametricPolynomial] {
        &self.loci
    }

    pub(crate) const fn stats(&self) -> CanonicalLocusTableStats {
        self.stats
    }

    pub(crate) fn try_intern(
        &mut self,
        context: &ParametricCoefficientContext,
        polynomial: &ParametricPolynomial,
    ) -> Result<CanonicalLocusInternOutcome, CanonicalLocusTableError> {
        catch_unwind(AssertUnwindSafe(|| {
            self.try_intern_inner(context, polynomial)
        }))
        .map_err(|_| CanonicalLocusTableError::SymbolicaPanic {
            stage: "canonical locus interning",
        })?
    }

    fn try_intern_inner(
        &mut self,
        context: &ParametricCoefficientContext,
        polynomial: &ParametricPolynomial,
    ) -> Result<CanonicalLocusInternOutcome, CanonicalLocusTableError> {
        self.authenticate_context(context)?;
        let census = context.preflight_polynomial_validation_payload_with_limits(
            polynomial,
            self.limits.exact_algebra,
            usize::MAX,
            usize::MAX,
            usize::MAX,
        )?;
        if polynomial.is_zero() {
            return Err(CanonicalLocusTableError::IdenticallyZeroLocus);
        }
        if !context
            .polynomial_depends_on_indices_with_limits(polynomial, self.limits.exact_algebra)?
        {
            return Err(CanonicalLocusTableError::CoefficientFieldLocus);
        }

        let mut prospective = self.stats;
        for (ordinal, retained) in self.loci.iter().enumerate() {
            prospective.equality_comparisons = checked_bounded_add(
                "canonical locus table equality comparisons",
                prospective.equality_comparisons,
                1,
                self.limits.max_equality_comparisons,
            )?;
            prospective.equality_term_pairs = checked_bounded_add(
                "canonical locus table equality term pairs",
                prospective.equality_term_pairs,
                checked_mul(
                    "canonical locus table equality term pairs",
                    retained.term_count(),
                    polynomial.term_count(),
                )?,
                self.limits.max_equality_term_pairs,
            )?;
            if retained == polynomial {
                self.stats = prospective;
                return Ok(CanonicalLocusInternOutcome {
                    locus_ordinal: ordinal,
                    disposition: CanonicalLocusInternDisposition::ExactDuplicate,
                });
            }
        }

        for (ordinal, retained) in self.loci.iter().enumerate() {
            prospective.associate_comparisons = checked_bounded_add(
                "canonical locus table associate comparisons",
                prospective.associate_comparisons,
                1,
                self.limits.max_associate_comparisons,
            )?;
            prospective.associate_term_pairs = checked_bounded_add(
                "canonical locus table associate term pairs",
                prospective.associate_term_pairs,
                checked_mul(
                    "canonical locus table associate term pairs",
                    retained.term_count(),
                    polynomial.term_count(),
                )?,
                self.limits.max_associate_term_pairs,
            )?;
            let mut child = self.limits.associate;
            child.exact_algebra =
                intersect_exact_limits(child.exact_algebra, self.limits.exact_algebra);
            let configured_native_cross_term_pairs = child.max_native_cross_term_pairs;
            let configured_peak_native_cross_term_pairs = child.max_peak_native_cross_term_pairs;
            let configured_exact_term_operations = child.exact_algebra.max_term_operations;
            let aggregate_native_cross_term_pairs_remaining = remaining_limit(
                "canonical locus table associate native cross term pairs",
                self.limits.max_associate_native_cross_term_pairs,
                prospective.associate_native_cross_term_pairs,
            )?;
            let aggregate_owns_native_cross_term_pairs = aggregate_native_cross_term_pairs_remaining
                <= configured_native_cross_term_pairs
                && aggregate_native_cross_term_pairs_remaining <= configured_exact_term_operations;
            let aggregate_owns_peak_native_cross_term_pairs =
                aggregate_native_cross_term_pairs_remaining <= configured_native_cross_term_pairs
                    && aggregate_native_cross_term_pairs_remaining
                        <= configured_peak_native_cross_term_pairs;
            child.max_native_cross_term_pairs =
                configured_native_cross_term_pairs.min(aggregate_native_cross_term_pairs_remaining);
            child.max_peak_native_cross_term_pairs = child
                .max_peak_native_cross_term_pairs
                .min(child.max_native_cross_term_pairs);
            let configured_visible = child.max_rustred_visible_temporary_byte_envelope;
            let configured_native = child.max_native_workspace_byte_envelope;
            let configured_combined = child.max_combined_temporary_byte_envelope;
            let construction_remaining = remaining_limit(
                "canonical locus table construction owned logical peak upper bound",
                self.limits.max_construction_owned_logical_peak_upper_bound,
                prospective.retained_owned_logical_bytes,
            )?;
            let construction_owns_combined = construction_remaining <= configured_combined;
            if construction_owns_combined {
                child.max_rustred_visible_temporary_byte_envelope = child
                    .max_rustred_visible_temporary_byte_envelope
                    .min(construction_remaining);
                child.max_native_workspace_byte_envelope = child
                    .max_native_workspace_byte_envelope
                    .min(construction_remaining);
            }
            child.max_combined_temporary_byte_envelope =
                configured_combined.min(construction_remaining);
            let result = match context
                .polynomial_loci_are_associates_with_census(retained, polynomial, child)
            {
                Ok(result) => result,
                Err(ParametricCoefficientError::ResourceLimit {
                    resource: "polynomial-associate combined temporary byte envelope",
                    requested,
                    ..
                }) if construction_owns_combined => {
                    return Err(map_construction_scratch_limit(
                        prospective.retained_owned_logical_bytes,
                        requested,
                        self.limits.max_construction_owned_logical_peak_upper_bound,
                    ));
                }
                Err(ParametricCoefficientError::ResourceLimit {
                    resource: "polynomial-associate RustRed-visible temporary byte envelope",
                    requested,
                    ..
                }) if construction_owns_combined
                    && construction_remaining <= configured_visible =>
                {
                    return Err(map_construction_scratch_limit(
                        prospective.retained_owned_logical_bytes,
                        requested,
                        self.limits.max_construction_owned_logical_peak_upper_bound,
                    ));
                }
                Err(ParametricCoefficientError::ResourceLimit {
                    resource: "polynomial-associate native workspace byte envelope",
                    requested,
                    ..
                }) if construction_owns_combined && construction_remaining <= configured_native => {
                    return Err(map_construction_scratch_limit(
                        prospective.retained_owned_logical_bytes,
                        requested,
                        self.limits.max_construction_owned_logical_peak_upper_bound,
                    ));
                }
                Err(ParametricCoefficientError::ResourceLimit {
                    resource: "polynomial-associate native cross term pairs",
                    requested,
                    ..
                }) if aggregate_owns_native_cross_term_pairs => {
                    return Err(map_native_cross_aggregate_limit(
                        prospective.associate_native_cross_term_pairs,
                        requested,
                        self.limits.max_associate_native_cross_term_pairs,
                    ));
                }
                Err(ParametricCoefficientError::ResourceLimit {
                    resource: "polynomial-associate peak native cross term pairs",
                    requested,
                    ..
                }) if aggregate_owns_peak_native_cross_term_pairs => {
                    return Err(map_native_cross_aggregate_limit(
                        prospective.associate_native_cross_term_pairs,
                        requested,
                        self.limits.max_associate_native_cross_term_pairs,
                    ));
                }
                Err(ParametricCoefficientError::ResourceCountOverflow {
                    resource: "polynomial-associate combined temporary byte envelope",
                }) if construction_owns_combined => {
                    return Err(CanonicalLocusTableError::ResourceCountOverflow {
                        resource: "canonical locus table construction owned logical peak upper bound",
                    });
                }
                Err(ParametricCoefficientError::ResourceCountOverflow {
                    resource: "polynomial-associate RustRed-visible temporary byte envelope",
                }) if construction_owns_combined
                    && construction_remaining <= configured_visible =>
                {
                    return Err(CanonicalLocusTableError::ResourceCountOverflow {
                        resource: "canonical locus table construction owned logical peak upper bound",
                    });
                }
                Err(ParametricCoefficientError::ResourceCountOverflow {
                    resource: "polynomial-associate native workspace byte envelope",
                }) if construction_owns_combined && construction_remaining <= configured_native => {
                    return Err(CanonicalLocusTableError::ResourceCountOverflow {
                        resource: "canonical locus table construction owned logical peak upper bound",
                    });
                }
                Err(ParametricCoefficientError::ResourceCountOverflow {
                    resource: "polynomial-associate native cross term pairs",
                }) if aggregate_owns_native_cross_term_pairs => {
                    return Err(CanonicalLocusTableError::ResourceCountOverflow {
                        resource: "canonical locus table associate native cross term pairs",
                    });
                }
                Err(ParametricCoefficientError::ResourceCountOverflow {
                    resource: "polynomial-associate peak native cross term pairs",
                }) if aggregate_owns_peak_native_cross_term_pairs => {
                    return Err(CanonicalLocusTableError::ResourceCountOverflow {
                        resource: "canonical locus table associate native cross term pairs",
                    });
                }
                Err(error) => return Err(error.into()),
            };
            charge_associate_stats(&mut prospective, result.stats(), self.limits)?;
            if result.associated() {
                self.stats = prospective;
                return Ok(CanonicalLocusInternOutcome {
                    locus_ordinal: ordinal,
                    disposition: CanonicalLocusInternDisposition::AssociateDuplicate,
                });
            }
        }

        let requested = checked_add("canonical locus table structural loci", self.loci.len(), 1)?;
        check_limit(
            "canonical locus table structural loci",
            requested,
            self.limits.max_structural_loci,
        )?;
        if requested > self.loci.capacity() {
            return Err(CanonicalLocusTableError::ReservedCapacityExhausted {
                requested,
                reserved: self.loci.capacity(),
            });
        }
        prospective.retained_polynomial_terms = checked_bounded_add(
            "canonical locus table retained polynomial terms",
            prospective.retained_polynomial_terms,
            census.source_terms(),
            self.limits.max_retained_polynomial_terms,
        )?;
        prospective.retained_polynomial_exponent_entries = checked_bounded_add(
            "canonical locus table retained polynomial exponent entries",
            prospective.retained_polynomial_exponent_entries,
            census.source_exponent_entries(),
            self.limits.max_retained_polynomial_exponent_entries,
        )?;
        prospective.retained_polynomial_integer_bits = checked_bounded_add(
            "canonical locus table retained polynomial integer bits",
            prospective.retained_polynomial_integer_bits,
            census.source_integer_bits(),
            self.limits.max_retained_polynomial_integer_bits,
        )?;
        let polynomial_owned = polynomial.owned_retained_byte_bound().ok_or(
            CanonicalLocusTableError::ResourceCountOverflow {
                resource: "canonical locus table retained owned logical bytes",
            },
        )?;
        let prospective_retained = checked_add(
            "canonical locus table retained owned logical bytes",
            self.stats.retained_owned_logical_bytes,
            polynomial_owned,
        )?;
        check_limit(
            "canonical locus table retained owned logical bytes",
            prospective_retained,
            self.limits.max_retained_owned_logical_bytes,
        )?;
        check_limit(
            "canonical locus table construction owned logical peak upper bound",
            prospective_retained,
            self.limits.max_construction_owned_logical_peak_upper_bound,
        )?;
        let copied = polynomial
            .try_copy_authenticated_sparse_payload()
            .map_err(|resource| CanonicalLocusTableError::AllocationFailure {
                resource,
                requested: census.source_terms().max(census.source_exponent_entries()),
            })?;
        let copied_owned = copied.owned_retained_byte_bound().ok_or(
            CanonicalLocusTableError::ResourceCountOverflow {
                resource: "canonical locus table retained owned logical bytes",
            },
        )?;
        if copied_owned > polynomial_owned {
            return Err(CanonicalLocusTableError::RetainedByteEnvelopeExceeded {
                observed: copied_owned,
                admitted: polynomial_owned,
            });
        }
        let prospective_observed = checked_add(
            "canonical locus table retained owned logical bytes",
            builder_retained_byte_bound(&self.context_fingerprint, &self.loci)?,
            copied_owned,
        )?;
        if prospective_observed > prospective_retained {
            return Err(CanonicalLocusTableError::RetainedByteEnvelopeExceeded {
                observed: prospective_observed,
                admitted: prospective_retained,
            });
        }
        let ordinal = self.loci.len();
        self.loci.push(copied);
        prospective.structural_loci = requested;
        prospective.construction_owned_logical_peak_upper_bound = prospective
            .construction_owned_logical_peak_upper_bound
            .max(prospective_retained);
        prospective.retained_owned_logical_bytes = prospective_observed;
        self.stats = prospective;
        Ok(CanonicalLocusInternOutcome {
            locus_ordinal: ordinal,
            disposition: CanonicalLocusInternDisposition::Inserted,
        })
    }

    pub(crate) fn seal(self) -> Result<CanonicalLocusTableOwner, CanonicalLocusTableError> {
        let mut stats = self.stats;
        let observed = table_retained_byte_bound(&self.context_fingerprint, &self.loci)?;
        check_limit(
            "canonical locus table retained owned logical bytes",
            observed,
            self.limits.max_retained_owned_logical_bytes,
        )?;
        check_limit(
            "canonical locus table construction owned logical peak upper bound",
            observed,
            self.limits.max_construction_owned_logical_peak_upper_bound,
        )?;
        if observed > stats.retained_owned_logical_bytes {
            return Err(CanonicalLocusTableError::RetainedByteEnvelopeExceeded {
                observed,
                admitted: stats.retained_owned_logical_bytes,
            });
        }
        stats.retained_owned_logical_bytes = observed;
        Ok(CanonicalLocusTableOwner {
            schema: self.schema,
            context_fingerprint: self.context_fingerprint,
            loci: self.loci,
            stats,
        })
    }

    fn authenticate_context(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<(), CanonicalLocusTableError> {
        if self.context_fingerprint == context.fingerprint() {
            Ok(())
        } else {
            Err(CanonicalLocusTableError::ContextMismatch)
        }
    }
}

/// Move-only proof that `loci` are nonzero, index-dependent, and pairwise
/// non-associate in deterministic first-seen order.
pub(crate) struct CanonicalLocusTableOwner {
    schema: &'static str,
    context_fingerprint: String,
    loci: Vec<ParametricPolynomial>,
    stats: CanonicalLocusTableStats,
}

impl CanonicalLocusTableOwner {
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    pub(crate) fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }

    pub(crate) fn loci(&self) -> &[ParametricPolynomial] {
        &self.loci
    }

    pub(crate) const fn stats(&self) -> CanonicalLocusTableStats {
        self.stats
    }

    pub(crate) fn retained_owned_logical_byte_bound(
        &self,
    ) -> Result<usize, CanonicalLocusTableError> {
        table_retained_byte_bound(&self.context_fingerprint, &self.loci)
    }

    /// No-allocation retained-byte bound for a compact authenticated copy.
    ///
    /// The context string and locus vector are projected at exact logical
    /// length. Each sparse polynomial is charged at its source-owned bound;
    /// the central authenticated polynomial copier guarantees that a
    /// successful fixed-length clone cannot retain more than that source.
    pub(crate) fn projected_compact_copy_owned_logical_byte_bound(
        &self,
    ) -> Result<usize, CanonicalLocusTableError> {
        compact_retained_byte_bound(&self.context_fingerprint, &self.loci)
    }

    pub(crate) fn try_copy_authenticated(
        &self,
        context: &ParametricCoefficientContext,
        expected_schema: &'static str,
        limits: CanonicalLocusTableCopyLimits,
    ) -> Result<Self, CanonicalLocusTableError> {
        catch_unwind(AssertUnwindSafe(|| {
            self.try_copy_authenticated_inner(context, expected_schema, limits)
        }))
        .map_err(|_| CanonicalLocusTableError::SymbolicaPanic {
            stage: "canonical locus authenticated copy",
        })?
    }

    fn try_copy_authenticated_inner(
        &self,
        context: &ParametricCoefficientContext,
        expected_schema: &'static str,
        limits: CanonicalLocusTableCopyLimits,
    ) -> Result<Self, CanonicalLocusTableError> {
        if self.schema != expected_schema {
            return Err(CanonicalLocusTableError::SchemaMismatch);
        }
        if self.context_fingerprint != context.fingerprint() {
            return Err(CanonicalLocusTableError::ContextMismatch);
        }
        check_limit(
            "canonical locus authenticated copy context fingerprint bytes",
            self.context_fingerprint.len(),
            limits.max_context_fingerprint_bytes,
        )?;
        check_limit(
            "canonical locus authenticated copy structural loci",
            self.loci.len(),
            limits.max_structural_loci,
        )?;
        let mut terms = 0usize;
        let mut exponents = 0usize;
        let mut bits = 0usize;
        for polynomial in &self.loci {
            let census = context.preflight_polynomial_validation_payload_with_limits(
                polynomial,
                limits.exact_algebra,
                usize::MAX,
                usize::MAX,
                usize::MAX,
            )?;
            if polynomial.is_zero() {
                return Err(CanonicalLocusTableError::IdenticallyZeroLocus);
            }
            if !context
                .polynomial_depends_on_indices_with_limits(polynomial, limits.exact_algebra)?
            {
                return Err(CanonicalLocusTableError::CoefficientFieldLocus);
            }
            terms = checked_bounded_add(
                "canonical locus authenticated copy retained polynomial terms",
                terms,
                census.source_terms(),
                limits.max_retained_polynomial_terms,
            )?;
            exponents = checked_bounded_add(
                "canonical locus authenticated copy retained polynomial exponent entries",
                exponents,
                census.source_exponent_entries(),
                limits.max_retained_polynomial_exponent_entries,
            )?;
            bits = checked_bounded_add(
                "canonical locus authenticated copy retained polynomial integer bits",
                bits,
                census.source_integer_bits(),
                limits.max_retained_polynomial_integer_bits,
            )?;
        }

        let admitted = compact_retained_byte_bound(&self.context_fingerprint, &self.loci)?;
        check_limit(
            "canonical locus authenticated copy retained owned logical bytes",
            admitted,
            limits.max_retained_owned_logical_bytes,
        )?;
        let source_retained = self.retained_owned_logical_byte_bound()?;
        check_limit(
            "canonical locus authenticated copy owned logical peak upper bound",
            checked_add(
                "canonical locus authenticated copy owned logical peak upper bound",
                source_retained,
                admitted,
            )?,
            limits.max_copy_owned_logical_peak_upper_bound,
        )?;
        let context_fingerprint = try_copy_string(
            &self.context_fingerprint,
            "canonical locus authenticated copy context fingerprint",
        )?;
        let mut loci = Vec::new();
        loci.try_reserve_exact(self.loci.len()).map_err(|_| {
            CanonicalLocusTableError::AllocationFailure {
                resource: "canonical locus authenticated copy structural loci",
                requested: self.loci.len(),
            }
        })?;
        if size_of::<ParametricPolynomial>() != 0 && loci.capacity() != self.loci.len() {
            return Err(CanonicalLocusTableError::AllocationFailure {
                resource: "canonical locus authenticated copy structural loci",
                requested: self.loci.len(),
            });
        }
        for polynomial in &self.loci {
            loci.push(
                polynomial
                    .try_copy_authenticated_sparse_payload()
                    .map_err(|resource| CanonicalLocusTableError::AllocationFailure {
                        resource,
                        requested: polynomial
                            .term_count()
                            .max(polynomial.raw().exponents.len()),
                    })?,
            );
        }
        let observed = table_retained_byte_bound(&context_fingerprint, &loci)?;
        if observed > admitted {
            return Err(CanonicalLocusTableError::RetainedByteEnvelopeExceeded {
                observed,
                admitted,
            });
        }
        let mut stats = self.stats;
        stats.structural_loci = loci.len();
        stats.retained_polynomial_terms = terms;
        stats.retained_polynomial_exponent_entries = exponents;
        stats.retained_polynomial_integer_bits = bits;
        stats.retained_owned_logical_bytes = observed;
        Ok(Self {
            schema: self.schema,
            context_fingerprint,
            loci,
            stats,
        })
    }
}

impl fmt::Debug for CanonicalLocusTableOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CanonicalLocusTableOwner")
            .field("schema", &self.schema)
            .field("context_fingerprint", &self.context_fingerprint)
            .field("locus_count", &self.loci.len())
            .field("stats", &self.stats)
            .finish()
    }
}

fn charge_associate_stats(
    stats: &mut CanonicalLocusTableStats,
    child: ParametricPolynomialAssociateStats,
    limits: CanonicalLocusTableLimits,
) -> Result<(), CanonicalLocusTableError> {
    stats.associate_native_cross_term_pairs = checked_bounded_add(
        "canonical locus table associate native cross term pairs",
        stats.associate_native_cross_term_pairs,
        child.native_cross_term_pairs(),
        limits.max_associate_native_cross_term_pairs,
    )?;
    stats.associate_rustred_visible_temporary_byte_peak = stats
        .associate_rustred_visible_temporary_byte_peak
        .max(child.rustred_visible_temporary_byte_envelope());
    stats.associate_native_workspace_byte_peak = stats
        .associate_native_workspace_byte_peak
        .max(child.native_workspace_byte_envelope());
    let combined = checked_add(
        "canonical locus table associate combined temporary byte peak",
        child.rustred_visible_temporary_byte_envelope(),
        child.native_workspace_byte_envelope(),
    )?;
    stats.associate_combined_temporary_byte_peak =
        stats.associate_combined_temporary_byte_peak.max(combined);
    let peak = checked_add(
        "canonical locus table construction owned logical peak upper bound",
        stats.retained_owned_logical_bytes,
        combined,
    )?;
    check_limit(
        "canonical locus table construction owned logical peak upper bound",
        peak,
        limits.max_construction_owned_logical_peak_upper_bound,
    )?;
    stats.construction_owned_logical_peak_upper_bound =
        stats.construction_owned_logical_peak_upper_bound.max(peak);
    Ok(())
}

fn initial_retained_byte_bound(
    context_fingerprint_len: usize,
    reserved_slots: usize,
) -> Result<usize, CanonicalLocusTableError> {
    checked_add(
        "canonical locus table retained owned logical bytes",
        checked_add(
            "canonical locus table retained owned logical bytes",
            size_of::<CanonicalLocusTableBuilder>(),
            context_fingerprint_len,
        )?,
        checked_mul(
            "canonical locus table retained owned logical bytes",
            reserved_slots,
            size_of::<ParametricPolynomial>(),
        )?,
    )
}

fn compact_retained_byte_bound(
    context_fingerprint: &str,
    loci: &[ParametricPolynomial],
) -> Result<usize, CanonicalLocusTableError> {
    let mut bytes = checked_add(
        "canonical locus table retained owned logical bytes",
        checked_add(
            "canonical locus table retained owned logical bytes",
            size_of::<CanonicalLocusTableOwner>(),
            context_fingerprint.len(),
        )?,
        checked_mul(
            "canonical locus table retained owned logical bytes",
            loci.len(),
            size_of::<ParametricPolynomial>(),
        )?,
    )?;
    for polynomial in loci {
        bytes = checked_add(
            "canonical locus table retained owned logical bytes",
            bytes,
            polynomial.owned_retained_byte_bound().ok_or(
                CanonicalLocusTableError::ResourceCountOverflow {
                    resource: "canonical locus table retained owned logical bytes",
                },
            )?,
        )?;
    }
    Ok(bytes)
}

fn table_retained_byte_bound(
    context_fingerprint: &String,
    loci: &Vec<ParametricPolynomial>,
) -> Result<usize, CanonicalLocusTableError> {
    let mut bytes = checked_add(
        "canonical locus table retained owned logical bytes",
        checked_add(
            "canonical locus table retained owned logical bytes",
            size_of::<CanonicalLocusTableOwner>(),
            context_fingerprint.capacity(),
        )?,
        checked_mul(
            "canonical locus table retained owned logical bytes",
            loci.capacity(),
            size_of::<ParametricPolynomial>(),
        )?,
    )?;
    for polynomial in loci {
        bytes = checked_add(
            "canonical locus table retained owned logical bytes",
            bytes,
            polynomial.owned_retained_byte_bound().ok_or(
                CanonicalLocusTableError::ResourceCountOverflow {
                    resource: "canonical locus table retained owned logical bytes",
                },
            )?,
        )?;
    }
    Ok(bytes)
}

fn builder_retained_byte_bound(
    context_fingerprint: &String,
    loci: &Vec<ParametricPolynomial>,
) -> Result<usize, CanonicalLocusTableError> {
    let mut bytes = checked_add(
        "canonical locus table retained owned logical bytes",
        checked_add(
            "canonical locus table retained owned logical bytes",
            size_of::<CanonicalLocusTableBuilder>(),
            context_fingerprint.capacity(),
        )?,
        checked_mul(
            "canonical locus table retained owned logical bytes",
            loci.capacity(),
            size_of::<ParametricPolynomial>(),
        )?,
    )?;
    for polynomial in loci {
        bytes = checked_add(
            "canonical locus table retained owned logical bytes",
            bytes,
            polynomial.owned_retained_byte_bound().ok_or(
                CanonicalLocusTableError::ResourceCountOverflow {
                    resource: "canonical locus table retained owned logical bytes",
                },
            )?,
        )?;
    }
    Ok(bytes)
}

fn map_construction_scratch_limit(
    retained: usize,
    scratch: usize,
    limit: usize,
) -> CanonicalLocusTableError {
    let resource = "canonical locus table construction owned logical peak upper bound";
    match retained.checked_add(scratch) {
        Some(requested) => CanonicalLocusTableError::ResourceLimit {
            resource,
            requested,
            limit,
        },
        None => CanonicalLocusTableError::ResourceCountOverflow { resource },
    }
}

fn map_native_cross_aggregate_limit(
    already_used: usize,
    child_requested: usize,
    limit: usize,
) -> CanonicalLocusTableError {
    let resource = "canonical locus table associate native cross term pairs";
    match already_used.checked_add(child_requested) {
        Some(requested) => CanonicalLocusTableError::ResourceLimit {
            resource,
            requested,
            limit,
        },
        None => CanonicalLocusTableError::ResourceCountOverflow { resource },
    }
}

fn try_copy_string(
    source: &str,
    resource: &'static str,
) -> Result<String, CanonicalLocusTableError> {
    let mut copy = String::new();
    copy.try_reserve_exact(source.len()).map_err(|_| {
        CanonicalLocusTableError::AllocationFailure {
            resource,
            requested: source.len(),
        }
    })?;
    if copy.capacity() != source.len() {
        return Err(CanonicalLocusTableError::AllocationFailure {
            resource,
            requested: source.len(),
        });
    }
    copy.push_str(source);
    Ok(copy)
}

fn intersect_exact_limits(
    left: ExactAlgebraLimits,
    right: ExactAlgebraLimits,
) -> ExactAlgebraLimits {
    ExactAlgebraLimits {
        max_exponent: left.max_exponent.min(right.max_exponent),
        max_polynomial_terms: left.max_polynomial_terms.min(right.max_polynomial_terms),
        max_term_operations: left.max_term_operations.min(right.max_term_operations),
    }
}

fn remaining_limit(
    resource: &'static str,
    limit: usize,
    used: usize,
) -> Result<usize, CanonicalLocusTableError> {
    limit
        .checked_sub(used)
        .ok_or(CanonicalLocusTableError::ResourceLimit {
            resource,
            requested: used,
            limit,
        })
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, CanonicalLocusTableError> {
    left.checked_add(right)
        .ok_or(CanonicalLocusTableError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, CanonicalLocusTableError> {
    left.checked_mul(right)
        .ok_or(CanonicalLocusTableError::ResourceCountOverflow { resource })
}

fn checked_bounded_add(
    resource: &'static str,
    current: usize,
    increment: usize,
    limit: usize,
) -> Result<usize, CanonicalLocusTableError> {
    let requested = checked_add(resource, current, increment)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), CanonicalLocusTableError> {
    if requested > limit {
        Err(CanonicalLocusTableError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use symbolica::domains::integer::Integer;

    use super::*;
    use crate::CoefficientContext;
    use crate::parametric_coefficient::{
        ParametricArithmeticLimits, inject_polynomial_associate_native_boundary_panic_for_test,
        polynomial_associate_native_boundary_calls_for_test,
        reset_polynomial_associate_native_boundary_calls_for_test,
    };

    const TEST_SCHEMA: &str = "rustred-test-canonical-locus-owner-v1";

    fn test_context(scope: &str) -> ParametricCoefficientContext {
        ParametricCoefficientContext::try_new(&CoefficientContext::new(["d"]), scope, 1).unwrap()
    }

    fn loci(
        context: &ParametricCoefficientContext,
    ) -> (
        ParametricPolynomial,
        ParametricPolynomial,
        ParametricPolynomial,
    ) {
        let n = context.index(0).unwrap();
        let p = context.numerator_condition(&n).unwrap();
        let two_p = context
            .numerator_condition(&context.mul(&context.integer(2), &n).unwrap())
            .unwrap();
        let q = context
            .numerator_condition(&context.add(&n, &context.one()).unwrap())
            .unwrap();
        (p, two_p, q)
    }

    #[test]
    fn first_seen_associate_interning_is_transactional_and_seals_once() {
        let context = test_context("canonical-locus-first-seen");
        let (p, two_p, q) = loci(&context);
        let mut builder = CanonicalLocusTableBuilder::try_new(
            &context,
            TEST_SCHEMA,
            4,
            CanonicalLocusTableLimits::default(),
        )
        .unwrap();
        assert_eq!(
            builder.context_fingerprint.capacity(),
            context.fingerprint().len()
        );
        assert_eq!(builder.loci.capacity(), 4);
        reset_polynomial_associate_native_boundary_calls_for_test();

        let first = builder.try_intern(&context, &p).unwrap();
        assert_eq!(first.locus_ordinal(), 0);
        assert_eq!(
            first.disposition(),
            CanonicalLocusInternDisposition::Inserted
        );

        let before_panic = builder.stats();
        inject_polynomial_associate_native_boundary_panic_for_test();
        assert!(matches!(
            builder.try_intern(&context, &two_p),
            Err(CanonicalLocusTableError::ParametricCoefficient(
                ParametricCoefficientError::Symbolica(_)
            ))
        ));
        assert_eq!(builder.stats(), before_panic);
        assert_eq!(builder.loci(), &[p.clone()]);

        let outcomes = [
            builder.try_intern(&context, &two_p).unwrap(),
            builder.try_intern(&context, &q).unwrap(),
            builder.try_intern(&context, &p).unwrap(),
        ];
        assert_eq!(
            outcomes.map(CanonicalLocusInternOutcome::locus_ordinal),
            [0, 1, 0]
        );
        assert_eq!(
            outcomes.map(CanonicalLocusInternOutcome::disposition),
            [
                CanonicalLocusInternDisposition::AssociateDuplicate,
                CanonicalLocusInternDisposition::Inserted,
                CanonicalLocusInternDisposition::ExactDuplicate,
            ]
        );
        let stats = builder.stats();
        assert_eq!(stats.structural_loci(), 2);
        assert_eq!(stats.equality_comparisons(), 3);
        assert_eq!(stats.associate_comparisons(), 2);
        // The injected boundary is counted as an attempted native call; the
        // three committed operations themselves enter Symbolica twice.
        assert_eq!(polynomial_associate_native_boundary_calls_for_test(), 3);

        let owner = builder.seal().unwrap();
        assert_eq!(owner.schema(), TEST_SCHEMA);
        assert_eq!(owner.context_fingerprint(), context.fingerprint());
        assert_eq!(owner.loci(), &[p, q]);
        assert_eq!(owner.stats().structural_loci(), 2);
        assert_eq!(
            owner.context_fingerprint.capacity(),
            context.fingerprint().len()
        );
        assert_eq!(owner.loci.capacity(), 4);
        assert_eq!(
            owner.stats().retained_owned_logical_bytes(),
            owner.retained_owned_logical_byte_bound().unwrap()
        );
    }

    #[test]
    fn authenticated_copy_checks_identity_limits_and_large_gmp() {
        let context = test_context("canonical-locus-copy-large-gmp");
        let n = context.index(0).unwrap();
        let huge = (Integer::from(1) << 4096_u32) + Integer::from(19);
        let huge_coefficient = context
            .integer_exact(&huge, ParametricArithmeticLimits::default())
            .unwrap();
        let polynomial = context
            .numerator_condition(&context.mul(&huge_coefficient, &n).unwrap())
            .unwrap();
        let mut builder = CanonicalLocusTableBuilder::try_new(
            &context,
            TEST_SCHEMA,
            1,
            CanonicalLocusTableLimits::default(),
        )
        .unwrap();
        builder.try_intern(&context, &polynomial).unwrap();
        let owner = builder.seal().unwrap();

        assert_eq!(
            owner
                .try_copy_authenticated(
                    &context,
                    "wrong-canonical-locus-schema",
                    CanonicalLocusTableCopyLimits::default(),
                )
                .unwrap_err(),
            CanonicalLocusTableError::SchemaMismatch
        );
        let foreign = test_context("canonical-locus-copy-foreign");
        assert_eq!(
            owner
                .try_copy_authenticated(
                    &foreign,
                    TEST_SCHEMA,
                    CanonicalLocusTableCopyLimits::default(),
                )
                .unwrap_err(),
            CanonicalLocusTableError::ContextMismatch
        );

        let copy = owner
            .try_copy_authenticated(
                &context,
                TEST_SCHEMA,
                CanonicalLocusTableCopyLimits::default(),
            )
            .unwrap();
        assert_eq!(copy.loci(), owner.loci());
        assert_eq!(copy.stats().retained_polynomial_integer_bits(), 4097);
        assert_eq!(
            copy.context_fingerprint.capacity(),
            context.fingerprint().len()
        );
        assert_eq!(copy.loci.capacity(), copy.loci.len());
        assert_eq!(
            copy.stats().retained_owned_logical_bytes(),
            copy.retained_owned_logical_byte_bound().unwrap()
        );
        let source_retained = owner.retained_owned_logical_byte_bound().unwrap();
        let projected_copy = owner
            .projected_compact_copy_owned_logical_byte_bound()
            .unwrap();
        let copy_peak = source_retained.checked_add(projected_copy).unwrap();
        let mut exact_peak = CanonicalLocusTableCopyLimits::default();
        exact_peak.max_copy_owned_logical_peak_upper_bound = copy_peak;
        owner
            .try_copy_authenticated(&context, TEST_SCHEMA, exact_peak)
            .unwrap();
        let mut one_below_peak = exact_peak;
        one_below_peak.max_copy_owned_logical_peak_upper_bound = copy_peak - 1;
        assert!(matches!(
            owner.try_copy_authenticated(&context, TEST_SCHEMA, one_below_peak),
            Err(CanonicalLocusTableError::ResourceLimit {
                resource: "canonical locus authenticated copy owned logical peak upper bound",
                requested,
                limit,
            }) if requested == copy_peak && limit == copy_peak - 1
        ));
        let retained = copy.retained_owned_logical_byte_bound().unwrap();
        let mut one_below = CanonicalLocusTableCopyLimits::default();
        one_below.max_retained_owned_logical_bytes = retained - 1;
        assert!(matches!(
            owner.try_copy_authenticated(&context, TEST_SCHEMA, one_below),
            Err(CanonicalLocusTableError::ResourceLimit {
                resource: "canonical locus authenticated copy retained owned logical bytes",
                requested,
                limit,
            }) if requested == retained && limit == retained - 1
        ));
        // A failed borrowed copy never consumes or mutates the source owner.
        assert_eq!(owner.loci(), &[polynomial]);
    }

    #[test]
    fn native_cross_aggregate_is_cumulative_transactional_and_owns_ties() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(["d"]),
            "canonical-locus-native-cross-aggregate",
            2,
        )
        .unwrap();
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let polynomial = |scale: i64| {
            context
                .numerator_condition(
                    &context
                        .add(&n0, &context.mul(&context.integer(scale), &n1).unwrap())
                        .unwrap(),
                )
                .unwrap()
        };
        let loci = [polynomial(1), polynomial(2), polynomial(3)];

        let build = |limits: CanonicalLocusTableLimits| {
            let mut builder =
                CanonicalLocusTableBuilder::try_new(&context, TEST_SCHEMA, 3, limits).unwrap();
            for locus in &loci {
                builder.try_intern(&context, locus)?;
            }
            Ok::<_, CanonicalLocusTableError>(builder)
        };
        let baseline = build(CanonicalLocusTableLimits::default()).unwrap();
        let total = baseline.stats().associate_native_cross_term_pairs();
        assert!(total > 0);
        assert!(baseline.stats().associate_comparisons() >= 3);

        let mut exact = CanonicalLocusTableLimits::default();
        exact.max_associate_native_cross_term_pairs = total;
        assert_eq!(
            build(exact)
                .unwrap()
                .stats()
                .associate_native_cross_term_pairs(),
            total
        );

        let mut one_below = CanonicalLocusTableLimits::default();
        one_below.max_associate_native_cross_term_pairs = total - 1;
        let mut builder =
            CanonicalLocusTableBuilder::try_new(&context, TEST_SCHEMA, 3, one_below).unwrap();
        builder.try_intern(&context, &loci[0]).unwrap();
        builder.try_intern(&context, &loci[1]).unwrap();
        let before = builder.stats();
        assert!(matches!(
            builder.try_intern(&context, &loci[2]),
            Err(CanonicalLocusTableError::ResourceLimit {
                resource: "canonical locus table associate native cross term pairs",
                requested,
                limit,
            }) if requested == total && limit == total - 1
        ));
        assert_eq!(builder.stats(), before);
        assert_eq!(builder.loci(), &loci[..2]);

        let per_comparison = total / baseline.stats().associate_comparisons();
        assert!(per_comparison > 0);
        let mut equality_tie = CanonicalLocusTableLimits::default();
        equality_tie.max_associate_native_cross_term_pairs = per_comparison - 1;
        equality_tie.associate.max_native_cross_term_pairs = per_comparison - 1;
        let mut tie_builder =
            CanonicalLocusTableBuilder::try_new(&context, TEST_SCHEMA, 2, equality_tie).unwrap();
        tie_builder.try_intern(&context, &loci[0]).unwrap();
        assert!(matches!(
            tie_builder.try_intern(&context, &loci[1]),
            Err(CanonicalLocusTableError::ResourceLimit {
                resource: "canonical locus table associate native cross term pairs",
                ..
            })
        ));

        let mut child_owned = CanonicalLocusTableLimits::default();
        child_owned.associate.max_native_cross_term_pairs = per_comparison - 1;
        let mut child_builder =
            CanonicalLocusTableBuilder::try_new(&context, TEST_SCHEMA, 2, child_owned).unwrap();
        child_builder.try_intern(&context, &loci[0]).unwrap();
        assert!(matches!(
            child_builder.try_intern(&context, &loci[1]),
            Err(CanonicalLocusTableError::ParametricCoefficient(
                ParametricCoefficientError::ResourceLimit {
                    resource: "polynomial-associate native cross term pairs",
                    ..
                }
            ))
        ));
    }
}
