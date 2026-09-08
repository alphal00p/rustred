use std::cmp::Ordering;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::domains::finite_field::{FiniteFieldCore, ToFiniteField, Zp64};
use symbolica::prelude::{Integer, Ring, Z};

use crate::algebra::indexed::{ceil_log2, integer_magnitude_bits};
use crate::algebra::{CoefficientPolynomial, IndexedCoefficientContext, IndexedPolynomial};
use crate::foundry::completion::stratum::{
    DecoratedStratum, GuardBranch, GuardBranchIdentity, GuardPredicateAuthority,
};

use super::super::CoefficientIdealGuardAtom;
use super::model::ExactGuardProbeValue;
use super::{
    ExactGuardPredicateCatalog, ExactGuardPredicateCatalogLimits, ExactGuardProbeError,
    ExactGuardProbeLimits, ExactGuardProbeWitness,
};

const CATALOG_PREDICATES: &str = "predicate catalog entries";
const CATALOG_INPUT_TERMS: &str = "predicate catalog input terms";
const CATALOG_IDENTITY_BYTES: &str = "predicate catalog identity bytes";
const GUARDS: &str = "decorated-stratum guards";
const INPUT_TERMS: &str = "guard input terms";
const INDEX_SPECIALIZATION_POWER_OPERATIONS: &str = "index-specialization power operations";
const BASE_EVALUATION_POWER_OPERATIONS: &str = "base-evaluation power operations";
const RETAINED_COORDINATES: &str = "retained exact coordinates";
const RETAINED_VALUE_BITS: &str = "retained exact guard-value bits";
const CATALOG_STORAGE: &str = "predicate catalog storage";
const VALUE_STORAGE: &str = "exact guard-value storage";
const BASE_POINT_STORAGE: &str = "exact base-point storage";
const INDEX_POINT_STORAGE: &str = "exact index-point storage";

impl ExactGuardPredicateCatalog {
    /// Compile exact guard polynomials which are already expressed in the
    /// target/index coordinates used by their future decorated strata.
    pub(crate) fn try_from_pulled_back(
        context: &IndexedCoefficientContext,
        polynomials: impl IntoIterator<Item = IndexedPolynomial>,
        limits: ExactGuardPredicateCatalogLimits,
    ) -> Result<Self, ExactGuardProbeError> {
        let mut atoms = Vec::new();
        let mut input_terms = 0usize;
        let mut identity_bytes = 0usize;
        for polynomial in polynomials {
            let requested = checked_add(CATALOG_PREDICATES, atoms.len(), 1)?;
            check_limit(CATALOG_PREDICATES, requested, limits.max_predicates)?;
            input_terms = checked_add(CATALOG_INPUT_TERMS, input_terms, polynomial.raw().nterms())?;
            check_limit(CATALOG_INPUT_TERMS, input_terms, limits.max_input_terms)?;

            let atom =
                CoefficientIdealGuardAtom::try_from_pulled_back(context, polynomial, limits.atom)?;
            identity_bytes = checked_add(
                CATALOG_IDENTITY_BYTES,
                identity_bytes,
                atom.predicate().representative_identity().predicate().len(),
            )?;
            check_limit(
                CATALOG_IDENTITY_BYTES,
                identity_bytes,
                limits.max_predicate_identity_bytes,
            )?;
            try_reserve(&mut atoms, 1, CATALOG_STORAGE)?;
            atoms.push(atom);
        }
        atoms.sort_unstable_by(compare_atoms);
        let mut retained = Vec::new();
        try_reserve(&mut retained, atoms.len(), CATALOG_STORAGE)?;
        for atom in atoms {
            if let Some(previous) = retained.last()
                && same_representative(previous, &atom)
            {
                if previous != &atom {
                    return Err(ExactGuardProbeError::Invariant {
                        detail: "equal guard identities retained different exact atom payloads",
                    });
                }
                continue;
            }
            retained.push(atom);
        }
        Ok(Self {
            context_fingerprint: context.fingerprint_owner(),
            entries: Arc::from(retained),
        })
    }

    pub(super) fn find(
        &self,
        identity: &GuardBranchIdentity,
    ) -> Option<&CoefficientIdealGuardAtom> {
        self.entries
            .binary_search_by(|atom| {
                compare_identity(atom.predicate().representative_identity(), identity)
            })
            .ok()
            .and_then(|ordinal| self.entries.get(ordinal))
    }
}

impl ExactGuardProbeWitness {
    /// Prove every branch at one exact integer point and one exact numeric base
    /// sample. No modular residue participates in this construction.
    pub(crate) fn try_new(
        context: &IndexedCoefficientContext,
        stratum: &DecoratedStratum,
        base_parameters: &[i64],
        index_anchor: &[i64],
        catalog: &ExactGuardPredicateCatalog,
        limits: ExactGuardProbeLimits,
    ) -> Result<Self, ExactGuardProbeError> {
        if stratum.guards().is_empty() {
            return Err(ExactGuardProbeError::GuardBlindStratum);
        }
        if stratum.context_fingerprint() != context.fingerprint() {
            return Err(ExactGuardProbeError::WrongContext);
        }
        if catalog.context_fingerprint() != context.fingerprint() {
            return Err(ExactGuardProbeError::WrongCatalogContext);
        }
        let expected_base = context.base().parameter_names().len();
        if base_parameters.len() != expected_base {
            return Err(ExactGuardProbeError::WrongBaseParameterArity {
                expected: expected_base,
                actual: base_parameters.len(),
            });
        }
        let expected_indices = context.index_count();
        if index_anchor.len() != expected_indices {
            return Err(ExactGuardProbeError::WrongIndexArity {
                expected: expected_indices,
                actual: index_anchor.len(),
            });
        }
        for (position, (&index, bounds)) in index_anchor
            .iter()
            .zip(stratum.domain().bounds())
            .enumerate()
        {
            if !bounds.contains(index) {
                return Err(ExactGuardProbeError::IndexOutsideStratum {
                    position,
                    index,
                    lower: bounds.lower(),
                    upper: bounds.upper(),
                });
            }
        }
        check_limit(GUARDS, stratum.guards().len(), limits.max_guards)?;
        let coordinates = checked_add(
            RETAINED_COORDINATES,
            base_parameters.len(),
            index_anchor.len(),
        )?;
        check_limit(
            RETAINED_COORDINATES,
            coordinates,
            limits.max_retained_coordinates,
        )?;

        preflight_all_guards(
            context,
            stratum,
            base_parameters,
            index_anchor,
            catalog,
            limits,
        )?;

        let base_point = try_integer_point(base_parameters)?;
        let mut values = Vec::new();
        try_reserve(&mut values, stratum.guards().len(), VALUE_STORAGE)?;
        for (guard_ordinal, guard) in stratum.guards().iter().enumerate() {
            let atom = catalog
                .find(guard)
                .ok_or(ExactGuardProbeError::MissingPredicatePayload { guard_ordinal })?;
            let specialized = context.specialize_polynomial_sealed(
                atom.predicate().representative_guard(),
                index_anchor,
                limits.indexed_algebra,
            )?;
            let actual = if specialized.is_zero() {
                GuardBranch::Zero
            } else {
                GuardBranch::NonZero
            };
            if actual != guard.branch() {
                return Err(ExactGuardProbeError::PredicateBranchMismatch {
                    guard_ordinal,
                    required: guard.branch(),
                    actual,
                });
            }
            match actual {
                GuardBranch::Zero => values.push(ExactGuardProbeValue::IdenticallyZero),
                GuardBranch::NonZero => {
                    let prospective_bits = exact_value_bit_bound(
                        atom.predicate().representative_guard().raw(),
                        base_parameters,
                        index_anchor,
                    )?;
                    let value = catch_unwind(AssertUnwindSafe(|| {
                        specialized.evaluate_with_coeff_map(
                            |coefficient| coefficient.clone(),
                            &base_point,
                            &Z,
                        )
                    }))
                    .map_err(|_| ExactGuardProbeError::SymbolicaPanic {
                        operation: "evaluating an exact guard at its base sample",
                    })?;
                    if Z.is_zero(&value) {
                        return Err(ExactGuardProbeError::BaseSampleOnNonzeroGuardWall {
                            guard_ordinal,
                        });
                    }
                    if usize::try_from(integer_magnitude_bits(&value)).unwrap_or(usize::MAX)
                        > prospective_bits
                    {
                        return Err(ExactGuardProbeError::Invariant {
                            detail: "exact guard value escaped its prospective bit bound",
                        });
                    }
                    values.push(ExactGuardProbeValue::NonZero(value));
                }
            }
        }

        Ok(Self {
            context_fingerprint: context.fingerprint_owner(),
            stratum_id: stratum.id().clone(),
            base_parameters: try_arc_i64(base_parameters, BASE_POINT_STORAGE)?,
            index_anchor: try_arc_i64(index_anchor, INDEX_POINT_STORAGE)?,
            values: Arc::from(values),
        })
    }

    /// Reject primes which erase a branch known to be nonzero at this exact
    /// integer/base point. Such a rejection is retryable and grants no new
    /// Zero-branch authority.
    pub(crate) fn try_validate_modulus(&self, field: &Zp64) -> Result<(), ExactGuardProbeError> {
        for (guard_ordinal, value) in self.values.iter().enumerate() {
            if let ExactGuardProbeValue::NonZero(value) = value
                && field.is_zero(&value.to_finite_field(field))
            {
                return Err(ExactGuardProbeError::UnluckyGuardPrime {
                    guard_ordinal,
                    modulus: field.get_prime(),
                });
            }
        }
        Ok(())
    }
}

fn preflight_all_guards(
    context: &IndexedCoefficientContext,
    stratum: &DecoratedStratum,
    base_parameters: &[i64],
    index_anchor: &[i64],
    catalog: &ExactGuardPredicateCatalog,
    limits: ExactGuardProbeLimits,
) -> Result<(), ExactGuardProbeError> {
    let mut input_terms = 0usize;
    let mut index_operations = 0usize;
    let mut base_operations = 0usize;
    let mut retained_value_bits = 0usize;
    for (guard_ordinal, guard) in stratum.guards().iter().enumerate() {
        if guard.authority() != GuardPredicateAuthority::IndexedPolynomial {
            return Err(ExactGuardProbeError::UnsupportedPredicateAuthority {
                guard_ordinal,
                authority: guard.authority(),
            });
        }
        let atom = catalog
            .find(guard)
            .ok_or(ExactGuardProbeError::MissingPredicatePayload { guard_ordinal })?;
        if atom.context_fingerprint() != context.fingerprint() {
            return Err(ExactGuardProbeError::WrongCatalogContext);
        }
        let terms = atom.predicate().input_terms();
        input_terms = checked_add(INPUT_TERMS, input_terms, terms)?;
        check_limit(INPUT_TERMS, input_terms, limits.max_input_terms)?;
        index_operations = checked_add(
            INDEX_SPECIALIZATION_POWER_OPERATIONS,
            index_operations,
            checked_mul(
                INDEX_SPECIALIZATION_POWER_OPERATIONS,
                terms,
                context.index_count(),
            )?,
        )?;
        check_limit(
            INDEX_SPECIALIZATION_POWER_OPERATIONS,
            index_operations,
            limits.max_index_specialization_power_operations,
        )?;
        base_operations = checked_add(
            BASE_EVALUATION_POWER_OPERATIONS,
            base_operations,
            checked_mul(
                BASE_EVALUATION_POWER_OPERATIONS,
                terms,
                context.base().parameter_names().len(),
            )?,
        )?;
        check_limit(
            BASE_EVALUATION_POWER_OPERATIONS,
            base_operations,
            limits.max_base_evaluation_power_operations,
        )?;
        if guard.branch() == GuardBranch::NonZero {
            retained_value_bits = checked_add(
                RETAINED_VALUE_BITS,
                retained_value_bits,
                exact_value_bit_bound(
                    atom.predicate().representative_guard().raw(),
                    base_parameters,
                    index_anchor,
                )?,
            )?;
            check_limit(
                RETAINED_VALUE_BITS,
                retained_value_bits,
                limits.max_retained_exact_value_bits,
            )?;
        }
    }
    Ok(())
}

fn exact_value_bit_bound(
    polynomial: &CoefficientPolynomial,
    base_parameters: &[i64],
    index_anchor: &[i64],
) -> Result<usize, ExactGuardProbeError> {
    let coordinate_count = checked_add(
        RETAINED_COORDINATES,
        base_parameters.len(),
        index_anchor.len(),
    )?;
    if polynomial.nvars() != coordinate_count {
        return Err(ExactGuardProbeError::Invariant {
            detail: "guard polynomial variable arity differs from its exact probe",
        });
    }
    let mut largest = 0usize;
    for (coefficient, exponents) in polynomial
        .coefficients
        .iter()
        .zip(polynomial.exponents_iter())
    {
        let mut bits = usize::try_from(integer_magnitude_bits(coefficient)).map_err(|_| {
            ExactGuardProbeError::ResourceCountOverflow {
                resource: RETAINED_VALUE_BITS,
            }
        })?;
        for (&value, &exponent) in base_parameters.iter().chain(index_anchor).zip(exponents) {
            if exponent == 0 || bits == 0 {
                continue;
            }
            let magnitude = value.unsigned_abs();
            if magnitude == 0 {
                bits = 0;
                break;
            }
            if magnitude != 1 {
                let value_bits =
                    usize::try_from(i64::BITS - magnitude.leading_zeros()).map_err(|_| {
                        ExactGuardProbeError::ResourceCountOverflow {
                            resource: RETAINED_VALUE_BITS,
                        }
                    })?;
                bits = checked_add(
                    RETAINED_VALUE_BITS,
                    bits,
                    checked_mul(RETAINED_VALUE_BITS, value_bits, usize::from(exponent))?,
                )?;
            }
        }
        largest = largest.max(bits);
    }
    checked_add(RETAINED_VALUE_BITS, largest, ceil_log2(polynomial.nterms()))
}

fn compare_atoms(left: &CoefficientIdealGuardAtom, right: &CoefficientIdealGuardAtom) -> Ordering {
    compare_identity(
        left.predicate().representative_identity(),
        right.predicate().representative_identity(),
    )
}

fn compare_identity(left: &GuardBranchIdentity, right: &GuardBranchIdentity) -> Ordering {
    left.authority()
        .cmp(&right.authority())
        .then_with(|| left.predicate().cmp(right.predicate()))
}

fn same_representative(
    left: &CoefficientIdealGuardAtom,
    right: &CoefficientIdealGuardAtom,
) -> bool {
    left.predicate()
        .representative_identity()
        .same_predicate(right.predicate().representative_identity())
}

fn try_integer_point(values: &[i64]) -> Result<Vec<Integer>, ExactGuardProbeError> {
    let mut point = Vec::new();
    try_reserve(&mut point, values.len(), BASE_POINT_STORAGE)?;
    point.extend(values.iter().copied().map(Integer::from));
    Ok(point)
}

fn try_arc_i64(values: &[i64], resource: &'static str) -> Result<Arc<[i64]>, ExactGuardProbeError> {
    let mut retained = Vec::new();
    try_reserve(&mut retained, values.len(), resource)?;
    retained.extend_from_slice(values);
    Ok(Arc::from(retained))
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ExactGuardProbeError> {
    left.checked_add(right)
        .ok_or(ExactGuardProbeError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ExactGuardProbeError> {
    left.checked_mul(right)
        .ok_or(ExactGuardProbeError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ExactGuardProbeError> {
    if requested > limit {
        Err(ExactGuardProbeError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn try_reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    resource: &'static str,
) -> Result<(), ExactGuardProbeError> {
    let requested = checked_add(resource, values.len(), additional)?;
    values
        .try_reserve_exact(additional)
        .map_err(|_| ExactGuardProbeError::AllocationFailure {
            resource,
            requested,
        })
}
