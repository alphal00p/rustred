use rustred::ExactRational;
use rustred_legacy_oracles::four_loop::{FourLoopTopology, equal_mass_four_loop_vacuum};
use rustred_legacy_oracles::{IbpGenerator, Integral};

fn reconstruct_denominator_rows(family: &rustred_legacy_oracles::VacuumFamily) {
    for (denominator_index, denominator) in family.denominators().iter().enumerate() {
        let mut constant = denominator.shift().clone();
        let mut coefficients = vec![ExactRational::zero(); family.denominator_count()];
        let mut scalar_product = 0;
        for left in 0..family.loops() {
            for right in left..family.loops() {
                let factor = &denominator.quadratic_form()[scalar_product];
                scalar_product += 1;
                if factor.is_zero() {
                    continue;
                }
                let expansion = family.scalar_product_expansion(left, right).unwrap();
                constant = &constant
                    + &family
                        .coefficients()
                        .scale_rational(expansion.constant(), factor);
                for (target, coefficient) in expansion.denominator_coefficients().iter().enumerate()
                {
                    let contribution = factor * coefficient;
                    coefficients[target] = &coefficients[target] + &contribution;
                }
            }
        }
        assert!(constant.is_zero());
        assert_eq!(
            coefficients,
            (0..family.denominator_count())
                .map(|target| {
                    if target == denominator_index {
                        ExactRational::one()
                    } else {
                        ExactRational::zero()
                    }
                })
                .collect::<Vec<_>>()
        );
    }
}

// Keep all Symbolica-backed checks in one process/test worker.
#[test]
fn four_loop_vakint_family_foundations() {
    for topology in FourLoopTopology::ALL {
        let family = equal_mass_four_loop_vacuum(topology).unwrap();
        assert_eq!(family.name(), topology.name());
        assert_eq!(family.loops(), 4);
        assert_eq!(family.denominator_count(), 10);
        assert_eq!(family.propagator_count(), topology.routings().len());
        assert_eq!(family.symmetries().len(), 1);
        assert!(
            family.denominators()[..family.propagator_count()]
                .iter()
                .all(|denominator| denominator.normalization() == Some(1))
        );
        assert!(
            family.denominators()[family.propagator_count()..]
                .iter()
                .all(|denominator| !denominator.is_propagator())
        );
        reconstruct_denominator_rows(&family);

        let mut powers = vec![0; family.denominator_count()];
        powers[..family.propagator_count()].fill(1);
        let raw = IbpGenerator::new(&family).generate_raw(&Integral::new(powers));
        assert_eq!(raw.len(), 16);
        assert!(raw.iter().all(|identity| !identity.equation.is_zero()));
    }
}
