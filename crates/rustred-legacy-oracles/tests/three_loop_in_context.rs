use rustred::CoefficientContext;
use rustred_legacy_oracles::FamilyError;
use rustred_legacy_oracles::{
    equal_mass_three_loop_tetrahedron, equal_mass_three_loop_tetrahedron_in_context,
};

// Keep all Symbolica context construction in one integration test so the
// process-global symbol registry is exercised serially within this binary.
#[test]
fn caller_context_tetrahedron_matches_builtin_and_rejects_missing_parameters() {
    let context = CoefficientContext::new(["d", "m2"]);
    let family = equal_mass_three_loop_tetrahedron_in_context(context.clone()).unwrap();
    let builtin = equal_mass_three_loop_tetrahedron().unwrap();

    assert!(family.coefficients().has_same_variable_map(&context));
    assert!(
        family
            .coefficients()
            .has_same_variable_map(builtin.coefficients())
    );
    assert_eq!(family.fingerprint(), builtin.fingerprint());
    assert_eq!(family.denominators().len(), builtin.denominators().len());
    for (actual, expected) in family.denominators().iter().zip(builtin.denominators()) {
        assert_eq!(actual.quadratic_form(), expected.quadratic_form());
        assert_eq!(actual.shift(), expected.shift());
        assert_eq!(actual.propagator_sign(), expected.propagator_sign());
        assert_eq!(actual.momentum(), expected.momentum());
    }
    assert_eq!(family.symmetries(), builtin.symmetries());

    assert!(matches!(
        equal_mass_three_loop_tetrahedron_in_context(CoefficientContext::new(["d", "mu2"])),
        Err(FamilyError::UnknownCoefficientParameter(parameter)) if parameter == "m2"
    ));
    assert!(matches!(
        equal_mass_three_loop_tetrahedron_in_context(CoefficientContext::new(["m2"])),
        Err(FamilyError::UnknownDimensionParameter(parameter)) if parameter == "d"
    ));
}
