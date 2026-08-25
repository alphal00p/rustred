//! Independent public-Symbolica matrix oracles for affine family maps.
//!
//! Concrete dimensions in this file are validation fixtures only.  The
//! production verifier remains loop-count and topology independent.

use rustred::{
    AFFINE_FAMILY_MAP_V2_SCHEMA, AffineDenominator, Coefficient, CoefficientContext,
    DenominatorRowAction, ExactAlgebraLimits, ExactMatrix, IntegralFamily, JacobianWitness,
    MomentumMap, ScalarProductCoordinate, SymmetryVerificationError, SymmetryVerificationLimits,
    VerifiedAffineFamilyMap, verify_affine_family_map,
};
use symbolica::{
    domains::rational_polynomial::RationalPolynomialField,
    prelude::{IntegerRing, Matrix, Z},
};

type OracleField = RationalPolynomialField<IntegerRing, u16>;
type OracleMatrix = Matrix<OracleField>;

fn oracle_matrix(
    entries: Vec<Coefficient>,
    rows: usize,
    columns: usize,
    label: &str,
) -> OracleMatrix {
    Matrix::from_linear(
        entries,
        u32::try_from(rows).expect("small oracle row count fits u32"),
        u32::try_from(columns).expect("small oracle column count fits u32"),
        RationalPolynomialField::new(Z),
    )
    .unwrap_or_else(|error| panic!("could not construct {label}: {error}"))
}

fn complete_coordinate_family(name: &str, loops: usize) -> IntegralFamily {
    let context = CoefficientContext::new(["d", "x"]);
    let coordinates = loops * (loops + 1) / 2;
    let denominators = (0..coordinates)
        .map(|selected| {
            let mut row = vec![context.zero(); coordinates];
            row[selected] = context.one();
            AffineDenominator::new(context.zero(), row)
        })
        .collect();
    IntegralFamily::new(
        name,
        (0..loops).map(|loop_id| format!("k{loop_id}")).collect(),
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        denominators,
        Vec::new(),
        vec![context.zero(); coordinates],
    )
    .unwrap()
}

fn vacuum_map(family: &IntegralFamily, loop_entries: Vec<Coefficient>) -> MomentumMap {
    let loops = family.loop_count();
    MomentumMap::new(
        ExactMatrix::try_new(loops, loops, loop_entries).unwrap(),
        ExactMatrix::try_new(loops, 0, []).unwrap(),
        ExactMatrix::try_new(0, 0, []).unwrap(),
    )
}

fn assert_coefficient_eq(actual: &Coefficient, expected: &Coefficient, label: &str) {
    assert!(
        (actual - expected).is_zero(),
        "{label}: expected {}, found {}",
        expected.to_expression(),
        actual.to_expression(),
    );
}

fn assert_oracle_matrix_eq(left: &OracleMatrix, right: &OracleMatrix, label: &str) {
    assert_eq!(left.nrows(), right.nrows(), "{label} row count");
    assert_eq!(left.ncols(), right.ncols(), "{label} column count");
    for row in 0..left.nrows() {
        for column in 0..left.ncols() {
            assert_coefficient_eq(
                &left[(row as u32, column as u32)],
                &right[(row as u32, column as u32)],
                &format!("{label}[{row},{column}]"),
            );
        }
    }
}

fn contextualize(context: &CoefficientContext, coefficient: &Coefficient) -> Coefficient {
    if context.contains(coefficient) {
        coefficient.clone()
    } else {
        let expression = coefficient.to_expression();
        context
            .parse_atom(expression.as_view())
            .expect("public Symbolica matrix output stays in the declared coefficient field")
    }
}

fn exact_matrix(context: &CoefficientContext, matrix: &OracleMatrix) -> ExactMatrix<Coefficient> {
    ExactMatrix::try_new(
        matrix.nrows(),
        matrix.ncols(),
        matrix
            .iter()
            .map(|entry| contextualize(context, entry))
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

fn affine_family(
    name: &str,
    context: &CoefficientContext,
    loops: usize,
    externals: usize,
    external_gram: Vec<Vec<Coefficient>>,
    basis: &OracleMatrix,
    constants: &OracleMatrix,
) -> IntegralFamily {
    let coordinates = loops * (loops + 1) / 2 + loops * externals;
    assert_eq!((basis.nrows(), basis.ncols()), (coordinates, coordinates));
    assert_eq!((constants.nrows(), constants.ncols()), (coordinates, 1));
    let denominators = (0..coordinates)
        .map(|row| {
            AffineDenominator::new(
                contextualize(context, &constants[(row as u32, 0)]),
                (0..coordinates)
                    .map(|column| contextualize(context, &basis[(row as u32, column as u32)]))
                    .collect(),
            )
        })
        .collect();
    IntegralFamily::new(
        name,
        (0..loops).map(|loop_id| format!("k{loop_id}")).collect(),
        (0..externals)
            .map(|external_id| format!("p{external_id}"))
            .collect(),
        context.clone(),
        context.parameter("d").unwrap(),
        denominators,
        external_gram,
        vec![context.zero(); coordinates],
    )
    .unwrap()
}

fn basis_matrix(family: &IntegralFamily) -> OracleMatrix {
    oracle_matrix(
        family
            .denominators()
            .iter()
            .flat_map(|denominator| denominator.coefficients().iter().cloned())
            .collect(),
        family.denominator_count(),
        family.denominator_count(),
        "family denominator basis",
    )
}

fn constant_column(family: &IntegralFamily) -> OracleMatrix {
    oracle_matrix(
        family
            .denominators()
            .iter()
            .map(|denominator| denominator.constant().clone())
            .collect(),
        family.denominator_count(),
        1,
        "family denominator constants",
    )
}

fn full_momentum_matrix(
    context: &CoefficientContext,
    loop_linear: &OracleMatrix,
    loop_external: &OracleMatrix,
    external_linear: &OracleMatrix,
) -> OracleMatrix {
    let loops = loop_linear.nrows();
    let externals = external_linear.nrows();
    assert_eq!((loop_linear.nrows(), loop_linear.ncols()), (loops, loops));
    assert_eq!(
        (loop_external.nrows(), loop_external.ncols()),
        (loops, externals)
    );
    assert_eq!(
        (external_linear.nrows(), external_linear.ncols()),
        (externals, externals)
    );
    let mut entries = Vec::with_capacity((loops + externals) * (loops + externals));
    for row in 0..loops + externals {
        for column in 0..loops + externals {
            entries.push(if row < loops && column < loops {
                loop_linear[(row as u32, column as u32)].clone()
            } else if row < loops {
                loop_external[(row as u32, (column - loops) as u32)].clone()
            } else if column < loops {
                context.zero()
            } else {
                external_linear[((row - loops) as u32, (column - loops) as u32)].clone()
            });
        }
    }
    oracle_matrix(
        entries,
        loops + externals,
        loops + externals,
        "full momentum map",
    )
}

fn target_gram_response(
    family: &IntegralFamily,
    coordinate: Option<ScalarProductCoordinate>,
) -> OracleMatrix {
    let context = family.coefficient_context();
    let loops = family.loop_count();
    let externals = family.external_count();
    let size = loops + externals;
    let mut gram = vec![context.zero(); size * size];
    match coordinate {
        None => {
            for row in 0..externals {
                for column in 0..externals {
                    gram[(loops + row) * size + loops + column] =
                        family.external_gram()[row][column].clone();
                }
            }
        }
        Some(ScalarProductCoordinate::LoopLoop { left, right }) => {
            gram[left * size + right] = context.one();
            gram[right * size + left] = context.one();
        }
        Some(ScalarProductCoordinate::LoopExternal {
            loop_index,
            external_index,
        }) => {
            gram[loop_index * size + loops + external_index] = context.one();
            gram[(loops + external_index) * size + loop_index] = context.one();
        }
    }
    oracle_matrix(gram, size, size, "target Gram response")
}

fn congruence(map: &OracleMatrix, gram: &OracleMatrix) -> OracleMatrix {
    let left = map * gram;
    &left * &map.transpose()
}

fn scalar_coordinate_entry(
    family: &IntegralFamily,
    matrix: &OracleMatrix,
    coordinate: ScalarProductCoordinate,
) -> Coefficient {
    match coordinate {
        ScalarProductCoordinate::LoopLoop { left, right } => {
            matrix[(left as u32, right as u32)].clone()
        }
        ScalarProductCoordinate::LoopExternal {
            loop_index,
            external_index,
        } => matrix[(
            loop_index as u32,
            (family.loop_count() + external_index) as u32,
        )]
            .clone(),
    }
}

fn public_symbolica_scalar_map(
    source: &IntegralFamily,
    target: &IntegralFamily,
    full_momentum: &OracleMatrix,
) -> (OracleMatrix, OracleMatrix) {
    let constant_gram = congruence(full_momentum, &target_gram_response(target, None));
    let constant = oracle_matrix(
        source
            .coordinates()
            .iter()
            .copied()
            .map(|coordinate| scalar_coordinate_entry(source, &constant_gram, coordinate))
            .collect(),
        source.denominator_count(),
        1,
        "scalar-map constant",
    );
    let mut linear = vec![
        source.coefficient_context().zero();
        source.denominator_count() * target.denominator_count()
    ];
    for (target_column, coordinate) in target.coordinates().iter().copied().enumerate() {
        let response = congruence(
            full_momentum,
            &target_gram_response(target, Some(coordinate)),
        );
        for (source_row, source_coordinate) in source.coordinates().iter().copied().enumerate() {
            linear[source_row * target.denominator_count() + target_column] =
                scalar_coordinate_entry(source, &response, source_coordinate);
        }
    }
    (
        constant,
        oracle_matrix(
            linear,
            source.denominator_count(),
            target.denominator_count(),
            "scalar-map linear response",
        ),
    )
}

fn public_symbolica_denominator_map(
    source: &IntegralFamily,
    target: &IntegralFamily,
    scalar_constant: &OracleMatrix,
    scalar_linear: &OracleMatrix,
) -> (OracleMatrix, OracleMatrix) {
    let source_basis = basis_matrix(source);
    let target_basis = basis_matrix(target);
    let target_inverse = target_basis
        .inv()
        .expect("the authenticated target denominator basis is invertible");
    assert_oracle_matrix_eq(
        &(&target_basis * &target_inverse),
        &Matrix::identity(
            target.denominator_count() as u32,
            target_basis.field().clone(),
        ),
        "target basis inverse replay",
    );
    let source_times_scalar = &source_basis * scalar_linear;
    let linear = &source_times_scalar * &target_inverse;
    let transformed_constant = &constant_column(source) + &(&source_basis * scalar_constant);
    let target_offset = &linear * &constant_column(target);
    let constant = &transformed_constant - &target_offset;
    (constant, linear)
}

fn assert_verified_maps_equal_oracle(
    verified: &VerifiedAffineFamilyMap,
    scalar_constant: &OracleMatrix,
    scalar_linear: &OracleMatrix,
    denominator_constant: &OracleMatrix,
    denominator_linear: &OracleMatrix,
) {
    for row in 0..scalar_constant.nrows() {
        assert_coefficient_eq(
            &verified.scalar_products().constant()[row],
            &scalar_constant[(row as u32, 0)],
            &format!("scalar constant {row}"),
        );
        for column in 0..scalar_linear.ncols() {
            assert_coefficient_eq(
                verified
                    .scalar_products()
                    .linear()
                    .get(row, column)
                    .unwrap(),
                &scalar_linear[(row as u32, column as u32)],
                &format!("scalar linear [{row},{column}]"),
            );
        }
    }
    for row in 0..denominator_constant.nrows() {
        assert_coefficient_eq(
            &verified.denominators().constant()[row],
            &denominator_constant[(row as u32, 0)],
            &format!("denominator constant {row}"),
        );
        for column in 0..denominator_linear.ncols() {
            assert_coefficient_eq(
                verified.denominators().linear().get(row, column).unwrap(),
                &denominator_linear[(row as u32, column as u32)],
                &format!("denominator linear [{row},{column}]"),
            );
        }
    }
}

#[test]
fn rational_four_loop_and_singular_maps_match_public_symbolica_determinants() {
    let family = complete_coordinate_family("symmetry-symbolica-det-oracle", 4);
    let context = family.coefficient_context();
    let x = context.parameter("x").unwrap();
    let x_plus_one = context
        .try_add(&x, &context.one(), ExactAlgebraLimits::default())
        .unwrap();
    let q = context
        .try_div(&x, &x_plus_one, ExactAlgebraLimits::default())
        .unwrap();

    let regular_entries = vec![
        q.clone(),
        context.one(),
        context.zero(),
        context.zero(),
        context.zero(),
        context.one(),
        context.one(),
        context.zero(),
        context.zero(),
        context.zero(),
        context.one(),
        context.one(),
        context.zero(),
        context.zero(),
        context.zero(),
        context.integer(-1),
    ];
    let public_determinant = oracle_matrix(
        regular_entries.clone(),
        family.loop_count(),
        family.loop_count(),
        "rational four-loop map",
    )
    .det()
    .expect("the public Symbolica determinant must succeed");
    let verified = verify_affine_family_map(
        &family,
        &family,
        vacuum_map(&family, regular_entries),
        SymmetryVerificationLimits::default(),
    )
    .unwrap();
    assert_coefficient_eq(
        verified.loop_determinant(),
        &public_determinant,
        "loop determinant",
    );
    assert_eq!(verified.external_determinant(), &context.one());
    assert_eq!(
        verified.jacobian(),
        &JacobianWitness::FormalDeterminantPower {
            determinant: public_determinant,
        }
    );
    // The vendored public Matrix::det currently reports a 0x0 matrix as
    // singular.  RustRed's structural vacuum convention must remain det(C)=1
    // before delegating nonempty determinants to Symbolica.
    assert_eq!(verified.momentum().external_linear().rows(), 0);
    assert_eq!(verified.momentum().external_linear().columns(), 0);

    let singular_entries = vec![
        context.one(),
        context.zero(),
        context.zero(),
        context.zero(),
        context.one(),
        context.zero(),
        context.zero(),
        context.zero(),
        context.zero(),
        context.zero(),
        context.one(),
        context.zero(),
        context.zero(),
        context.zero(),
        context.one(),
        context.zero(),
    ];
    let singular_determinant = oracle_matrix(
        singular_entries.clone(),
        family.loop_count(),
        family.loop_count(),
        "singular four-loop map",
    )
    .det()
    .expect("the public Symbolica determinant must classify singularity");
    assert!(singular_determinant.is_zero());
    assert_eq!(
        verify_affine_family_map(
            &family,
            &family,
            vacuum_map(&family, singular_entries),
            SymmetryVerificationLimits::default(),
        )
        .unwrap_err(),
        SymmetryVerificationError::SingularLoopMap,
    );
}

#[test]
fn complete_nonvacuum_transport_matches_public_symbolica_products() {
    let context = CoefficientContext::new(["d", "x"]);
    let x = context.parameter("x").unwrap();
    let x_plus_one = context
        .try_add(&x, &context.one(), ExactAlgebraLimits::default())
        .unwrap();
    let q = context
        .try_div(&x, &x_plus_one, ExactAlgebraLimits::default())
        .unwrap();
    let coordinates = 7;
    let identity = Matrix::identity(coordinates as u32, RationalPolynomialField::new(Z));
    let source_constants = oracle_matrix(
        (-7..0).map(|value| context.integer(value)).collect(),
        coordinates,
        1,
        "source constants",
    );
    let target_constants = oracle_matrix(
        (1..=7).map(|value| context.integer(value)).collect(),
        coordinates,
        1,
        "target constants",
    );
    let target = affine_family(
        "symbolica-product-target",
        &context,
        2,
        2,
        vec![
            vec![context.integer(2), context.one()],
            vec![context.one(), context.integer(3)],
        ],
        &identity,
        &target_constants,
    );
    let source = affine_family(
        "symbolica-product-source",
        &context,
        2,
        2,
        vec![
            vec![context.integer(7), context.integer(4)],
            vec![context.integer(4), context.integer(3)],
        ],
        &identity,
        &source_constants,
    );
    let loop_linear = oracle_matrix(
        vec![q, context.one(), context.one(), context.zero()],
        2,
        2,
        "loop map A",
    );
    let loop_external = oracle_matrix(
        vec![
            context.one(),
            context.integer(-1),
            context.integer(2),
            context.one(),
        ],
        2,
        2,
        "loop-external map B",
    );
    let external_linear = oracle_matrix(
        vec![context.one(), context.one(), context.zero(), context.one()],
        2,
        2,
        "external map C",
    );
    let public_loop_determinant = loop_linear.det().unwrap();
    let public_external_determinant = external_linear.det().unwrap();
    let momentum = MomentumMap::new(
        exact_matrix(&context, &loop_linear),
        exact_matrix(&context, &loop_external),
        exact_matrix(&context, &external_linear),
    );
    let verified = verify_affine_family_map(
        &source,
        &target,
        momentum,
        SymmetryVerificationLimits::default(),
    )
    .unwrap();
    assert_coefficient_eq(
        verified.loop_determinant(),
        &public_loop_determinant,
        "public loop determinant",
    );
    assert_coefficient_eq(
        verified.external_determinant(),
        &public_external_determinant,
        "public external determinant",
    );
    assert_eq!(VerifiedAffineFamilyMap::SCHEMA, AFFINE_FAMILY_MAP_V2_SCHEMA);
    assert_eq!(verified.stats().determinant_states(), 0);
    assert_eq!(verified.stats().symbolica_determinant_calls(), 2);
    assert_eq!(verified.stats().symbolica_product_calls(), 6);
    assert_eq!(verified.stats().symbolica_transpose_calls(), 1);
    assert!(verified.stats().symbolica_exact_operations() > 0);

    let full = full_momentum_matrix(&context, &loop_linear, &loop_external, &external_linear);
    let (scalar_constant, scalar_linear) = public_symbolica_scalar_map(&source, &target, &full);
    let (denominator_constant, denominator_linear) =
        public_symbolica_denominator_map(&source, &target, &scalar_constant, &scalar_linear);
    assert_verified_maps_equal_oracle(
        &verified,
        &scalar_constant,
        &scalar_linear,
        &denominator_constant,
        &denominator_linear,
    );

    let mapped_constant_gram = congruence(&full, &target_gram_response(&target, None));
    for row in 0..source.external_count() {
        for column in 0..source.external_count() {
            assert_coefficient_eq(
                &mapped_constant_gram[(
                    (source.loop_count() + row) as u32,
                    (source.loop_count() + column) as u32,
                )],
                &source.external_gram()[row][column],
                &format!("external Gram [{row},{column}]"),
            );
        }
    }
    verified
        .replay(&source, &target, SymmetryVerificationLimits::default())
        .unwrap();
}

fn verify_sheared_spelling(
    label: &str,
    context: &CoefficientContext,
    common_basis: &OracleMatrix,
    family_action: &OracleMatrix,
    base_constants: &OracleMatrix,
) -> VerifiedAffineFamilyMap {
    let target_basis = common_basis.clone();
    let target_constants = common_basis * base_constants;
    let source_basis = family_action * common_basis;
    let source_constants = family_action * &target_constants;
    let gram = vec![vec![context.parameter("s").unwrap()]];
    let target = affine_family(
        &format!("{label}-target"),
        context,
        2,
        1,
        gram.clone(),
        &target_basis,
        &target_constants,
    );
    let source = affine_family(
        &format!("{label}-source"),
        context,
        2,
        1,
        gram,
        &source_basis,
        &source_constants,
    );
    let loop_identity = Matrix::identity(2, RationalPolynomialField::new(Z));
    let loop_external = oracle_matrix(vec![context.zero(); 2], 2, 1, "zero shift");
    let external_identity = Matrix::identity(1, RationalPolynomialField::new(Z));
    let verified = verify_affine_family_map(
        &source,
        &target,
        MomentumMap::new(
            exact_matrix(context, &loop_identity),
            exact_matrix(context, &loop_external),
            exact_matrix(context, &external_identity),
        ),
        SymmetryVerificationLimits::default(),
    )
    .unwrap();
    let full = full_momentum_matrix(context, &loop_identity, &loop_external, &external_identity);
    let (scalar_constant, scalar_linear) = public_symbolica_scalar_map(&source, &target, &full);
    let (denominator_constant, denominator_linear) =
        public_symbolica_denominator_map(&source, &target, &scalar_constant, &scalar_linear);
    assert_verified_maps_equal_oracle(
        &verified,
        &scalar_constant,
        &scalar_linear,
        &denominator_constant,
        &denominator_linear,
    );
    assert_oracle_matrix_eq(
        &denominator_linear,
        family_action,
        "basis-independent denominator action",
    );
    assert!(denominator_constant.is_zero());
    verified
}

#[test]
fn simultaneous_denominator_basis_shears_preserve_the_exact_family_map() {
    let context = CoefficientContext::new(["d", "s"]);
    let field = RationalPolynomialField::new(Z);
    let identity = Matrix::identity(5, field.clone());
    let shear = oracle_matrix(
        vec![
            1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1,
        ]
        .into_iter()
        .map(|value| context.integer(value))
        .collect(),
        5,
        5,
        "common denominator-basis shear",
    );
    let action = oracle_matrix(
        vec![
            0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 1, 1, 1, 0, 0, 0, 0, 0, -1, 0, 0, 0, 0, 0, 1,
        ]
        .into_iter()
        .map(|value| context.integer(value))
        .collect(),
        5,
        5,
        "frozen family action",
    );
    let base_constants = oracle_matrix(
        (1..=5).map(|value| context.integer(value)).collect(),
        5,
        1,
        "base denominator constants",
    );
    assert_coefficient_eq(&shear.det().unwrap(), &context.one(), "shear determinant");
    let direct = verify_sheared_spelling(
        "direct-basis-spelling",
        &context,
        &identity,
        &action,
        &base_constants,
    );
    let sheared = verify_sheared_spelling(
        "sheared-basis-spelling",
        &context,
        &shear,
        &action,
        &base_constants,
    );
    assert_eq!(direct.denominators(), sheared.denominators());
    assert_eq!(direct.scalar_products(), sheared.scalar_products());
    assert_eq!(direct.row_actions(), sheared.row_actions());
    assert_eq!(
        direct.row_actions(),
        &[
            DenominatorRowAction::Monomial {
                target: 1,
                scale: context.one(),
            },
            DenominatorRowAction::Monomial {
                target: 0,
                scale: context.one(),
            },
            DenominatorRowAction::Affine,
            DenominatorRowAction::Monomial {
                target: 3,
                scale: context.integer(-1),
            },
            DenominatorRowAction::Monomial {
                target: 4,
                scale: context.one(),
            },
        ]
    );
}
