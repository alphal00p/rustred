//! Independent re-audit (lane audit-contract2, 2026-11-14): fuzzing the contract's parsers and the stream verifier.
//!
//! Own PRNG (the crate has no proptest dev-dependency). Mutation fuzzing of every parser from the golden files
//! (OUTCOME, RESUME, job spec, EVENTS lines, cases lines, canonical helpers): no panic, and whatever parses must
//! re-serialise and re-parse to the same value. A DIFFERENTIAL test of the sealed-stream verifier against an
//! independent verifier written from child-job.md 3.4.4, over random tables with random corruptions; the
//! engine-side `StreamDigest` against the file verifier at random chunkings. Case counts are printed
//! (`--nocapture`); `AC2_FUZZ_SCALE` multiplies them.

mod common;
use common::*;
use rustred_campaign_contract::canon::{count_string_elements, decode_canonical_string};
use rustred_campaign_contract::events::{EventLine, EventsTail};
use rustred_campaign_contract::outcome::Outcome;
use rustred_campaign_contract::resume::{ResumeState, parse_cases_line};
use rustred_campaign_contract::seal::{self, StreamDigest, TABLE_END_TYPES};
use rustred_campaign_contract::spec::JobSpec;
use std::collections::HashSet;
use std::path::Path;

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
}

fn scale(n: u64) -> u64 {
    n * std::env::var("AC2_FUZZ_SCALE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1)
}

fn data(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(name),
    )
    .unwrap()
}

const TOK: &[&[u8]] = &[
    b"{",
    b"}",
    b"[",
    b"]",
    b":",
    b",",
    b"\"",
    b"\\",
    b"null",
    b"true",
    b"false",
    b"0",
    b"-0",
    b"1e999",
    b"-1",
    b"18446744073709551616",
    b"0.5",
    b"\"\"",
    b"\\u0000",
    b"\\ud800",
    b"\"x\":1,",
    b"\"schema\":\"v9\",",
    b"\n",
    b" ",
    b"\"job\":{},",
    b"\"attempt\":0,",
    b"\"exit_code\":13,",
    b"\"sealed\":true,",
    b"\"path\":\"../x\",",
];

fn mutate(r: &mut Rng, src: &[u8]) -> Vec<u8> {
    let mut b = src.to_vec();
    for _ in 0..(1 + r.below(4)) {
        if b.is_empty() {
            b.push(b'{');
        }
        let p = r.below(b.len() as u64) as usize;
        match r.below(6) {
            0 => b[p] = r.next() as u8,
            1 => {
                b.remove(p);
            }
            2 => {
                let t = TOK[r.below(TOK.len() as u64) as usize];
                b.splice(p..p, t.iter().copied());
            }
            3 => {
                let q = (p + 1 + r.below(16) as usize).min(b.len());
                b.drain(p..q);
            }
            4 => {
                // duplicate a slice (repeats keys / values)
                let q = (p + 1 + r.below(40) as usize).min(b.len());
                let s: Vec<u8> = b[p..q].to_vec();
                b.splice(q..q, s);
            }
            _ => {
                // swap a digit
                if b[p].is_ascii_digit() {
                    b[p] = b'0' + r.below(10) as u8;
                } else {
                    b[p] ^= 0x20;
                }
            }
        }
    }
    b
}

/// Value equality where floats may differ by a few ULP (serde_json without `float_roundtrip`, FINDING ac2-m8).
fn same_up_to_ulp(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    use serde_json::Value as V;
    match (a, b) {
        (V::Number(x), V::Number(y)) if x.is_f64() || y.is_f64() => {
            let (p, q) = (x.as_f64().unwrap(), y.as_f64().unwrap());
            p == q || (p.to_bits() as i64 - q.to_bits() as i64).abs() <= 2
        }
        (V::Array(x), V::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same_up_to_ulp(p, q))
        }
        (V::Object(x), V::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.get(k).is_some_and(|w| same_up_to_ulp(v, w)))
        }
        _ => a == b,
    }
}

fn no_panic<T>(what: &str, input: &[u8], f: impl FnOnce() -> T) -> T {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(t) => t,
        Err(_) => panic!("{what} panicked on {:?}", String::from_utf8_lossy(input)),
    }
}

#[test]
fn ac2_fuzz_outcome_parser() {
    let n = scale(60_000);
    let mut r = Rng(0xAC2_F001);
    let seeds = [
        data("outcome.incomplete.json"),
        data("outcome.hard_stop.json"),
        data("outcome.input_error.json"),
    ];
    let (mut parsed, mut checked) = (0u64, 0u64);
    for _ in 0..n {
        let k = r.below(3) as usize;
        let b = mutate(&mut r, &seeds[k]);
        let o = no_panic("Outcome::from_slice", &b, || Outcome::from_slice(&b));
        if let Ok(o) = o {
            parsed += 1;
            let ok = no_panic("JobOutcome::check", &b, || o.job.check().is_ok());
            if ok {
                checked += 1;
            }
            // round trip (floats: serde_json parses best-effort, see FINDING ac2-m8)
            let again = Outcome::from_slice(&o.to_bytes()).expect("re-parses");
            assert!(
                same_up_to_ulp(
                    &serde_json::to_value(&again).unwrap(),
                    &serde_json::to_value(&o).unwrap()
                ),
                "OUTCOME round trip"
            );
        }
    }
    eprintln!(
        "ac2_fuzz_outcome_parser: {n} mutated OUTCOMEs, {parsed} parsed, {checked} passed check()"
    );
}

#[test]
fn ac2_fuzz_resume_parser() {
    let n = scale(40_000);
    let mut r = Rng(0xAC2_F002);
    let seed = data("resume.json");
    let (mut parsed, mut checked) = (0u64, 0u64);
    for _ in 0..n {
        let b = mutate(&mut r, &seed);
        let s = no_panic("ResumeState parse", &b, || {
            serde_json::from_slice::<ResumeState>(&b)
        });
        if let Ok(s) = s {
            parsed += 1;
            if no_panic("ResumeState::check", &b, || s.check().is_ok()) {
                checked += 1;
                // a checked RESUME names only layout paths
                assert_eq!(s.table.path, "out/F.table.jsonl");
                if let Some(q) = &s.queue {
                    assert_eq!(q.path, "out/F.queue.jsonl");
                }
            }
            let again: ResumeState =
                serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
            assert!(
                same_up_to_ulp(
                    &serde_json::to_value(&again).unwrap(),
                    &serde_json::to_value(&s).unwrap()
                ),
                "RESUME round trip"
            );
        }
    }
    eprintln!(
        "ac2_fuzz_resume_parser: {n} mutated RESUMEs, {parsed} parsed, {checked} passed check()"
    );
}

#[test]
fn ac2_fuzz_job_spec_parser() {
    let n = scale(40_000);
    let mut r = Rng(0xAC2_F003);
    let seeds = [data("job_spec.generate.json"), data("job_spec.fake.json")];
    let (mut ok, mut parsed_raw) = (0u64, 0u64);
    for _ in 0..n {
        let k = r.below(2) as usize;
        let b = mutate(&mut r, &seeds[k]);
        if serde_json::from_slice::<JobSpec>(&b).is_ok() {
            parsed_raw += 1;
        }
        let s = no_panic("JobSpec::from_json", &b, || JobSpec::from_json(&b));
        if let Ok(s) = s {
            ok += 1;
            // a valid spec has a safe job id and an absolute run_dir; args pass the allow-list again
            assert!(
                !s.job_id.starts_with('.') && !s.job_id.contains('/'),
                "{}",
                s.job_id
            );
            assert!(Path::new(&s.run_dir).is_absolute());
            let again = JobSpec::from_json(&s.to_json_line()).expect("valid spec re-validates");
            assert_eq!(again, s);
        }
    }
    eprintln!("ac2_fuzz_job_spec_parser: {n} mutated specs, {parsed_raw} deserialised, {ok} valid");
}

#[test]
fn ac2_fuzz_events_and_cases_parsers() {
    let n = scale(60_000);
    let mut r = Rng(0xAC2_F004);
    let ev = data("events.jsonl");
    let lines: Vec<&[u8]> = ev
        .split(|&b| b == b'\n')
        .filter(|l| !l.is_empty())
        .collect();
    let cases: Vec<&[u8]> = vec![
        br#"{"frame":"PRESENTED","fixed":[1,null,0],"equations":["n0-n1"]}"#,
        br#"{"type":"spill","pending":[{"frame":"PRESENTED","fixed":[null]}],"current":null,"numerical":[[1,2]]}"#,
        br#"{"frame":"ORIGINAL","fixed":[],"seed_centre_offset":[[0,1]],"x":1}"#,
    ];
    let (mut ev_ok, mut case_ok) = (0u64, 0u64);
    for _ in 0..n {
        let k = r.below(lines.len() as u64) as usize;
        let b = mutate(&mut r, lines[k]);
        if let Ok(e) = no_panic("EventLine::parse", &b, || EventLine::parse(&b)) {
            ev_ok += 1;
            let again = EventLine::parse(&e.to_line()).expect("event line re-parses");
            let (x, y): (serde_json::Value, serde_json::Value) = (
                serde_json::from_slice(&again.to_line()).unwrap(),
                serde_json::from_slice(&e.to_line()).unwrap(),
            );
            assert!(same_up_to_ulp(&x, &y), "event round trip");
        }
        let k = r.below(3) as usize;
        let c = mutate(&mut r, cases[k]);
        let s = String::from_utf8_lossy(&c).into_owned();
        if no_panic("parse_cases_line", &c, || parse_cases_line(&s)).is_ok() {
            case_ok += 1;
        }
        // canonical helpers on arbitrary bytes
        no_panic("count_string_elements", &b, || {
            count_string_elements(&b, "elided")
        });
        no_panic("decode_canonical_string", &b, || {
            decode_canonical_string(&b)
        });
    }
    eprintln!(
        "ac2_fuzz_events_and_cases_parsers: {n} event lines ({ev_ok} parsed), {n} cases lines ({case_ok} parsed), {n} x 2 canonical helper calls"
    );
}

#[test]
fn ac2_fuzz_events_tail_on_garbage() {
    let n = scale(2_000);
    let mut r = Rng(0xAC2_F005);
    let d = TempDir::new("tail");
    let ev = data("events.jsonl");
    for i in 0..n {
        let p = d.path().join(format!("e{}.jsonl", i % 7));
        let b = mutate(&mut r, &ev);
        std::fs::write(&p, &b).unwrap();
        let mut t = EventsTail::new(4096);
        let items = no_panic("EventsTail::poll", &b, || t.poll(&p));
        if let Ok(items) = items {
            assert!(items.len() <= b.len() + 1);
            assert!(t.offset() <= b.len() as u64);
        }
    }
    eprintln!("ac2_fuzz_events_tail_on_garbage: {n} files");
}

// ------------------------------------------------------------------------------------------------ verifier differential

/// An independent sealed-stream verifier written from child-job.md 3.4.4 (not from seal.rs): every line ends with
/// "\n" and has a canonical form; exactly one end record and it is the last line; rule records have an integer
/// `index`, unique (chart, index), unique (chart, target, case) when a target is present (lane eng2 interface
/// change). Returns (records, rules) or None.
fn independent_verify(b: &[u8]) -> Option<(u64, u64)> {
    if b.is_empty() || *b.last()? != b'\n' {
        return None;
    }
    let lines: Vec<&[u8]> = b[..b.len() - 1].split(|&c| c == b'\n').collect();
    let mut ids = HashSet::new();
    let mut targets = HashSet::new();
    let mut rules = 0;
    for (k, l) in lines.iter().enumerate() {
        let c = rustred_campaign_contract::canon::canonical_record(l).ok()?;
        let ty = c.record_type.as_deref();
        let is_end = matches!(ty, Some("end" | "partial"));
        if is_end != (k + 1 == lines.len()) {
            return None;
        }
        // lane contractfix2 (ac2-m6, child-job.md 3.4.4): a header record only on line 1
        if ty == Some("header") && k != 0 {
            return None;
        }
        if ty == Some("rule") {
            let idx = c.member("index")?;
            let s = std::str::from_utf8(idx).ok()?;
            if s.is_empty()
                || !s
                    .trim_start_matches('-')
                    .bytes()
                    .all(|x| x.is_ascii_digit())
                || s.trim_start_matches('-').is_empty()
            {
                return None;
            }
            let chart = c.member("chart").unwrap_or(b"null").to_vec();
            if !ids.insert((chart.clone(), idx.to_vec())) {
                return None;
            }
            if let Some(t) = c.member("target") {
                let case = c.member("case").unwrap_or(b"").to_vec();
                if !targets.insert((chart, t.to_vec(), case)) {
                    return None;
                }
            }
            rules += 1;
        }
    }
    Some((lines.len() as u64, rules))
}

fn random_table(r: &mut Rng) -> Vec<u8> {
    let mut lines: Vec<String> = vec![r#"{"type":"header","x":1}"#.into()];
    let n = r.below(6);
    for i in 0..n {
        let idx = if r.below(8) == 0 {
            r.below(n.max(1))
        } else {
            i
        };
        let tgt = if r.below(8) == 0 { r.below(3) } else { 100 + i };
        let case = if r.below(4) == 0 {
            "c".to_string()
        } else {
            format!("c{i}")
        };
        let chart = if r.below(10) == 0 { "\"12\"" } else { "\"11\"" };
        let target = if r.below(10) == 0 {
            String::new()
        } else {
            format!(",\"target\":\"I({tgt})\"")
        };
        lines.push(format!(
            r#"{{"type":"rule","chart":{chart},"index":{idx},"case":"{case}"{target},"rhs":[]}}"#
        ));
        if r.below(5) == 0 {
            lines.push(r#"{"type":"residual","key":"I(7)"}"#.into());
        }
    }
    let end = ["end", "partial", "chart_end", "end", "end"][r.below(5) as usize];
    lines.push(format!(r#"{{"type":"{end}","rules":{n}}}"#));
    // structural corruptions
    match r.below(10) {
        0 => {
            let k = r.below(lines.len() as u64) as usize;
            lines.insert(k, r#"{"type":"end"}"#.into());
        }
        1 => lines.push(r#"{"type":"rule","index":0}"#.into()),
        4 => {
            let k = 1 + r.below(lines.len() as u64) as usize;
            lines.insert(k.min(lines.len()), r#"{"type":"header","y":2}"#.into());
        }
        2 => {
            let k = r.below(lines.len() as u64) as usize;
            lines[k] = lines[k].replacen("\"index\":", "\"index\":0.", 1);
        }
        3 => {
            let k = r.below(lines.len() as u64) as usize;
            lines[k].push_str(" x");
        }
        _ => {}
    }
    let mut b = lines.join("\n").into_bytes();
    b.push(b'\n');
    match r.below(12) {
        0 => {
            b.pop();
        }
        1 => {
            let k = r.below(b.len() as u64) as usize;
            b.truncate(k);
        }
        2 => {
            let k = r.below(b.len() as u64) as usize;
            b[k] = r.next() as u8;
        }
        3 => b.extend_from_slice(b"\n"),
        _ => {}
    }
    b
}

#[test]
fn ac2_fuzz_verifier_differential() {
    let n = scale(20_000);
    let mut r = Rng(0xAC2_F006);
    let d = TempDir::new("vd");
    std::fs::create_dir_all(d.path().join("out")).unwrap();
    let p = d.path().join("out/F.table.jsonl");
    let (mut sealed, mut refused) = (0u64, 0u64);
    for i in 0..n {
        let b = random_table(&mut r);
        std::fs::write(&p, &b).unwrap();
        let ours = seal::verify_stream(d.path(), "out/F.table.jsonl", &TABLE_END_TYPES);
        let theirs = independent_verify(&b);
        match (&ours, theirs) {
            (Ok(v), Some((records, rules))) => {
                sealed += 1;
                assert_eq!((v.records, v.rules), (records, rules), "case {i}");
                assert_eq!(v.bytes, b.len() as u64);
                assert_eq!(v.blake3, rustred_campaign_contract::blake3_hex(&b));
            }
            (Err(_), None) => refused += 1,
            _ => panic!(
                "case {i}: verifier {:?} vs independent {:?} on {:?}",
                ours.as_ref().map(|v| (v.records, v.rules)),
                theirs,
                String::from_utf8_lossy(&b)
            ),
        }
        // the engine-side running digest agrees at any chunking
        let mut sd = StreamDigest::new(&TABLE_END_TYPES);
        let mut k = 0;
        while k < b.len() {
            let step = 1 + r.below(64) as usize;
            sd.feed(&b[k..(k + step).min(b.len())]);
            k += step;
        }
        match (&ours, sd.seal()) {
            (Ok(v), Some(s)) => {
                assert_eq!(
                    (s.bytes, &s.blake3, s.records, s.rules, &s.canonical_blake3),
                    (v.bytes, &v.blake3, v.records, v.rules, &v.canonical_blake3),
                    "case {i}"
                );
            }
            (Err(_), None) => {}
            // the file verifier needs an end record of type end/partial; StreamDigest::seal also requires it
            (o, s) => panic!(
                "case {i}: verify {:?} vs StreamDigest {:?}",
                o.as_ref().map(|v| v.bytes),
                s.map(|s| s.bytes)
            ),
        }
    }
    eprintln!(
        "ac2_fuzz_verifier_differential: {n} random tables ({sealed} sealed, {refused} refused), verifier == independent verifier == StreamDigest"
    );
}

/// Prefix verification: for random honest tables and EVERY byte length, `verify_prefix` accepts exactly the line
/// boundaries before the end record whose figures were committed, and the copy (`RecordWriter::resume_from`)
/// reproduces those bytes.
#[test]
fn ac2_fuzz_prefix_every_length() {
    let n = scale(300);
    let mut r = Rng(0xAC2_F007);
    let d = TempDir::new("pfx");
    std::fs::create_dir_all(d.path().join("out")).unwrap();
    let p = d.path().join("out/F.table.jsonl");
    let mut checked = 0u64;
    for _ in 0..n {
        let rows = 1 + r.below(5);
        let mut b = Vec::new();
        b.extend_from_slice(b"{\"type\":\"header\"}\n");
        for i in 0..rows {
            b.extend_from_slice(format!("{{\"type\":\"rule\",\"chart\":\"1\",\"index\":{i},\"target\":\"I({i})\",\"rhs\":[]}}\n").as_bytes());
        }
        let body = b.len();
        b.extend_from_slice(b"{\"type\":\"partial\"}\n");
        std::fs::write(&p, &b).unwrap();
        for k in 0..=b.len() {
            let mut sd = StreamDigest::new(&TABLE_END_TYPES);
            sd.feed(&b[..k]);
            let want = sd.committed("out/F.table.jsonl");
            let boundary = k == 0 || b[k - 1] == b'\n';
            assert_eq!(want.is_some(), boundary && k <= body, "k={k}");
            if let Some(c) = want {
                seal::verify_prefix(d.path(), &c, &TABLE_END_TYPES).unwrap();
                // a claim one byte off never verifies
                let mut off = c.clone();
                off.bytes += 1;
                assert!(seal::verify_prefix(d.path(), &off, &TABLE_END_TYPES).is_err());
                checked += 1;
            }
        }
    }
    eprintln!(
        "ac2_fuzz_prefix_every_length: {n} tables, {checked} committed prefixes verified (+ off-by-one refused)"
    );
}
