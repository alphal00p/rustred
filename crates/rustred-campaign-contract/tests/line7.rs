//! Line 7 of the contract (lane contract-line7, 2026-11-14; child-job.md "Line 7", coord-pole-engine.md 8): argument
//! shapes Rational / RationalList / UIntList with the prime rule (deterministic primality against an oracle), kind
//! solve-recipe's probe mode and m1 cap, the line-7 members of relation records, kind node-basis, proof-instantiate
//! `--elided-loci`, effective node_options, canonical form v5, decision rows 30 / 31 / 32 (end to end through
//! `assess`). Pure except the end-to-end tests, which write run directories under TMPDIR and spawn nothing.

mod common;
use common::*;
use rustred_campaign_contract::ExitCode;
use rustred_campaign_contract::arith::{
    self, MR_BASES, PROBE_DEFAULT_PRIMES, Rational, is_prime_u64,
};
use rustred_campaign_contract::canon::{VOLATILE_KEYS_VERSION, canonical_line};
use rustred_campaign_contract::classify::recipe::{
    self, FailureClass, KeyLine, KeyOutcome, KeyRoute, ProbeVerdict, RelationTally, Verdict,
};
use rustred_campaign_contract::classify::{self, *};
use rustred_campaign_contract::instantiate::{AnswerUse, answer_uses, instantiate_answer_use};
use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::node_basis::{self, NodeBasisArgs};
use rustred_campaign_contract::outcome::{
    Effective, EndKind, ExtraOutput, JobOutcome, LICENCE_PRESENT, Outcome, SolveRecipeNodeOptions,
    StopInfo, TimeDecision, TimeDecisionWhat,
};
use rustred_campaign_contract::seal::RecordWriter;
use rustred_campaign_contract::spec::*;
use rustred_campaign_contract::stop::StopCode;
use serde_json::{Value, json};

const P1: u64 = PROBE_DEFAULT_PRIMES[0];
const P2: u64 = PROBE_DEFAULT_PRIMES[1];

// ------------------------------------------------------------------------------------------------ primality

fn trial_division(n: u64) -> bool {
    if n < 2 {
        return false;
    }
    let mut d = 2u64;
    while d * d <= n {
        if n.is_multiple_of(d) {
            return false;
        }
        d += 1;
    }
    true
}

#[test]
fn primality_agrees_with_trial_division_on_small_numbers() {
    for n in 0..200_000u64 {
        assert_eq!(is_prime_u64(n), trial_division(n), "n = {n}");
    }
    // a deterministic pseudo-random sample up to 2^40 (trial division up to 2^20 per number)
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    for _ in 0..3_000 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let n = x % (1 << 40);
        assert_eq!(is_prime_u64(n), trial_division(n), "n = {n}");
    }
}

#[test]
fn primality_on_known_primes_composites_carmichael_and_strong_pseudoprimes() {
    assert_eq!(MR_BASES, [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37]);
    // the engine's default probe primes: 2^61 - 1 and 2^61 - 31 (coord-pole-engine.md 8)
    assert_eq!(P1, (1u64 << 61) - 1);
    assert_eq!(P2, (1u64 << 61) - 31);
    assert!(is_prime_u64(P1) && is_prime_u64(P2));
    for p in [
        2u64,
        3,
        37,
        41,
        2_147_483_647,              // 2^31 - 1
        4_294_967_291,              // largest prime < 2^32
        1_099_511_627_791,          // smallest prime >= 2^40 (2^40 + 15)
        4_611_686_018_427_387_847,  // largest prime < 2^62 (2^62 - 57)
        18_446_744_073_709_551_557, // largest prime < 2^64 (2^64 - 59)
    ] {
        assert!(is_prime_u64(p), "{p} is prime");
    }
    // Carmichael numbers (Fermat pseudoprimes to every coprime base)
    for c in [
        561u64,
        1105,
        1729,
        2465,
        2821,
        6601,
        8911,
        10585,
        15841,
        29341,
        41041,
        62745,
        63973,
        75361,
        101101,
        825_265,
        321_197_185,
        9_746_347_772_161,
    ] {
        assert!(!is_prime_u64(c), "Carmichael {c}");
    }
    // the smallest strong pseudoprimes to the first k prime bases (psi_1 .. psi_11; OEIS A014233): each passes the
    // first k bases, so a test with fewer bases than ours would call it prime
    for c in [
        2047u64,
        1_373_653,
        25_326_001,
        3_215_031_751,
        2_152_302_898_747,
        3_474_749_660_383,
        341_550_071_728_321,
        3_825_123_056_546_413_051,
    ] {
        assert!(!is_prime_u64(c), "strong pseudoprime {c}");
    }
    // other composites: squares and semiprimes of large primes, 2^62 - 1, even numbers, 0, 1
    let q = 4_294_967_291u64;
    for c in [
        0u64,
        1,
        4,
        q * q,
        2_147_483_647 * 2_147_483_629,
        (1 << 62) - 1,
        u64::MAX,
        1 << 63,
    ] {
        assert!(!is_prime_u64(c), "composite {c}");
    }
}

#[test]
fn line7_prime_rule() {
    assert!(arith::check_line7_prime(P1).is_ok());
    assert!(arith::check_line7_prime(1_099_511_627_791).is_ok());
    let e = arith::check_line7_prime(P1 - 2).unwrap_err();
    assert!(e.contains("not prime"), "{e}");
    assert!(
        arith::check_line7_prime(2_147_483_647)
            .unwrap_err()
            .contains("outside")
    );
    assert!(
        arith::check_line7_prime(18_446_744_073_709_551_557)
            .unwrap_err()
            .contains("outside")
    );
    assert!(arith::check_line7_prime(1 << 40).is_err());
    assert_eq!(
        arith::parse_prime_list(&format!("{P1},{P2}")).unwrap(),
        vec![P1, P2]
    );
    assert!(arith::parse_prime_list(&format!("{P1},{P1}")).is_err());
    assert!(arith::parse_prime_list(&format!("{P1},{}", P2 + 2)).is_err());
}

#[test]
fn rational_order_and_serde() {
    let r = |s: &str| Rational::parse_canonical(s).unwrap();
    assert!(r("-1") < r("-1/2") && r("-1/2") < r("0") && r("1/3") < r("1/2") && r("4") > r("7/2"));
    assert_eq!(serde_json::to_value(r("-3/2")).unwrap(), json!("-3/2"));
    assert!(serde_json::from_value::<Rational>(json!("8/2")).is_err());
    assert!(serde_json::from_value::<Rational>(json!(4)).is_err());
}

// ------------------------------------------------------------------------------------------------ specs

fn inputs(roles: &[&str]) -> Vec<InputRef> {
    roles
        .iter()
        .map(|r| InputRef {
            role: (*r).into(),
            path: format!("/abs/{r}"),
            blake3: D1.into(),
            upto: None,
        })
        .collect()
}

fn sr_spec(extra: &[&str]) -> JobSpec {
    let mut s = sample_spec("/r");
    s.kind = JobKind::SolveRecipe;
    s.program.features_required = vec![];
    s.budgets = Budgets::default();
    s.args = ["--order", "id"]
        .iter()
        .chain(extra)
        .map(|a| a.to_string())
        .collect();
    s.inputs = inputs(&["family", "recipe", "keys"]);
    s
}

fn nb_spec(extra: &[&str]) -> JobSpec {
    let mut s = sample_spec("/r");
    s.kind = JobKind::NodeBasis;
    s.program.features_required = vec![];
    s.budgets = Budgets::default();
    s.args = extra.iter().map(|a| a.to_string()).collect();
    s.inputs = inputs(&["family", "terminals"]);
    s
}

fn pi_spec(extra: &[&str]) -> JobSpec {
    let mut s = sample_spec("/r");
    s.kind = JobKind::ProofInstantiate;
    s.program.name = "rr_proof".into();
    s.program.features_required = vec![];
    s.budgets = Budgets::default();
    s.args = extra.iter().map(|a| a.to_string()).collect();
    s.inputs = inputs(&["requests", "table"]);
    s
}

fn usage(s: &JobSpec) -> String {
    let e = s.validate().unwrap_err();
    assert_eq!(e.exit_code(), ExitCode::Usage, "{e}");
    e.to_string()
}

#[test]
fn solve_recipe_line7_arguments() {
    let pp = format!("{P1},{P2}");
    let s = sr_spec(&[
        "--probe-d",
        "4",
        "--probe-primes",
        &pp,
        "--max-m1-rows",
        "100000",
    ]);
    s.validate().unwrap();
    let p = s.probe_mode().unwrap().unwrap();
    assert_eq!(
        (p.d0.to_string(), p.primes),
        ("4".to_string(), vec![P1, P2])
    );
    assert_eq!(s.max_m1_rows(), Some(100_000));
    // default primes when --probe-primes is absent; a solve job has no probe mode
    let s = sr_spec(&["--probe-d", "-7/2"]);
    s.validate().unwrap();
    assert_eq!(
        s.probe_mode().unwrap().unwrap().primes,
        PROBE_DEFAULT_PRIMES.to_vec()
    );
    assert_eq!(sr_spec(&[]).probe_mode().unwrap(), None);
    // shapes: non-canonical rationals, floats, exponents; non-primes, out of range, repeats; all exit 40
    for bad in ["8/2", "4/1", "1.5", "1e3", "+4", "04", "-0", "1/0", "x", ""] {
        let m = usage(&sr_spec(&["--probe-d", bad]));
        assert!(m.contains("--probe-d"), "{bad}: {m}");
    }
    let composite = format!("{P1},{}", P1 - 2);
    let m = usage(&sr_spec(&["--probe-d", "4", "--probe-primes", &composite]));
    assert!(m.contains("not prime"), "{m}");
    for bad in [
        "2147483647".to_string(),
        format!("{P1},{P1}"),
        format!("{P1},"),
        format!("0{P1}"),
        "18446744073709551557".into(),
        "18446744073709551616".into(),
    ] {
        usage(&sr_spec(&["--probe-d", "4", "--probe-primes", &bad]));
    }
    // --probe-primes needs --probe-d; --max-m1-rows 0 is not a limit; values twice; = form
    assert!(usage(&sr_spec(&["--probe-primes", &pp])).contains("needs --probe-d"));
    usage(&sr_spec(&["--max-m1-rows", "0"]));
    usage(&sr_spec(&["--max-m1-rows", "00"]));
    usage(&sr_spec(&["--max-m1-rows", "-1"]));
    usage(&sr_spec(&["--probe-d", "4", "--probe-d", "5"]));
    usage(&sr_spec(&["--probe-d=4"]));
    // the new flags belong to solve-recipe only
    let mut g = sample_spec("/r");
    g.args.extend(["--probe-d".into(), "4".into()]);
    usage(&g);
}

#[test]
fn node_basis_kind_rules() {
    let k = JobKind::NodeBasis;
    assert!(JobKind::ALL.contains(&k));
    assert_eq!(serde_json::to_string(&k).unwrap(), "\"node-basis\"");
    assert_eq!((k.level(), k.program()), (Level::L1, "rr_node"));
    assert!(!k.produces_table() && !k.is_checker() && !k.legacy_wrapped() && !k.fake_emulated());
    assert_eq!(k.sealed_outputs().unwrap()[0].role, "results");
    let pp = format!("{P1},{P2}");
    let s = nb_spec(&[
        "--mask",
        "1111000",
        "--box",
        "2,1",
        "--dims",
        "4,37/10,-1/3",
        "--primes",
        &pp,
        "--symmetry",
        "on",
    ]);
    s.validate().unwrap();
    let a = NodeBasisArgs::of_spec(&s).unwrap();
    assert_eq!(a.dims.unwrap().len(), 3);
    assert_eq!(a.primes.unwrap(), vec![P1, P2]);
    nb_spec(&[]).validate().unwrap();
    for bad in [
        vec!["--dims", "4,8/2"],
        vec!["--dims", "4,4"],
        vec!["--dims", "4.0"],
        vec!["--primes", "3"],
        vec!["--primes", "2"],
        vec!["--symmetry", "yes"],
        vec!["--symmetry"],
        vec!["--target", "I(1)"],
        vec!["--mask", "1x"],
    ] {
        usage(&nb_spec(&bad));
    }
    // roles: family and terminals, exactly once
    let mut y = nb_spec(&[]);
    y.inputs.pop();
    usage(&y);
    let mut y = nb_spec(&[]);
    y.inputs[1].role = "keys".into();
    usage(&y);
    // L1 constraints
    let mut y = nb_spec(&[]);
    y.budgets.wall.case_s = Some(5);
    usage(&y);
    let mut y = nb_spec(&[]);
    y.resume = Some(rustred_campaign_contract::resume::ResumeRef {
        from: "/x/RESUME.json".into(),
        blake3: D0.into(),
    });
    usage(&y);
}

#[test]
fn proof_instantiate_elided_loci_argument() {
    let s = pi_spec(&[]);
    s.validate().unwrap();
    assert_eq!(s.elided_loci(), ElidedLoci::Refuse);
    let s = pi_spec(&["--elided-loci", "facts"]);
    s.validate().unwrap();
    assert_eq!(s.elided_loci(), ElidedLoci::Facts);
    assert_eq!(
        pi_spec(&["--elided-loci", "refuse"]).elided_loci(),
        ElidedLoci::Refuse
    );
    usage(&pi_spec(&["--elided-loci", "allow"]));
    usage(&pi_spec(&["--elided-loci"]));
    usage(&pi_spec(&[
        "--elided-loci",
        "facts",
        "--elided-loci",
        "facts",
    ]));
    // the other legacy kinds take no argument
    for k in [JobKind::ProofInstanceSpan, JobKind::CoefficientFacts] {
        assert!(k.allowed_args().is_empty());
    }
}

// ------------------------------------------------------------------------------------------------ effective digest

fn effective(node_options: Value) -> Effective {
    Effective {
        subcommand: "solve-recipe".into(),
        args: vec!["--order".into(), "id".into()],
        node_options,
        order: json!({"name": "id"}),
        env: [("SYMBOLICA_LICENSE".to_string(), LICENCE_PRESENT.to_string())]
            .into_iter()
            .collect(),
        blake3: String::new(),
    }
}

type SetOption = fn(&mut SolveRecipeNodeOptions);
/// A named change to a record.
type Mutation = Box<dyn Fn(&mut Value)>;

fn base_options() -> SolveRecipeNodeOptions {
    SolveRecipeNodeOptions {
        recipe_digest: "abc".into(),
        determinism_id: "d".into(),
        relation_rule: Some("m1_first".into()),
        threads: 1,
        max_trace_rows: None,
        key_budget_s: None,
        proof: true,
        probe_d: None,
        probe_primes: None,
        max_m1_rows: None,
        discovery_d: None,
    }
}

#[test]
fn effective_digest_unchanged_without_line7_options() {
    // the pinned vector of tests/effective_digest.rs (EFFECTIVE_DIGEST_VERSION 2, before line 7)
    let pinned = "04d5bb45173ff273bc02b20999b0c851f668226c22b5abafc519a2c27bb05012";
    let o = base_options();
    assert_eq!(effective(o.to_value()).digest().unwrap(), pinned);
    // every line-7 key is present only when set, and then changes the digest
    let set: [(&str, SetOption); 4] = [
        ("probe_d", |o| o.probe_d = Some(Rational::integer(4))),
        ("probe_primes", |o| o.probe_primes = Some(vec![P1, P2])),
        ("max_m1_rows", |o| o.max_m1_rows = Some(1000)),
        ("discovery_d", |o| {
            o.discovery_d = Some(Rational::new(7, 2).unwrap())
        }),
    ];
    for (k, f) in set {
        assert!(o.to_value().get(k).is_none());
        let mut x = base_options();
        f(&mut x);
        let v = x.to_value();
        assert!(v.get(k).is_some(), "{k}");
        assert_ne!(effective(v).digest().unwrap(), pinned, "{k}");
    }
    let mut x = base_options();
    x.probe_d = Some(Rational::new(-3, 2).unwrap());
    x.probe_primes = Some(vec![P1, P2]);
    let v = x.to_value();
    assert_eq!(v["probe_d"], json!("-3/2"));
    assert_eq!(v["probe_primes"], json!([P1, P2]));
    assert_eq!(
        serde_json::from_value::<SolveRecipeNodeOptions>(v).unwrap(),
        x
    );
}

// ------------------------------------------------------------------------------------------------ relation records

const DET: &str = "det-1";

fn solved(i: u64, key: &[i64]) -> Value {
    let mut lower = key.to_vec();
    lower[0] -= 1;
    let t = format!(
        "I({})",
        lower
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );
    json!({"type":"relation","ordinal":i,"key":key,"tag":null,"verdict":"SOLVED","determinism_id":DET,
        "recipe_digest":D0,"rhs":[[t,"1","top"]],"relation_digest":D1,"all_lower":true,
        "proof":{"kind":"exact-weights","ok":true,"weights":3},"failure":null,"seconds":1.5})
}

fn unsolved(i: u64, key: &[i64], verdict: &str, detail: Value) -> Value {
    json!({"type":"relation","ordinal":i,"key":key,"tag":null,"verdict":verdict,"determinism_id":DET,
        "recipe_digest":D0,"rhs":null,"relation_digest":null,
        "failure":{"schema":"rustred.recipe-failure.v1","class":verdict.to_lowercase(),"message":"m","detail":detail}})
}

fn end_rec(ty: &str, recs: &[Value], det: &str, digest_field: Option<&str>) -> Value {
    let mut counts = serde_json::Map::new();
    for r in recs {
        let v = r["verdict"].as_str().unwrap().to_string();
        let n = counts.get(&v).and_then(Value::as_u64).unwrap_or(0) + 1;
        counts.insert(v, json!(n));
    }
    let mut e =
        json!({"type":ty,"records":recs.len(),"counts":counts,"determinism_id":det,"seconds":3.0});
    if let Some(d) = digest_field {
        e["recipe_digest"] = json!(d);
    }
    e
}

fn lines(recs: &[Value], end: Value) -> Vec<Vec<u8>> {
    recs.iter()
        .chain(std::iter::once(&end))
        .map(|v| canonical_line(v.to_string().as_bytes()).unwrap())
        .collect()
}

fn keys(n: usize) -> Vec<KeyLine> {
    (0..n)
        .map(|i| KeyLine {
            key: vec![i as i64 + 2, 1, 0],
            tag: None,
        })
        .collect()
}

fn key(i: usize) -> Vec<i64> {
    vec![i as i64 + 2, 1, 0]
}

fn rel_file(recs: &[Value]) -> Vec<Vec<u8>> {
    lines(recs, end_rec("end", recs, DET, Some(D0)))
}

#[test]
fn verdicts_and_classes_of_line7() {
    assert_eq!(Verdict::NotGenericAtD0.name(), "NOT_GENERIC_AT_D0");
    assert_eq!(Verdict::D0SingularRows.name(), "D0_SINGULAR_ROWS");
    assert_eq!(FailureClass::NotGenericAtD0.name(), "not_generic_at_d0");
    assert_eq!(FailureClass::D0SingularRows.name(), "d0_singular_rows");
    for c in [FailureClass::NotGenericAtD0, FailureClass::D0SingularRows] {
        // routed like no_relation_in_seeds: the next escalation step, never a resource failure
        assert_eq!(c.route(), FailureClass::NoRelationInSeeds.route());
        assert!(!c.time_decided());
    }
    let names: Vec<String> = ProbeVerdict::ALL.iter().map(|v| v.name()).collect();
    assert_eq!(
        names,
        [
            "EXISTS_AT_D0",
            "EXISTS_ONLY_AT_D0",
            "NO_RELATION_AT_D0",
            "D0_SINGULAR_ROWS",
            "UNRESOLVED_PRIMES",
            "BUDGET_WALL",
            "ERROR",
            "INTERRUPTED"
        ]
    );
    assert_eq!(
        ProbeVerdict::UnresolvedPrimes.route(),
        Some(KeyRoute::OtherPrimesThenAttention)
    );
    assert!(
        ProbeVerdict::ALL[..4]
            .iter()
            .all(|v| v.is_answer() && v.route().is_none())
    );
    assert!(
        ProbeVerdict::ALL[4..]
            .iter()
            .all(|v| !v.is_answer() && v.route().is_some())
    );
}

#[test]
fn relation_records_with_line7_members() {
    let mut a = solved(0, &key(0));
    a["discovery"] = json!({"rank": 10, "rows": 12, "columns": 30});
    a["hidden_d_poles"] = json!(["d-4"]);
    a["coef_d_poles"] = json!(["3*d-16", "d-4"]);
    a["d_poles"] = json!(["3*d-16", "d-4"]);
    a["m1_work"] = json!({"sub_solves": 2, "rows": 500, "trace_rows": 40});
    let b = unsolved(
        1,
        &key(1),
        "NO_RELATION_IN_SEEDS",
        json!({"seeds": 3, "rows": 9, "rank": 7, "columns": 20}),
    );
    let (out, t) = recipe::key_outcomes(&rel_file(&[a.clone(), b.clone()]), &keys(2)).unwrap();
    assert_eq!((t.accepted, t.unsolved_total(), t.probe), (1, 1, false));
    // the members are NON-volatile: part of the canonical record
    let KeyOutcome::Accepted(acc) = &out[0] else {
        panic!()
    };
    let c = String::from_utf8(acc.canonical.clone()).unwrap();
    for m in ["discovery", "hidden_d_poles", "coef_d_poles", "m1_work"] {
        assert!(c.contains(m), "{m} dropped from the canonical form");
    }
    assert!(!c.contains("seconds"));
    // refusals
    let refuse = |r: Value, what: &str| {
        let e = recipe::key_outcomes(&rel_file(&[r, b.clone()]), &keys(2));
        assert!(e.is_err(), "{what} accepted");
    };
    let mut x = a.clone();
    x["d_poles"] = json!(["d-4"]);
    refuse(x, "d_poles not the union");
    let mut x = a.clone();
    x.as_object_mut().unwrap().remove("coef_d_poles");
    refuse(x, "hidden without coef");
    let mut x = a.clone();
    x["discovery"]["rank"] = json!(13);
    refuse(x, "rank > rows");
    let mut x = a.clone();
    x["discovery"]["columns"] = json!("30");
    refuse(x, "columns a string");
    let mut x = a.clone();
    x["m1_work"]["rows"] = json!(-1);
    refuse(x, "negative m1 rows");
    let mut x = a.clone();
    x["type"] = json!("probe");
    refuse(x, "a probe record in a solve job");
    let mut y = b.clone();
    y["failure"]["detail"]["rank"] = json!(1.5);
    let e = recipe::key_outcomes(&rel_file(&[a.clone(), y]), &keys(2));
    assert!(e.is_err(), "failure.detail.rank a float");
}

#[test]
fn m1_work_cap_and_budget_work_counter() {
    let mut a = solved(0, &key(0));
    a["m1_work"] = json!({"sub_solves": 2, "rows": 500, "trace_rows": 40});
    let bw = unsolved(
        1,
        &key(1),
        "BUDGET_WORK",
        json!({"counter": "m1_work_rows", "limit": 400, "value": 401}),
    );
    let mut bw_m = bw.clone();
    bw_m["m1_work"] = json!({"sub_solves": 1, "rows": 401, "trace_rows": 3});
    // with the cap 500: the SOLVED record (500 rows, not above) and the BUDGET_WORK key; the key goes to the next
    // work class, never "no relation"
    let f = rel_file(&[a.clone(), bw_m.clone()]);
    assert!(
        recipe::key_outcomes_with(&f, &keys(2), Some(500)).is_err(),
        "limit 400 != cap 500"
    );
    let mut ok = bw_m.clone();
    ok["failure"]["detail"] = json!({"counter": "m1_work_rows", "limit": 500, "value": 501});
    ok["m1_work"]["rows"] = json!(501);
    let (out, t) =
        recipe::key_outcomes_with(&rel_file(&[a.clone(), ok.clone()]), &keys(2), Some(500))
            .unwrap();
    assert_eq!(t.unsolved.get(&FailureClass::BudgetWork), Some(&1));
    let KeyOutcome::Unsolved(u) = &out[1] else {
        panic!()
    };
    assert_eq!(u.route, KeyRoute::NextWorkClass);
    // the same file without the cap: the m1_work_rows counter is refused
    assert!(recipe::key_outcomes(&rel_file(&[a.clone(), ok.clone()]), &keys(2)).is_err());
    // a SOLVED key above the cap is refused (the key must end BUDGET_WORK, never a fallback)
    let mut over = a.clone();
    over["m1_work"]["rows"] = json!(501);
    over["relation_rule_taken"] = json!("c_fallback");
    assert!(
        recipe::key_outcomes_with(&rel_file(&[over, ok.clone()]), &keys(2), Some(500)).is_err()
    );
    // value not above the limit; counter m1_work_rows in a NO_RELATION record; unknown counter
    let mut x = ok.clone();
    x["failure"]["detail"]["value"] = json!(500);
    assert!(recipe::key_outcomes_with(&rel_file(&[a.clone(), x]), &keys(2), Some(500)).is_err());
    let nr = unsolved(
        1,
        &key(1),
        "NO_RELATION_IN_SEEDS",
        json!({"counter": "m1_work_rows"}),
    );
    assert!(recipe::key_outcomes_with(&rel_file(&[a.clone(), nr]), &keys(2), Some(500)).is_err());
    let x = unsolved(
        1,
        &key(1),
        "BUDGET_WORK",
        json!({"counter": "lift_rows", "limit": 1, "value": 2}),
    );
    assert!(recipe::key_outcomes(&rel_file(&[a.clone(), x]), &keys(2)).is_err());
    // the legacy trace_rows counter and an empty detail stay valid
    for d in [
        json!({"counter": "trace_rows", "limit": 10, "value": 11}),
        json!({}),
    ] {
        let x = unsolved(1, &key(1), "BUDGET_WORK", d);
        recipe::key_outcomes(&rel_file(&[a.clone(), x]), &keys(2)).unwrap();
    }
}

#[test]
fn discovery_d_and_the_d0_verdicts() {
    let mut a = solved(0, &key(0));
    a["discovery_d"] = json!("4");
    let mut b = unsolved(
        1,
        &key(1),
        "NOT_GENERIC_AT_D0",
        json!({"rank": 3, "columns": 9}),
    );
    b["discovery_d"] = json!("4");
    let mut c = unsolved(2, &key(2), "D0_SINGULAR_ROWS", json!({}));
    c["discovery_d"] = json!("4");
    let (out, t) =
        recipe::key_outcomes(&rel_file(&[a.clone(), b.clone(), c.clone()]), &keys(3)).unwrap();
    assert_eq!(t.unsolved.get(&FailureClass::NotGenericAtD0), Some(&1));
    assert_eq!(t.unsolved.get(&FailureClass::D0SingularRows), Some(&1));
    for o in &out[1..] {
        let KeyOutcome::Unsolved(u) = o else { panic!() };
        assert_eq!(u.route, KeyRoute::NextRecipeRung);
    }
    // the d0 verdicts need discovery_d; discovery_d is one value per file and canonical
    let mut x = b.clone();
    x.as_object_mut().unwrap().remove("discovery_d");
    assert!(recipe::key_outcomes(&rel_file(&[solved(0, &key(0)), x]), &keys(2)).is_err());
    let mut x = b.clone();
    x["discovery_d"] = json!("5");
    assert!(recipe::key_outcomes(&rel_file(&[a.clone(), x]), &keys(2)).is_err());
    let mut x = a.clone();
    x["discovery_d"] = json!("8/2");
    assert!(recipe::key_outcomes(&rel_file(&[x]), &keys(1)).is_err());
    let mut x = a.clone();
    x["discovery_d"] = json!(4);
    assert!(recipe::key_outcomes(&rel_file(&[x]), &keys(1)).is_err());
    assert!(recipe::key_outcomes(&rel_file(&[a.clone(), solved(1, &key(1))]), &keys(2)).is_err());
}

// ------------------------------------------------------------------------------------------------ probe records

const PDET: &str =
    "rustred.recipe-probe.v1/elim=rustred.recipe-elim.v1/seed=0/primes=recorded/d0=recorded";

fn per(p: u64, v: &str) -> Value {
    json!({"prime": p, "verdict": v, "rank": 5, "rows": 9, "columns": 20, "trace_rows": 4})
}

fn probe(i: usize, verdict: &str, per_prime: Vec<Value>) -> Value {
    json!({"type":"probe","ordinal":i,"key":key(i),"recipe":{"id":"R/1","digest":D0},"d0":"4","verdict":verdict,
        "per_prime":per_prime,"top_support":[],"seconds":0.7,"vmhwm_kb":10,"determinism_id":PDET})
}

fn answered(i: usize, v: &str) -> Value {
    probe(i, v, vec![per(P1, v), per(P2, v)])
}

fn probe_file(recs: &[Value], ty: &str) -> Vec<Vec<u8>> {
    lines(recs, end_rec(ty, recs, PDET, None))
}

fn pargs() -> ProbeArgs {
    sr_spec(&["--probe-d", "4"]).probe_mode().unwrap().unwrap()
}

#[test]
fn probe_records_are_never_relations() {
    let recs = vec![
        answered(0, "EXISTS_AT_D0"),
        answered(1, "EXISTS_ONLY_AT_D0"),
        answered(2, "NO_RELATION_AT_D0"),
        answered(3, "D0_SINGULAR_ROWS"),
        probe(
            4,
            "UNRESOLVED_PRIMES",
            vec![per(P1, "EXISTS_AT_D0"), per(P2, "NO_RELATION_AT_D0")],
        ),
        probe(5, "BUDGET_WALL", vec![per(P1, "EXISTS_AT_D0")]),
        probe(6, "ERROR", vec![]),
    ];
    let (out, t) = recipe::probe_outcomes(&probe_file(&recs, "end"), &keys(7), &pargs()).unwrap();
    assert!(t.probe);
    assert_eq!((t.records, t.accepted, t.unsolved_total()), (7, 0, 0));
    assert_eq!(t.unanswered_probes(), 3);
    assert_eq!(t.time_decided_keys(), 1);
    assert!(out[..4].iter().all(|o| o.route.is_none()));
    assert_eq!(out[4].route, Some(KeyRoute::OtherPrimesThenAttention));
    assert_eq!(out[5].route, Some(KeyRoute::LadderDecides));
    assert_eq!(out[6].route, Some(KeyRoute::Attention));
    // the same file read as a SOLVE job's results: refused (a probe record in a solve job)
    assert!(recipe::key_outcomes(&probe_file(&recs, "end"), &keys(7)).is_err());
    // a relation record in a probe job: refused
    let mut mixed = recs[..2].to_vec();
    mixed.push(solved(2, &key(2)));
    assert!(recipe::probe_outcomes(&probe_file(&mixed, "end"), &keys(3), &pargs()).is_err());
    // INTERRUPTED: only in a partial file
    let intr = vec![answered(0, "EXISTS_AT_D0"), probe(1, "INTERRUPTED", vec![])];
    assert!(recipe::probe_outcomes(&probe_file(&intr, "end"), &keys(2), &pargs()).is_err());
    recipe::probe_outcomes(&probe_file(&intr, "partial"), &keys(2), &pargs()).unwrap();
}

#[test]
fn probe_record_refusals() {
    let ok = answered(0, "EXISTS_AT_D0");
    let check = |r: Value| recipe::probe_outcomes(&probe_file(&[r], "end"), &keys(1), &pargs());
    check(ok.clone()).unwrap();
    let cases: Vec<(&str, Mutation)> = vec![
        ("d0 of another value", Box::new(|r| r["d0"] = json!("5"))),
        ("d0 not canonical", Box::new(|r| r["d0"] = json!("8/2"))),
        (
            "verdict SOLVED",
            Box::new(|r| r["verdict"] = json!("SOLVED")),
        ),
        (
            "relation digest",
            Box::new(|r| r["relation_digest"] = json!(D1)),
        ),
        ("rhs", Box::new(|r| r["rhs"] = json!([]))),
        (
            "recipe digest",
            Box::new(|r| r["recipe"]["digest"] = json!("abc")),
        ),
        (
            "determinism id",
            Box::new(|r| r["determinism_id"] = json!(DET)),
        ),
        (
            "primes in another order",
            Box::new(|r| {
                r["per_prime"] = json!([per(P2, "EXISTS_AT_D0"), per(P1, "EXISTS_AT_D0")])
            }),
        ),
        (
            "one prime missing",
            Box::new(|r| r["per_prime"] = json!([per(P1, "EXISTS_AT_D0")])),
        ),
        (
            "a per-prime non-answer",
            Box::new(|r| r["per_prime"][1]["verdict"] = json!("ERROR")),
        ),
        (
            "per-prime disagreement",
            Box::new(|r| r["per_prime"][1]["verdict"] = json!("NO_RELATION_AT_D0")),
        ),
        (
            "per-prime rank a float",
            Box::new(|r| r["per_prime"][0]["rank"] = json!(1.5)),
        ),
        ("ordinal", Box::new(|r| r["ordinal"] = json!(1))),
        ("key", Box::new(|r| r["key"] = json!([9, 9, 9]))),
    ];
    for (what, f) in cases {
        let mut r = ok.clone();
        f(&mut r);
        assert!(check(r).is_err(), "{what} accepted");
    }
    // UNRESOLVED_PRIMES with agreeing primes is refused
    let r = probe(
        0,
        "UNRESOLVED_PRIMES",
        vec![per(P1, "EXISTS_AT_D0"), per(P2, "EXISTS_AT_D0")],
    );
    assert!(check(r).is_err());
    // end record counts
    let recs = vec![ok.clone()];
    let mut e = end_rec("end", &recs, PDET, None);
    e["counts"] = json!({"EXISTS_ONLY_AT_D0": 1});
    assert!(recipe::probe_outcomes(&lines(&recs, e), &keys(1), &pargs()).is_err());
    let mut e = end_rec("end", &recs, "rustred.recipe-probe.v1/other", None);
    assert!(recipe::probe_outcomes(&lines(&recs, e.clone()), &keys(1), &pargs()).is_err());
    e["determinism_id"] = json!(PDET);
    e["recipe_digest"] = json!(D1);
    assert!(recipe::probe_outcomes(&lines(&recs, e), &keys(1), &pargs()).is_err());
}

#[test]
fn canonical_form_v5() {
    assert_eq!(VOLATILE_KEYS_VERSION, 5);
    let a = answered(0, "EXISTS_AT_D0");
    let mut b = a.clone();
    b["seconds"] = json!(99.0);
    b["vmhwm_kb"] = json!(1);
    let ca = canonical_line(a.to_string().as_bytes()).unwrap();
    assert_eq!(ca, canonical_line(b.to_string().as_bytes()).unwrap());
    assert!(!String::from_utf8(ca).unwrap().contains("seconds"));
    // node-basis records have no volatile key
    let s = json!({"type":"sample","d":"4","seconds":1.0});
    assert!(
        String::from_utf8(canonical_line(s.to_string().as_bytes()).unwrap())
            .unwrap()
            .contains("seconds")
    );
}

// ------------------------------------------------------------------------------------------------ node-basis records

fn sample(d: &str, p: u64) -> Value {
    json!({"type":"sample","d":d,"prime":p,"rank_system":10,"rows":12,"columns":15,"free_columns":["I(1)"],
        "rank_T":2,"pivots_in_terminals":[{"pivot":"I(2)","row":[["I(1)","5"]]}]})
}

fn dim(d: &str) -> Value {
    json!({"type":"dim","d":d,"rank_T":2,"rank_system":10,"free_columns":["I(1)"],
        "null_directions":[[["I(1)","6"],["I(2)","-3"]]],"status":"ok"})
}

fn nb_file(recs: &[Value], verdict: &str) -> Vec<Vec<u8>> {
    lines(
        recs,
        json!({"type":"end","deficiency":{"4":1},"verdict":verdict}),
    )
}

#[test]
fn node_basis_records() {
    let pp = format!("{P1},{P2}");
    let s = nb_spec(&["--dims", "4,7/2", "--primes", &pp]);
    let a = NodeBasisArgs::of_spec(&s).unwrap();
    let full = vec![
        sample("4", P1),
        sample("4", P2),
        sample("7/2", P1),
        sample("7/2", P2),
        dim("4"),
        dim("7/2"),
    ];
    let sum = node_basis::check_results(&nb_file(&full, "degenerate"), &a).unwrap();
    assert_eq!(
        (sum.samples, sum.dims, sum.verdict.as_str()),
        (4, 2, "degenerate")
    );
    // without --dims / --primes: any canonical d, any line-7 prime, no completeness rule
    node_basis::check_results(&nb_file(&full[..1], "regular"), &NodeBasisArgs::default()).unwrap();
    let bad: Vec<(&str, Vec<Value>, &str)> = vec![
        ("incomplete", full[..5].to_vec(), "regular"),
        ("verdict", full.clone(), "maybe"),
        (
            "a d not in --dims",
            [full.clone(), vec![dim("5")]].concat(),
            "regular",
        ),
        (
            "duplicate sample",
            [full.clone(), vec![sample("4", P1)]].concat(),
            "regular",
        ),
        (
            "a relation record",
            [full.clone(), vec![solved(0, &key(0))]].concat(),
            "regular",
        ),
    ];
    for (what, recs, v) in bad {
        assert!(
            node_basis::check_results(&nb_file(&recs, v), &a).is_err(),
            "{what} accepted"
        );
    }
    let mut x = full.clone();
    x[0]["prime"] = json!(P1 - 2);
    assert!(node_basis::check_results(&nb_file(&x, "regular"), &NodeBasisArgs::default()).is_err());
    let mut x = full.clone();
    x[0]["d"] = json!(4);
    assert!(node_basis::check_results(&nb_file(&x, "regular"), &a).is_err());
    let mut x = full.clone();
    x[0]["rank_system"] = json!(13);
    assert!(node_basis::check_results(&nb_file(&x, "regular"), &a).is_err());
    let mut x = full.clone();
    x[4]["status"] = json!("done");
    assert!(node_basis::check_results(&nb_file(&x, "regular"), &a).is_err());
}

// ------------------------------------------------------------------------------------------------ proof-instantiate

fn inst(rule: i64, k: &[i64], regular: &str) -> Value {
    json!({"schema":"rustred.proof-kernel.instantiate.v1","record_sha256":"aa","rule":rule,"key":k,"in_case":true,
        "chart_ok":true,"branch":null,"regular":regular,"irregular_terms":[],"hidden_loci":[],
        "rhs":[{"term":0,"key":k,"coef":"nonzero"}],"zero_test":"exact","error":null})
}

fn elided_inst(rule: i64, k: &[i64], regular: &str) -> Value {
    let mut v = inst(rule, k, regular);
    v["witness_loci_elided"] = json!(2);
    v["elided_cover"] = json!("guards_v2");
    v
}

fn inst_file(recs: &[Value]) -> Vec<Vec<u8>> {
    lines(
        recs,
        json!({"type":"end","records":recs.len(),"answers":recs.len()}),
    )
}

#[test]
fn elided_answers_are_coefficient_facts_only() {
    use ElidedLoci::*;
    let full = inst(0, &[1, 2], "regular");
    let el = elided_inst(1, &[1, 2], "unknown");
    assert_eq!(
        instantiate_answer_use(&full, Refuse).unwrap(),
        AnswerUse::RegularityAndFacts
    );
    assert_eq!(
        instantiate_answer_use(&full, Facts).unwrap(),
        AnswerUse::RegularityAndFacts
    );
    assert_eq!(
        instantiate_answer_use(&el, Facts).unwrap(),
        AnswerUse::CoefficientFactsOnly
    );
    // under refuse an elided answer is a violation (R-f); under facts it never claims regular
    assert!(instantiate_answer_use(&el, Refuse).is_err());
    assert!(instantiate_answer_use(&elided_inst(1, &[1, 2], "regular"), Facts).is_err());
    let mut t = el.clone();
    t["regular"] = json!(true);
    assert!(instantiate_answer_use(&t, Facts).is_err());
    let mut t = el.clone();
    t["elided_cover"] = json!("trust_me");
    assert!(instantiate_answer_use(&t, Facts).is_err());
    let mut t = el.clone();
    t.as_object_mut().unwrap().remove("elided_cover");
    assert!(
        instantiate_answer_use(&t, Facts).is_err(),
        "witness_loci_elided without elided_cover"
    );
    // decided non-regular answers and errors
    assert_eq!(
        instantiate_answer_use(&inst(0, &[1], "hidden"), Refuse).unwrap(),
        AnswerUse::CoefficientFactsOnly
    );
    let mut e = inst(0, &[1], "unknown");
    e["error"] = json!("EMPTY_EXCEPTION_BRANCH");
    assert_eq!(
        instantiate_answer_use(&e, Refuse).unwrap(),
        AnswerUse::Nothing
    );
    // a whole file; fact answers about an elided (rule, key) are demoted, a regular claim among them refused
    let fact = |label: i64, k: &[i64], kind: &str, value: &str| {
        json!({"schema":"rustred.campaign.fact.v1","table":"/t","record":0,"label":label,"key":k,"kind":kind,
            "value":value,"method":"exact","evidence":"e","error":null})
    };
    let f = inst_file(&[
        full.clone(),
        el.clone(),
        fact(1, &[1, 2], "coefficient", "zero"),
        fact(1, &[1, 2], "regularity", "unknown"),
        fact(0, &[1, 2], "regularity", "regular"),
    ]);
    assert_eq!(
        answer_uses(&f, Facts).unwrap(),
        vec![
            AnswerUse::RegularityAndFacts,
            AnswerUse::CoefficientFactsOnly,
            AnswerUse::CoefficientFactsOnly,
            AnswerUse::Nothing,
            AnswerUse::RegularityAndFacts
        ]
    );
    assert!(answer_uses(&f, Refuse).is_err());
    let f = inst_file(&[el.clone(), fact(1, &[1, 2], "regularity", "regular")]);
    assert!(answer_uses(&f, Facts).is_err());
    let f = inst_file(&[json!({"schema":"other"})]);
    assert!(answer_uses(&f, Facts).is_err());
}

// ------------------------------------------------------------------------------------------------ decision rows

fn evidence(
    kind: JobKind,
    code: ExitCode,
    stop: Option<StopCode>,
    t: Option<RelationTally>,
) -> Evidence {
    let mut s = sample_spec("/r");
    s.kind = kind;
    let mut j = job_outcome(&s, code, stop, false);
    j.kind = kind;
    j.outputs.table = None;
    Evidence {
        kind,
        wait: WaitStatus::Exited(code.code()),
        outcome: OutcomeState::Valid(Box::new(j)),
        verified: Some(VerifiedOutputs {
            rules: 0,
            elided_loci: 0,
            streams: vec![],
            relations: t,
        }),
        unsealed_why: None,
        resume_valid: false,
        resume_ref: None,
        kill: KillReason::None,
        oom_kill_observed: false,
        elision_forbidden: true,
    }
}

fn probe_tally(answered: u64, open: u64) -> RelationTally {
    let mut t = RelationTally {
        records: answered + open,
        probe: true,
        ..Default::default()
    };
    if answered > 0 {
        t.probes.insert(ProbeVerdict::ExistsAtD0, answered);
    }
    if open > 0 {
        t.probes.insert(ProbeVerdict::UnresolvedPrimes, open);
    }
    t
}

#[test]
fn decision_rows_30_31_32() {
    let rowof = |e: &Evidence| {
        let c = classify::classify(e);
        (c.row, c.accept, c.action)
    };
    let nb = evidence(JobKind::NodeBasis, ExitCode::Complete, None, None);
    assert_eq!(
        rowof(&nb),
        (30, Accept::DiagnosticRecords, Action::ItemDone)
    );
    for (code, stop) in [
        (ExitCode::Incomplete, Some(StopCode::MaxCases)),
        (ExitCode::Incomplete, Some(StopCode::KeysUnsolved)),
        (ExitCode::BudgetWork, Some(StopCode::BudgetWork)),
    ] {
        assert_eq!(
            rowof(&evidence(JobKind::NodeBasis, code, stop, None)).0,
            22,
            "{code:?}"
        );
    }
    let kk = Some(StopCode::KeysUnsolved);
    let sr = |code, stop, t| rowof(&evidence(JobKind::SolveRecipe, code, stop, Some(t)));
    assert_eq!(
        sr(ExitCode::Complete, None, probe_tally(3, 0)),
        (31, Accept::DiagnosticRecords, Action::ItemDone)
    );
    assert_eq!(
        sr(ExitCode::Incomplete, kk, probe_tally(2, 1)),
        (32, Accept::DiagnosticRecords, Action::QueueFailures)
    );
    assert_eq!(sr(ExitCode::Incomplete, kk, probe_tally(0, 1)).0, 32);
    assert_eq!(
        sr(ExitCode::Complete, None, probe_tally(2, 1)).0,
        22,
        "exit 0 with an unanswered key"
    );
    assert_eq!(
        sr(ExitCode::Incomplete, kk, probe_tally(2, 0)).0,
        22,
        "keys_unsolved, all answered"
    );
    let mut lying = probe_tally(2, 0);
    lying.accepted = 1;
    assert_eq!(
        sr(ExitCode::Complete, None, lying).0,
        22,
        "an accepted relation in a probe tally"
    );
    // refused / resource / interrupted rows apply to the new kind as to every kind
    for (code, row) in [
        (ExitCode::Usage, 12),
        (ExitCode::Input, 13),
        (ExitCode::Refused, 14),
        (ExitCode::NoSpace, 15),
    ] {
        let mut e = evidence(JobKind::NodeBasis, code, None, None);
        e.verified = None;
        assert_eq!(classify::classify(&e).row, row, "{code:?}");
    }
    let mut e = evidence(
        JobKind::NodeBasis,
        ExitCode::Interrupted,
        Some(StopCode::Interrupted),
        None,
    );
    e.verified = None;
    assert_eq!(classify::classify(&e).row, 11);
}

// ------------------------------------------------------------------------------------------------ end to end

struct Run {
    _d: TempDir,
    layout: RunLayout,
    spec: JobSpec,
    digest: String,
}

fn run_dir(tag: &str, mut spec: JobSpec, keys_jsonl: Option<&str>) -> Run {
    let d = TempDir::new(tag);
    let root = d.path().join("J.a1");
    std::fs::create_dir_all(root.join("out")).unwrap();
    std::fs::create_dir_all(root.join("in")).unwrap();
    spec.run_dir = root.to_str().unwrap().into();
    for i in spec.inputs.iter_mut() {
        let body = if i.role == "keys" {
            keys_jsonl.unwrap_or("")
        } else {
            "x\n"
        };
        let p = root.join("in").join(&i.role);
        std::fs::write(&p, body).unwrap();
        i.path = p.to_str().unwrap().into();
        i.blake3 = rustred_campaign_contract::blake3_hex(body.as_bytes());
    }
    spec.validate().unwrap();
    let bytes = spec.to_json_line();
    std::fs::write(root.join("job.json"), &bytes).unwrap();
    let digest = rustred_campaign_contract::spec::spec_blake3(&bytes);
    Run {
        _d: d,
        layout: RunLayout::new(&root),
        spec,
        digest,
    }
}

fn judge(
    r: &Run,
    recs: &[Vec<u8>],
    code: ExitCode,
    stop: Option<StopCode>,
    time: bool,
) -> Classification {
    let mut w = RecordWriter::create(&r.layout.root.join("out/results.jsonl")).unwrap();
    let (end, body) = recs.split_last().unwrap();
    for l in body {
        w.append(l).unwrap();
    }
    let s = w.seal(end).unwrap();
    let mut j = JobOutcome::for_spec(
        &r.spec,
        &r.digest,
        program_info(),
        code,
        StopInfo {
            code: stop,
            detail: Value::Null,
            source: None,
            graceful: true,
        },
    );
    j.time_decided = time;
    if time {
        j.time_decisions = vec![TimeDecision {
            what: TimeDecisionWhat::CaseWall,
            case_ordinal: Some(1),
            rule: None,
            limit_s: 5.0,
            measured_s: 5.1,
            taken: "budget_wall".into(),
        }];
    }
    j.outputs.extra.push(ExtraOutput {
        role: "results".into(),
        path: "out/results.jsonl".into(),
        bytes: s.bytes,
        blake3: s.blake3.clone(),
        records: Some(s.records),
        canonical_blake3: Some(s.canonical_blake3.clone()),
        end: Some(EndKind::End),
        sealed: true,
    });
    j.check().unwrap();
    Outcome::new(j).write_new(&r.layout.outcome()).unwrap();
    let ev = classify::assess(
        &r.layout,
        &r.spec,
        &r.digest,
        WaitStatus::Exited(code.code()),
        KillReason::None,
        false,
    );
    let c = classify::classify(&ev);
    eprintln!(
        "{tag}: row {} ({:?})",
        c.row,
        ev.unsealed_why,
        tag = r.spec.job_id
    );
    c
}

fn keys_text(n: usize) -> String {
    (0..n)
        .map(|i| format!("{:?}\n", key(i)))
        .collect::<String>()
        .replace(' ', "")
}

#[test]
fn probe_job_end_to_end() {
    let recs = vec![
        answered(0, "EXISTS_AT_D0"),
        answered(1, "NO_RELATION_AT_D0"),
    ];
    let r = run_dir(
        "probe-ok",
        sr_spec(&["--probe-d", "4"]),
        Some(&keys_text(2)),
    );
    let c = judge(
        &r,
        &probe_file(&recs, "end"),
        ExitCode::Complete,
        None,
        false,
    );
    assert_eq!((c.row, c.accept), (31, Accept::DiagnosticRecords));
    // the probe tally is reachable for the caller; nothing is accepted
    // a relation file under a probe spec: S false -> 22 (contract violation)
    let rel = vec![solved(0, &key(0)), solved(1, &key(1))];
    let r = run_dir(
        "probe-rel",
        sr_spec(&["--probe-d", "4"]),
        Some(&keys_text(2)),
    );
    let c = judge(&r, &rel_file(&rel), ExitCode::Complete, None, false);
    assert_eq!((c.row, c.accept), (22, Accept::Nothing));
    // a probe file under a solve spec: 22
    let r = run_dir("solve-probe", sr_spec(&[]), Some(&keys_text(2)));
    let c = judge(
        &r,
        &probe_file(&recs, "end"),
        ExitCode::Complete,
        None,
        false,
    );
    assert_eq!((c.row, c.accept), (22, Accept::Nothing));
    // unanswered keys: exit 10 keys_unsolved -> 32; a BUDGET_WALL probe needs time_decided
    let recs = vec![
        answered(0, "EXISTS_AT_D0"),
        probe(1, "BUDGET_WALL", vec![per(P1, "EXISTS_AT_D0")]),
    ];
    let r = run_dir(
        "probe-open",
        sr_spec(&["--probe-d", "4", "--key-budget-s", "5"]),
        Some(&keys_text(2)),
    );
    let c = judge(
        &r,
        &probe_file(&recs, "end"),
        ExitCode::Incomplete,
        Some(StopCode::KeysUnsolved),
        true,
    );
    assert_eq!(
        (c.row, c.accept, c.action),
        (32, Accept::DiagnosticRecords, Action::QueueFailures)
    );
    let r = run_dir(
        "probe-open2",
        sr_spec(&["--probe-d", "4", "--key-budget-s", "5"]),
        Some(&keys_text(2)),
    );
    let c = judge(
        &r,
        &probe_file(&recs, "end"),
        ExitCode::Incomplete,
        Some(StopCode::KeysUnsolved),
        false,
    );
    assert_eq!(c.row, 22, "BUDGET_WALL probe without time_decided");
    // a solve job with the m1 cap end to end
    let mut over = solved(0, &key(0));
    over["m1_work"] = json!({"sub_solves": 1, "rows": 11, "trace_rows": 1});
    let r = run_dir(
        "m1cap",
        sr_spec(&["--max-m1-rows", "10"]),
        Some(&keys_text(1)),
    );
    // (judge_solved: counters.rules_committed = 1, the one SOLVED record, so only the cap can refuse it)
    let j = judge_solved(&r, &rel_file(std::slice::from_ref(&over)));
    assert_eq!(
        (j.row, j.accept),
        (22, Accept::Nothing),
        "SOLVED above the m1 cap"
    );
    let r = run_dir(
        "m1cap2",
        sr_spec(&["--max-m1-rows", "11"]),
        Some(&keys_text(1)),
    );
    let j = judge_solved(&r, &rel_file(&[over]));
    assert_eq!((j.row, j.accept), (1, Accept::Canonical));
}

fn judge_solved(r: &Run, recs: &[Vec<u8>]) -> Classification {
    // as `judge`, with counters.rules_committed = 1 (one accepted relation)
    let mut w = RecordWriter::create(&r.layout.root.join("out/results.jsonl")).unwrap();
    let (end, body) = recs.split_last().unwrap();
    for l in body {
        w.append(l).unwrap();
    }
    let s = w.seal(end).unwrap();
    let mut j = JobOutcome::for_spec(
        &r.spec,
        &r.digest,
        program_info(),
        ExitCode::Complete,
        StopInfo::none(),
    );
    j.counters.rules_committed = 1;
    j.outputs.extra.push(ExtraOutput {
        role: "results".into(),
        path: "out/results.jsonl".into(),
        bytes: s.bytes,
        blake3: s.blake3.clone(),
        records: Some(s.records),
        canonical_blake3: Some(s.canonical_blake3.clone()),
        end: Some(EndKind::End),
        sealed: true,
    });
    Outcome::new(j).write_new(&r.layout.outcome()).unwrap();
    let ev = classify::assess(
        &r.layout,
        &r.spec,
        &r.digest,
        WaitStatus::Exited(0),
        KillReason::None,
        false,
    );
    classify::classify(&ev)
}

#[test]
fn node_basis_job_end_to_end() {
    let pp = format!("{P1},{P2}");
    let spec = nb_spec(&["--dims", "4", "--primes", &pp]);
    let full = vec![sample("4", P1), sample("4", P2), dim("4")];
    let r = run_dir("nb-ok", spec.clone(), None);
    let c = judge(
        &r,
        &nb_file(&full, "degenerate"),
        ExitCode::Complete,
        None,
        false,
    );
    assert_eq!((c.row, c.accept), (30, Accept::DiagnosticRecords));
    let r = run_dir("nb-short", spec.clone(), None);
    let c = judge(
        &r,
        &nb_file(&full[..2], "degenerate"),
        ExitCode::Complete,
        None,
        false,
    );
    assert_eq!((c.row, c.accept), (22, Accept::Nothing));
    let r = run_dir("nb-rule", spec, None);
    let c = judge(
        &r,
        &nb_file(
            &[full.clone(), vec![json!({"type":"rule","index":0})]].concat(),
            "regular",
        ),
        ExitCode::Complete,
        None,
        false,
    );
    assert_eq!(c.row, 22);
}
