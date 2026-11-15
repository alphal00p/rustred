//! What a composed spec SHIPS for a node, as relations for the gate: every served row of an import check of the
//! spec, instantiated at its key.
//!
//! - a table rule: the rule instantiated at the key;
//! - a symmetry credit: the IMAGE rule instantiated at the image, its terms attached to the ORIGINAL key (the composite;
//!   the alias edge `K -> gK` is never an edge);
//! - a relation (supplied point relation or a recipe's solved relation): its right-hand side at the key (or image);
//! - a relation the import REFUSED for not descending (open `recipeunverified`): its right-hand side as well (it was
//!   handed to the import; the gate reports it).
//!
//! Non-zero-ness comes from the evidence's coefficient facts ([`FactSource`]): `Zero` drops the term, `NonZero` and
//! `Unknown` keep it as an edge. Relation right-hand sides list the terms of an exact relation: kept as edges. The
//! provenance of each relation is `<source class>|<server kind>|<term class>` (term class from the frame's line
//! mask: `top`, `positive_numerator`, `lower_sector`), so the report breaks offenders down by both.

use std::io::BufRead;

use serde::Serialize;
use serde_json::Value;

use super::NonZero;
use super::gate::{GateConfig, GateReport, ShipGate};
use crate::closure::{CoefFact, FactSource, RuleId};
use crate::import::{CheckResult, Evidence, ImportedSet, RowStatus, Server};

/// Counts of what was enumerated.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ShippedStats {
    pub rule_rows: u64,
    pub credit_rows: u64,
    pub relation_rows: u64,
    pub refused_relation_rows: u64,
    pub open_rows: u64,
    pub terminal_rows: u64,
    /// Served rows whose record has no body or whose relation is missing from the evidence (no edges known).
    pub no_body: u64,
    pub terms: u64,
}

fn term_class(lines: &[bool], k: &[i32]) -> &'static str {
    if k.iter().zip(lines).any(|(&x, &l)| !l && x > 0) {
        "positive_numerator"
    } else if k
        .iter()
        .zip(lines)
        .all(|(&x, &l)| if l { x >= 1 } else { x <= 0 })
    {
        "top"
    } else {
        "lower_sector"
    }
}

fn nz(f: CoefFact) -> NonZero {
    match f {
        CoefFact::Zero => NonZero::Zero,
        CoefFact::NonZero => NonZero::Yes,
        CoefFact::Unknown => NonZero::Unknown,
    }
}

/// A relation handed to the sink: key, provenance, terms.
pub type RelationSink<'a> =
    dyn FnMut(&[i32], &str, &[(Vec<i32>, NonZero)]) -> Result<(), String> + 'a;

struct Emitter<'s> {
    lines: Vec<bool>,
    buf: Vec<(Vec<i32>, NonZero)>,
    by_class: std::collections::BTreeMap<&'static str, Vec<(Vec<i32>, NonZero)>>,
    sink: &'s mut RelationSink<'s>,
}

impl Emitter<'_> {
    /// Emits the relation split by term class (one relation per class, same key).
    fn emit(&mut self, key: &[i32], prefix: &str) -> Result<(), String> {
        if self.buf.is_empty() {
            return (self.sink)(key, &format!("{prefix}|none"), &[]);
        }
        for (k, n) in self.buf.drain(..) {
            let c = term_class(&self.lines, &k);
            self.by_class.entry(c).or_default().push((k, n));
        }
        for (c, terms) in std::mem::take(&mut self.by_class) {
            (self.sink)(key, &format!("{prefix}|{c}"), &terms)?;
        }
        Ok(())
    }
}

fn rule_terms(
    set: &ImportedSet,
    facts: &dyn FactSource,
    table: usize,
    record: usize,
    applied: &[i32],
    out: &mut Vec<(Vec<i32>, NonZero)>,
) -> Result<bool, String> {
    let (mut buf, mut cm) = (vec![], vec![]);
    let nt = set
        .bodies
        .instantiate(
            &set.disp,
            set.disp.rule_ref(table, record),
            applied,
            &mut buf,
            &mut cm,
        )
        .map_err(|e| format!("instantiate {table}/{record} at {applied:?}: {e:?}"))?;
    let Some(nt) = nt else { return Ok(false) };
    let i = &set.rules[table][record];
    let rid = RuleId {
        table: &i.table_name,
        record,
        label: i.label,
        digest: "",
    };
    let n = set.arity();
    for j in 0..nt {
        let tk = buf[j * n..(j + 1) * n].to_vec();
        let f = nz(facts.coefficient(&rid, applied, j, &tk));
        out.push((tk, f));
    }
    Ok(true)
}

fn relation_rhs(
    set: &ImportedSet,
    ev: &Evidence,
    name: &str,
    recipe: bool,
    at: &[i32],
) -> Option<Vec<Vec<i32>>> {
    if recipe {
        return ev
            .solved
            .get(name)
            .and_then(|m| m.get(at))
            .map(|s| s.rhs.clone());
    }
    let rs = set.supplied.iter().find(|x| x.name == name)?;
    let i = rs.relations.find(at)?;
    let st = &rs.relations;
    let w = st.codec().words();
    let mut out = vec![];
    for c in st.rhs_codes(i).chunks_exact(w) {
        out.push(st.codec().decode_vec(c));
    }
    Some(out)
}

/// Enumerates the shipped relations of a checked spec into `sink` (see the module documentation).
pub fn shipped_relations<'s>(
    set: &ImportedSet,
    ev: &Evidence,
    facts: &dyn FactSource,
    res: &CheckResult,
    sink: &'s mut RelationSink<'s>,
) -> Result<ShippedStats, String> {
    let mut st = ShippedStats::default();
    let mut em = Emitter {
        lines: set.frame.mask().to_vec(),
        buf: vec![],
        by_class: Default::default(),
        sink,
    };
    let class_of_rule = |t: usize, r: usize| set.rules[t][r].class.clone();
    // table-rule rows of a streamed check live in its rows file
    if let Some(sr) = &res.streamed {
        let f = std::fs::File::open(&sr.path).map_err(|e| format!("{}: {e}", sr.path))?;
        for (li, l) in std::io::BufReader::with_capacity(1 << 20, f)
            .lines()
            .enumerate()
        {
            let l = l.map_err(|e| format!("{}: {e}", sr.path))?;
            if l.trim().is_empty() {
                continue;
            }
            let v: Value = serde_json::from_str(&l)
                .map_err(|e| format!("{} line {}: {e}", sr.path, li + 1))?;
            if v["status"] != "served" || v["server"]["kind"] != "rule" {
                continue;
            }
            let (Some(sha), Ok(key)) = (
                v["server"]["record_sha256"].as_str(),
                serde_json::from_value::<Vec<i32>>(v["key"].clone()),
            ) else {
                return Err(format!("{} line {}: key / server record", sr.path, li + 1));
            };
            let (t, r) = set.find_sha(sha).ok_or_else(|| {
                format!("{} line {}: record {sha} not in the set", sr.path, li + 1)
            })?;
            st.rule_rows += 1;
            if !rule_terms(set, facts, t, r, &key, &mut em.buf)? {
                st.no_body += 1;
                continue;
            }
            st.terms += em.buf.len() as u64;
            em.emit(&key, &format!("{}|rule", class_of_rule(t, r)))?;
        }
    }
    for row in &res.rows {
        match (row.status, &row.server) {
            (RowStatus::Served, Some(Server::Rule { table, record })) => {
                if res.streamed.is_some() {
                    continue; // already read from the rows file
                }
                st.rule_rows += 1;
                if !rule_terms(set, facts, *table, *record, &row.key, &mut em.buf)? {
                    st.no_body += 1;
                    continue;
                }
                st.terms += em.buf.len() as u64;
                em.emit(
                    &row.key,
                    &format!("{}|rule", class_of_rule(*table, *record)),
                )?;
            }
            (RowStatus::Served, Some(Server::Credit { table, record })) => {
                st.credit_rows += 1;
                let Some(img) = &row.image else {
                    return Err(format!("credit row {:?} without its image", row.key));
                };
                if !rule_terms(set, facts, *table, *record, img, &mut em.buf)? {
                    st.no_body += 1;
                    continue;
                }
                st.terms += em.buf.len() as u64;
                em.emit(
                    &row.key,
                    &format!("{}|credit", class_of_rule(*table, *record)),
                )?;
            }
            (RowStatus::Served, Some(Server::Relation { set: name, recipe })) => {
                st.relation_rows += 1;
                let at = row.image.clone().unwrap_or_else(|| row.key.clone());
                let Some(rhs) = relation_rhs(set, ev, name, *recipe, &at) else {
                    st.no_body += 1;
                    continue;
                };
                st.terms += rhs.len() as u64;
                em.buf.extend(rhs.into_iter().map(|k| (k, NonZero::Yes)));
                let class = set
                    .classes
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| name.clone());
                let kind = if *recipe { "recipe" } else { "relation" };
                em.emit(&row.key, &format!("{class}|{kind}"))?;
            }
            (RowStatus::Open, _) if row.open_status.as_deref() == Some("recipeunverified") => {
                st.open_rows += 1;
                for rs in &set.supplied {
                    if let Some(i) = rs.relations.find(&row.key) {
                        let s = &rs.relations;
                        let w = s.codec().words();
                        for c in s.rhs_codes(i).chunks_exact(w) {
                            em.buf.push((s.codec().decode_vec(c), NonZero::Yes));
                        }
                        st.refused_relation_rows += 1;
                        st.terms += em.buf.len() as u64;
                        em.emit(&row.key, &format!("{}|refused_by_import", rs.name))?;
                    }
                }
            }
            (RowStatus::Open, _) => st.open_rows += 1,
            (RowStatus::Terminal, _) => st.terminal_rows += 1,
            (RowStatus::Served, None) => st.no_body += 1,
        }
    }
    Ok(st)
}

/// The gate over what a checked spec ships (call site: the repair flow before shipping; the closing loop may call it
/// on its decided check).
pub fn gate_check(
    set: &ImportedSet,
    ev: &Evidence,
    res: &CheckResult,
    cfg: GateConfig,
) -> Result<(GateReport, ShippedStats), String> {
    let mut gate = ShipGate::new(&set.cmp, set.arity(), cfg).map_err(|e| e.to_string())?;
    let mut err: Option<String> = None;
    let stats = {
        let mut sink =
            |key: &[i32], prov: &str, terms: &[(Vec<i32>, NonZero)]| -> Result<(), String> {
                gate.push_relation(key, prov, terms.iter().map(|(k, n)| (k.as_slice(), *n)))
                    .map_err(|e| e.to_string())
            };
        match shipped_relations(set, ev, ev, res, &mut sink) {
            Ok(s) => s,
            Err(e) => {
                err = Some(e);
                ShippedStats::default()
            }
        }
    };
    if let Some(e) = err {
        return Err(e);
    }
    let rep = gate.finish().map_err(|e| e.to_string())?;
    Ok((rep, stats))
}
