//! Why an item is in its state (requirement R3 "explain WHY an item is in its state"): a machine-readable record
//! (serde JSON) and a human-readable text, built purely from the item's state and the node's parameters.

use serde::{Deserialize, Serialize};

use crate::worklist::items::NodeParams;
use crate::worklist::ladder::{
    Disposition, ItemState, Next, PointVerdict, SolveOutcome, attempt_key, next_steps,
};
use crate::worklist::seed::seed_space;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateLine {
    pub index: usize,
    pub digest: String,
    pub rhs_terms: u64,
    /// Gap points credited / total; None while not judged.
    pub credited: Option<usize>,
    pub total: usize,
    /// Why points were not credited: verdict kind -> count.
    pub not_credited: Vec<(String, usize)>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttemptLine {
    pub setting: String,
    pub wave: u8,
    /// "pending" | "interrupted" | outcome tag.
    pub outcome: String,
    pub detail: Option<String>,
    pub cpu_ms: u64,
    pub operator_interrupts: u32,
    pub infra_interrupts: u32,
    pub candidates: Vec<CandidateLine>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Explanation {
    pub item: String,
    pub kind: String,
    pub case: String,
    pub gap_points: usize,
    /// "open" | "covered" | "split" | "terminal_candidates" | "attention" | "optimise_kept"
    pub state: String,
    pub sub_counter: usize,
    pub attempts_made: usize,
    pub attempts: Vec<AttemptLine>,
    /// What the state machine would do next.
    pub next: String,
    /// What blocks the item, if anything (a missing verdict, evidence, a counter).
    pub blocked_by: Option<String>,
    pub summary: String,
}

fn verdict_kind(v: &PointVerdict) -> &'static str {
    match v {
        PointVerdict::Covered => "covered",
        PointVerdict::NotInCase => "not_in_case",
        PointVerdict::OnException { .. } => "on_exception",
        PointVerdict::OnPole => "on_pole",
        PointVerdict::Elided => "elided",
        PointVerdict::Blocked { .. } => "blocked",
        PointVerdict::HiddenPole => "hidden_pole",
        PointVerdict::NotDescending { .. } => "not_descending",
        PointVerdict::Unknown { .. } => "unknown",
    }
}

/// Explain one item.
pub fn explain(state: &ItemState, params: &NodeParams) -> Explanation {
    let item = &state.item;
    let waves = seed_space(&item.case, params);
    let total: usize = waves.iter().map(|w| w.len()).sum();
    let mut attempts = Vec::new();
    // wave-0 offers / inherited candidates first, then the seed space wave by wave
    let prefix = format!("{}|offer|", item.id.0);
    let mut listed: Vec<(u8, String, String)> = state
        .records
        .iter()
        .filter(|(k, _)| k.starts_with(&prefix))
        .map(|(k, _)| (0u8, format!("offer:{}", &k[prefix.len()..]), k.clone()))
        .collect();
    for (w, wave) in waves.iter().enumerate() {
        for s in wave {
            listed.push((w as u8, s.label(), attempt_key(&item.id, s)));
        }
    }
    for (w, label, k) in listed {
        {
            let Some(r) = state.records.get(&k) else {
                continue;
            };
            let (outcome, detail) = match &r.answer {
                None if r.settled_without_verdict(&params.budgets) => (
                    "no_verdict".to_string(),
                    Some(format!(
                        "non-verdict outcomes {:?}, infra interrupts {}",
                        r.non_verdict, r.infra_interrupts
                    )),
                ),
                None if !r.non_verdict.is_empty() => (
                    "retry_pending".to_string(),
                    Some(format!("non-verdict outcomes {:?}", r.non_verdict)),
                ),
                None => ("interrupted".to_string(), None),
                Some(a) => (
                    a.outcome.tag().to_string(),
                    match &a.outcome {
                        SolveOutcome::RefusedHeavy { reason }
                        | SolveOutcome::ProfileRefused { reason } => Some(reason.clone()),
                        SolveOutcome::Error(e) => Some(e.clone()),
                        SolveOutcome::BudgetStop { time_decided } => {
                            Some(format!("time_decided={time_decided}"))
                        }
                        _ => None,
                    },
                ),
            };
            let candidates = r
                .candidates()
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let v = state.verdicts.get(&(k.clone(), i));
                    let mut nc: Vec<(String, usize)> = Vec::new();
                    if let Some(v) = v {
                        for x in v.iter().filter(|x| !x.covered()) {
                            let kind = verdict_kind(x).to_string();
                            match nc.iter_mut().find(|(n, _)| *n == kind) {
                                Some(e) => e.1 += 1,
                                None => nc.push((kind, 1)),
                            }
                        }
                    }
                    CandidateLine {
                        index: i,
                        digest: c.digest.clone(),
                        rhs_terms: c.rhs_terms,
                        credited: v.map(|v| v.iter().filter(|x| x.covered()).count()),
                        total: item.gap_points.len(),
                        not_credited: nc,
                    }
                })
                .collect();
            attempts.push(AttemptLine {
                setting: label,
                wave: w,
                outcome,
                detail,
                cpu_ms: r.cpu_ms_total,
                operator_interrupts: r.operator_interrupts,
                infra_interrupts: r.infra_interrupts,
                candidates,
            });
        }
    }
    let (st, next, blocked_by, summary) = match next_steps(state, params) {
        Next::Run(list) => (
            "open".to_string(),
            format!(
                "run {} attempt(s) of wave {}: {}",
                list.len(),
                list[0].wave,
                list.iter()
                    .take(4)
                    .map(|a| a.setting.label())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            None,
            "seed search in progress".to_string(),
        ),
        Next::NeedVerdicts(r) => (
            "open".into(),
            format!("judge {} candidate(s) at the gap points", r.len()),
            Some("point verdicts (facts) missing".to_string()),
            "waiting for acceptance facts".into(),
        ),
        Next::NeedEvidence(r) => (
            "open".into(),
            format!("prove {} candidate(s) at the required level", r.len()),
            Some(format!(
                "identity evidence at level {:?} missing",
                params.min_evidence
            )),
            "waiting for identity evidence".into(),
        ),
        Next::NeedOffers => (
            "open".into(),
            "ask the candidate source(s) for wave-0 offers".into(),
            None,
            "wave 0 (supplied candidates) not asked yet".into(),
        ),
        Next::NeedContainment(r) => (
            "open".into(),
            format!("containment verdict for {} candidate(s)", r.len()),
            Some("containment proof missing (optimise item)".to_string()),
            "waiting for a containment proof".into(),
        ),
        Next::Done(d) => match d {
            Disposition::Covered { cover } => (
                "covered".into(),
                "none".into(),
                None,
                format!(
                    "covered by {} case(s): {}",
                    cover.cases(),
                    cover
                        .members
                        .iter()
                        .map(|m| m.setting.label())
                        .collect::<Vec<_>>()
                        .join(" + ")
                ),
            ),
            Disposition::Split {
                cover, children, ..
            } => (
                "split".into(),
                format!("{} child item(s)", children.len()),
                None,
                format!(
                    "no one-case cover after every setting of the seed search ({} attempts); {} case(s) serve {} point(s), {} child(ren) hold the rest",
                    state.attempts_made(),
                    cover.cases(),
                    cover.members.iter().map(|m| m.served.len()).sum::<usize>(),
                    children.len()
                ),
            ),
            Disposition::TerminalCandidates { keys, exhausted } => (
                "terminal_candidates".into(),
                "terminal proposal (declared at S9 only)".into(),
                None,
                format!(
                    "{} key(s) without an accepted relation after {} attempts (depth 3 tried: {}, depth 3 unfloored tried: {})",
                    keys.len(),
                    exhausted.attempts(),
                    exhausted.depth3_tried(),
                    exhausted.depth3_unfloored_tried()
                ),
            ),
            Disposition::Attention { why, .. } => (
                "attention".into(),
                "operator".into(),
                Some(why.clone()),
                why,
            ),
            Disposition::OptimiseKept { why } => ("optimise_kept".into(), "none".into(), None, why),
        },
    };
    Explanation {
        item: item.id.0.clone(),
        kind: format!("{:?}", item.kind),
        case: item.case.canonical_text(),
        gap_points: item.gap_points.len(),
        state: st,
        sub_counter: total,
        attempts_made: state.attempts_made(),
        attempts,
        next,
        blocked_by,
        summary,
    }
}

impl Explanation {
    /// Human-readable text (one line per attempt).
    pub fn to_text(&self) -> String {
        let mut s = format!(
            "item {} ({}) {}\n  state: {} - {}\n  gap points: {}; attempts {}/{} of the seed-search space\n",
            self.item,
            self.kind,
            self.case,
            self.state,
            self.summary,
            self.gap_points,
            self.attempts_made,
            self.sub_counter
        );
        for a in &self.attempts {
            s.push_str(&format!("  [w{}] {:<28} {}", a.wave, a.setting, a.outcome));
            if let Some(d) = &a.detail {
                s.push_str(&format!(" ({d})"));
            }
            for c in &a.candidates {
                match c.credited {
                    Some(n) => s.push_str(&format!("; rule {} covers {}/{}", c.index, n, c.total)),
                    None => s.push_str(&format!("; rule {} not judged", c.index)),
                }
                for (k, n) in &c.not_credited {
                    s.push_str(&format!(" {k}:{n}"));
                }
            }
            s.push('\n');
        }
        s.push_str(&format!("  next: {}\n", self.next));
        if let Some(b) = &self.blocked_by {
            s.push_str(&format!("  blocked by: {b}\n"));
        }
        s
    }
}
