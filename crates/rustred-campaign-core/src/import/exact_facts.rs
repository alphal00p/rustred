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
//!
//! A RECORD LONGER THAN THE PER-RECORD BUDGET (spec `reader.record_budget_bytes`; lane bigline) is never read whole:
//! it is streamed twice from its offset, each pass checked against `record_sha256` (pass 1: the record without its rhs
//! value, for the domain and the fields; pass 2: the rhs terms one at a time, each decoded strictly and evaluated
//! with the same exact code, then dropped). The answer is identical to the in-memory evaluation. A single term larger
//! than the budget cannot be evaluated in memory: a typed refusal (`record_too_large`, naming the record and the term
//! size); the key is then left to the child job, never skipped, never counted as served.

use std::io::{BufReader, Read, Seek, SeekFrom};

use serde_json::{Value, json};

use super::error::{ImportError, TableRefusal, io_err};
use super::evidence::FACT_VERDICT_SCHEMA;
use super::lines::term_key_digest;
use super::load::{ImportedSet, RuleInfo};
use crate::accept::facts::{CoefficientFacts, ExactRecordFacts, RegVerdict, Regularity};
use crate::accept::rule::{RuleText, RuleTextError};
use crate::closure::CoefFact;
use crate::closure::sha256::Sha256;
use crate::domain::PoleKind;
use crate::record::stream::{StreamOptions, StreamedLine, TermVisitor, stream_line};
use crate::record::{RecordError, RhsMode};

fn too_large(info: &RuleInfo, what: String, bytes: u64, budget: u64) -> ImportError {
    ImportError::Refused {
        source: info.source.clone(),
        path: info.path.clone(),
        line: 0,
        reason: Box::new(TableRefusal::RecordTooLarge {
            record: Some(info.label),
            what: format!("{what} (record at byte offset {})", info.offset),
            bytes,
            budget,
        }),
    }
}

/// The record line of (table, record), read at its offset and checked against its `record_sha256`. A record longer
/// than the spec's per-record budget is refused, typed (never read whole).
pub fn record_line(set: &ImportedSet, table: usize, record: usize) -> Result<Vec<u8>, ImportError> {
    let info = set.rule(table, record);
    let budget = set.spec.read_limits().record_budget;
    if info.bytes > budget {
        return Err(too_large(
            info,
            "the record line".into(),
            info.bytes,
            budget,
        ));
    }
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

/// One streaming pass over the record of `info` (module documentation), checked against its length and
/// `record_sha256`.
fn stream_record(
    info: &RuleInfo,
    budget: u64,
    visit: Option<&mut TermVisitor>,
) -> Result<StreamedLine, ImportError> {
    let p = std::path::Path::new(&info.path);
    let mut f = std::fs::File::open(p).map_err(|e| io_err(p, e))?;
    f.seek(SeekFrom::Start(info.offset))
        .map_err(|e| io_err(p, e))?;
    let mut rd = BufReader::with_capacity(1 << 20, f);
    let mut h = Sha256::new();
    let opts = StreamOptions {
        rhs: RhsMode::Skip,
        rhs_digest: false,
        budget,
        line: 1,
    };
    let sl = stream_line(&mut rd, &opts, &mut |seg| h.update(seg), visit).map_err(|e| match e {
        RecordError::TooLarge {
            what,
            bytes,
            budget,
            ..
        } => too_large(info, what, bytes, budget),
        RecordError::Io(err) => io_err(p, err),
        e => ImportError::Source {
            source: info.source.clone(),
            msg: format!("record {}: {e}", info.label),
        },
    })?;
    let got = h.finish_hex();
    if sl.bytes != info.bytes || got != info.record_sha256 {
        return Err(ImportError::Digest {
            path: format!("{}@{}", info.path, info.offset),
            expected: info.record_sha256.clone(),
            got,
        });
    }
    Ok(sl)
}

/// "term 0: .." of a one-term evaluation, renumbered to the term's place in the record.
fn renumber(m: String, term: usize) -> String {
    match m.strip_prefix("term 0: ") {
        Some(rest) => format!("term {term}: {rest}"),
        None => m,
    }
}

/// What the verdict line needs of a record: its parsed text (rhs empty when streamed), the regularity verdict, the
/// fact of every coefficient at the key.
struct Evaluated {
    rt: RuleText,
    reg: RegVerdict,
    coefs: Vec<CoefFact>,
}

/// The record evaluated in memory (a record within the budget).
fn evaluate_whole(rt: RuleText, key: &[i32]) -> Evaluated {
    let mut ex = ExactRecordFacts::new();
    let reg = ex.regular(&rt, key);
    let coefs = (0..rt.rhs.len())
        .map(|j| ex.coefficient(&rt, key, j))
        .collect();
    Evaluated { rt, reg, coefs }
}

/// The record streamed (a record above the budget): one term at a time, the same exact code on a one-term copy of the
/// record (witness loci checked once, after the terms, as [`ExactRecordFacts::regular`] does).
fn evaluate_streamed(
    set: &ImportedSet,
    info: &RuleInfo,
    key: &[i32],
    budget: u64,
) -> Result<Evaluated, ImportError> {
    let src_err = |e: RuleTextError| ImportError::Source {
        source: info.source.clone(),
        msg: format!("record {}: {e:?}", info.label),
    };
    let sl = stream_record(info, budget, None)?;
    let rt = RuleText::parse(&sl.compact, &set.frame).map_err(src_err)?;
    match &sl.rhs {
        // no rhs was left out: the compact line is the record
        None => return Ok(evaluate_whole(rt, key)),
        Some(r) if !r.array => {
            return Err(src_err(RuleTextError::NotARule(
                "rhs is not an array (a record above the budget is evaluated term by term)".into(),
            )));
        }
        Some(_) => {}
    }
    let mut one = rt.clone();
    if let Some(d) = &mut one.domain {
        d.poles.retain(|p| p.kind != PoleKind::Witness);
    }
    let mut first: Option<RegVerdict> = None;
    let mut coefs = vec![];
    let mut bad: Option<RuleTextError> = None;
    let mut visit = |j: usize, raw: &[u8]| -> Result<(), RecordError> {
        if bad.is_some() {
            return Ok(());
        }
        let t: Value = match serde_json::from_slice(raw) {
            Ok(t) => t,
            Err(e) => {
                bad = Some(RuleTextError::NotJson(format!("rhs term {j}: {e}")));
                return Ok(());
            }
        };
        let (Some(k), Some(c)) = (
            t.get(0).and_then(Value::as_str),
            t.get(1).and_then(Value::as_str),
        ) else {
            bad = Some(RuleTextError::NotARule(
                "rhs term is not [pattern, coefficient]".into(),
            ));
            return Ok(());
        };
        one.rhs = vec![(k.to_string(), c.to_string())];
        one.line_sha256 = format!("sha256:{}#term{j}", info.record_sha256);
        let mut ex = ExactRecordFacts::new();
        if first.is_none() {
            match ex.regular(&one, key) {
                RegVerdict::Regular => {}
                RegVerdict::Pole { .. } => first = Some(RegVerdict::Pole { term: j }),
                RegVerdict::Unknown(m) => first = Some(RegVerdict::Unknown(renumber(m, j))),
                h @ RegVerdict::Hidden { .. } => first = Some(h),
            }
        }
        coefs.push(ex.coefficient(&one, key, 0));
        Ok(())
    };
    stream_record(info, budget, Some(&mut visit))?;
    if let Some(e) = bad {
        return Err(src_err(e));
    }
    let reg = first.unwrap_or_else(|| ExactRecordFacts::new().regular(&rt, key));
    Ok(Evaluated { rt, reg, coefs })
}

/// The exact fact verdict line of (table, record) at `key` (module documentation).
pub fn fact_verdict_line(
    set: &ImportedSet,
    table: usize,
    record: usize,
    key: &[i32],
) -> Result<Value, ImportError> {
    let info = set.rule(table, record);
    let budget = set.spec.read_limits().record_budget;
    let ev = if info.bytes > budget {
        evaluate_streamed(set, info, key, budget)?
    } else {
        let line = record_line(set, table, record)?;
        let rt = RuleText::parse(&line, &set.frame).map_err(|e| ImportError::Source {
            source: info.source.clone(),
            msg: format!("record {}: {e:?}", info.label),
        })?;
        evaluate_whole(rt, key)
    };
    let rt = &ev.rt;
    let dom = rt.domain.as_ref().ok_or_else(|| ImportError::Source {
        source: info.source.clone(),
        msg: format!("record {}: no domain ({:?})", info.label, rt.domain_error),
    })?;
    let verdict = match ev.reg.clone() {
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
    if nt != ev.coefs.len() {
        return Err(ImportError::Core(format!(
            "record {}: {nt} instantiated terms, {} in the record text",
            info.label,
            ev.coefs.len()
        )));
    }
    let (mut nonzero, mut unknown, mut zero) = (vec![], vec![], 0usize);
    for j in 0..nt {
        let d = format!("{:016x}", term_key_digest(&ib[j * n..(j + 1) * n]));
        match ev.coefs[j] {
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
