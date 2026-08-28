use rustred::ExactRational;
use rustred_legacy_oracles::equal_mass_five_loop_banana;
use rustred_legacy_oracles::{IbpGenerator, Integral};

// Keep every Symbolica-backed structural check on one restricted worker.
#[test]
fn five_loop_banana_foundation() {
    let family = equal_mass_five_loop_banana().unwrap();
    assert_eq!(family.loops(), 5);
    assert_eq!(family.denominator_count(), 15);
    assert_eq!(family.propagator_count(), 6);
    assert_eq!(family.symmetries().len(), 1);
    assert_eq!(family.physical_sectors().len(), 64);

    // Independently substitute all inverse scalar-product rows back into each
    // physical or generated auxiliary denominator.
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

    let mut powers = vec![0; family.denominator_count()];
    powers[..family.propagator_count()].fill(1);
    let identities = IbpGenerator::new(&family).generate_raw(&Integral::new(powers));
    assert_eq!(identities.len(), 25);
    assert!(
        identities
            .iter()
            .all(|identity| !identity.equation.is_zero())
    );
}
