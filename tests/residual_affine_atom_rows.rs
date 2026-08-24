use symbolica::prelude::Integer;

use rustred::{
    CoefficientContext, ParametricCoefficient, ParametricCoefficientContext, ParametricPolynomial,
    ResidualAffineAtomRowCertificate, ResidualAffineAtomRowError, ResidualAffineAtomRowLimits,
    ResidualAffineAtomRowOutcome, ResidualAffineAtomRowUnsupported,
};

fn polynomial(
    context: &ParametricCoefficientContext,
    value: &ParametricCoefficient,
) -> ParametricPolynomial {
    context.numerator_condition(value).unwrap()
}

fn affine(
    context: &ParametricCoefficientContext,
    constant: i64,
    coefficients: &[i64],
) -> ParametricCoefficient {
    let mut result = context.integer(constant);
    for (position, &coefficient) in coefficients.iter().enumerate() {
        let term = context
            .mul(
                &context.integer(coefficient),
                &context.index(position).unwrap(),
            )
            .unwrap();
        result = context.add(&result, &term).unwrap();
    }
    result
}

#[test]
fn public_api_recognizes_a_topology_independent_affine_atom_and_replays() {
    let base = CoefficientContext::new(["mass_squared", "dimension"]);
    let context = ParametricCoefficientContext::try_new(&base, "affine-atom-public", 4).unwrap();
    let mass = context
        .lift(&base.parameter("mass_squared").unwrap())
        .unwrap();
    let dimension = context.lift(&base.parameter("dimension").unwrap()).unwrap();
    let base_factor = context
        .add(
            &context.mul(&context.integer(-12), &mass).unwrap(),
            &context.mul(&context.integer(18), &dimension).unwrap(),
        )
        .unwrap();
    let source = polynomial(
        &context,
        &context
            .mul(&base_factor, &affine(&context, -3, &[6, 0, -9, 12]))
            .unwrap(),
    );

    let certificate = ResidualAffineAtomRowCertificate::compile(
        &context,
        source,
        ResidualAffineAtomRowLimits::default(),
    )
    .unwrap();
    assert_eq!(certificate.outcome(), ResidualAffineAtomRowOutcome::Row);
    let row = certificate.row().unwrap();
    assert_eq!(row.arity(), 4);
    assert_eq!(row.constant(), &Integer::from(1));
    assert_eq!(
        row.coefficients(),
        &[
            Integer::from(-2),
            Integer::from(0),
            Integer::from(3),
            Integer::from(-4),
        ]
    );
    assert_eq!(certificate.block_witnesses().len(), 2);
    certificate.replay(&context).unwrap();
}

#[test]
fn public_api_keeps_unsupported_shape_distinct_from_branch_inconsistency() {
    let base = CoefficientContext::new(["theta"]);
    let context =
        ParametricCoefficientContext::try_new(&base, "affine-atom-public-unsupported", 2).unwrap();
    let theta = context.lift(&base.parameter("theta").unwrap()).unwrap();
    let first = context.mul(&theta, &affine(&context, 1, &[1, 0])).unwrap();
    let source = polynomial(
        &context,
        &context.add(&first, &affine(&context, 1, &[0, 1])).unwrap(),
    );

    assert!(matches!(
        ResidualAffineAtomRowCertificate::compile(
            &context,
            source,
            ResidualAffineAtomRowLimits::default(),
        ),
        Err(ResidualAffineAtomRowError::Unsupported {
            reason: ResidualAffineAtomRowUnsupported::NonAssociateBaseBlock { .. }
        })
    ));
}
