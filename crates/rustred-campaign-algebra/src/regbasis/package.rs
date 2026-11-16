//! A node SERVICE PACKAGE (`rustred.node-service-manifest.v1`: a table, a certified key list, relation-entry segments)
//! as a `ReductionSource` of lane epsdepth's composition (lane regbasis; interfaces/regbasis.md 9).
//!
//! - The TABLE is read through the strict core import (`import::load_with` on an `engine_table` source over the
//!   package table: end records, closed member list, domain binding): its records are addressed by the set's
//!   provenance (`RuleInfo`: path, offset, record sha256), parsed with `RuleText` and applied at their RULE POINT.
//! - The certified key list (`keys.jsonl`: `[key, kind, rule index | null, in_box, [D, r]]`) says which relation serves
//!   each key: `table` = the table record whose `index` is given, `recipe` / `symmetry_*` = the key's relation entry
//!   (`rustred.relation-entry.v1`, body `relation` with exact coefficients), `terminal`.
//! - WITNESS LOCI = BLOCK (coordinator decision G5): a record whose witness loci are ELIDED in the table is applied at a
//!   key only when RESTORED loci are supplied for it (by the record's `origin.name`) and none of them vanishes at the key
//!   (the locus polynomial with the key's indices put in is identically zero in d); else the key is `Open` (blocked).
//! - Only TOP-sector terms are returned (modulo sub-sectors, as lane epsdepth's statement source); positive-numerator
//!   terms are skipped (their coefficients vanish in a certified package), sub-sector terms are dropped (counted).
//!
//! Generic (L8): arity, frame, terminals, files and loci are data.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde_json::Value;

use rustred_campaign_core::accept::rule::RuleText;
use rustred_campaign_core::closure::bodies::{
    RhsSlot, Sectors, TermClass, parse_rhs_pattern, rule_point,
};
use rustred_campaign_core::cover::Dispatch;
use rustred_campaign_core::domain::Verdict;
use rustred_campaign_core::epsdepth::{KeyStatus, ReductionSource, Terms};
use rustred_campaign_core::import::ImportedSet;
use rustred_campaign_core::key::{Key, parse_key};

use crate::SymbolicaAlgebra;
use crate::epsdepth::record_source::{line_at, parse_coefficient};
use crate::epsdepth::{SymbolicaLaurent, TCoeff};

type ZPoly = symbolica::poly::polynomial::MultivariatePolynomial<
    symbolica::domains::integer::IntegerRing,
    u16,
>;

/// How a certified key is served.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Serv {
    Terminal(usize),
    /// A table record (its `index` member).
    Table(i64),
    /// A relation entry of the segments.
    Entry,
    /// A key OUTSIDE the certified list served by the package table's first match (opt-in,
    /// [`PackageSource::serve_outside_by_table`]): `(table, record)` of the strict import, as the engine dispatches.
    TableAt(usize, usize),
}

struct Rec {
    rt: RuleText,
    pats: Vec<Vec<RhsSlot>>,
    coef: HashMap<usize, Result<(ZPoly, ZPoly), String>>,
    /// Restored witness loci (parsed polynomials over Z in n0.., d) when the record's loci are elided; `None` = the
    /// record's own loci are present (the strict import's dispatch already honours them).
    restored: Option<Vec<(ZPoly, ZPoly)>>,
    elided_unrestored: bool,
}

/// Counters (reports).
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct PackageStats {
    /// Keys served by their relation entry ahead of the table (manifest `entries.over_table_keys`).
    pub over_table_entries: usize,
    pub certified_keys: usize,
    pub entries: usize,
    pub table_records: usize,
    pub instantiations: u64,
    pub entry_reads: u64,
    pub subsector_terms_dropped: u64,
    pub positive_numerator_terms_skipped: u64,
    pub blocked_by_restored_locus: u64,
    pub elided_unrestored_hits: u64,
    pub outside_certified: u64,
    /// Instantiations at keys outside the certified list served by the table's first match (opt-in).
    pub outside_served_by_table: u64,
}

/// The package as a reduction source.
pub struct PackageSource<'a> {
    set: &'a ImportedSet,
    alg: SymbolicaAlgebra,
    lau: SymbolicaLaurent,
    arity: usize,
    sectors: Sectors,
    keys: Vec<Key>,
    index: HashMap<Key, usize>,
    certified: HashMap<Key, Serv>,
    terminals: Vec<Key>,
    entries: HashMap<Key, (usize, u64)>,
    seg_files: Vec<PathBuf>,
    by_index: HashMap<i64, (usize, usize)>,
    recs: HashMap<(usize, usize), Rec>,
    restored: HashMap<String, Vec<String>>,
    /// Interned indices of the keys reported Open to the composition (outside the certified set): the exact open set
    /// of the cones composed so far (only nonzero instantiated terms are followed).
    open_seen: std::sync::Mutex<HashSet<usize>>,
    /// Serve keys outside the certified list by the table's first match (the engine's dispatch); off by default.
    outside_by_table: bool,
    fm_cache: std::sync::Mutex<HashMap<Key, Option<(usize, usize)>>>,
    pub stats: PackageStats,
}

fn read_line_at(path: &Path, offset: u64) -> Result<String, String> {
    let mut f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    f.seek(SeekFrom::Start(offset)).map_err(|e| e.to_string())?;
    let mut r = BufReader::new(f);
    let mut s = String::new();
    r.read_line(&mut s).map_err(|e| e.to_string())?;
    Ok(s)
}

impl<'a> PackageSource<'a> {
    /// Open a package. `set` = the strict import of the package table; `keys_jsonl` the certified key list;
    /// `segments` the relation-entry segment files; `restored_loci` = JSONL files of `{"name", "witness_loci": [..]}`.
    pub fn open(
        set: &'a ImportedSet,
        keys_jsonl: &Path,
        segments: &[PathBuf],
        restored_loci: &[PathBuf],
        alg: SymbolicaAlgebra,
        lau: SymbolicaLaurent,
    ) -> Result<PackageSource<'a>, String> {
        let arity = set.arity();
        let mut certified = HashMap::new();
        let mut terminals = vec![];
        let f = std::fs::File::open(keys_jsonl)
            .map_err(|e| format!("{}: {e}", keys_jsonl.display()))?;
        for (ln, line) in BufReader::new(f).lines().enumerate() {
            let line = line.map_err(|e| e.to_string())?;
            if line.trim().is_empty() {
                continue;
            }
            let v: Value =
                serde_json::from_str(&line).map_err(|e| format!("keys line {}: {e}", ln + 1))?;
            let k = v[0]
                .as_str()
                .ok_or("keys: key text")
                .and_then(|t| parse_key(t).map_err(|_| "keys: key"))?;
            let s = match v[1].as_str() {
                Some("terminal") => {
                    terminals.push(k.clone());
                    Serv::Terminal(terminals.len() - 1)
                }
                Some("table") => Serv::Table(
                    v[2].as_i64()
                        .ok_or_else(|| format!("keys line {}: rule index", ln + 1))?,
                ),
                Some(_) => Serv::Entry,
                None => return Err(format!("keys line {}: kind", ln + 1)),
            };
            certified.insert(k, s);
        }
        let mut entries = HashMap::new();
        for (fi, p) in segments.iter().enumerate() {
            let mut r = BufReader::new(
                std::fs::File::open(p).map_err(|e| format!("{}: {e}", p.display()))?,
            );
            let mut off = 0u64;
            let mut buf = String::new();
            loop {
                buf.clear();
                let n = r.read_line(&mut buf).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                if !buf.trim().is_empty() {
                    // the key member only (cheap): `"key":[..]` of the entry
                    let v: Value =
                        serde_json::from_str(&buf).map_err(|e| format!("{}: {e}", p.display()))?;
                    if v["schema"] != "rustred.relation-entry.v1" {
                        return Err(format!("{}: not a relation entry", p.display()));
                    }
                    let k: Key =
                        serde_json::from_value(v["key"].clone()).map_err(|e| e.to_string())?;
                    entries.insert(k, (fi, off));
                }
                off += n as u64;
            }
        }
        // the manifest's entry precedence: "after_table, except over_table for the keys in entries.over_table_keys"
        let mut over_table = 0usize;
        for p in segments
            .iter()
            .filter_map(|p| p.parent())
            .collect::<HashSet<_>>()
        {
            if let Ok(t) = std::fs::read_to_string(p.join("MANIFEST.json"))
                && let Ok(m) = serde_json::from_str::<Value>(&t)
                && let Some(a) = m["entries"]["over_table_keys"].as_array()
            {
                for k in a {
                    if let Some(k) = k.as_str().and_then(|t| parse_key(t).ok())
                        && entries.contains_key(&k)
                        && matches!(certified.get(&k), Some(Serv::Table(_)))
                    {
                        certified.insert(k, Serv::Entry);
                        over_table += 1;
                    }
                }
            }
        }
        let mut by_index = HashMap::new();
        let mut nrec = 0usize;
        for (t, tab) in set.rules.iter().enumerate() {
            for (r, info) in tab.iter().enumerate() {
                if let Some(ix) = info.index {
                    by_index.insert(ix, (t, r));
                }
                nrec += 1;
            }
        }
        let mut restored = HashMap::new();
        for p in restored_loci {
            let f = std::fs::File::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
            for line in BufReader::new(f).lines() {
                let line = line.map_err(|e| e.to_string())?;
                if line.trim().is_empty() {
                    continue;
                }
                let v: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
                let name = v["name"].as_str().ok_or("restored loci: name")?.to_string();
                let loci: Vec<String> =
                    serde_json::from_value(v["witness_loci"].clone()).map_err(|e| e.to_string())?;
                restored.insert(name, loci);
            }
        }
        let stats = PackageStats {
            over_table_entries: over_table,
            certified_keys: certified.len(),
            entries: entries.len(),
            table_records: nrec,
            ..Default::default()
        };
        Ok(PackageSource {
            set,
            alg,
            lau,
            arity,
            sectors: Sectors::new(set.frame.mask().to_vec()),
            keys: vec![],
            index: HashMap::new(),
            certified,
            terminals,
            entries,
            seg_files: segments.to_vec(),
            by_index,
            recs: HashMap::new(),
            restored,
            open_seen: std::sync::Mutex::new(HashSet::new()),
            outside_by_table: false,
            fm_cache: std::sync::Mutex::new(HashMap::new()),
            stats,
        })
    }

    /// Serve keys OUTSIDE the certified list by the package table's first match (strict import's dispatcher), as the
    /// engine does before it falls back to relation entries and face recipes: `Covered` serves; `Blocked` by elided
    /// witness loci serves through the restored loci (BLOCK on the locus, as for certified keys); every other dispatch
    /// (uncovered, pole / exception / bound blocks, undecided, outside the chart) leaves the key open.
    pub fn serve_outside_by_table(&mut self, on: bool) {
        self.outside_by_table = on;
    }

    /// How `k` is served: the certified list, else (opt-in) the table's first match.
    pub fn serv(&self, k: &[i32]) -> Option<Serv> {
        if let Some(s) = self.certified.get(k) {
            return Some(*s);
        }
        if !self.outside_by_table {
            return None;
        }
        if let Ok(c) = self.fm_cache.lock()
            && let Some(hit) = c.get(k)
        {
            return hit.map(|(t, r)| Serv::TableAt(t, r));
        }
        let hit = match self.set.disp.first_match(k) {
            Ok(Dispatch::Covered(rr)) => Some((rr.table, rr.record)),
            Ok(Dispatch::Blocked {
                at,
                reason: Verdict::Elided,
            }) => Some((at.table, at.record)),
            _ => None,
        };
        if let Ok(mut c) = self.fm_cache.lock() {
            c.insert(k.to_vec(), hit);
        }
        hit.map(|(t, r)| Serv::TableAt(t, r))
    }

    /// `(table, record)` of a table server.
    fn table_pos(&self, s: Serv) -> Result<(usize, usize), String> {
        match s {
            Serv::Table(ix) => self
                .by_index
                .get(&ix)
                .copied()
                .ok_or_else(|| format!("no table record with index {ix}")),
            Serv::TableAt(t, r) => Ok((t, r)),
            _ => Err("not a table server".into()),
        }
    }

    /// The keys reported Open to the composition so far (sorted), and forget them.
    pub fn take_open_keys(&mut self) -> Vec<Key> {
        let ix: Vec<usize> = match self.open_seen.get_mut() {
            Ok(set) => set.drain().collect(),
            Err(_) => vec![],
        };
        let mut v: Vec<Key> = ix.into_iter().map(|i| self.keys[i].clone()).collect();
        v.sort();
        v
    }

    pub fn terminals(&self) -> &[Key] {
        &self.terminals
    }

    /// Intern a key (any key; status from the certified list).
    pub fn intern(&mut self, k: &[i32]) -> usize {
        if let Some(&i) = self.index.get(k) {
            return i;
        }
        let i = self.keys.len();
        self.keys.push(k.to_vec());
        self.index.insert(k.to_vec(), i);
        i
    }

    pub fn index_of(&self, k: &[i32]) -> Option<usize> {
        self.index.get(k).copied()
    }

    fn rec(&mut self, t: usize, r: usize) -> Result<(), String> {
        if self.recs.contains_key(&(t, r)) {
            return Ok(());
        }
        let info = self.set.rule(t, r);
        let (line, sha) = line_at(Path::new(&info.path), info.offset)?;
        let want = info.record_sha256.trim_start_matches("sha256:");
        if sha.trim_start_matches("sha256:") != want {
            return Err(format!(
                "record at {}@{}: sha256 {sha} != {want}",
                info.path, info.offset
            ));
        }
        let rt = RuleText::parse(&line, &self.set.frame).map_err(|e| e.to_string())?;
        let pats = rt
            .rhs
            .iter()
            .map(|(p, _)| parse_rhs_pattern(p, self.arity))
            .collect::<Result<Vec<_>, _>>()?;
        let wl = &rt.value["witness_loci"];
        let elided = wl.as_str().is_some_and(|s| s.contains("elided"))
            || wl.as_array().is_some_and(|a| {
                a.iter()
                    .any(|x| x.as_str().is_some_and(|s| s.contains("elided")))
            });
        let name = rt.value["origin"]["name"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let (restored, elided_unrestored) = if elided {
            match self.restored.get(&name) {
                Some(loci) => {
                    let mut ps = vec![];
                    for l in loci {
                        ps.push(self.alg.parse_ratio(l, self.arity)?);
                    }
                    (Some(ps), false)
                }
                None => (None, true),
            }
        } else {
            (None, false)
        };
        self.recs.insert(
            (t, r),
            Rec {
                rt,
                pats,
                coef: HashMap::new(),
                restored,
                elided_unrestored,
            },
        );
        Ok(())
    }

    /// The top-sector terms of the relation serving `k` (coefficients at the key / rule point, exact).
    fn relation_of(&mut self, k: &[i32]) -> Result<Vec<(Key, TCoeff)>, String> {
        let served = self.serv(k);
        if matches!(served, Some(Serv::TableAt(..))) {
            self.stats.outside_served_by_table += 1;
        }
        match served {
            None => {
                self.stats.outside_certified += 1;
                Err("key outside the package's certified set".into())
            }
            Some(Serv::Terminal(_)) => Ok(vec![]),
            Some(s @ (Serv::Table(_) | Serv::TableAt(..))) => {
                let (t, r) = self.table_pos(s)?;
                let ix = self.set.rules[t][r].index.unwrap_or(-1);
                self.rec(t, r)?;
                let arity = self.arity;
                let mut pt = vec![];
                {
                    let e = &self.recs[&(t, r)];
                    let ok = match &e.rt.domain {
                        Some(dm) => rule_point(dm, k, &mut pt),
                        None => {
                            let fixed: Option<Vec<i32>> =
                                e.rt.record
                                    .fixed
                                    .iter()
                                    .map(|x| x.and_then(|v| i32::try_from(v).ok()))
                                    .collect();
                            if fixed.as_deref() == Some(k) {
                                pt = k.to_vec();
                                true
                            } else {
                                false
                            }
                        }
                    };
                    if !ok {
                        return Err(format!("table record {ix}: no rule point at the key"));
                    }
                    if e.elided_unrestored {
                        self.stats.elided_unrestored_hits += 1;
                        return Err(format!(
                            "table record {ix}: witness loci elided and not restored (BLOCK)"
                        ));
                    }
                    if let Some(loci) = &e.restored {
                        let vals: Vec<symbolica::domains::integer::Integer> = pt
                            .iter()
                            .map(|&x| symbolica::domains::integer::Integer::from(x as i64))
                            .collect();
                        let reps: Vec<(usize, &symbolica::domains::integer::Integer)> =
                            vals.iter().enumerate().collect();
                        for (num, _den) in loci {
                            if num.replace_multiple(&reps, false).is_zero() {
                                self.stats.blocked_by_restored_locus += 1;
                                return Err(format!(
                                    "table record {ix}: the key lies on a restored witness locus (BLOCK)"
                                ));
                            }
                        }
                    }
                }
                self.stats.instantiations += 1;
                let n = self.recs[&(t, r)].pats.len();
                let mut out = vec![];
                for j in 0..n {
                    let tk: Option<Vec<i32>> = self.recs[&(t, r)].pats[j]
                        .iter()
                        .map(|s| match *s {
                            RhsSlot::Param { param, offset } => pt.get(param)?.checked_add(offset),
                            RhsSlot::Const(c) => Some(c),
                        })
                        .collect();
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
                    let alg = &self.alg;
                    let e = self.recs.get_mut(&(t, r)).expect("record");
                    let text = e.rt.rhs[j].1.clone();
                    let pc = e
                        .coef
                        .entry(j)
                        .or_insert_with(|| parse_coefficient(alg, &text, arity));
                    let (num, den) = pc.as_ref().map_err(|er| format!("term {j}: {er}"))?;
                    let c = self.lau.coeff_at_key(num, den, arity, &pt)?;
                    if !matches!(c, TCoeff::Zero) {
                        out.push((tk, c));
                    }
                }
                Ok(out)
            }
            Some(Serv::Entry) => {
                let (fi, off) = *self
                    .entries
                    .get(k)
                    .ok_or("certified entry key without a relation entry")?;
                let line = read_line_at(&self.seg_files[fi], off)?;
                self.stats.entry_reads += 1;
                let v: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
                let body = &v["body"];
                if body["type"] != "relation" {
                    return Err(format!("entry of type {}", body["type"]));
                }
                let mut out = vec![];
                for term in body["rhs"].as_array().ok_or("entry rhs")? {
                    let tk = parse_key(term[0].as_str().ok_or("entry term key")?)
                        .map_err(|e| e.to_string())?;
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
                    let (num, den) = parse_coefficient(
                        &self.alg,
                        term[1].as_str().ok_or("entry coefficient")?,
                        self.arity,
                    )?;
                    let c = self.lau.coeff_at_key(&num, &den, self.arity, k)?;
                    if !matches!(c, TCoeff::Zero) {
                        out.push((tk, c));
                    }
                }
                Ok(out)
            }
        }
    }

    /// The TOP-sector term keys of the relation serving `k`, WITHOUT coefficients (keys-only superset: every top term is
    /// followed, also one whose coefficient vanishes at the key). The witness-locus BLOCK is applied as in the exact path.
    fn terms_keys_only(&mut self, k: &[i32]) -> Result<Vec<Key>, String> {
        match self.serv(k) {
            None => {
                self.stats.outside_certified += 1;
                Err("key outside the package's certified set".into())
            }
            Some(Serv::Terminal(_)) => Ok(vec![]),
            Some(s @ (Serv::Table(_) | Serv::TableAt(..))) => {
                let (t, r) = self.table_pos(s)?;
                let ix = self.set.rules[t][r].index.unwrap_or(-1);
                self.rec(t, r)?;
                let e = &self.recs[&(t, r)];
                let mut pt = vec![];
                let ok = match &e.rt.domain {
                    Some(dm) => rule_point(dm, k, &mut pt),
                    None => {
                        let fixed: Option<Vec<i32>> =
                            e.rt.record
                                .fixed
                                .iter()
                                .map(|x| x.and_then(|v| i32::try_from(v).ok()))
                                .collect();
                        if fixed.as_deref() == Some(k) {
                            pt = k.to_vec();
                            true
                        } else {
                            false
                        }
                    }
                };
                if !ok {
                    return Err(format!("table record {ix}: no rule point at the key"));
                }
                if e.elided_unrestored {
                    return Err(format!(
                        "table record {ix}: witness loci elided and not restored (BLOCK)"
                    ));
                }
                if let Some(loci) = &e.restored {
                    let vals: Vec<symbolica::domains::integer::Integer> = pt
                        .iter()
                        .map(|&x| symbolica::domains::integer::Integer::from(x as i64))
                        .collect();
                    let reps: Vec<(usize, &symbolica::domains::integer::Integer)> =
                        vals.iter().enumerate().collect();
                    for (num, _den) in loci {
                        if num.replace_multiple(&reps, false).is_zero() {
                            return Err(format!(
                                "table record {ix}: the key lies on a restored witness locus (BLOCK)"
                            ));
                        }
                    }
                }
                let mut out = vec![];
                for pat in &e.pats {
                    let tk: Option<Vec<i32>> = pat
                        .iter()
                        .map(|s| match *s {
                            RhsSlot::Param { param, offset } => pt.get(param)?.checked_add(offset),
                            RhsSlot::Const(c) => Some(c),
                        })
                        .collect();
                    let tk = tk.ok_or("term key out of range")?;
                    if matches!(self.sectors.classify_masked(&tk, 0), TermClass::Top) {
                        out.push(tk);
                    }
                }
                Ok(out)
            }
            Some(Serv::Entry) => {
                let (fi, off) = *self
                    .entries
                    .get(k)
                    .ok_or("certified entry key without a relation entry")?;
                let line = read_line_at(&self.seg_files[fi], off)?;
                let v: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
                let mut out = vec![];
                for term in v["body"]["rhs"].as_array().ok_or("entry rhs")? {
                    let tk = parse_key(term[0].as_str().ok_or("entry term key")?)
                        .map_err(|e| e.to_string())?;
                    if matches!(self.sectors.classify_masked(&tk, 0), TermClass::Top) {
                        out.push(tk);
                    }
                }
                Ok(out)
            }
        }
    }

    /// Keys-only SUPERSET cone of `roots` (every top-sector term followed): (cone keys, open keys, height).
    #[allow(clippy::type_complexity)]
    pub fn cone_keys_only(
        &mut self,
        roots: &[Key],
        max_keys: usize,
    ) -> Result<(usize, Vec<(Key, String)>, usize), String> {
        let mut seen: HashSet<Key> = roots.iter().cloned().collect();
        let mut frontier: Vec<Key> = roots.to_vec();
        let mut open = vec![];
        let mut height = 0usize;
        while !frontier.is_empty() && seen.len() <= max_keys {
            height += 1;
            let mut next = vec![];
            for k in &frontier {
                match self.terms_keys_only(k) {
                    Ok(t) => {
                        for c in t {
                            if seen.insert(c.clone()) {
                                next.push(c);
                            }
                        }
                    }
                    Err(e) => open.push((k.clone(), e)),
                }
            }
            frontier = next;
        }
        Ok((seen.len(), open, height))
    }
}

impl ReductionSource for PackageSource<'_> {
    type Coeff = TCoeff;
    fn n_keys(&self) -> usize {
        self.keys.len()
    }
    fn key(&self, i: usize) -> Vec<i32> {
        self.keys[i].clone()
    }
    fn status(&self, i: usize) -> KeyStatus {
        match self.serv(&self.keys[i]) {
            Some(Serv::Terminal(j)) => KeyStatus::Terminal(j),
            Some(_) => KeyStatus::Served,
            None => {
                if let Ok(mut set) = self.open_seen.lock() {
                    set.insert(i);
                }
                KeyStatus::Open("key outside the package's certified set".into())
            }
        }
    }
    fn server(&self, i: usize) -> String {
        match self.serv(&self.keys[i]) {
            Some(Serv::Table(ix)) => format!("table#{ix}"),
            Some(Serv::TableAt(t, r)) => format!("table@{t}:{r}"),
            Some(Serv::Entry) => "entry".into(),
            Some(Serv::Terminal(j)) => format!("terminal#{j}"),
            None => "none".into(),
        }
    }
    fn relations(
        &mut self,
        keys: &[usize],
        visit: &mut dyn FnMut(usize, Result<Terms<TCoeff>, String>),
    ) -> Result<(), String> {
        for &i in keys {
            let k = self.keys[i].clone();
            let r = self.relation_of(&k).map(|t| {
                t.into_iter()
                    .map(|(tk, c)| (self.intern(&tk), c))
                    .collect::<Vec<_>>()
            });
            visit(i, r);
        }
        Ok(())
    }
}

/// Read the whole of a small text file (helper for the drivers).
pub fn read_text(p: &Path) -> Result<String, String> {
    let mut s = String::new();
    std::fs::File::open(p)
        .and_then(|mut f| f.read_to_string(&mut s))
        .map_err(|e| format!("{}: {e}", p.display()))?;
    Ok(s)
}
