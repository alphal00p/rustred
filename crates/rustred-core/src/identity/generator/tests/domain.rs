use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, CoefficientLocation, IntegralFamily};
use crate::identity::condition::IdentityConditionSource;

use super::super::ParametricIbpGenerator;

#[test]
fn every_row_inherits_input_and_determinant_domain_conditions() {
    let base = CoefficientContext::new(["d", "a", "b", "s", "g"]);
    let family = IntegralFamily::new(
        "conditioned-one-loop-one-leg",
        vec!["k".into()],
        vec!["p".into()],
        base.clone(),
        base.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(
                base.zero(),
                vec![base.coefficient_fixture("a/s"), base.one()],
            ),
            AffineDenominator::new(
                base.zero(),
                vec![base.parameter("b").unwrap(), base.integer(2)],
            ),
        ],
        vec![vec![base.parameter("g").unwrap()]],
        vec![base.zero(), base.zero()],
    )
    .unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let batch = generator.prepare_ordinary_ibp().unwrap();
    let row_results = (0..batch.len())
        .map(|ordinal| batch.generate(ordinal))
        .collect();
    let ordinary = batch.complete(row_results).unwrap().into_relations();
    let determinant = generator
        .context()
        .lift_base_polynomial(family.domain().determinant_nonzero().polynomial())
        .unwrap();
    let input_denominator = generator
        .context()
        .lift_base_polynomial(
            family
                .domain()
                .input_denominators()
                .iter()
                .find(|condition| !condition.polynomial().is_constant())
                .unwrap()
                .polynomial(),
        )
        .unwrap();
    assert_eq!(ordinary.len(), 2);
    assert!(ordinary.iter().all(|row| {
        row.nonzero_conditions()
            .iter()
            .any(|condition| condition.polynomial() == &determinant)
            && row
                .nonzero_conditions()
                .iter()
                .any(|condition| condition.polynomial() == &input_denominator)
    }));
    assert!(ordinary.iter().all(|row| {
        let determinant_condition = row
            .nonzero_conditions()
            .iter()
            .find(|condition| condition.polynomial() == &determinant)
            .unwrap();
        let input_condition = row
            .nonzero_conditions()
            .iter()
            .find(|condition| condition.polynomial() == &input_denominator)
            .unwrap();
        determinant_condition
            .sources()
            .contains(&IdentityConditionSource::FamilyBasisDeterminantNumerator)
            && input_condition.sources().contains(
                &IdentityConditionSource::FamilyInputCoefficientDenominator {
                    location: CoefficientLocation::DenominatorCoefficient {
                        denominator: 0,
                        coordinate: 0,
                    },
                },
            )
    }));
}
