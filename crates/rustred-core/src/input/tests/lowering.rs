use super::*;

#[test]
fn target_and_tensor_numerator_do_not_specialize_the_derived_family() {
    let first = compiler()
        .compile_compact(&one_loop_source(3, "vec(k,mu)"), None)
        .expect("first target should normalize");
    let second = compiler()
        .compile_compact(&one_loop_source(-1, "metric(mu,nu)*vec(k,nu)"), None)
        .expect("second target should normalize");
    assert_eq!(first.target().powers(), &[3]);
    assert_eq!(second.target().powers(), &[-1]);
    let first = first
        .into_lowered(LoweringLimits::default())
        .expect("first target family should lower");
    let second = second
        .into_lowered(LoweringLimits::default())
        .expect("second target family should lower");
    assert_eq!(first.family().fingerprint(), second.family().fingerprint());
}

#[test]
fn one_external_gram_entry_lowers_a_complete_one_loop_basis() {
    let source =
        "I(loops(k),externals(p),dimension(d),prop(D1,k^2-m2,1),prop(D2,(k+p)^2-m2,1),gram(p,p,s))";
    let normalized = compiler()
        .compile_compact(source, None)
        .expect("one-external family should normalize");
    assert_eq!(normalized.external_gram().len(), 1);
    assert_eq!(normalized.external_gram()[0].len(), 1);
    let lowered = normalized
        .into_lowered(LoweringLimits::default())
        .expect("complete one-external basis should lower");
    assert_eq!(lowered.family().external_momenta(), &["p".to_owned()]);
    assert_eq!(lowered.denominators().len(), 2);
}

#[test]
fn signed_numbers_work_in_denominators_and_target_powers() {
    let normalized = compiler()
        .compile_compact(
            "I(loops(k),externals(),dimension(d),prop(D1,k^2-1,-2))",
            None,
        )
        .expect("negative constants and target powers must remain valid exact integers");
    assert_eq!(normalized.target().powers(), &[-2]);
    normalized
        .into_lowered(LoweringLimits::default())
        .expect("a denominator with a negative constant should lower");
}
