//! Exact differential controls for the grouped lane: it must rebuild the
//! generic AtomField polynomial coefficient for coefficient, so the complete
//! weighted lowering (terms and guards) equals the forced generic lane.
use super::rational_weighted::{Lane, compare};
use super::*;
use crate::scalar_numerator::weighted::polynomial::testing::observed;
use std::collections::HashMap;

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
            - Atom::num(32) / Atom::num(81)
                * g.clone().pow(10)
                * (Atom::num(4) - Atom::num(2) * &eps).pow(2)
                / (&pole * (Atom::num(6) - Atom::num(2) * &eps))
                * s.dot(0, 0).pow(2)
                * s.dot(last, 0)
            + Atom::num(5) * g.clone().pow(10) * s.dot(0, last) * s.dot(last, last)
            - Atom::num(5) * g.clone().pow(10) * s.dot(last, 0) * s.dot(last, last)
            + Atom::num(7) / Atom::num(3) * g.clone().pow(10) / &pole * s.dot(0, 1).pow(2)
            - g.pow(10) * s.dot(0, 0);
        for (mass, lane) in [
            (Atom::one(), Lane::Grouped),
            (Atom::num(3) / Atom::num(2), Lane::Grouped),
            (symbol("grouped_physical_m").to_atom().pow(2), Lane::Grouped),
            // Product masses keep the generic lane (review R7GL-2).
            (
                Atom::num(2)
                    * symbol("grouped_physical_m").to_atom()
                    * symbol("grouped_physical_n").to_atom().pow(2),
                Lane::Generic,
            ),
        ] {
            let result = compare(&service, &numerator, &s.base(), &mass, lane);
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
            (&x + Atom::one()) * &y * s.dot(0, 0)
                + (Atom::num(2) * &x + Atom::num(2)) * &y * s.dot(1, 1),
            m.clone(),
        ),
        // An inhomogeneous scalar-product sum with a symbolic mass.
        (
            &x * (s.dot(0, 0) * s.dot(1, 1) + s.dot(0, 1)) * (s.dot(0, 0) + s.dot(1, 1)),
            m.clone(),
        ),
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
    compare(
        &service,
        &numerator,
        &s.base(),
        &(Atom::num(5) / Atom::num(3)),
        Lane::Grouped,
    );
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

/// Lowers with the default lanes and with the forced generic lane and
/// requires identical terms and guards (or identical errors), whatever lane
/// is taken. Returns whether the grouped lane was taken.
fn differential(
    service: &FamilyScalarNumeratorService<'_>,
    numerator: &Atom,
    base: &IntegralKey,
    mass: &Atom,
) -> bool {
    let (fast, _, _, grouped) = observed(false, || service.lower_weighted(numerator, base, mass));
    let (generic, ..) = observed(true, || service.lower_weighted(numerator, base, mass));
    match (fast, generic) {
        (Ok(fast), Ok(generic)) => {
            assert_eq!(
                fast.terms(),
                generic.terms(),
                "{numerator} with mass {mass}"
            );
            assert_eq!(
                fast.nonzero_conditions(),
                generic.nonzero_conditions(),
                "{numerator} with mass {mass}"
            );
        }
        (Err(fast), Err(generic)) => assert_eq!(fast.to_string(), generic.to_string()),
        other => panic!("lanes disagree for {numerator} with mass {mass}: {other:?}"),
    }
    grouped == 1
}

/// Fresh symbols named `<prefix>_<name>`, registered in the order given.
/// The generic lane's output forms (and so the cases a lane mismatch shows
/// up in) depend on the symbol registration order (review R7GL-1/R7GL-4).
fn registered(prefix: &str, names: &[&str]) -> HashMap<String, Atom> {
    names
        .iter()
        .map(|name| {
            (
                (*name).to_owned(),
                symbol(&format!("{prefix}_{name}")).to_atom(),
            )
        })
        .collect()
}

/// Review R7GL-1: a bare-sum prefactor `P`. Native normalization flattens
/// `1*P` but keeps `c*P`, so the generic sequential sums of two
/// contributions of `P` (`P + 2*P`) are not a merged `3*P`. The grouped lane
/// refuses such inputs and keeps bare sums that receive one contribution
/// per monomial.
#[test]
fn grouped_bare_sum_prefactors_match_generic_in_several_symbol_orders() {
    let s = setup(2, "bare");
    let service = s.service();
    let base = s.base();
    for order in [["c", "a", "w"], ["w", "c", "a"], ["a", "c", "w"]] {
        let symbols = registered(&format!("grouped_bare_{}", order.concat()), &order);
        let (a, c, w) = (&symbols["a"], &symbols["c"], &symbols["w"]);
        let m = symbol(&format!("grouped_bare_{}_m", order.concat())).to_atom();
        let pole = s.dot(0, 0) / (a - c);
        let shared = a + w;
        // The reviewer's M4 (mass 1) and M1 (mass 3/2): one bare sum in
        // two terms with common monomials.
        compare(
            &service,
            &(&shared * s.dot(0, 0).pow(2) + &shared * s.dot(0, 0) + &pole),
            &base,
            &Atom::one(),
            Lane::Generic,
        );
        let difference = a - Atom::num(2) * w;
        compare(
            &service,
            &(&difference * s.dot(0, 1).pow(2) * s.dot(1, 1) + &difference * s.dot(0, 0) + &pole),
            &base,
            &(Atom::num(3) / Atom::num(2)),
            Lane::Generic,
        );
        // Contributions that cancel are contributions too (the generic lane
        // cancels `P - P` only by its statistical zero test).
        compare(
            &service,
            &(&shared * s.dot(0, 0) - &shared * s.dot(1, 1) + &pole),
            &base,
            &Atom::one(),
            Lane::Generic,
        );
        // One term: one contribution per monomial, the grouped lane is exact.
        compare(
            &service,
            &(&shared * s.dot(0, 0) + &pole),
            &base,
            &Atom::one(),
            Lane::Grouped,
        );
        // The reviewer's B1: `P` and `P/m * m` are the same bare part from
        // two leaf groups.
        let p = a + Atom::one() / (w + Atom::num(2));
        compare(
            &service,
            &(&p * (s.dot(1, 1) - s.dot(0, 0)) + &p / &m * (&m * s.dot(0, 0) + &m * s.dot(1, 1))),
            &base,
            &m,
            Lane::Generic,
        );
        // A bare-sum prefix ahead of two sums: the lane depends on the
        // factor order the generic lane multiplies in; the result never does.
        for numerator in [
            &shared * (s.dot(0, 0) + s.dot(1, 1)) * (s.dot(0, 0) - Atom::num(2) * s.dot(1, 1)),
            &shared * c * (s.dot(0, 0) + s.dot(1, 1)) * (s.dot(0, 0) - s.dot(0, 1)) + &pole,
            &shared * (s.dot(0, 0) + s.dot(0, 1)).pow(2) * (s.dot(1, 1) + s.dot(0, 1)) + &pole,
        ] {
            for mass in [Atom::one(), m.clone(), m.clone().pow(2)] {
                differential(&service, &numerator, &base, &mass);
            }
        }
    }
}

/// Review R7GL-2: with a product mass a homogeneous sum mixes mass monomials
/// (`m*n` and `m^2` under mass `m*n`); the generic lane keeps them inside one
/// product, so product masses keep the generic lane.
#[test]
fn grouped_lane_refuses_product_masses() {
    let s = setup(2, "product");
    let service = s.service();
    let x = symbol("grouped_product_x").to_atom();
    let y = symbol("grouped_product_y").to_atom();
    let m = symbol("grouped_product_m").to_atom();
    let n = symbol("grouped_product_n").to_atom();
    for (numerator, mass) in [
        (&x * (s.dot(0, 0) + m.clone().pow(2)), &m * &n),
        (
            (s.dot(0, 0) + m.clone().pow(2)) * (s.dot(1, 1) - n.clone().pow(2)) / (&x - &y),
            &m * &n,
        ),
        (
            &x * (s.dot(0, 0) + m.clone().pow(3)) + &y * (s.dot(1, 1) + &m * n.clone().pow(2)),
            Atom::num(2) * &m * n.clone().pow(2),
        ),
    ] {
        compare(&service, &numerator, &s.base(), &mass, Lane::Generic);
    }
}

/// Review R7GL-3: the independence proof samples independent pseudo-random
/// residues per symbol and point, so plain symbols and polynomial leaves
/// in few symbols are proven independent (the former affine sample put every
/// point on one conic), and large exponents cost a modular power.
#[test]
fn grouped_independence_proof_accepts_independent_leaves() {
    let s = setup(2, "proof");
    let service = s.service();
    let g = symbol("grouped_proof_g").to_atom();
    let h = symbol("grouped_proof_h").to_atom();
    let q = symbol("grouped_proof_q").to_atom();
    let x = symbol("grouped_proof_x").to_atom();
    let y = symbol("grouped_proof_y").to_atom();
    let three = &g * s.dot(0, 0) + &h * s.dot(0, 1) + &q * s.dot(1, 1);
    compare(&service, &three, &s.base(), &Atom::one(), Lane::Grouped);
    let mut monomials = Atom::Zero;
    for i in 0..7 {
        for j in 0..7 {
            monomials = monomials
                + Atom::num(i + 2 * j + 1) * x.clone().pow(i) * y.clone().pow(j) * s.dot(0, 0)
                - x.clone().pow(j) * y.clone().pow(i) * s.dot(0, 1);
        }
    }
    compare(&service, &monomials, &s.base(), &Atom::one(), Lane::Grouped);
    let large = x.clone().pow(100_000) * s.dot(0, 0) + &y * s.dot(1, 1);
    compare(&service, &large, &s.base(), &Atom::one(), Lane::Grouped);
    // Dependent leaves are still refused.
    let dependent = &g * s.dot(0, 0) + &h * s.dot(0, 1) + (&g + &h) * s.dot(1, 1);
    compare(&service, &dependent, &s.base(), &Atom::one(), Lane::Generic);
}

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[(self.next() % items.len() as u64) as usize]
    }
}

/// Review R7GL-4: a seeded lane-differential fuzz (complete lowerings, terms
/// and guards) over physical-looking prefactors, bare sums shared by several
/// terms, mass variables in leaves and in the polynomial parts, numeric,
/// single-variable and product masses, in two symbol registration orders.
#[test]
fn grouped_lane_differential_fuzz_in_two_symbol_orders() {
    let s = setup(2, "fuzz");
    let service = s.service();
    let base = s.base();
    let names = ["x", "y", "z", "m", "n"];
    let mut reversed = names;
    reversed.reverse();
    for (tag, order) in [("forward", names), ("reversed", reversed)] {
        let v = registered(&format!("grouped_fuzz_{tag}"), &order);
        let (x, y, z, m, n) = (&v["x"], &v["y"], &v["z"], &v["m"], &v["n"]);
        let masses = [
            Atom::one(),
            Atom::num(3) / Atom::num(2),
            m.clone(),
            m.clone().pow(2),
            m * n,
            Atom::num(2) * m * n.clone().pow(2),
        ];
        let numbers = [
            Atom::one(),
            Atom::num(-1),
            Atom::num(2),
            Atom::num(-1) / Atom::num(2),
            Atom::num(3) / Atom::num(4),
        ];
        let bare = x + Atom::one() / (y + Atom::num(2));
        let mut grouped = 0;
        let mut rng = Lcg(0xfeed_beef_1234);
        let cases = 1500;
        for case in 0..cases {
            let mass = &masses[case % masses.len()];
            let symbolic = case % masses.len() >= 2;
            let mut leaves = vec![
                x.clone(),
                y.clone(),
                x + Atom::one(),
                x + y,
                Atom::one() / (x + Atom::one()),
                Atom::one() / (y + Atom::num(2)),
                Atom::one() / (x - y),
                Atom::one() / (y - x),
                x.clone().pow(2),
                x * y,
                bare.clone(),
                z / (z - Atom::num(3)),
            ];
            if symbolic {
                leaves.extend([
                    Atom::one() / m,
                    Atom::one() / m.clone().pow(2),
                    Atom::one() / (m + x),
                    m * x,
                    &bare / m,
                    &bare / mass,
                    Atom::one() / n,
                ]);
            }
            let mut parts = vec![
                s.dot(0, 0),
                s.dot(0, 1),
                s.dot(1, 1),
                s.dot(0, 0) - s.dot(1, 1),
                s.dot(0, 0) + Atom::num(2) * s.dot(0, 1) + s.dot(1, 1),
                s.dot(0, 0) + mass,
                s.dot(1, 1) - mass,
                s.dot(0, 1) + Atom::num(1) / Atom::num(2) * mass,
                (s.dot(0, 0) + mass).pow(2),
                s.dot(0, 0) * mass - mass.clone().pow(2),
                mass * s.dot(0, 0) + mass * s.dot(1, 1),
            ];
            if case % masses.len() == 4 {
                parts.extend([
                    s.dot(0, 0) + m.clone().pow(2),
                    s.dot(1, 1) - n.clone().pow(2),
                ]);
            }
            // A third of the cases share one prefactor between all terms.
            let shared = (rng.next() % 3 == 0).then(|| rng.pick(&leaves).clone());
            let mut numerator = Atom::Zero;
            for _ in 0..1 + rng.next() % 4 {
                let mut term = rng.pick(&numbers).clone();
                if let Some(shared) = &shared {
                    term = term * shared;
                } else {
                    for _ in 0..rng.next() % 3 {
                        term = term * rng.pick(&leaves);
                    }
                }
                for _ in 0..1 + rng.next() % 2 {
                    term = term * rng.pick(&parts);
                }
                numerator = numerator + term;
            }
            grouped += usize::from(differential(&service, &numerator, &base, mass));
        }
        // The fuzz exercises the grouped lane, not only its refusals.
        assert!(grouped > cases / 2, "{tag}: grouped {grouped} of {cases}");
    }
}
