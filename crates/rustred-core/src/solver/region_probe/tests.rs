use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};
use symbolica::prelude::{Integer, Ring};

use crate::algebra::{CoefficientContext, CoefficientPolynomial};

use super::{region_point, vanishes_at};

fn polys(context: &CoefficientContext, expressions: &[&str]) -> Vec<CoefficientPolynomial> {
    expressions
        .iter()
        .map(|e| context.coefficient_fixture(e).numerator)
        .collect()
}

fn base(field: &Zp64, n: usize) -> Vec<symbolica::domains::finite_field::FiniteFieldElement<u64>> {
    (0..n)
        .map(|i| field.to_element(1000 + 17 * i as u64))
        .collect()
}

const P: u64 = (1 << 61) - 1;

/// T266 prefix region R05 on its chart (n0, n1 solved by the chart equations): one quadric in n2, n3, n11, n12
/// (graph type: an infinite non-affine integer family). The point must lie on it and leave `d` alone.
#[test]
fn point_on_the_t266_r05_quadric_and_d_untouched() {
    let context = CoefficientContext::new(["d", "n2", "n3", "n11", "n12"]);
    let system = polys(
        &context,
        &["2*n2^2-10*n2*n3+8*n3^2-7*n2*n11+7*n3*n11-4*n2*n12+4*n3*n12+7*n2-n3+3*n11+4*n12-8"],
    );
    let field = Zp64::new(P);
    let b = base(&field, 5);
    for seed in 0..8 {
        let point = region_point(&system, &b, &field, seed)
            .unwrap()
            .expect("a Zp point of a quadric");
        assert!(vanishes_at(&system, &point, &field));
        assert_eq!(point[0], b[0], "d is not constrained by the region");
    }
    // a polynomial that is not in the ideal does not vanish at the point
    let other = polys(&context, &["n2-n3"]);
    let point = region_point(&system, &b, &field, 1).unwrap().unwrap();
    assert!(!vanishes_at(&other, &point, &field));
}

/// T266 prefix region R12, cubic scroll component (codim 2, three quadric generators; Singular minAssGTZ).
#[test]
fn point_on_a_codim_two_cubic_scroll() {
    let context = CoefficientContext::new(["d", "n0", "n1", "n2", "n3", "n9", "n13"]);
    let system = polys(
        &context,
        &[
            "5*n1^2-2*n0*n2+14*n1*n2-6*n2^2+8*n0*n3-36*n1*n3+18*n2*n3-n3^2+4*n0*n9-8*n1*n9+12*n2*n9-8*n3*n9-2*n0*n13+14*n1*n13+14*n2*n13-26*n3*n13-2*n0-36*n1-48*n2+62*n3-16*n9-22*n13+83",
            "5*n0*n1-8*n0*n2+16*n1*n2-4*n2^2+27*n0*n3-39*n1*n3+12*n2*n3-9*n3^2+16*n0*n9-12*n1*n9+8*n2*n9-12*n3*n9+2*n0*n13+6*n1*n13+16*n2*n13-24*n3*n13-33*n0-9*n1-42*n2+48*n3-24*n9-18*n13+87",
            "5*n0^2-57*n0*n2+54*n1*n2+4*n2^2+173*n0*n3-126*n1*n3+13*n2*n3-66*n3^2+84*n0*n9-48*n1*n9+12*n2*n9-48*n3*n9+18*n0*n13-6*n1*n13+54*n2*n13-66*n3*n13-167*n0+69*n1-103*n2+57*n3-96*n9-42*n13+243",
        ],
    );
    let field = Zp64::new(P);
    let b = base(&field, 7);
    let point = region_point(&system, &b, &field, 3)
        .unwrap()
        .expect("a Zp point of the scroll");
    assert!(vanishes_at(&system, &point, &field));
}

/// Chart constraints are part of the system: a fixed slot and an affine equation hold at the point.
#[test]
fn chart_constraints_hold_at_the_point() {
    let context = CoefficientContext::new(["d", "x", "y", "z"]);
    let system = polys(&context, &["x*y-z^2-3", "x-2", "y-z-1"]);
    let field = Zp64::new(P);
    let b = base(&field, 4);
    let point = region_point(&system, &b, &field, 0)
        .unwrap()
        .expect("x = 2, y = z + 1, z^2 - 2z + 1 = 0");
    assert_eq!(field.from_element(&point[1]), 2);
    // z = 1 (double root), y = 2
    assert_eq!(field.from_element(&point[3]), 1);
    assert_eq!(field.from_element(&point[2]), 2);
}

/// An inconsistent system has no point (and the caller keeps its random point).
#[test]
fn empty_system_has_no_point() {
    let context = CoefficientContext::new(["d", "x", "y"]);
    let field = Zp64::new(P);
    let b = base(&field, 3);
    assert!(
        region_point(&polys(&context, &["x-y", "x-y-1"]), &b, &field, 0)
            .unwrap()
            .is_none()
    );
    // x^2 + 1 has no root mod 2^61-1 (= 3 mod 4): only points over an extension
    assert!(
        region_point(&polys(&context, &["x^2+1"]), &b, &field, 0)
            .unwrap()
            .is_none()
    );
    let _ = Integer::one();
    assert!(field.is_zero(&field.zero()));
}

/// gb-fix: zero-dimensional, and the reduced GREVLEX basis {x^2 - y, y^2 - x} has no univariate element (the old code
/// computed a LEX basis here); the minimal polynomial of x in the quotient (x^4 - x, dimension 4) gives the point.
#[test]
fn zero_dimensional_without_univariate_grevlex_element() {
    let context = CoefficientContext::new(["d", "x", "y"]);
    let system = polys(&context, &["x^2-y", "y^2-x"]);
    let field = Zp64::new(P);
    let b = base(&field, 3);
    for seed in 0..8 {
        let point = region_point(&system, &b, &field, seed)
            .unwrap()
            .expect("x^4 = x has 4 roots mod 2^61-1");
        assert!(vanishes_at(&system, &point, &field));
        assert_eq!(point[0], b[0]);
    }
}

/// gb-fix: the minimal polynomial is found by linear algebra (x^4 - x), and a quotient above the cap is a typed
/// refusal, never a panic.
#[test]
fn minimal_polynomial_and_its_cap() {
    use symbolica::poly::GrevLexOrder;
    use symbolica::poly::groebner::GroebnerBasis;
    let context = CoefficientContext::new(["d", "x", "y"]);
    let field = Zp64::new(P);
    let grevlex: Vec<_> = polys(&context, &["x^2-y", "y^2-x"])
        .iter()
        .map(|p| super::to_zp(p, &field).reorder::<GrevLexOrder>())
        .collect();
    let basis = GroebnerBasis::new(&grevlex, false).system;
    let m = super::minimal_polynomial(&basis, 1, &field, 16).unwrap();
    assert_eq!(m.degree(1), 4);
    assert_eq!(m.nterms(), 2, "x^4 - x");
    let refusal = super::minimal_polynomial(&basis, 1, &field, 3).unwrap_err();
    assert!(
        refusal.0.contains("> 3") || refusal.0.contains("within 3"),
        "{}",
        refusal.0
    );
}

/// gb-fix: an ideal whose LEX basis (y > x) has the univariate element x^65536 - 2 (beyond u16 exponents); the
/// grevlex route never builds it: a point (or none) without a panic.
#[test]
fn lex_degree_beyond_u16_does_not_panic() {
    let context = CoefficientContext::new(["d", "y", "x"]);
    let system = polys(&context, &["y-x^256", "y^256-2"]);
    let field = Zp64::new(P);
    let b = base(&field, 3);
    let out =
        region_point(&system, &b, &field, 0).expect("no refusal: the grevlex basis has y^256 - 2");
    if let Some(point) = out {
        assert!(vanishes_at(&system, &point, &field));
    }
}

/// gb-fix regression: T266 region R56.3.2 on its chart (the system of the region probe of that case: conjunction,
/// fixed slots, affine equations). The old lex step ran to 92 GB and panicked (u16 exponent overflow).
#[test]
fn t266_region_r56_3_2_gets_a_point() {
    let names: Vec<String> = std::iter::once("d".to_string())
        .chain((0..20).map(|i| format!("n{i}")))
        .collect();
    let context = CoefficientContext::new(names.iter().map(|s| s.as_str()));
    let system = polys(
        &context,
        &[
            "3539859790934567213-1094820986595376376*n12+597547484845144856*n12^2+176496254080597462*n12^3-6670551306938790*n12^4-4688235047976480*n12^5-7121008910734338221*n3+306206263336213541*n3*n12+175503964727824716*n3*n12^2+5698074302955380784*n3^2+437659355098540884*n3^2*n12-1654671614278631040*n3^3-1245054999483155689*n2-1071546712408973753*n2*n12-224925759652053744*n2*n12^2+161288080570454640*n2*n3+61071349303833972*n2*n3*n12",
            "71478846410-34379691409*n12+21907306498*n12^2+4417743418*n12^3-240831978*n12^4-151320024060*n3+33384029237*n3*n12-5083099733*n3*n12^2-2113169256*n3*n12^3+119080486812*n3^2-7834114272*n3^2*n12-32830614144*n3^3-8564258976*n2-45712190471*n2*n12-2858109343*n2*n12^2-8781062268*n2*n3+25750150248*n2*n3*n12",
            "1575696235374-1177078092613*n12+746747215258*n12^2+24294413286*n12^3+2462232450*n12^4-3155064504188*n3+1478210364693*n3*n12-552354073837*n3*n12^2+2281768810092*n3^2-503083021728*n3^2*n12+126790155360*n3^2*n12^2-578292048000*n3^3-163792699792*n2-950659661439*n2*n12-32524235207*n2*n12^2-140669331660*n2*n3+588221016456*n2*n3*n12",
            "9926519219971-3110095686372*n12+2899221285312*n12^2+459289730834*n12^3+58534314750*n12^4-19309059001307*n3-337190008673*n3*n12-689502847928*n3*n12^2+14588996463168*n3^2+3834004507308*n3^2*n12-4009568065920*n3^3-1521481864320*n3^3*n12-1488484718863*n2-7315947032131*n2*n12-337547631988*n2*n12^2-1229263078560*n2*n3+3593374097004*n2*n3*n12",
            "25233750652277-26387178125444*n12+8242284368384*n12^2-2616538132142*n12^3+154723913970*n12^4-80515899555749*n3+49705768583789*n3*n12-13283585480136*n3*n12^2+105164274404316*n3^2-22326907092504*n3^2*n12-69537652986240*n3^3+18257782371840*n3^4-10936518941941*n2+6030355777423*n2*n12+3859696108284*n2*n12^2+16537463909340*n2*n3+2601745987488*n2*n3*n12",
            "-141750622558-43829706139*n12+18289210654*n12^2-25646505962*n12^3-2972089590*n12^4+336634473576*n3+114362525819*n3*n12-47969668031*n3*n12^2-298259722284*n3^2-66210433104*n3^2*n12+90102533760*n3^3+66386598924*n2+33435787663*n2*n12+40752890339*n2*n12^2+10565846280*n2*n12^3-19565521140*n2*n3+14286354888*n2*n3*n12",
            "-75419954094+84023665519*n12-43068094894*n12^2+6931792758*n12^3+425922162*n12^4+134716685432*n3-125246744067*n3*n12+28163894167*n3*n12^2-85646452356*n3^2+55043935536*n3^2*n12+19071552384*n3^3-29995913756*n2+122733146289*n2*n12-53177058283*n2*n12^2+41407193124*n2*n3-91659828936*n2*n3*n12+25358031072*n2*n3*n12^2",
            "51071-608547*n12+93342*n12^2-4496*n12^3+97883*n3+859232*n3*n12-76063*n3*n12^2-229632*n3^2-295272*n3^2*n12+92880*n3^3+449767*n2+284734*n2*n12-53153*n2*n12^2-733800*n2*n3-198936*n2*n3*n12+321840*n2*n3^2",
            "-1-2*n12+7*n3+n3*n12-4*n3^2-n2-n2*n12-4*n2*n3+8*n2^2",
            "n4-1",
            "n5-1",
            "n6-1",
            "n7-1",
            "n8-1",
            "n9-1",
            "n10-1",
            "n11-1",
            "n13-1",
            "n14",
            "n15",
            "n16",
            "n17",
            "n18",
            "n19",
            "-n3+n0",
            "-1+n3-2*n2+n1",
        ],
    );
    let field = Zp64::new(P);
    let b = base(&field, 21);
    let point = region_point(&system, &b, &field, 0)
        .unwrap()
        .expect("a Zp point of R56.3.2");
    assert!(vanishes_at(&system, &point, &field));
}
