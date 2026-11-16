//! Lane poles, B2: the node-basis records written by the core's adapter from an `rr_node symmasters` report
//! (rustred_campaign_core::poles::basis::node_basis_from_symmasters; the frozen engine has no node-basis job kind) pass
//! the CONTRACT's reader (`rustred_campaign_contract::node_basis::check_results`, the reader the verified-results path
//! of kind node-basis uses), and the screen reads rank_T at d0 against the generic dims. A constructed report, then the
//! real route E reports of this lane when present (scratch, skipped otherwise). Pure: spawns nothing.

use rustred_campaign_contract::arith::{parse_prime_list, parse_rational_list};
use rustred_campaign_contract::node_basis::{NodeBasisArgs, check_results};
use rustred_campaign_core::poles::basis::{node_basis_from_symmasters, screen_of_node_basis};
use serde_json::{Value, json};

const P1: &str = "2305843009213693951";
const P2: &str = "2305843009213693921";

fn lines(recs: &[Value]) -> Vec<Vec<u8>> {
    recs.iter()
        .map(|r| serde_json::to_vec(r).unwrap())
        .collect()
}

fn args(dims: &str, primes: &str) -> NodeBasisArgs {
    NodeBasisArgs {
        dims: Some(parse_rational_list(dims).unwrap()),
        primes: Some(parse_prime_list(primes).unwrap()),
    }
}

/// A constructed symmasters report: terminals a, b, c; at d = 4 c is pivoted (c = 3a - 3/2 b), generic all free.
fn report() -> Value {
    let s = |d: &str, p: &str, free: &[&str]| {
        json!({"d": d, "p": p, "seeds": 10, "rows_ibp": 50, "rows_sym": 5, "cols": 60, "pivots_ibp_T": 40,
            "VG_M": free, "unconstrained_inner": 0, "leaks": 0})
    };
    json!({
        "samples": [s("4/1", P1, &["a", "b", "x"]), s("4/1", P2, &["a", "b", "x"]),
                    s("17/7", P1, &["a", "b", "c"]), s("17/7", P2, &["a", "b", "c"]),
                    s("19/7", P1, &["a", "b", "c"]), s("19/7", P2, &["a", "b", "c"])],
        "exact": [
            {"d": "4/1", "masters": {"a": {"a": "1"}, "b": {"b": "1"}, "c": {"a": "3", "b": "-3/2"}}},
            {"d": "17/7", "masters": {"a": {"a": "1"}, "b": {"b": "1"}, "c": {"c": "1"}}},
            {"d": "19/7", "masters": {"a": {"a": "1"}, "b": {"b": "1"}, "c": {"c": "1"}}},
        ],
        "box": [2, 1],
    })
}

#[test]
fn adapter_records_pass_the_contract_reader() {
    let t: Vec<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
    let recs = node_basis_from_symmasters(&report(), &t, "4").unwrap();
    let sum = check_results(&lines(&recs), &args("4,17/7,19/7", &format!("{P1},{P2}"))).unwrap();
    assert_eq!(
        (sum.samples, sum.dims, sum.verdict.as_str()),
        (6, 3, "degenerate")
    );
    let s = screen_of_node_basis(&recs, "4");
    assert_eq!(s["rank_T_d0"], 2);
    assert_eq!(s["rank_T_generic"], json!([3, 3]));
    assert_eq!(s["deficiency"], 1);
    assert_eq!(
        s["null_directions_d0"],
        json!([[["c", "1"], ["a", "-3"], ["b", "3/2"]]])
    );
    // a report missing a prime's sample is refused by the contract reader (records incomplete for the job's args)
    let mut r = report();
    r["samples"].as_array_mut().unwrap().remove(1);
    let recs = node_basis_from_symmasters(&r, &t, "4").unwrap();
    assert!(check_results(&lines(&recs), &args("4,17/7,19/7", &format!("{P1},{P2}"))).is_err());
    // samples disagreeing between primes: unresolved, never regular
    let mut r = report();
    r["samples"][1]["VG_M"] = json!(["a", "b", "c"]);
    let recs = node_basis_from_symmasters(&r, &t, "4").unwrap();
    let sum = check_results(&lines(&recs), &args("4,17/7,19/7", &format!("{P1},{P2}"))).unwrap();
    assert_eq!(sum.verdict, "unresolved");
}

/// The lane's real route E reports (frozen rr_node symmasters), when present: the records pass the reader; the screen is
/// printed (it never decides: the system can lose rank at d0 itself).
#[test]
fn real_route_e_reports() {
    let base = "/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/poles/routeE";
    let n0 = ",0,0,0,0,0,0";
    let z11 = ",0,0,0,0,0,0,0,0,0,0,0";
    let c266 = format!("I(1,1,1,1,1,1,1,1,1,1,1,1,1,1{n0})");
    let c162 = format!("I(1,1,1,1,1,1,1,1,1{z11})");
    let cases = [
        (
            "t266-T",
            vec![
                c266.clone(),
                format!("I(2,1,1,1,1,1,1,1,1,1,1,1,1,1{n0})"),
                format!("I(1,1,1,1,2,1,1,1,1,1,1,1,1,1{n0})"),
            ],
        ),
        (
            "t266-L",
            vec![
                c266.clone(),
                format!("I(2,1,1,1,1,1,1,1,1,1,1,1,1,1{n0})"),
                format!("I(3,1,1,1,1,1,1,1,1,1,1,1,1,1{n0})"),
            ],
        ),
        (
            "t162-T",
            vec![c162.clone(), format!("I(1,1,1,2,1,1,1,1,1{z11})")],
        ),
        (
            "t162-P",
            vec![c162.clone(), format!("I(1,1,1,1,1,2,1,1,1{z11})")],
        ),
        (
            "t162-R",
            vec![
                format!("I(1,1,1,1,2,1,1,1,1{z11})"),
                format!("I(1,1,1,1,1,1,2,2,1{z11})"),
            ],
        ),
        (
            "t162b21-T",
            vec![c162.clone(), format!("I(1,1,1,2,1,1,1,1,1{z11})")],
        ),
        (
            "t162b21-P",
            vec![c162.clone(), format!("I(1,1,1,1,1,2,1,1,1{z11})")],
        ),
        (
            "t162b21-R",
            vec![
                format!("I(1,1,1,1,2,1,1,1,1{z11})"),
                format!("I(1,1,1,1,1,1,2,2,1{z11})"),
            ],
        ),
    ];
    for (tag, terms) in cases {
        let p = format!("{base}/{tag}/OUT.json");
        let Ok(b) = std::fs::read(&p) else {
            eprintln!("SKIP (not run yet): {p}");
            continue;
        };
        let doc: Value = serde_json::from_slice(&b).unwrap();
        let primes: Vec<String> = doc["samples"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["p"].as_str().unwrap().to_string())
            .collect();
        let mut uniq = primes.clone();
        uniq.sort();
        uniq.dedup();
        let recs = node_basis_from_symmasters(&doc, &terms, "4").unwrap();
        let sum = check_results(&lines(&recs), &args("4,17/7,19/7", &uniq.join(","))).unwrap();
        let s = screen_of_node_basis(&recs, "4");
        eprintln!(
            "{tag}: contract reader OK ({} samples, {} dims, end verdict {}); screen {s}",
            sum.samples, sum.dims, sum.verdict
        );
    }
}
