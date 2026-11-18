//! Deterministic tests of the weight-certificate checker on small synthetic certificates (arity 2, a toy row generator
//! standing in for the ordinary IBP rows), plus the real certificates of the engine lane as TEST DATA when present
//! (RUSTRED_WCERT_DATA = engine-line/wcert directory, RUSTRED_WCERT_TABLE = the certified table, RUSTRED_WCERT_T1 = the
//! patched table with the 987 pieces; skipped with a message when absent).
//!
//! The toy setup (hand-derived, see the notes): operator rows
//!   O_0 = (d-2 n0) I(n0,n1) + n0 I(n0+1,n1) + I(n0,n1-1),     O_1 = n1 I(n0,n1) + (n0-1) I(n0-1,n1+1)
//! weights W_0 = 1/(d-2 n0), W_1 = 1/(n0-1) at the seed I(n0,n1) give the raw row
//!   R = n0/(d-2n0) I(n0+1,n1) + (n0-1+n1)/(n0-1) I(n0,n1) + 1/(d-2n0) I(n0,n1-1) + I(n0-1,n1+1),
//! whose canonical form (translation n = m + (-1, 0)) is the stored rule below.
use super::*;
use crate::core::jparse;

fn rows() -> EngineRows {
    EngineRows {
        note: "toy generator (test)".into(),
        lines: vec![
            br#"{"type":"source_row","m":0,"differentiated_loop":0,"contraction_momentum":0,"terms":[["I(n0,n1)","d-2*n0"],["I(n0+1,n1)","n0"],["I(n0,n1-1)","1"]]}"#.to_vec(),
            br#"{"type":"source_row","m":1,"differentiated_loop":0,"contraction_momentum":1,"terms":[["I(n0,n1)","n1"],["I(n0-1,n1+1)","-1+n0"]]}"#.to_vec(),
        ],
    }
}

const RHS_OK: &str = r#"[["I(n0-1,n1)", "-(n0-2+n1)*(d-2*n0+2)/((n0-2)*(n0-1))"], ["I(n0-1,n1-1)", "-1/(n0-1)"], ["I(n0-2,n1+1)", "-(d-2*n0+2)/(n0-1)"]]"#;

fn stored(rhs: &str, exceptions: &str, extra: &str) -> Vec<u8> {
    format!(
        r#"{{"type": "rule", "chart": "10", "index": 0, "case": "toy", "fixed": [null, null], "equations": [], "target": "I(n0,n1)", "rhs": {rhs}, "exceptions": {exceptions}, "recenter": [-1, 0]{extra}}}"#
    )
    .into_bytes()
}

const ROW: &str = r#"[["I(n0+1,n1)","n0/(d-2*n0)"],["I(n0,n1)","(n0-1+n1)/(n0-1)"],["I(n0,n1-1)","1/(d-2*n0)"],["I(n0-1,n1+1)","1"]]"#;
const FACTORS: &str =
    r#"[{"factor":"d-2*n0","index":true,"terms":2},{"factor":"-1+n0","index":true,"terms":2}]"#;

fn v1(terms: &str, factors: &str, extra: &str) -> Vec<u8> {
    format!(
        r#"{{"schema":"rustred.weight-cert.v1","basis":"ordinary","case":"toy","target":"I(n0,n1)","ordinary_terms":{terms},"row":{ROW},"denominator_factors":{factors}{extra}}}"#
    )
    .into_bytes()
}

const TERMS_OK: &str =
    r#"[{"seed":"I(n0,n1)","m":0,"w":"1/(d-2*n0)"},{"seed":"I(n0,n1)","m":1,"w":"1/(n0-1)"}]"#;

fn field(rec: &str, path: &[&str]) -> String {
    let b = rec.as_bytes();
    let j = jparse(b);
    let mut cur = &j;
    for k in path {
        cur = cur
            .get(b, k)
            .unwrap_or_else(|| panic!("no field {k} in {rec}"));
    }
    match cur {
        J::Str(s, e) => String::from_utf8_lossy(&b[*s..*e]).to_string(),
        J::Num(s, e) => String::from_utf8_lossy(&b[*s..*e]).to_string(),
        J::Bool(x) => x.to_string(),
        J::Null => "null".into(),
        _ => "<compound>".into(),
    }
}

fn run_ok(stored_line: &[u8], cert: &[u8], engine: Option<&EngineRows>) -> String {
    let r = run_bytes(stored_line, cert, engine).unwrap_or_else(|e| panic!("refused: {e}"));
    eprintln!("{r}");
    r
}

fn verdict_of(rec: &str, factor: &str) -> String {
    let b = rec.as_bytes();
    let j = jparse(b);
    for l in j.get(b, "loci").unwrap().arr() {
        let f = String::from_utf8_lossy(l.get(b, "factor").unwrap().s(b)).to_string();
        if f == factor {
            return String::from_utf8_lossy(l.get(b, "verdict").unwrap().s(b)).to_string();
        }
    }
    panic!("factor {factor} not in loci of {rec}")
}

// ---------------------------------------------------------------- algebra layer

#[test]
fn shift_is_substitution_x_plus_s() {
    let v = Vars::new(2);
    let r = v.rf("n0/(d-2*n0)").unwrap();
    let s = alg::shift_rf(&r, &[(Vars::slot(0), -1)]);
    assert_eq!(s, v.rf("(n0-1)/(d-2*n0+2)").unwrap());
}

#[test]
fn factors_are_primitive_with_positive_leading_coefficient() {
    let v = Vars::new(2);
    let p = v.poly("(-36-8*n1-4*n0+12*d)*(2-2*n0)^2").unwrap();
    let f: Vec<String> = alg::irreducible_factors(&p)
        .iter()
        .map(|x| x.to_string())
        .collect();
    assert_eq!(f.len(), 2, "{f:?}");
    assert!(f.contains(&v.poly("-9-2*n1-n0+3*d").unwrap().to_string()));
    assert!(f.contains(&v.poly("n0-1").unwrap().to_string()));
}

#[test]
fn sign_on_box_and_ideal_membership() {
    let v = Vars::new(2);
    let mut b = BTreeMap::new();
    b.insert(
        Vars::slot(0),
        alg::Bound {
            lo: Some(2),
            hi: None,
        },
    );
    b.insert(
        Vars::slot(1),
        alg::Bound {
            lo: None,
            hi: Some(0),
        },
    );
    // n0 - n1 >= 2 on the box: positive
    assert!(alg::sign_on_box(&v.poly("n0-n1").unwrap(), &b, false).is_some());
    // n0 + n1 changes sign on the box
    assert!(alg::sign_on_box(&v.poly("n0+n1").unwrap(), &b, false).is_none());
    // n0 - 1 > 0 on n0 >= 2
    assert!(alg::sign_on_box(&v.poly("n0-1").unwrap(), &b, false).is_some());
    let g = alg::groebner(&[v.poly("n0-1").unwrap(), v.poly("n1").unwrap()]);
    assert!(alg::in_ideal(&v.poly("n0*n1+n1^2+n0-1").unwrap(), &g));
    assert!(!alg::in_ideal(&v.poly("n0").unwrap(), &g));
    assert!(alg::is_unit_ideal(&alg::groebner(&[
        v.poly("n0-1").unwrap(),
        v.poly("n0-2").unwrap()
    ])));
}

#[test]
fn binding_hashes_match_python_json_dumps() {
    let r = parse_rule(&stored(RHS_OK, r#"[["-1+n0"], ["-2+n0"]]"#, "")).unwrap();
    // reference values computed with fc_tool.py's definitions (python3 json.dumps, separators (',', ':'))
    assert_eq!(
        r.target_rhs_sha256,
        "1fa2f6ada10cc334dcc6a5ca65859c2bbe99c7434b1e1c492404e2176fc12c19"
    );
    assert_eq!(
        r.domain_sha256,
        "de62e78251438d2384fdfd6b972bfb3c5c09f9d480477ec6dbfd6af0598adbf7"
    );
}

// ---------------------------------------------------------------- the seven typed outcomes of the brief

#[test]
fn correct_certificate_is_whole_domain() {
    let rec = run_ok(
        &stored(RHS_OK, r#"[["-1+n0"], ["-2+n0"]]"#, ""),
        &v1(TERMS_OK, FACTORS, ""),
        Some(&rows()),
    );
    assert_eq!(field(&rec, &["rows", "verdict"]), "ROWS_VERIFIED");
    assert_eq!(field(&rec, &["identity", "verdict"]), "IDENTITY_EXACT");
    assert_eq!(field(&rec, &["binding", "verdict"]), "BINDING_EQUAL");
    assert_eq!(field(&rec, &["binding", "stored_recenter_agrees"]), "true");
    assert_eq!(field(&rec, &["factors", "producer"]), "PRODUCER_LIST_EQUAL");
    assert_eq!(verdict_of(&rec, "2-2*n0+d"), "EMPTY_CONSTANT");
    assert_eq!(verdict_of(&rec, "-2+n0"), "IN_EXCEPTION");
    assert_eq!(verdict_of(&rec, "-1+n0"), "IN_EXCEPTION");
    assert_eq!(field(&rec, &["class"]), "WHOLE_DOMAIN");
}

#[test]
fn wrong_weight_fails_the_identity() {
    let terms =
        r#"[{"seed":"I(n0,n1)","m":0,"w":"1/(d-2*n0)"},{"seed":"I(n0,n1)","m":1,"w":"2/(n0-1)"}]"#;
    let rec = run_ok(
        &stored(RHS_OK, r#"[["-1+n0"], ["-2+n0"]]"#, ""),
        &v1(terms, FACTORS, ""),
        Some(&rows()),
    );
    assert_eq!(field(&rec, &["identity", "verdict"]), "IDENTITY_FAIL");
    assert_eq!(field(&rec, &["class"]), "FAIL");
}

#[test]
fn missing_source_fails() {
    // (i) a weighted term removed: the identity fails
    let terms = r#"[{"seed":"I(n0,n1)","m":0,"w":"1/(d-2*n0)"}]"#;
    let rec = run_ok(
        &stored(RHS_OK, r#"[["-1+n0"], ["-2+n0"]]"#, ""),
        &v1(terms, FACTORS, ""),
        Some(&rows()),
    );
    assert_eq!(field(&rec, &["identity", "verdict"]), "IDENTITY_FAIL");
    assert_eq!(field(&rec, &["class"]), "FAIL");
    // (ii) a weight on a source the certificate does not have (composed form): malformed, rows FAIL
    let cert = format!(
        r#"{{"schema":"rustred.weight-cert.v1","basis":"composed","case":"toy","target":"I(n0,n1)","sources":[{{"basis_row":7,"seed":"I(n0,n1)"}}],"basis_rows":{{"7":[[0,"1"]]}},"weights":[{{"j":0,"w":"1/(d-2*n0)"}},{{"j":1,"w":"1/(n0-1)"}}],"row":{ROW}}}"#
    );
    let rec = run_ok(
        &stored(RHS_OK, r#"[["-1+n0"], ["-2+n0"]]"#, ""),
        cert.as_bytes(),
        Some(&rows()),
    );
    assert_eq!(field(&rec, &["rows", "verdict"]), "ROWS_FAIL");
    assert_eq!(field(&rec, &["class"]), "FAIL");
}

#[test]
fn wrong_binding_fails() {
    // one stored coefficient differs
    let rhs = RHS_OK.replace("\"-1/(n0-1)\"", "\"-2/(n0-1)\"");
    let rec = run_ok(
        &stored(&rhs, r#"[["-1+n0"], ["-2+n0"]]"#, ""),
        &v1(TERMS_OK, FACTORS, ""),
        Some(&rows()),
    );
    assert_eq!(field(&rec, &["identity", "verdict"]), "IDENTITY_EXACT");
    assert_eq!(field(&rec, &["binding", "verdict"]), "BINDING_TERMS_DIFFER");
    assert_eq!(field(&rec, &["class"]), "FAIL");
    // the certificate names another target
    let cert = String::from_utf8(v1(TERMS_OK, FACTORS, ""))
        .unwrap()
        .replace(r#""target":"I(n0,n1)""#, r#""target":"I(n0,n1-1)""#);
    let rec = run_ok(
        &stored(RHS_OK, r#"[["-1+n0"], ["-2+n0"]]"#, ""),
        cert.as_bytes(),
        Some(&rows()),
    );
    assert_eq!(
        field(&rec, &["binding", "verdict"]),
        "BINDING_TARGET_DIFFERS"
    );
    assert_eq!(field(&rec, &["class"]), "FAIL");
    // the certificate names another case
    let cert = String::from_utf8(v1(TERMS_OK, FACTORS, ""))
        .unwrap()
        .replace(r#""case":"toy""#, r#""case":"other""#);
    let rec = run_ok(
        &stored(RHS_OK, r#"[["-1+n0"], ["-2+n0"]]"#, ""),
        cert.as_bytes(),
        Some(&rows()),
    );
    assert_eq!(field(&rec, &["binding", "verdict"]), "BINDING_CASE_DIFFERS");
}

#[test]
fn domain_is_read_from_the_stored_line_only() {
    // the certificate's own domain fields are ignored (and reported)
    let extra = r#","domain_bounds":{"min":[3,null],"max":[null,0]},"exceptions":[]"#;
    let rec = run_ok(
        &stored(RHS_OK, r#"[["-1+n0"], ["-2+n0"]]"#, ""),
        &v1(TERMS_OK, FACTORS, extra),
        Some(&rows()),
    );
    assert_eq!(field(&rec, &["class"]), "WHOLE_DOMAIN");
    assert!(
        rec.contains(r#""ignored_domain_fields":["domain_bounds","exceptions"]"#),
        "{rec}"
    );
    // a stored domain with bounds n0 >= 3 and no branches: both index loci empty by sign
    let rec = run_ok(
        &stored(
            RHS_OK,
            "[]",
            r#", "domain_bounds": {"min": [3, null], "max": [null, 0]}"#,
        ),
        &v1(TERMS_OK, FACTORS, ""),
        Some(&rows()),
    );
    assert_eq!(verdict_of(&rec, "-2+n0"), "EMPTY_SIGN");
    assert_eq!(verdict_of(&rec, "-1+n0"), "EMPTY_SIGN");
    assert_eq!(field(&rec, &["class"]), "WHOLE_DOMAIN");
}

#[test]
fn locus_nonempty_on_the_domain_is_open() {
    // stored domain without the branch n0 = 2: the weight pole n0 - 1 (raw) = n0 - 2 (stored frame) has the keys n0 = 2
    let rec = run_ok(
        &stored(RHS_OK, r#"[["-1+n0"]]"#, ""),
        &v1(TERMS_OK, FACTORS, ""),
        Some(&rows()),
    );
    assert_eq!(verdict_of(&rec, "-2+n0"), "OPEN");
    assert_eq!(field(&rec, &["open_loci"]), "1");
    assert_eq!(field(&rec, &["class"]), "OPEN_LOCI");
}

#[test]
fn producer_list_omitting_a_factor_is_reported() {
    let rec = run_ok(
        &stored(RHS_OK, r#"[["-1+n0"], ["-2+n0"]]"#, ""),
        &v1(
            TERMS_OK,
            r#"[{"factor":"d-2*n0","index":true,"terms":2}]"#,
            "",
        ),
        Some(&rows()),
    );
    assert_eq!(
        field(&rec, &["factors", "producer"]),
        "PRODUCER_LIST_INCOMPLETE"
    );
    assert!(rec.contains(r#""producer_missing":["-1+n0"]"#), "{rec}");
    // the tool's own list decides: the class is unchanged
    assert_eq!(field(&rec, &["class"]), "WHOLE_DOMAIN");
}

// ---------------------------------------------------------------- forms

#[test]
fn v0_without_rows_is_never_whole_domain() {
    let cert = format!(
        r#"{{"schema":"rustred.weight-cert.v0","case":"toy","target":"I(n0,n1)","sources":[{{"basis_row":3,"seed":"I(n0,n1)","shifts":[0,0]}},{{"basis_row":4,"seed":"I(n0,n1)","shifts":[0,0]}}],"weights":[{{"j":0,"w":"1/(d-2*n0)"}},{{"j":1,"w":"1/(n0-1)"}}],"row":{ROW},"denominator_factors":{FACTORS}}}"#
    );
    let rec = run_ok(
        &stored(RHS_OK, r#"[["-1+n0"], ["-2+n0"]]"#, ""),
        cert.as_bytes(),
        Some(&rows()),
    );
    assert_eq!(
        field(&rec, &["rows", "verdict"]),
        "ROWS_NOT_INDEPENDENTLY_VERIFIED"
    );
    assert_eq!(
        field(&rec, &["identity", "verdict"]),
        "IDENTITY_NOT_CHECKABLE"
    );
    assert_eq!(field(&rec, &["binding", "verdict"]), "BINDING_EQUAL");
    assert_eq!(field(&rec, &["class"]), "NOT_WHOLE_DOMAIN");
    assert_eq!(field(&rec, &["conditional_class"]), "WHOLE_DOMAIN_IF_ROWS");
}

#[test]
fn v0_with_rows_checks_the_identity_but_not_the_rows() {
    let carried = r#"[[["I(n0,n1)","d-2*n0"],["I(n0+1,n1)","n0"],["I(n0,n1-1)","1"]],[["I(n0,n1)","n1"],["I(n0-1,n1+1)","n0-1"]]]"#;
    let cert = format!(
        r#"{{"schema":"rustred.weight-cert.v0","case":"toy","target":"I(n0,n1)","sources":[{{"basis_row":3,"seed":"I(n0,n1)"}},{{"basis_row":4,"seed":"I(n0,n1)"}}],"weights":[{{"j":0,"w":"1/(d-2*n0)"}},{{"j":1,"w":"1/(n0-1)"}}],"row":{ROW},"rows":{carried}}}"#
    );
    let rec = run_ok(
        &stored(RHS_OK, r#"[["-1+n0"], ["-2+n0"]]"#, ""),
        cert.as_bytes(),
        None,
    );
    assert_eq!(field(&rec, &["identity", "verdict"]), "IDENTITY_EXACT");
    assert_eq!(
        field(&rec, &["rows", "verdict"]),
        "ROWS_NOT_INDEPENDENTLY_VERIFIED"
    );
    assert_eq!(
        field(&rec, &["factors", "producer"]),
        "PRODUCER_LIST_ABSENT"
    );
    assert_eq!(field(&rec, &["class"]), "NOT_WHOLE_DOMAIN");
    // a wrong carried row is caught by the identity
    let bad = cert.replace(r#"["I(n0-1,n1+1)","n0-1"]"#, r#"["I(n0-1,n1+1)","n0-2"]"#);
    let rec = run_ok(
        &stored(RHS_OK, r#"[["-1+n0"], ["-2+n0"]]"#, ""),
        bad.as_bytes(),
        None,
    );
    assert_eq!(field(&rec, &["identity", "verdict"]), "IDENTITY_FAIL");
    assert_eq!(field(&rec, &["class"]), "FAIL");
}

#[test]
fn v1_composed_form_with_a_composition_pole() {
    // basis row 8 = O_1 / (n0 - 1) as an operator; weight 1 on it: same identity, the pole comes from the composition
    let cert = format!(
        r#"{{"schema":"rustred.weight-cert.v1","basis":"composed","case":"toy","target":"I(n0,n1)","sources":[{{"basis_row":7,"seed":"I(n0,n1)"}},{{"basis_row":8,"seed":"I(n0,n1)"}}],"basis_rows":{{"7":[[0,"1"]],"8":[[1,"1/(n0-1)"]]}},"weights":[{{"j":0,"w":"1/(d-2*n0)"}},{{"j":1,"w":"1"}}],"row":{ROW}}}"#
    );
    let rec = run_ok(
        &stored(RHS_OK, r#"[["-1+n0"], ["-2+n0"]]"#, ""),
        cert.as_bytes(),
        Some(&rows()),
    );
    assert_eq!(field(&rec, &["identity", "verdict"]), "IDENTITY_EXACT");
    assert!(rec.contains(r#""compositions":1"#), "{rec}");
    assert_eq!(field(&rec, &["class"]), "WHOLE_DOMAIN");
}

#[test]
fn v1_without_a_generator_is_not_checkable() {
    let rec = run_ok(
        &stored(RHS_OK, r#"[["-1+n0"], ["-2+n0"]]"#, ""),
        &v1(TERMS_OK, FACTORS, ""),
        None,
    );
    assert_eq!(
        field(&rec, &["identity", "verdict"]),
        "IDENTITY_NOT_CHECKABLE"
    );
    assert_eq!(field(&rec, &["class"]), "NOT_WHOLE_DOMAIN");
}

// ---------------------------------------------------------------- row recovery (lane gencheck, recover.rs)

fn recover_side() -> Side {
    Side {
        recover_rows: true,
        ..Default::default()
    }
}

fn v0_rows_cert(sources: &str, weights: &str, carried: &str) -> String {
    format!(
        r#"{{"schema":"rustred.weight-cert.v0","case":"toy","target":"I(n0,n1)","sources":{sources},"weights":{weights},"row":{ROW},"rows":{carried}}}"#
    )
}

#[test]
fn v0_rows_recovered_over_the_generator_is_whole_domain() {
    // carried A_0 = O_0 + n0 O_1 (a polynomial combination, as the engine's preconditioner builds), A_1 = O_1;
    // w_0 = 1/(d-2n0), w_1 = 1/(n0-1) - n0/(d-2n0): sum w A = W_0 O_0 + W_1 O_1 = ROW
    let carried = r#"[[["I(n0,n1)","d-2*n0+n0*n1"],["I(n0+1,n1)","n0"],["I(n0,n1-1)","1"],["I(n0-1,n1+1)","n0^2-n0"]],[["I(n0,n1)","n1"],["I(n0-1,n1+1)","n0-1"]]]"#;
    let cert = v0_rows_cert(
        r#"[{"basis_row":3,"seed":"I(n0,n1)"},{"basis_row":4,"seed":"I(n0,n1)"}]"#,
        r#"[{"j":0,"w":"1/(d-2*n0)"},{"j":1,"w":"1/(n0-1)-n0/(d-2*n0)"}]"#,
        carried,
    );
    let st = stored(RHS_OK, r#"[["-1+n0"], ["-2+n0"]]"#, "");
    // without recovery: identity over the carried rows only
    let rec = run_ok(&st, cert.as_bytes(), Some(&rows()));
    assert_eq!(field(&rec, &["rows", "verdict"]), "ROWS_NOT_INDEPENDENTLY_VERIFIED");
    assert_eq!(field(&rec, &["class"]), "NOT_WHOLE_DOMAIN");
    // with recovery but no generator: still not verified
    let rec = run_bytes_side(&st, cert.as_bytes(), None, &recover_side()).unwrap();
    assert_eq!(field(&rec, &["rows", "verdict"]), "ROWS_NOT_INDEPENDENTLY_VERIFIED");
    assert_eq!(field(&rec, &["class"]), "NOT_WHOLE_DOMAIN");
    // with recovery and the generator: rows verified, identity on the ORDINARY rows, whole domain
    let rec = run_bytes_side(&st, cert.as_bytes(), Some(&rows()), &recover_side()).unwrap();
    eprintln!("{rec}");
    assert_eq!(field(&rec, &["rows", "verdict"]), "ROWS_VERIFIED_BY_RECOVERY");
    assert_eq!(field(&rec, &["rows", "recovery", "recovered"]), "2");
    assert_eq!(field(&rec, &["identity", "verdict"]), "IDENTITY_EXACT");
    assert_eq!(field(&rec, &["binding", "verdict"]), "BINDING_EQUAL");
    assert_eq!(field(&rec, &["class"]), "WHOLE_DOMAIN");
}

#[test]
fn v0_rows_outside_the_ibp_span_are_not_verified() {
    // A_1 = O_1 + X and A_2 = X (X = I(n0+3,n1), not an IBP identity) with weights W_1 and -W_1: the carried-row identity
    // is EXACT, so without recovery only the rows stage stands between this certificate and a whole-domain class
    let carried = r#"[[["I(n0,n1)","d-2*n0"],["I(n0+1,n1)","n0"],["I(n0,n1-1)","1"]],[["I(n0,n1)","n1"],["I(n0-1,n1+1)","n0-1"],["I(n0+3,n1)","1"]],[["I(n0+3,n1)","1"]]]"#;
    let cert = v0_rows_cert(
        r#"[{"basis_row":0,"seed":"I(n0,n1)"},{"basis_row":1,"seed":"I(n0,n1)"},{"basis_row":2,"seed":"I(n0,n1)"}]"#,
        r#"[{"j":0,"w":"1/(d-2*n0)"},{"j":1,"w":"1/(n0-1)"},{"j":2,"w":"-1/(n0-1)"}]"#,
        carried,
    );
    let st = stored(RHS_OK, r#"[["-1+n0"], ["-2+n0"]]"#, "");
    let rec = run_bytes_side(&st, cert.as_bytes(), Some(&rows()), &recover_side()).unwrap();
    eprintln!("{rec}");
    assert_eq!(field(&rec, &["rows", "verdict"]), "ROWS_NOT_INDEPENDENTLY_VERIFIED");
    assert_eq!(field(&rec, &["rows", "recovery", "recovered"]), "1");
    assert_eq!(field(&rec, &["rows", "recovery", "failed"]), "2");
    assert_eq!(field(&rec, &["identity", "verdict"]), "IDENTITY_EXACT");
    assert_eq!(field(&rec, &["class"]), "NOT_WHOLE_DOMAIN");
}

#[test]
fn recovered_combination_denominators_join_the_factor_list() {
    // A_0 = O_0 / (n0-3) with w_0 = (n0-3)/(d-2n0): the combination has a pole at raw n0 = 3, i.e. stored n0 = 4 (the
    // factors are reported in the stored frame, translation n = m + (-1, 0)), inside the domain -> OPEN
    let carried = r#"[[["I(n0,n1)","(d-2*n0)/(n0-3)"],["I(n0+1,n1)","n0/(n0-3)"],["I(n0,n1-1)","1/(n0-3)"]],[["I(n0,n1)","n1"],["I(n0-1,n1+1)","n0-1"]]]"#;
    let cert = v0_rows_cert(
        r#"[{"basis_row":0,"seed":"I(n0,n1)"},{"basis_row":1,"seed":"I(n0,n1)"}]"#,
        r#"[{"j":0,"w":"(n0-3)/(d-2*n0)"},{"j":1,"w":"1/(n0-1)"}]"#,
        carried,
    );
    let st = stored(RHS_OK, r#"[["-1+n0"], ["-2+n0"]]"#, "");
    let rec = run_bytes_side(&st, cert.as_bytes(), Some(&rows()), &recover_side()).unwrap();
    eprintln!("{rec}");
    assert_eq!(field(&rec, &["rows", "verdict"]), "ROWS_VERIFIED_BY_RECOVERY");
    assert_eq!(field(&rec, &["identity", "verdict"]), "IDENTITY_EXACT");
    assert_eq!(verdict_of(&rec, "-4+n0"), "OPEN");
    assert_eq!(field(&rec, &["class"]), "OPEN_LOCI");
}

// ---------------------------------------------------------------- real certificates (test data, env)

fn env_path(k: &str) -> Option<String> {
    std::env::var(k)
        .ok()
        .filter(|p| std::path::Path::new(p).exists())
}

/// (rule index, table offset in the given table, certificate path relative to RUSTRED_WCERT_DATA)
fn real_case(table_var: &str, index: i64, offset: u64, cert_rel: &str) -> Option<String> {
    let (Some(table), Some(data)) = (env_path(table_var), env_path("RUSTRED_WCERT_DATA")) else {
        eprintln!(
            "SKIP: {table_var} / RUSTRED_WCERT_DATA not set or absent (real-certificate test data)"
        );
        return None;
    };
    let args = WcertArgs {
        cert: format!("{data}/{cert_rel}"),
        table,
        rule: index,
        offset: Some(offset),
        ..Default::default()
    };
    let rec = run(&args, &|| None).unwrap_or_else(|e| panic!("refused: {e}"));
    eprintln!("{rec}");
    Some(rec)
}

fn assert_v0_real(rec: &str) {
    assert_eq!(
        field(rec, &["rows", "verdict"]),
        "ROWS_NOT_INDEPENDENTLY_VERIFIED"
    );
    assert_eq!(
        field(rec, &["identity", "verdict"]),
        "IDENTITY_NOT_CHECKABLE"
    );
    assert_eq!(field(rec, &["binding", "verdict"]), "BINDING_EQUAL");
    assert_eq!(field(rec, &["class"]), "NOT_WHOLE_DOMAIN");
}

// T162-wcert-1 positions 987 / 988 / 989 = pieces R_a / R_b / R_c (offsets of that table, sha256 b6845248...)
#[test]
fn real_r_a() {
    if let Some(r) = real_case(
        "RUSTRED_WCERT_T1",
        987,
        36212742,
        "L/987/p0/cert/cert-af536e8cb6bde96b.json",
    ) {
        assert_v0_real(&r);
        assert_eq!(field(&r, &["conditional_class"]), "WHOLE_DOMAIN_IF_ROWS");
    }
}
#[test]
fn real_r_b() {
    if let Some(r) = real_case(
        "RUSTRED_WCERT_T1",
        988,
        36241859,
        "L/987/p1/cert/cert-eeced1f07b14fd04.json",
    ) {
        assert_v0_real(&r);
        assert_eq!(field(&r, &["conditional_class"]), "WHOLE_DOMAIN_IF_ROWS");
    }
}
#[test]
fn real_r_c() {
    if let Some(r) = real_case(
        "RUSTRED_WCERT_T1",
        989,
        36320551,
        "L/987/p2/cert/cert-a2be7f357fccf059.json",
    ) {
        assert_v0_real(&r);
        assert_eq!(field(&r, &["conditional_class"]), "WHOLE_DOMAIN_IF_ROWS");
    }
}
// T162.cand.table.jsonl (sha256 a9c34170...) rules 1260, 1359, 1610
#[test]
fn real_1260() {
    if let Some(r) = real_case(
        "RUSTRED_WCERT_TABLE",
        1260,
        407684564,
        "L3/1260/p0/cert/cert-6d4848e74045bb10.json",
    ) {
        assert_v0_real(&r);
    }
}
#[test]
fn real_1359() {
    if let Some(r) = real_case(
        "RUSTRED_WCERT_TABLE",
        1359,
        1398496378,
        "L3/1359/p0/cert/cert-ce74c2e423c694d2.json",
    ) {
        assert_v0_real(&r);
    }
}
#[test]
fn real_1610() {
    if let Some(r) = real_case(
        "RUSTRED_WCERT_TABLE",
        1610,
        3996207455,
        "L3/1610/p0/cert/cert-efef896d1de0f5bb.json",
    ) {
        assert_v0_real(&r);
    }
}

// ---------------------------------------------------------------- binding modulo zero sectors and symmetry

fn side_file(name: &str, text: &str) -> String {
    let dir = std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".into());
    let p = format!("{dir}/wcert-test-{}-{name}", std::process::id());
    std::fs::write(&p, text).unwrap();
    p
}

#[test]
fn binding_modulo_zero_sectors_and_symmetry() {
    // row: target I(n0,1,0); I(n0,0,0) lies in the zero sector "100"; I(n0,2,0) and I(n0,0,2) are one class under the
    // slot swap 1 <-> 2 (representative I(n0,0,2)); the stored rule keeps only the class sum
    let row =
        r#"[["I(n0,1,0)","1"],["I(n0,0,0)","1/(n0-1)"],["I(n0,2,0)","3"],["I(n0,0,2)","n0"]]"#;
    let cert = format!(
        r#"{{"schema":"rustred.weight-cert.v0","case":"toy3","target":"I(n0,1,0)","sources":[],"weights":[],"row":{row}}}"#
    );
    let stored = |c: &str| {
        format!(
            r#"{{"type":"rule","chart":"110","index":0,"case":"toy3","fixed":[null,1,0],"equations":[],"target":"I(n0,1,0)","rhs":[["I(n0,0,2)","{c}"]],"exceptions":[]}}"#
        )
        .into_bytes()
    };
    let zs = side_file(
        "zero",
        r#"{"type":"zero_sectors","scope":"downset","masks":["000","100"]}"#,
    );
    let sg = side_file(
        "sym",
        r#"{"type":"symmetry_group","chart":"110","group":{"elements":[{"action":[[0,false],[1,false],[2,false]]},{"action":[[0,false],[2,false],[1,false]]}]}}"#,
    );
    let side = Side {
        zero_sectors: Some(zs.clone()),
        symmetry_group: Some(sg.clone()),
        ..Default::default()
    };
    // without the records: the integral sets differ
    let rec = run_ok(&stored("-3-n0"), cert.as_bytes(), None);
    assert_eq!(field(&rec, &["binding", "verdict"]), "BINDING_TERMS_DIFFER");
    // with them: equal modulo
    let rec = run_bytes_side(&stored("-3-n0"), cert.as_bytes(), None, &side).unwrap();
    eprintln!("{rec}");
    assert_eq!(field(&rec, &["binding", "verdict"]), "BINDING_EQUAL_MODULO");
    assert_eq!(field(&rec, &["class"]), "NOT_WHOLE_DOMAIN");
    // a wrong class sum is still caught
    let rec = run_bytes_side(&stored("-3-2*n0"), cert.as_bytes(), None, &side).unwrap();
    assert_eq!(field(&rec, &["binding", "verdict"]), "BINDING_TERMS_DIFFER");
    assert_eq!(field(&rec, &["class"]), "FAIL");
    // the zero-sector drop needs the domain: I(n0-1,0,0) has n0 - 1 >= 0 only (n0 >= 1), its sector is not decided
    let cert2 = cert.replace(
        r#"["I(n0,0,0)","1/(n0-1)"]"#,
        r#"["I(n0-1,0,0)","1/(n0-1)"]"#,
    );
    let rec = run_bytes_side(&stored("-3-n0"), cert2.as_bytes(), None, &side).unwrap();
    assert_eq!(field(&rec, &["binding", "verdict"]), "BINDING_TERMS_DIFFER");
    let sg010 = side_file("unused", "{}");
    for p in [zs, sg, sg010] {
        let _ = std::fs::remove_file(p);
    }
}
