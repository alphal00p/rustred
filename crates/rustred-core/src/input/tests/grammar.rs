use super::*;

#[test]
fn wrong_and_duplicate_clauses_fail_closed() {
    let compiler = compiler();
    let duplicate = compiler.compile_compact(
        "I(loops(k),externals(),dimension(d),dimension(d),prop(D1,k^2-m2,1))",
        None,
    );
    assert!(matches!(
        duplicate,
        Err(Error::DuplicateClause { kind: "dimension" })
    ));

    let wrong_arity = compiler.compile_compact(
        "I(loops(k),externals(),dimension(d,4),prop(D1,k^2-m2,1))",
        None,
    );
    assert!(matches!(
        wrong_arity,
        Err(Error::WrongClauseArity {
            kind: "dimension",
            ..
        })
    ));

    let unknown = compiler.compile_compact(
        "I(loops(k),externals(),dimension(d),bogus(x),prop(D1,k^2-m2,1))",
        None,
    );
    assert!(matches!(unknown, Err(Error::UnsupportedToken { .. })));
}

#[test]
fn grammar_clause_heads_are_rejected_inside_payload_expressions() {
    let compiler = compiler();
    let nested_numerator = compiler.compile_compact(
        "I(loops(k),externals(),dimension(d),prop(D1,k^2-m2,1),numerator(prop(X,x,1)))",
        None,
    );
    assert!(matches!(
        nested_numerator,
        Err(Error::UnsupportedToken { .. })
    ));

    let nested_scalar = compiler.compile_compact(
        "I(loops(k),externals(),dimension(gram(k,k,d)),prop(D1,k^2-m2,1))",
        None,
    );
    assert!(matches!(nested_scalar, Err(Error::UnsupportedToken { .. })));
}

#[test]
fn base_coefficient_fields_reject_scalar_products_and_momenta() {
    let compiler = compiler();
    let scalar_product_dimension = compiler.compile_compact(
        "I(loops(k),externals(p),dimension(sp(p,p)),prop(D1,k^2-m2,1),prop(D2,(k+p)^2-m2,1),gram(p,p,s))",
        None,
    );
    assert!(matches!(
        scalar_product_dimension,
        Err(Error::UnsupportedToken { .. })
    ));

    let momentum_shift = compiler.compile_compact(
        "I(loops(k),externals(),dimension(d),prop(D1,k^2-m2,1),power_shift(D1,k))",
        None,
    );
    assert!(matches!(
        momentum_shift,
        Err(Error::UnsupportedToken { .. })
    ));

    let explicit_momentum_dimension = compiler.compile_text(TextProject {
        name: None,
        parameters: None,
        loop_momenta: vec!["k".to_owned()],
        external_momenta: vec![],
        dimension: "k".to_owned(),
        propagators: vec![TextPropagator {
            id: "D1".to_owned(),
            expression: "k^2-m2".to_owned(),
            target_power: 1,
            power_shift: None,
        }],
        external_gram: vec![],
        numerator: None,
    });
    assert!(matches!(
        explicit_momentum_dimension,
        Err(Error::UnsupportedToken { .. })
    ));
}
