use symbolica::atom::{NamespacedSymbol, SymbolAttribute, SymbolBuilder, UserData};

use super::super::context::authenticate_feynman_symbol;
use super::super::error::FeynmanPolynomialError;

#[test]
fn feynman_symbol_authentication_rejects_unsafe_process_global_metadata() {
    // Mutate a test-only name: Symbolica symbol metadata is process-global and
    // cannot be unregistered, so poisoning a real positional Feynman symbol
    // would make unrelated K >= 6 tests scheduling-dependent.
    let parameter = 5;
    let qualified = "rustred_test::unsafe_feynman_parameter_authentication_v1";
    let namespaced = NamespacedSymbol::try_parse(qualified).unwrap();
    let symbol = SymbolBuilder::new(namespaced)
        .with_attributes(&[SymbolAttribute::Symmetric])
        .with_tags(["rustred_test::unsafe_feynman_parameter"])
        .with_user_data(UserData::Integer(29))
        .build()
        .unwrap();

    assert_eq!(
        authenticate_feynman_symbol(symbol, qualified, parameter),
        Err(FeynmanPolynomialError::FeynmanParameterSymbolCollision { parameter })
    );
}
