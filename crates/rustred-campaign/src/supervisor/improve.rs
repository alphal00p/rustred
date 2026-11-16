//! `ImproveSource`: IMPROVE MODE on an EXISTING table (lane sizegen; interfaces/genchoice.md 5; USER guideline
//! 2026-11-15 "ITERATABLE"). Per targeted rule (rule filter of the settings, largest first) and per rule-scope candidate
//! not yet in the improvement record: ONE generate job of that rule's case under the TABLE'S OWN order with
//! `--descend open` (only that case is solved). A candidate replaces the rule only when
//! `improve::replacement_verdict` holds (same target / case, exceptions a subset, smaller); the improved table keeps
//! every other record verbatim and the replaced rule's dispatch position; the old rules are kept aside
//! (`replaced_rules.jsonl`); the record is appended (never repeats a candidate; a second run with unchanged settings
//! does nothing). The node's closing check on the improved table (closure, terminals, statement set unchanged; rr_proof
//! green) is its acceptance (import spec written when the node names a box). Decisions are functions of the ledger
//! state and the registered answer files (interrupt + resume = same answers).

use super::genchoice::{FakeGen, blake3_hex, check_flags, fake_script, file_digests, json_bytes};
use crate::ledger::{Event, ItemEvent, NodeSpecValues};
use crate::state::{AttemptState, CampaignState, ItemPhase, NodeEvent, NodePhase, Rung};
use crate::supervisor::spec::CampaignSpec;
use crate::supervisor::worksource::{
    AcceptedRecord, CampaignOutcome, JobRequest, Routing, Settled, SourceContext, WorkSource,
};
use rustred_campaign_contract::classify::OutcomeKind;
use rustred_campaign_contract::spec::{Budgets, InputRef, JobKind, WallBudgets, WorkBudgets};
use rustred_campaign_core::economy::genchoice as gc;
use rustred_campaign_core::economy::improve as im;
use rustred_campaign_core::economy::polescore as ps;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

pub const KIND_IMPROVE: &str = "improve-rule";
pub const ROLE_IMPROVE_ANSWER: &str = "improve_answer";
pub const ROLE_IMPROVE_RESULT: &str = "improve_result";

fn input_result(node: &str) -> String {
    format!("improve:{node}:result")
}
fn input_answer(item: &str, job: &str, attempt: u32) -> String {
    format!("improve:{item}:answer:{job}:a{attempt}")
}
fn input_table(node: &str) -> String {
    format!("improve:{node}:table")
}
fn input_family(node: &str) -> String {
    format!("improve:{node}:family")
}
fn dir(node: &str) -> String {
    format!("genchoice/{node}/improve")
}
fn item_id(node: &str, index: i64, cand: &str) -> String {
    format!("{node}.im.{index}.{cand}")
}

/// The answer of one candidate job.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImproveAnswer {
    pub node: String,
    pub item: String,
    pub index: i64,
    pub cand: String,
    pub job: String,
    pub attempt: u32,
    pub class: String,
    pub status: String,
    pub new_rule: Option<im::TableRule>,
    /// The new rule's record line (single mode), kept only when it qualifies as a replacement.
    pub new_line: Option<String>,
    /// Sub-tree mode: every rule line of the complete sub-table (target case first), kept only when it qualifies.
    #[serde(default)]
    pub new_lines: Vec<String>,
    /// Sub-tree mode: summed monomials / bytes and rule count of the sub-table.
    #[serde(default)]
    pub sub_monomials: Option<u64>,
    #[serde(default)]
    pub sub_bytes: Option<u64>,
    #[serde(default)]
    pub sub_rules: Option<u64>,
    /// Ok = qualifies as a replacement; else why not.
    pub verdict: String,
    /// The job's sealed table (absolute), when it sealed one: what a pole scorer evaluates.
    #[serde(default)]
    pub table_path: Option<String>,
}

struct NodeRt {
    e: im::ImproveNode,
    family_blake3: String,
    family_bytes: u64,
    table_blake3: String,
    table_bytes: u64,
    order: String,
    rules: Vec<im::TableRule>,
    record: im::ImproveRecord,
    /// (rule position, candidates) to GENERATE in this run.
    targets: Vec<(usize, Vec<String>)>,
    /// IMPROVE POLES: (rule position, candidate, its rule-scope score line) to DECIDE in this run (no job).
    decisions: Vec<(usize, String, ps::ScoreLine)>,
    /// IMPROVE POLES: rule-scope score lines of the node (`original` per index, candidates).
    pole_scores: Vec<ps::ScoreLine>,
    /// IMPROVE POLES: rule indices without a decided own score (not targeted).
    unscored: Vec<i64>,
}

pub struct ImproveSource {
    cfg_input: String,
    cfg: im::ImproveConfig,
    /// TESTS ONLY (config key `fake`): runs keyed by rule index, then candidate name.
    fake: Option<FakeGen>,
    cdir: PathBuf,
    nodes: Vec<NodeRt>,
    answers: BTreeMap<String, ImproveAnswer>,
}

/// Rule lines of a table file, streamed (TableRule metadata only).
fn read_rules(path: &Path) -> Result<(Option<Value>, Vec<im::TableRule>), String> {
    let f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut header = None;
    let mut rules = Vec::new();
    for (n, l) in std::io::BufReader::new(f).lines().enumerate() {
        let l = l.map_err(|e| format!("{}: {e}", path.display()))?;
        if n == 0 && l.contains("\"header\"") {
            header = serde_json::from_str(&l).ok();
            continue;
        }
        if let Some(r) = im::TableRule::from_line(&l)
            .map_err(|e| format!("{} line {}: {e}", path.display(), n + 1))?
        {
            rules.push(r);
        }
    }
    Ok((header, rules))
}

impl ImproveSource {
    pub fn from_spec(
        spec: &CampaignSpec,
        cdir: &Path,
        text: &str,
        cfg_input: &str,
    ) -> Result<Self, String> {
        let mut v: Value =
            serde_json::from_str(text).map_err(|e| format!("improve config: {e}"))?;
        let fake = match v.as_object_mut().and_then(|o| o.remove("fake")) {
            Some(f) => Some(
                serde_json::from_value::<FakeGen>(f)
                    .map_err(|e| format!("improve config: fake: {e}"))?,
            ),
            None => None,
        };
        let cfg = im::ImproveConfig::parse(&v.to_string())?;
        if !spec.programs.contains_key(&cfg.program) {
            return Err(format!("improve: program {} not in the spec", cfg.program));
        }
        for c in &cfg.improve.candidates {
            check_flags(&c.args)?;
        }
        check_flags(&cfg.common_args)?;
        let total = spec.resources.mem_total.0;
        let cap = spec
            .resources
            .job_mem_cap
            .map_or(cfg.job.full_cap_bytes.unwrap_or(cfg.job.cap_bytes), |c| c.0);
        if total > 0 && cap > total {
            return Err(format!(
                "improve: a job cap of {cap} bytes exceeds the campaign memory total {total} (resources.mem_total / --mem-total)"
            ));
        }
        let mut nodes = Vec::new();
        for e in &cfg.nodes {
            let (fb, fb3, fs2) = file_digests(Path::new(&e.family))?;
            if fs2 != e.family_sha256 {
                return Err(format!(
                    "improve: {}: family sha256 {fs2} differs from the config's",
                    e.node
                ));
            }
            let (tb, tb3, ts2) = file_digests(Path::new(&e.table.path))?;
            if ts2 != e.table.sha256 {
                return Err(format!(
                    "improve: {}: table sha256 {ts2} differs from the config's",
                    e.node
                ));
            }
            let (header, rules) = read_rules(Path::new(&e.table.path))?;
            let order = header
                .as_ref()
                .and_then(im::header_order)
                .ok_or_else(|| format!("improve: {}: the table header names no node_order: refused (the replacement rules must be generated under the table's own order)", e.node))?;
            let mut record = match &e.record {
                Some(r) => {
                    let (_, _, rs2) = file_digests(Path::new(&r.path))?;
                    if rs2 != r.sha256 {
                        return Err(format!(
                            "improve: {}: record sha256 differs from the config's",
                            e.node
                        ));
                    }
                    let rec = im::ImproveRecord::parse(
                        &std::fs::read_to_string(&r.path)
                            .map_err(|er| format!("{}: {er}", r.path))?,
                    )?;
                    if rec.table_sha256 != ts2 {
                        return Err(format!(
                            "improve: {}: the record describes table sha256 {}, not this table ({ts2})",
                            e.node, rec.table_sha256
                        ));
                    }
                    rec
                }
                None => im::ImproveRecord::empty(&e.node, &ts2, &rules),
            };
            record.align(&rules);
            let pole_scores = match &e.pole_scores {
                Some(f) => {
                    let (_, _, h) = file_digests(Path::new(&f.path))?;
                    if h != f.sha256 {
                        return Err(format!(
                            "improve: {}: pole scores {} sha256 {h} differs from the config's",
                            e.node, f.path
                        ));
                    }
                    let t = std::fs::read_to_string(&f.path).map_err(|er| format!("{}: {er}", f.path))?;
                    // an `original` line describes the rule of the table it was computed on: a line bound to
                    // another table (an earlier, since improved one) is not this table's score
                    ps::parse_scores(&t)?
                        .into_iter()
                        .filter(|l| l.node == e.node)
                        .filter(|l| {
                            !(l.cand == "original" && l.table.as_ref().is_some_and(|t| t.sha256 != ts2))
                        })
                        .collect()
                }
                None => vec![],
            };
            let (targets, decisions, unscored) = if cfg.improve.poles_on() {
                let (t, unscored) = im::pole_targets(&rules, &record, &cfg.improve, &pole_scores);
                let mut gen_t = vec![];
                let mut dec = vec![];
                for (pos, steps) in t {
                    let g: Vec<String> = steps
                        .iter()
                        .filter(|(_, s)| *s == im::PoleStep::Generate)
                        .map(|(c, _)| c.clone())
                        .collect();
                    for (c, st) in steps {
                        if let im::PoleStep::Decide(l) = st {
                            dec.push((pos, c, l));
                        }
                    }
                    if !g.is_empty() {
                        gen_t.push((pos, g));
                    }
                }
                (gen_t, dec, unscored)
            } else {
                (im::targets(&rules, &record, &cfg.improve), vec![], vec![])
            };
            nodes.push(NodeRt {
                e: e.clone(),
                family_blake3: fb3,
                family_bytes: fb,
                table_blake3: tb3,
                table_bytes: tb,
                order,
                rules,
                record,
                targets,
                decisions,
                pole_scores,
                unscored,
            });
        }
        Ok(ImproveSource {
            cfg_input: cfg_input.to_string(),
            cfg,
            fake,
            cdir: cdir.to_path_buf(),
            nodes,
            answers: BTreeMap::new(),
        })
    }

    fn reg(&self, rt: &mut Routing, id: String, role: &str, rec: AcceptedRecord) {
        let f = rec.file_ref();
        rt.events.push(Event::InputRegistered {
            input: id,
            role: role.into(),
            path: self.cdir.join(&rec.path).display().to_string(),
            bytes: f.bytes,
            blake3: f.blake3,
        });
        rt.accepts.push(rec);
    }

    fn node_index(&self, node: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.e.node == node)
    }

    fn parse_label(label: &str) -> Option<(i64, String)> {
        let mut p = label.split_whitespace();
        Some((p.next()?.parse().ok()?, p.next()?.to_string()))
    }

    fn answer(&mut self, st: &CampaignState, item: &str) -> Option<ImproveAnswer> {
        if let Some(a) = self.answers.get(item) {
            return Some(a.clone());
        }
        let it = st.items.get(item)?;
        for job in it.jobs.iter().rev() {
            let j = st.jobs.get(job)?;
            for att in (0..=j.attempt).rev() {
                let id = input_answer(item, job, att);
                if let Some(r) = st.inputs.get(&id) {
                    let b = std::fs::read(&r.path).ok()?;
                    if blake3_hex(&b) != r.blake3 {
                        return None;
                    }
                    let a: ImproveAnswer = serde_json::from_slice(&b).ok()?;
                    self.answers.insert(item.to_string(), a.clone());
                    return Some(a);
                }
            }
        }
        None
    }

    fn final_phase(p: &ItemPhase) -> bool {
        matches!(
            p,
            ItemPhase::Produced
                | ItemPhase::Escalated(_)
                | ItemPhase::Skipped
                | ItemPhase::Rejected(_)
                | ItemPhase::Accepted(_)
        )
    }

    fn spent_ms(st: &CampaignState, node: &str) -> u64 {
        st.items
            .values()
            .filter(|it| it.node == node && it.kind == KIND_IMPROVE)
            .flat_map(|it| it.jobs.iter())
            .filter_map(|j| st.jobs.get(j))
            .map(|j| j.cpu_ms)
            .sum()
    }

    fn budget_spent(&self, st: &CampaignState, node: &str) -> bool {
        self.cfg
            .improve
            .budget_cpu_s
            .is_some_and(|b| Self::spent_ms(st, node) >= b.saturating_mul(1000))
    }

    /// The result of a node: improved table (when a rule was replaced), replaced rules, the appended record, the import
    /// spec; registered as `improve:<node>:result`.
    fn finish_node(
        &mut self,
        st: &CampaignState,
        i: usize,
        rt: &mut Routing,
    ) -> Result<(), String> {
        let node = self.nodes[i].e.node.clone();
        let p = self.cfg.improve.clone();
        let run = self.nodes[i].record.next_run();
        // the answers of this run, per rule index: the best qualifying candidate
        let mut per_rule: BTreeMap<i64, Vec<ImproveAnswer>> = BTreeMap::new();
        let mut untried: BTreeMap<i64, Vec<String>> = BTreeMap::new();
        let targets = self.nodes[i].targets.clone();
        for (pos, cands) in &targets {
            let idx = self.nodes[i].rules[*pos].index;
            for c in cands {
                let item = item_id(&node, idx, c);
                let fin = st
                    .items
                    .get(&item)
                    .is_some_and(|it| Self::final_phase(&it.phase));
                match (fin, self.answer(st, &item)) {
                    (true, Some(a)) => per_rule.entry(idx).or_default().push(a),
                    _ => untried.entry(idx).or_default().push(format!("{c}: not run (total budget improve.budget_cpu_s spent or no settled attempt)")),
                }
            }
        }
        // index -> replacement lines (single mode: one line; sub-tree mode: the sub-table, target case first)
        let mut repl: BTreeMap<i64, Vec<String>> = BTreeMap::new();
        let mut chosen: BTreeMap<i64, String> = BTreeMap::new();
        for (idx, v) in &per_rule {
            let best = v
                .iter()
                .filter(|a| a.verdict == "ok" && (a.new_line.is_some() || !a.new_lines.is_empty()))
                .min_by_key(|a| {
                    let r = a.new_rule.as_ref().expect("ok verdict has a rule");
                    let (m, b) = (a.sub_monomials.unwrap_or(r.monomials), a.sub_bytes.unwrap_or(r.bytes));
                    let c = &p.cost;
                    let primary = match p.filter.by {
                        im::RankBy::Monomials => m,
                        im::RankBy::Bytes => b,
                        im::RankBy::UseCost => (c.load_ns_per_byte * b as f64
                            + c.apply_ns_base * a.sub_rules.unwrap_or(1) as f64
                            + c.apply_ns_per_monomial * m as f64)
                            .round() as u64,
                    };
                    (primary, m, b, a.cand.clone())
                });
            if let Some(b) = best {
                let lines = match &b.new_line {
                    Some(l) => vec![l.clone()],
                    None => b.new_lines.clone(),
                };
                repl.insert(*idx, lines);
                chosen.insert(*idx, b.cand.clone());
            }
        }
        // IMPROVE POLES: decide the scored candidates (generated in an earlier run; never generated again)
        let mut pole_tried: BTreeMap<i64, Vec<im::Tried>> = BTreeMap::new();
        let mut pole_new: BTreeMap<i64, (ps::PoleScore, String)> = BTreeMap::new();
        if let Some(pol) = p.poles.as_ref().filter(|_| p.poles_on()) {
            let decisions = self.nodes[i].decisions.clone();
            let mut best: BTreeMap<i64, (Vec<u64>, u64, String, String, ps::PoleScore, im::TableRule)> =
                BTreeMap::new();
            for (pos, cand, line) in &decisions {
                let old = self.nodes[i].rules[*pos].clone();
                let own = self.nodes[i]
                    .pole_scores
                    .iter()
                    .find(|s| s.scope == "rule" && s.cand == "original" && s.rule_index == Some(old.index))
                    .map(|s| s.score.clone());
                let t = line.table.as_ref().expect("decide steps carry a table");
                let (_, _, h) = file_digests(Path::new(&t.path))?;
                let (verdict, new) = if h != t.sha256 {
                    (format!("table_changed (sha256 {h} vs the score's {})", t.sha256), None)
                } else {
                    // the candidate's rule as the CANONICAL line (what a size-mode replacement writes and measures)
                    let raw = std::fs::read_to_string(&t.path).map_err(|e| format!("{}: {e}", t.path))?;
                    let mut found = None;
                    for l in raw.lines() {
                        let Ok(c) = rustred_campaign_contract::canon::canonical_line(l.as_bytes()) else {
                            continue;
                        };
                        let c = String::from_utf8_lossy(&c).into_owned();
                        if let Some(r) = im::TableRule::from_line(&c)?.filter(|r| r.case == old.case) {
                            found = Some((r, c));
                            break;
                        }
                    }
                    match found {
                        Some((n, l)) => match ps::rule_pole_verdict(
                            &old,
                            own.as_ref(),
                            &n,
                            Some(&line.score),
                            p.min_gain,
                            pol,
                        ) {
                            Ok(v) => (v, Some((n, l))),
                            Err(w) => (w, None),
                        },
                        None => ("no_rule_for_the_case".to_string(), None),
                    }
                };
                pole_tried.entry(old.index).or_default().push(im::Tried {
                    cand: cand.clone(),
                    scope: "rule_poles".into(),
                    run,
                    args: vec![],
                    status: "scored".into(),
                    monomials: new.as_ref().map(|(n, _)| n.monomials),
                    bytes: new.as_ref().map(|(n, _)| n.bytes),
                    cpu_ms: None,
                    verdict: verdict.clone(),
                    table: Some(t.path.clone()),
                    pole: Some(line.score.clone()),
                });
                if let Some((n, l)) = new {
                    let key = (ps::pole_values(&line.score, pol), n.monomials, cand.clone());
                    let better = best
                        .get(&old.index)
                        .is_none_or(|b| (&key.0, key.1, &key.2) < (&b.0, b.1, &b.2));
                    if better {
                        best.insert(old.index, (key.0, key.1, key.2, l, line.score.clone(), n));
                    }
                }
            }
            for (idx, (_, _, cand, l, sc, _)) in best {
                repl.insert(idx, vec![l]);
                chosen.insert(idx, cand.clone());
                pole_new.insert(idx, (sc, cand));
            }
        }
        let tin = self.nodes[i].e.table.sha256.clone();
        let (out_sha, out_path) = if repl.is_empty() {
            (tin.clone(), self.nodes[i].e.table.path.clone())
        } else {
            // stream the old table, replacing the chosen rules at their index
            let rel_t = format!("{}/improved.r{run}.table.jsonl", dir(&node));
            let rel_o = format!("{}/replaced_rules.r{run}.jsonl", dir(&node));
            let abs_t = self.cdir.join(&rel_t);
            let abs_o = self.cdir.join(&rel_o);
            std::fs::create_dir_all(abs_t.parent().expect("dir")).map_err(|e| e.to_string())?;
            let tmp_t = abs_t.with_extension("jsonl.tmp");
            let mut wt =
                std::io::BufWriter::new(std::fs::File::create(&tmp_t).map_err(|e| e.to_string())?);
            let mut old_lines = Vec::new();
            // the new rule counts (completion markers): old rules - replaced + the replacement lines, per chart
            let mut per_chart: BTreeMap<String, u64> = BTreeMap::new();
            for r in &self.nodes[i].rules {
                if !repl.contains_key(&r.index) {
                    *per_chart.entry(r.chart.clone()).or_default() += 1;
                }
            }
            for lines in repl.values() {
                for l in lines {
                    if let Some(r) = im::TableRule::from_line(l)? {
                        *per_chart.entry(r.chart).or_default() += 1;
                    }
                }
            }
            let total: u64 = per_chart.values().sum();
            let mut next_free = self.nodes[i]
                .rules
                .iter()
                .map(|r| r.index)
                .max()
                .unwrap_or(-1)
                + 1;
            let f = std::fs::File::open(&self.nodes[i].e.table.path).map_err(|e| e.to_string())?;
            for l in std::io::BufReader::new(f).lines() {
                let l = l.map_err(|e| e.to_string())?;
                match im::TableRule::from_line(&l)? {
                    Some(r) if repl.contains_key(&r.index) => {
                        for n in im::subtree_lines(&repl[&r.index], r.index, &mut next_free)? {
                            writeln!(wt, "{n}").map_err(|e| e.to_string())?;
                        }
                        old_lines.push(l);
                    }
                    Some(_) => writeln!(wt, "{l}").map_err(|e| e.to_string())?,
                    None => writeln!(wt, "{}", im::restate_marker(&l, total, &per_chart))
                        .map_err(|e| e.to_string())?,
                }
            }
            wt.flush().map_err(|e| e.to_string())?;
            drop(wt);
            std::fs::rename(&tmp_t, &abs_t).map_err(|e| e.to_string())?;
            let mut ob = Vec::new();
            for l in &old_lines {
                ob.extend_from_slice(l.as_bytes());
                ob.push(b'\n');
            }
            crate::atomic::write_atomic(&abs_o, &ob).map_err(|e| e.to_string())?;
            let (_, _, s2) = file_digests(&abs_t)?;
            (s2, abs_t.display().to_string())
        };
        // the appended record
        let (_, new_rules) = if repl.is_empty() {
            (None, self.nodes[i].rules.clone())
        } else {
            read_rules(Path::new(&out_path))?
        };
        let mut rec = self.nodes[i].record.clone();
        let old_by_index: BTreeMap<i64, im::TableRule> = self.nodes[i]
            .rules
            .iter()
            .map(|r| (r.index, r.clone()))
            .collect();
        rec.align(&new_rules);
        for e in rec.rules.iter_mut() {
            if let Some(v) = per_rule.get(&e.index) {
                for a in v {
                    let j = st.jobs.get(&a.job);
                    e.tried.push(im::Tried {
                        cand: a.cand.clone(),
                        scope: "rule".into(),
                        run,
                        args: self
                            .cfg
                            .improve
                            .candidates
                            .iter()
                            .find(|c| c.name == a.cand)
                            .map(|c| c.args.clone())
                            .unwrap_or_default(),
                        status: a.status.clone(),
                        monomials: a.sub_monomials.or(a.new_rule.as_ref().map(|r| r.monomials)),
                        bytes: a.sub_bytes.or(a.new_rule.as_ref().map(|r| r.bytes)),
                        cpu_ms: j.map(|j| j.cpu_ms),
                        verdict: if chosen.get(&e.index) == Some(&a.cand) && !p.poles_on() {
                            "replaced".into()
                        } else {
                            a.verdict.clone()
                        },
                        table: a.table_path.clone(),
                        pole: None,
                    });
                }
            }
            let own_score = self.nodes[i]
                .pole_scores
                .iter()
                .find(|s| s.scope == "rule" && s.cand == "original" && s.rule_index == Some(e.index))
                .map(|s| s.score.clone());
            if let Some(v) = pole_tried.remove(&e.index) {
                e.tried.extend(v);
            }
            if p.poles_on() {
                e.pole = match pole_new.get(&e.index) {
                    Some((sc, _)) => Some(sc.clone()),
                    None => own_score.or(e.pole.take()),
                };
            }
            if let Some(c) = chosen.get(&e.index) {
                let old = &old_by_index[&e.index];
                e.chosen = format!("rule:{c}");
                e.why = if p.poles_on() {
                    let own = self.nodes[i]
                        .pole_scores
                        .iter()
                        .find(|s| s.scope == "rule" && s.cand == "original" && s.rule_index == Some(e.index))
                        .map(|s| s.score.clone());
                    let tag = |s: Option<&ps::PoleScore>| {
                        s.map_or("undecided".to_string(), |s| {
                            format!("max order {} on {} of {} keys", s.max_order.map_or("?".into(), |m| m.to_string()), s.keys_with_pole, s.keys)
                        })
                    };
                    format!(
                        "improve run {run} (pole objective): route poles {} -> {}; {} -> {} monomials; exceptions a subset of the old rule's; same dispatch position",
                        tag(own.as_ref()),
                        tag(pole_new.get(&e.index).map(|x| &x.0)),
                        old.monomials,
                        e.monomials
                    )
                } else if self.cfg.improve.subtree {
                    format!(
                        "improve run {run}: rule of {} monomials / {} bytes replaced at its dispatch position by a COMPLETE sub-table of {} rule(s) (first: {} monomials); the sub-table covers the whole case domain",
                        old.monomials,
                        old.bytes,
                        repl.get(&e.index).map_or(0, |v| v.len()),
                        e.monomials
                    )
                } else {
                    format!(
                        "improve run {run}: {} -> {} monomials, {} -> {} bytes; exceptions a subset of the old rule's; same dispatch position",
                        old.monomials, e.monomials, old.bytes, e.bytes
                    )
                };
            }
            if let Some(u) = untried.get(&e.index) {
                e.untried.extend(u.iter().cloned());
            }
        }
        let tb: u64 = targets
            .iter()
            .map(|(p, _)| self.nodes[i].rules[*p].bytes)
            .sum();
        let tm: u64 = targets
            .iter()
            .map(|(p, _)| self.nodes[i].rules[*p].monomials)
            .sum();
        let after: Vec<&im::RuleEntry> = rec
            .rules
            .iter()
            .filter(|e| {
                targets
                    .iter()
                    .any(|(p, _)| self.nodes[i].rules[*p].index == e.index)
            })
            .collect();
        let (out_bytes, _, _) = file_digests(Path::new(&out_path))?;
        rec.table_sha256 = out_sha.clone();
        rec.runs.push(im::RunEntry {
            run,
            mode: "improve".into(),
            settings: json!({"improve": p, "common_args": self.cfg.common_args, "order": self.nodes[i].order}),
            table_in_sha256: Some(tin.clone()),
            table_out_sha256: out_sha.clone(),
            budget_cpu_ms: p.budget_cpu_s.map(|b| b * 1000),
            spent_cpu_ms: Self::spent_ms(st, &node),
            totals: json!({"targets": targets.len(), "candidates": targets.iter().map(|(_, c)| c.len()).sum::<usize>(),
                "objective": if p.poles_on() { "poles" } else { "size" },
                "decided_from_scores": self.nodes[i].decisions.len(), "unscored_rules": self.nodes[i].unscored.len(),
                "replaced": repl.len(), "table_bytes_before": self.nodes[i].table_bytes, "table_bytes_after": out_bytes,
                "targets_bytes_before": tb, "targets_bytes_after": after.iter().map(|e| e.bytes).sum::<u64>(),
                "targets_monomials_before": tm, "targets_monomials_after": after.iter().map(|e| e.monomials).sum::<u64>()}),
        });
        rec.rank();
        let rec_rel = format!("{}/record.r{run}.json", dir(&node));
        let rec_bytes = json_bytes(&serde_json::to_value(&rec).map_err(|e| e.to_string())?);
        let rec_sha = {
            let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
            h.update(&rec_bytes);
            h.finish_hex()
        };
        rt.accepts.push(AcceptedRecord {
            path: rec_rel.clone(),
            bytes: rec_bytes,
        });
        let import = self.nodes[i]
            .e
            .r#box
            .as_ref()
            .map(|b| gc::import_spec(&node, &b.path, &b.sha256, &out_path, &out_sha));
        if let Some(s) = &import {
            rt.accepts.push(AcceptedRecord {
                path: format!("{}/import.r{run}.json", dir(&node)),
                bytes: json_bytes(s),
            });
        }
        let res = json!({"schema": "rustred.campaign.improve-result.v1", "node": node, "run": run,
            "table_in": {"path": self.nodes[i].e.table.path, "sha256": tin},
            "table_out": {"path": out_path, "sha256": out_sha, "bytes": out_bytes},
            "replaced": repl.len(), "record": {"path": self.cdir.join(&rec_rel).display().to_string(), "sha256": rec_sha},
            "import_spec": import.as_ref().map(|_| self.cdir.join(format!("{}/import.r{run}.json", dir(&node))).display().to_string()),
            "nothing_to_try": targets.is_empty() && self.nodes[i].decisions.is_empty()});
        let r = AcceptedRecord {
            path: format!("{}/result.r{run}.json", dir(&node)),
            bytes: json_bytes(&res),
        };
        self.reg(rt, input_result(&node), ROLE_IMPROVE_RESULT, r);
        Ok(())
    }
}

impl WorkSource for ImproveSource {
    fn bootstrap(
        &mut self,
        st: &CampaignState,
        _ctx: &SourceContext,
    ) -> Result<Vec<Event>, String> {
        let mut ev = Vec::new();
        for n in &self.nodes {
            let node = &n.e.node;
            for (id, role, path, bytes, b3) in [
                (
                    input_family(node),
                    "improve_family",
                    &n.e.family,
                    n.family_bytes,
                    &n.family_blake3,
                ),
                (
                    input_table(node),
                    "improve_table",
                    &n.e.table.path,
                    n.table_bytes,
                    &n.table_blake3,
                ),
            ] {
                match st.inputs.get(&id) {
                    Some(x) if &x.blake3 != b3 => {
                        return Err(format!(
                            "improve: {node}: {path} changed since the campaign started: refused"
                        ));
                    }
                    Some(_) => {}
                    None => ev.push(Event::InputRegistered {
                        input: id,
                        role: role.into(),
                        path: path.clone(),
                        bytes,
                        blake3: b3.clone(),
                    }),
                }
            }
            match st.nodes.get(node) {
                None => {
                    ev.push(Event::NodeRegistered {
                        node: node.clone(),
                        inputs: vec![
                            self.cfg_input.clone(),
                            input_family(node),
                            input_table(node),
                        ],
                        terminal_bound: None,
                        spec_box: None,
                        spec: NodeSpecValues::default(),
                        reference: None,
                    });
                    ev.push(Event::Node {
                        node: node.clone(),
                        ev: NodeEvent::Start,
                    });
                }
                Some(x) if x.state.phase == NodePhase::Registered => ev.push(Event::Node {
                    node: node.clone(),
                    ev: NodeEvent::Start,
                }),
                _ => {}
            }
            for (pos, cands) in &n.targets {
                let idx = n.rules[*pos].index;
                for c in cands {
                    let item = item_id(node, idx, c);
                    if !st.items.contains_key(&item) {
                        ev.push(Event::ItemCreated {
                            item,
                            node: node.clone(),
                            kind: KIND_IMPROVE.into(),
                            rung: Rung(0),
                            parent: None,
                            inputs: vec![input_family(node), input_table(node)],
                            label: format!("{idx} {c}"),
                            weight: 1,
                        });
                    }
                }
            }
        }
        Ok(ev)
    }

    fn ready(&mut self, st: &CampaignState, _ctx: &SourceContext, limit: usize) -> Vec<JobRequest> {
        let mut out = Vec::new();
        for i in 0..self.nodes.len() {
            let node = self.nodes[i].e.node.clone();
            if st.inputs.contains_key(&input_result(&node)) || self.budget_spent(st, &node) {
                continue;
            }
            let n = &self.nodes[i];
            for (rank, (pos, cands)) in n.targets.iter().enumerate() {
                let r = &n.rules[*pos];
                for (ci, c) in cands.iter().enumerate() {
                    let item = item_id(&node, r.index, c);
                    let Some(it) = st.items.get(&item) else {
                        continue;
                    };
                    let job = match &it.phase {
                        ItemPhase::Open => format!("{item}-t{}", it.jobs.len()),
                        ItemPhase::InProgress => match it
                            .jobs
                            .last()
                            .and_then(|j| st.jobs.get(j).map(|x| (j, x.state)))
                        {
                            Some((j, AttemptState::Queued)) => j.clone(),
                            Some((_, AttemptState::Failed(_))) => {
                                format!("{item}-t{}", it.jobs.len())
                            }
                            _ => continue,
                        },
                        _ => continue,
                    };
                    // the case input: the rule's case in the table's PRESENTED frame
                    let line =
                        json!({"frame": "PRESENTED", "fixed": r.fixed, "equations": r.equations})
                            .to_string()
                            + "\n";
                    let rel = format!("{}/cases/{}.jsonl", dir(&node), r.index);
                    let abs = self.cdir.join(&rel);
                    if std::fs::read(&abs).ok().as_deref() != Some(line.as_bytes()) {
                        if let Some(d) = abs.parent() {
                            let _ = std::fs::create_dir_all(d);
                        }
                        if crate::atomic::write_atomic(&abs, line.as_bytes()).is_err() {
                            continue;
                        }
                    }
                    let cand = self.cfg.improve.candidates.iter().find(|k| &k.name == c);
                    let mut args = vec!["--order".to_string(), n.order.clone()];
                    args.extend(cand.map(|k| k.args.clone()).unwrap_or_default());
                    // only the target case is solved; an exhausted search is "no rule" (open obligation), not an
                    // engine error (found on the real engine: `tight` exhausts at depth 3 on some cases -> exit 70)
                    if self.cfg.improve.single() {
                        args.extend(["--descend".to_string(), "open".to_string()]);
                    }
                    if !args.iter().any(|a| a == "--open-on-refusal") {
                        args.push("--open-on-refusal".to_string());
                    }
                    args.extend(self.cfg.common_args.iter().cloned());
                    let j = &self.cfg.job;
                    out.push(JobRequest {
                        job,
                        item: item.clone(),
                        rung: it.rung,
                        kind: JobKind::Generate,
                        priority: 10_000 - (rank as i64) * 10 - ci as i64 - i as i64 * 100_000,
                        cap_bytes: j.full_cap_bytes.unwrap_or(j.cap_bytes),
                        expected_peak_bytes: j
                            .full_expected_peak_bytes
                            .unwrap_or(j.expected_peak_bytes),
                        cores: 1,
                        heartbeat_s: j.heartbeat_s,
                        stop_grace_s: j.stop_grace_s,
                        program: self
                            .fake
                            .as_ref()
                            .map_or(self.cfg.program.clone(), |f| f.program.clone()),
                        args,
                        inputs: vec![
                            InputRef {
                                role: "family".into(),
                                path: n.e.family.clone(),
                                blake3: n.family_blake3.clone(),
                                upto: None,
                            },
                            InputRef {
                                role: "cases".into(),
                                path: abs.display().to_string(),
                                blake3: blake3_hex(line.as_bytes()),
                                upto: None,
                            },
                        ],
                        depends_on: vec![input_family(&node), input_table(&node)],
                        budgets: Budgets {
                            work: WorkBudgets {
                                cases: (!self.cfg.improve.single())
                                    .then_some(self.cfg.improve.subtree_max_cases),
                                ..WorkBudgets::default()
                            },
                            wall: WallBudgets {
                                case_s: self.cfg.improve.wall_case_s,
                                job_s: None,
                            },
                            mem_soft_bytes: None,
                        },
                        sample_seed: 0,
                        fake: self
                            .fake
                            .as_ref()
                            .map(|f| fake_script(f, &r.index.to_string(), c)),
                    });
                }
            }
        }
        out.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.job.cmp(&b.job)));
        out.truncate(limit);
        out
    }

    fn route(
        &mut self,
        st: &CampaignState,
        _ctx: &SourceContext,
        done: &Settled,
    ) -> Result<Routing, String> {
        let it = st.items.get(&done.item).ok_or("improve: unknown item")?;
        let i = self
            .node_index(&it.node)
            .ok_or("improve: node not in the config")?;
        let (index, cand) = Self::parse_label(&it.label).ok_or("improve: bad label")?;
        let c = &done.classification;
        if matches!(c.kind, OutcomeKind::Interrupted | OutcomeKind::Lost) {
            return Err("interrupted / lost attempts are requeued by the supervisor".into());
        }
        let old = self.nodes[i]
            .rules
            .iter()
            .find(|r| r.index == index)
            .cloned()
            .ok_or("improve: rule index unknown")?;
        let mut new_rule = None;
        let mut new_line = None;
        let mut all: Vec<(im::TableRule, String)> = Vec::new();
        for l in &done.canonical_lines {
            if let Ok(Some(r)) = im::TableRule::from_line(l) {
                if new_rule.is_none() && r.case == old.case {
                    new_rule = Some(r.clone());
                    new_line = Some(l.clone());
                }
                all.push((r, l.clone()));
            }
        }
        // sub-tree order: the target case's rule first, then the rest in file order
        if let Some(p) = all.iter().position(|(r, _)| r.case == old.case) {
            let first = all.remove(p);
            all.insert(0, first);
        }
        let status = match c.kind {
            OutcomeKind::KilledMemory => "lost_at_cap".to_string(),
            OutcomeKind::KilledTime => "wall".to_string(),
            k => format!("{k:?}").to_lowercase(),
        };
        let subtree = !self.cfg.improve.single();
        let sub_rules: Vec<im::TableRule> = all.iter().map(|(r, _)| r.clone()).collect();
        let verdict = match &new_rule {
            Some(_) if subtree && c.kind != OutcomeKind::Complete => format!(
                "subtree_incomplete ({status}: not closed within improve.subtree_max_cases)"
            ),
            Some(_) if subtree => {
                match im::subtree_verdict_by(
                    &old,
                    &sub_rules,
                    self.cfg.improve.min_gain,
                    self.cfg.improve.filter.by,
                    &self.cfg.improve.cost,
                ) {
                    Ok(()) => "ok".to_string(),
                    Err(w) => w,
                }
            }
            Some(n) => match im::replacement_verdict_by(
                &old,
                n,
                self.cfg.improve.min_gain,
                self.cfg.improve.filter.by,
                &self.cfg.improve.cost,
            ) {
                Ok(()) => "ok".to_string(),
                Err(w) => w,
            },
            None => format!("no_rule ({status})"),
        };
        let (sub_monomials, sub_bytes, sub_n) = if subtree && !sub_rules.is_empty() {
            (
                Some(sub_rules.iter().map(|r| r.monomials).sum()),
                Some(sub_rules.iter().map(|r| r.bytes).sum()),
                Some(sub_rules.len() as u64),
            )
        } else {
            (None, None, None)
        };
        // IMPROVE POLES: a fresh candidate waits for its rule-scope score (never replaces on size alone)
        let verdict = if self.cfg.improve.poles_on() && new_rule.is_some() {
            format!("pole_score_pending (size: {verdict})")
        } else {
            verdict
        };
        let run_dir = st
            .jobs
            .get(&done.job)
            .and_then(|j| j.run_dir.clone())
            .unwrap_or_default();
        let table_path = (done.table.is_some() && !run_dir.is_empty())
            .then(|| Path::new(&run_dir).join("out").join("F.table.jsonl").display().to_string());
        let mut new_lines = Vec::new();
        if verdict != "ok" {
            new_line = None;
        } else if subtree {
            new_lines = all.into_iter().map(|(_, l)| l).collect();
            new_line = None;
        }
        let a = ImproveAnswer {
            node: it.node.clone(),
            item: done.item.clone(),
            index,
            cand,
            job: done.job.clone(),
            attempt: done.attempt,
            class: format!("{:?} row {}{}: {}", c.kind, c.row, c.sub, c.why),
            status,
            new_rule,
            new_line,
            new_lines,
            sub_monomials,
            sub_bytes,
            sub_rules: sub_n,
            verdict,
            table_path,
        };
        let mut rt = Routing::default();
        let rec = AcceptedRecord {
            path: format!(
                "{}/answers/{}.{}.a{}.json",
                dir(&it.node),
                done.item,
                done.job,
                done.attempt
            ),
            bytes: json_bytes(&serde_json::to_value(&a).map_err(|e| e.to_string())?),
        };
        self.reg(
            &mut rt,
            input_answer(&done.item, &done.job, done.attempt),
            ROLE_IMPROVE_ANSWER,
            rec,
        );
        let ev = match (&done.table, a.verdict == "ok") {
            (Some(t), true) => ItemEvent::Produce {
                job: done.job.clone(),
                attempt: done.attempt,
                table: t.clone(),
            },
            _ => ItemEvent::Escalate {
                to: Rung(1),
                reason: format!("improve: {} ({})", a.verdict, a.status),
            },
        };
        rt.events.push(Event::Item {
            item: done.item.clone(),
            ev,
        });
        self.answers.insert(done.item.clone(), a);
        Ok(rt)
    }

    fn control(
        &mut self,
        st: &CampaignState,
        verb: &str,
        target: &str,
    ) -> Result<Vec<Event>, String> {
        if !st.items.contains_key(target) {
            return Err(format!("unknown item {target}"));
        }
        match verb {
            "retry" | "skip" => {
                self.answers.remove(target);
                Ok(vec![])
            }
            v => Err(format!("verb {v} not supported by the improve source")),
        }
    }

    fn poll(&mut self, st: &CampaignState, _ctx: &SourceContext) -> Result<Routing, String> {
        let mut rt = Routing::default();
        for i in 0..self.nodes.len() {
            let node = self.nodes[i].e.node.clone();
            if st.inputs.contains_key(&input_result(&node)) || !st.nodes.contains_key(&node) {
                continue;
            }
            let items: Vec<&crate::state::ItemRec> = st
                .items
                .values()
                .filter(|it| it.node == node && it.kind == KIND_IMPROVE)
                .collect();
            let expected: usize = self.nodes[i].targets.iter().map(|(_, c)| c.len()).sum();
            if items.len() < expected {
                continue;
            }
            let running = items.iter().any(|it| {
                matches!(it.phase, ItemPhase::InProgress)
                    && it
                        .jobs
                        .last()
                        .and_then(|j| st.jobs.get(j))
                        .is_some_and(|j| !matches!(j.state, AttemptState::Failed(_)))
            });
            let all_final = items.iter().all(|it| Self::final_phase(&it.phase));
            if all_final || (!running && self.budget_spent(st, &node)) {
                self.finish_node(st, i, &mut rt)?;
            }
        }
        Ok(rt)
    }

    fn finished(&self, st: &CampaignState) -> bool {
        self.nodes
            .iter()
            .all(|n| st.inputs.contains_key(&input_result(&n.e.node)))
    }

    fn outcome(&self, st: &CampaignState) -> CampaignOutcome {
        if self.finished(st) {
            CampaignOutcome::Closed
        } else {
            CampaignOutcome::Open("improve runs not finished".into())
        }
    }

    fn describe(&self) -> String {
        format!(
            "improve: {} node(s), {} rule-candidate pair(s) to try",
            self.nodes.len(),
            self.nodes
                .iter()
                .map(|n| n.targets.iter().map(|(_, c)| c.len()).sum::<usize>())
                .sum::<usize>()
        )
    }
}
