use super::conjunction::{exact_domain, expression};
use super::*;

const FG: &str = "6*n8^2+2*n8^3-4*n2*n8+2*n2*n8^2-2*n2^2*n8-3*d*n8^2+2*d*n2*n8";
const H: &str = "-24*n7^2-44*n7^3-12*n7^4+16*n0*n7+4*n0*n7^2-16*n0*n7^3+16*n0^2*n7+8*n0^2*n7^2+4*n0^3*n7+24*d*n7^2+22*d*n7^3-16*d*n0*n7-2*d*n0*n7^2-8*d*n0^2*n7-6*d^2*n7^2+4*d^2*n0*n7";
const BMW: &str = "3+n9-3*n9^2-n9^3-4*n4-6*n4*n9-2*n4*n9^2+5*n3+12*n3*n9+3*n3*n9^2+2*n3*n4-5*n3^2+n3^2*n9+2*n3^2*n4-3*n3^3-d+d*n9^2+2*d*n4+2*d*n4*n9-2*d*n3-4*d*n3*n9-2*d*n3*n4+3*d*n3^2";

fn captured_context() -> IndexedCoefficientContext {
    IndexedCoefficientContext::try_new(
        &CoefficientContext::new(["d"]),
        "factor-consequence-captures",
        10,
    )
    .unwrap()
}

// Full guard from FG sector115 retained rule83/177, outer guard3. The sparse
// highest-base coefficient must not hide the complete original ingress.
const FG115: &str = "411840-140064*n8-445428*n8^2-70104*n8^3+137460*n8^4+83376*n8^5+20436*n8^6+2376*n8^7+108*n8^8+1347272*n2+201666*n2*n8-876356*n2*n8^2-489970*n2*n8^3-65808*n2*n8^4+13054*n2*n8^5+4236*n2*n8^6+306*n2*n8^7+1734230*n2^2+798904*n2^2*n8-353930*n2^2*n8^2-286720*n2^2*n8^3-59726*n2^2*n8^4-3704*n2^2*n8^5+66*n2^2*n8^6+1150436*n2^3+662898*n2^3*n8+9900*n2^3*n8^2-56248*n2^3*n8^3-11088*n2^3*n8^4-570*n2^3*n8^5+439878*n2^4+250020*n2^4*n8+31272*n2^4*n8^2-3220*n2^4*n8^3-574*n2^4*n8^4+101008*n2^5+48952*n2^5*n8+6128*n2^5*n8^2+72*n2^5*n8^3+13760*n2^6+4848*n2^6*n8+368*n2^6*n8^2+1024*n2^7+192*n2^7*n8+32*n2^8-849360*d+523356*d*n8+784260*d*n8^2-66012*d*n8^3-271416*d*n8^4-103932*d*n8^5-15996*d*n8^6-900*d*n8^7-2601491*d*n2+187768*d*n2*n8+1611139*d*n2*n8^2+601636*d*n2*n8^3+31591*d*n2*n8^4-15068*d*n2*n8^5-1815*d*n2*n8^6-3035800*d*n2^2-859416*d*n2^2*n8+640058*d*n2^2*n8^2+302418*d*n2^2*n8^3+37262*d*n2^2*n8^4+822*d*n2^2*n8^5-1744191*d*n2^3-714962*d*n2^3*n8+45000*d*n2^3*n8^2+43930*d*n2^3*n8^3+3839*d*n2^3*n8^4-548378*d*n2^4-219574*d*n2^4*n8-13446*d*n2^4*n8^2+1606*d*n2^4*n8^3-96424*d*n2^5-29968*d*n2^5*n8-1688*d*n2^5*n8^2-8904*d*n2^6-1528*d*n2^6*n8-336*d*n2^7+727851*d^2-649650*d^2*n8-501939*d^2*n8^2+192792*d^2*n8^3+185721*d^2*n8^4+42138*d^2*n8^5+3087*d^2*n8^6+2076755*d^2*n2-593780*d^2*n2*n8-1128420*d^2*n2*n8^2-258706*d^2*n2*n8^3+3481*d^2*n2*n8^4+3726*d^2*n2*n8^5+2167197*d^2*n2^2+270469*d^2*n2^2*n8-393307*d^2*n2^2*n8^2-103097*d^2*n2^2*n8^3-5526*d^2*n2^2*n8^4+1043125*d^2*n2^3+272755*d^2*n2^3*n8-32501*d^2*n2^3*n8^2-8243*d^2*n2^3*n8^3+253856*d^2*n2^4+62676*d^2*n2^4*n8+1004*d^2*n2^4*n8^2+30456*d^2*n2^5+4520*d^2*n2^5*n8+1432*d^2*n2^6-331653*d^3+387768*d^3*n8+126126*d^3*n8^2-122988*d^3*n8^3-53673*d^3*n8^4-5580*d^3*n8^5-876348*d^3*n2+428820*d^3*n2*n8+377064*d^3*n2*n8^2+44244*d^3*n2*n8^3-2244*d^3*n2*n8^4-803019*d^3*n2^2+4854*d^3*n2^2*n8+100797*d^3*n2^2*n8^2+11340*d^3*n2^2*n8^3-306786*d^3*n2^3-41964*d^3*n2^3*n8+5430*d^3*n2^3*n8^2-51642*d^3*n2^4-5766*d^3*n2^4*n8-3180*d^3*n2^5+84726*d^4-122517*d^4*n8+387*d^4*n8^2+31797*d^4*n8^3+5607*d^4*n8^4+205884*d^4*n2-139869*d^4*n2*n8-59850*d^4*n2*n8^2-2241*d^4*n2*n8^3+161478*d^4*n2^2-15372*d^4*n2^2*n8-9234*d^4*n2^2*n8^2+44208*d^4*n2^3+1980*d^4*n2^3*n8+3888*d^4*n2^4-11502*d^5+19818*d^5*n8-5346*d^5*n8^2-2970*d^5*n8^3-25488*d^5*n2+21708*d^5*n2*n8+3564*d^5*n2*n8^2-16470*d^5*n2^2+1890*d^5*n2^2*n8-2484*d^5*n2^3+648*d^6-1296*d^6*n8+648*d^6*n8^2+1296*d^6*n2-1296*d^6*n2*n8+648*d^6*n2^2";

fn check_capture(
    context: &IndexedCoefficientContext,
    source: &str,
    sector: &[bool; 10],
    piece: &LatticeBox,
    target: &AffineApplicationDomain,
    exclusions: &[Arc<AffineApplicationDomain>],
) -> Result<(), SourcePortAuditError> {
    let chart = target.prepare_restriction().unwrap();
    validate_guard_on_domain_with_limits(
        context,
        &polynomial(context, &expression(context, source)),
        piece,
        sector,
        Some((target, &chart)),
        exclusions,
        Default::default(),
    )
}

#[test]
fn captured_fg115_small_coefficient_proves_whole_exclusion_before_dense_factor_work() {
    let context = captured_context();
    let sector = [
        true, true, false, false, true, true, true, false, false, false,
    ];
    let fixed = [
        Some(1),
        Some(1),
        None,
        None,
        Some(1),
        Some(1),
        Some(1),
        Some(0),
        None,
        Some(0),
    ];
    let target = exact_domain(&context, &sector, fixed, &["-1-n8+2*n3"]);
    let exclusion = exact_domain(&context, &sector, fixed, &["1-n8+n2", "-1-n8+2*n3"]);
    let piece = LatticeBox::try_new(
        [0, 0, 2, 2, 0, 0, 0, 0, 1, 0],
        [
            Some(0),
            Some(0),
            None,
            None,
            Some(0),
            Some(0),
            Some(0),
            Some(0),
            None,
            Some(0),
        ],
    )
    .unwrap();
    let guard = polynomial(&context, &expression(&context, FG115));
    let system = context
        .base_coefficient_system(&guard, Default::default(), Default::default())
        .unwrap();
    assert_eq!(system.equations().len(), 7);
    let sparse = system.equations().last().unwrap().index_polynomial();
    assert_eq!(
        sparse.raw(),
        polynomial(&context, &expression(&context, "648*(1-n8+n2)^2")).raw()
    );
    assert!(sparse.raw().nterms() < system.equations()[0].index_polynomial().raw().nterms());
    // This is the old failure: whole-system coordinate factor scheduling
    // reaches a dense sibling's prospective work cap before the cheap proof.
    let old_issue = super::super::misses_target(
        &context,
        &guard,
        &piece,
        &sector,
        Some(&target),
        Default::default(),
        &mut Work::default(),
    )
    .unwrap_err();
    assert!(old_issue.to_string().contains("859963392"), "{old_issue}");
    check_capture(
        &context,
        FG115,
        &sector,
        &piece,
        &target,
        &[exclusion.clone()],
    )
    .unwrap();

    // Exact native replay of a genuine zero in the original piece/target.
    // It is excluded by the intended complete exclusion, but not by an
    // exclusion with one extra independent AND sibling.
    let point = [1, 1, -6, -2, 1, 1, 1, 0, -5, 0];
    assert!(
        context
            .specialize_polynomial(&guard, &point, Default::default())
            .unwrap()
            .is_zero()
    );
    assert!(check_capture(&context, FG115, &sector, &piece, &target, &[]).is_err());
    // Keep the negative fixture in this helper's coupled-domain carrier:
    // the piece still fixes n9=0, so this is exactly the extra n8+3 condition
    // on every tested point, without fixing every axis in the domain itself.
    let mut extra_fixed = fixed;
    extra_fixed[9] = None;
    let incomplete = exact_domain(
        &context,
        &sector,
        extra_fixed,
        &["1-n8+n2", "-1-n8+2*n3", "n8+n9+3"],
    );
    assert!(check_capture(&context, FG115, &sector, &piece, &target, &[incomplete]).is_err());

    let chart = target.prepare_restriction().unwrap();
    let mut limited = RuleCellLimits::default();
    limited.guard_algebra.max_input_terms = guard.raw().nterms() - 1;
    assert!(
        validate_guard_on_domain_with_limits(
            &context,
            &guard,
            &piece,
            &sector,
            Some((&target, &chart)),
            &[exclusion.clone()],
            limited
        )
        .is_err()
    );
    limited = RuleCellLimits::default();
    limited.guard_algebra.max_gcd_factor_work = 0;
    assert!(
        validate_guard_on_domain_with_limits(
            &context,
            &guard,
            &piece,
            &sector,
            Some((&target, &chart)),
            &[exclusion],
            limited
        )
        .is_err()
    );
}

#[test]
fn cheap_coefficient_scheduling_is_generic_and_admits_all_exclusion_siblings() {
    let context = context();
    let piece = LatticeBox::try_new([0; 3], [None; 3]).unwrap();
    let exclusion = exact_domain(&context, &[false; 3], [None; 3], &["n0-n1"]);
    // Degree8/dense first coefficients versus a sparse square; invert base
    // placement as a control that this is not a highest-d-only special case.
    for source in [
        "(n0-n1)^2*(1+n0+n1)^6+d^6*(n0-n1)^2",
        "d^6*(n0-n1)^2*(1+n0+n1)^6+(n0-n1)^2",
    ] {
        let guard = expression(&context, source);
        check(
            &context,
            &guard,
            &piece,
            None,
            &[exclusion.clone()],
            Default::default(),
        )
        .unwrap();
        // At n0=n1=0 the full polynomial vanishes; an additional unrelated
        // equation cannot be dropped just because the first one is implied.
        let extra = exact_domain(&context, &[false; 3], [None; 3], &["n0-n1", "n2+1"]);
        assert!(check(&context, &guard, &piece, None, &[extra], Default::default()).is_err());
        let mut limits = RuleCellLimits::default();
        limits.guard_algebra.max_coefficient_equations = 1;
        assert!(check(&context, &guard, &piece, None, &[exclusion.clone()], limits).is_err());
        // A late foreign-index binding is admitted before any cheap success.
        let wrong = domain(&context, [None; 3], &[g(&context)], [2, 1, 3]);
        assert!(
            check(
                &context,
                &guard,
                &piece,
                None,
                &[exclusion.clone(), wrong],
                Default::default()
            )
            .is_err()
        );
    }
}

#[test]
fn captured_h58_factor_consequence_retains_all_coefficients_and_real_zero() {
    let context = captured_context();
    let sector = [
        false, true, false, true, true, true, false, false, false, false,
    ];
    let fixed = [
        None,
        Some(1),
        Some(0),
        Some(1),
        Some(1),
        Some(1),
        None,
        None,
        Some(0),
        Some(0),
    ];
    let target = exact_domain(&context, &sector, fixed, &["n6-n7"]);
    let upper = [
        None,
        Some(0),
        Some(0),
        Some(0),
        Some(0),
        Some(0),
        None,
        None,
        Some(0),
        Some(0),
    ];
    let piece = LatticeBox::try_new([2, 0, 0, 0, 0, 0, 1, 1, 0, 0], upper).unwrap();
    check_capture(&context, H, &sector, &piece, &target, &[]).unwrap();
    // n7=0 is an actual common coefficient zero after this widening.
    let widened = LatticeBox::try_new([2, 0, 0, 0, 0, 0, 0, 0, 0, 0], upper).unwrap();
    assert!(check_capture(&context, H, &sector, &widened, &target, &[]).is_err());
}

#[test]
fn captured_fg83_factor_consequence_retains_all_coefficients_and_real_zero() {
    let context = captured_context();
    let sector = [
        true, true, false, false, true, false, true, false, false, false,
    ];
    let fixed = [
        Some(1),
        Some(1),
        None,
        Some(0),
        Some(1),
        Some(0),
        Some(1),
        None,
        None,
        Some(0),
    ];
    let target = exact_domain(&context, &sector, fixed, &["n7-n8"]);
    let upper = [
        Some(0),
        Some(0),
        None,
        Some(0),
        Some(0),
        Some(0),
        Some(0),
        None,
        None,
        Some(0),
    ];
    let piece = LatticeBox::try_new([0, 0, 2, 0, 0, 0, 0, 1, 1, 0], upper).unwrap();
    check_capture(&context, FG, &sector, &piece, &target, &[]).unwrap();
    let widened = LatticeBox::try_new([0, 0, 2, 0, 0, 0, 0, 0, 0, 0], upper).unwrap();
    assert!(check_capture(&context, FG, &sector, &widened, &target, &[]).is_err());
}

#[test]
fn captured_bmw230_and_broader_piece_keep_whole_exclusions_in_each_chart() {
    let context = captured_context();
    let sector = [
        false, true, true, false, false, true, true, true, false, false,
    ];
    let fixed = [
        None,
        Some(1),
        Some(1),
        None,
        None,
        Some(1),
        Some(1),
        Some(1),
        Some(0),
        None,
    ];
    let target = exact_domain(&context, &sector, fixed, &["2*n0-n3-n9-1"]);
    let first = exact_domain(&context, &sector, fixed, &["n0-n9-1", "n3-n9-1"]);
    let second = exact_domain(&context, &sector, fixed, &["n0-n9", "n3-n9+1", "n4-n9+1"]);
    let actual = LatticeBox::try_new(
        [2, 0, 0, 1, 2, 0, 0, 0, 0, 0],
        [
            None,
            Some(0),
            Some(0),
            None,
            None,
            Some(0),
            Some(0),
            Some(0),
            Some(0),
            Some(0),
        ],
    )
    .unwrap();
    check_capture(
        &context,
        BMW,
        &sector,
        &actual,
        &target,
        &[first.clone(), second.clone()],
    )
    .unwrap();
    // This larger domain contains both distinct zero components. It needs
    // aligned factor classification after the first strict-rank refinement.
    let broad = LatticeBox::try_new(
        [0; 10],
        [
            None,
            Some(0),
            Some(0),
            None,
            None,
            Some(0),
            Some(0),
            Some(0),
            Some(0),
            None,
        ],
    )
    .unwrap();
    check_capture(
        &context,
        BMW,
        &sector,
        &broad,
        &target,
        &[first.clone(), second.clone()],
    )
    .unwrap();
    for exclusions in [vec![], vec![first], vec![second]] {
        assert!(check_capture(&context, BMW, &sector, &broad, &target, &exclusions).is_err());
    }
}

#[test]
fn affine_factor_consequence_is_generic_under_order_scaling_and_multiplicity() {
    let context = context();
    let source = "6*n1^2+2*n1^3-4*n0*n1+2*n0*n1^2-2*n0^2*n1-3*d*n1^2+2*d*n0*n1";
    for order in [[0, 1, 2], [2, 0, 1], [1, 2, 0]] {
        let mut renamed = source.to_owned();
        for axis in 0..3 {
            renamed = renamed.replace(&format!("n{axis}"), &format!("{{{axis}}}"));
        }
        for (axis, replacement) in order.into_iter().enumerate() {
            renamed = renamed.replace(&format!("{{{axis}}}"), &format!("n{replacement}"));
        }
        let mut lower = [0; 3];
        lower[order[0]] = 2;
        lower[order[1]] = 1;
        let guard = expression(&context, &format!("-7*n{}^2*({renamed})", order[1]));
        check(
            &context,
            &guard,
            &LatticeBox::try_new(lower, [None; 3]).unwrap(),
            None,
            &[],
            Default::default(),
        )
        .unwrap();
    }
}

#[test]
fn unresolved_factor_alternatives_and_mixed_exclusions_never_become_an_and() {
    let context = context();
    let piece = LatticeBox::try_new([0; 3], [None; 3]).unwrap();
    let domain = super::super::conjunction::GuardDomain {
        piece: &piece,
        sector: &[false; 3],
        target: None,
    };
    assert!(matches!(
        super::super::factors::classify(
            &context,
            &polynomial(&context, &expression(&context, "n0*n1")),
            &[],
            &domain,
            Default::default(),
            &mut Work::default(),
            &mut 0,
        )
        .unwrap(),
        super::super::factors::Consequence::Unresolved
    ));
    let guard = expression(&context, "(n0-n1)*(n1-n2)+d*(n0-n1)*(n1-n2+1)");
    let first = exact_domain(&context, &[false; 3], [None; 3], &["n0-n1", "n0+n1+n2+2"]);
    let second = exact_domain(&context, &[false; 3], [None; 3], &["n1-n2", "n0+n1+n2+2"]);
    // (n0,n1,n2)=(0,0,-1) is a genuine common zero outside both conjunctions.
    assert!(
        check(
            &context,
            &guard,
            &piece,
            None,
            &[first, second],
            Default::default()
        )
        .is_err()
    );
}

#[test]
fn factor_work_and_guard_work_remain_shared_and_context_authenticated() {
    let context = context();
    let piece = LatticeBox::try_new([0, 1, 0], [None; 3]).unwrap();
    let domain = super::super::conjunction::GuardDomain {
        piece: &piece,
        sector: &[false; 3],
        target: None,
    };
    let coefficient = polynomial(&context, &expression(&context, "n0*n1"));
    let mut work = Work::default();
    let mut factor_work = 0;
    assert!(matches!(
        super::super::factors::classify(
            &context,
            &coefficient,
            &[],
            &domain,
            Default::default(),
            &mut work,
            &mut factor_work
        )
        .unwrap(),
        super::super::factors::Consequence::Affine(_)
    ));
    assert!(factor_work > 0 && work.operations > 0);
    let mut limits = RuleCellLimits::default();
    limits.guard_algebra.max_gcd_factor_work = factor_work;
    assert!(
        super::super::factors::classify(
            &context,
            &coefficient,
            &[],
            &domain,
            limits,
            &mut Work::default(),
            &mut factor_work
        )
        .is_err()
    );
    limits = RuleCellLimits::default();
    limits.guard_algebra.max_exact_hyperplane_replay_work = work.operations;
    assert!(
        super::super::factors::classify(
            &context,
            &coefficient,
            &[],
            &domain,
            limits,
            &mut work,
            &mut 0
        )
        .is_err()
    );
    let mut exhausted_factor_work = usize::MAX;
    assert!(
        super::super::factors::classify(
            &context,
            &coefficient,
            &[],
            &domain,
            Default::default(),
            &mut Work::default(),
            &mut exhausted_factor_work
        )
        .is_err()
    );
    let foreign = IndexedCoefficientContext::try_new(
        &CoefficientContext::new(["d"]),
        "foreign-factor-consequence",
        3,
    )
    .unwrap();
    assert!(
        super::super::factors::classify(
            &foreign,
            &coefficient,
            &[],
            &domain,
            Default::default(),
            &mut Work::default(),
            &mut 0
        )
        .is_err()
    );
}

#[test]
fn derived_rows_share_the_original_equation_quota_before_retention() {
    let context = context();
    let system = context
        .base_coefficient_system(
            &polynomial(&context, &expression(&context, "n0*n1+d*n1*(n0+1)")),
            Default::default(),
            Default::default(),
        )
        .unwrap();
    assert_eq!(system.equations().len(), 2);
    let mut consequences = Vec::new();
    let mut limits = RuleCellLimits::default();
    limits.guard_algebra.max_coefficient_equations = 2;
    assert!(
        super::super::conjunction::retain_consequence(
            &system,
            &[],
            &mut consequences,
            polynomial(&context, &expression(&context, "n0")),
            limits,
            &mut Work::default(),
        )
        .is_err()
    );
    assert!(consequences.is_empty());
    limits.guard_algebra.max_coefficient_equations = 3;
    super::super::conjunction::retain_consequence(
        &system,
        &[],
        &mut consequences,
        polynomial(&context, &expression(&context, "n0")),
        limits,
        &mut Work::default(),
    )
    .unwrap();
    assert!(
        super::super::conjunction::retain_consequence(
            &system,
            &[],
            &mut consequences,
            polynomial(&context, &expression(&context, "n1")),
            limits,
            &mut Work::default(),
        )
        .is_err()
    );
    assert_eq!(consequences.len(), 1);
}
