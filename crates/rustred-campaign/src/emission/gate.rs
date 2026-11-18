//! THE EMISSION GATE of a writer that produces a whole table (emission contract section 10, routing; lane gencheck
//! round 2 item 1): every rule line goes through [`Emitter::emit`] before it is written. Lifted records are checked
//! with the certificate of THEIR lift (found among the producer's certificates by the record's `case`, the first member
//! of an engine certificate); point / symmetry records with the instance-span check at their own key. FAIL CLOSED: one
//! refused record and nothing is written (the caller keeps its old output or stops with the refusals). Used by
//! `rr_campaign emit-table` (any table a flow wrote: node assembly, closeloop relation / symmetry tables) and by the
//! closeloop repair writer when its policy has `emission`.
use super::emitter::{Candidate, Emitter, point_key};
use rustred_campaign_core::emission::body::{body_ids, string_member, strip_derivation};
use rustred_campaign_core::emission::class::{Kind, Liveness, Origin};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

/// The `case` text of an engine certificate (its first member; read from the file's head only).
pub fn cert_case(p: &Path) -> Option<String> {
    let mut f = std::fs::File::open(p).ok()?;
    let mut buf = vec![0u8; 1 << 22];
    let n = f.read(&mut buf).ok()?;
    let s = std::str::from_utf8(&buf[..n]).ok().or_else(|| {
        // a cut in the middle of a UTF-8 sequence: keep the valid prefix
        std::str::from_utf8(&buf[..n.saturating_sub(4)]).ok()
    })?;
    let rest = s.trim_start().strip_prefix('{')?.trim_start();
    let rest = rest.strip_prefix("\"case\"")?.trim_start().strip_prefix(':')?.trim_start();
    if !rest.starts_with('"') {
        return None;
    }
    let b = rest.as_bytes();
    let mut i = 1;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            b'"' => return serde_json::from_str::<String>(&rest[..=i]).ok(),
            _ => i += 1,
        }
    }
    None
}

/// The producer's certificates, by case.
#[derive(Clone, Debug, Default)]
pub struct CertIndex {
    pub by_case: BTreeMap<String, Vec<PathBuf>>,
    pub unreadable: Vec<PathBuf>,
}

impl CertIndex {
    /// Every `*.json` of the given directories (missing directories are skipped).
    pub fn from_dirs(dirs: &[PathBuf]) -> CertIndex {
        let mut ix = CertIndex::default();
        for d in dirs {
            let Ok(rd) = std::fs::read_dir(d) else {
                continue;
            };
            let mut fs: Vec<PathBuf> = rd
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x == "json"))
                .collect();
            fs.sort();
            for f in fs {
                match cert_case(&f) {
                    Some(c) => ix.by_case.entry(c).or_default().push(f),
                    None => ix.unreadable.push(f),
                }
            }
        }
        ix
    }
    /// The certificates of a case (those without a readable case are tried last).
    pub fn for_case(&self, case: &str) -> Vec<PathBuf> {
        let mut v = self.by_case.get(case).cloned().unwrap_or_default();
        v.extend(self.unreadable.iter().cloned());
        v
    }
}

/// Which check kind the records get.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateKind {
    Lifted,
    Point,
    Symmetry,
    /// point when every slot is fixed, else lifted
    Auto,
}

impl GateKind {
    pub fn parse(s: &str) -> Result<GateKind, String> {
        Ok(match s {
            "lifted" => GateKind::Lifted,
            "point" => GateKind::Point,
            "symmetry" => GateKind::Symmetry,
            "auto" => GateKind::Auto,
            o => return Err(format!("unknown kind {o} (lifted / point / symmetry / auto)")),
        })
    }
}

/// The gated lines (only when nothing was refused) and the refusals.
#[derive(Clone, Debug, Default)]
pub struct Gated {
    pub lines: Vec<String>,
    pub refused: Vec<Value>,
    pub classes: BTreeMap<String, u64>,
}

impl Gated {
    pub fn ok(&self) -> bool {
        self.refused.is_empty()
    }
}

/// Gate rule lines (each already at its position: `index` = position) of a table with `header`.
pub fn gate_lines(
    em: &Emitter,
    header: &str,
    lines: &[String],
    certs: &CertIndex,
    kind: GateKind,
    tag: &str,
) -> Result<Gated, String> {
    let mut g = Gated::default();
    for (k, l) in lines.iter().enumerate() {
        let (body, _) = strip_derivation(l)?;
        let ids = body_ids(&body)?;
        let kind = match kind {
            GateKind::Lifted => Kind::Lifted,
            GateKind::Point => Kind::Point,
            GateKind::Symmetry => Kind::Symmetry,
            GateKind::Auto if point_key(&body).is_some() => Kind::Point,
            GateKind::Auto => Kind::Lifted,
        };
        let tries: Vec<Option<PathBuf>> = if kind == Kind::Lifted {
            let c = certs.for_case(&ids.case);
            if c.is_empty() { vec![None] } else { c.into_iter().map(Some).collect() }
        } else {
            vec![None]
        };
        let mut got = None;
        let mut why = String::new();
        for (t, cert) in tries.into_iter().enumerate() {
            let c = Candidate {
                header: header.to_string(),
                line: l.clone(),
                kind,
                origin: Origin::Lift,
                cert,
                provenance: None,
                lift_seconds: 0.0,
                ladder: None,
                cone: None,
                sample: None,
                tag: format!("{tag}-l{k}-c{t}"),
            };
            let e = em.emit(&c, Liveness::Unknown, false)?;
            match e.line {
                Some(x) => {
                    *g.classes.entry(e.classed.class.word().to_string()).or_default() += 1;
                    got = Some(x);
                    break;
                }
                None => why = e.refused.unwrap_or_default(),
            }
        }
        match got {
            Some(x) => g.lines.push(x),
            None => g.refused.push(json!({"line": k, "index": serde_json::from_str::<Value>(&body).ok().and_then(|v| v["index"].as_i64()),
                                          "case": ids.case, "kind": kind.word(), "reason": why})),
        }
    }
    Ok(g)
}

/// `emit-table`: gate every rule line of `table_in`; write `table_out` (header and other records verbatim) and its side
/// directory only when NOTHING is refused.
pub fn emit_table(
    em_for: &dyn Fn(&Path) -> Result<Emitter, String>,
    table_in: &Path,
    table_out: &Path,
    certs: &CertIndex,
    kind: GateKind,
) -> Result<(Value, bool), String> {
    let text = std::fs::read_to_string(table_in).map_err(|e| format!("{}: {e}", table_in.display()))?;
    let all: Vec<&str> = text.lines().collect();
    let header = all
        .iter()
        .find(|l| string_member(l, "type").as_deref() == Some("header"))
        .copied()
        .unwrap_or("");
    let rules: Vec<String> = all
        .iter()
        .filter(|l| string_member(l, "type").as_deref() == Some("rule"))
        .map(|l| l.to_string())
        .collect();
    let em = em_for(&rustred_campaign_core::emission::side_dir(table_out))?;
    let g = gate_lines(&em, header, &rules, certs, kind, "t")?;
    let ok = g.ok() && !rules.is_empty();
    if ok {
        let mut out = String::new();
        let mut it = g.lines.iter();
        for l in &all {
            if string_member(l, "type").as_deref() == Some("rule") {
                out.push_str(it.next().expect("one gated line per rule"));
            } else {
                out.push_str(l);
            }
            out.push('\n');
        }
        let tmp = table_out.with_extension("jsonl.part");
        std::fs::write(&tmp, out).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, table_out).map_err(|e| e.to_string())?;
    }
    Ok((
        json!({"schema": "rustred.campaign.emit-table.v1", "table_in": table_in.display().to_string(),
               "table_out": ok.then(|| table_out.display().to_string()), "rules": rules.len(),
               "classes": g.classes, "refused": g.refused.len(),
               "refusals": g.refused.iter().take(50).collect::<Vec<_>>(),
               "certificates": certs.by_case.values().map(Vec::len).sum::<usize>(),
               "verdict": if ok { "EMITTED" } else { "REFUSED (nothing written)" }}),
        ok,
    ))
}

/// An emitter built from flow settings (`emission` member of a flow config) for the side directory `side`.
pub fn emitter_from(
    es: &rustred_campaign_core::emission::Settings,
    side: &Path,
    work: &Path,
) -> Result<Emitter, String> {
    let mut ck = super::emitter::Checker::new(Path::new(&es.checker))?;
    ck.recover_rows = es.recover_rows;
    ck.family = es.family.as_ref().map(PathBuf::from);
    ck.symmetry_group = es.symmetry_group.as_ref().map(PathBuf::from);
    ck.zero_sectors = es.zero_sectors.as_ref().map(PathBuf::from);
    if let Some(f) = es.floor_s {
        ck.floor_s = f;
    }
    Emitter::new(ck, es.policy.clone().unwrap_or_default(), side, work)
}

/// The rule lines of a table text with their `derivation` member removed (and every other line as it is).
fn bodies(text: &str) -> Result<Vec<String>, String> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            if string_member(l, "type").as_deref() == Some("rule") {
                strip_derivation(l).map(|x| x.0)
            } else {
                Ok(l.to_string())
            }
        })
        .collect()
}

/// THE IN-FLOW GATE of a table a flow writes as TEXT (closeloop relation / symmetry tables; lane gencheck round 3).
/// The flow's own table is untouched (its bytes stay those the flow re-derives and verifies on a restart); the
/// EMITTED copy `emitted` (every rule line with its derivation member) and its side directory are written ONCE. A later
/// pass or a restart REUSES an existing emitted copy when its bodies are exactly the plain text's and `table-check`
/// accepts it (never re-gated, never rewritten); otherwise, when `may_write`, the plain text is gated (fail closed: Err
/// with the refusals, nothing written); when not `may_write` (a verifying restart), a missing or different copy is an
/// Err. Returns the gate's summary.
pub fn gate_beside(
    es: &rustred_campaign_core::emission::Settings,
    plain: &str,
    emitted: &Path,
    kind: GateKind,
    cert_dirs: &[PathBuf],
    may_write: bool,
) -> Result<Value, String> {
    if let Ok(have) = std::fs::read_to_string(emitted)
        && bodies(&have)? == bodies(plain)?
    {
        let (tc, ok) = super::tablecheck::table_check(&super::tablecheck::Args {
            table: emitted.to_path_buf(),
            every: 1,
            ..Default::default()
        })?;
        if ok {
            return Ok(json!({"emitted": emitted.display().to_string(), "reused": true, "classes": tc["classes"]}));
        }
    }
    if !may_write {
        return Err(format!(
            "emission: {}: the emitted copy is missing or differs from the re-derived table (restart: never re-gated)",
            emitted.display()
        ));
    }
    let plain_path = emitted.with_extension("plain.jsonl");
    std::fs::write(&plain_path, plain).map_err(|e| format!("{}: {e}", plain_path.display()))?;
    let work = emitted.with_extension("emit-work");
    let em_for = |side: &Path| emitter_from(es, side, &work);
    let (s, ok) = emit_table(&em_for, &plain_path, emitted, &CertIndex::from_dirs(cert_dirs), kind)?;
    let _ = std::fs::remove_file(&plain_path);
    if !ok {
        return Err(format!(
            "emission refused {} record(s) of {} (nothing written): {}",
            s["refused"],
            emitted.display(),
            s["refusals"]
        ));
    }
    Ok(s)
}

/// CLI: `emit-table --table T --out O --checker BIN [--certs DIR ...] [--kind lifted|point|symmetry|auto]
/// [--policy FILE] [--family F] [--zero-sectors F] [--symmetry-group F] [--no-recover-rows] [--check-floor S]
/// [--work DIR]`. Exit 0 emitted, 3 refused (nothing written).
pub fn cmd(rest: &[String]) -> Result<(String, bool), String> {
    let (mut table, mut out, mut checker) = (None::<PathBuf>, None::<PathBuf>, None::<PathBuf>);
    let mut certs: Vec<PathBuf> = vec![];
    let mut kind = GateKind::Auto;
    let (mut policy, mut family, mut zeros, mut sym, mut work) = (None, None, None, None, None);
    let mut recover = true;
    let mut floor: Option<f64> = None;
    let mut i = 0;
    let val = |i: usize| rest.get(i + 1).cloned().ok_or(format!("{} needs a value", rest[i]));
    while i < rest.len() {
        match rest[i].as_str() {
            "--table" => table = Some(val(i)?.into()),
            "--out" => out = Some(val(i)?.into()),
            "--checker" => checker = Some(val(i)?.into()),
            "--certs" => certs.push(val(i)?.into()),
            "--kind" => kind = GateKind::parse(&val(i)?)?,
            "--policy" => policy = Some(PathBuf::from(val(i)?)),
            "--family" => family = Some(PathBuf::from(val(i)?)),
            "--zero-sectors" => zeros = Some(PathBuf::from(val(i)?)),
            "--symmetry-group" => sym = Some(PathBuf::from(val(i)?)),
            "--work" => work = Some(PathBuf::from(val(i)?)),
            "--check-floor" => floor = Some(val(i)?.parse().map_err(|e| format!("--check-floor: {e}"))?),
            "--no-recover-rows" => {
                recover = false;
                i -= 1;
            }
            o => return Err(format!("unknown flag {o}")),
        }
        i += 2;
    }
    let (Some(table), Some(out), Some(checker)) = (table, out, checker) else {
        return Err("emit-table --table T --out O --checker BIN [--certs DIR ...] [--kind K] [...]".into());
    };
    let pol = match &policy {
        Some(p) => serde_json::from_slice(&std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?)
            .map_err(|e| format!("{}: {e}", p.display()))?,
        None => rustred_campaign_core::emission::class::Policy::default(),
    };
    let work = work.unwrap_or_else(|| out.with_extension("emit-work"));
    let em_for = |side: &Path| -> Result<Emitter, String> {
        let mut ck = super::emitter::Checker::new(&checker)?;
        ck.recover_rows = recover;
        ck.family = family.clone();
        ck.zero_sectors = zeros.clone();
        ck.symmetry_group = sym.clone();
        if let Some(f) = floor {
            ck.floor_s = f;
        }
        Emitter::new(ck, pol.clone(), side, &work)
    };
    let (s, ok) = emit_table(&em_for, &table, &out, &CertIndex::from_dirs(&certs), kind)?;
    Ok((s.to_string(), ok))
}
