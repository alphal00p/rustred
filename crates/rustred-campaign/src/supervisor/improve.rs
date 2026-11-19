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
use rustred_campaign_core::economy::derivation as dv;
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
    /// The EFFECTIVE DERIVATION of every rule of the job's table (`rustred.campaign.rule-derivation.v1`).
    #[serde(default)]
    pub derivations: Vec<Value>,
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
    /// Rules inside the filter window the record already holds completely (skipped by this run).
    skipped: Vec<i64>,
    /// Reference yield of the tranche stop (bytes per CPU-h).
    ref_yield: Option<f64>,
    /// Where the reference comes from (logged with every yield line).
    ref_from: String,
    /// Last logged yield-stop state of this session: (stopped, launches held back).
    yield_logged: Option<(bool, usize)>,
    /// A partial snapshot was written at the yield stop's engagement in this session.
    yield_partial_done: bool,
}

/// Yield of a node's run so far (see `improve.yield_stop`).
#[derive(Clone, Debug, Default)]
struct YieldNow {
    saved: u64,
    spent_ms: u64,
    bytes_per_cpu_h: Option<f64>,
    stop: bool,
}

/// A sink that writes a table (or only digests it, when there is no room for the copy) and collects its rule lines'
/// metadata on the way (no re-read of a multi-GB table).
struct TableSink {
    file: Option<std::io::BufWriter<std::fs::File>>,
    sha: rustred_campaign_core::closure::sha256::Sha256,
    bytes: u64,
    rules: Vec<im::TableRule>,
    /// The native importer's order rule, enforced on every written rule (index = position in its chart).
    order: im::Positions,
}

impl TableSink {
    fn line(&mut self, l: &str) -> Result<(), String> {
        if let Some(r) = im::TableRule::from_line(l)? {
            self.order.take(&r.chart, r.index)?;
            self.rules.push(r);
        }
        self.sha.update(l.as_bytes());
        self.sha.update(b"\n");
        self.bytes += l.len() as u64 + 1;
        if let Some(f) = self.file.as_mut() {
            writeln!(f, "{l}").map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

pub struct ImproveSource {
    cfg_input: String,
    cfg: im::ImproveConfig,
    /// The engine binary (path, sha256) recorded in every derivation.
    engine: (String, String),
    /// TESTS ONLY (config key `fake`): runs keyed by rule index, then candidate name.
    fake: Option<FakeGen>,
    cdir: PathBuf,
    /// The spec's disk floor: a partial snapshot never copies the table below it.
    disk_floor: u64,
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
    im::assign_occurrences(&mut rules);
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
            if let Some(f) = &e.dependences {
                let (_, _, h) = file_digests(Path::new(&f.path))?;
                if h != f.sha256 {
                    return Err(format!(
                        "improve: {}: dependence table {} sha256 {h} differs from the config's",
                        e.node, f.path
                    ));
                }
            }
            let pole_scores = match &e.pole_scores {
                Some(f) => {
                    let (_, _, h) = file_digests(Path::new(&f.path))?;
                    if h != f.sha256 {
                        return Err(format!(
                            "improve: {}: pole scores {} sha256 {h} differs from the config's",
                            e.node, f.path
                        ));
                    }
                    let t = std::fs::read_to_string(&f.path)
                        .map_err(|er| format!("{}: {er}", f.path))?;
                    // an `original` line describes the rule of the table it was computed on: a line bound to
                    // another table (an earlier, since improved one) is not this table's score
                    ps::parse_scores(&t)?
                        .into_iter()
                        .filter(|l| l.node == e.node)
                        .filter(|l| {
                            !(l.cand == "original"
                                && l.table.as_ref().is_some_and(|t| t.sha256 != ts2))
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
            let skipped = if cfg.improve.poles_on() {
                vec![]
            } else {
                im::skipped_by_record(&rules, &record, &cfg.improve)
            };
            let (ref_yield, ref_from) = match cfg.improve.yield_stop.as_ref() {
                None => (None, "no yield stop configured".to_string()),
                Some(y) => match y.reference_bytes_per_cpu_h {
                    Some(r) => (Some(r), "reference given in improve.yield_stop".to_string()),
                    None => match im::reference_yield_run(&record) {
                        Some((r, run)) => (
                            Some(r),
                            format!(
                                "reference = record run {run}, its first improve run that saved bytes"
                            ),
                        ),
                        None => (
                            None,
                            "the record has no improve run that saved bytes".to_string(),
                        ),
                    },
                },
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
                skipped,
                ref_yield,
                ref_from,
                yield_logged: None,
                yield_partial_done: false,
            });
        }
        let engine_path = fake
            .as_ref()
            .and_then(|f| spec.programs.get(&f.program))
            .or_else(|| spec.programs.get(&cfg.program))
            .map(|p| p.path.display().to_string())
            .unwrap_or_default();
        let engine_sha = file_digests(Path::new(&engine_path))
            .map(|d| d.2)
            .unwrap_or_default();
        Ok(ImproveSource {
            cfg_input: cfg_input.to_string(),
            engine: (engine_path, engine_sha),
            cfg,
            fake,
            cdir: cdir.to_path_buf(),
            disk_floor: spec.resources.disk_floor.0,
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

    /// Bytes saved so far by the qualifying candidates of this run (best per rule).
    fn saved_so_far(&mut self, st: &CampaignState, i: usize) -> u64 {
        let node = self.nodes[i].e.node.clone();
        let targets = self.nodes[i].targets.clone();
        let mut saved = 0u64;
        for (pos, cands) in &targets {
            let old = self.nodes[i].rules[*pos].clone();
            let best = cands
                .iter()
                .filter_map(|c| self.answer(st, &item_id(&node, old.index, c)))
                .filter(|a| a.verdict == "ok")
                .filter_map(|a| a.sub_bytes.or(a.new_rule.as_ref().map(|r| r.bytes)))
                .min();
            if let Some(b) = best {
                saved += old.bytes.saturating_sub(b);
            }
        }
        saved
    }

    /// (yield so far in bytes per CPU-h, the run is below the tranche stop line).
    fn yield_state(&mut self, st: &CampaignState, i: usize) -> (Option<f64>, bool) {
        let y = self.yield_now(st, i);
        (y.bytes_per_cpu_h, y.stop)
    }

    /// The yield the stop judges: CUMULATIVE over the run (bytes saved by the best qualifying answer per rule / CPU of
    /// every settled job of the node), or over the SLIDING WINDOW `yield_stop.window_cpu_s` (the most recently settled
    /// items; an answer's gain = what it improved over the best before it on the same rule). Without a yield stop: the
    /// cumulative numbers, never a stop.
    fn yield_now(&mut self, st: &CampaignState, i: usize) -> YieldNow {
        let y = self.cfg.improve.yield_stop.clone();
        let (saved, spent) = match y.as_ref().and_then(|y| y.window_cpu_s) {
            Some(w) => im::window_totals(&self.settled_gains(st, i), w.saturating_mul(1000)),
            None => (
                self.saved_so_far(st, i),
                Self::spent_ms(st, &self.nodes[i].e.node),
            ),
        };
        // the arming threshold (min_cpu_s) is on the RUN's CPU, also with a window
        let (v, stop) = match &y {
            Some(y) => im::yield_check(
                saved,
                spent,
                self.nodes[i].ref_yield,
                &im::YieldStop {
                    min_cpu_s: 0,
                    ..y.clone()
                },
            ),
            None => im::yield_check(saved, spent, None, &im::YieldStop::default()),
        };
        let armed = y.as_ref().is_none_or(|y| {
            Self::spent_ms(st, &self.nodes[i].e.node) >= y.min_cpu_s.saturating_mul(1000)
        });
        YieldNow {
            saved,
            spent_ms: spent,
            bytes_per_cpu_h: v,
            stop: stop && armed,
        }
    }

    /// (cpu_ms, gain bytes) of the node's SETTLED items in settle order (ledger sequence of their final phase).
    fn settled_gains(&mut self, st: &CampaignState, i: usize) -> Vec<(u64, u64)> {
        let node = self.nodes[i].e.node.clone();
        let old: BTreeMap<i64, u64> = self.nodes[i]
            .targets
            .iter()
            .map(|(p, _)| (self.nodes[i].rules[*p].index, self.nodes[i].rules[*p].bytes))
            .collect();
        let mut its: Vec<(u64, String, u64)> = st
            .items
            .iter()
            .filter(|(_, it)| {
                it.node == node && it.kind == KIND_IMPROVE && Self::final_phase(&it.phase)
            })
            .map(|(id, it)| (it.updated_seq, id.clone(), it.cpu_ms))
            .collect();
        its.sort();
        let mut best: BTreeMap<i64, u64> = BTreeMap::new();
        let mut out = Vec::with_capacity(its.len());
        for (_, id, cpu) in its {
            let gain = match self.answer(st, &id) {
                Some(a) if a.verdict == "ok" => {
                    let b = a.sub_bytes.or(a.new_rule.as_ref().map(|r| r.bytes));
                    match (b, old.get(&a.index)) {
                        (Some(b), Some(&o)) => {
                            let cur = best.entry(a.index).or_insert(o);
                            let g = cur.saturating_sub(b);
                            *cur = (*cur).min(b);
                            g
                        }
                        _ => 0,
                    }
                }
                _ => 0,
            };
            out.push((cpu, gain));
        }
        out
    }

    /// The effective yield-stop setting of node `i` (one line, logged at every session start).
    fn yield_line(&self, i: usize) -> String {
        match &self.cfg.improve.yield_stop {
            None => "yield stop: none (improve.yield_stop not set)".to_string(),
            Some(y) => y.describe(self.nodes[i].ref_yield, &self.nodes[i].ref_from),
        }
    }

    /// The job id an item would launch with now (open, or in progress with a queued / failed last attempt).
    fn launch_job(st: &CampaignState, item: &str, it: &crate::state::ItemRec) -> Option<String> {
        match &it.phase {
            ItemPhase::Open => Some(format!("{item}-t{}", it.jobs.len())),
            ItemPhase::InProgress => match it
                .jobs
                .last()
                .and_then(|j| st.jobs.get(j).map(|x| (j, x.state)))
            {
                Some((j, AttemptState::Queued)) => Some(j.clone()),
                Some((_, AttemptState::Failed(_))) => Some(format!("{item}-t{}", it.jobs.len())),
                _ => None,
            },
            _ => None,
        }
    }

    /// An attempt of the item may hold a process (launching / running / unconfirmed). A QUEUED attempt (e.g. requeued
    /// by a drain) does not: it never blocks the node's end (the deadlock found on v31, 2026-11-17).
    fn live(st: &CampaignState, it: &crate::state::ItemRec) -> bool {
        matches!(it.phase, ItemPhase::InProgress)
            && it
                .jobs
                .iter()
                .filter_map(|j| st.jobs.get(j))
                .any(|j| !j.state.is_settled())
    }

    /// The `improve_yield` note of node `i` when the stop state or the number of launches it holds back changed.
    fn yield_note(&mut self, st: &CampaignState, i: usize, y: &YieldNow, rt: &mut Routing) {
        let Some(ys) = self.cfg.improve.yield_stop.clone() else {
            return;
        };
        let node = self.nodes[i].e.node.clone();
        let held = if y.stop {
            st.items
                .iter()
                .filter(|(_, it)| it.node == node && it.kind == KIND_IMPROVE)
                .filter(|(id, it)| Self::launch_job(st, id, it).is_some())
                .count()
        } else {
            0
        };
        let cur = (y.stop, held);
        let was_stopped = self.nodes[i].yield_logged.is_some_and(|(s, _)| s);
        if self.nodes[i].yield_logged != Some(cur) && (y.stop || was_stopped) {
            let v = json!({"node": node, "state": if y.stop { "engaged" } else { "released" }, "held_back": held,
                "bytes_saved": y.saved, "cpu_h": y.spent_ms as f64 / 3.6e6, "bytes_per_cpu_h": y.bytes_per_cpu_h,
                "reference_bytes_per_cpu_h": self.nodes[i].ref_yield, "reference_from": self.nodes[i].ref_from,
                "min_fraction": ys.min_fraction, "stop_line_bytes_per_cpu_h": self.nodes[i].ref_yield.map(|r| r * ys.min_fraction),
                "window_cpu_s": ys.window_cpu_s, "measure": if ys.window_cpu_s.is_some() { "sliding_window" } else { "cumulative" },
                "min_cpu_s": ys.min_cpu_s, "run_cpu_h": Self::spent_ms(st, &node) as f64 / 3.6e6});
            rt.events.push(Event::Note {
                text: format!("improve_yield {v}"),
            });
        }
        self.nodes[i].yield_logged = Some(cur);
    }

    /// The result of a node: improved table (when a rule was replaced), replaced rules, the appended record, the import
    /// spec; registered as `improve:<node>:result`.
    fn finish_node(
        &mut self,
        st: &CampaignState,
        i: usize,
        rt: &mut Routing,
    ) -> Result<(), String> {
        self.write_outputs(st, i, rt, None)
    }

    /// The outputs of node `i` from the settled answers: FINAL (`partial` None: registered as the node's result) or a
    /// PARTIAL snapshot (`partial` = why: under `improve/partial/`, never registered, the node goes on; a ledger note
    /// `improve_partial` names the files). interfaces/genchoice.md 5b.
    fn write_outputs(
        &mut self,
        st: &CampaignState,
        i: usize,
        rt: &mut Routing,
        partial: Option<&str>,
    ) -> Result<(), String> {
        let node = self.nodes[i].e.node.clone();
        let p = self.cfg.improve.clone();
        let run = self.nodes[i].record.next_run();
        let (base, sfx) = match partial {
            Some(_) => (format!("{}/partial", dir(&node)), ".partial"),
            None => (dir(&node), ""),
        };
        let ynow = self.yield_now(st, i);
        let budget_spent = self.budget_spent(st, &node);
        let mut pending: Vec<String> = Vec::new();
        // the answers of this run, per rule index: the best qualifying candidate
        let mut per_rule: BTreeMap<i64, Vec<ImproveAnswer>> = BTreeMap::new();
        let mut untried: BTreeMap<i64, Vec<String>> = BTreeMap::new();
        let targets = self.nodes[i].targets.clone();
        for (pos, cands) in &targets {
            let idx = self.nodes[i].rules[*pos].index;
            for c in cands {
                let item = item_id(&node, idx, c);
                let phase = st.items.get(&item).map(|it| it.phase.clone());
                let fin = phase.as_ref().is_some_and(Self::final_phase);
                match (fin, self.answer(st, &item)) {
                    (true, Some(a)) => per_rule.entry(idx).or_default().push(a),
                    _ => {
                        let why = if phase == Some(ItemPhase::Skipped) {
                            "skipped by the operator".to_string()
                        } else if fin {
                            "no settled answer".to_string()
                        } else if let Some(w) = partial {
                            pending.push(item.clone());
                            format!("pending at a partial snapshot ({w})")
                        } else if ynow.stop {
                            format!(
                                "not run: tranche yield stop ({:.0} B/CPU-h < {} x {:.0})",
                                ynow.bytes_per_cpu_h.unwrap_or(0.0),
                                p.yield_stop.as_ref().map_or(0.0, |y| y.min_fraction),
                                self.nodes[i].ref_yield.unwrap_or(0.0)
                            )
                        } else if budget_spent {
                            "not run: improve.budget_cpu_s spent".to_string()
                        } else {
                            "not run (no settled attempt)".to_string()
                        };
                        untried.entry(idx).or_default().push(format!("{c}: {why}"));
                    }
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
                    let (m, b) = (
                        a.sub_monomials.unwrap_or(r.monomials),
                        a.sub_bytes.unwrap_or(r.bytes),
                    );
                    let c = &p.cost;
                    let primary = match p.filter.by {
                        im::RankBy::Monomials => m,
                        im::RankBy::Bytes => b,
                        im::RankBy::UseCost => (c.load_ns_per_byte * b as f64
                            + c.apply_ns_base * a.sub_rules.unwrap_or(1) as f64
                            + c.apply_ns_per_monomial * m as f64)
                            .round() as u64,
                        // lane rulecost: candidates of one rule share its calls; rank by the case rule's cost per
                        // application at the candidate's (sub-table) monomials (the verdict used every piece)
                        im::RankBy::RuleCost => im::rule_cost_model(c)
                            .per_application_ns(m, r.cost.as_ref())
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
            type Best = (Vec<u64>, u64, String, String, ps::PoleScore, im::TableRule);
            let mut best: BTreeMap<i64, Best> = BTreeMap::new();
            for (pos, cand, line) in &decisions {
                let old = self.nodes[i].rules[*pos].clone();
                let own = self.nodes[i]
                    .pole_scores
                    .iter()
                    .find(|s| {
                        s.scope == "rule" && s.cand == "original" && s.rule_index == Some(old.index)
                    })
                    .map(|s| s.score.clone());
                let t = line.table.as_ref().expect("decide steps carry a table");
                let (_, _, h) = file_digests(Path::new(&t.path))?;
                let (verdict, new) = if h != t.sha256 {
                    (
                        format!("table_changed (sha256 {h} vs the score's {})", t.sha256),
                        None,
                    )
                } else {
                    // the candidate's rule as the CANONICAL line (what a size-mode replacement writes and measures)
                    let raw =
                        std::fs::read_to_string(&t.path).map_err(|e| format!("{}: {e}", t.path))?;
                    let mut found = None;
                    for l in raw.lines() {
                        let Ok(c) = rustred_campaign_contract::canon::canonical_line(l.as_bytes())
                        else {
                            continue;
                        };
                        let c = String::from_utf8_lossy(&c).into_owned();
                        if let Some(r) =
                            im::TableRule::from_line(&c)?.filter(|r| r.case == old.case)
                        {
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
                    derivation: None,
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
        // EMISSION (lane gencheck, interfaces/emission-check.md): when configured, every replacement line is checked on
        // its own bytes with the certificate its job wrote, classed and admitted by the policy BEFORE it is written; a
        // refused replacement keeps the old rule (its reason is recorded with the untried notes)
        if let Some(es) = self.cfg.emission.clone() {
            let side = rustred_campaign_core::emission::side_dir(
                &self
                    .cdir
                    .join(format!("{base}/improved.r{run}{sfx}.table.jsonl")),
            );
            let work = self.cdir.join(format!("{base}/emission-work.r{run}{sfx}"));
            let mut ck = crate::emission::emitter::Checker::new(Path::new(&es.checker))?;
            ck.recover_rows = es.recover_rows;
            ck.family = es.family.as_ref().map(PathBuf::from);
            if let Some(f) = es.floor_s {
                ck.floor_s = f;
            }
            if let Some(n) = &es.descent_node {
                ck.descent_node = Some((PathBuf::from(n), crate::emission::child::file_sha256(Path::new(n))?));
            }
            let em = crate::emission::emitter::Emitter::new(
                ck,
                es.policy.clone().unwrap_or_default(),
                &side,
                &work,
            )?;
            let header = std::io::BufReader::new(
                std::fs::File::open(&self.nodes[i].e.table.path).map_err(|e| e.to_string())?,
            )
            .lines()
            .next()
            .transpose()
            .map_err(|e| e.to_string())?
            .unwrap_or_default();
            // THE d = 4 REGULARITY ACCEPTANCE (contract section 13): the old table loaded once, the old rule lines read
            // once; every replacement compared with its old rule at the old rule's first-match keys before it is written
            // (an old table the dispatcher cannot read refuses every replacement, fail closed; the flow goes on)
            let old_table = crate::emission::regcheck::OldTable::load(Path::new(&self.nodes[i].e.table.path));
            let want: std::collections::BTreeSet<i64> = repl.keys().copied().collect();
            let old_lines = match &old_table {
                Ok(ot) => ot.lines_of(&want)?,
                Err(_) => BTreeMap::new(),
            };
            let reg_opts = es
                .regularity
                .as_ref()
                .map(|r| crate::emission::regcheck::Options {
                    cone: r.cone,
                    max_keys: r.max_keys,
                    threads: r.threads.unwrap_or(1),
                })
                .unwrap_or_default();
            for (idx, lines) in repl.clone() {
                let run_dir = chosen
                    .get(&idx)
                    .and_then(|c| {
                        per_rule
                            .get(&idx)
                            .and_then(|v| v.iter().find(|a| &a.cand == c))
                    })
                    .and_then(|a| st.jobs.get(&a.job).and_then(|j| j.run_dir.clone()));
                let certs: Vec<PathBuf> = run_dir
                    .map(|d| Path::new(&d).join(&es.cert_dir))
                    .and_then(|d| std::fs::read_dir(d).ok())
                    .map(|rd| {
                        let mut v: Vec<PathBuf> = rd
                            .filter_map(|e| e.ok().map(|e| e.path()))
                            .filter(|p| p.extension().is_some_and(|x| x == "json"))
                            .collect();
                        v.sort();
                        v
                    })
                    .unwrap_or_default();
                let mut out_lines = vec![];
                let mut refused: Option<String> = None;
                for (k, l) in lines.iter().enumerate() {
                    let tries: Vec<Option<PathBuf>> = if certs.is_empty() {
                        vec![None]
                    } else {
                        certs.iter().cloned().map(Some).collect()
                    };
                    let mut got = None;
                    let mut last = String::new();
                    for (t, cert) in tries.into_iter().enumerate() {
                        let c = crate::emission::emitter::Candidate {
                            header: header.clone(),
                            line: l.clone(),
                            kind: rustred_campaign_core::emission::class::Kind::Lifted,
                            origin: rustred_campaign_core::emission::class::Origin::Lift,
                            cert,
                            provenance: None,
                            lift_seconds: 0.0,
                            ladder: None,
                            cone: None,
                            sample: None,
                            tag: format!("r{idx}-l{k}-c{t}"),
                        };
                        let e = em.emit(
                            &c,
                            rustred_campaign_core::emission::class::Liveness::Unknown,
                            false,
                        )?;
                        match e.line {
                            Some(x) => {
                                got = Some(x);
                                break;
                            }
                            None => last = e.refused.unwrap_or_default(),
                        }
                    }
                    match got {
                        Some(x) => out_lines.push(x),
                        None => {
                            refused = Some(last);
                            break;
                        }
                    }
                }
                // the d = 4 REPORT (rule 21: a tie-break only, never a refusal): the verdict is stamped when it can be measured;
                // when it cannot, the replacement goes in without one
                if refused.is_none() {
                    match (&old_table, old_lines.get(&idx)) {
                        (Err(_), _) | (Ok(_), None) => {}
                        (Ok(ot), Some(old_line)) => {
                            let st = crate::emission::regcheck::accept_and_stamp(
                                &em,
                                ot,
                                idx,
                                old_line,
                                &out_lines,
                                &reg_opts,
                                &work.join(format!("regularity-r{idx}")),
                            )?;
                            if let Some(stamped) = st.lines {
                                out_lines = stamped;
                            }
                        }
                    }
                }
                match refused {
                    None => {
                        repl.insert(idx, out_lines);
                    }
                    Some(why) => {
                        repl.remove(&idx);
                        let c = chosen.remove(&idx).unwrap_or_default();
                        untried
                            .entry(idx)
                            .or_default()
                            .push(format!("{c}: emission refused ({why})"));
                    }
                }
            }
        }
        // the derivations of this run's candidates (every rule each job sealed), one file per run
        let der_rel = format!("{base}/derivations.r{run}{sfx}.jsonl");
        let mut der_bytes = Vec::new();
        let mut der_lines = 0usize;
        for v in per_rule.values() {
            for a in v {
                for d in &a.derivations {
                    der_bytes.extend_from_slice(d.to_string().as_bytes());
                    der_bytes.push(b'\n');
                    der_lines += 1;
                }
            }
        }
        let der_sha = {
            let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
            h.update(&der_bytes);
            h.finish_hex()
        };
        if der_lines > 0 {
            rt.accepts.push(AcceptedRecord {
                path: der_rel.clone(),
                bytes: der_bytes,
            });
        }
        let tin = self.nodes[i].e.table.sha256.clone();
        // a partial snapshot copies the table only when the copy keeps the cdir above the spec's disk floor
        let table_not_written: Option<String> = match partial {
            Some(_) if !repl.is_empty() => {
                let need = self.nodes[i].table_bytes;
                match super::statvfs_avail(&self.cdir) {
                    Some(a) if a >= need.saturating_add(self.disk_floor) => None,
                    Some(a) => Some(format!(
                        "{a} bytes free < table {need} + disk floor {}: the improved table is only digested (its sha256 binds the record); the final outputs or a later snapshot write it",
                        self.disk_floor
                    )),
                    None => Some("free space unknown: the improved table is only digested".into()),
                }
            }
            _ => None,
        };
        let mut positions_map: Option<Vec<Value>> = None;
        let mut certification: Option<(String, String, usize)> = None;
        let (out_sha, out_path, out_bytes, new_rules) = if repl.is_empty() {
            (
                tin.clone(),
                self.nodes[i].e.table.path.clone(),
                self.nodes[i].table_bytes,
                self.nodes[i].rules.clone(),
            )
        } else {
            // stream the old table, replacing the chosen rules at their index
            let rel_t = format!("{base}/improved.r{run}{sfx}.table.jsonl");
            let rel_o = format!("{base}/replaced_rules.r{run}{sfx}.jsonl");
            let abs_t = self.cdir.join(&rel_t);
            let abs_o = self.cdir.join(&rel_o);
            std::fs::create_dir_all(abs_t.parent().expect("dir")).map_err(|e| e.to_string())?;
            // EMISSION: the kept records' derivation members reference side files of the INPUT table's side
            // directory: carry them (content-addressed: hard links, else copies) so the new table checks on its own
            crate::emission::carry_side(
                &rustred_campaign_core::emission::side_dir(Path::new(&self.nodes[i].e.table.path)),
                &rustred_campaign_core::emission::side_dir(&abs_t),
            )?;
            let tmp_t = abs_t.with_extension("jsonl.tmp");
            let mut sink = TableSink {
                file: match table_not_written {
                    None => Some(std::io::BufWriter::new(
                        std::fs::File::create(&tmp_t).map_err(|e| e.to_string())?,
                    )),
                    Some(_) => None,
                },
                sha: rustred_campaign_core::closure::sha256::Sha256::new(),
                bytes: 0,
                rules: Vec::new(),
                order: im::Positions::default(),
            };
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
            // POSITIONED (native importer order rule): every rule record's index = its position among its chart's rules
            // in file order; a sub-table takes consecutive positions at the replaced rule's place, the rules after it
            // move (only their index number changes); the map old -> new is written beside the table
            let mut pos = im::Positions::default();
            let mut moved: Vec<Value> = Vec::new();
            // the certification list (one line per replacement) and the replay mini table (input header + every
            // replacement line as written): the tool's own outputs for the final certification (no post-step script)
            let mut certs: Vec<Value> = Vec::new();
            let rel_m = format!("{base}/replacements.r{run}{sfx}.mini.table.jsonl");
            let mut mini: Vec<u8> = Vec::new();
            let f = std::fs::File::open(&self.nodes[i].e.table.path).map_err(|e| e.to_string())?;
            for l in std::io::BufReader::new(f).lines() {
                let l = l.map_err(|e| e.to_string())?;
                match im::TableRule::from_line(&l)? {
                    Some(r) if repl.contains_key(&r.index) => {
                        let first = pos.peek(&r.chart);
                        let lines =
                            im::replacement_lines(&repl[&r.index], &l, r.index, first, run)?;
                        for n in &lines {
                            let nr = im::TableRule::from_line(n)?
                                .ok_or("replacement line is not a rule")?;
                            pos.take(&nr.chart, nr.index)?;
                            sink.line(n)?;
                            mini.extend_from_slice(n.as_bytes());
                            mini.push(b'\n');
                        }
                        let mut facts =
                            im::replacement_facts(&l, &lines, &self.nodes[i].e.terminals)?;
                        facts["node"] = json!(node);
                        facts["run"] = json!(run);
                        facts["partial"] = json!(partial.is_some());
                        if let Some(a) = chosen.get(&r.index).and_then(|c| {
                            per_rule
                                .get(&r.index)
                                .and_then(|v| v.iter().find(|a| &a.cand == c))
                        }) {
                            facts["candidate"] = json!(a.cand);
                            facts["job"] = json!(a.job);
                            facts["attempt"] = json!(a.attempt);
                            facts["derivation"] = json!(
                                (!a.derivations.is_empty())
                                    .then(|| format!("{der_rel}#{}.a{}", a.job, a.attempt))
                            );
                            let run_dir = st
                                .jobs
                                .get(&a.job)
                                .and_then(|j| j.run_dir.clone())
                                .unwrap_or_default();
                            let side: Vec<Value> = ["job.json", "OUTCOME.json", "EVENTS.jsonl", "stderr.log", "out/F.cert.jsonl"]
                                .iter()
                                .filter_map(|f| {
                                    let p = Path::new(&run_dir).join(f);
                                    let (b, _, h) = file_digests(&p).ok()?;
                                    Some(json!({"file": p.display().to_string(), "sha256": h, "bytes": b}))
                                })
                                .collect();
                            facts["side_files"] = json!(side);
                        }
                        certs.push(facts);
                        moved.push(json!({"chart": r.chart, "old": r.index, "new": (first..first + lines.len() as i64).collect::<Vec<_>>(), "replaced": true}));
                        old_lines.push(l);
                    }
                    Some(r) => {
                        let at = pos.peek(&r.chart);
                        pos.take(&r.chart, at)?;
                        if at != r.index {
                            moved.push(json!({"chart": r.chart, "old": r.index, "new": [at]}));
                        }
                        sink.line(&im::set_index(&l, at)?)?;
                    }
                    None => sink.line(&im::restate_marker(&l, total, &per_chart))?,
                }
            }
            positions_map = Some(moved);
            // the input table's header first (a replay reads the frame from it)
            let header = std::io::BufReader::new(
                std::fs::File::open(&self.nodes[i].e.table.path).map_err(|e| e.to_string())?,
            )
            .lines()
            .next()
            .transpose()
            .map_err(|e| e.to_string())?
            .unwrap_or_default();
            let mut m = header.into_bytes();
            m.push(b'\n');
            m.extend_from_slice(&mini);
            rt.accepts.push(AcceptedRecord {
                path: rel_m.clone(),
                bytes: m,
            });
            let mut cb = Vec::new();
            for c in &certs {
                cb.extend_from_slice(c.to_string().as_bytes());
                cb.push(b'\n');
            }
            let rel_c = format!("{base}/certification.r{run}{sfx}.jsonl");
            rt.accepts.push(AcceptedRecord {
                path: rel_c.clone(),
                bytes: cb,
            });
            certification = Some((rel_c, rel_m, certs.len()));
            let written = sink.file.is_some();
            if let Some(mut w) = sink.file.take() {
                w.flush().map_err(|e| e.to_string())?;
                drop(w);
                std::fs::rename(&tmp_t, &abs_t).map_err(|e| e.to_string())?;
            }
            let mut ob = Vec::new();
            for l in &old_lines {
                ob.extend_from_slice(l.as_bytes());
                ob.push(b'\n');
            }
            crate::atomic::write_atomic(&abs_o, &ob).map_err(|e| e.to_string())?;
            let path = if written {
                abs_t.display().to_string()
            } else {
                String::new()
            };
            let mut written_rules = sink.rules;
            im::assign_occurrences(&mut written_rules);
            (sink.sha.finish_hex(), path, sink.bytes, written_rules)
        };
        // the positions map (consumers that address rules by position, e.g. positioned recipe entries, re-point by it)
        let positions_map_entries: Vec<Value> = positions_map.clone().unwrap_or_default();
        let positions_rel = positions_map.map(|m| {
            let rel = format!("{base}/positions.r{run}{sfx}.json");
            rt.accepts.push(AcceptedRecord {
                path: rel.clone(),
                bytes: json_bytes(&json!({"schema": "rustred.campaign.improve-positions.v1",
                    "rule": "every rule record's index = its position among its chart's rules in file order (native importer); listed: the input rules whose index changed (old -> new) and the replaced rules (old -> the positions of their replacement lines); every other rule kept its index",
                    "table_in_sha256": self.nodes[i].e.table.sha256, "table_out_sha256": out_sha, "entries": m})),
            });
            rel
        });
        // the appended record
        let mut rec = self.nodes[i].record.clone();
        let old_by_index: BTreeMap<i64, im::TableRule> = self.nodes[i]
            .rules
            .iter()
            .map(|r| (r.index, r.clone()))
            .collect();
        // the run's decisions are keyed by the rule's index in the INPUT table; the improved table renumbers (positions).
        // Audit 2026-11-19 (a3): an output rule finds its input rule by its IDENTITY (an unchanged rule keeps its record
        // body) or, for the lines of a replacement, through the positions map (replaced input index -> the positions of
        // its replacement lines); never by `target | case`, which distinct records share.
        let old_by_key: BTreeMap<String, i64> = self.nodes[i]
            .rules
            .iter()
            .map(|r| (r.key(), r.index))
            .collect();
        let mut pred_at: BTreeMap<(String, i64), i64> = BTreeMap::new();
        for m in positions_map_entries.iter() {
            if m["replaced"].as_bool() != Some(true) {
                continue;
            }
            let (Some(chart), Some(old)) = (m["chart"].as_str(), m["old"].as_i64()) else {
                continue;
            };
            for n in m["new"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_i64)
            {
                pred_at.insert((chart.to_string(), n), old);
            }
        }
        let new_ids: BTreeMap<String, (String, i64)> = new_rules
            .iter()
            .map(|r| (r.key(), (r.chart.clone(), r.index)))
            .collect();
        // a replacement line inherits the history of the rule it replaced (carried over before `align`)
        let mut carried: Vec<im::RuleEntry> = Vec::new();
        for (k, (chart, at)) in &new_ids {
            if let Some(old) = pred_at.get(&(chart.clone(), *at)) {
                let old_key = self.nodes[i]
                    .rules
                    .iter()
                    .find(|r| r.index == *old && &r.chart == chart)
                    .map(|r| r.key());
                if let Some(e) = old_key.and_then(|ok| rec.rules.iter().find(|e| e.key == ok)) {
                    carried.push(im::RuleEntry {
                        key: k.clone(),
                        ..e.clone()
                    });
                }
            }
        }
        rec.rules.extend(carried);
        rec.align(&new_rules);
        for e in rec.rules.iter_mut() {
            let ix = new_ids
                .get(&e.key)
                .and_then(|(chart, at)| pred_at.get(&(chart.clone(), *at)).copied())
                .or_else(|| old_by_key.get(&e.key).copied())
                .unwrap_or(i64::MIN);
            if let Some(v) = per_rule.get(&ix) {
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
                        verdict: if chosen.get(&ix) == Some(&a.cand) && !p.poles_on() {
                            "replaced".into()
                        } else {
                            a.verdict.clone()
                        },
                        table: a.table_path.clone(),
                        pole: None,
                        derivation: (!a.derivations.is_empty())
                            .then(|| format!("{der_rel}#{}.a{}", a.job, a.attempt)),
                    });
                }
            }
            let own_score = self.nodes[i]
                .pole_scores
                .iter()
                .find(|s| s.scope == "rule" && s.cand == "original" && s.rule_index == Some(ix))
                .map(|s| s.score.clone());
            if let Some(v) = pole_tried.remove(&ix) {
                e.tried.extend(v);
            }
            if p.poles_on() {
                e.pole = match pole_new.get(&ix) {
                    Some((sc, _)) => Some(sc.clone()),
                    None => own_score.or(e.pole.take()),
                };
            }
            if let Some(c) = chosen.get(&ix) {
                let old = &old_by_index[&ix];
                e.chosen = format!("rule:{c}");
                e.why = if p.poles_on() {
                    let own = self.nodes[i]
                        .pole_scores
                        .iter()
                        .find(|s| {
                            s.scope == "rule" && s.cand == "original" && s.rule_index == Some(ix)
                        })
                        .map(|s| s.score.clone());
                    let tag = |s: Option<&ps::PoleScore>| {
                        s.map_or("undecided".to_string(), |s| {
                            format!(
                                "max order {} on {} of {} keys",
                                s.max_order.map_or("?".into(), |m| m.to_string()),
                                s.keys_with_pole,
                                s.keys
                            )
                        })
                    };
                    format!(
                        "improve run {run} (pole objective): route poles {} -> {}; {} -> {} monomials; exceptions a subset of the old rule's; same dispatch position",
                        tag(own.as_ref()),
                        tag(pole_new.get(&ix).map(|x| &x.0)),
                        old.monomials,
                        e.monomials
                    )
                } else if self.cfg.improve.subtree {
                    format!(
                        "improve run {run}: rule of {} monomials / {} bytes replaced at its dispatch position by a COMPLETE sub-table of {} rule(s) (first: {} monomials); the sub-table covers the whole case domain",
                        old.monomials,
                        old.bytes,
                        repl.get(&ix).map_or(0, |v| v.len()),
                        e.monomials
                    )
                } else {
                    format!(
                        "improve run {run}: {} -> {} monomials, {} -> {} bytes; exceptions a subset of the old rule's; same dispatch position",
                        old.monomials, e.monomials, old.bytes, e.bytes
                    )
                };
            }
            if let Some(u) = untried.get(&ix) {
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
                let ix = old_by_key.get(&e.key).copied().unwrap_or(i64::MIN);
                targets
                    .iter()
                    .any(|(p, _)| self.nodes[i].rules[*p].index == ix)
            })
            .collect();
        // the yield: CUMULATIVE over the run (bytes_saved / cpu_h / bytes_per_cpu_h); `judged_*` = what the stop judges
        // (the same, or the sliding window `yield_stop.window_cpu_s`)
        let saved_now = self.saved_so_far(st, i);
        let spent_now = Self::spent_ms(st, &node);
        let cumulative = (spent_now > 0).then(|| saved_now as f64 / (spent_now as f64 / 3.6e6));
        let yield_json = json!({"bytes_saved": saved_now, "cpu_h": spent_now as f64 / 3.6e6,
        "bytes_per_cpu_h": cumulative, "measure": match p.yield_stop.as_ref().and_then(|y| y.window_cpu_s) {
            Some(_) => "sliding_window", None => "cumulative" },
        "judged_bytes_saved": ynow.saved, "judged_cpu_h": ynow.spent_ms as f64 / 3.6e6,
        "judged_bytes_per_cpu_h": ynow.bytes_per_cpu_h,
        "stop": self.yield_line(i), "yield_stop": p.yield_stop,
        "reference_bytes_per_cpu_h": self.nodes[i].ref_yield, "reference_from": self.nodes[i].ref_from,
        "min_fraction": p.yield_stop.as_ref().map(|y| y.min_fraction), "stopped_by_yield": ynow.stop,
        "next_tranche_advised": match (ynow.bytes_per_cpu_h, self.nodes[i].ref_yield, &p.yield_stop) {
            (Some(v), Some(r), Some(y)) => Some(v >= y.min_fraction * r),
            _ => None,
        }});
        let not_run: usize = untried.values().map(|v| v.len()).sum();
        let stopped_by = if partial.is_some() {
            Value::Null
        } else if ynow.stop {
            json!("yield_stop")
        } else if budget_spent {
            json!("budget_cpu_s")
        } else {
            Value::Null
        };
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
                "derivations": (der_lines > 0).then(|| json!({"path": self.cdir.join(&der_rel).display().to_string(),
                    "sha256": der_sha, "lines": der_lines})),
                "skipped_by_record": self.nodes[i].skipped, "yield": yield_json,
                "score_form": p.poles.as_ref().filter(|_| p.poles_on()).map(|q| q.score.word()),
                "score_level": p.poles.as_ref().filter(|_| p.poles_on()).map(|q| q.level.word()),
                "dependences": if p.poles_on() {
                    self.nodes[i].e.dependences.as_ref().map_or(json!("upper_bound"), |f| json!(f))
                } else {
                    Value::Null
                },
                "decided_from_scores": self.nodes[i].decisions.len(), "unscored_rules": self.nodes[i].unscored.len(),
                "partial": partial.is_some(), "partial_why": partial, "pending": pending.len(), "not_run": not_run,
                "positions": positions_rel.as_ref().map(|r| self.cdir.join(r).display().to_string()),
                "stopped_by": stopped_by,
                "replaced": repl.len(), "table_bytes_before": self.nodes[i].table_bytes, "table_bytes_after": out_bytes,
                "targets_bytes_before": tb, "targets_bytes_after": after.iter().map(|e| e.bytes).sum::<u64>(),
                "targets_monomials_before": tm, "targets_monomials_after": after.iter().map(|e| e.monomials).sum::<u64>()}),
        });
        rec.rank();
        let rec_rel = format!("{base}/record.r{run}{sfx}.json");
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
        let import_rel = format!("{base}/import.r{run}{sfx}.json");
        let import = self.nodes[i]
            .e
            .r#box
            .as_ref()
            .filter(|_| !out_path.is_empty())
            .map(|b| gc::import_spec(&node, &b.path, &b.sha256, &out_path, &out_sha));
        if let Some(s) = &import {
            rt.accepts.push(AcceptedRecord {
                path: import_rel.clone(),
                bytes: json_bytes(s),
            });
        }
        let mut res = json!({"schema": "rustred.campaign.improve-result.v1", "node": node, "run": run,
            "partial": partial.is_some(),
            "table_in": {"path": self.nodes[i].e.table.path, "sha256": tin},
            "table_out": {"path": out_path, "sha256": out_sha, "bytes": out_bytes},
            "replaced": repl.len(), "record": {"path": self.cdir.join(&rec_rel).display().to_string(), "sha256": rec_sha},
            "import_spec": import.as_ref().map(|_| self.cdir.join(&import_rel).display().to_string()),
            "skipped_by_record": self.nodes[i].skipped, "yield": yield_json,
            "not_run": not_run, "stopped_by": stopped_by,
            "positions": positions_rel.as_ref().map(|r| self.cdir.join(r).display().to_string()),
            "certification": certification.as_ref().map(|(c, m, n)| json!({"list": self.cdir.join(c).display().to_string(),
                "replay_mini_table": self.cdir.join(m).display().to_string(), "replacements": n})),
            "nothing_to_try": targets.is_empty() && self.nodes[i].decisions.is_empty()});
        let res_rel = format!("{base}/result.r{run}{sfx}.json");
        match partial {
            None => {
                let r = AcceptedRecord {
                    path: res_rel,
                    bytes: json_bytes(&res),
                };
                self.reg(rt, input_result(&node), ROLE_IMPROVE_RESULT, r);
            }
            Some(why) => {
                res["why"] = json!(why);
                res["pending"] = json!(pending);
                res["table_not_written"] = json!(table_not_written);
                res["resume"] = json!(
                    "PARTIAL snapshot, not the node's result: the same launch line resumes this campaign from its ledger and writes the final outputs of this run; or a NEW improve run with table / record = these partial outputs re-targets the pending pairs (every settled pair is in the record)"
                );
                let bytes = json_bytes(&res);
                let res_sha = {
                    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
                    h.update(&bytes);
                    h.finish_hex()
                };
                rt.accepts.push(AcceptedRecord {
                    path: res_rel.clone(),
                    bytes,
                });
                let note = json!({"node": node, "run": run, "why": why,
                    "result": self.cdir.join(&res_rel).display().to_string(), "result_sha256": res_sha,
                    "record": self.cdir.join(&rec_rel).display().to_string(), "record_sha256": rec_sha,
                    "table": (!out_path.is_empty()).then_some(&out_path), "table_sha256": out_sha,
                    "table_not_written": table_not_written, "replaced": repl.len(), "pending": pending.len()});
                rt.events.push(Event::Note {
                    text: format!("improve_partial {note}"),
                });
            }
        }
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
            if st.inputs.contains_key(&input_result(&node))
                || self.budget_spent(st, &node)
                || self.yield_state(st, i).1
            {
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
                    let Some(job) = Self::launch_job(st, &item, it) else {
                        continue;
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
                match im::subtree_verdict_eq(
                    &old,
                    &sub_rules,
                    self.cfg.improve.min_gain,
                    self.cfg.improve.filter.by,
                    &self.cfg.improve.cost,
                    self.cfg.improve.accept_equal,
                ) {
                    Ok(()) => "ok".to_string(),
                    Err(w) => w,
                }
            }
            Some(n) => match im::replacement_verdict_eq(
                &old,
                n,
                self.cfg.improve.min_gain,
                self.cfg.improve.filter.by,
                &self.cfg.improve.cost,
                self.cfg.improve.accept_equal,
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
        // declared terminals (spec data): a regeneration serving one never replaces anything
        let terminals = self.nodes[i].e.terminals.clone();
        let verdict = if verdict == "ok" && !terminals.is_empty() {
            let mut hit = None;
            for (_, l) in &all {
                let v: Value = serde_json::from_str(l).map_err(|e| e.to_string())?;
                if let Some(t) = terminals
                    .iter()
                    .find(|t| im::serves_key(&v, t).unwrap_or(true))
                {
                    hit = Some((v.get("index").cloned().unwrap_or(Value::Null), t.clone()));
                    break;
                }
            }
            match hit {
                Some((ix, t)) => format!(
                    "serves_declared_terminal (rule {ix} serves {t:?}, or its domain could not be evaluated there)"
                ),
                None => verdict,
            }
        } else {
            verdict
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
        let table_path = (done.table.is_some() && !run_dir.is_empty()).then(|| {
            Path::new(&run_dir)
                .join("out")
                .join("F.table.jsonl")
                .display()
                .to_string()
        });
        // the effective derivation of every rule the job sealed (TRICKS 31 / 35): from the job's own files
        let derivations = if run_dir.is_empty() || done.canonical_lines.is_empty() {
            vec![]
        } else {
            let rd = Path::new(&run_dir);
            let rd_text = |f: &str| std::fs::read_to_string(rd.join(f)).unwrap_or_default();
            let (spec_t, events_t, stderr_t) = (
                rd_text("job.json"),
                rd_text("EVENTS.jsonl"),
                rd_text("stderr.log"),
            );
            dv::rule_derivations(&dv::JobFiles {
                node: &it.node,
                item: &done.item,
                job: &done.job,
                attempt: done.attempt,
                job_spec: &spec_t,
                events: &events_t,
                stderr: &stderr_t,
                table_lines: &done.canonical_lines,
                engine_path: &self.engine.0,
                engine_sha256: &self.engine.1,
            })
            .unwrap_or_default()
        };
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
            derivations,
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
            // only an attempt that may hold a process is running: a QUEUED one (requeued by a drain / SIGTERM) never
            // keeps a stopped node from finishing (v31 deadlock, 2026-11-17)
            let running = items.iter().any(|it| Self::live(st, it));
            let all_final = items.iter().all(|it| Self::final_phase(&it.phase));
            let y = self.yield_now(st, i);
            self.yield_note(st, i, &y, &mut rt);
            if all_final || (!running && (self.budget_spent(st, &node) || y.stop)) {
                self.finish_node(st, i, &mut rt)?;
            } else if y.stop && !self.nodes[i].yield_partial_done {
                // the yield stop holds back every launch while jobs still run: a snapshot of what is settled now
                self.nodes[i].yield_partial_done = true;
                self.write_outputs(
                    st,
                    i,
                    &mut rt,
                    Some("yield stop engaged; running jobs continue"),
                )?;
            }
        }
        Ok(rt)
    }

    fn drained(&mut self, st: &CampaignState, _ctx: &SourceContext) -> Result<Routing, String> {
        let mut rt = Routing::default();
        for i in 0..self.nodes.len() {
            let node = self.nodes[i].e.node.clone();
            if st.inputs.contains_key(&input_result(&node)) || !st.nodes.contains_key(&node) {
                continue;
            }
            let settled = st
                .items
                .values()
                .filter(|it| it.node == node && it.kind == KIND_IMPROVE)
                .any(|it| Self::final_phase(&it.phase));
            if settled || !self.nodes[i].decisions.is_empty() {
                self.write_outputs(st, i, &mut rt, Some("orderly stop (drain)"))?;
            } else {
                rt.events.push(Event::Note {
                    text: format!(
                        "improve_partial {}",
                        json!({"node": node, "why": "orderly stop (drain)", "written": false, "reason": "nothing settled in this run yet"})
                    ),
                });
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
        let ys: Vec<String> = (0..self.nodes.len())
            .map(|i| format!("{}: {}", self.nodes[i].e.node, self.yield_line(i)))
            .collect();
        format!(
            "improve: {} node(s), {} rule-candidate pair(s) to try; {}",
            self.nodes.len(),
            self.nodes
                .iter()
                .map(|n| n.targets.iter().map(|(_, c)| c.len()).sum::<usize>())
                .sum::<usize>(),
            ys.join("; ")
        )
    }
}
