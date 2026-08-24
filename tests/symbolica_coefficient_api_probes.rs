//! Regression probes for the Symbolica APIs that RustRed's guarded coefficient
//! layer will rely on.
//!
//! These helpers intentionally live in the test target until the production
//! `CoeffContext` exists.  They encode the safety preconditions identified in
//! `docs/research/symbolica_rust_api_for_litered.md` instead of exposing
//! Symbolica's panic-prone convenience paths directly.

use std::collections::HashSet;
use std::sync::Arc;

use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::id::Evaluate;
use symbolica::prelude::*;

type IntPoly = MultivariatePolynomial<IntegerRing, u16>;
type IntRat = RationalPolynomial<IntegerRing, u16>;

fn checked_polynomial_from_terms(
    coefficients: Vec<Integer>,
    exponents: Vec<u16>,
    variables: Arc<Vec<PolyVariable>>,
) -> Result<IntPoly, String> {
    if coefficients.is_empty() {
        if !exponents.is_empty() {
            return Err("an empty coefficient list must have no exponent rows".into());
        }

        // `MultivariatePolynomial::from_coefficient_list` divides by
        // `coefficients.len()`.  Construct zero directly so that the variable
        // map is retained without entering that panic path.
        return Ok(IntPoly::new(&Z, None, variables));
    }

    let expected_exponents = coefficients
        .len()
        .checked_mul(variables.len())
        .ok_or_else(|| "term/exponent dimensions overflowed usize".to_string())?;
    if exponents.len() != expected_exponents {
        return Err(format!(
            "expected {expected_exponents} flattened exponents, got {}",
            exponents.len()
        ));
    }

    Ok(IntPoly::from_coefficient_list(
        coefficients,
        exponents,
        variables,
        &Z,
    ))
}

fn checked_remap_polynomial(
    source: &IntPoly,
    target_variables: Arc<Vec<PolyVariable>>,
) -> Result<IntPoly, String> {
    if source.nvars() != target_variables.len() {
        return Err("source and target maps have different lengths".into());
    }

    let unique_targets: HashSet<_> = target_variables.iter().cloned().collect();
    if unique_targets.len() != target_variables.len() {
        return Err("target variable map contains duplicates".into());
    }

    let source_to_target = source
        .get_vars_ref()
        .iter()
        .map(|source_variable| {
            target_variables
                .iter()
                .position(|target_variable| target_variable == source_variable)
                .ok_or_else(|| format!("target map is missing {source_variable}"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut remapped_exponents = vec![0; source.nterms() * source.nvars()];
    for (term, source_exponents) in source.exponents_iter().enumerate() {
        let target_exponents =
            &mut remapped_exponents[term * source.nvars()..(term + 1) * source.nvars()];
        for (source_position, exponent) in source_exponents.iter().enumerate() {
            target_exponents[source_to_target[source_position]] = *exponent;
        }
    }

    checked_polynomial_from_terms(
        source.coefficients.clone(),
        remapped_exponents,
        target_variables,
    )
}

fn checked_remap_rational(
    source: &IntRat,
    target_variables: Arc<Vec<PolyVariable>>,
) -> Result<IntRat, String> {
    if source.numerator.get_vars_ref() != source.denominator.get_vars_ref() {
        return Err("rational numerator and denominator maps differ".into());
    }

    let numerator = checked_remap_polynomial(&source.numerator, target_variables.clone())?;
    let denominator = checked_remap_polynomial(&source.denominator, target_variables)?;
    if denominator.is_zero() {
        return Err("a rational polynomial cannot have a zero denominator".into());
    }

    Ok(IntRat::from_num_den(numerator, denominator, &Z, true))
}

#[derive(Debug)]
struct CheckedSubstitution {
    value: Option<IntRat>,
    /// Denominators whose non-vanishing is required before cancellation.
    guards: Vec<IntPoly>,
}

fn checked_substitute_rational(
    source: &IntRat,
    images: &[IntRat],
    target_variables: Arc<Vec<PolyVariable>>,
) -> Result<CheckedSubstitution, String> {
    if source.numerator.get_vars_ref() != source.denominator.get_vars_ref() {
        return Err("source numerator and denominator maps differ".into());
    }
    if images.len() != source.numerator.nvars() {
        return Err(format!(
            "expected {} substitution images, got {}",
            source.numerator.nvars(),
            images.len()
        ));
    }

    let mut guards = Vec::with_capacity(images.len() + 1);
    for image in images {
        if image.numerator.get_vars_ref() != target_variables.as_slice()
            || image.denominator.get_vars_ref() != target_variables.as_slice()
        {
            return Err("a substitution image does not use the target map".into());
        }

        guards.push(image.denominator.clone());
        if image.denominator.is_zero() {
            return Ok(CheckedSubstitution {
                value: None,
                guards,
            });
        }
    }

    let base = IntPoly::new(&Z, None, target_variables.clone());
    let target_field = RationalPolynomialField::<IntegerRing, u16>::new(Z);
    let mapped_numerator = source.numerator.evaluate_with_coeff_map(
        |coefficient| IntRat::from(base.constant(coefficient.clone())),
        images,
        &target_field,
    );
    let mapped_denominator = source.denominator.evaluate_with_coeff_map(
        |coefficient| IntRat::from(base.constant(coefficient.clone())),
        images,
        &target_field,
    );

    // This is the source denominator after substitution and before the final
    // fraction-field division can cancel it.  It is part of the rule domain.
    guards.push(mapped_denominator.numerator.clone());

    // RP-SUB-2: keep checked division in trait method resolution.  Replacing
    // this with `/` would turn a failed guard into a panic or invalid result.
    let value = target_field.try_div(&mapped_numerator, &mapped_denominator);
    if let Some(value) = &value {
        if value.numerator.get_vars_ref() != target_variables.as_slice()
            || value.denominator.get_vars_ref() != target_variables.as_slice()
        {
            return Err("Symbolica changed the target map during substitution".into());
        }
    }

    Ok(CheckedSubstitution { value, guards })
}

#[test]
fn map_1_checked_construction_and_arbitrary_permutation() {
    let (x, y, z) = symbol!(
        "rustred::probe::map_x",
        "rustred::probe::map_y",
        "rustred::probe::map_z"
    );
    let source_variables = Arc::new(vec![x.into(), y.into(), z.into()]);
    let target_variables = Arc::new(vec![z.into(), x.into(), y.into()]);

    let zero = checked_polynomial_from_terms(vec![], vec![], source_variables.clone()).unwrap();
    assert!(zero.is_zero());
    assert_eq!(zero.get_vars_ref(), source_variables.as_slice());
    assert!(
        checked_polynomial_from_terms(vec![], vec![0], source_variables.clone()).is_err(),
        "invalid empty input must be rejected before Symbolica's division-by-zero path"
    );

    // 2*x^2*y - 3*z + 5.  The input rows are deliberately unordered;
    // `from_coefficient_list` must canonicalize terms without changing the map.
    let source = checked_polynomial_from_terms(
        vec![2.into(), (-3).into(), 5.into()],
        vec![2, 1, 0, 0, 0, 1, 0, 0, 0],
        source_variables.clone(),
    )
    .unwrap();
    assert_eq!(source.get_vars_ref(), source_variables.as_slice());

    let remapped = checked_remap_polynomial(&source, target_variables.clone()).unwrap();
    let expected = checked_polynomial_from_terms(
        vec![2.into(), (-3).into(), 5.into()],
        vec![0, 2, 1, 1, 0, 0, 0, 0, 0],
        target_variables.clone(),
    )
    .unwrap();
    assert_eq!(remapped, expected);
    assert_eq!(remapped.get_vars_ref(), target_variables.as_slice());

    let remapped_zero = checked_remap_polynomial(&zero, target_variables.clone()).unwrap();
    assert!(remapped_zero.is_zero());
    assert_eq!(remapped_zero.get_vars_ref(), target_variables.as_slice());
}

#[test]
fn map_1_remaps_both_rational_parts_to_one_canonical_map() {
    let (x, y, z) = symbol!(
        "rustred::probe::rp_map_x",
        "rustred::probe::rp_map_y",
        "rustred::probe::rp_map_z"
    );
    let source_variables = Arc::new(vec![x.into(), y.into(), z.into()]);
    let target_variables = Arc::new(vec![y.into(), z.into(), x.into()]);

    let numerator = checked_polynomial_from_terms(
        vec![1.into(), 2.into()],
        vec![1, 0, 1, 0, 1, 0],
        source_variables.clone(),
    )
    .unwrap();
    let denominator = checked_polynomial_from_terms(
        vec![1.into(), (-1).into(), 3.into()],
        vec![0, 1, 0, 0, 0, 1, 0, 0, 0],
        source_variables,
    )
    .unwrap();
    let source = IntRat::from_num_den(numerator, denominator, &Z, true);

    let remapped = checked_remap_rational(&source, target_variables.clone()).unwrap();
    let expected_numerator = checked_polynomial_from_terms(
        vec![1.into(), 2.into()],
        vec![0, 1, 1, 1, 0, 0],
        target_variables.clone(),
    )
    .unwrap();
    let expected_denominator = checked_polynomial_from_terms(
        vec![1.into(), (-1).into(), 3.into()],
        vec![1, 0, 0, 0, 1, 0, 0, 0, 0],
        target_variables.clone(),
    )
    .unwrap();
    let expected = IntRat::from_num_den(expected_numerator, expected_denominator, &Z, true);

    assert_eq!(remapped, expected);
    assert_eq!(
        remapped.numerator.get_vars_ref(),
        target_variables.as_slice()
    );
    assert_eq!(
        remapped.denominator.get_vars_ref(),
        target_variables.as_slice()
    );
}

#[test]
fn rp_sub_1_evaluates_polynomials_in_the_rational_polynomial_field() {
    let (x, y, u, v) = symbol!(
        "rustred::probe::sub_x",
        "rustred::probe::sub_y",
        "rustred::probe::sub_u",
        "rustred::probe::sub_v"
    );
    let source_variables = Arc::new(vec![x.into(), y.into()]);
    let target_variables = Arc::new(vec![u.into(), v.into()]);

    // (x + 2*y)/(x - y), specialized with x=u/(v+1), y=v.
    let source_numerator = checked_polynomial_from_terms(
        vec![1.into(), 2.into()],
        vec![1, 0, 0, 1],
        source_variables.clone(),
    )
    .unwrap();
    let source_denominator = checked_polynomial_from_terms(
        vec![1.into(), (-1).into()],
        vec![1, 0, 0, 1],
        source_variables,
    )
    .unwrap();
    let source = IntRat::from_num_den(source_numerator, source_denominator, &Z, true);

    let image_x_numerator =
        checked_polynomial_from_terms(vec![1.into()], vec![1, 0], target_variables.clone())
            .unwrap();
    let image_x_denominator = checked_polynomial_from_terms(
        vec![1.into(), 1.into()],
        vec![0, 1, 0, 0],
        target_variables.clone(),
    )
    .unwrap();
    let image_x = IntRat::from_num_den(image_x_numerator, image_x_denominator.clone(), &Z, true);
    let base = IntPoly::new(&Z, None, target_variables.clone());
    let image_y = IntRat::from(base.variable(&v.into()).unwrap());

    let checked =
        checked_substitute_rational(&source, &[image_x, image_y], target_variables.clone())
            .unwrap();
    let value = checked.value.expect("the generic substitution is defined");

    // (u + 2*v^2 + 2*v)/(u - v^2 - v).
    let expected_numerator = checked_polynomial_from_terms(
        vec![1.into(), 2.into(), 2.into()],
        vec![1, 0, 0, 2, 0, 1],
        target_variables.clone(),
    )
    .unwrap();
    let expected_denominator = checked_polynomial_from_terms(
        vec![1.into(), (-1).into(), (-1).into()],
        vec![1, 0, 0, 2, 0, 1],
        target_variables.clone(),
    )
    .unwrap();
    let expected = IntRat::from_num_den(expected_numerator, expected_denominator, &Z, true);

    assert_eq!(value, expected);
    assert_eq!(value.numerator.get_vars_ref(), target_variables.as_slice());
    assert_eq!(
        value.denominator.get_vars_ref(),
        target_variables.as_slice()
    );
    assert_eq!(checked.guards[0], image_x_denominator);
    assert_eq!(checked.guards.len(), 3);
    assert!(
        checked
            .guards
            .iter()
            .all(|guard| { guard.get_vars_ref() == target_variables.as_slice() })
    );
}

#[test]
fn rp_sub_2_checked_division_retains_cancelled_poles_and_rejects_zero() {
    let (x, u) = symbol!("rustred::probe::cancel_x", "rustred::probe::cancel_u");
    let source_variables = Arc::new(vec![x.into()]);
    let target_variables = Arc::new(vec![u.into()]);

    // (x^2 - 1)/(x - 1), kept as separate source polynomials so that the
    // x=1 exceptional point cannot disappear before specialization.
    let source_numerator = checked_polynomial_from_terms(
        vec![1.into(), (-1).into()],
        vec![2, 0],
        source_variables.clone(),
    )
    .unwrap();
    let source_denominator =
        checked_polynomial_from_terms(vec![1.into(), (-1).into()], vec![1, 0], source_variables)
            .unwrap();
    let unsimplified_source = IntRat {
        numerator: source_numerator,
        denominator: source_denominator,
    };

    let base = IntPoly::new(&Z, None, target_variables.clone());
    let u_poly = base.variable(&u.into()).unwrap();
    let image_x = IntRat::from(&u_poly + &base.constant(1.into()));
    let checked =
        checked_substitute_rational(&unsimplified_source, &[image_x], target_variables.clone())
            .unwrap();
    let value = checked.value.expect("u is only conditionally excluded");
    let expected = IntRat::from(&u_poly + &base.constant(2.into()));
    assert_eq!(value, expected, "the rational result may cancel u");
    assert_eq!(
        checked.guards.last().unwrap(),
        &u_poly,
        "the pre-cancellation source denominator must remain a guard"
    );

    let image_at_pole = IntRat::from(base.constant(1.into()));
    let at_pole = checked_substitute_rational(
        &unsimplified_source,
        &[image_at_pole],
        target_variables.clone(),
    )
    .unwrap();
    assert!(
        at_pole.value.is_none(),
        "Ring::try_div must report a zero mapped denominator"
    );
    assert!(at_pole.guards.last().unwrap().is_zero());

    assert!(
        checked_substitute_rational(&unsimplified_source, &[], target_variables).is_err(),
        "the wrapper must reject short points before evaluate_with_coeff_map asserts"
    );
}

#[test]
fn pat_1_inconclusive_matcher_conditions_are_not_proof_guards() {
    let target = parse!("rustred::probe::pat_f(1)");
    let pattern = parse!("rustred::probe::pat_f(x_)").to_pattern();
    let x = symbol!("x_");
    let never_bound = symbol!("rustred::probe::never_bound_");
    let condition = x.filter_cmp(never_bound, |_x, _missing| true);
    let settings = MatchSettings::default().partial(false);

    let mut matches = target.pattern_match(&pattern, Some(&condition), Some(&settings));
    let matched_stack = matches
        .next_detailed()
        .expect("Symbolica currently permits an inconclusive candidate")
        .match_stack
        .clone();
    let final_result = condition.evaluate(&matched_stack).unwrap();

    assert_eq!(final_result, ConditionResult::Inconclusive);
    assert!(
        !final_result.is_true(),
        "RustRed's guard policy must admit only a final True result"
    );
}

#[test]
fn pat_1_wildcard_function_builder_does_not_attach_its_nominal_restriction() {
    let target = parse!("rustred::probe::actual_head(1)");
    let pattern = parse!("head_(argument_)").to_pattern();
    let head = symbol!("head_");
    let settings = MatchSettings::default().partial(false);

    // This is the exact restriction `ReplaceBuilder::with` says it adds for a
    // wildcard function head.  A function head is represented as
    // `Match::FunctionName`, while `IsAtomType(Var)` accepts only
    // `Match::Single(AtomView::Var)`, so the explicit restriction rejects it.
    let nominal_condition = head.restrict(WildcardRestriction::IsAtomType(AtomType::Var));
    assert!(
        target
            .pattern_match(&pattern, Some(&nominal_condition), Some(&settings))
            .next()
            .is_none()
    );

    // The builder still performs the replacement.  This behaviorally confirms
    // the audited inverted branch: its nominal condition was not attached.
    let replaced = target
        .replace(&pattern)
        .partial(false)
        .with(parse!("rustred::probe::matched"));
    assert_eq!(replaced, parse!("rustred::probe::matched"));
}
