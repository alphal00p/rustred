//! Closed policy for refusing admission of the next native guard operation.
use super::IndexedAlgebraError;

/// A prior GCD/equation may already have run; this does not mean zero native
/// work. Input/output/replay caps and non-ResourceLimit faults are excluded.
/// Callers decide whether their particular operation is optional or retryable.
pub(crate) fn is_native_guard_preflight_refusal(error: &IndexedAlgebraError) -> bool {
    matches!(
        error,
        IndexedAlgebraError::ResourceLimit {
            resource: "guard univariate degree"
                | "guard gcd/factor work"
                | "guard factor variables"
                | "guard factor per-variable degree"
                | "guard factor total degree"
                | "guard factor dense slots"
                | "guard factor recombination subsets"
                | "guard prospective factor terms"
                | "guard prospective factor integer bits"
                | "guard separable factor work",
            ..
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_guard_preflight_policy_is_closed() {
        for resource in [
            "guard univariate degree",
            "guard gcd/factor work",
            "guard factor variables",
            "guard factor per-variable degree",
            "guard factor total degree",
            "guard factor dense slots",
            "guard factor recombination subsets",
            "guard prospective factor terms",
            "guard prospective factor integer bits",
            "guard separable factor work",
        ] {
            assert!(is_native_guard_preflight_refusal(
                &IndexedAlgebraError::ResourceLimit {
                    resource,
                    requested: 10,
                    limit: 1
                }
            ));
        }
        for resource in [
            "guard coefficient split input terms",
            "guard coefficient equations",
            "guard factor terms",
            "guard factor integer bits",
            "guard univariate coefficient bits",
            "guard exact-hyperplane replay work",
            "guard exact-hyperplane replay substitutions",
            "guard exact-hyperplane replay terms",
            "unrecognized future admission",
        ] {
            assert!(!is_native_guard_preflight_refusal(
                &IndexedAlgebraError::ResourceLimit {
                    resource,
                    requested: 10,
                    limit: 1
                }
            ));
        }
        for error in [
            IndexedAlgebraError::ResourceCountOverflow {
                resource: "guard separable factor work",
            },
            IndexedAlgebraError::AllocationFailure {
                resource: "guard prospective factor terms",
                requested: 10,
            },
            IndexedAlgebraError::WrongContext,
            IndexedAlgebraError::ZeroDenominator,
            IndexedAlgebraError::ExactAlgebra(crate::algebra::ExactAlgebraError::ResourceLimit {
                resource: "guard separable factor work",
                requested: 10,
                limit: 1,
            }),
            IndexedAlgebraError::Symbolica("backend failure".into()),
        ] {
            assert!(!is_native_guard_preflight_refusal(&error));
        }
    }
}
