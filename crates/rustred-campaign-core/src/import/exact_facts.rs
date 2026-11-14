//! EXACT fact verdict lines for one (record, key) from the record's own text (lane import3, item 5).
//!
//! The record line is read at its byte offset in its file and checked against its `record_sha256`; every coefficient
//! `num/den` is evaluated EXACTLY at the rule point with the core's exact code (`accept::facts::ExactRecordFacts`:
//! `poly::Poly` with arbitrary-precision coefficients, `domain::PoleLocus::vanishes_at` = identically zero in d), the
//! terms are instantiated with the closure's own bodies, and the answer is written in the form the importer reads:
//! a `rustred.campaign.fact-verdict.v1` line (interfaces/import.md 5.5) listing the term-key digests of the NON-ZERO
//! terms only (an unlisted term is identically zero at the key and is NOT an edge) and of the terms whose denominator
//! vanishes identically (`unknown`). Verdict: `regular` (no denominator and no witness locus vanishes), `irregular`
//! (a denominator vanishes), `hidden` (a witness locus vanishes, or the witness loci are ELIDED: regularity cannot be
//! read off the record; a bound per-key proof supplies it), anything else is not usable.
//!
//! In a campaign run these facts come from the `proof-instantiate` child job (rr_proof instantiate answering the
//! import's fact requests, adapter (b)); this function is the same exact evaluation inside the library, for small
//! numbers of (record, key) pairs (each call reads and parses one record line).

use std::io::{Read, Seek, SeekFrom};

use serde_json::{Value, json};

use super::error::{ImportError, io_err};
use super::evidence::FACT_VERDICT_SCHEMA;
use super::lines::term_key_digest;
use super::load::ImportedSet;
use crate::accept::facts::{CoefficientFacts, ExactRecordFacts, RegVerdict, Regularity};
use crate::accept::rule::RuleText;
use crate::closure::CoefFact;
use crate::closure::sha256::Sha256;

/// The record line of (table, record), read at its offset and checked against its `record_sha256`.
pub fn record_line(set: &ImportedSet, table: usize, record: usize) -> Result<Vec<u8>, ImportError> {
    let info = set.rule(table, record);
    let p = std::path::Path::new(&info.path);
    let mut f = std::fs::File::open(p).map_err(|e| io_err(p, e))?;
    f.seek(SeekFrom::Start(info.offset))
        .map_err(|e| io_err(p, e))?;
    let mut buf = vec![0u8; info.bytes as usize];
    f.read_exact(&mut buf).map_err(|e| io_err(p, e))?;
    let mut h = Sha256::new();
    h.update(&buf);
    let got = h.finish_hex();
    if got != info.record_sha256 {
        return Err(ImportError::Digest {
            path: format!("{}@{}", info.path, info.offset),
            expected: info.record_sha256.clone(),
            got,
        });
    }
    Ok(buf)
}

/// The exact fact verdict line of (table, record) at `key` (module documentation).
pub fn fact_verdict_line(
    set: &ImportedSet,
    table: usize,
    record: usize,
    key: &[i32],
) -> Result<Value, ImportError> {
    let info = set.rule(table, record);
    let line = record_line(set, table, record)?;
    let rt = RuleText::parse(&line, &set.frame).map_err(|e| ImportError::Source {
        source: info.source.clone(),
        msg: format!("record {}: {e:?}", info.label),
    })?;
    let dom = rt.domain.as_ref().ok_or_else(|| ImportError::Source {
        source: info.source.clone(),
        msg: format!("record {}: no domain ({:?})", info.label, rt.domain_error),
    })?;
    let mut ex = ExactRecordFacts::new();
    let verdict = match ex.regular(&rt, key) {
        RegVerdict::Regular if dom.witness_elided => "hidden".to_string(),
        RegVerdict::Regular => "regular".into(),
        RegVerdict::Pole { .. } => "irregular".into(),
        RegVerdict::Hidden { .. } => "hidden".into(),
        RegVerdict::Unknown(m) => format!("unknown: {m}"),
    };
    let (mut ib, mut cm) = (vec![], vec![]);
    let n = set.arity();
    let nt = set
        .bodies
        .instantiate(
            &set.disp,
            set.disp.rule_ref(table, record),
            key,
            &mut ib,
            &mut cm,
        )
        .map_err(|e| ImportError::Core(format!("instantiate: {e:?}")))?
        .ok_or_else(|| {
            ImportError::Core(format!("record {}: no rule point at the key", info.label))
        })?;
    if nt != rt.rhs.len() {
        return Err(ImportError::Core(format!(
            "record {}: {nt} instantiated terms, {} in the record text",
            info.label,
            rt.rhs.len()
        )));
    }
    let (mut nonzero, mut unknown, mut zero) = (vec![], vec![], 0usize);
    for j in 0..nt {
        let d = format!("{:016x}", term_key_digest(&ib[j * n..(j + 1) * n]));
        match ex.coefficient(&rt, key, j) {
            CoefFact::Zero => zero += 1,
            CoefFact::NonZero => nonzero.push(d),
            CoefFact::Unknown => unknown.push(d),
        }
    }
    nonzero.sort();
    nonzero.dedup();
    unknown.sort();
    unknown.dedup();
    Ok(
        json!({"schema": FACT_VERDICT_SCHEMA, "index": rt.index(), "record_sha256": info.record_sha256,
        "key": key, "verdict": verdict, "n_terms": nt, "nonzero": nonzero, "unknown": unknown,
        "zero_terms": zero, "producer": "rustred-campaign-core import::exact_facts (exact, record text)"}),
    )
}
