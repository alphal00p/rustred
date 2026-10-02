use std::collections::BTreeMap;

use super::polynomial::{PolynomialWork, RawPolynomial};
use super::*;
use crate::algebra::{CoefficientContext, IndexedCoefficient, IndexedCoefficientContext};
use crate::family::{
    AffineDenominator, ContractionMomentum as Q, IntegralFamily, ScalarProductCoordinate,
};
use crate::identity::{IndexShift, IntegralShift, ParametricIbpGenerator, TranslatedSourceLimits};

fn family(name: &str, external: bool, scale: bool) -> IntegralFamily {
    let c = CoefficientContext::new(["d", "m0", "m1", "nu", "u", "s", "t", "r"]);
    let (loops, externals, n) = if external { (1, 2, 3) } else { (3, 0, 6) };
    let denominators = (0..n)
        .map(|i| {
            let mass = if i % 2 == 0 {
                c.parameter("m0").unwrap()
            } else {
                c.parameter("m1").unwrap()
            };
            let constant = -(&mass * &c.integer((i + 1) as i64));
            let coefficients = (0..n)
                .map(|j| {
                    if i == j {
                        if scale && i == 0 {
                            c.parameter("u").unwrap()
                        } else {
                            c.one()
                        }
                    } else {
                        c.zero()
                    }
                })
                .collect();
            AffineDenominator::new(constant, coefficients)
        })
        .collect();
    let gram = if external {
        vec![
            vec![c.parameter("s").unwrap(), c.parameter("r").unwrap()],
            vec![c.parameter("r").unwrap(), c.parameter("t").unwrap()],
        ]
    } else {
        vec![]
    };
    let mut power_shifts = vec![c.zero(); n];
    power_shifts[2] = c.parameter("nu").unwrap();
    IntegralFamily::new(
        name,
        (0..loops).map(|i| format!("k{i}")).collect(),
        (0..externals).map(|i| format!("p{i}")).collect(),
        c.clone(),
        c.parameter("d").unwrap(),
        denominators,
        gram,
        power_shifts,
    )
    .unwrap()
}
fn spec(f: &IntegralFamily) -> TangentSourceSpec {
    let contractions = if f.external_count() == 0 {
        [Q::Loop(0), Q::Loop(1), Q::Loop(2)]
    } else {
        [Q::Loop(0), Q::External(0), Q::External(1)]
    };
    let mut recenter = vec![0; f.denominator_count()];
    recenter[2] = 1;
    TangentSourceSpec {
        differentiated_loop: 0,
        protected_denominators: [0, 1],
        contractions,
        recenter: IntegralShift::try_new(recenter).unwrap(),
        multiplier: None,
    }
}
fn completed(g: &ParametricIbpGenerator<'_>) -> crate::identity::CompletedIbpSourceRows {
    let p = g.prepare_ordinary_ibp().unwrap();
    let generated = (0..p.len()).map(|i| p.generate(i)).collect();
    p.complete(generated).unwrap()
}
fn put(
    row: &mut BTreeMap<IndexShift, IndexedCoefficient>,
    k: Vec<i64>,
    c: IndexedCoefficient,
    ctx: &IndexedCoefficientContext,
) {
    let k = IndexShift::try_new(k, ctx.index_count()).unwrap();
    let v = match row.remove(&k) {
        Some(old) => ctx.add(&old, &c).unwrap(),
        None => c,
    };
    if !v.is_zero() {
        row.insert(k, v);
    }
}
fn polynomial_row(
    row: &mut BTreeMap<IndexShift, IndexedCoefficient>,
    p: &RawPolynomial,
    offset: &[i64],
    scale: &IndexedCoefficient,
    ctx: &IndexedCoefficientContext,
) {
    for (c, e) in p.coefficients.iter().zip(p.exponents_iter()) {
        put(
            row,
            offset
                .iter()
                .zip(e)
                .map(|(&b, &a)| b - i64::from(a))
                .collect(),
            ctx.mul(scale, &ctx.lift(c).unwrap()).unwrap(),
            ctx,
        );
    }
}
/// Independent product rule, not source-row enumeration. Includes nonzero
/// native power shifts and every unprotected derivative contribution.
fn product_rule(
    f: &IntegralFamily,
    p: &TangentSourcePlan,
    ctx: &IndexedCoefficientContext,
) -> BTreeMap<IndexShift, IndexedCoefficient> {
    let mut work = PolynomialWork::new(f, TangentSourceLimits::default()).unwrap();
    let s = p.spec();
    let n = f.denominator_count();
    let derivatives: Vec<Vec<_>> = (0..n)
        .map(|i| {
            s.contractions
                .iter()
                .map(|&q| {
                    work.affine(
                        f.derivative_contraction(i, s.differentiated_loop, q)
                            .unwrap(),
                    )
                    .unwrap()
                })
                .collect()
        })
        .collect();
    let mut divergence = work.template.zero();
    for j in 0..3 {
        let a = &p.vector_coefficients()[j].raw;
        if s.contractions[j] == Q::Loop(s.differentiated_loop) {
            divergence = &divergence + &(a * &work.template.constant(f.dimension().clone()));
        }
        for (i, row) in derivatives.iter().enumerate() {
            divergence = &divergence + &(&a.derivative(i) * &row[j]);
        }
    }
    let mut row = BTreeMap::new();
    polynomial_row(&mut row, &divergence, s.recenter.values(), &ctx.one(), ctx);
    for (i, derivatives) in derivatives.iter().enumerate() {
        let mut contracted = work.template.zero();
        for (j, derivative) in derivatives.iter().enumerate() {
            contracted = &contracted + &(&p.vector_coefficients()[j].raw * derivative);
        }
        if s.protected_denominators.contains(&i) {
            assert!(contracted.is_zero());
        }
        let power = ctx
            .add(
                &ctx.add(&ctx.index(i).unwrap(), &ctx.integer(s.recenter.values()[i]))
                    .unwrap(),
                &ctx.lift(&f.power_shifts()[i]).unwrap(),
            )
            .unwrap();
        let mut offset = s.recenter.values().to_vec();
        offset[i] += 1;
        polynomial_row(
            &mut row,
            &contracted,
            &offset,
            &ctx.neg_with_limits(&power, Default::default()).unwrap(),
            ctx,
        );
    }
    row
}

#[test]
fn unequal_masses_and_power_shifts_preserve_full_product_and_spectators() {
    let f = family("tangent-unequal-masses", false, true);
    let g = ParametricIbpGenerator::try_new(&f).unwrap();
    let p = TangentSourcePlan::try_new(&f, &spec(&f), Default::default()).unwrap();
    let original = completed(&g);
    let result = p.materialize(&g, &original, Default::default()).unwrap();
    assert_eq!(result.product(), &product_rule(&f, &p, g.context()));
    assert!(!result.product().is_empty());
    assert_eq!(result.sources().sources().len(), p.contributions().len());
    assert_eq!(result.weights().len(), p.contributions().len());
    // The unprotected D2 depends on the differentiated loop: its arbitrary
    // index must not be silently treated as an absent numerator/spectator.
    assert!(result.product().values().any(|c| {
        g.context()
            .specialize_fixed_indices(c, &[(2, 0)], Default::default())
            .unwrap()
            .0
            != *c
    }));
    // Complete source terms that vanish only at a subsequent point remain.
    assert!(
        result
            .sources()
            .sources()
            .iter()
            .flat_map(|s| s.terms().values())
            .any(|c| g
                .context()
                .specialize(c, &vec![0; f.denominator_count()], Default::default())
                .unwrap()
                .0
                .is_zero())
    );
    let original_terms: usize = result
        .sources()
        .sources()
        .iter()
        .map(|s| s.terms().len())
        .sum();
    let original_conditions: usize = result
        .sources()
        .sources()
        .iter()
        .map(|s| s.nonzero_conditions().len())
        .sum();
    assert_eq!(
        result.conditions().len(),
        original_terms + original_conditions + result.weights().len()
    );
    assert!(
        result
            .conditions()
            .iter()
            .any(|g| !g.polynomial().is_nonzero_constant())
    );
    assert_eq!(result.family_fingerprint(), f.fingerprint());
    assert_eq!(result.context_fingerprint(), g.context().fingerprint());
}

#[test]
fn external_directions_and_optional_multiplier_work_under_default_limits() {
    let f = family("tangent-external", true, false);
    let g = ParametricIbpGenerator::try_new(&f).unwrap();
    let mut s = spec(&f);
    s.multiplier = Some(ScalarProductCoordinate::LoopExternal {
        loop_index: 0,
        external_index: 1,
    });
    let p = TangentSourcePlan::try_new(&f, &s, Default::default()).unwrap();
    let result = p
        .materialize(&g, &completed(&g), Default::default())
        .unwrap();
    assert_eq!(result.product(), &product_rule(&f, &p, g.context()));
    assert!(p.contributions().iter().any(|c| matches!(
        c.row_id(),
        crate::identity::RowId::OrdinaryIbp {
            contraction_momentum: 2,
            differentiated_loop: 0
        }
    )));
    assert!(
        p.vector_coefficients()
            .iter()
            .flat_map(|p| p.terms())
            .any(|(_, e)| e.iter().copied().sum::<u16>() == 3)
    );
}

#[test]
fn direction_permutation_retains_derived_sign_and_scale() {
    let f = family("tangent-permutation", false, false);
    let g = ParametricIbpGenerator::try_new(&f).unwrap();
    let original = completed(&g);
    let s = spec(&f);
    let p = TangentSourcePlan::try_new(&f, &s, Default::default()).unwrap();
    let a = p.materialize(&g, &original, Default::default()).unwrap();
    let mut swapped = s.clone();
    swapped.contractions.swap(0, 1);
    let p2 = TangentSourcePlan::try_new(&f, &swapped, Default::default()).unwrap();
    let b = p2.materialize(&g, &original, Default::default()).unwrap();
    assert_eq!(a.product().len(), b.product().len());
    for (k, c) in a.product() {
        assert_eq!(
            b.product()[k],
            g.context().neg_with_limits(c, Default::default()).unwrap()
        );
    }
    assert_eq!(p2.vector[0].raw, -p.vector[1].raw.clone());
    assert_eq!(p2.vector[1].raw, -p.vector[0].raw.clone());
    assert_eq!(p2.vector[2].raw, -p.vector[2].raw.clone());
}

#[test]
fn degenerate_and_invalid_nominations_are_typed_refusals() {
    let f = family("tangent-input", false, false);
    let s = spec(&f);
    let mut bad = s.clone();
    bad.protected_denominators = [3, 5]; // k1^2 and k2^2: both independent of k0.
    assert!(matches!(
        TangentSourcePlan::try_new(&f, &bad, Default::default()),
        Err(TangentSourceError::DegenerateProtectedSystem)
    ));
    for bad in [
        TangentSourceSpec {
            differentiated_loop: 99,
            ..s.clone()
        },
        TangentSourceSpec {
            protected_denominators: [0, 0],
            ..s.clone()
        },
        TangentSourceSpec {
            protected_denominators: [0, 99],
            ..s.clone()
        },
        TangentSourceSpec {
            contractions: [Q::Loop(0), Q::Loop(0), Q::Loop(1)],
            ..s.clone()
        },
        TangentSourceSpec {
            contractions: [Q::Loop(0), Q::Loop(1), Q::External(0)],
            ..s.clone()
        },
        TangentSourceSpec {
            recenter: IntegralShift::try_new([0]).unwrap(),
            ..s.clone()
        },
    ] {
        assert!(matches!(
            TangentSourcePlan::try_new(&f, &bad, Default::default()),
            Err(TangentSourceError::InvalidInput { .. })
        ));
    }
    let mut bad = s;
    bad.recenter = IntegralShift::try_new(vec![i64::MIN; f.denominator_count()]).unwrap();
    assert!(matches!(
        TangentSourcePlan::try_new(&f, &bad, Default::default()),
        Err(TangentSourceError::ResourceOverflow { .. })
    ));
}

#[test]
fn foreign_or_incomplete_original_transcripts_do_not_materialize() {
    let f = family("tangent-scope", false, false);
    let other = family("other-tangent-scope", false, false);
    let g = ParametricIbpGenerator::try_new(&f).unwrap();
    let other_g = ParametricIbpGenerator::try_new(&other).unwrap();
    let p = TangentSourcePlan::try_new(&f, &spec(&f), Default::default()).unwrap();
    assert!(matches!(
        p.materialize(&other_g, &completed(&other_g), Default::default()),
        Err(TangentSourceError::ScopeMismatch)
    ));
    let mut foreign_context = completed(&g);
    foreign_context.replace_context_fingerprint_for_test("not-this-context");
    assert!(matches!(
        p.materialize(&g, &foreign_context, Default::default()),
        Err(TangentSourceError::ScopeMismatch)
    ));
    let mut wrong_rows = completed(&g);
    assert!(wrong_rows.swap_source_rows_for_test(0, 3));
    assert!(matches!(
        p.materialize(&g, &wrong_rows, Default::default()),
        Err(TangentSourceError::SourceChronologyMismatch)
    ));
    let f = family("tangent-external-only", true, false);
    let g = ParametricIbpGenerator::try_new(&f).unwrap();
    let p = TangentSourcePlan::try_new(&f, &spec(&f), Default::default()).unwrap();
    let prepared = g.prepare_external_ibp_sources().unwrap();
    let generated = (0..prepared.len()).map(|i| prepared.generate(i)).collect();
    let external = prepared.complete(generated).unwrap();
    assert!(matches!(
        p.materialize(&g, &external, Default::default()),
        Err(TangentSourceError::IncompleteOrdinarySources)
    ));
}

#[test]
fn structural_and_materialization_budgets_fail_closed() {
    let f = family("tangent-limits", false, false);
    let s = spec(&f);
    for limits in [
        TangentSourceLimits {
            max_denominators: 1,
            ..Default::default()
        },
        TangentSourceLimits {
            max_polynomial_terms: 1,
            ..Default::default()
        },
        TangentSourceLimits {
            max_polynomial_degree: 1,
            ..Default::default()
        },
        TangentSourceLimits {
            max_exponent_entries: 1,
            ..Default::default()
        },
        TangentSourceLimits {
            max_term_operations: 0,
            ..Default::default()
        },
        TangentSourceLimits {
            max_selected_sources: 0,
            ..Default::default()
        },
    ] {
        assert!(matches!(
            TangentSourcePlan::try_new(&f, &s, limits),
            Err(TangentSourceError::ResourceLimit { .. })
        ));
    }
    let p = TangentSourcePlan::try_new(
        &f,
        &s,
        TangentSourceLimits {
            max_conditions: 0,
            ..Default::default()
        },
    )
    .unwrap();
    let g = ParametricIbpGenerator::try_new(&f).unwrap();
    let original = completed(&g);
    assert!(matches!(
        p.materialize(&g, &original, Default::default()),
        Err(TangentSourceError::ResourceLimit { .. })
    ));
    let p = TangentSourcePlan::try_new(&f, &s, Default::default()).unwrap();
    let limits = TranslatedSourceLimits {
        max_requested_source_translations: 0,
        ..Default::default()
    };
    assert!(matches!(
        p.materialize(&g, &original, limits),
        Err(TangentSourceError::Translated(_))
    ));
}
