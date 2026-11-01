//! Exact differential controls for the grouped lane: it must rebuild the
//! generic AtomField polynomial coefficient for coefficient, so the complete
//! weighted lowering (terms and guards) equals the forced generic lane.
use super::rational_weighted::{Lane, compare};
use super::*;

struct Setup {
    family: crate::family::IntegralFamily,
    labels: Vec<Atom>,
    head: Symbol,
    d: Atom,
}

fn setup(loops: usize, tag: &str) -> Setup {
    let (family, _) = super::whole_weighted::family(loops);
    let labels = (0..loops)
        .map(|index| symbol(&format!("grouped_{tag}_{loops}_{index}")).to_atom())
        .collect::<Vec<_>>();
    let d = family
        .coefficient_context()
        .parameter("d")
        .unwrap()
        .to_expression();
    Setup {
        family,
        labels,
        head: dot_head(),
        d,
    }
}

impl Setup {
    fn service(&self) -> FamilyScalarNumeratorService<'_> {
        FamilyScalarNumeratorService::try_new(
            &self.family,
            self.head,
            self.labels.clone(),
            Default::default(),
        )
        .unwrap()
    }
    fn dot(&self, i: usize, j: usize) -> Atom {
        dot(self.head, self.labels[i].clone(), self.labels[j].clone())
    }
    fn base(&self) -> IntegralKey {
        IntegralKey::try_new(vec![3; self.family.denominator_count()]).unwrap()
    }
}

#[test]
fn grouped_physical_prefactors_match_generic_in_two_three_and_five_loops() {
    for loops in [2, 3, 5] {
        let s = setup(loops, "physical");
        let service = s.service();
        let last = loops - 1;
        let g = symbol("grouped_physical_g").to_atom();
        let eps = (Atom::num(4) - &s.d) / Atom::num(2);
        let pole = Atom::num(3) - Atom::num(2) * &eps;
        // Projected-numerator shapes: distinct physical prefactors, scalar
        // products of several degrees, products kept unexpanded, and
        // deliberate cancellations between and inside coefficient classes.
        let numerator = Atom::num(256) / Atom::num(81)
            * g.clone().pow(10)
            * (Atom::num(4) - Atom::num(2) * &eps)
            * (s.dot(0, 1) - s.dot(1, last))
            * (s.dot(last, last) - Atom::num(2) * s.dot(0, last) + s.dot(0, 0)).pow(2)
            - Atom::num(32) / Atom::num(81) * g.clone().pow(10) * (Atom::num(4) - Atom::num(2) * &eps).pow(2)
                / (&pole * (Atom::num(6) - Atom::num(2) * &eps))
                * s.dot(0, 0).pow(2)
                * s.dot(last, 0)
            + Atom::num(5) * g.clone().pow(10) * s.dot(0, last) * s.dot(last, last)
            - Atom::num(5) * g.clone().pow(10) * s.dot(last, 0) * s.dot(last, last)
            + Atom::num(7) / Atom::num(3) * g.clone().pow(10) / &pole * s.dot(0, 1).pow(2)
            - g.pow(10) * s.dot(0, 0);
        for mass in [
            Atom::one(),
            Atom::num(3) / Atom::num(2),
            symbol("grouped_physical_m").to_atom().pow(2),
            Atom::num(2) * symbol("grouped_physical_m").to_atom() * symbol("grouped_physical_n").to_atom().pow(2),
        ] {
            let result = compare(&service, &numerator, &s.base(), &mass, Lane::Grouped);
            assert!(!result.is_zero());
        }
    }
}

#[test]
fn grouped_mass_variables_inside_leaves_merge_and_cancel_like_generic() {
    let s = setup(2, "massleaf");
    let service = s.service();
    let m = symbol("grouped_massleaf_m").to_atom();
    let y = symbol("grouped_massleaf_y").to_atom();
    // y*m*k² and -y*(k²)² share non-numeric parts across leaf groups after
    // the mass power is restored; the constant parts cancel exactly.
    for numerator in [
        &y * &m * s.dot(0, 0) - &y * s.dot(0, 0).pow(2),
        &y * &m * s.dot(0, 1) / (&s.d - Atom::num(3)) - &y * s.dot(0, 0) * s.dot(1, 1),
        &y / &m * s.dot(0, 0).pow(2) + &y * s.dot(1, 1),
    ] {
        compare(&service, &numerator, &s.base(), &m, Lane::Grouped);
    }
}

#[test]
fn grouped_lane_refuses_dependent_leaves_and_inhomogeneous_products() {
    let s = setup(2, "refuse");
    let service = s.service();
    let x = symbol("grouped_refuse_x").to_atom();
    let y = symbol("grouped_refuse_y").to_atom();
    let m = symbol("grouped_refuse_m").to_atom();
    let generic = [
        // (x+1)*y and (2x+2)*y are distinct atoms with dependent values.
        (
            (&x + Atom::one()) * &y * s.dot(0, 0) + (Atom::num(2) * &x + Atom::num(2)) * &y * s.dot(1, 1),
            m.clone(),
        ),
        // An inhomogeneous scalar-product sum with a symbolic mass.
        (&x * (s.dot(0, 0) * s.dot(1, 1) + s.dot(0, 1)) * (s.dot(0, 0) + s.dot(1, 1)), m.clone()),
        // A coefficient inside a scalar-product factor.
        ((&x * s.dot(0, 0) + s.dot(1, 1)) * s.dot(0, 1), Atom::one()),
        // A leaf sum with a vanishing partial sum, x*(y+1) - x*y - x, which
        // the generic lane would rebuild as y.
        (
            (&x * (&y + Atom::one()) - &x * &y - &x + &y) * s.dot(0, 1),
            Atom::one(),
        ),
    ];
    for (numerator, mass) in generic {
        compare(&service, &numerator, &s.base(), &mass, Lane::Generic);
    }
    // The same inhomogeneous sum is exact with a numeric mass.
    let numerator = &x * (s.dot(0, 0) * s.dot(1, 1) + s.dot(0, 1)) * (s.dot(0, 0) + s.dot(1, 1));
    compare(&service, &numerator, &s.base(), &(Atom::num(5) / Atom::num(3)), Lane::Grouped);
}

#[test]
fn grouped_complete_cancellations_match_generic() {
    let s = setup(3, "cancel");
    let service = s.service();
    let x = symbol("grouped_cancel_x").to_atom();
    let m = symbol("grouped_cancel_m").to_atom();
    for mass in [Atom::one(), m] {
        let numerator = &x * (s.dot(0, 1) + s.dot(2, 2)).pow(2)
            - &x * s.dot(0, 1).pow(2)
            - Atom::num(2) * &x * s.dot(0, 1) * s.dot(2, 2)
            - &x * s.dot(2, 2).pow(2)
            + s.dot(1, 2) / (&s.d - Atom::one());
        let result = compare(&service, &numerator, &s.base(), &mass, Lane::Grouped);
        assert!(!result.is_zero());
    }
}
