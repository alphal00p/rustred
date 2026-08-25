#![cfg(feature = "legacy-authored-oracles")]

use std::collections::BTreeSet;

use rustred::three_loop::{
    THREE_LOOP_TETRAHEDRON_EDGES, THREE_LOOP_TETRAHEDRON_ROUTINGS,
    THREE_LOOP_TETRAHEDRON_SYMMETRY_GENERATORS, equal_mass_three_loop_tetrahedron,
};
use rustred::{
    ExactRational, IbpGenerator, Integral, LinearCombination, ReductionTable, SeedConfig,
    SparseReducer, VacuumFamily, generate_seeds,
};

fn rational(numerator: i64, denominator: i64) -> ExactRational {
    ExactRational::new(numerator, denominator)
}

fn check_complete_basis_and_inverse(family: &VacuumFamily) {
    assert_eq!(family.name(), "equal_mass_three_loop_tetrahedron");
    assert_eq!(family.loops(), 3);
    assert_eq!(family.denominator_count(), 6);
    assert_eq!(family.propagator_count(), 6);
    assert!(
        family
            .denominators()
            .iter()
            .all(|d| d.normalization() == Some(1))
    );

    let half = rational(1, 2);
    let minus_half = rational(-1, 2);
    let zero = ExactRational::zero();
    let one = ExactRational::one();
    let expected = [
        (
            (0, 0),
            "-m2",
            vec![
                one.clone(),
                zero.clone(),
                zero.clone(),
                zero.clone(),
                zero.clone(),
                zero.clone(),
            ],
        ),
        (
            (0, 1),
            "-m2/2",
            vec![
                half.clone(),
                half.clone(),
                zero.clone(),
                zero.clone(),
                minus_half.clone(),
                zero.clone(),
            ],
        ),
        (
            (0, 2),
            "-m2/2",
            vec![
                half.clone(),
                zero.clone(),
                half.clone(),
                minus_half.clone(),
                zero.clone(),
                zero.clone(),
            ],
        ),
        (
            (1, 1),
            "-m2",
            vec![
                zero.clone(),
                one.clone(),
                zero.clone(),
                zero.clone(),
                zero.clone(),
                zero.clone(),
            ],
        ),
        (
            (1, 2),
            "-m2/2",
            vec![
                zero.clone(),
                half.clone(),
                half.clone(),
                zero.clone(),
                zero.clone(),
                minus_half.clone(),
            ],
        ),
        (
            (2, 2),
            "-m2",
            vec![
                zero.clone(),
                zero.clone(),
                one.clone(),
                zero.clone(),
                zero.clone(),
                zero.clone(),
            ],
        ),
    ];

    for ((left, right), constant, coefficients) in &expected {
        let expansion = family.scalar_product_expansion(*left, *right).unwrap();
        assert_eq!(
            expansion.constant(),
            &family.coefficients().parse(constant).unwrap()
        );
        assert_eq!(expansion.denominator_coefficients(), coefficients);
    }

    // Independently substitute all six inverse scalar-product formulae back
    // into every q_i^2+m2.  Each result must be precisely D_i.
    for (denominator_index, denominator) in family.denominators().iter().enumerate() {
        let mut constant = denominator.shift().clone();
        let mut coefficients = vec![ExactRational::zero(); 6];
        for (scalar_product, ((left, right), _, _)) in expected.iter().enumerate() {
            let factor = &denominator.quadratic_form()[scalar_product];
            if factor.is_zero() {
                continue;
            }
            let expansion = family.scalar_product_expansion(*left, *right).unwrap();
            constant = &constant
                + &family
                    .coefficients()
                    .scale_rational(expansion.constant(), factor);
            for (target, coefficient) in expansion.denominator_coefficients().iter().enumerate() {
                let contribution = factor * coefficient;
                coefficients[target] = &coefficients[target] + &contribution;
            }
        }
        assert!(constant.is_zero());
        assert_eq!(
            coefficients,
            (0..6)
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

fn vertex_permutations() -> Vec<[usize; 4]> {
    let mut permutations = Vec::with_capacity(24);
    for first in 0..4 {
        for second in 0..4 {
            for third in 0..4 {
                for fourth in 0..4 {
                    let candidate = [first, second, third, fourth];
                    let values: BTreeSet<_> = candidate.into_iter().collect();
                    if values.len() == 4 {
                        permutations.push(candidate);
                    }
                }
            }
        }
    }
    permutations
}

fn induced_edge_permutation(vertex_permutation: [usize; 4]) -> Vec<usize> {
    THREE_LOOP_TETRAHEDRON_EDGES
        .iter()
        .map(|edge| {
            let mut mapped = [vertex_permutation[edge[0]], vertex_permutation[edge[1]]];
            mapped.sort();
            THREE_LOOP_TETRAHEDRON_EDGES
                .iter()
                .position(|candidate| *candidate == mapped)
                .expect("a vertex permutation maps an edge of K4 to another edge")
        })
        .collect()
}

fn check_proven_symmetry_group(family: &VacuumFamily) {
    assert_eq!(THREE_LOOP_TETRAHEDRON_ROUTINGS.len(), 6);
    assert_eq!(family.symmetries().len(), 24);

    // Derive the complete edge action directly from all four-vertex
    // permutations, independently of VacuumFamily's loop-momentum proof.
    let graph_automorphisms: BTreeSet<Vec<usize>> = vertex_permutations()
        .into_iter()
        .map(induced_edge_permutation)
        .collect();
    let accepted: BTreeSet<Vec<usize>> = family.symmetries().iter().cloned().collect();
    assert_eq!(graph_automorphisms.len(), 24);
    assert_eq!(accepted, graph_automorphisms);
    for generator in THREE_LOOP_TETRAHEDRON_SYMMETRY_GENERATORS {
        assert!(accepted.contains(generator.as_slice()));
    }
}

fn transformed_sector(mask: u8, permutation: &[usize]) -> Vec<i32> {
    permutation
        .iter()
        .map(|&source| i32::from(mask & (1 << source) != 0))
        .collect()
}

fn powers_mask(powers: &[i32]) -> u8 {
    powers.iter().enumerate().fold(0, |mask, (index, power)| {
        mask | (u8::from(*power > 0) << index)
    })
}

fn check_all_sector_statistics(family: &VacuumFamily) {
    let mut total_by_lines = [0_usize; 7];
    let mut zero_by_lines = [0_usize; 7];
    let mut nonzero_by_lines = [0_usize; 7];
    let mut zero_representatives = BTreeSet::new();
    let mut nonzero_representatives = BTreeSet::new();

    for mask in 0_u8..64 {
        let powers: Vec<i32> = (0..6)
            .map(|index| i32::from(mask & (1 << index) != 0))
            .collect();
        let integral = Integral::new(powers);
        let lines = mask.count_ones() as usize;
        total_by_lines[lines] += 1;

        let representative = family
            .symmetries()
            .iter()
            .map(|permutation| transformed_sector(mask, permutation))
            .max()
            .expect("the symmetry group contains the identity");
        let representative = powers_mask(&representative);

        if family.is_scaleless(&integral) {
            zero_by_lines[lines] += 1;
            zero_representatives.insert(representative);
            assert_eq!(family.canonicalize(&integral), None);
        } else {
            nonzero_by_lines[lines] += 1;
            nonzero_representatives.insert(representative);
            assert_eq!(
                family.canonicalize(&integral),
                Some(Integral::new(transformed_sector(
                    representative,
                    &(0..6).collect::<Vec<_>>(),
                )))
            );
        }
    }

    // A sector is non-scaleless exactly when its K4 edge subgraph is
    // connected.  There are 38 connected and 26 disconnected labelled graphs
    // on four vertices, split by edge count as follows.
    assert_eq!(total_by_lines, [1, 6, 15, 20, 15, 6, 1]);
    assert_eq!(zero_by_lines, [1, 6, 15, 4, 0, 0, 0]);
    assert_eq!(nonzero_by_lines, [0, 0, 0, 16, 15, 6, 1]);
    assert_eq!(zero_by_lines.iter().sum::<usize>(), 26);
    assert_eq!(nonzero_by_lines.iter().sum::<usize>(), 38);

    // The S4 quotient is the eleven unlabeled graphs on four vertices: five
    // disconnected zero-sector types and six connected nonzero types.
    assert_eq!(zero_representatives, BTreeSet::from([0, 1, 3, 19, 33]));
    assert_eq!(
        nonzero_representatives,
        BTreeSet::from([7, 11, 15, 31, 43, 63])
    );
}

fn check_raw_ibp_oracle_and_invariant(family: &VacuumFamily) {
    let corner = Integral::from([1, 1, 1, 1, 1, 1]);
    let identities = IbpGenerator::new(family).generate_raw(&corner);
    assert_eq!(identities.len(), 9);

    // Independent hand derivation of d/dk1.k1.  Only D1, D4, and D5 depend
    // on k1, with contractions 2(D1-m2), D1-D3+D4-m2, and
    // D1-D2+D5-m2 respectively.
    let mut expected = LinearCombination::new();
    for (powers, coefficient) in [
        ([1, 1, 1, 1, 1, 1], "d-4"),
        ([2, 1, 1, 1, 1, 1], "2*m2"),
        ([0, 1, 1, 2, 1, 1], "-1"),
        ([1, 1, 0, 2, 1, 1], "1"),
        ([1, 1, 1, 2, 1, 1], "m2"),
        ([0, 1, 1, 1, 2, 1], "-1"),
        ([1, 0, 1, 1, 2, 1], "1"),
        ([1, 1, 1, 1, 2, 1], "m2"),
    ] {
        expected.add_term(
            Integral::from(powers),
            family.coefficients().parse(coefficient).unwrap(),
        );
    }
    assert_eq!(identities[0].differentiated_loop, 0);
    assert_eq!(identities[0].contraction_loop, 0);
    assert_eq!(identities[0].equation, expected);

    // At k1=k2=k3=0 every denominator equals m2 and every derivative
    // contraction vanishes.  Weighting a shifted integral by
    // m2^(-sum(delta powers)) must therefore leave only d*delta_ij.  This
    // checks all nine raw identities independently of the selected oracle.
    let mass = family.coefficients().parameter("m2").unwrap();
    let inverse_mass = &family.coefficients().one() / &mass;
    let corner_degree: i64 = corner.powers().iter().map(|&power| i64::from(power)).sum();
    for identity in &identities {
        let mut value = family.coefficients().zero();
        for (integral, coefficient) in identity.equation.terms() {
            let degree: i64 = integral
                .powers()
                .iter()
                .map(|&power| i64::from(power))
                .sum();
            let weight = match degree - corner_degree {
                0 => family.coefficients().one(),
                1 => inverse_mass.clone(),
                delta => panic!("unexpected total IBP shift {delta}"),
            };
            value = &value + &(coefficient * &weight);
        }
        let expected = if identity.differentiated_loop == identity.contraction_loop {
            family.dimension().clone()
        } else {
            family.coefficients().zero()
        };
        assert_eq!(value, expected);
    }
}

fn terminal_integrals(table: &ReductionTable) -> BTreeSet<Integral> {
    let mut terminals = BTreeSet::new();
    for pivot in table.rules().keys() {
        let normal_form = table.reduce_integral(pivot).unwrap();
        for integral in normal_form.terms().keys() {
            assert!(!table.rules().contains_key(integral));
            terminals.insert(integral.clone());
        }
    }
    terminals
}

fn check_low_depth_sparse_stability(family: &VacuumFamily) {
    // This is deliberately only a corner-sector exploration.  It demonstrates
    // deterministic elimination and stable terminal candidates; it is not a
    // complete three-loop reduction or a proof of the master basis.
    let seeds = generate_seeds(family, SeedConfig::default());
    assert_eq!(seeds.len(), 6);
    let identities = IbpGenerator::new(family).generate_for_seeds(&seeds);
    assert_eq!(identities.len(), 54);

    let first = SparseReducer::new(family.clone())
        .reduce(&identities)
        .unwrap();
    let second = SparseReducer::new(family.clone())
        .reduce(&identities)
        .unwrap();
    assert!(first.stats().rules > 0);
    assert_eq!(first.stats(), second.stats());
    assert_eq!(first.rules(), second.rules());
    first.validate_identities(&identities).unwrap();

    let first_terminals = terminal_integrals(&first);
    let second_terminals = terminal_integrals(&second);
    assert!(!first_terminals.is_empty());
    assert_eq!(first_terminals, second_terminals);
    assert!(first_terminals.iter().all(|integral| {
        family.canonicalize(integral) == Some(integral.clone()) && !family.is_scaleless(integral)
    }));
}

// Restricted Symbolica must remain on one test worker.  Keep the complete
// three-loop structural and bounded exploratory suite in one test function.
#[test]
fn three_loop_tetrahedron_foundation() {
    let family = equal_mass_three_loop_tetrahedron().unwrap();
    check_complete_basis_and_inverse(&family);
    check_proven_symmetry_group(&family);
    check_all_sector_statistics(&family);
    check_raw_ibp_oracle_and_invariant(&family);
    check_low_depth_sparse_stability(&family);
}
