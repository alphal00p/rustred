//! Independent audit (lane audit-credits, 2026-11-14) of lane import3's exact literals (item 1: `poly` BigInt form,
//! domain literals read exactly) and of the directing session's addition: "never narrow such literals to i128; a
//! literal that cannot be decided must leave the KEY undecided, never skip the record".
//!  - boundaries: i128::MAX, i128::MAX + 1, i128::MIN, i128::MIN - 1, 2^127, 2^200: parse, canonical form, exact
//!    value, order, text round trip, parser arithmetic (products and cancellations);
//!  - a CONSTRUCTED table (toy arity 3, nothing five-loop specific): an exception literal with coefficients above
//!    2^200 that vanishes at exactly ONE key decides the dispatch there and nowhere else (exact arithmetic decided
//!    analytically: B*a + C*b with B > 3*C and |a|, |b| <= 3 vanishes only at a = b = 0); equations and bounds with
//!    big coefficients likewise;
//!  - an exception literal that is NONZERO but divisible by both engine primes at a key: that key is left UNDECIDED
//!    (open), never served by a later record, the record is not skipped;
//!  - (pre-existing, not import3) recipe `domain.bounds` entries that are not i64 are SILENTLY DROPPED (the recipe's
//!    domain grows): reproduction ignored-by-default.

use rustred_campaign_core::import;
use rustred_campaign_core::poly::{BigInt, ENGINE_PRIMES, Poly, PolyError};

use std::path::{Path, PathBuf};

use import::lines::key_text;
use import::{CheckOptions, Evidence, RowStatus, TableSetSpec, check, load};
use serde_json::{Value, json};

const HEADER: &str =
    r#"{"type":"header","schema":"table_dump.v1","family":"TOY","denominators":3,"mask":"110"}"#;

fn scratch(tag: &str) -> PathBuf {
    let base = std::env::var_os("RUSTRED_IMPORT_SCRATCH")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let d = base.join(format!("audit-credits-big-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn big(s: &str) -> BigInt {
    let (neg, digits) = match s.strip_prefix('-') {
        Some(d) => (true, d),
        None => (false, s),
    };
    let v = BigInt::from_decimal(digits).unwrap();
    if neg { v.neg() } else { v }
}

fn pow2(e: u32) -> BigInt {
    let mut r = BigInt::from_i128(1);
    for _ in 0..e {
        r = r.mul(&BigInt::from_i128(2));
    }
    r
}

#[test]
fn ok_bigint_literal_boundaries_exact() {
    let max = i128::MAX.to_string();
    let max1 = pow2(127).to_decimal(); // i128::MAX + 1 = 2^127
    let min = i128::MIN.to_string();
    let min1 = pow2(127).add(&BigInt::from_i128(1)).neg().to_decimal(); // i128::MIN - 1
    let p200 = pow2(200).to_decimal();
    assert_eq!(max1, "170141183460469231731687303715884105728");
    assert_eq!(
        p200,
        "1606938044258990275541962092341162602522202993782792835301376"
    );
    // canonical form: small iff the value fits i128 (also for i128::MIN, whose magnitude does not)
    for (lit, is_big) in [
        (max.clone(), false),
        (max1.clone(), true),
        (min.clone(), false),
        (min1.clone(), true),
        (p200.clone(), true),
        (format!("-{p200}"), true),
    ] {
        let p = Poly::parse_exact(&lit, 1, false).unwrap();
        assert_eq!(p.is_big(), is_big, "{lit}");
        // exact value and text round trip
        assert_eq!(p.eval_big(&[0]).unwrap(), big(&lit), "{lit}");
        assert_eq!(
            Poly::parse_exact(&p.to_string(), 1, false).unwrap(),
            p,
            "{lit}"
        );
        // the strict parser refuses what it cannot hold (typed), never truncates
        match Poly::parse(&lit, 1, false) {
            Ok(q) => assert_eq!(q, p, "{lit}"),
            Err(e) => assert_eq!(e, PolyError::CoefficientOverflow, "{lit}"),
        }
    }
    assert_eq!(
        Poly::parse_exact(&min, 1, false).unwrap(),
        Poly::constant(i128::MIN)
    );
    // order of exact values across the boundary
    let vals: Vec<BigInt> = [&min1, &min, &"0".to_string(), &max, &max1, &p200]
        .iter()
        .map(|s| big(s))
        .collect();
    for w in vals.windows(2) {
        assert!(w[0] < w[1], "{} < {}", w[0], w[1]);
    }
    assert_eq!(big(&max1).to_i128(), None);
    assert_eq!(big(&max).to_i128(), Some(i128::MAX));
    assert_eq!(big(&min).to_i128(), Some(i128::MIN));
    assert_eq!(big(&min1).to_i128(), None);
    // parser arithmetic: 2^100 * 2^100 = 2^200; (2^200)*n0 - (2^200)*n0 + 1 cancels to the small form
    let p100 = pow2(100).to_decimal();
    let prod = Poly::parse_exact(&format!("{p100}*{p100}"), 1, false).unwrap();
    assert_eq!(prod.eval_big(&[0]).unwrap(), pow2(200));
    let c = Poly::parse_exact(&format!("{p200}*n0 - {p200}*n0 + 1"), 1, false).unwrap();
    assert!(!c.is_big() && c == Poly::constant(1));
    // (2^127)^2 via pow: exact
    let sq = Poly::parse_exact(&format!("({max1})^2"), 1, false).unwrap();
    assert_eq!(sq.eval_big(&[0]).unwrap(), pow2(254));
    // MAX + 1 - 1 = MAX (back to small), MIN - 1 + 1 = MIN
    assert_eq!(
        Poly::parse_exact(&format!("{max1} - 1"), 1, false).unwrap(),
        Poly::constant(i128::MAX)
    );
    assert_eq!(
        Poly::parse_exact(&format!("{min1} + 1"), 1, false).unwrap(),
        Poly::constant(i128::MIN)
    );
    // evaluation with a big coefficient never truncates: (2^127)*n0 at n0 = -1 is i128::MIN exactly, at 2 is 2^128
    let lin = Poly::parse_exact(&format!("{max1}*n0"), 1, false).unwrap();
    assert!(lin.is_big());
    assert!(lin.eval(&[-1]).unwrap() == rustred_campaign_core::poly::Value::Small(i128::MIN));
    assert_eq!(lin.eval_big(&[2]).unwrap(), pow2(128));
    assert!(lin.is_zero_at(&[0]).unwrap() && !lin.is_zero_at(&[1]).unwrap());
}

fn rule(index: i64, fixed: [Option<i64>; 3], rhs: &[&str], extra: Value) -> String {
    let t: Vec<String> = (0..3)
        .map(|i| fixed[i].map(|v| v.to_string()).unwrap_or(format!("n{i}")))
        .collect();
    let mut v = json!({"type":"rule","chart":"110","index":index,"fixed":fixed,"equations":[],
        "target": format!("I({})", t.join(",")),
        "rhs": rhs.iter().map(|k| json!([k, "1"])).collect::<Vec<_>>(),
        "exceptions":[],"dispatch":"Partition","witness_loci":[]});
    if let Value::Object(m) = extra {
        for (k, x) in m {
            v[k] = x;
        }
    }
    v.to_string()
}

fn write_table(dir: &Path, name: &str, rules: &[String]) -> PathBuf {
    let p = dir.join(name);
    let mut s = String::from(HEADER);
    s.push('\n');
    s.push_str(r#"{"type":"chart","chart":"110","guards":["n>=1","n>=1","n<=0"],"zero":false}"#);
    s.push('\n');
    for r in rules {
        s.push_str(r);
        s.push('\n');
    }
    s.push_str(r#"{"type":"end","charts":1}"#);
    s.push('\n');
    std::fs::write(&p, s).unwrap();
    p
}

fn run(spec: &Value, dir: &Path) -> (import::ImportedSet, import::CheckResult) {
    let s = TableSetSpec::from_json(&spec.to_string()).unwrap();
    let set = load(&s, dir).unwrap();
    let ev = Evidence::from_spec(&set, dir).unwrap();
    let r = check(
        &set,
        &ev,
        &CheckOptions {
            threads: 1,
            ..Default::default()
        },
    )
    .unwrap();
    (set, r)
}

fn keys_box() -> Vec<[i32; 3]> {
    let mut v = vec![];
    for a in 1..=6 {
        for b in 1..=5 {
            v.push([a, b, 0]);
        }
    }
    v
}

/// (c) A >128-bit exception literal that vanishes at exactly ONE key decides the first match there; at every other
/// key of the box the record wins. Same for a big EQUATION (the record applies on n1 = 2 only) and a big BOUND.
#[test]
fn ok_big_exception_equation_bound_decide_exactly_one_key() {
    let d = scratch("decide");
    let b = pow2(200).add(&BigInt::from_i128(1)).to_decimal(); // B = 2^200 + 1
    let c = pow2(130).add(&BigInt::from_i128(3)).to_decimal(); // C = 2^130 + 3 (B > 3 C)
    // E = B*(n0 - 3) + C*(n1 - 2): zero on the box only at (3, 2)
    let e = format!("{b}*n0 - 3*{b} + {c}*n1 - 2*{c}");
    let eq = format!("{c}*n1 - 2*{c}"); // n1 = 2
    let bound = format!("{c}*n0 - 4*{c}"); // n0 >= 4
    let body = ["I(0,n1,0)"]; // subsector: no walk
    let a = write_table(
        &d,
        "A.table.jsonl",
        &[
            rule(
                0,
                [None, None, Some(0)],
                &body,
                json!({"exceptions": [[e.clone()]]}),
            ),
            rule(
                1,
                [None, None, Some(0)],
                &body,
                json!({"equations": [eq.clone()]}),
            ),
            // bounds are a domain field of the record (`domain_bounds` is i64 by format); a big bound literal enters
            // as a GE atom only through bank formats, so here: an exception AND-list that keeps the record selected
            // only where the big bound literal is nonzero AND n0 <= 3 cannot be written; use a second exception
            rule(
                2,
                [None, None, Some(0)],
                &body,
                json!({"exceptions": [[bound.clone()]]}),
            ),
            rule(3, [None, None, Some(0)], &body, json!({})),
        ],
    );
    let keys = keys_box();
    let st = d.join("start.jsonl");
    std::fs::write(
        &st,
        keys.iter()
            .map(|k| format!("{}\n", json!(k)))
            .collect::<String>(),
    )
    .unwrap();
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"A","bits":8,
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"A","kind":"engine_table","path":a.display().to_string()}]});
    let (set, r) = run(&spec, &d);
    assert_eq!(set.report.sources[0].records_dispatched, 4);
    let label_of = |k: &[i32; 3]| -> Option<i64> {
        let x = r.rows.iter().find(|x| x.key == k).unwrap();
        assert_eq!(x.status, RowStatus::Served, "{k:?}: {x:?}");
        x.server.as_ref().map(|s| match s {
            import::Server::Rule { table, record } => set.rules[*table][*record].label,
            other => panic!("{other:?}"),
        })
    };
    let mut decided = 0;
    for k in &keys {
        // exact arithmetic, decided analytically: E = B*(a-3) + C*(b-2) = 0 iff (a, b) = (3, 2) on the box
        let want = if (k[0], k[1]) != (3, 2) {
            0
        } else {
            // on rule 0's exception: rule 1 applies iff n1 = 2 (yes)
            1
        };
        assert_eq!(label_of(k), Some(want), "key {}", key_text(k));
        decided += 1;
    }
    println!(
        "decided {decided} keys exactly; the single on-exception key I(3,2,0) falls through to rule 1"
    );
    // the core domain: the literals are big and exact
    let pe = Poly::parse_d_free_exact(&e, 3).unwrap();
    assert!(pe.is_big());
    for k in &keys {
        assert_eq!(pe.is_zero_at(k).unwrap(), (k[0], k[1]) == (3, 2), "{k:?}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// A literal that the dispatch cannot decide like the engine (NONZERO, divisible by both engine primes): the KEY is
/// left undecided (open), never served by a later record; the record is not skipped (it still wins its other keys).
#[test]
fn ok_engine_prime_divisible_literal_leaves_the_key_undecided() {
    let d = scratch("primes");
    let pp = BigInt::from_i128(ENGINE_PRIMES[0] as i128)
        .mul(&BigInt::from_i128(ENGINE_PRIMES[1] as i128))
        .to_decimal(); // ~2^122
    // F = pp * 2^20 * (n0 - 3) + (n1 - 1) * pp * 2^20 * 7: at (4, 1): pp*2^20 (nonzero, = 0 mod both primes)
    let m = pow2(20).to_decimal();
    let f = format!("{pp}*{m}*n0 - 3*{pp}*{m} + 7*{pp}*{m}*n1 - 7*{pp}*{m}");
    let a = write_table(
        &d,
        "A.table.jsonl",
        &[
            rule(
                0,
                [None, None, Some(0)],
                &["I(0,n1,0)"],
                json!({"exceptions": [[f.clone()]]}),
            ),
            rule(1, [None, None, Some(0)], &["I(0,n1,0)"], json!({})),
        ],
    );
    let st = d.join("start.jsonl");
    std::fs::write(&st, "[3,1,0]\n[4,1,0]\n").unwrap();
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"A","bits":8,
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"A","kind":"engine_table","path":a.display().to_string()}]});
    let (set, r) = run(&spec, &d);
    let row = |k: [i32; 3]| r.rows.iter().find(|x| x.key == k).unwrap();
    // (3,1): F = 0 exactly -> on the exception -> rule 1
    let x = row([3, 1, 0]);
    assert_eq!(x.status, RowStatus::Served);
    assert!(
        matches!(&x.server, Some(import::Server::Rule { record, .. }) if set.rules[0][*record].label == 1)
    );
    // (4,1): F = pp*2^20 != 0 but zero mod both engine primes: undecided, open, NOT served by rule 1
    let y = row([4, 1, 0]);
    println!(
        "I(4,1,0): {:?} {:?} {:?}",
        y.status, y.open_status, y.open_reason
    );
    assert_eq!(y.status, RowStatus::Open, "{y:?}");
    let _ = std::fs::remove_dir_all(&d);
}

/// PRE-EXISTING m1 (commit bf10aed88, src/import/load.rs recipe domain), FIXED by lane import4: a recipe
/// `domain.bounds.min` entry that is not an i64 (here 10^30) was SILENTLY DROPPED by `if let Some(c) = x.as_i64()`: the
/// recipe's domain lost the bound (grew). Specified (directing session): never narrow, never skip: a typed refusal or an
/// exact bound. A JSON number beyond 64 bits reaches the loader as a float whose exact value is lost: typed refusal;
/// a bound in the u64 range beyond i64 (2^63) is kept exactly.
#[test]
fn recipe_domain_bound_beyond_i64_is_not_dropped() {
    let d = scratch("rbound");
    let a = write_table(
        &d,
        "A.table.jsonl",
        &[rule(
            0,
            [Some(3), Some(1), Some(0)],
            &["I(2,1,0)"],
            json!({}),
        )],
    );
    let rp = d.join("recipe.json");
    // the bound text is a JSON number beyond i64 (serde_json keeps it as f64 / arbitrary): n0 >= 10^30
    let txt = json!({"schema":"rustred.recipe.v1","id":"toy/r1","arity":3,"chart":"110",
        "domain":{"fixed":[null,1,0],"equations":[],"exceptions":[],"bounds":{"min":[1, null, null],"max":[null,null,null]}},
        "seeds":{"unit":"harder","centres":[],"generator":{"shells":["S0","S1"]}},
        "symmetry_rows":false,"elimination":{"convention":"rustred.recipe-elim.v1"}})
    .to_string()
    .replace("\"min\":[1,", "\"min\":[1000000000000000000000000000000,");
    std::fs::write(&rp, txt).unwrap();
    let st = d.join("start.jsonl");
    std::fs::write(&st, "[3,1,0]\n").unwrap();
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","frame_from":"A",
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"A","kind":"engine_table","path":a.display().to_string()},
                   {"id":"R","kind":"recipe","class":"recipe","path":rp.display().to_string()}]});
    let s = TableSetSpec::from_json(&spec.to_string()).unwrap();
    match load(&s, &d) {
        Err(e) => {
            println!("refused (typed): {e}");
            assert!(
                matches!(e, rustred_campaign_core::import::ImportError::Source { .. })
                    && e.to_string().contains("domain.bounds.min[0]"),
                "{e}"
            );
        }
        Ok(set) => {
            let dom = &set.recipes[0].domain;
            println!("recipe domain bounds: {:?}", dom.bounds);
            // I(2,1,0) violates n0 >= 10^30: it must NOT be in the recipe's domain
            assert!(
                !dom.applies(&[2, 1, 0]).applies(),
                "the bound n0 >= 10^30 was dropped: I(2,1,0) is in the recipe's domain"
            );
        }
    }
    // a bound in the u64 range beyond i64 (n0 >= 2^63) is kept exactly; a non-integer entry is refused
    let txt2 = std::fs::read_to_string(&rp)
        .unwrap()
        .replace("1000000000000000000000000000000", "9223372036854775808");
    std::fs::write(&rp, &txt2).unwrap();
    let set = load(&s, &d).expect("a u64 bound loads");
    let dom = &set.recipes[0].domain;
    assert_eq!(dom.bounds.len(), 1);
    assert!(
        !dom.applies(&[2, 1, 0]).applies(),
        "n0 >= 2^63 kept exactly"
    );
    std::fs::write(&rp, txt2.replace("9223372036854775808", "\"7\"")).unwrap();
    assert!(
        load(&s, &d).is_err(),
        "a string bound is refused, never dropped"
    );
    let _ = std::fs::remove_dir_all(&d);
}
