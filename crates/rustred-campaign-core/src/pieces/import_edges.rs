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
//! Non-zero-ness comes from the evidence's coefficient facts (the FIRST route): `NonZero` and `Unknown` keep the term as
//! an edge; a first-route `Zero` drops it ONLY when an independent SECOND route confirms it (audit r2-planners G1; the
//! closing loop's rule "zeros need a second route", [`crate::import::zero_proofs::confirm_first_route_zero`]): closeloop4's
//! term-by-term comparison (a confirmed basis word in the evidence) or a structural zero proof of the term at the key
//! ([`FactSource::zero_proof`], e.g. `ZeroProofFacts`) when the first route's producer is one the zeros route is
//! independent of. An unconfirmed zero stays an EDGE ([`NonZero::ZeroUnconfirmed`]): non-descending, the gate refuses
//! with `all_zeros_unconfirmed:N`. A structural proof alone (no first-route zero) drops nothing. The gate refuses a
//! check of ANOTHER table set (`check_of_another_table_set`, G2). Relation right-hand sides list the terms of an exact
//! relation: kept as edges. The
//! provenance of each relation is `<source class>|<server kind>|<term class>` (term class from the frame's line
//! mask: `top`, `positive_numerator`, `lower_sector`), so the report breaks offenders down by both.

use std::io::BufRead;

use serde::Serialize;
use serde_json::Value;

use super::NonZero;
use super::gate::{GateConfig, GateReport, ShipGate};
use crate::closure::{CoefFact, FactSource, RuleId};
use crate::import::zero_proofs::confirm_first_route_zero;
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
    /// audit r2-planners G1: first-route ZERO terms by confirmation (`confirmed_basis`, `zeros_route`: dropped;
    /// `second_route_unconfirmed`, `second_route_basis_unknown`: kept as edges).
    pub zero_terms: std::collections::BTreeMap<String, u64>,
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

/// The terms of (table, record) at `applied` with their non-zero-ness: the evidence `ev` is the FIRST route; a first-route
/// zero is a (dropped) `Zero` only when confirmed (closeloop4's confirmed basis in `ev`, or `facts`' structural proof).
#[allow(clippy::too_many_arguments)]
fn rule_terms(
    set: &ImportedSet,
    ev: &Evidence,
    facts: &dyn FactSource,
    table: usize,
    record: usize,
    applied: &[i32],
    out: &mut Vec<(Vec<i32>, NonZero)>,
    zeros: &mut std::collections::BTreeMap<String, u64>,
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
        let f = match ev.coefficient(&rid, applied, j, &tk) {
            CoefFact::NonZero => NonZero::Yes,
            CoefFact::Unknown => NonZero::Unknown,
            CoefFact::Zero => {
                let c =
                    confirm_first_route_zero(ev.fact_basis(&rid, applied), facts, &rid, applied, j);
                *zeros.entry(c.word().to_string()).or_default() += 1;
                if c.confirmed() {
                    NonZero::Zero
                } else {
                    NonZero::ZeroUnconfirmed
                }
            }
        };
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
            if !rule_terms(set, ev, facts, t, r, &key, &mut em.buf, &mut st.zero_terms)? {
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
                if !rule_terms(
                    set,
                    ev,
                    facts,
                    *table,
                    *record,
                    &row.key,
                    &mut em.buf,
                    &mut st.zero_terms,
                )? {
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
                if !rule_terms(
                    set,
                    ev,
                    facts,
                    *table,
                    *record,
                    img,
                    &mut em.buf,
                    &mut st.zero_terms,
                )? {
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
/// on its decided check), with the evidence's own coefficient facts.
pub fn gate_check(
    set: &ImportedSet,
    ev: &Evidence,
    res: &CheckResult,
    cfg: GateConfig,
) -> Result<(GateReport, ShippedStats), String> {
    gate_check_with(set, ev, ev, res, cfg)
}

/// [`gate_check`] with a SECOND-ROUTE source for the first-route zero terms (e.g. the evidence wrapped by the structural
/// zero proofs, `import::zero_proofs::ZeroProofFacts`: its [`FactSource::zero_proof`]). The first route is always the
/// evidence `ev`. Refuses (`Err`, typed `check_of_another_table_set`) a check whose spec digest is not the set's (G2).
pub fn gate_check_with(
    set: &ImportedSet,
    ev: &Evidence,
    facts: &dyn FactSource,
    res: &CheckResult,
    cfg: GateConfig,
) -> Result<(GateReport, ShippedStats), String> {
    // audit r2-planners G2: the check must be of THIS table set
    if res.spec_digest != set.report.spec_digest {
        return Err(format!(
            "check_of_another_table_set: the check's spec digest {} is not the gated set's {}: refused",
            res.spec_digest, set.report.spec_digest
        ));
    }
    let mut gate = ShipGate::new(&set.cmp, set.arity(), cfg).map_err(|e| e.to_string())?;
    let mut err: Option<String> = None;
    let stats = {
        let mut sink =
            |key: &[i32], prov: &str, terms: &[(Vec<i32>, NonZero)]| -> Result<(), String> {
                gate.push_relation(key, prov, terms.iter().map(|(k, n)| (k.as_slice(), *n)))
                    .map_err(|e| e.to_string())
            };
        match shipped_relations(set, ev, facts, res, &mut sink) {
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

/// audit r2-planners G2: the TABLE-SET digest of an imported set, `sha256:<hex>` of its PINNED TABLE LIST: per source in
/// spec order one line `<id>\t<class>\t<kind>` followed by one line `\t<file sha256>` per file (the files' content
/// pins; paths do not enter). A plan's input carries it ([`super::plan::PlanInput::table_set_digest`]).
pub fn table_set_digest(set: &ImportedSet) -> String {
    let mut h = crate::closure::sha256::Sha256::new();
    h.update(b"pieces.table-set.v1\n");
    for s in &set.report.sources {
        h.update(format!("{}\t{}\t{}\n", s.id, s.class, s.kind).as_bytes());
        for (_, sha) in &s.files {
            h.update(format!("\t{sha}\n").as_bytes());
        }
    }
    format!("sha256:{}", h.finish_hex())
}

/// audit r2-planners G2: a plan is gated / shipped only against the table set it was made for: `Err` (typed
/// `plan_table_set_unstated` | `plan_of_another_table_set`) otherwise.
pub fn check_plan_table_set(plan: &super::plan::Plan, set: &ImportedSet) -> Result<(), String> {
    let want = table_set_digest(set);
    match &plan.table_set_digest {
        None => Err(format!(
            "plan_table_set_unstated: the plan names no table set (this set is {want}): refused"
        )),
        Some(d) if *d != want => Err(format!(
            "plan_of_another_table_set: the plan was made for {d}, the set is {want}: refused"
        )),
        Some(_) => Ok(()),
    }
}
