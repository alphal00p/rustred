use symbolica::atom::{NamespacedSymbol, SymbolAttribute, SymbolBuilder, UserData};

use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily};

use super::super::context::FeynmanPolynomialContext;
use super::super::error::FeynmanPolynomialError;
use super::super::model::FeynmanPolynomialLimits;

fn six_parameter_vacuum_family(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d"]);
    let denominators = (0..6)
        .map(|coordinate| {
            AffineDenominator::new(
                coefficients.zero(),
                (0..6)
                    .map(|candidate| {
                        if candidate == coordinate {
                            coefficients.one()
                        } else {
                            coefficients.zero()
                        }
                    })
                    .collect(),
            )
        })
        .collect();
    IntegralFamily::new(
        name,
        vec!["k0".into(), "k1".into(), "k2".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        denominators,
        Vec::new(),
        vec![coefficients.zero(); 6],
    )
    .unwrap()
}

#[test]
fn feynman_context_rejects_unsafe_process_global_symbol_squatting() {
    // Native Symanzik tests construct at most five parameters, making the real
    // positional x_5 name fresh regardless of test scheduling.
    let parameter = 5;
    let qualified = "rustred::feynman_x_5";
    let namespaced = NamespacedSymbol::try_parse(qualified).unwrap();
    SymbolBuilder::new(namespaced)
        .with_attributes(&[SymbolAttribute::Symmetric])
        .with_tags(["rustred_test::unsafe_feynman_parameter"])
        .with_user_data(UserData::Integer(29))
        .build()
        .unwrap();
    let family = six_parameter_vacuum_family("feynman-symbol-squatting");

    assert_eq!(
        FeynmanPolynomialContext::try_new(&family, FeynmanPolynomialLimits::default()).unwrap_err(),
        FeynmanPolynomialError::FeynmanParameterSymbolCollision { parameter }
    );
}
