use super::rank::checked_scalar_product_count;
use super::{IspCompletion, IspCompletionError, IspCompletionLimits};
use crate::algebra::matrix::DEFAULT_MAX_INPUT_RETAINED_BYTES;
use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::AffineDenominator;

#[cfg(test)]
mod coefficient_limit_tests {
    use super::*;

    fn one_loop_completion(
        coefficient: Coefficient,
        limits: IspCompletionLimits,
    ) -> Result<IspCompletion, IspCompletionError> {
        let context = CoefficientContext::new(["d"]);
        IspCompletion::try_new_with_limits(
            "automatic-isp-coefficient-census",
            vec!["k".to_owned()],
            Vec::new(),
            context.clone(),
            context.parameter("d").unwrap(),
            vec![AffineDenominator::new(context.zero(), vec![coefficient])],
            Vec::new(),
            vec![context.zero()],
            limits,
        )
    }

    #[test]
    fn coefficient_term_census_accepts_exact_boundary_and_rejects_one_below() {
        let context = CoefficientContext::new(["d"]);
        // Every rational polynomial owns a numerator and a denominator term.
        let exact = context.one().numerator.nterms() + context.one().denominator.nterms();
        assert_eq!(exact, 2);

        let mut limits = IspCompletionLimits::default();
        limits.max_rank_coefficient_terms = exact;
        one_loop_completion(context.one(), limits).unwrap();

        limits.max_rank_coefficient_terms = exact - 1;
        assert!(matches!(
            one_loop_completion(context.one(), limits),
            Err(IspCompletionError::ResourceLimit {
                resource: "automatic ISP rank coefficient terms",
                requested,
                limit,
            }) if requested == exact && limit == exact - 1
        ));
    }

    #[test]
    fn coefficient_byte_census_bounds_a_large_integer_before_native_copy() {
        let context = CoefficientContext::new(["d"]);
        let coefficient = context.coefficient_fixture("123456789012345678901234567890123456789");
        let exact = coefficient.to_string().len();
        assert!(exact > 32);

        let mut limits = IspCompletionLimits::default();
        limits.max_rank_coefficient_bytes = exact;
        one_loop_completion(coefficient.clone(), limits).unwrap();

        limits.max_rank_coefficient_bytes = exact - 1;
        assert!(matches!(
            one_loop_completion(coefficient, limits),
            Err(IspCompletionError::ResourceLimit {
                resource: "automatic ISP rank coefficient bytes",
                requested,
                limit,
            }) if requested > limit && limit == exact - 1
        ));
    }

    #[test]
    fn native_rank_input_and_output_payloads_have_separate_retained_byte_limits() {
        let context = CoefficientContext::new(["d"]);
        let mut limits = IspCompletionLimits::default();
        limits.max_rank_input_retained_bytes = 0;
        assert!(matches!(
            one_loop_completion(context.one(), limits),
            Err(IspCompletionError::ResourceLimit {
                resource: "automatic ISP rank native input retained bytes",
                requested,
                limit: 0,
            }) if requested > 0
        ));

        limits.max_rank_input_retained_bytes = DEFAULT_MAX_INPUT_RETAINED_BYTES;
        limits.max_rank_output_retained_bytes = 0;
        assert!(matches!(
            one_loop_completion(context.one(), limits),
            Err(IspCompletionError::ResourceLimit {
                resource: "automatic ISP rank native output retained bytes",
                requested,
                limit: 0,
            }) if requested > 0
        ));
    }

    #[test]
    fn native_rank_operation_limit_is_cumulative_across_candidate_tests() {
        let context = CoefficientContext::new(["d", "s"]);
        let mut limits = IspCompletionLimits::default();
        // The initial [1,0] rank costs one native inverse.  Scouting the
        // dependent e0 row needs four more native scalar calls, so a cumulative
        // limit of four must fail at requested operation five.
        limits.max_rank_operations = 4;
        let error = IspCompletion::try_new_with_limits(
            "automatic-isp-native-operation-census",
            vec!["k".to_owned()],
            vec!["p".to_owned()],
            context.clone(),
            context.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                context.zero(),
                vec![context.one(), context.zero()],
            )],
            vec![vec![context.parameter("s").unwrap()]],
            vec![context.zero()],
            limits,
        )
        .unwrap_err();
        assert_eq!(
            error,
            IspCompletionError::ResourceLimit {
                resource: "automatic ISP rank operations",
                requested: 5,
                limit: 4,
            }
        );
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn triangular_count_avoids_a_false_intermediate_product_overflow() {
        let loops = 6_000_000_000usize;
        assert_eq!(
            checked_scalar_product_count(loops, 0).unwrap(),
            18_000_000_003_000_000_000usize
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentinel_retains_authored_rows_and_appends_the_missing_two_loop_isp() {
        let context = CoefficientContext::new(["d", "m0", "m1", "nu0", "nu1"]);
        let zero = context.zero();
        let one = context.one();
        let input_denominators = vec![
            AffineDenominator::new(
                context.coefficient_fixture("-m0"),
                vec![one.clone(), zero.clone(), zero.clone()],
            ),
            AffineDenominator::new(
                context.coefficient_fixture("-m1"),
                vec![zero.clone(), zero.clone(), one],
            ),
        ];
        let input_shifts = vec![
            context.parameter("nu0").unwrap(),
            context.parameter("nu1").unwrap(),
        ];

        let completion = IspCompletion::try_new(
            "two-loop-vacuum-isp-sentinel",
            vec!["k0".into(), "k1".into()],
            Vec::new(),
            context.clone(),
            context.parameter("d").unwrap(),
            input_denominators.clone(),
            Vec::new(),
            input_shifts.clone(),
        )
        .unwrap();

        assert_eq!(completion.input_denominator_count(), 2);
        assert_eq!(completion.appended_coordinate_ordinals(), &[1]);
        assert_eq!(completion.rank_progression(), &[2, 3]);
        assert_eq!(&completion.family().denominators()[..2], input_denominators);
        assert_eq!(&completion.family().power_shifts()[..2], input_shifts);
        assert!(completion.family().denominators()[2].constant().is_zero());
        assert_eq!(
            completion.family().denominators()[2].coefficients(),
            &[context.zero(), context.one(), context.zero()]
        );
        assert!(completion.family().power_shifts()[2].is_zero());
    }

    #[test]
    fn completion_exposes_the_native_rank_work_census() {
        let context = CoefficientContext::new(["d", "m"]);
        let completion = IspCompletion::try_new(
            "isp-work-census",
            vec!["k".into()],
            Vec::new(),
            context.clone(),
            context.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                context.coefficient_fixture("-m"),
                vec![context.one()],
            )],
            Vec::new(),
            vec![context.zero()],
        )
        .unwrap();
        assert_eq!(completion.stats().rank_tests(), 1);
        assert_eq!(completion.stats().rank_operations(), 1);
        assert_eq!(completion.stats().appended_isps(), 0);
    }
}
