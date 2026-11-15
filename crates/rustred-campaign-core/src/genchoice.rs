//! Generation-choice portfolio of a whole node (lane sizegen; interfaces/genchoice.md; request
//! reports/COORDINATOR-REQUEST-20261115-SIZE.md; R37, U-13). PURE: no process, no file, no clock, no ledger.
//!
//! From the node DEFINITION alone (the family file: slots and their line / ISP flags) and a policy, this module gives
//! the candidate set (orders x poses x ladders), the R1 "boundary < free < interior" order VOTED over the cases of a
//! base probe weighted by rule size (order-study R1 + R2: the heavy rules sit on the cases without a depth-1 rule, so
//! the measured size replaces the oracle's hard-case test), the measures of a sealed table, the deterministic probe
//! ranking, the economy step of U-13 (extra CPU <= factor x the CPU of the first acceptable full table; optimise mode
//! = no cap), the choice among acceptable full tables, the per-rule comparison of the winner with the losers and the
//! import spec of the winner. Every threshold is a policy field (L8): nothing here knows a node, a loop count, an arity
//! or a family.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const CONFIG_SCHEMA: &str = "rustred.campaign.genchoice.v1";
pub const CHOICE_SCHEMA: &str = "rustred.campaign.genchoice-choice.v1";
pub const ANSWER_SCHEMA: &str = "rustred.campaign.genchoice-answer.v1";
pub const CANDIDATES_SCHEMA: &str = "rustred.campaign.genchoice-candidates.v1";

// ------------------------------------------------------------------------------------------------ family shape

/// Slots of a family: arity, line slots (propagators, flag 1) and ISP slots (numerators, flag 0), in slot order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FamilyShape {
    pub arity: usize,
    pub lines: Vec<usize>,
    pub isps: Vec<usize>,
}

impl FamilyShape {
    pub fn is_line(&self, slot: usize) -> bool {
        self.lines.contains(&slot)
    }
}

/// The shape of a family text (`prop(NAME, EXPR, FLAG)` entries in slot order; FLAG 1 = line, 0 = ISP). The
/// expression may contain parentheses and commas; the flag is the last top-level argument.
pub fn family_shape(text: &str) -> Result<FamilyShape, String> {
    let b = text.as_bytes();
    let mut flags: Vec<bool> = Vec::new();
    let mut i = 0;
    while let Some(p) = text[i..].find("prop(") {
        let start = i + p;
        // a word boundary before `prop(`
        if start > 0 && (b[start - 1].is_ascii_alphanumeric() || b[start - 1] == b'_') {
            i = start + 5;
            continue;
        }
        let mut depth = 0i32;
        let mut last_comma = None;
        let mut end = None;
        for (k, &c) in b.iter().enumerate().skip(start + 4) {
            match c {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(k);
                        break;
                    }
                }
                b',' if depth == 1 => last_comma = Some(k),
                _ => {}
            }
        }
        let (Some(e), Some(c)) = (end, last_comma) else {
            return Err(format!("family: unterminated prop( at byte {start}"));
        };
        let flag = text[c + 1..e].trim();
        flags.push(match flag {
            "1" => true,
            "0" => false,
            f => return Err(format!("family: prop flag {f:?} (expected 0 or 1)")),
        });
        i = e + 1;
    }
    if flags.is_empty() {
        return Err("family: no prop( entries".into());
    }
    Ok(FamilyShape {
        arity: flags.len(),
        lines: (0..flags.len()).filter(|&s| flags[s]).collect(),
        isps: (0..flags.len()).filter(|&s| !flags[s]).collect(),
    })
}

// ------------------------------------------------------------------------------------------------ policy

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderKind {
    /// The engine's legacy order (no coordinate priority).
    Id,
    /// R1 vote over the base probe's cases (needs the base probe).
    Bfi,
    /// Lines in slot order, ISPs reversed.
    IspRev,
    /// Lines reversed, ISPs in slot order.
    LinesRev,
}

impl OrderKind {
    pub fn word(self) -> &'static str {
        match self {
            OrderKind::Id => "id",
            OrderKind::Bfi => "bfi",
            OrderKind::IspRev => "isp_rev",
            OrderKind::LinesRev => "lines_rev",
        }
    }
}

/// A named set of extra generate flags (a pose or a ladder rung).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedArgs {
    pub name: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ProbePolicy {
    /// Deterministic work cap of a probe (`budgets.work.cases`).
    pub cases: u64,
    /// Safety nets (time-decided): a probe stopped by them is HEAVY.
    pub wall_case_s: Option<u64>,
    pub wall_job_s: Option<u64>,
    /// Extra generate flags of PROBES only (full runs never get them). Default: the engine's lift pre-screen refuses a
    /// heavy relation BEFORE its exact lift (deterministic modular estimate) and leaves the case OPEN, so a probe never
    /// hangs on a heavy lift; the refused cases are listed in the probe's sealed `F.subcases.jsonl` (kind `refused`):
    /// their number ranks the probe and their patterns vote in the BFI order.
    pub args: Vec<String>,
}

impl Default for ProbePolicy {
    fn default() -> Self {
        ProbePolicy {
            cases: 600,
            wall_case_s: Some(600),
            wall_job_s: Some(2700),
            args: [
                "--on-heavy",
                "refuse",
                "--screen",
                "0,64,24,2500",
                "--open-on-refusal",
            ]
            .iter()
            .map(|x| x.to_string())
            .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct FullPolicy {
    pub wall_case_s: Option<u64>,
    pub wall_job_s: Option<u64>,
    /// Full runs tried until the first acceptable table (in ranking order).
    pub max_first_attempts: u32,
}

impl Default for FullPolicy {
    fn default() -> Self {
        FullPolicy {
            wall_case_s: Some(3600),
            wall_job_s: None,
            max_first_attempts: 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoteWeight {
    /// max(1, coefficient monomials) of the rule.
    Monomials,
    /// Bytes of the rule record.
    Bytes,
    /// Every case counts 1.
    Uniform,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct BfiPolicy {
    pub weight: VoteWeight,
    /// The candidate id whose probe feeds the vote; None = `id` x first pose x first ladder.
    pub base: Option<String>,
    /// Vote weight of a case the probe REFUSED as heavy (no rule, so no measured size): at least the engine's accept
    /// ruling (1,000,000 monomials), i.e. a hard case outweighs every ordinary one (order-study R2: the heavy rules are
    /// the cases without a depth-1 rule, which R1 must fix first).
    pub refused_weight: u64,
}

impl Default for BfiPolicy {
    fn default() -> Self {
        BfiPolicy {
            weight: VoteWeight::Monomials,
            base: None,
            refused_weight: 1_000_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct EconomyPolicy {
    /// U-13: extra CPU spent on alternatives <= factor x CPU of the first acceptable full table (<= 0: none).
    pub factor: f64,
    /// OPTIMISE MODE: every ranked candidate with a table runs to completion, no cap.
    pub optimise: bool,
    /// Alternatives run after the first acceptable table (each gets factor x C1 / A as its wall cap).
    pub max_alternatives: u32,
}

impl Default for EconomyPolicy {
    fn default() -> Self {
        EconomyPolicy {
            factor: 2.0,
            optimise: false,
            max_alternatives: 2,
        }
    }
}

/// Estimated use cost of a table (the repair flow's cost model, repairflow.md 2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct CostModel {
    pub load_ns_per_byte: f64,
    pub apply_ns_base: f64,
    pub apply_ns_per_monomial: f64,
}

impl Default for CostModel {
    fn default() -> Self {
        CostModel {
            load_ns_per_byte: 30.0,
            apply_ns_base: 20_000.0,
            apply_ns_per_monomial: 100.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Select {
    UseCost,
    Bytes,
    MaxMonomials,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct JobPolicy {
    pub cap_bytes: u64,
    pub expected_peak_bytes: u64,
    pub heartbeat_s: u32,
    pub stop_grace_s: u32,
}

impl Default for JobPolicy {
    fn default() -> Self {
        JobPolicy {
            cap_bytes: 32 << 30,
            expected_peak_bytes: 4 << 30,
            heartbeat_s: 30,
            stop_grace_s: 60,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Policy {
    pub orders: Vec<OrderKind>,
    pub poses: Vec<NamedArgs>,
    pub ladders: Vec<NamedArgs>,
    pub common_args: Vec<String>,
    pub probe: ProbePolicy,
    pub full: FullPolicy,
    pub bfi: BfiPolicy,
    pub economy: EconomyPolicy,
    pub cost: CostModel,
    pub select: Select,
    pub job: JobPolicy,
}

impl Default for Policy {
    fn default() -> Self {
        Policy {
            orders: vec![
                OrderKind::Id,
                OrderKind::Bfi,
                OrderKind::IspRev,
                OrderKind::LinesRev,
            ],
            poses: vec![
                NamedArgs {
                    name: "default".into(),
                    args: vec![],
                },
                NamedArgs {
                    name: "rung".into(),
                    args: vec!["--seed-pose-rung".into()],
                },
            ],
            ladders: vec![NamedArgs {
                name: "std".into(),
                args: vec![],
            }],
            common_args: vec!["--numeric-depth-max".into(), "3".into()],
            probe: ProbePolicy::default(),
            full: FullPolicy::default(),
            bfi: BfiPolicy::default(),
            economy: EconomyPolicy::default(),
            cost: CostModel::default(),
            select: Select::UseCost,
            job: JobPolicy::default(),
        }
    }
}

impl Policy {
    /// Structural checks (the allow-list check of the flags is the supervisor's, against the contract).
    pub fn check(&self) -> Result<(), String> {
        if self.orders.is_empty() || self.poses.is_empty() || self.ladders.is_empty() {
            return Err("genchoice policy: orders, poses and ladders must be non-empty".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        for o in &self.orders {
            if !seen.insert(*o) {
                return Err(format!("genchoice policy: order {} listed twice", o.word()));
            }
        }
        for (what, l) in [("pose", &self.poses), ("ladder", &self.ladders)] {
            let mut names = std::collections::BTreeSet::new();
            for p in l {
                if p.name.is_empty()
                    || !p
                        .name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_')
                {
                    return Err(format!(
                        "genchoice policy: {what} name {:?}: [A-Za-z0-9_]+",
                        p.name
                    ));
                }
                if !names.insert(p.name.as_str()) {
                    return Err(format!("genchoice policy: {what} {} listed twice", p.name));
                }
            }
        }
        if self.probe.cases == 0 {
            return Err("genchoice policy: probe.cases must be >= 1".into());
        }
        if self.full.max_first_attempts == 0 {
            return Err("genchoice policy: full.max_first_attempts must be >= 1".into());
        }
        if !self.economy.factor.is_finite() && !self.economy.optimise {
            return Err(
                "genchoice policy: economy.factor must be finite (optimise: true lifts the cap)"
                    .into(),
            );
        }
        if let Some(b) = &self.bfi.base
            && !self.candidates().iter().any(|c| &c.id == b)
        {
            return Err(format!(
                "genchoice policy: bfi.base {b:?} is not a candidate"
            ));
        }
        if let Some(b) = self.bfi_base()
            && b.order == OrderKind::Bfi
        {
            return Err("genchoice policy: the bfi base cannot itself be a bfi candidate".into());
        }
        if self.orders.contains(&OrderKind::Bfi) && self.bfi_base().is_none() {
            return Err(
                "genchoice policy: order bfi needs a base candidate (order id, or bfi.base)".into(),
            );
        }
        Ok(())
    }

    /// The candidate list in its fixed enumeration order (orders x poses x ladders).
    pub fn candidates(&self) -> Vec<Candidate> {
        let mut out = Vec::new();
        for &o in &self.orders {
            for (p, pose) in self.poses.iter().enumerate() {
                for (l, lad) in self.ladders.iter().enumerate() {
                    out.push(Candidate {
                        index: out.len(),
                        id: format!("{}-{}-{}", o.word(), pose.name, lad.name),
                        order: o,
                        pose: p,
                        ladder: l,
                    });
                }
            }
        }
        out
    }

    /// The candidate whose probe feeds the BFI vote.
    pub fn bfi_base(&self) -> Option<Candidate> {
        let all = self.candidates();
        match &self.bfi.base {
            Some(b) => all.into_iter().find(|c| &c.id == b),
            None => all
                .into_iter()
                .find(|c| c.order == OrderKind::Id && c.pose == 0 && c.ladder == 0),
        }
    }

    /// The generate flags of a candidate (order JSON first, then the pose's, the ladder's and the common args).
    pub fn args(&self, c: &Candidate, order_json: &str) -> Vec<String> {
        let mut a = vec!["--order".to_string(), order_json.to_string()];
        a.extend(self.poses[c.pose].args.iter().cloned());
        a.extend(self.ladders[c.ladder].args.iter().cloned());
        a.extend(self.common_args.iter().cloned());
        a
    }
}

/// One candidate generation choice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub index: usize,
    /// `<order>-<pose>-<ladder>` ([A-Za-z0-9_-]).
    pub id: String,
    pub order: OrderKind,
    pub pose: usize,
    pub ladder: usize,
}

// ------------------------------------------------------------------------------------------------ orders

/// The coordinate priority of an order kind: Some(None) = the legacy order (no priority), Some(Some(p)) = a
/// priority, None = not yet computable (bfi without its vote).
pub fn coordinate_priority(
    kind: OrderKind,
    shape: &FamilyShape,
    bfi: Option<&[usize]>,
) -> Option<Option<Vec<usize>>> {
    match kind {
        OrderKind::Id => Some(None),
        OrderKind::IspRev => {
            let mut p = shape.lines.clone();
            p.extend(shape.isps.iter().rev());
            Some(Some(p))
        }
        OrderKind::LinesRev => {
            let mut p: Vec<usize> = shape.lines.iter().rev().copied().collect();
            p.extend(shape.isps.iter());
            Some(Some(p))
        }
        OrderKind::Bfi => bfi.map(|b| Some(b.to_vec())),
    }
}

/// The engine's NodeOrder JSON (`node/order.rs`): comparator-only (no presentation change, input source order).
pub fn order_json(name: &str, priority: Option<&[usize]>) -> String {
    json!({"name": name, "slot_permutation": Value::Null,
        "coordinate_priority": priority.map(|p| p.to_vec()), "source_order": "Input"})
    .to_string()
}

/// R1 class of a slot in a case: 0 boundary (line at 1 / ISP at 0), 1 free, 2 interior (line >= 2 / ISP <= -1).
pub fn r1_class(is_line: bool, fixed: Option<i64>) -> u8 {
    match (is_line, fixed) {
        (_, None) => 1,
        (true, Some(v)) if v <= 1 => 0,
        (true, Some(_)) => 2,
        (false, Some(0)) => 0,
        (false, Some(_)) => 2,
    }
}

/// The BFI coordinate priority voted over `rows` (R1: boundary < free < interior inside each group): per slot the
/// weighted sum of its R1 class; lines first, then ISPs, each ascending by that sum (ties: slot order). Every case
/// gives every slot the same weight, so the sums compare exactly (no division).
pub fn bfi_priority(
    shape: &FamilyShape,
    rows: &[RuleRow],
    refused: &[Vec<Option<i64>>],
    weight: VoteWeight,
    refused_weight: u64,
) -> Vec<usize> {
    let mut score = vec![0u128; shape.arity];
    let mut add = |fixed: &[Option<i64>], w: u128| {
        if fixed.len() != shape.arity {
            return;
        }
        for (s, f) in fixed.iter().enumerate() {
            score[s] += w * r1_class(shape.is_line(s), *f) as u128;
        }
    };
    for r in rows {
        let w: u128 = match weight {
            VoteWeight::Monomials => r.monomials.max(1) as u128,
            VoteWeight::Bytes => r.bytes.max(1) as u128,
            VoteWeight::Uniform => 1,
        };
        add(&r.fixed, w);
    }
    for f in refused {
        add(f, refused_weight.max(1) as u128);
    }
    let sorted = |g: &[usize]| {
        let mut v = g.to_vec();
        v.sort_by_key(|&s| (score[s], s));
        v
    };
    let mut p = sorted(&shape.lines);
    p.extend(sorted(&shape.isps));
    p
}

// ------------------------------------------------------------------------------------------------ measures

/// One rule of a table: its case (fixed values + equations), size and record bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleRow {
    /// Canonical case key: fixed values (`_` = free) and the equations, e.g. `_,1,0|n1-n2`.
    pub case: String,
    pub fixed: Vec<Option<i64>>,
    pub monomials: u64,
    pub rhs_terms: u64,
    pub bytes: u64,
}

/// Measures of one sealed table.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TableMeasure {
    pub rules: u64,
    pub residuals: u64,
    pub residual_keys: Vec<String>,
    /// Bytes of the whole sealed table file.
    pub table_bytes: u64,
    /// Bytes of the rule records (canonical lines).
    pub rule_bytes: u64,
    pub monomials: u64,
    pub max_monomials: u64,
    pub rhs_terms: u64,
    /// An `end` record is present.
    pub end_record: bool,
    /// The engine's own wall seconds and VmHWM (end record), when stated.
    pub engine_seconds: Option<f64>,
    pub vmhwm_kb: Option<u64>,
    /// Estimated use cost (ns) under the policy's cost model.
    pub use_cost_ns: f64,
}

fn case_key(fixed: &[Option<i64>], equations: &[String]) -> String {
    let f: Vec<String> = fixed
        .iter()
        .map(|x| x.map_or("_".to_string(), |v| v.to_string()))
        .collect();
    let mut e = equations.to_vec();
    e.sort();
    format!("{}|{}", f.join(","), e.join(";"))
}

/// Measures and rule rows of a table given as its (canonical) lines.
pub fn measure_table<'a>(
    lines: impl IntoIterator<Item = &'a str>,
    table_bytes: u64,
    cost: &CostModel,
) -> Result<(TableMeasure, Vec<RuleRow>), String> {
    let mut m = TableMeasure {
        table_bytes,
        ..TableMeasure::default()
    };
    let mut rows = Vec::new();
    for (n, line) in lines.into_iter().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let v: Value =
            serde_json::from_str(line).map_err(|e| format!("table line {}: {e}", n + 1))?;
        match v.get("type").and_then(|t| t.as_str()) {
            Some("rule") => {
                let rm = super::RuleMeasure::from_record_json(line)
                    .map_err(|e| format!("table line {}: {e}", n + 1))?;
                let fixed: Vec<Option<i64>> = v
                    .get("fixed")
                    .and_then(|f| f.as_array())
                    .map(|a| a.iter().map(|x| x.as_i64()).collect())
                    .unwrap_or_default();
                let equations: Vec<String> = v
                    .get("equations")
                    .and_then(|f| f.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
                m.rules += 1;
                m.rule_bytes += rm.bytes;
                m.monomials += rm.monomials;
                m.max_monomials = m.max_monomials.max(rm.monomials);
                m.rhs_terms += rm.rhs_terms;
                rows.push(RuleRow {
                    case: case_key(&fixed, &equations),
                    fixed,
                    monomials: rm.monomials,
                    rhs_terms: rm.rhs_terms,
                    bytes: rm.bytes,
                });
            }
            Some("residual") => {
                m.residuals += 1;
                if let Some(k) = v.get("key").and_then(|k| k.as_str()) {
                    m.residual_keys.push(k.to_string());
                }
            }
            Some("end") => {
                m.end_record = true;
                m.engine_seconds = v.get("seconds").and_then(|s| s.as_f64());
                m.vmhwm_kb = v.get("vmhwm_kb").and_then(|s| s.as_u64());
            }
            _ => {}
        }
    }
    m.residual_keys.sort();
    m.use_cost_ns = use_cost_ns(cost, m.table_bytes, m.rules, m.monomials);
    Ok((m, rows))
}

/// load_ns_per_byte x bytes + rules x apply_ns_base + monomials x apply_ns_per_monomial.
pub fn use_cost_ns(c: &CostModel, bytes: u64, rules: u64, monomials: u64) -> f64 {
    c.load_ns_per_byte * bytes as f64
        + c.apply_ns_base * rules as f64
        + c.apply_ns_per_monomial * monomials as f64
}

// ------------------------------------------------------------------------------------------------ ranking

/// How a probe (or full run) ended, from the supervisor's classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// Exit class complete (row 1): the table is the whole node.
    Complete,
    /// Stopped by the work cap with rules (row 2).
    MaxCases,
    /// Stopped with open regions / obligations (row 5).
    OpenRegions,
    /// A case was refused, or a wall safety net stopped it: the candidate meets a heavy relation.
    Heavy,
    /// The job ended at its memory cap (the session's `--mem-cap` / the policy's cap): LOST AT CAP, not an error;
    /// ranked like a heavy run (its measured peak is in the record).
    LostAtCap,
    /// Nothing usable (no sealed table, refused job, crash).
    Failed,
}

impl RunStatus {
    pub fn rank(self) -> u8 {
        match self {
            RunStatus::Complete | RunStatus::MaxCases => 0,
            RunStatus::OpenRegions => 1,
            RunStatus::Heavy | RunStatus::LostAtCap => 2,
            RunStatus::Failed => 3,
        }
    }
}

/// Factor-2 band of a non-negative number (0 for 0).
pub fn band(x: f64) -> u32 {
    if !(x >= 1.0) {
        return 0;
    }
    let v = if x >= u64::MAX as f64 {
        u64::MAX
    } else {
        x as u64
    };
    64 - v.leading_zeros()
}

/// Ranking key of a probe (smaller = better); see interfaces/genchoice.md 1.3.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProbeKey {
    pub status: u8,
    /// Heavy relations the probe refused (pre-lift screen; fewer = better).
    pub refused: u64,
    pub cost_band: u32,
    pub max_band: u32,
    /// Use cost per rule, ns (rounded).
    pub cost_per_rule: u64,
    pub index: usize,
}

pub fn probe_key(
    status: RunStatus,
    m: Option<&TableMeasure>,
    refused: u64,
    index: usize,
) -> ProbeKey {
    match m {
        Some(m) if m.rules > 0 && status != RunStatus::Failed => {
            let per = m.use_cost_ns / m.rules as f64;
            ProbeKey {
                status: status.rank(),
                refused,
                cost_band: band(per),
                max_band: band(m.max_monomials as f64),
                cost_per_rule: per.round() as u64,
                index,
            }
        }
        // a HEAVY run without a table (wall safety net: the contract seals nothing for a time-decided stop) stays
        // eligible, after every measured candidate of its status
        _ if status.rank() == RunStatus::Heavy.rank() => ProbeKey {
            status: status.rank(),
            refused: u64::MAX,
            cost_band: u32::MAX,
            max_band: u32::MAX,
            cost_per_rule: u64::MAX,
            index,
        },
        _ => ProbeKey {
            status: RunStatus::Failed.rank(),
            refused: u64::MAX,
            cost_band: u32::MAX,
            max_band: u32::MAX,
            cost_per_rule: u64::MAX,
            index,
        },
    }
}

/// Candidate indices in ranking order; failed probes (no table, not heavy) excluded.
/// Each probe: (candidate index, status, measures, refused heavy relations).
pub fn ranking(probes: &[(usize, RunStatus, Option<TableMeasure>, u64)]) -> Vec<usize> {
    let mut keys: Vec<ProbeKey> = probes
        .iter()
        .map(|(i, s, m, r)| probe_key(*s, m.as_ref(), *r, *i))
        .collect();
    keys.sort();
    keys.into_iter()
        .filter(|k| k.status < RunStatus::Failed.rank())
        .map(|k| k.index)
        .collect()
}

// ------------------------------------------------------------------------------------------------ economy

/// A finished full run (or a probe that completed the node).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FullRun {
    pub cand: usize,
    pub acceptable: bool,
    pub cpu_ms: u64,
    pub measure: Option<TableMeasure>,
    /// Ran as an economy alternative (its CPU counts against the budget).
    pub alternative: bool,
}

/// The next step of the full-run phase of one node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NextStep {
    /// Run this candidate's full job (no extra cap beyond the policy's safety nets).
    First(usize),
    /// Run these alternatives (candidate, wall cap in seconds; None = no cap: optimise mode).
    Alternatives(Vec<(usize, Option<u64>)>),
    /// Every planned run is final: choose.
    Decide,
    /// No acceptable table can be reached (ranking exhausted or first attempts used up).
    NoCandidate(String),
}

/// The full-run step from the ranking and the runs so far (a pure function of them: resume = same answers).
/// `done` holds every final full run (and every probe that completed the node, flagged non-alternative).
pub fn next_step(ranking: &[usize], done: &[FullRun], p: &Policy) -> NextStep {
    let get = |c: usize| done.iter().find(|r| r.cand == c);
    let mut first_attempts = 0u32;
    let mut first: Option<(usize, u64)> = None;
    for (pos, &c) in ranking.iter().enumerate() {
        match get(c) {
            Some(r) if r.acceptable && !r.alternative => {
                first = Some((pos, r.cpu_ms));
                break;
            }
            Some(r) if !r.alternative => {
                // a probe that completed but is not acceptable, or a failed full run: counts as an attempt
                first_attempts += 1;
            }
            Some(_) => {}
            None => {
                if first_attempts >= p.full.max_first_attempts {
                    return NextStep::NoCandidate(format!(
                        "{first_attempts} full attempt(s) without an acceptable table (full.max_first_attempts)"
                    ));
                }
                return NextStep::First(c);
            }
        }
        if first_attempts >= p.full.max_first_attempts {
            return NextStep::NoCandidate(format!(
                "{first_attempts} full attempt(s) without an acceptable table (full.max_first_attempts)"
            ));
        }
    }
    let Some((pos, c1)) = first else {
        return NextStep::NoCandidate("ranking exhausted without an acceptable table".into());
    };
    let alts = alternatives(ranking, pos, done, p);
    if alts.is_empty() {
        return NextStep::Decide;
    }
    let caps = alternative_caps(c1, alts.len(), p);
    let pending: Vec<(usize, Option<u64>)> = alts
        .iter()
        .filter(|&&c| get(c).is_none())
        .map(|&c| (c, caps))
        .collect();
    if pending.is_empty() {
        NextStep::Decide
    } else {
        NextStep::Alternatives(pending)
    }
}

/// The alternatives after the first acceptable run at ranking position `pos`: the next ranked candidates that were not
/// a (failed) first attempt; at most `max_alternatives` (all under optimise; none when factor <= 0).
pub fn alternatives(ranking: &[usize], pos: usize, done: &[FullRun], p: &Policy) -> Vec<usize> {
    let tried_first = |c: usize| done.iter().any(|r| r.cand == c && !r.alternative);
    let rest = ranking[pos + 1..]
        .iter()
        .copied()
        .filter(|&c| !tried_first(c) || done.iter().any(|r| r.cand == c && r.alternative));
    if p.economy.optimise {
        return rest.collect();
    }
    if !(p.economy.factor > 0.0) || p.economy.max_alternatives == 0 {
        return vec![];
    }
    rest.take(p.economy.max_alternatives as usize).collect()
}

/// Wall cap (s) of each alternative: floor(factor x C1 / A), at least 1 s; None under optimise.
pub fn alternative_caps(c1_ms: u64, n: usize, p: &Policy) -> Option<u64> {
    if p.economy.optimise {
        return None;
    }
    let s = (p.economy.factor * c1_ms as f64 / n.max(1) as f64 / 1000.0).floor();
    Some(if s >= 1.0 { s as u64 } else { 1 })
}

/// The winner among acceptable runs under `select` (ties: bytes, largest rule, candidate index).
pub fn choose(done: &[FullRun], select: Select) -> Option<usize> {
    let mut acc: Vec<&FullRun> = done
        .iter()
        .filter(|r| r.acceptable && r.measure.is_some())
        .collect();
    let key = |r: &FullRun| {
        let m = r.measure.as_ref().expect("measured");
        let primary = match select {
            Select::UseCost => m.use_cost_ns.round() as u128,
            Select::Bytes => m.table_bytes as u128,
            Select::MaxMonomials => m.max_monomials as u128,
        };
        (primary, m.table_bytes, m.max_monomials, r.cand)
    };
    acc.sort_by_key(|r| key(r));
    acc.first().map(|r| r.cand)
}

/// The economy account of a node: C1, the budget, the alternatives' CPU and verdicts.
pub fn economy_account(ranking: &[usize], done: &[FullRun], p: &Policy) -> Value {
    let first = ranking.iter().find_map(|&c| {
        done.iter()
            .find(|r| r.cand == c && r.acceptable && !r.alternative)
    });
    let alts: Vec<&FullRun> = done.iter().filter(|r| r.alternative).collect();
    let extra: u64 = alts.iter().map(|r| r.cpu_ms).sum();
    json!({
        "factor": p.economy.factor, "optimise": p.economy.optimise,
        "max_alternatives": p.economy.max_alternatives,
        "first_acceptable": first.map(|r| r.cand), "c1_cpu_ms": first.map(|r| r.cpu_ms),
        "budget_cpu_ms": first.map(|r| if p.economy.optimise { Value::Null } else {
            json!((p.economy.factor.max(0.0) * r.cpu_ms as f64).floor() as u64) }),
        "extra_cpu_ms": extra,
        "alternatives": alts.iter().map(|r| json!({"cand": r.cand, "acceptable": r.acceptable, "cpu_ms": r.cpu_ms}))
            .collect::<Vec<_>>(),
    })
}

// ------------------------------------------------------------------------------------------------ comparison

/// Per-rule comparison of the winner's rules with the same case in each losing table: one JSON line per winner rule
/// (`case`, `winner` monomials, `others` {candidate id: monomials | null}) and a summary per loser (shared cases, the
/// summed monomials of both on them, cases only on one side, the ten shared cases with the largest loser / winner
/// ratio).
pub fn per_rule_comparison(
    winner: &[RuleRow],
    others: &[(String, Vec<RuleRow>)],
) -> (Vec<Value>, Value) {
    let maps: Vec<BTreeMap<&str, u64>> = others
        .iter()
        .map(|(_, rows)| {
            let mut m = BTreeMap::new();
            for r in rows {
                m.entry(r.case.as_str()).or_insert(r.monomials);
            }
            m
        })
        .collect();
    let mut lines = Vec::with_capacity(winner.len());
    for r in winner {
        let o: serde_json::Map<String, Value> = others
            .iter()
            .zip(&maps)
            .map(|((id, _), m)| {
                (
                    id.clone(),
                    m.get(r.case.as_str()).map_or(Value::Null, |&v| json!(v)),
                )
            })
            .collect();
        lines.push(json!({"case": r.case, "winner": r.monomials, "others": o}));
    }
    let wmap: BTreeMap<&str, u64> = {
        let mut m = BTreeMap::new();
        for r in winner {
            m.entry(r.case.as_str()).or_insert(r.monomials);
        }
        m
    };
    let mut summary = serde_json::Map::new();
    for ((id, _), m) in others.iter().zip(&maps) {
        let mut shared = 0u64;
        let (mut ws, mut os) = (0u64, 0u64);
        let mut ratios: Vec<(u64, u64, &str)> = Vec::new();
        for (case, &w) in &wmap {
            if let Some(&o) = m.get(case) {
                shared += 1;
                ws += w;
                os += o;
                ratios.push((o, w.max(1), case));
            }
        }
        // largest loser / winner ratio first (exact: compare o1 * w2 vs o2 * w1)
        ratios.sort_by(|a, b| {
            (b.0 as u128 * a.1 as u128)
                .cmp(&(a.0 as u128 * b.1 as u128))
                .then(a.2.cmp(b.2))
        });
        let top: Vec<Value> = ratios
            .iter()
            .take(10)
            .map(|(o, w, c)| json!({"case": c, "loser": o, "winner": w}))
            .collect();
        summary.insert(
            id.clone(),
            json!({"shared_cases": shared, "winner_monomials_on_shared": ws, "loser_monomials_on_shared": os,
                "only_in_winner": wmap.len() as u64 - shared, "only_in_loser": m.len() as u64 - shared,
                "largest_ratios": top}),
        );
    }
    (lines, Value::Object(summary))
}

/// The closing loop's import spec of the winner (rustred.campaign.import-spec.v1; frame from the table, start = box).
pub fn import_spec(
    node: &str,
    box_path: &str,
    box_sha256: &str,
    table_path: &str,
    table_sha256: &str,
) -> Value {
    json!({"schema": "rustred.campaign.import-spec.v1", "node": node, "frame_from": "table",
        "start": {"path": box_path, "format": "box", "sha256": box_sha256},
        "sources": [{"id": "table", "kind": "engine_table", "path": table_path, "sha256": table_sha256,
            "table_terminals": "ledger_and_residuals"}]})
}

// ------------------------------------------------------------------------------------------------ config

/// One node of the config.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeEntry {
    pub node: String,
    pub family: String,
    pub family_sha256: String,
    #[serde(default)]
    pub r#box: Option<FileEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileEntry {
    pub path: String,
    pub sha256: String,
}

/// The config file (campaign input of role `genchoice`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema: String,
    /// Key of `spec.programs` (the engine's job entry).
    pub program: String,
    pub nodes: Vec<NodeEntry>,
    #[serde(default)]
    pub policy: Policy,
}

impl Config {
    pub fn parse(text: &str) -> Result<Config, String> {
        let c: Config = serde_json::from_str(text).map_err(|e| format!("genchoice config: {e}"))?;
        if c.schema != CONFIG_SCHEMA {
            return Err(format!(
                "genchoice config: schema {:?}, expected {CONFIG_SCHEMA}",
                c.schema
            ));
        }
        if c.nodes.is_empty() {
            return Err("genchoice config: no nodes".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for n in &c.nodes {
            if n.node.is_empty()
                || !n
                    .node
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
            {
                return Err(format!("genchoice config: node id {:?}", n.node));
            }
            if !ids.insert(n.node.as_str()) {
                return Err(format!("genchoice config: node {} listed twice", n.node));
            }
            for (what, h) in [
                ("family_sha256", Some(&n.family_sha256)),
                ("box.sha256", n.r#box.as_ref().map(|b| &b.sha256)),
            ] {
                if let Some(h) = h
                    && (h.len() != 64
                        || !h
                            .chars()
                            .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase()))
                {
                    return Err(format!(
                        "genchoice config: {}: {what} must be 64 lower-case hex",
                        n.node
                    ));
                }
            }
        }
        c.policy.check()?;
        Ok(c)
    }
}
