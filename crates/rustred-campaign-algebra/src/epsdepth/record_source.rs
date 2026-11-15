//! A node's reduction AS SERVED by a closing-loop statement, read through verified readers only (interfaces/epsdepth.md
//! 2): the statement (`statement.jsonl`, checked against its `SHA256SUMS`), the imported table set (core
//! `import::load`: the spec's tables with their sha256 manifests, the dispatcher, the verified symmetry group), and each
//! serving record line read at its recorded offset and bound by its sha256 to the statement row.
//!
//! Per served key: a RULE row -> the record at the key (rule point by the core's `closure::bodies::rule_point`, term
//! keys by `parse_rhs_pattern`); a CREDIT row -> the record at the image `gK` (the first group element, in group order,
//! whose image the dispatcher gives to the same record and whose composed relation descends below the ORIGINAL key:
//! the closure's own Tier C rule, TRICKS 1); terminals from the statement's terminal list; other rows `open`.
//! Only TOP-sector terms are composed (modulo subsectors); positive-numerator terms carry zero coefficients in a
//! certified statement and are skipped (counted); coefficients are put in at the RULE POINT (exact integers).
//! Generic (L8): arity, frame, slots, terms and terminals are data.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use rustred_campaign_core::accept::facts::split_ratio;
use rustred_campaign_core::accept::rule::RuleText;
use rustred_campaign_core::closure::bodies::{RhsSlot, Sectors, TermClass, parse_rhs_pattern, rule_point};
use rustred_campaign_core::closure::sha256::Sha256;
use rustred_campaign_core::cover::Dispatch;
use rustred_campaign_core::domain::CaseDomain;
use rustred_campaign_core::epsdepth::{KeyStatus, LocalProfile, ReductionSource, TermKind, TermVal, Terms};
use rustred_campaign_core::import::ImportedSet;
use rustred_campaign_core::symmetry::apply_framed;
use serde_json::Value;
use symbolica::domains::integer::IntegerRing;
use symbolica::poly::polynomial::MultivariatePolynomial;

use super::{SymbolicaLaurent, TCoeff};
use crate::SymbolicaAlgebra;

type ZPoly = MultivariatePolynomial<IntegerRing, u16>;

/// One statement row as this source uses it.
#[derive(Clone, Debug)]
enum St {
    Terminal(usize),
    Rule { table: usize, record: usize },
    Credit { table: usize, record: usize, images: Vec<Vec<i32>> },
    Open(String),
}

struct Entry {
    domain: CaseDomain,
    patterns: Vec<Vec<RhsSlot>>,
    texts: Vec<String>,
    parsed: HashMap<usize, Result<(ZPoly, ZPoly), String>>,
    bytes: usize,
    last: u64,
}

/// Counters of the source (reports).
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct SourceStats {
    pub statement_rows: usize,
    pub box_keys: usize,
    pub served_rule: usize,
    pub served_credit: usize,
    pub terminals: usize,
    pub open: usize,
    pub credit_unresolved: usize,
    pub records_read: u64,
    pub records_evicted: u64,
    pub coefficients_parsed: u64,
    pub instantiations: u64,
    pub positive_numerator_terms_skipped: u64,
    pub subsector_terms_dropped: u64,
    pub outside_statement_terms: u64,
}

/// The reduction source over a statement + imported set.
pub struct RecordSource<'a> {
    set: &'a ImportedSet,
    alg: SymbolicaAlgebra,
    lau: SymbolicaLaurent,
    keys: Vec<Vec<i32>>,
    index: HashMap<Vec<i32>, usize>,
    st: Vec<St>,
    server: Vec<String>,
    in_start: Vec<bool>,
    terminals: Vec<Vec<i32>>,
    sectors: Sectors,
    cache: HashMap<(usize, usize), Entry>,
    cache_bytes: usize,
    pub budget_bytes: usize,
    tick: u64,
    pub stats: SourceStats,
}

/// The raw line at `offset` of `path` (without its newline) and its sha256 (hex): the record reader of the import's
/// table index (a record is addressed by byte offset, TRICKS 11).
pub fn line_at(path: &Path, offset: u64) -> Result<(Vec<u8>, String), String> {
    use std::io::{BufRead, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    f.seek(SeekFrom::Start(offset)).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut r = std::io::BufReader::with_capacity(1 << 20, f);
    let mut line = Vec::new();
    r.read_until(b'\n', &mut line).map_err(|e| format!("{}: {e}", path.display()))?;
    if line.last() == Some(&b'\n') {
        line.pop();
    }
    let mut h = Sha256::new();
    h.update(&line);
    Ok((line, h.finish_hex()))
}

/// sha256 hex of a file (streamed).
pub fn file_sha256(path: &Path) -> Result<String, String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf).map_err(|e| format!("{}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finish_hex())
}

/// Check `dir/name` against the `SHA256SUMS` of `dir` (refuses a missing entry or a mismatch).
pub fn verify_in_sums(dir: &Path, name: &str) -> Result<String, String> {
    let sums = std::fs::read_to_string(dir.join("SHA256SUMS")).map_err(|e| format!("SHA256SUMS: {e}"))?;
    let want = sums
        .lines()
        .find_map(|l| {
            let mut it = l.split_whitespace();
            let h = it.next()?;
            let n = it.next()?.trim_start_matches('*');
            (n == name || n.ends_with(&format!("/{name}"))).then(|| h.to_string())
        })
        .ok_or_else(|| format!("{name} is not listed in {}/SHA256SUMS", dir.display()))?;
    let got = file_sha256(&dir.join(name))?;
    if got != want {
        return Err(format!("{name}: sha256 {got} differs from SHA256SUMS {want}"));
    }
    Ok(got)
}

fn parse_key(v: &Value) -> Option<Vec<i32>> {
    v.as_array()?.iter().map(|x| x.as_i64().and_then(|x| i32::try_from(x).ok())).collect()
}

/// `I(a,b,..)` or a JSON array -> key.
fn parse_key_text(s: &str) -> Option<Vec<i32>> {
    let inner = s.trim().strip_prefix("I(")?.strip_suffix(')')?;
    inner.split(',').map(|t| t.trim().parse().ok()).collect()
}

impl<'a> RecordSource<'a> {
    /// Build from a statement directory (`statement.jsonl`, `terminals.json`, `SHA256SUMS`) and the imported set the
    /// statement was made from. `budget_bytes`: memory budget of the parsed-record cache.
    pub fn from_statement(
        set: &'a ImportedSet,
        statement_dir: &Path,
        alg: SymbolicaAlgebra,
        lau: SymbolicaLaurent,
        budget_bytes: usize,
    ) -> Result<RecordSource<'a>, String> {
        verify_in_sums(statement_dir, "statement.jsonl")?;
        verify_in_sums(statement_dir, "terminals.json")?;
        let tj: Value = serde_json::from_str(
            &std::fs::read_to_string(statement_dir.join("terminals.json")).map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("terminals.json: {e}"))?;
        let terminals: Vec<Vec<i32>> = tj["terminals"]
            .as_array()
            .ok_or("terminals.json: no terminals list")?
            .iter()
            .map(|t| t.as_str().and_then(parse_key_text).or_else(|| parse_key(t)))
            .collect::<Option<_>>()
            .ok_or("terminals.json: a terminal is not a key")?;
        let mut s = RecordSource {
            set,
            alg,
            lau,
            keys: vec![],
            index: HashMap::new(),
            st: vec![],
            server: vec![],
            in_start: vec![],
            terminals: terminals.clone(),
            sectors: Sectors::new(set.frame.mask().to_vec()),
            cache: HashMap::new(),
            cache_bytes: 0,
            budget_bytes,
            tick: 0,
            stats: SourceStats::default(),
        };
        let text = std::fs::read_to_string(statement_dir.join("statement.jsonl")).map_err(|e| e.to_string())?;
        for (ln, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let v: Value = serde_json::from_str(line).map_err(|e| format!("statement line {}: {e}", ln + 1))?;
            let key = parse_key(&v["key"]).ok_or_else(|| format!("statement line {}: key", ln + 1))?;
            if key.len() != set.arity() {
                return Err(format!("statement line {}: key arity {} != {}", ln + 1, key.len(), set.arity()));
            }
            let srv = &v["server"];
            let label = format!(
                "{}#{} {}",
                srv["source"].as_str().unwrap_or("-"),
                srv["label"],
                srv["hand_name"].as_str().unwrap_or("")
            );
            let st = match v["status"].as_str() {
                Some("terminal") => match terminals.iter().position(|t| *t == key) {
                    Some(j) => St::Terminal(j),
                    None => return Err(format!("statement line {}: terminal row not in terminals.json", ln + 1)),
                },
                Some("served") => {
                    let sha = srv["record_sha256"].as_str().unwrap_or("");
                    match (srv["kind"].as_str(), set.find_sha(sha)) {
                        (Some("rule"), Some((table, record))) => St::Rule { table, record },
                        (Some("credit"), Some((table, record))) => {
                            let images = s.credit_images(&key, table, record);
                            if images.is_empty() {
                                St::Open("credit image not reproduced".into())
                            } else {
                                St::Credit { table, record, images }
                            }
                        }
                        (Some(k), None) if k == "rule" || k == "credit" => {
                            St::Open(format!("record {sha} not dispatched by the set"))
                        }
                        (k, _) => St::Open(format!("server kind {k:?} not composed by this source")),
                    }
                }
                other => St::Open(format!("statement status {other:?}")),
            };
            match &st {
                St::Terminal(_) => s.stats.terminals += 1,
                St::Rule { .. } => s.stats.served_rule += 1,
                St::Credit { .. } => s.stats.served_credit += 1,
                St::Open(r) => {
                    s.stats.open += 1;
                    if r.starts_with("credit") {
                        s.stats.credit_unresolved += 1;
                    }
                }
            }
            let in_start = v["in_start"].as_bool().unwrap_or(false);
            s.stats.statement_rows += 1;
            s.stats.box_keys += usize::from(in_start);
            let i = s.keys.len();
            if s.index.insert(key.clone(), i).is_some() {
                return Err(format!("statement line {}: key {key:?} twice", ln + 1));
            }
            s.keys.push(key);
            s.st.push(st);
            s.server.push(label);
            s.in_start.push(in_start);
        }
        for t in &terminals {
            if !s.index.contains_key(t) {
                return Err(format!("terminal {t:?} has no statement row"));
            }
        }
        Ok(s)
    }

    /// Candidate images of a credited key whose first-match server is (table, record), in group order.
    fn credit_images(&self, key: &[i32], table: usize, record: usize) -> Vec<Vec<i32>> {
        let Some(pol) = self.set.credits.as_ref() else {
            return vec![];
        };
        let lines = self.set.frame.mask();
        let has_num = key.iter().zip(lines).any(|(&v, &l)| !l && v != 0);
        let mut out = vec![];
        for (ei, g) in pol.group().iter().enumerate() {
            let image = if has_num && !pol.pure_numerator_maps().contains(&ei) {
                pol.partial_image(ei, key, lines)
            } else {
                apply_framed(g, key, lines)
            };
            let Some(image) = image else { continue };
            if image == key || self.terminals.contains(&image) {
                continue;
            }
            let at = match self.set.disp.first_match(&image) {
                Ok(Dispatch::Covered(r)) => (r.table, r.record),
                Ok(Dispatch::Blocked { at, .. }) => (at.table, at.record),
                _ => continue,
            };
            if at == (table, record) && !out.contains(&image) {
                out.push(image);
            }
        }
        out
    }

    pub fn terminals(&self) -> &[Vec<i32>] {
        &self.terminals
    }
    pub fn n_terminals(&self) -> usize {
        self.terminals.len()
    }
    pub fn index_of(&self, k: &[i32]) -> Option<usize> {
        self.index.get(k).copied()
    }
    /// Key indices of the statement's start (box) rows, in statement order.
    pub fn box_keys(&self) -> Vec<usize> {
        (0..self.in_start.len()).filter(|&i| self.in_start[i]).collect()
    }
    pub fn lines(&self) -> &[bool] {
        self.set.frame.mask()
    }

    fn intern_outside(&mut self, k: Vec<i32>) -> usize {
        if let Some(&i) = self.index.get(&k) {
            return i;
        }
        let i = self.keys.len();
        self.index.insert(k.clone(), i);
        self.keys.push(k);
        self.st.push(St::Open("term outside the statement's closure".into()));
        self.server.push(String::new());
        self.in_start.push(false);
        self.stats.outside_statement_terms += 1;
        i
    }

    fn entry(&mut self, table: usize, record: usize) -> Result<(), String> {
        self.tick += 1;
        if let Some(e) = self.cache.get_mut(&(table, record)) {
            e.last = self.tick;
            return Ok(());
        }
        let info = self.set.rule(table, record);
        let (line, sha) = line_at(Path::new(&info.path), info.offset)?;
        let want = info.record_sha256.trim_start_matches("sha256:");
        if sha.trim_start_matches("sha256:") != want {
            return Err(format!("record at {}@{}: sha256 {sha} != {want}", info.path, info.offset));
        }
        let rt = RuleText::parse(&line, &self.set.frame).map_err(|e| e.to_string())?;
        let domain = rt.domain.clone().ok_or_else(|| {
            format!("record {want}: no case domain ({})", rt.domain_error.as_deref().unwrap_or("?"))
        })?;
        let arity = self.set.arity();
        let mut patterns = Vec::with_capacity(rt.rhs.len());
        let mut texts = Vec::with_capacity(rt.rhs.len());
        let mut bytes = 0usize;
        for (p, c) in rt.rhs {
            patterns.push(parse_rhs_pattern(&p, arity)?);
            bytes += c.len() + 8 * arity;
            texts.push(c);
        }
        self.stats.records_read += 1;
        self.cache_bytes += bytes;
        self.cache.insert((table, record), Entry { domain, patterns, texts, parsed: HashMap::new(), bytes, last: self.tick });
        self.evict();
        Ok(())
    }

    fn evict(&mut self) {
        while self.cache_bytes > self.budget_bytes && self.cache.len() > 1 {
            let Some((&k, _)) = self.cache.iter().min_by_key(|(_, e)| e.last) else { break };
            if let Some(e) = self.cache.remove(&k) {
                self.cache_bytes -= e.bytes;
                self.stats.records_evicted += 1;
            }
        }
    }

    /// The top terms of record (table, record) applied at `point_key` (the key, or a credit's image).
    fn instantiate(
        &mut self,
        table: usize,
        record: usize,
        point_key: &[i32],
    ) -> Result<Vec<(Vec<i32>, TCoeff)>, String> {
        self.entry(table, record)?;
        let arity = self.set.arity();
        let mut pt = vec![];
        {
            let e = &self.cache[&(table, record)];
            if !rule_point(&e.domain, point_key, &mut pt) {
                return Err(format!("no rule point at {point_key:?}"));
            }
        }
        self.stats.instantiations += 1;
        let n = self.cache[&(table, record)].patterns.len();
        let mut out = vec![];
        let mut added = 0usize;
        for j in 0..n {
            let tk: Option<Vec<i32>> = {
                let e = &self.cache[&(table, record)];
                e.patterns[j]
                    .iter()
                    .map(|s| match *s {
                        RhsSlot::Param { param, offset } => pt.get(param)?.checked_add(offset),
                        RhsSlot::Const(c) => Some(c),
                    })
                    .collect()
            };
            let tk = tk.ok_or_else(|| format!("term {j}: key out of range"))?;
            match self.sectors.classify_masked(&tk, 0) {
                TermClass::Top => {}
                TermClass::PositiveNumerator { .. } => {
                    self.stats.positive_numerator_terms_skipped += 1;
                    continue;
                }
                _ => {
                    self.stats.subsector_terms_dropped += 1;
                    continue;
                }
            }
            let parsed = {
                let e = self.cache.get_mut(&(table, record)).expect("entry");
                if !e.parsed.contains_key(&j) {
                    let text = e.texts[j].clone();
                    let r = parse_coefficient(&self.alg, &text, arity);
                    if let Ok((a, b)) = &r {
                        let b2 = 40 * (a.nterms() + b.nterms()) * (arity + 2);
                        e.bytes += b2;
                        added += b2;
                    }
                    e.parsed.insert(j, r);
                    self.stats.coefficients_parsed += 1;
                }
                e.parsed[&j].clone()
            };
            let (num, den) = parsed.map_err(|e| format!("term {j}: {e}"))?;
            let c = self.lau.coeff_at_key(&num, &den, arity, &pt).map_err(|e| format!("term {j}: {e}"))?;
            out.push((tk, c));
        }
        self.cache_bytes += added;
        self.evict();
        Ok(out)
    }

    fn relation_of(&mut self, k: usize) -> Result<Terms<TCoeff>, String> {
        let key = self.keys[k].clone();
        let terms = match self.st[k].clone() {
            St::Rule { table, record } => self.instantiate(table, record, &key)?,
            St::Credit { table, record, images } => {
                // the first image whose composed relation descends below the ORIGINAL key (non-zero top terms)
                let mut chosen = None;
                for im in &images {
                    let t = self.instantiate(table, record, im)?;
                    let ok = t
                        .iter()
                        .all(|(tk, c)| matches!(c, TCoeff::Zero) || self.set.cmp.lower(tk, &key));
                    if ok {
                        chosen = Some(t);
                        break;
                    }
                }
                chosen.ok_or("no credit image descends below the key")?
            }
            St::Terminal(_) => return Err("a terminal has no relation".into()),
            St::Open(r) => return Err(r),
        };
        Ok(terms.into_iter().map(|(tk, c)| (self.intern_outside(tk), c)).collect())
    }

    /// The FULL local profile of the relation serving key `k` (every non-zero term: top AND subsector; zero-sector and
    /// positive-numerator terms skipped): the per-relation census of POLES_OBJECTIVE 2A ("route poles"), never a verdict
    /// on the key (TRICKS 16). Parses every term of the record (cached).
    pub fn local_profile_full(&mut self, k: usize, worst: usize) -> Result<LocalProfile, String> {
        let key = self.keys[k].clone();
        let (table, record, point) = match self.st[k].clone() {
            St::Rule { table, record } => (table, record, key.clone()),
            St::Credit { table, record, images } => (table, record, images[0].clone()),
            _ => return Err("not served".into()),
        };
        self.entry(table, record)?;
        let arity = self.set.arity();
        let mut pt = vec![];
        if !rule_point(&self.cache[&(table, record)].domain, &point, &mut pt) {
            return Err(format!("no rule point at {point:?}"));
        }
        let n = self.cache[&(table, record)].patterns.len();
        let mut terms = Vec::with_capacity(n);
        for j in 0..n {
            let tk: Option<Vec<i32>> = self.cache[&(table, record)].patterns[j]
                .iter()
                .map(|s| match *s {
                    RhsSlot::Param { param, offset } => pt.get(param)?.checked_add(offset),
                    RhsSlot::Const(c) => Some(c),
                })
                .collect();
            let tk = tk.ok_or_else(|| format!("term {j}: key out of range"))?;
            let kind = match self.sectors.classify_masked(&tk, 0) {
                TermClass::Top => TermKind::Top,
                TermClass::Subsector { .. } => TermKind::Subsector,
                _ => continue,
            };
            let parsed = {
                let e = self.cache.get_mut(&(table, record)).expect("entry");
                if !e.parsed.contains_key(&j) {
                    let text = e.texts[j].clone();
                    let r = parse_coefficient(&self.alg, &text, arity);
                    e.parsed.insert(j, r);
                    self.stats.coefficients_parsed += 1;
                }
                e.parsed[&j].clone()
            };
            let (num, den) = parsed.map_err(|e| format!("term {j}: {e}"))?;
            let c = self.lau.coeff_at_key(&num, &den, arity, &pt).map_err(|e| format!("term {j}: {e}"))?;
            terms.push(TermVal { key: tk, kind, val: c.valuation() });
        }
        Ok(LocalProfile::of(&terms, worst))
    }

    /// The server label and (table, record) of a served key (census grouping).
    pub fn record_of(&self, k: usize) -> Option<(usize, usize)> {
        match &self.st[k] {
            St::Rule { table, record } | St::Credit { table, record, .. } => Some((*table, *record)),
            _ => None,
        }
    }

    /// Cache counters (reports).
    pub fn cache_state(&self) -> (usize, usize) {
        (self.cache.len(), self.cache_bytes)
    }

    /// Server words of the statement rows by kind (reports).
    pub fn kinds(&self) -> BTreeMap<&'static str, usize> {
        let mut m = BTreeMap::new();
        for s in &self.st {
            *m.entry(match s {
                St::Terminal(_) => "terminal",
                St::Rule { .. } => "rule",
                St::Credit { .. } => "credit",
                St::Open(_) => "open",
            })
            .or_default() += 1;
        }
        m
    }
}

/// A coefficient text as numerator / denominator over Z in n0..n{N-1}, d (the importer's split rule when it applies,
/// else the whole text; the engine's parse path; the exact quotient of the two sides, never reduced: valuations and
/// expansions are invariant under a common factor).
pub fn parse_coefficient(alg: &SymbolicaAlgebra, text: &str, arity: usize) -> Result<(ZPoly, ZPoly), String> {
    let Ok((nt, dt)) = split_ratio(text.trim()) else {
        // not of the importer's `(num)/(den)` forms: the whole text is one rational function (same value)
        return alg.parse_ratio(text.trim(), arity);
    };
    let (nn, nd) = alg.parse_ratio(nt.trim(), arity)?;
    match dt {
        None => Ok((nn, nd)),
        Some(dt) => {
            let (dn, dd) = alg.parse_ratio(dt.trim(), arity)?;
            if dn.is_zero() {
                return Err("denominator text is zero".into());
            }
            Ok((&nn * &dd, &nd * &dn))
        }
    }
}

impl ReductionSource for RecordSource<'_> {
    type Coeff = TCoeff;
    fn n_keys(&self) -> usize {
        self.keys.len()
    }
    fn key(&self, i: usize) -> Vec<i32> {
        self.keys[i].clone()
    }
    fn status(&self, i: usize) -> KeyStatus {
        match &self.st[i] {
            St::Terminal(j) => KeyStatus::Terminal(*j),
            St::Rule { .. } | St::Credit { .. } => KeyStatus::Served,
            St::Open(r) => KeyStatus::Open(r.clone()),
        }
    }
    fn server(&self, i: usize) -> String {
        self.server[i].clone()
    }
    fn relations(
        &mut self,
        keys: &[usize],
        visit: &mut dyn FnMut(usize, Result<Terms<TCoeff>, String>),
    ) -> Result<(), String> {
        // record-grouped order (one parse per record and batch while it stays in the cache)
        let mut order: Vec<usize> = keys.to_vec();
        order.sort_by_key(|&k| match &self.st[k] {
            St::Rule { table, record } | St::Credit { table, record, .. } => (*table, *record, k),
            _ => (usize::MAX, usize::MAX, k),
        });
        for k in order {
            let r = self.relation_of(k);
            visit(k, r);
        }
        Ok(())
    }
}
