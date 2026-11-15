//! Kind `solve-recipe` (coord-recipe-solve.md v1.1; lane adapters, interfaces/adapters.md 1.2 / 1.4): the records of a
//! sealed `out/results.jsonl`, their consistency with the job's `keys` input (part of S for this kind), and the TYPED
//! per-key outcomes: each SOLVED + proved record is acceptable on its own, every other key is an unsolved key routed by
//! its failure class.
//!
//! Everything here works on VERIFIED canonical lines (`seal::VerifiedHandle::canonical_lines`), never on a path.
//! Generic: a key is an integer vector of the length the keys input gives; nothing here knows an arity.
//!
//! Mounted at the crate root (`rustred_campaign_contract::recipe`, lane int5); `classify::recipe` re-exports it.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// `type` of a per-key record.
pub const RELATION_TYPE: &str = "relation";
/// `failure.schema` of an unsolved record.
pub const FAILURE_SCHEMA: &str = "rustred.recipe-failure.v1";
/// Line 7: `type` of a per-key record of a PROBE job (`--probe-d`): a NON-ACCEPTING record class (never a relation,
/// never counts a key as solved, never evidence).
pub const PROBE_TYPE: &str = "probe";
/// Line 7: prefix of the `determinism_id` of probe records (coord-pole-engine.md 2).
pub const PROBE_DETERMINISM_PREFIX: &str = "rustred.recipe-probe.v1/";
/// Line 7: work counters a `BUDGET_WORK` record may name in `failure.detail.counter` (the trace-row limit and the m1
/// work cap of one key).
pub const BUDGET_WORK_COUNTERS: [&str; 2] = ["trace_rows", M1_WORK_ROWS];
/// Line 7: the counter name of the `--max-m1-rows` cap.
pub const M1_WORK_ROWS: &str = "m1_work_rows";
/// Line 7 (seed minimisation): `minimise.mode` values.
pub const MINIMISE_MODES: [&str; 2] = ["support", "greedy"];
/// Line 7 (coord-seed-minimise.md 7): `minimise.measure` values.
pub const MINIMISE_MEASURES: [&str; 2] = ["comparator", "m1"];
/// Line 7: `minimise.order` values (the only order defined now: outermost shell first).
pub const MINIMISE_ORDERS: [&str; 1] = ["outermost_first"];
/// Line 7: `minimise.stopped` values.
pub const MINIMISE_STOPS: [&str; 2] = ["done", "max_solves"];
/// Line 7 (seed minimisation, coord-seed-minimise.md 6): `centre_scan.reason` values.
pub const CENTRE_SCAN_REASONS: [&str; 3] = ["regular", "smallest", "size_guard"];

/// Line 7 (seed minimisation, coord-seed-minimise.md 6): the `centre_scan` member of a RELATION or RULE record, checked
/// when present: `{entries: [{centre, verdict, rhs_before, rhs_after, solves, regular_at_d0}], chosen, reason,
/// size_ratio}`; `entries` non-empty; `centre` a scan token (`floor_of_free` / `each_free_slot`) or a list of integers;
/// `verdict` a relation verdict name; `rhs_before` / `rhs_after` unsigned integers or null (an entry that did not
/// solve); `solves` an unsigned integer; `regular_at_d0` a boolean or null; `chosen` an index into `entries`;
/// `reason` in [`CENTRE_SCAN_REASONS`]; `size_ratio` a number >= 0 or null. In a SOLVED record the chosen entry is
/// SOLVED. Deterministic (entries sequential per key / case in list order), so NON-volatile: part of the canonical
/// record.
pub fn check_centre_scan(cs: &Value, record_verdict: Option<Verdict>) -> Result<(), String> {
    let entries = cs["entries"]
        .as_array()
        .filter(|e| !e.is_empty())
        .ok_or("centre_scan.entries is not a non-empty list")?;
    let mut verdicts = Vec::with_capacity(entries.len());
    for (i, e) in entries.iter().enumerate() {
        let at = |m: &str| format!("centre_scan.entries[{i}]: {m}");
        let c = &e["centre"];
        let ok_centre = c
            .as_str()
            .is_some_and(|t| crate::arith::CENTRE_TOKENS.contains(&t))
            || c.as_array()
                .is_some_and(|v| !v.is_empty() && v.iter().all(|x| x.as_i64().is_some()));
        if !ok_centre {
            return Err(at("centre is not a scan token or an integer vector"));
        }
        let v = e["verdict"]
            .as_str()
            .and_then(Verdict::from_name)
            .ok_or_else(|| at("verdict not in the closed list"))?;
        for m in ["rhs_before", "rhs_after"] {
            if !e[m].is_null() && e[m].as_u64().is_none() {
                return Err(at(&format!("{m} is not an unsigned integer")));
            }
        }
        u64_member(e, "solves", "centre_scan entry").map_err(|m| at(&m))?;
        if !e["regular_at_d0"].is_null() && !e["regular_at_d0"].is_boolean() {
            return Err(at("regular_at_d0 is not a boolean"));
        }
        verdicts.push(v);
    }
    let chosen = u64_member(cs, "chosen", "centre_scan")? as usize;
    if chosen >= entries.len() {
        return Err(format!("centre_scan.chosen {chosen} is not an entry"));
    }
    match cs["reason"].as_str() {
        Some(r) if CENTRE_SCAN_REASONS.contains(&r) => {}
        _ => {
            return Err(format!(
                "centre_scan.reason {} not in {CENTRE_SCAN_REASONS:?}",
                cs["reason"]
            ));
        }
    }
    let ratio = &cs["size_ratio"];
    if !ratio.is_null() && !ratio.as_f64().is_some_and(|x| x >= 0.0) {
        return Err("centre_scan.size_ratio is not a number >= 0".into());
    }
    if record_verdict == Some(Verdict::Solved) && verdicts[chosen] != Verdict::Solved {
        return Err("a SOLVED record whose chosen centre did not solve".into());
    }
    Ok(())
}

/// Line 7 (multi-term symmetry rows): `symmetry_rows_used.position` values.
pub const SYMMETRY_POSITIONS: [&str; 2] = ["key_first", "after_seed"];

/// Line 7 (seed minimisation, coord-seed-minimise.md 5 / 7): the `minimise` member of a RELATION or RULE record,
/// checked when present: `{mode, measure, order, solves, stopped, seeds_before, seeds_after, rhs_before, rhs_after,
/// original_relation_digest}` (+ optional `target_domain`, a case domain ([`crate::arith::check_case_domain`]), and
/// `target_domain_blake3`, 64 hex, only beside it) with mode / measure / order / stopped from their closed lists, the
/// counts unsigned integers, `seeds_after <= seeds_before`, `stopped = "max_solves"` only with `solves >= 1`, and the
/// digest 64 hex. It is deterministic (a work budget, sequential decisions per key / case), so NON-volatile: part of
/// the canonical record.
pub fn check_minimise(m: &Value) -> Result<(), String> {
    let pick = |f: &str, list: &[&str]| -> Result<(), String> {
        match m[f].as_str() {
            Some(v) if list.contains(&v) => Ok(()),
            _ => Err(format!("minimise.{f} {} not in {list:?}", m[f])),
        }
    };
    pick("mode", &MINIMISE_MODES)?;
    pick("measure", &MINIMISE_MEASURES)?;
    if !m["target_domain"].is_null() {
        crate::arith::check_case_domain(&m["target_domain"])
            .map_err(|e| format!("minimise.target_domain: {e}"))?;
    }
    if !m["target_domain_blake3"].is_null()
        && (m["target_domain"].is_null()
            || !m["target_domain_blake3"].as_str().is_some_and(is_hex64))
    {
        return Err("minimise.target_domain_blake3 is not 64 hex beside a target_domain".into());
    }
    pick("order", &MINIMISE_ORDERS)?;
    pick("stopped", &MINIMISE_STOPS)?;
    let solves = u64_member(m, "solves", "minimise")?;
    let before = u64_member(m, "seeds_before", "minimise")?;
    let after = u64_member(m, "seeds_after", "minimise")?;
    u64_member(m, "rhs_before", "minimise")?;
    u64_member(m, "rhs_after", "minimise")?;
    if after > before {
        return Err(format!(
            "minimise.seeds_after {after} > seeds_before {before}"
        ));
    }
    if m["stopped"] == "max_solves" && solves == 0 {
        return Err("minimise stopped at max_solves after 0 solves".into());
    }
    if !m["original_relation_digest"].as_str().is_some_and(is_hex64) {
        return Err("minimise.original_relation_digest is not 64 hex".into());
    }
    Ok(())
}

/// Line 7 (multi-term symmetry rows, coord-pole-engine.md 9): `symmetry_rows_used {elements, rows, terms, position}` and
/// the proof's `symmetry {rows, form_check: "exact", group_order}`, checked when present; a SOLVED record that used
/// symmetry rows (`symmetry_rows_used.rows >= 1`) carries the proof block; `ibp_rows` (9d) is an unsigned integer and a
/// SOLVED record with `ibp_rows: 0` used symmetry rows. Deterministic: NON-volatile.
pub fn check_symmetry_members(r: &Value, verdict: Verdict) -> Result<(), String> {
    let used = &r["symmetry_rows_used"];
    let mut rows_used = 0;
    if !used.is_null() {
        u64_member(used, "elements", "symmetry_rows_used")?;
        u64_member(used, "terms", "symmetry_rows_used")?;
        rows_used = u64_member(used, "rows", "symmetry_rows_used")?;
        match used["position"].as_str() {
            Some(p) if SYMMETRY_POSITIONS.contains(&p) => {}
            _ => {
                return Err(format!(
                    "symmetry_rows_used.position {} not in {SYMMETRY_POSITIONS:?}",
                    used["position"]
                ));
            }
        }
    }
    // coord-pole-engine.md 9d: `ibp_rows` (0 when the recipe has `seeds.ibp_rows: false`): a SOLVED relation from
    // symmetry rows only names them
    if !r["ibp_rows"].is_null() {
        let ibp = r["ibp_rows"]
            .as_u64()
            .ok_or("ibp_rows is not an unsigned integer")?;
        if ibp == 0 && verdict == Verdict::Solved && rows_used == 0 {
            return Err("SOLVED with ibp_rows 0 and no symmetry rows used".into());
        }
    }
    let ps = &r["proof"]["symmetry"];
    if !ps.is_null() {
        u64_member(ps, "rows", "proof.symmetry")?;
        u64_member(ps, "group_order", "proof.symmetry")?;
        if ps["form_check"] != "exact" {
            return Err("proof.symmetry.form_check is not \"exact\"".into());
        }
    } else if verdict == Verdict::Solved && rows_used >= 1 {
        return Err("SOLVED with symmetry rows but no proof.symmetry block".into());
    }
    Ok(())
}

/// Verdicts (closed list v1.1, coord-recipe-solve.md 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Verdict {
    Solved,
    NoRelationInSeeds,
    NotInDomain,
    NotLower,
    Irregular,
    ProofFailed,
    DigestMismatch,
    BudgetWork,
    BudgetWall,
    Interrupted,
    Error,
    /// `--no-proof` diagnostics; never accepted, never in a job's sealed file.
    SolvedUnproved,
    /// Line 7 (lift mode with the recipe's `elimination.discovery_d` set): the d0 trace holds no GENERIC target relation.
    NotGenericAtD0,
    /// Line 7 (discovery_d set): a seed row's coefficient denominator vanishes at d0 (typed, never a silent redraw).
    D0SingularRows,
    /// Line 7 (multi-term symmetry rows, coord-pole-engine.md 9): an element's image form does not decompose EXACTLY
    /// (fail closed; never a resource failure, never evidence).
    SymmetryFormCheckFailed,
}

impl Verdict {
    pub const ALL: [Verdict; 15] = [
        Verdict::Solved,
        Verdict::NoRelationInSeeds,
        Verdict::NotInDomain,
        Verdict::NotLower,
        Verdict::Irregular,
        Verdict::ProofFailed,
        Verdict::DigestMismatch,
        Verdict::BudgetWork,
        Verdict::BudgetWall,
        Verdict::Interrupted,
        Verdict::Error,
        Verdict::SolvedUnproved,
        Verdict::NotGenericAtD0,
        Verdict::D0SingularRows,
        Verdict::SymmetryFormCheckFailed,
    ];

    /// The verdict's name as the verb writes it.
    pub fn name(self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default()
    }

    pub fn from_name(s: &str) -> Option<Verdict> {
        serde_json::from_value(Value::String(s.to_string())).ok()
    }

    /// The failure class of a non-SOLVED verdict (= the verdict in lower case); `None` for SOLVED / SOLVED_UNPROVED.
    pub const fn class(self) -> Option<FailureClass> {
        Some(match self {
            Verdict::Solved | Verdict::SolvedUnproved => return None,
            Verdict::NoRelationInSeeds => FailureClass::NoRelationInSeeds,
            Verdict::NotInDomain => FailureClass::NotInDomain,
            Verdict::NotLower => FailureClass::NotLower,
            Verdict::Irregular => FailureClass::Irregular,
            Verdict::ProofFailed => FailureClass::ProofFailed,
            Verdict::DigestMismatch => FailureClass::DigestMismatch,
            Verdict::BudgetWork => FailureClass::BudgetWork,
            Verdict::BudgetWall => FailureClass::BudgetWall,
            Verdict::Interrupted => FailureClass::Interrupted,
            Verdict::Error => FailureClass::Error,
            Verdict::NotGenericAtD0 => FailureClass::NotGenericAtD0,
            Verdict::D0SingularRows => FailureClass::D0SingularRows,
            Verdict::SymmetryFormCheckFailed => FailureClass::SymmetryFormCheckFailed,
        })
    }

    /// Line 7: the verdict exists only when the recipe fixes the discovery point's d (`discovery_d`).
    pub const fn needs_discovery_d(self) -> bool {
        matches!(self, Verdict::NotGenericAtD0 | Verdict::D0SingularRows)
    }
}

/// Failure classes (`failure.class`, the verdict in lower case).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    NoRelationInSeeds,
    NotInDomain,
    NotLower,
    Irregular,
    ProofFailed,
    DigestMismatch,
    BudgetWork,
    BudgetWall,
    Interrupted,
    Error,
    /// Line 7: per-key, routed like `no_relation_in_seeds` (next escalation step); never a resource failure.
    NotGenericAtD0,
    /// Line 7: per-key, routed like `no_relation_in_seeds` (next escalation step); never a resource failure.
    D0SingularRows,
    /// Line 7: per-key, routed like `no_relation_in_seeds` (next escalation step); never a resource failure, never
    /// evidence.
    SymmetryFormCheckFailed,
    /// Lane symjob (kind symmetry-composite): no element of the verified group gives a composed relation that descends
    /// at the key (outcome `none`): per-key, routed to the next route of the repair flow (like no_relation_in_seeds);
    /// never a job failure. Never produced by a solve-recipe record (classes there come from the verdict).
    NoSymmetryRelation,
    /// Lane symjob (kind symmetry-composite): the symmetry row expansion exceeded the spec bound `route0.max_terms`
    /// (outcome `over_cap`): per-key, routed to the next work class (a larger bound); never "no relation".
    OverCapRowExpansion,
}

impl FailureClass {
    pub fn name(self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default()
    }

    /// What the caller does with a key of this class (interfaces/adapters.md 1.4).
    pub const fn route(self) -> KeyRoute {
        match self {
            FailureClass::NoRelationInSeeds
            | FailureClass::NotInDomain
            | FailureClass::NotGenericAtD0
            | FailureClass::D0SingularRows
            | FailureClass::SymmetryFormCheckFailed
            | FailureClass::NoSymmetryRelation => KeyRoute::NextRecipeRung,
            FailureClass::BudgetWork | FailureClass::OverCapRowExpansion => KeyRoute::NextWorkClass,
            FailureClass::BudgetWall => KeyRoute::LadderDecides,
            FailureClass::Irregular => KeyRoute::SubCase,
            FailureClass::Interrupted => KeyRoute::Rerun,
            FailureClass::ProofFailed
            | FailureClass::NotLower
            | FailureClass::Error
            | FailureClass::DigestMismatch => KeyRoute::Attention,
        }
    }

    /// The class is decided by a clock (per-key wall budget).
    pub const fn time_decided(self) -> bool {
        matches!(self, FailureClass::BudgetWall)
    }
}

/// Route of an unsolved key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyRoute {
    /// Another recipe (the next recipe rung) for that key.
    NextRecipeRung,
    /// The same recipe with the next work class (a larger `--max-trace-rows`).
    NextWorkClass,
    /// Time-decided (per-key wall budget): the ladder decides.
    LadderDecides,
    /// The pole locus is a sub-case (not a rung).
    SubCase,
    /// Interrupted before it was solved: run again (never inside a sealed `end` file).
    Rerun,
    /// Never retried silently.
    Attention,
    /// Line 7, probe verdict `UNRESOLVED_PRIMES` (the primes disagree): probe again with other primes, then attention.
    /// Never evidence.
    OtherPrimesThenAttention,
}

/// Line 7: verdicts of a `probe` record (closed list, coord-pole-engine.md 8). The first four ANSWER the key (exit 0
/// when every key has one); the others leave it unanswered (exit 10, stop `keys_unsolved`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProbeVerdict {
    /// The target is a pivot at d0 and the trace carries a generic relation.
    ExistsAtD0,
    /// The target is a pivot at d0, no generic relation in its trace.
    ExistsOnlyAtD0,
    /// No pivot at d0: a LOWER bound ("not in these seeds at d0").
    NoRelationAtD0,
    /// A seed row is not evaluable at d0.
    D0SingularRows,
    /// The per-prime verdicts differ.
    UnresolvedPrimes,
    BudgetWall,
    Error,
    /// Never inside a sealed `end` file.
    Interrupted,
}

impl ProbeVerdict {
    pub const ALL: [ProbeVerdict; 8] = [
        ProbeVerdict::ExistsAtD0,
        ProbeVerdict::ExistsOnlyAtD0,
        ProbeVerdict::NoRelationAtD0,
        ProbeVerdict::D0SingularRows,
        ProbeVerdict::UnresolvedPrimes,
        ProbeVerdict::BudgetWall,
        ProbeVerdict::Error,
        ProbeVerdict::Interrupted,
    ];

    pub fn name(self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default()
    }

    pub fn from_name(s: &str) -> Option<ProbeVerdict> {
        serde_json::from_value(Value::String(s.to_string())).ok()
    }

    /// One of the four answers (also the only verdicts a `per_prime` entry may carry).
    pub const fn is_answer(self) -> bool {
        matches!(
            self,
            ProbeVerdict::ExistsAtD0
                | ProbeVerdict::ExistsOnlyAtD0
                | ProbeVerdict::NoRelationAtD0
                | ProbeVerdict::D0SingularRows
        )
    }

    /// What the caller does with an UNANSWERED key; `None` for an answer (a diagnostic, never evidence).
    pub const fn route(self) -> Option<KeyRoute> {
        match self {
            ProbeVerdict::UnresolvedPrimes => Some(KeyRoute::OtherPrimesThenAttention),
            ProbeVerdict::BudgetWall => Some(KeyRoute::LadderDecides),
            ProbeVerdict::Error => Some(KeyRoute::Attention),
            ProbeVerdict::Interrupted => Some(KeyRoute::Rerun),
            _ => None,
        }
    }

    /// Decided by a clock (per-key wall budget).
    pub const fn time_decided(self) -> bool {
        matches!(self, ProbeVerdict::BudgetWall)
    }
}

/// Line 7: the per-key outcome of one input line of a PROBE job. Never an accepted relation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeOutcome {
    pub ordinal: u64,
    pub key: Vec<i64>,
    pub tag: Option<String>,
    pub verdict: ProbeVerdict,
    /// `None` for an answer; the route of an unanswered key otherwise.
    pub route: Option<KeyRoute>,
    pub recipe_digest: String,
    pub determinism_id: String,
    /// The verified canonical line of the record.
    pub canonical: Vec<u8>,
}

/// One line of a `keys` input: `[ints]` or `{"key":[ints], "tag": "..", ...}` (other members ignored).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyLine {
    pub key: Vec<i64>,
    #[serde(default)]
    pub tag: Option<String>,
}

/// Parse a `keys` input (JSONL; blank lines skipped). Every key must have the length of the first.
pub fn parse_keys_input(bytes: &[u8]) -> Result<Vec<KeyLine>, String> {
    let text = std::str::from_utf8(bytes).map_err(|e| format!("keys input: {e}"))?;
    let mut out: Vec<KeyLine> = Vec::new();
    for (i, l) in text.lines().enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        let v: Value =
            serde_json::from_str(l).map_err(|e| format!("keys input line {}: {e}", i + 1))?;
        let (key, tag) = match &v {
            Value::Array(_) => (v.clone(), None),
            Value::Object(m) => (
                m.get("key").cloned().unwrap_or(Value::Null),
                m.get("tag").and_then(Value::as_str).map(str::to_string),
            ),
            _ => return Err(format!("keys input line {}: not a key", i + 1)),
        };
        let key: Vec<i64> = serde_json::from_value(key)
            .map_err(|e| format!("keys input line {}: key: {e}", i + 1))?;
        if key.is_empty() || out.first().is_some_and(|f| f.key.len() != key.len()) {
            return Err(format!("keys input line {}: key length differs", i + 1));
        }
        out.push(KeyLine { key, tag });
    }
    Ok(out)
}

/// An accepted relation: a SOLVED, exactly proved, descending record at the key of its input line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptedRelation {
    pub ordinal: u64,
    pub key: Vec<i64>,
    pub tag: Option<String>,
    pub relation_digest: String,
    /// Right-hand-side keys (from `rhs[*][0]`, `I(..)` texts), in record order.
    pub rhs_keys: Vec<Vec<i64>>,
    pub recipe_digest: String,
    pub determinism_id: String,
    /// The verified canonical line of the record.
    pub canonical: Vec<u8>,
}

/// An unsolved key: a typed per-key outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnsolvedKey {
    pub ordinal: u64,
    pub key: Vec<i64>,
    pub tag: Option<String>,
    pub verdict: Verdict,
    pub class: FailureClass,
    pub route: KeyRoute,
    /// The record's `failure` object, verbatim.
    pub failure: Value,
    pub recipe_digest: String,
    pub determinism_id: String,
}

/// The per-key outcome of one input line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum KeyOutcome {
    Accepted(AcceptedRelation),
    Unsolved(UnsolvedKey),
}

impl KeyOutcome {
    pub fn ordinal(&self) -> u64 {
        match self {
            KeyOutcome::Accepted(a) => a.ordinal,
            KeyOutcome::Unsolved(u) => u.ordinal,
        }
    }
}

/// What a sealed results file holds (part of `classify::VerifiedOutputs` for kind solve-recipe).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationTally {
    /// Relation records (= lines of the keys input).
    pub records: u64,
    /// Accepted records (SOLVED + proved + descending).
    pub accepted: u64,
    /// Unsolved records per failure class.
    pub unsolved: BTreeMap<FailureClass, u64>,
    pub recipe_digest: Option<String>,
    pub determinism_id: Option<String>,
    /// Line 7: the job ran in PROBE mode (`--probe-d`): `records` counts probe records, `accepted` is 0 and
    /// `unsolved` empty; the probe records per verdict are in `probes`.
    #[serde(default)]
    pub probe: bool,
    /// Line 7: probe records per verdict (probe mode only).
    #[serde(default)]
    pub probes: BTreeMap<ProbeVerdict, u64>,
}

impl RelationTally {
    /// Line 7: probe records whose key is not answered (UNRESOLVED_PRIMES, BUDGET_WALL, ERROR, INTERRUPTED).
    pub fn unanswered_probes(&self) -> u64 {
        self.probes
            .iter()
            .filter(|(v, _)| !v.is_answer())
            .map(|(_, n)| n)
            .sum()
    }
    pub fn unsolved_total(&self) -> u64 {
        self.unsolved.values().sum()
    }
    /// Keys whose outcome a clock decided (per-key wall budget).
    pub fn time_decided_keys(&self) -> u64 {
        self.unsolved
            .iter()
            .filter(|(c, _)| c.time_decided())
            .map(|(_, n)| n)
            .sum::<u64>()
            + self
                .probes
                .iter()
                .filter(|(v, _)| v.time_decided())
                .map(|(_, n)| n)
                .sum::<u64>()
    }
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Parse an `I(a,b,..)` key text of length `arity`.
pub fn parse_key_text(t: &str, arity: usize) -> Option<Vec<i64>> {
    let inner = t.trim().strip_prefix("I(")?.strip_suffix(')')?;
    let v: Option<Vec<i64>> = inner.split(',').map(|x| x.trim().parse().ok()).collect();
    v.filter(|v| v.len() == arity)
}

fn same_or_set(slot: &mut Option<String>, v: Option<&str>, what: &str) -> Result<String, String> {
    let v = v.ok_or_else(|| format!("{what} missing"))?;
    match slot {
        None => {
            *slot = Some(v.to_string());
        }
        Some(old) if old != v => return Err(format!("two different {what} values in one file")),
        _ => {}
    }
    Ok(v.to_string())
}

/// The per-key outcomes of a VERIFIED results file (its canonical lines, end record last) against the job's keys
/// input, with the tally. `Err` = the file is not a consistent results file of this job (S false: contract
/// violation). Checks (interfaces/adapters.md 1.2): one relation record per input line, in input order, ordinal and key
/// equal; closed verdict list; SOLVED records proved, descending, with a 64-hex relation digest and rhs, no failure;
/// unsolved records with `failure.class` = the verdict in lower case and `failure.schema`; no INTERRUPTED /
/// SOLVED_UNPROVED record in an `end` file; one recipe digest and one determinism id in the whole file; the end
/// record's `records` and `counts` equal the records.
pub fn key_outcomes(
    lines: &[Vec<u8>],
    keys: &[KeyLine],
) -> Result<(Vec<KeyOutcome>, RelationTally), String> {
    key_outcomes_with(lines, keys, None)
}

/// [`key_outcomes`] of a solve job whose spec sets `--max-m1-rows` (line 7): a record whose `m1_work.rows` exceeds the
/// cap must be `BUDGET_WORK` with `failure.detail {counter: "m1_work_rows", limit: cap, value > cap}`; that detail
/// without the cap is refused. Line-7 members are checked when present ([`check_line7_relation`]); a `probe` record in a
/// solve job is a contract violation.
pub fn key_outcomes_with(
    lines: &[Vec<u8>],
    keys: &[KeyLine],
    max_m1_rows: Option<u64>,
) -> Result<(Vec<KeyOutcome>, RelationTally), String> {
    let (end_line, recs) = lines.split_last().ok_or("results file without records")?;
    let end: Value = serde_json::from_slice(end_line).map_err(|e| format!("end record: {e}"))?;
    let end_type = end["type"].as_str().unwrap_or("");
    if !matches!(end_type, "end" | "partial") {
        return Err(format!(
            "last record of type {end_type:?} is not an end record"
        ));
    }
    let sealed_end = end_type == "end";
    if recs.len() != keys.len() {
        return Err(format!(
            "{} relation records for {} input keys",
            recs.len(),
            keys.len()
        ));
    }
    let arity = keys.first().map_or(0, |k| k.key.len());
    let mut t = RelationTally::default();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut out = Vec::with_capacity(recs.len());
    // line 7: the recipe's discovery_d, echoed by every record (one recipe per file): None = not seen yet
    let mut discovery_d: Option<Option<String>> = None;
    for (i, (line, want)) in recs.iter().zip(keys).enumerate() {
        let r: Value = serde_json::from_slice(line).map_err(|e| format!("record {i}: {e}"))?;
        let at = |m: &str| format!("record {i}: {m}");
        if r["type"] == PROBE_TYPE {
            return Err(at("a probe record in a solve job (no --probe-d)"));
        }
        if r["type"] != RELATION_TYPE {
            return Err(at("not a relation record"));
        }
        if r["ordinal"].as_u64() != Some(i as u64) {
            return Err(at("ordinal is not the input line number"));
        }
        let key: Vec<i64> =
            serde_json::from_value(r["key"].clone()).map_err(|e| at(&format!("key: {e}")))?;
        if key != want.key {
            return Err(at("key is not the input line's key"));
        }
        let verdict = r["verdict"]
            .as_str()
            .and_then(Verdict::from_name)
            .ok_or_else(|| at("verdict not in the closed list"))?;
        *counts.entry(verdict.name()).or_default() += 1;
        let recipe_digest = same_or_set(
            &mut t.recipe_digest,
            r["recipe_digest"].as_str(),
            "recipe_digest",
        )
        .map_err(|e| at(&e))?;
        let determinism_id = same_or_set(
            &mut t.determinism_id,
            r["determinism_id"].as_str(),
            "determinism_id",
        )
        .map_err(|e| at(&e))?;
        check_line7_relation(&r, verdict, max_m1_rows, &mut discovery_d).map_err(|e| at(&e))?;
        t.records += 1;
        match verdict {
            Verdict::Solved => {
                let digest = r["relation_digest"].as_str().unwrap_or("");
                if !is_hex64(digest) {
                    return Err(at("SOLVED without a 64-hex relation_digest"));
                }
                if r["proof"]["ok"] != true {
                    return Err(at("SOLVED without an exact proof (proof.ok)"));
                }
                if r["all_lower"] != true {
                    return Err(at("SOLVED with a right-hand side not all lower"));
                }
                if !r["failure"].is_null() {
                    return Err(at("SOLVED with a failure object"));
                }
                let rhs = r["rhs"]
                    .as_array()
                    .ok_or_else(|| at("SOLVED without rhs"))?;
                let mut rhs_keys = Vec::with_capacity(rhs.len());
                for x in rhs {
                    let txt = x[0].as_str().ok_or_else(|| at("rhs entry"))?;
                    rhs_keys.push(
                        parse_key_text(txt, arity)
                            .ok_or_else(|| at(&format!("rhs key {txt:?}")))?,
                    );
                }
                t.accepted += 1;
                out.push(KeyOutcome::Accepted(AcceptedRelation {
                    ordinal: i as u64,
                    key,
                    tag: want.tag.clone(),
                    relation_digest: digest.to_string(),
                    rhs_keys,
                    recipe_digest,
                    determinism_id,
                    canonical: line.clone(),
                }));
            }
            Verdict::SolvedUnproved => return Err(at("SOLVED_UNPROVED in a job's results")),
            v => {
                let class = v.class().expect("non-SOLVED verdict has a class");
                if class == FailureClass::Interrupted && sealed_end {
                    return Err(at("INTERRUPTED record in a sealed `end` file"));
                }
                let f = &r["failure"];
                if f["schema"] != FAILURE_SCHEMA || f["class"].as_str() != Some(&class.name()) {
                    return Err(at(
                        "failure object without the schema or with another class",
                    ));
                }
                if !r["relation_digest"].is_null() {
                    return Err(at("an unsolved record with a relation digest"));
                }
                *t.unsolved.entry(class).or_default() += 1;
                out.push(KeyOutcome::Unsolved(UnsolvedKey {
                    ordinal: i as u64,
                    key,
                    tag: want.tag.clone(),
                    verdict: v,
                    class,
                    route: class.route(),
                    failure: f.clone(),
                    recipe_digest,
                    determinism_id,
                }));
            }
        }
    }
    check_end(
        &end,
        recs.len(),
        &counts,
        [
            ("recipe_digest", &t.recipe_digest),
            ("determinism_id", &t.determinism_id),
        ],
        IDS_REQUIRED,
    )?;
    Ok((out, t))
}

fn u64_member(o: &Value, m: &str, what: &str) -> Result<u64, String> {
    o[m].as_u64()
        .ok_or_else(|| format!("{what}.{m} is not an unsigned integer"))
}

fn string_list(v: &Value, what: &str) -> Result<Vec<String>, String> {
    v.as_array()
        .ok_or_else(|| format!("{what} is not a list"))?
        .iter()
        .map(|x| {
            x.as_str()
                .map(str::to_string)
                .ok_or_else(|| format!("{what}: an entry is not a string"))
        })
        .collect()
}

/// Line 7 members of a `relation` record, checked when present (coord-pole-engine.md 8; child-job.md "Line 7"). They
/// are deterministic by the engine's statement, so NON-volatile: part of the canonical record (`canon` lists no
/// volatile key for them).
/// - `discovery {rank, rows, columns}`: unsigned integers, rank <= rows and rank <= columns;
/// - `hidden_d_poles`, `coef_d_poles`: lists of strings, both or neither, and then `d_poles` (kept for old readers) is
///   exactly their union as a set;
/// - `m1_work {sub_solves, rows, trace_rows}`: unsigned integers; with the cap `max_m1_rows` = N, `rows > N` only in a
///   `BUDGET_WORK` record whose `failure.detail` is `{counter: "m1_work_rows", limit: N, value > N}`;
/// - `discovery_d`: the canonical rational text, the same in every record of the file (one recipe); the verdicts
///   `NOT_GENERIC_AT_D0` / `D0_SINGULAR_ROWS` only with it;
/// - `minimise` ([`check_minimise`]), `centre_scan` ([`check_centre_scan`]), `symmetry_rows_used` / `proof.symmetry` ([`check_symmetry_members`]);
/// - `failure.detail.rank` / `.columns`: unsigned integers when present; `failure.detail.counter`: in a `BUDGET_WORK`
///   record one of [`BUDGET_WORK_COUNTERS`] when present, and `m1_work_rows` in no other verdict.
pub fn check_line7_relation(
    r: &Value,
    verdict: Verdict,
    max_m1_rows: Option<u64>,
    discovery_seen: &mut Option<Option<String>>,
) -> Result<(), String> {
    if !r["discovery"].is_null() {
        let d = &r["discovery"];
        let rank = u64_member(d, "rank", "discovery")?;
        let rows = u64_member(d, "rows", "discovery")?;
        let columns = u64_member(d, "columns", "discovery")?;
        if rank > rows || rank > columns {
            return Err(format!(
                "discovery rank {rank} exceeds rows {rows} or columns {columns}"
            ));
        }
    }
    match (r["hidden_d_poles"].is_null(), r["coef_d_poles"].is_null()) {
        (true, true) => {}
        (false, false) => {
            let mut union = string_list(&r["hidden_d_poles"], "hidden_d_poles")?;
            union.extend(string_list(&r["coef_d_poles"], "coef_d_poles")?);
            union.sort();
            union.dedup();
            let mut all = string_list(
                &r["d_poles"],
                "d_poles (with hidden_d_poles / coef_d_poles)",
            )?;
            all.sort();
            all.dedup();
            if all != union {
                return Err("d_poles is not the union of hidden_d_poles and coef_d_poles".into());
            }
        }
        _ => return Err("hidden_d_poles and coef_d_poles come together".into()),
    }
    let detail = &r["failure"]["detail"];
    let m1_detail = detail["counter"] == M1_WORK_ROWS;
    if !r["m1_work"].is_null() {
        let m = &r["m1_work"];
        u64_member(m, "sub_solves", "m1_work")?;
        u64_member(m, "trace_rows", "m1_work")?;
        let rows = u64_member(m, "rows", "m1_work")?;
        if let Some(cap) = max_m1_rows
            && rows > cap
            && !(verdict == Verdict::BudgetWork && m1_detail)
        {
            return Err(format!(
                "m1_work.rows {rows} exceeds --max-m1-rows {cap} but the key is not BUDGET_WORK (m1_work_rows)"
            ));
        }
    }
    if !detail.is_null() {
        for m in ["rank", "columns"] {
            if !detail[m].is_null() && detail[m].as_u64().is_none() {
                return Err(format!("failure.detail.{m} is not an unsigned integer"));
            }
        }
        if verdict == Verdict::BudgetWork && !detail["counter"].is_null() {
            let c = detail["counter"].as_str().unwrap_or("");
            if !BUDGET_WORK_COUNTERS.contains(&c) {
                return Err(format!(
                    "BUDGET_WORK counter {:?} not in the closed list",
                    detail["counter"]
                ));
            }
        }
        if m1_detail {
            if verdict != Verdict::BudgetWork {
                return Err(format!(
                    "counter m1_work_rows in a {} record",
                    verdict.name()
                ));
            }
            let Some(cap) = max_m1_rows else {
                return Err("BUDGET_WORK m1_work_rows without --max-m1-rows".into());
            };
            let limit = u64_member(detail, "limit", "failure.detail")?;
            let value = u64_member(detail, "value", "failure.detail")?;
            if limit != cap || value <= limit {
                return Err(format!(
                    "BUDGET_WORK m1_work_rows limit {limit} / value {value} for --max-m1-rows {cap}"
                ));
            }
        }
    }
    let dd = match &r["discovery_d"] {
        Value::Null => None,
        Value::String(t) => {
            crate::arith::Rational::parse_canonical(t).map_err(|e| format!("discovery_d: {e}"))?;
            Some(t.clone())
        }
        _ => return Err("discovery_d is not a rational text".into()),
    };
    match discovery_seen {
        None => *discovery_seen = Some(dd.clone()),
        Some(prev) if *prev != dd => {
            return Err("two different discovery_d values in one file (one recipe)".into());
        }
        _ => {}
    }
    if verdict.needs_discovery_d() && dd.is_none() {
        return Err(format!("{} without discovery_d", verdict.name()));
    }
    if !r["minimise"].is_null() {
        check_minimise(&r["minimise"])?;
    }
    if !r["centre_scan"].is_null() {
        check_centre_scan(&r["centre_scan"], Some(verdict))?;
    }
    check_symmetry_members(r, verdict)?;
    Ok(())
}

const IDS_REQUIRED: bool = false;
/// A probe end record (coord-pole-engine.md 8: `type: "end"` with `counts`) need not repeat the ids; when it does, they
/// must be the records'.
const IDS_WHEN_PRESENT: bool = true;

fn check_end(
    end: &Value,
    n: usize,
    counts: &BTreeMap<String, u64>,
    ids: [(&str, &Option<String>); 2],
    ids_optional: bool,
) -> Result<(), String> {
    if end["records"].as_u64() != Some(n as u64) {
        return Err("end record `records` differs from the records".into());
    }
    let end_counts: BTreeMap<String, u64> =
        serde_json::from_value(end["counts"].clone()).map_err(|e| format!("end counts: {e}"))?;
    if end_counts != *counts {
        return Err("end record `counts` differ from the records".into());
    }
    if n > 0 {
        for (field, want) in ids {
            if ids_optional && end[field].is_null() {
                continue;
            }
            if end[field].as_str() != want.as_deref() {
                return Err(format!("end record {field} differs from the records'"));
            }
        }
    }
    Ok(())
}

/// Line 7: the per-key outcomes of a VERIFIED results file of a PROBE job (`--probe-d`, [`crate::spec::ProbeArgs`])
/// against its keys input. `Err` = not a consistent probe results file of this job (S false: contract violation).
/// Checks: one `probe` record per input line, in input order, ordinal and key equal (a `relation` record in a probe job
/// is refused); verdict in the closed list [`ProbeVerdict`]; `d0` = the job's `--probe-d` (canonical text); one
/// `recipe.digest` (64 hex) and one `determinism_id` (prefix [`PROBE_DETERMINISM_PREFIX`]) in the file; no relation
/// members (`relation_digest`, `rhs` null or absent); `per_prime` a list of `{prime, verdict, ...}` whose primes are the
/// job's primes in order (all of them for an answer and for UNRESOLVED_PRIMES, a prefix otherwise), per-prime verdicts
/// answers only, `rank` / `rows` / `columns` / `trace_rows` unsigned integers or null; the combined verdict = the common
/// per-prime verdict, UNRESOLVED_PRIMES iff they differ; no INTERRUPTED record in a sealed `end` file; the end record's
/// `records` and `counts` equal the records (its `recipe_digest` / `determinism_id`, when present, the records').
/// The tally has `probe = true`, `accepted = 0`: nothing in a probe file is ever accepted.
pub fn probe_outcomes(
    lines: &[Vec<u8>],
    keys: &[KeyLine],
    args: &crate::spec::ProbeArgs,
) -> Result<(Vec<ProbeOutcome>, RelationTally), String> {
    let (end_line, recs) = lines.split_last().ok_or("results file without records")?;
    let end: Value = serde_json::from_slice(end_line).map_err(|e| format!("end record: {e}"))?;
    let end_type = end["type"].as_str().unwrap_or("");
    if !matches!(end_type, "end" | "partial") {
        return Err(format!(
            "last record of type {end_type:?} is not an end record"
        ));
    }
    let sealed_end = end_type == "end";
    if recs.len() != keys.len() {
        return Err(format!(
            "{} probe records for {} input keys",
            recs.len(),
            keys.len()
        ));
    }
    let d0 = args.d0.to_string();
    let mut t = RelationTally {
        probe: true,
        ..RelationTally::default()
    };
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut out = Vec::with_capacity(recs.len());
    for (i, (line, want)) in recs.iter().zip(keys).enumerate() {
        let r: Value = serde_json::from_slice(line).map_err(|e| format!("record {i}: {e}"))?;
        let at = |m: &str| format!("record {i}: {m}");
        if r["type"] == RELATION_TYPE {
            return Err(at("a relation record in a probe job (--probe-d)"));
        }
        if r["type"] != PROBE_TYPE {
            return Err(at("not a probe record"));
        }
        if r["ordinal"].as_u64() != Some(i as u64) {
            return Err(at("ordinal is not the input line number"));
        }
        let key: Vec<i64> =
            serde_json::from_value(r["key"].clone()).map_err(|e| at(&format!("key: {e}")))?;
        if key != want.key {
            return Err(at("key is not the input line's key"));
        }
        let verdict = r["verdict"]
            .as_str()
            .and_then(ProbeVerdict::from_name)
            .ok_or_else(|| at("probe verdict not in the closed list"))?;
        if verdict == ProbeVerdict::Interrupted && sealed_end {
            return Err(at("INTERRUPTED record in a sealed `end` file"));
        }
        if r["d0"].as_str() != Some(d0.as_str()) {
            return Err(at(&format!("d0 is not the job's --probe-d {d0}")));
        }
        if !r["relation_digest"].is_null() || !r["rhs"].is_null() {
            return Err(at(
                "a probe record with relation members (never a relation)",
            ));
        }
        let rd = r["recipe"]["digest"].as_str().filter(|d| is_hex64(d));
        if rd.is_none() {
            return Err(at("recipe.digest is not 64 hex"));
        }
        let recipe_digest =
            same_or_set(&mut t.recipe_digest, rd, "recipe.digest").map_err(|e| at(&e))?;
        let det = r["determinism_id"]
            .as_str()
            .filter(|d| d.starts_with(PROBE_DETERMINISM_PREFIX));
        if det.is_none() {
            return Err(at("determinism_id is not a probe determinism id"));
        }
        let determinism_id =
            same_or_set(&mut t.determinism_id, det, "determinism_id").map_err(|e| at(&e))?;
        let per = r["per_prime"]
            .as_array()
            .ok_or_else(|| at("per_prime is not a list"))?;
        let full = verdict.is_answer() || verdict == ProbeVerdict::UnresolvedPrimes;
        if per.len() > args.primes.len() || (full && per.len() != args.primes.len()) {
            return Err(at("per_prime does not list the job's primes"));
        }
        let mut seen: Vec<ProbeVerdict> = Vec::with_capacity(per.len());
        for (e, want_p) in per.iter().zip(&args.primes) {
            if e["prime"].as_u64() != Some(*want_p) {
                return Err(at("per_prime primes are not the job's primes in order"));
            }
            let pv = e["verdict"]
                .as_str()
                .and_then(ProbeVerdict::from_name)
                .filter(|v| v.is_answer())
                .ok_or_else(|| at("a per-prime verdict that is not an answer"))?;
            for m in ["rank", "rows", "columns", "trace_rows"] {
                if !e[m].is_null() && e[m].as_u64().is_none() {
                    return Err(at(&format!("per_prime.{m} is not an unsigned integer")));
                }
            }
            seen.push(pv);
        }
        if full {
            let agree = seen.windows(2).all(|w| w[0] == w[1]);
            let ok = match verdict {
                ProbeVerdict::UnresolvedPrimes => !agree,
                v => agree && seen.first() == Some(&v),
            };
            if !ok {
                return Err(at(
                    "the verdict is not the combination of the per-prime verdicts",
                ));
            }
        }
        *counts.entry(verdict.name()).or_default() += 1;
        *t.probes.entry(verdict).or_default() += 1;
        t.records += 1;
        out.push(ProbeOutcome {
            ordinal: i as u64,
            key,
            tag: want.tag.clone(),
            verdict,
            route: verdict.route(),
            recipe_digest,
            determinism_id,
            canonical: line.clone(),
        });
    }
    check_end(
        &end,
        recs.len(),
        &counts,
        [
            ("recipe_digest", &t.recipe_digest),
            ("determinism_id", &t.determinism_id),
        ],
        IDS_WHEN_PRESENT,
    )?;
    Ok((out, t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdict_names_and_classes() {
        for v in Verdict::ALL {
            assert_eq!(Verdict::from_name(&v.name()), Some(v));
            if let Some(c) = v.class() {
                assert_eq!(c.name(), v.name().to_lowercase());
            }
        }
        assert_eq!(Verdict::Solved.class(), None);
        assert_eq!(FailureClass::NotInDomain.route(), KeyRoute::NextRecipeRung);
        assert_eq!(FailureClass::BudgetWall.route(), KeyRoute::LadderDecides);
        assert_eq!(FailureClass::Irregular.route(), KeyRoute::SubCase);
        assert_eq!(FailureClass::DigestMismatch.route(), KeyRoute::Attention);
    }

    #[test]
    fn key_texts_are_generic_in_arity() {
        assert_eq!(parse_key_text("I(1,-2,0)", 3), Some(vec![1, -2, 0]));
        assert_eq!(parse_key_text("I(1,-2,0)", 4), None);
        assert_eq!(parse_key_text("J(1)", 1), None);
    }

    #[test]
    fn keys_input_forms() {
        let k = parse_keys_input(b"[1,2]\n\n{\"key\":[3,4],\"tag\":\"t\",\"x\":1}\n").unwrap();
        assert_eq!(k.len(), 2);
        assert_eq!(k[1].tag.as_deref(), Some("t"));
        assert!(parse_keys_input(b"[1,2]\n[1]\n").is_err());
    }
}
