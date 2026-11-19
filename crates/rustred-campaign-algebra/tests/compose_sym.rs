//! Lane repairflow: COMPOSITE DESCENT with the exact field Q(d) on Symbolica (`compose_sym::SymQd`): the key reappearing
//! with a d-dependent coefficient is solved through (1 - c) exactly; two contributions that cancel exactly in d are not
//! an edge; the coefficient text is the engine's `(P)/(Q)` grammar. Constructed toy (arity 3, every slot a line; test
//! data only). SKIPs without the Symbolica licence (source WS/ops/env.sh).

mod common;
#[path = "../src/compose_sym.rs"]
#[allow(dead_code)]
mod compose_sym;
mod inputs;

use common::licensed_or_skip;
use compose_sym::{Rd, SymQd};
use rustred_campaign_algebra::SymbolicaAlgebra;
use rustred_campaign_core::repairflow::composite::*;
use rustred_campaign_core::replay_base::algebra::Q;
use std::cmp::Ordering;
use std::collections::HashMap;

fn cmp(a: &[i32], b: &[i32]) -> Ordering {
    let s = |k: &[i32]| k.iter().map(|&x| x as i64).sum::<i64>();
    s(a).cmp(&s(b)).then(a.iter().rev().cmp(b.iter().rev()))
}

fn lower(k: &[i32], t: &[i32]) -> bool {
    import_lower(&[true, true, true], k, t, &cmp)
}

struct Cat(HashMap<Vec<i32>, Vec<(Vec<i32>, Rd)>>);

impl Catalogue<SymQd> for Cat {
    fn serve(&self, _: &SymQd, t: &[i32]) -> Result<Service<Rd>, String> {
        Ok(match self.0.get(t) {
            Some(rhs) => Service::Rule {
                rule: RuleUse {
                    table: 0,
                    record: 0,
                    source: "toy".into(),
                    label: 0,
                    record_sha256: "sha256:toy".into(),
                },
                rhs: rhs.clone(),
            },
            None => Service::Unserved("uncovered".into()),
        })
    }
}

#[test]
fn composite_in_qd_solves_the_key_and_drops_exact_zeros() {
    if !licensed_or_skip("composite_in_qd_solves_the_key_and_drops_exact_zeros") {
        return;
    }
    let alg = SymbolicaAlgebra::new().unwrap();
    let f = SymQd::new().unwrap();
    let p = |t: &str| f.parse_d(&alg, t).unwrap();
    // text forms
    assert_eq!(f.text(&p("(3*d-12)/(2*d-6)")), "(3*d-12)/(2*d-6)");
    assert_eq!(f.text(&p("d^2-4")), "d^2-4");
    assert_eq!(f.text(&f.from_q(Q { n: -3, d: 2 }).unwrap()), "(-3)/(2)");
    let key = vec![2, 1, 1];
    let gk = vec![1, 1, 2];
    let mid = vec![1, 2, 1];
    let low = vec![1, 1, 1];
    // I(gK) = (d-4)/(d-3) I(K) + 1/(d-3) I(1,1,1): the key reappears with c = (d-4)/(d-3); 1 - c = 1/(d-3)
    let cat = Cat([(
        gk.clone(),
        vec![(key.clone(), p("(d-4)/(d-3)")), (low.clone(), p("1/(d-3)"))],
    )]
    .into_iter()
    .collect());
    let image = vec![(gk.clone(), Q::ONE)];
    let c = compose_image(
        &f,
        &cat,
        &image,
        &key,
        &lower,
        &cmp,
        &ComposeOptions::default(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(c.rhs.len(), 1);
    assert_eq!(c.rhs[0].0, low);
    assert_eq!(
        f.text(&c.rhs[0].1),
        "1",
        "(1/(d-3)) / (1/(d-3)) = 1 exactly"
    );
    assert!(c.key_coefficient.is_some());
    // two image terms whose rules contribute (d-2)/(d-3) and -(d-2)/(d-3) at I(1,2,1): exact zero, not an edge
    let cat = Cat([
        (gk.clone(), vec![(low.clone(), p("(d-2)/(d-3)"))]),
        (
            mid.clone(),
            vec![(low.clone(), p("(2-d)/(d-3)")), (vec![1, 1, 0], p("d"))],
        ),
    ]
    .into_iter()
    .collect());
    let image = vec![(gk.clone(), Q::ONE), (mid.clone(), Q::ONE)];
    let c = compose_image(
        &f,
        &cat,
        &image,
        &key,
        &lower,
        &cmp,
        &ComposeOptions::default(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(c.dropped_zero, vec![low.clone()]);
    assert_eq!(c.rhs.len(), 1);
    assert_eq!(c.rhs[0].0, vec![1, 1, 0]);
    assert_eq!(f.text(&c.rhs[0].1), "d");
    // the key reappearing with coefficient exactly 1 (as a function of d) cannot be solved for
    let cat = Cat([(
        gk.clone(),
        vec![(key.clone(), p("(d-3)/(d-3)")), (low, p("d"))],
    )]
    .into_iter()
    .collect());
    let r = compose_image(
        &f,
        &cat,
        &[(gk, Q::ONE)],
        &key,
        &lower,
        &cmp,
        &ComposeOptions::default(),
    )
    .unwrap();
    assert_eq!(r.err(), Some(Refusal::KeyCancels));
}
