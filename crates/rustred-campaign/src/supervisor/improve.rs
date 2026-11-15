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
    /// The new rule's record line, kept only when it qualifies as a replacement.
    pub new_line: Option<String>,
    /// Ok = qualifies as a replacement; else why not.
    pub verdict: String,
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
    /// (rule position, candidates) to try in this run.
    targets: Vec<(usize, Vec<String>)>,
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
            let targets = im::targets(&rules, &record, &cfg.improve);
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
        let mut repl: BTreeMap<i64, String> = BTreeMap::new();
        let mut chosen: BTreeMap<i64, String> = BTreeMap::new();
        for (idx, v) in &per_rule {
            let best = v
                .iter()
                .filter(|a| a.verdict == "ok" && a.new_line.is_some())
                .min_by_key(|a| {
                    let r = a.new_rule.as_ref().expect("ok verdict has a rule");
                    (r.monomials, r.bytes, a.cand.clone())
                });
            if let Some(b) = best {
                repl.insert(*idx, b.new_line.clone().expect("line"));
                chosen.insert(*idx, b.cand.clone());
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
            let f = std::fs::File::open(&self.nodes[i].e.table.path).map_err(|e| e.to_string())?;
            for l in std::io::BufReader::new(f).lines() {
                let l = l.map_err(|e| e.to_string())?;
                let (out, old) = im::apply(std::slice::from_ref(&l), &repl)?;
                writeln!(wt, "{}", out[0]).map_err(|e| e.to_string())?;
                old_lines.extend(old);
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
                        monomials: a.new_rule.as_ref().map(|r| r.monomials),
                        bytes: a.new_rule.as_ref().map(|r| r.bytes),
                        cpu_ms: j.map(|j| j.cpu_ms),
                        verdict: if chosen.get(&e.index) == Some(&a.cand) {
                            "replaced".into()
                        } else {
                            a.verdict.clone()
                        },
                    });
                }
            }
            if let Some(c) = chosen.get(&e.index) {
                let old = &old_by_index[&e.index];
                e.chosen = format!("rule:{c}");
                e.why = format!(
                    "improve run {run}: {} -> {} monomials, {} -> {} bytes; exceptions a subset of the old rule's; same dispatch position",
                    old.monomials, e.monomials, old.bytes, e.bytes
                );
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
            "nothing_to_try": targets.is_empty()});
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
                    args.extend(["--descend".to_string(), "open".to_string()]);
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
                            work: WorkBudgets::default(),
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
        for l in &done.canonical_lines {
            if let Ok(Some(r)) = im::TableRule::from_line(l)
                && r.case == old.case
            {
                new_rule = Some(r);
                new_line = Some(l.clone());
                break;
            }
        }
        let status = match c.kind {
            OutcomeKind::KilledMemory => "lost_at_cap".to_string(),
            OutcomeKind::KilledTime => "wall".to_string(),
            k => format!("{k:?}").to_lowercase(),
        };
        let verdict = match &new_rule {
            Some(n) => match im::replacement_verdict(&old, n, self.cfg.improve.min_gain) {
                Ok(()) => "ok".to_string(),
                Err(w) => w,
            },
            None => format!("no_rule ({status})"),
        };
        if verdict != "ok" {
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
            verdict,
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
