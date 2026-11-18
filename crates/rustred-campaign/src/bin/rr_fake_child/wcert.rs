//! `rr_fake_child --mode weight-cert --cert C --table T --rule N --offset B [...]` (lane gencheck, emission contract): a
//! FAKE of the independent checker `rr_proof --mode weight-cert`, for supervisor-level tests of the emission hook
//! without Symbolica. It reads the rule line at byte `B` of `T` and binds its answer to that line's relation and domain
//! (computed with the campaign core's body hashes) and to the sha256 of `C`, exactly like the real checker. The stage
//! verdicts come from the certificate's `fake_checker` member (defaults: rows verified, identity exact, binding equal,
//! no open locus, class WHOLE_DOMAIN). The output says `"fake": true`. Also the rule-commit side: a fake rule record
//! with a `fake_weight_cert` member has that member removed and written as `out/wcert/r<index>.json` of its run.
use rustred_campaign_core::emission::body::{body_ids, sha256_hex};
use serde_json::{Value, json};
use std::io::{BufRead, Seek, SeekFrom};

fn die(msg: &str) -> ! {
    println!("{}", json!({"verdict": "ERROR", "error": msg}));
    std::process::exit(2)
}

pub fn main(args: &[String]) -> ! {
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let (Some(cert), Some(table), Some(offset)) = (flag("--cert"), flag("--table"), flag("--offset")) else {
        die("fake weight-cert: --cert, --table and --offset are required")
    };
    let cert_bytes = std::fs::read(&cert).unwrap_or_else(|e| die(&format!("{cert}: {e}")));
    let c: Value = serde_json::from_slice(&cert_bytes).unwrap_or(Value::Null);
    let off: u64 = offset.parse().unwrap_or_else(|_| die("bad --offset"));
    let mut f = std::fs::File::open(&table).unwrap_or_else(|e| die(&format!("{table}: {e}")));
    f.seek(SeekFrom::Start(off)).unwrap_or_else(|e| die(&e.to_string()));
    let mut line = String::new();
    std::io::BufReader::new(f)
        .read_line(&mut line)
        .unwrap_or_else(|e| die(&e.to_string()));
    let ids = body_ids(line.trim_end()).unwrap_or_else(|e| die(&format!("rule line: {e}")));
    let fk = &c["fake_checker"];
    let word = |k: &str, d: &str| fk[k].as_str().unwrap_or(d).to_string();
    let open = fk["open"].as_u64().unwrap_or(0);
    let out = json!({"schema": "rustred.proof-kernel.weight-cert.v1", "mode": "weight-cert", "fake": true,
        "rule": {"target_rhs_sha256": ids.relation_sha256, "domain_sha256": ids.domain_sha256},
        "cert": {"sha256": sha256_hex(&cert_bytes)},
        "rows": {"verdict": word("rows", "ROWS_VERIFIED")},
        "identity": {"verdict": word("identity", "IDENTITY_EXACT")},
        "binding": {"verdict": word("binding", "BINDING_EQUAL")},
        "factors": {"producer": "fake"},
        "loci_counts": {"OPEN": open},
        "loci": (0..open).map(|i| json!({"verdict": "OPEN", "factor": format!("n{i}"), "argument": {"generators": [format!("n{i}")]}})).collect::<Vec<_>>(),
        "class": word("class", "WHOLE_DOMAIN")});
    println!("{out}");
    std::process::exit(0)
}

/// Split a fake rule's `fake_weight_cert` member off its record (the record is written without it).
pub fn split_cert(
    r: &rustred_campaign_contract::fake::FakeRule,
) -> (rustred_campaign_contract::fake::FakeRule, Option<Value>) {
    let mut r = r.clone();
    let cert = r
        .record
        .as_object_mut()
        .and_then(|m| m.remove("fake_weight_cert"));
    (r, cert)
}

/// `rr_fake_child --mode instance-span --table T --rule N --offset B --keys K [...]`: a FAKE of `rr_proof
/// --mode instance-span` for point / symmetry records: binds its answer to sha256 of the line at `B` (like the real
/// kernel's `record_sha256`); each key of `K` gets the status of the line's `fake_span` member (default
/// EXACT_IN_SPAN_AT_KEY).
pub fn span_main(args: &[String]) -> ! {
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let (Some(table), Some(offset), Some(keys)) = (flag("--table"), flag("--offset"), flag("--keys")) else {
        die("fake instance-span: --table, --offset and --keys are required")
    };
    let off: u64 = offset.parse().unwrap_or_else(|_| die("bad --offset"));
    let mut f = std::fs::File::open(&table).unwrap_or_else(|e| die(&format!("{table}: {e}")));
    f.seek(SeekFrom::Start(off)).unwrap_or_else(|e| die(&e.to_string()));
    let mut line = String::new();
    std::io::BufReader::new(f)
        .read_line(&mut line)
        .unwrap_or_else(|e| die(&e.to_string()));
    let line = line.trim_end_matches('\n');
    let rec: Value = serde_json::from_str(line).unwrap_or(Value::Null);
    let status = rec["fake_span"].as_str().unwrap_or("EXACT_IN_SPAN_AT_KEY").to_string();
    let ks: Vec<Value> = std::fs::read_to_string(&keys)
        .unwrap_or_else(|e| die(&format!("{keys}: {e}")))
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .map(|k| json!({"key": k["key"], "status": status}))
        .collect();
    let n = ks.len() as u64;
    let exact = status == "EXACT_IN_SPAN_AT_KEY";
    let out = json!({"schema": "rustred.proof-kernel.span.v1", "mode": "instance-span", "fake": true,
        "rule": rec["index"], "record_sha256": sha256_hex(line.as_bytes()), "keys": ks,
        "verdict": if exact { "EXACT_ALL_KEYS_IN_SPAN" } else { "NOT_ALL_KEYS_IN_SPAN" },
        "counts": {"in_span": if exact { n } else { 0 }, "not_in_span": if exact { 0 } else { n }, "other": 0}});
    println!("{out}");
    std::process::exit(0)
}
