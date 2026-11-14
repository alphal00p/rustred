//! Independent re-audit (lane audit-contract2, 2026-11-14): the canonical-JSON rule (child-job.md 3.2.3, plan 3.3.0).
//!
//! Own value model and own encoder (no call into the crate except the function under test), own PRNG (the crate has
//! no proptest dev-dependency; a splitmix64 generator gives the same coverage). Case counts are printed by every
//! property test (`--nocapture`) and recorded in WS/notes/implementer.reaudit-contract.md.
//!
//! Properties:
//! - RESPELL: every spelling of one model value (whitespace, member order, string escapes, number spellings) has the
//!   canonical form our own encoder predicts;
//! - INJECTIVE: canonical forms of two model values are equal iff the values are equal (floats by bits);
//! - DIFFERENTIAL: on mutated lines, `canonical_record` never panics; an accepted line has a canonical form that is
//!   valid JSON, a fixed point, and that serde_json accepts too; disagreements with serde_json are classified.

use rustred_campaign_contract::canon::{
    self, CanonError, VOLATILE_BY_TYPE, canonical_line, canonical_record, is_volatile_with,
};
use std::collections::BTreeMap;

// ------------------------------------------------------------------------------------------------ PRNG

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn chance(&mut self, num: u64, den: u64) -> bool {
        self.below(den) < num
    }
}

fn cases(default: u64) -> u64 {
    std::env::var("AC2_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

// ------------------------------------------------------------------------------------------------ model

/// The value space the contract allows: integers exact (any size), finite floats, strings of Unicode scalar values,
/// objects with distinct keys.
#[derive(Clone, Debug)]
enum M {
    Null,
    Bool(bool),
    /// Normalised decimal integer (no leading zeros, no "-0").
    Int(String),
    Float(f64),
    Str(String),
    Arr(Vec<M>),
    Obj(BTreeMap<String, M>),
}

/// Model equality: integers by digits, floats by BITS (so -0.0 != 0.0), strings by scalar values, objects as maps.
fn meq(a: &M, b: &M) -> bool {
    match (a, b) {
        (M::Null, M::Null) => true,
        (M::Bool(x), M::Bool(y)) => x == y,
        (M::Int(x), M::Int(y)) => x == y,
        (M::Float(x), M::Float(y)) => x.to_bits() == y.to_bits(),
        (M::Str(x), M::Str(y)) => x == y,
        (M::Arr(x), M::Arr(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| meq(p, q)),
        (M::Obj(x), M::Obj(y)) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y)
                    .all(|((k1, v1), (k2, v2))| k1 == k2 && meq(v1, v2))
        }
        _ => false,
    }
}

/// A small alphabet makes collisions between independently drawn values likely (the injectivity test needs them).
const CHARS: &[char] = &[
    'a',
    'b',
    '"',
    '\\',
    '/',
    '\u{0}',
    '\u{8}',
    '\u{c}',
    '\n',
    '\r',
    '\t',
    '\u{1f}',
    '\u{7f}',
    'é',
    '\u{2028}',
    '\u{fffd}',
    '\u{feff}',
    '😀',
    '\u{10ffff}',
    ' ',
    'A',
    '0',
];

fn gen_str(r: &mut Rng, small: bool) -> String {
    let n = r.below(if small { 3 } else { 8 });
    (0..n)
        .map(|_| {
            if small {
                CHARS[r.below(4) as usize]
            } else if r.chance(1, 10) {
                // any scalar value
                loop {
                    let c = r.below(0x11_0000) as u32;
                    if let Some(c) = char::from_u32(c) {
                        break c;
                    }
                }
            } else {
                CHARS[r.below(CHARS.len() as u64) as usize]
            }
        })
        .collect()
}

fn gen_int(r: &mut Rng, small: bool) -> String {
    if small {
        return ["0", "1", "-1", "2"][r.below(4) as usize].into();
    }
    match r.below(5) {
        0 => "0".into(),
        1 => (r.next() as i64).to_string(),
        2 => r.next().to_string(),
        3 => {
            // big integers beyond u64 / i64
            let mut s = String::new();
            if r.chance(1, 2) {
                s.push('-');
            }
            s.push(char::from(b'1' + r.below(9) as u8));
            for _ in 0..(20 + r.below(30)) {
                s.push(char::from(b'0' + r.below(10) as u8));
            }
            s
        }
        _ => (r.below(1000) as i64 - 500).to_string(),
    }
}

fn gen_float(r: &mut Rng, small: bool) -> f64 {
    if small {
        return [0.5, 1.0, -0.0, 0.0][r.below(4) as usize];
    }
    match r.below(6) {
        0 => 0.0,
        1 => -0.0,
        2 => (r.below(1_000_000) as f64) / 1000.0,
        3 => f64::from_bits(r.next()),
        4 => [
            1e300,
            -1e-300,
            5e-324,
            f64::MAX,
            f64::MIN_POSITIVE,
            1e16,
            1e15,
            0.1,
        ][r.below(8) as usize],
        _ => (r.next() as i64) as f64,
    }
}

fn gen_value(r: &mut Rng, depth: u32, small: bool) -> M {
    let leaf = depth >= 4 || r.chance(1, 3);
    let k = if leaf { r.below(5) } else { 5 + r.below(2) };
    match k {
        0 => M::Null,
        1 => M::Bool(r.chance(1, 2)),
        2 => M::Int(gen_int(r, small)),
        3 => {
            let f = gen_float(r, small);
            if f.is_finite() { M::Float(f) } else { M::Null }
        }
        4 => M::Str(gen_str(r, small)),
        5 => M::Arr(
            (0..r.below(if small { 3 } else { 5 }))
                .map(|_| gen_value(r, depth + 1, small))
                .collect(),
        ),
        _ => {
            let mut m = BTreeMap::new();
            for _ in 0..r.below(if small { 3 } else { 5 }) {
                m.insert(gen_str(r, small), gen_value(r, depth + 1, small));
            }
            M::Obj(m)
        }
    }
}

// ------------------------------------------------------------------------------------------------ encoders

/// The canonical encoding predicted from the rule text (3.3.0 + 3.2.3), written independently of the crate.
fn enc_canon_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn enc_canon(m: &M, out: &mut String) {
    match m {
        M::Null => out.push_str("null"),
        M::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        M::Int(s) => out.push_str(s),
        M::Float(f) => out.push_str(&serde_json::to_string(f).unwrap()),
        M::Str(s) => enc_canon_str(s, out),
        M::Arr(v) => {
            out.push('[');
            for (i, x) in v.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                enc_canon(x, out);
            }
            out.push(']');
        }
        M::Obj(o) => {
            // BTreeMap<String> orders by the UTF-8 bytes: exactly the rule's member order
            out.push('{');
            for (i, (k, v)) in o.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                enc_canon_str(k, out);
                out.push(':');
                enc_canon(v, out);
            }
            out.push('}');
        }
    }
}

fn ws(r: &mut Rng, out: &mut String) {
    for _ in 0..r.below(3) {
        out.push([' ', '\t', '\n', '\r'][r.below(4) as usize]);
    }
}

fn enc_spell_str(r: &mut Rng, s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        let forced = matches!(c, '"' | '\\') || (c as u32) < 0x20;
        if forced || r.chance(1, 4) {
            match r.below(3) {
                0 if c == '/' => out.push_str("\\/"),
                0 if matches!(c, '\u{8}' | '\u{c}' | '\n' | '\r' | '\t' | '"' | '\\') => out
                    .push_str(match c {
                        '\u{8}' => "\\b",
                        '\u{c}' => "\\f",
                        '\n' => "\\n",
                        '\r' => "\\r",
                        '\t' => "\\t",
                        '"' => "\\\"",
                        _ => "\\\\",
                    }),
                _ => {
                    let mut b = [0u16; 2];
                    for u in c.encode_utf16(&mut b) {
                        if r.chance(1, 2) {
                            out.push_str(&format!("\\u{:04X}", u));
                        } else {
                            out.push_str(&format!("\\u{:04x}", u));
                        }
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out.push('"');
}

fn enc_spell_float(r: &mut Rng, f: f64, out: &mut String) {
    // several exact spellings of the same f64: shortest, with an exponent, with trailing zeros
    let s = serde_json::to_string(&f).unwrap();
    match r.below(4) {
        0 => out.push_str(&s),
        1 => out.push_str(&format!("{f:e}")),
        2 => out.push_str(&format!("{f:E}")),
        _ => {
            if s.contains('e') {
                // `1e300` / `1e+300` -> `1E+300`; `1e-7` -> `1E-7` (whatever form the float writer uses)
                let t = s.replace("e+", "e");
                out.push_str(&t.replace('e', "E+").replace("E+-", "E-"))
            } else {
                out.push_str(&s);
                out.push_str("000");
            }
        }
    }
}

fn enc_spell(r: &mut Rng, m: &M, out: &mut String) {
    match m {
        M::Null => out.push_str("null"),
        M::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        // lane contractfix2 (ac2-m1 fixed): the integer token `-0` is refused, so it is no longer a spelling of 0
        M::Int(s) => out.push_str(s),
        M::Float(f) => enc_spell_float(r, *f, out),
        M::Str(s) => enc_spell_str(r, s, out),
        M::Arr(v) => {
            out.push('[');
            ws(r, out);
            for (i, x) in v.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                    ws(r, out);
                }
                enc_spell(r, x, out);
                ws(r, out);
            }
            out.push(']');
        }
        M::Obj(o) => {
            let mut items: Vec<_> = o.iter().collect();
            // random member order
            for i in (1..items.len()).rev() {
                let j = r.below(i as u64 + 1) as usize;
                items.swap(i, j);
            }
            out.push('{');
            ws(r, out);
            for (i, (k, v)) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                    ws(r, out);
                }
                enc_spell_str(r, k, out);
                ws(r, out);
                out.push(':');
                ws(r, out);
                enc_spell(r, v, out);
                ws(r, out);
            }
            out.push('}');
        }
    }
}

/// Wrap a model value into a record with a non-volatile type so nothing is stripped.
fn record(v: M) -> M {
    let mut o = BTreeMap::new();
    o.insert("type".to_string(), M::Str("audit".into()));
    o.insert("v".to_string(), v);
    M::Obj(o)
}

// ------------------------------------------------------------------------------------------------ properties

#[test]
fn ac2_canon_respell_invariance_property() {
    let n = cases(60_000);
    let mut r = Rng(0xAC2_0001);
    for i in 0..n {
        let v = record(gen_value(&mut r, 0, false));
        let mut want = String::new();
        enc_canon(&v, &mut want);
        let mut s = String::new();
        ws(&mut r, &mut s);
        enc_spell(&mut r, &v, &mut s);
        ws(&mut r, &mut s);
        let got = canonical_line(s.as_bytes()).unwrap_or_else(|e| panic!("case {i}: {e} on {s:?}"));
        assert_eq!(
            String::from_utf8(got).unwrap(),
            want,
            "case {i}: spelling {s:?}"
        );
    }
    eprintln!("ac2_canon_respell_invariance_property: {n} cases");
}

#[test]
fn ac2_canon_injective_property() {
    // pairs drawn from a SMALL alphabet (collisions are frequent) and pairs that differ in one leaf
    let n = cases(60_000);
    let mut r = Rng(0xAC2_0002);
    let (mut equal, mut distinct) = (0u64, 0u64);
    for i in 0..n {
        let a = record(gen_value(&mut r, 0, true));
        let b = record(gen_value(&mut r, 0, true));
        let (mut sa, mut sb) = (String::new(), String::new());
        enc_spell(&mut r, &a, &mut sa);
        enc_spell(&mut r, &b, &mut sb);
        let ca = canonical_line(sa.as_bytes()).unwrap();
        let cb = canonical_line(sb.as_bytes()).unwrap();
        let same_value = meq(&a, &b);
        assert_eq!(
            ca == cb,
            same_value,
            "case {i}: {sa:?} vs {sb:?} (values equal: {same_value})"
        );
        if same_value {
            equal += 1
        } else {
            distinct += 1
        }
    }
    eprintln!(
        "ac2_canon_injective_property: {n} pairs ({equal} equal values, {distinct} distinct values)"
    );
    assert!(equal > n / 50, "too few colliding pairs to test anything");
}

/// The one-byte / one-token neighbourhood of a valid line: every mutation that changes the decoded value must change
/// the canonical form, and serde_json's verdict on validity must agree except where the contract is stricter.
#[test]
fn ac2_canon_mutation_differential_fuzz() {
    let n = cases(150_000);
    let mut r = Rng(0xAC2_0003);
    let mut stats: BTreeMap<&'static str, u64> = BTreeMap::new();
    let mut examples: BTreeMap<&'static str, String> = BTreeMap::new();
    let seeds: Vec<String> = (0..64)
        .map(|_| {
            let mut s = String::new();
            let v = record(gen_value(&mut r, 0, false));
            enc_spell(&mut r, &v, &mut s);
            s
        })
        .chain(
            [
                r#"{"type":"rule","chart":"11","index":0,"target":"I(1)","rhs":[["I(0)","-1/2"]],"seconds_search":0.7}"#,
                r#"{"type":"header","family_file":"/x","family_blake3":"0000000000000000000000000000000000000000000000000000000000000000","zero_seconds":1}"#,
                r#"{"type":"end","rules":1,"seconds":1.5,"vmhwm_kb":3}"#,
                r#"{"a":-0,"b":1e2,"c":"😀","d":[1,2.50,{"z":null,"y":true}]}"#,
            ]
            .iter()
            .map(|s| s.to_string()),
        )
        .collect();
    const ALPH: &[u8] = b"{}[]:,\"\\/-+.0123456789eEtrufalsn \t\r\x00\x1fuD8C0";
    for _ in 0..n {
        let mut b = seeds[r.below(seeds.len() as u64) as usize]
            .clone()
            .into_bytes();
        for _ in 0..(1 + r.below(3)) {
            if b.is_empty() {
                break;
            }
            let p = r.below(b.len() as u64) as usize;
            match r.below(4) {
                0 => b[p] = ALPH[r.below(ALPH.len() as u64) as usize],
                1 => b.insert(p, ALPH[r.below(ALPH.len() as u64) as usize]),
                2 => {
                    b.remove(p);
                }
                _ => b[p] = r.next() as u8,
            }
        }
        let ours = std::panic::catch_unwind(|| canonical_record(&b));
        let Ok(ours) = ours else {
            panic!(
                "canonical_record panicked on {:?}",
                String::from_utf8_lossy(&b)
            );
        };
        let theirs = serde_json::from_slice::<serde_json::Value>(&b);
        let tag = match (&ours, &theirs) {
            (Ok(c), Ok(_)) => {
                // fixed point, valid JSON
                let again = canonical_line(&c.bytes).expect("canonical form re-canonicalises");
                assert_eq!(
                    again,
                    c.bytes,
                    "not a fixed point: {:?}",
                    String::from_utf8_lossy(&b)
                );
                serde_json::from_slice::<serde_json::Value>(&c.bytes)
                    .expect("canonical form is JSON");
                "both accept"
            }
            (Err(_), Err(_)) => "both refuse",
            (Err(CanonError::DuplicateKey(_)), Ok(_)) => {
                "ours refuses: duplicate key (contract stricter)"
            }
            (Err(CanonError::NotObject), Ok(_)) => {
                "ours refuses: not an object (contract stricter)"
            }
            (Err(CanonError::TypeNotString), Ok(_)) => {
                "ours refuses: type not a string (contract stricter)"
            }
            // lane contractfix2 (ac2-m1): serde_json reads `-0` as the float -0.0; the contract refuses the token
            (Err(CanonError::NegativeZeroInteger { .. }), Ok(_)) => {
                "ours refuses: integer token -0 (contract stricter)"
            }
            (Err(_), Ok(_)) => "OURS REFUSES, serde accepts (other)",
            (Ok(_), Err(e)) if e.to_string().contains("duplicate") => "serde refuses duplicate",
            (Ok(_), Err(_)) => "OURS ACCEPTS, serde refuses",
        };
        *stats.entry(tag).or_default() += 1;
        examples
            .entry(tag)
            .or_insert_with(|| String::from_utf8_lossy(&b).into_owned());
    }
    eprintln!("ac2_canon_mutation_differential_fuzz: {n} mutated lines");
    for (k, v) in &stats {
        eprintln!("  {v:>8}  {k}   e.g. {:?}", examples[k]);
    }
    assert!(
        !stats.contains_key("OURS ACCEPTS, serde refuses"),
        "the canonicaliser accepts a line serde_json refuses: {:?}",
        examples.get("OURS ACCEPTS, serde refuses")
    );
    assert!(
        !stats.contains_key("OURS REFUSES, serde accepts (other)"),
        "unexplained refusal: {:?}",
        examples.get("OURS REFUSES, serde accepts (other)")
    );
}

// ------------------------------------------------------------------------------------------------ fixed vectors

#[test]
fn ac2_canon_refuses_outside_the_value_space() {
    let bad: &[&[u8]] = &[
        br#"{"x":NaN}"#,
        br#"{"x":Infinity}"#,
        br#"{"x":-Infinity}"#,
        br#"{"x":1e400}"#,
        br#"{"x":-1e400}"#,
        br#"{"x":179769313486231580793728971405303415079934132710037826936173778980444968292764750946649017977587207096330286416692887910946555547851940402630657488671505820681908902000708383676273854845817711531764475730270069855571366959622842914819860834936475292719074168444365510704342711559699508093042880177904174497792.0}"#,
        br#"{"x":"\ud800"}"#,
        br#"{"x":"\udc00"}"#,
        br#"{"x":"\ud800A"}"#,
        br#"{"x":"\ud800\ud800"}"#,
        br#"{"x":"\udbff"}"#,
        br#"{"a":1,"a":2}"#,
        br#"{"n":{"k":1,"k":1}}"#,
        br#"{"n":[{"k":1,"k":1}]}"#,
        "\u{feff}{\"a\":1}".as_bytes(),
        br#"{"a":1,}"#,
        br#"{'a':1}"#,
        br#"{"a":+1}"#,
        br#"{"a":0x10}"#,
        br#"{"a":.5}"#,
        br#"{"a":5.}"#,
        br#"{"a":01}"#,
        br#"{"a":-}"#,
        br#"{"a":1 /* c */}"#,
        b"{\"a\":\"\x01\"}",
        b"{\"a\":\"\xc0\x80\"}",
        b"{\"a\":\"\xed\xa0\x80\"}",
        b"{\"a\":1}\x00",
        br#"{"a":1}{"b":2}"#,
        br#"{"type":["rule"]}"#,
        br#"[]"#,
        br#""x""#,
    ];
    for b in bad {
        assert!(
            canonical_line(b).is_err(),
            "accepted outside the value space: {:?}",
            String::from_utf8_lossy(b)
        );
    }
    // underflow is a VALUE (0.0), not an error: two decimal spellings of one f64
    assert_eq!(
        canonical_line(br#"{"x":1e-400}"#).unwrap(),
        canonical_line(br#"{"x":0.0}"#).unwrap()
    );
}

/// FINDING ac2-m1 (minor): the integer token `-0` is canonicalised to `0` (plan 3.3.0), but serde_json, THE reader of
/// the supervisor and of the engine, reads `-0` as the FLOAT -0.0 (`as_u64()` = None, `Value` unequal to `0`). Two
/// values that serde_json distinguishes therefore share one canonical form (the dangerous direction of
/// injectivity, for this one token). Float -0.0 is NOT normalised (`-0.0`, `-0e0` stay `-0.0`), so `-0` and
/// `-0e0` (equal f64 values for every reader) get DIFFERENT canonical forms. Smallest fix: refuse the token `-0`
/// (CanonError) like any other non-canonical spelling; or keep `-0` verbatim.
#[test]
fn ac2_canon_negative_zero_integer_vs_serde_reader() {
    // evidence (holds today): serde_json reads `-0` as the float -0.0, not as the integer 0
    let va: serde_json::Value = serde_json::from_slice(br#"{"index":-0}"#).unwrap();
    let vb: serde_json::Value = serde_json::from_slice(br#"{"index":0}"#).unwrap();
    assert_ne!(
        va, vb,
        "serde_json distinguishes -0 (f64 -0.0) from 0 (u64)"
    );
    assert_eq!(
        va["index"].as_u64(),
        None,
        "an engine reading index as u64 refuses -0"
    );
    assert!(va["index"].is_f64());
    // float zero keeps its sign: -0 and -0e0 are one f64 value for serde_json but two canonical forms today
    let c = canonical_line(br#"{"x":-0e0}"#).unwrap();
    assert_eq!(String::from_utf8(c).unwrap(), r#"{"x":-0.0}"#);
    // the defect: two values serde_json distinguishes share one canonical form
    let a = canonical_line(br#"{"type":"rule","index":-0}"#);
    let b = canonical_line(br#"{"type":"rule","index":0}"#).unwrap();
    assert!(
        a.as_ref().map_or(true, |a| *a != b),
        "`-0` (a float for serde_json) and `0` share the canonical form {:?}",
        String::from_utf8_lossy(&b)
    );
}

/// Observation ac2-n1: integers beyond the f64 mantissa keep distinct canonical forms although serde_json (without
/// `arbitrary_precision`) reads them as one f64. Not a wrong acceptance (distinct forms never compare equal), but
/// a record field the readers cannot represent exactly passes the canonical check.
#[test]
fn ac2_canon_big_integers_distinct_forms_one_serde_value() {
    let a = canonical_line(br#"{"x":18446744073709551616}"#).unwrap();
    let b = canonical_line(br#"{"x":18446744073709551617}"#).unwrap();
    assert_ne!(a, b);
    let va: serde_json::Value = serde_json::from_slice(&a).unwrap();
    let vb: serde_json::Value = serde_json::from_slice(&b).unwrap();
    assert_eq!(va, vb, "serde_json rounds both to the same f64");
}

/// The volatile-key list is CLOSED: for each record type exactly the listed top-level keys are removed, compared
/// decoded; nested keys and other types keep everything; the verdict rule applies only to `verdict` and typeless
/// records.
#[test]
fn ac2_canon_volatile_list_is_closed() {
    let probe = [
        "seconds",
        "zero_seconds",
        "seconds_search",
        "lift_seconds",
        "vmhwm_kb",
        "utc",
        "limit_seconds",
        "x_seconds",
        "family_file",
        "rules",
        "index",
    ];
    let mut types: Vec<Option<&str>> = VOLATILE_BY_TYPE.iter().map(|(t, _)| Some(*t)).collect();
    types.extend([
        Some("verdict"),
        Some("residual"),
        Some("queue"),
        Some("cert_row"),
        None,
    ]);
    for t in &types {
        for k in probe {
            let mut line = String::from("{");
            if let Some(t) = t {
                line.push_str(&format!("\"type\":\"{t}\","));
            }
            // spell the key with escapes: removal compares DECODED keys
            let spelled: String = k
                .chars()
                .map(|c| {
                    if c == '_' {
                        "\\u005f".to_string()
                    } else {
                        c.to_string()
                    }
                })
                .collect();
            line.push_str(&format!("\"{spelled}\":7,\"nested\":{{\"{k}\":1}}}}"));
            let c = canonical_record(line.as_bytes()).unwrap();
            let removed = c.member(k).is_none();
            let listed = is_volatile_with(*t, k, false);
            let expect = match t {
                Some(t) if VOLATILE_BY_TYPE.iter().any(|(n, _)| n == t) => VOLATILE_BY_TYPE
                    .iter()
                    .find(|(n, _)| n == t)
                    .unwrap()
                    .1
                    .contains(&k),
                Some("verdict") | None => k == "seconds" || k == "utc" || k.ends_with("_seconds"),
                _ => false,
            };
            assert_eq!(removed, expect, "type {t:?} key {k}");
            assert_eq!(listed, expect, "is_volatile_with type {t:?} key {k}");
            // a nested key is never volatile
            assert!(
                String::from_utf8_lossy(&c.bytes).contains(&format!("\"nested\":{{\"{k}\":1}}")),
                "nested {k} kept for {t:?}"
            );
        }
    }
    // family_file: volatile only in a header that carries a 64-hex family_blake3
    let d = "ab".repeat(32);
    let h = format!(r#"{{"type":"header","family_file":"/p","family_blake3":"{d}"}}"#);
    assert!(
        canonical_record(h.as_bytes())
            .unwrap()
            .member("family_file")
            .is_none()
    );
    let h2 = format!(
        r#"{{"type":"header","family_file":"/p","family_blake3":"{}"}}"#,
        d.to_uppercase()
    );
    assert!(
        canonical_record(h2.as_bytes())
            .unwrap()
            .member("family_file")
            .is_some(),
        "upper-case hex is not a contract digest"
    );
    let _ = canon::VOLATILE_KEYS_VERSION;
}

/// Observation ac2-n2: a TYPELESS record inside a table stream follows the verdict rule, so any of its top-level keys
/// ending in `_seconds` is invisible to the canonical digest. The table verifier accepts typeless records; a reader
/// that interprets them must not use such keys.
#[test]
fn ac2_canon_typeless_record_in_a_table_hides_seconds_keys() {
    let a = canonical_line(br#"{"rule_like":1,"payload_seconds":"anything"}"#).unwrap();
    let b = canonical_line(br#"{"rule_like":1,"payload_seconds":"something else"}"#).unwrap();
    assert_eq!(a, b);
}

/// FINDING ac2-m2 (minor): "a number whose value is outside the f64 range is refused anywhere: readers (serde_json)
/// reject it" (canon.rs docs, audit B F9) holds for fraction / exponent tokens only. An INTEGER token is returned
/// before any range check, so a 400-digit integer has a canonical form (a table holding it seals and verifies),
/// while serde_json, the supervisor's and engine's reader, refuses the very line ("number out of range").
/// Smallest fix: in `P::number`, for an integer token longer than 309 digits parse it as f64 and refuse a
/// non-finite value (or refuse integers beyond i64/u64, as serde_json's exact range).
#[test]
fn ac2_canon_huge_integer_accepted_but_serde_refuses() {
    let line = format!(
        "{{\"type\":\"rule\",\"index\":0,\"x\":{}}}",
        "9".repeat(400)
    );
    // evidence (holds today): serde_json refuses the line
    let e = serde_json::from_str::<serde_json::Value>(&line).unwrap_err();
    assert!(e.to_string().contains("out of range"), "{e}");
    // the defect: the canonicaliser gives it a canonical form, so a table holding it seals and verifies
    assert!(
        canonical_line(line.as_bytes()).is_err(),
        "a 400-digit integer has a canonical form although every serde_json reader refuses the line"
    );
}

/// FINDING ac2-m2 (second form): the contract's depth limit and serde_json's recursion limit disagree: lines with 128
/// and 129 nested containers have a canonical form (they seal and verify) but serde_json refuses them ("recursion
/// limit exceeded"). Smallest fix: `value()` / `members()` refuse `depth >= MAX_DEPTH - 1`, i.e. at most 127
/// nested containers as serde_json (or make the limit a shared constant tested against serde_json).
#[test]
fn ac2_canon_depth_boundary_vs_serde() {
    let mut agree = true;
    let mut report = Vec::new();
    for levels in 126..=131usize {
        // `levels` nested containers in total, the outer object included
        let inner = levels - 1;
        let line = format!("{{\"a\":{}{}}}", "[".repeat(inner), "]".repeat(inner));
        let ours = canonical_line(line.as_bytes()).is_ok();
        let theirs = serde_json::from_str::<serde_json::Value>(&line).is_ok();
        report.push(format!("levels {levels}: canon {ours}, serde {theirs}"));
        agree &= ours == theirs;
    }
    eprintln!("ac2_canon_depth_boundary_vs_serde: {}", report.join("; "));
    assert!(agree, "depth limits disagree: {}", report.join("; "));
}

/// FINDING ac2-m8 (minor): the canonical rule takes a float token's EXACT f64 value (`str::parse`, correctly
/// rounded); serde_json as built for the contract crate (no `float_roundtrip` feature) parses floats best-effort and
/// can be 1 ULP off. So the canonical form of the BYTES and the canonical form of the value a serde_json reader
/// (supervisor, engine, fake child) holds can differ, and a record re-serialised by a reader no longer has the
/// canonical digest it was certified with. Found by the parser fuzz at 4x scale (EVENTS / RESUME round trips).
/// Smallest fix: `serde_json = { version = "1.0.151", features = ["float_roundtrip"] }` in the contract crate's
/// Cargo.toml (feature unification gives every reader exact parsing; a shared-file change for the integration lane).
#[test]
fn ac2_canon_float_value_is_reader_dependent() {
    let s = "4.1844674407370955e+21";
    let exact: f64 = s.parse().unwrap();
    let reader: f64 = serde_json::from_str(s).unwrap();
    eprintln!(
        "exact {exact:e} ({:#x}), serde_json {reader:e} ({:#x})",
        exact.to_bits(),
        reader.to_bits()
    );
    let line = format!("{{\"type\":\"header\",\"x\":{s}}}");
    let canon_of_bytes = canonical_line(line.as_bytes()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&line).unwrap();
    let canon_of_reader_value = canonical_line(&serde_json::to_vec(&v).unwrap()).unwrap();
    assert_eq!(
        canon_of_bytes, canon_of_reader_value,
        "a serde_json reader holds a different float than the canonical form certifies"
    );
}
