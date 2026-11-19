//! `rr_campaign rule-cost` (lane rulecost; WS/plan/auto-campaign/interfaces/rule-cost.md 3): measure the APPLICATION
//! COST of stored rules of one owner through the engine probe `p_rule_cost` (a child process, the production Custom step
//! under a clock), aggregate per rule and join a run profile. The pure parts live in `rustred_campaign_core::rulecost`.
//!
//! ```text
//! rr_campaign rule-cost --spec SPEC.json --dir DIR [--aggregate-only]
//! ```
//! DIR receives `rules.jsonl` (the probe's rule shapes), `keys.jsonl` (the keys given or sampled), `probe-keys.jsonl`
//! (the probe's measurements), `report.json` and `report.md`. `--aggregate-only` re-reads the existing probe outputs
//! (e.g. to join another profile) and runs no child.

use rustred_campaign_core::economy::improve::TableRule;
use rustred_campaign_core::rulecost::{
    self as rc, KeysHeader, ProbeKey, ProbeRule, RuleCost, RuleId, RulesHeader, RunRule,
    SamplerSpec,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// A file bound by SHA-256.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bound {
    pub path: PathBuf,
    pub sha256: String,
}

impl Bound {
    fn check(&self, what: &str) -> Result<(), String> {
        let b = std::fs::read(&self.path)
            .map_err(|e| format!("{what} {}: {e}", self.path.display()))?;
        let h = sha256_hex(&b);
        if h != self.sha256 {
            return Err(format!(
                "{what} {}: sha256 {h} is not the spec's {}",
                self.path.display(),
                self.sha256
            ));
        }
        Ok(())
    }
}

fn sha256_hex(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

/// The spec (schema [`rc::SPEC_SCHEMA`]).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleCostSpec {
    pub schema: String,
    /// The engine probe binary (`p_rule_cost` of a rustred build), bound by SHA-256.
    pub probe: Bound,
    /// Package directory and owner class (label or hex id).
    pub package: PathBuf,
    pub owner: String,
    /// Rules to sample keys for (`"<chart hex>:<position>"`); with explicit `keys`, only a filter of the report
    /// (empty = every rule the keys hit).
    #[serde(default)]
    pub rules: Vec<String>,
    /// Explicit keys (owner presentation slots; `{"key": [..]}` or `n1,n2,..` lines); absent = the sampler.
    #[serde(default)]
    pub keys: Option<PathBuf>,
    #[serde(default)]
    pub sampler: SamplerSpec,
    /// At most this many keys go to the probe (sampled keys are taken round-robin over the rules).
    #[serde(default = "default_max_keys")]
    pub max_keys: usize,
    #[serde(default = "default_repeat")]
    pub repeat: usize,
    /// fwd-profile level of the extra profiled step (0 = none; needs a probe built with the feature).
    #[serde(default = "default_profile_level")]
    pub profile_level: u8,
    /// Owner statistics of a run (`RUSTRED_P_OWNER_STATS`): calls and seconds per rule.
    #[serde(default)]
    pub profile: Option<PathBuf>,
    /// A table of the owner (table_dump.v1) for bytes / monomials per rule, and its slot map (table slot i =
    /// presentation slot `table_to_presentation[i]`; identity when empty).
    #[serde(default)]
    pub table: Option<PathBuf>,
    #[serde(default)]
    pub table_to_presentation: Vec<usize>,
    /// Extra environment of the probe child (e.g. a private TMPDIR).
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

fn default_max_keys() -> usize {
    256
}
fn default_repeat() -> usize {
    3
}
fn default_profile_level() -> u8 {
    2
}

fn arg(args: &[String], k: &str) -> Option<String> {
    args.iter()
        .position(|a| a == k)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn rule_id(text: &str) -> Result<RuleId, String> {
    let (c, p) = text
        .split_once(':')
        .ok_or_else(|| format!("rule {text:?}: not <chart hex>:<position>"))?;
    Ok(RuleId {
        chart: rc::hex(c)?,
        position: p
            .trim()
            .parse()
            .map_err(|_| format!("rule {text:?}: position"))?,
    })
}

/// Run the probe with `args`, stdout and stderr into `<dir>/<stem>.out` / `.err`; an error names the exit status
/// and the last stderr line.
fn run_probe(spec: &RuleCostSpec, dir: &Path, stem: &str, args: &[String]) -> Result<(), String> {
    let out = std::fs::File::create(dir.join(format!("{stem}.out"))).map_err(|e| e.to_string())?;
    let err_path = dir.join(format!("{stem}.err"));
    let err = std::fs::File::create(&err_path).map_err(|e| e.to_string())?;
    let status = Command::new(&spec.probe.path)
        .args(args)
        .envs(&spec.env)
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(err)
        .status()
        .map_err(|e| format!("probe {}: {e}", spec.probe.path.display()))?;
    if !status.success() {
        let tail = std::fs::read_to_string(&err_path).unwrap_or_default();
        let last = tail.lines().last().unwrap_or("").to_string();
        return Err(format!("probe {stem}: {status}: {last}"));
    }
    Ok(())
}

/// A sampled key, the rule it was drawn for and its shell.
pub type SampledKey = (Vec<i64>, RuleId, [u32; 2]);

/// Keys of the sampler for every listed rule (round-robin over the rules up to `max_keys`), each with the rule and
/// shell it was drawn for.
pub fn sample_keys(
    spec: &RuleCostSpec,
    header: &RulesHeader,
    shapes: &[ProbeRule],
    targets: &[RuleId],
) -> Result<Vec<SampledKey>, String> {
    let support = rc::hex(&header.support)?;
    let mut per_rule: Vec<std::vec::IntoIter<(Vec<i64>, [u32; 2])>> = Vec::new();
    let mut ids = Vec::new();
    for t in targets {
        let shape = shapes
            .iter()
            .find(|s| s.id().ok() == Some(*t))
            .ok_or_else(|| format!("rule {t}: not a rule of the owner"))?;
        let salt = (t.chart.rotate_left(13)) ^ t.position as u64;
        per_rule.push(
            rc::face_keys(header.arity, support, &shape.fixed, &spec.sampler, salt).into_iter(),
        );
        ids.push(*t);
    }
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    loop {
        let mut any = false;
        for (it, id) in per_rule.iter_mut().zip(&ids) {
            if out.len() >= spec.max_keys {
                return Ok(out);
            }
            if let Some((k, shell)) = it.next() {
                any = true;
                if seen.insert(k.clone()) {
                    out.push((k, *id, shell));
                }
            }
        }
        if !any {
            return Ok(out);
        }
    }
}

/// Bytes and monomials per rule of a table, keyed by the probe's rule id (charts mapped to the presentation frame,
/// positions = the native importer's ORDINARY payload positions: slot-alias records have none; audit 2026-11-19 a1), and
/// the table's source <-> payload map.
fn table_sizes(
    path: &Path,
    t2p: &[usize],
) -> Result<(BTreeMap<RuleId, TableRule>, rc::PayloadMap), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let map = rc::PayloadMap::from_lines(text.lines())?;
    let (_, rules) = rustred_campaign_core::economy::improve::table_rules(text.lines())?;
    let mut out = BTreeMap::new();
    for r in rules {
        let Some(&position) = map.by_source.get(&(r.chart.clone(), r.index)) else {
            continue; // a slot alias: not a payload rule
        };
        let chart = (r.chart.chars().enumerate())
            .filter(|(_, c)| *c == '1')
            .fold(0u64, |m, (i, _)| {
                let p = t2p.get(i).copied().unwrap_or(i);
                if p < 64 { m | 1 << p } else { m }
            });
        out.insert(RuleId { chart, position }, r);
    }
    Ok((out, map))
}

/// One report row.
#[derive(Clone, Debug, Serialize)]
pub struct Row {
    pub rule: String,
    pub ordinal: usize,
    pub table_bytes: Option<u64>,
    pub table_monomials: Option<u64>,
    pub rhs_terms: usize,
    pub coefficient_monomials: u64,
    pub max_degree_d: u32,
    pub max_total_degree_indices: u32,
    pub max_bits: u64,
    pub distinct_original_denominators: usize,
    pub exception_branches: usize,
    /// TRICKS 72: distinct coefficients (by value), rhs terms per distinct coefficient, their summed monomials, the
    /// largest coefficient's monomials and the distinct coefficients' text bytes (when the probe gives them).
    pub distinct_coefficients: usize,
    pub sharing: f64,
    pub distinct_coefficient_monomials: Option<u64>,
    pub max_coefficient_monomials: Option<u64>,
    pub distinct_text_bytes: Option<u64>,
    pub max_text_bytes: Option<u64>,
    /// Monomial visits of one application today, and with a per-application cache of distinct coefficients
    /// ([`rc::visits_per_term`], [`rc::visits_distinct`]); measured ns per visit; the cached time this predicts.
    pub visits_per_term: u64,
    pub visits_distinct: Option<u64>,
    pub ns_per_visit: Option<f64>,
    pub predicted_cached_seconds: Option<f64>,
    /// Children per application over distinct coefficients.
    pub children_per_distinct: Option<f64>,
    pub cost: Option<RuleCost>,
    pub run: Option<RunRule>,
    /// calls x median cost per application (seconds): the run time the measurement predicts for this rule.
    pub predicted_run_seconds: Option<f64>,
}

/// The rows (one per shape that is targeted, measured or profiled), in report order: targets first, then by
/// predicted (else run) seconds.
pub fn rows(
    shapes: &[ProbeRule],
    costs: &BTreeMap<RuleId, RuleCost>,
    profile: &BTreeMap<RuleId, RunRule>,
    table: &BTreeMap<RuleId, TableRule>,
    targets: &[RuleId],
) -> Vec<Row> {
    let mut out = Vec::new();
    for s in shapes {
        let Ok(id) = s.id() else { continue };
        let cost = costs.get(&id).cloned();
        if cost.is_none() && !targets.contains(&id) {
            continue;
        }
        let run = profile.get(&id).copied();
        // a table record joins only when its rhs count is the rule's (an index is a position only when the payload
        // was imported from exactly that table under the native order rule; a payload may hold rules no record names)
        let trec = table.get(&id).filter(|t| {
            t.cost
                .as_ref()
                .is_some_and(|c| c.rhs_terms == s.rhs_terms as u64)
        });
        let visits = rc::visits_per_term(s);
        let cached = rc::visits_distinct(s);
        let ns_per_visit = cost
            .as_ref()
            .filter(|_| visits > 0)
            .map(|c| c.seconds_median * 1e9 / visits as f64);
        let distinct = s.distinct_coefficients.max(1);
        let predicted = match (&cost, run) {
            (Some(c), Some(r)) => Some(c.seconds_median * r.calls as f64),
            _ => None,
        };
        out.push(Row {
            rule: id.to_string(),
            ordinal: s.ordinal,
            table_bytes: trec.map(|t| t.bytes),
            table_monomials: trec.map(|t| t.monomials),
            rhs_terms: s.rhs_terms,
            coefficient_monomials: s.coefficient_monomials(),
            max_degree_d: s
                .numerator
                .max_degree_base
                .max(s.denominator.max_degree_base),
            max_total_degree_indices: s
                .numerator
                .max_total_degree_indices
                .max(s.denominator.max_total_degree_indices),
            max_bits: s.numerator.max_bits.max(s.denominator.max_bits),
            distinct_original_denominators: s.distinct_original_denominators,
            exception_branches: s.exception_branches,
            distinct_coefficients: s.distinct_coefficients,
            sharing: s.rhs_terms as f64 / distinct as f64,
            distinct_coefficient_monomials: s
                .coefficients
                .as_ref()
                .map(|c| c.numerator.terms + c.denominator.terms),
            max_coefficient_monomials: s.coefficients.as_ref().map(|c| c.max_terms),
            distinct_text_bytes: s.coefficients.as_ref().and_then(|c| c.text_bytes),
            max_text_bytes: s.coefficients.as_ref().and_then(|c| c.max_text_bytes),
            visits_per_term: visits,
            visits_distinct: cached,
            ns_per_visit,
            predicted_cached_seconds: match (&cost, cached) {
                (Some(c), Some(v)) if visits > 0 => {
                    Some(c.seconds_median * v as f64 / visits as f64)
                }
                _ => None,
            },
            children_per_distinct: cost.as_ref().map(|c| c.children_median / distinct as f64),
            cost,
            run,
            predicted_run_seconds: predicted,
        });
    }
    let pos = |r: &Row| {
        targets
            .iter()
            .position(|t| t.to_string() == r.rule)
            .unwrap_or(usize::MAX)
    };
    out.sort_by(|a, b| {
        pos(a).cmp(&pos(b)).then(
            b.predicted_run_seconds
                .or(b.run.map(|r| r.seconds))
                .unwrap_or(0.0)
                .total_cmp(
                    &a.predicted_run_seconds
                        .or(a.run.map(|r| r.seconds))
                        .unwrap_or(0.0),
                ),
        )
    });
    out
}

/// The markdown table of `rows`.
pub fn markdown(rows: &[Row]) -> String {
    let mut s = String::from(
        "| rule (chart:pos) | bytes | rhs terms | coef. monomials | distinct coef. (terms per coef.) | distinct coef. monomials | deg d / deg n | distinct den. | applications | s per application (median / p90) | ns per monomial visit | cached s (pred.) | children (median) | children per distinct coef. | same-owner children | children per receiver (per application) | calls in run | run s | calls x median s |\n|---|---:|---:|---:|---|---:|---|---:|---:|---|---:|---:|---:|---:|---:|---|---:|---:|---:|\n",
    );
    for r in rows {
        let (apps, sec, ch, same, recv) = match &r.cost {
            Some(c) => (
                c.applications.to_string(),
                format!("{:.4} / {:.4}", c.seconds_median, c.seconds_p90),
                format!("{:.0}", c.children_median),
                format!("{:.1}", c.same_owner_mean),
                c.receivers
                    .iter()
                    .filter(|x| x.per_application >= 0.05)
                    .map(|x| format!("{} {}L {:.1}", x.lane, x.lines, x.per_application))
                    .collect::<Vec<_>>()
                    .join("; "),
            ),
            None => ("0".into(), "-".into(), "-".into(), "-".into(), "-".into()),
        };
        s.push_str(&format!(
            "| {} | {} | {} | {} | {} ({:.2}) | {} | {} / {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            r.rule,
            r.table_bytes.map_or("-".into(), |b| b.to_string()),
            r.rhs_terms,
            r.coefficient_monomials,
            r.distinct_coefficients,
            r.sharing,
            r.distinct_coefficient_monomials
                .map_or("-".into(), |m| m.to_string()),
            r.max_degree_d,
            r.max_total_degree_indices,
            r.distinct_original_denominators,
            apps,
            sec,
            r.ns_per_visit.map_or("-".into(), |x| format!("{x:.0}")),
            r.predicted_cached_seconds
                .map_or("-".into(), |x| format!("{x:.4}")),
            ch,
            r.children_per_distinct
                .map_or("-".into(), |x| format!("{x:.2}")),
            same,
            recv,
            r.run.map_or("-".into(), |x| x.calls.to_string()),
            r.run.map_or("-".into(), |x| format!("{:.1}", x.seconds)),
            r.predicted_run_seconds
                .map_or("-".into(), |x| format!("{x:.1}")),
        ));
    }
    s
}

/// `rr_campaign rule-cost --spec S.json --dir D [--aggregate-only]`; returns a one-line summary.
pub fn rule_cost_cmd(args: &[String]) -> Result<String, String> {
    let (Some(sp), Some(dir)) = (arg(args, "--spec"), arg(args, "--dir")) else {
        return Err("usage: rule-cost --spec S.json --dir D [--aggregate-only]".into());
    };
    let aggregate_only = args.iter().any(|a| a == "--aggregate-only");
    let sb = std::fs::read(&sp).map_err(|e| format!("{sp}: {e}"))?;
    let spec: RuleCostSpec = serde_json::from_slice(&sb).map_err(|e| format!("spec: {e}"))?;
    if spec.schema != rc::SPEC_SCHEMA {
        return Err(format!(
            "spec schema {} is not {}",
            spec.schema,
            rc::SPEC_SCHEMA
        ));
    }
    spec.probe.check("probe")?;
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let package = spec.package.display().to_string();
    let targets: Vec<RuleId> = spec
        .rules
        .iter()
        .map(|r| rule_id(r))
        .collect::<Result<_, _>>()?;
    // 1. rule shapes
    let rules_path = dir.join("rules.jsonl");
    if !aggregate_only {
        run_probe(
            &spec,
            &dir,
            "probe-rules",
            &[
                "rules".into(),
                package.clone(),
                spec.owner.clone(),
                rules_path.display().to_string(),
            ],
        )?;
    }
    let text = std::fs::read_to_string(&rules_path)
        .map_err(|e| format!("{}: {e}", rules_path.display()))?;
    let (header, shapes): (RulesHeader, Vec<ProbeRule>) =
        rc::parse_probe(&text, rc::PROBE_RULES_SCHEMA)?;
    let support = rc::hex(&header.support)?;
    // 2. keys
    let keys_path = dir.join("keys.jsonl");
    if !aggregate_only {
        let lines: Vec<String> = match &spec.keys {
            Some(p) => std::fs::read_to_string(p)
                .map_err(|e| format!("{}: {e}", p.display()))?
                .lines()
                .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
                .take(spec.max_keys)
                .map(str::to_string)
                .collect(),
            None => sample_keys(&spec, &header, &shapes, &targets)?
                .into_iter()
                .map(|(k, r, s)| json!({"key": k, "rule": r.to_string(), "shell": s}).to_string())
                .collect(),
        };
        if lines.is_empty() {
            return Err("no keys (give keys, or rules and sampler shells the faces reach)".into());
        }
        std::fs::write(&keys_path, lines.join("\n") + "\n").map_err(|e| e.to_string())?;
        // 3. measure
        let a = vec![
            "keys".into(),
            package,
            spec.owner.clone(),
            keys_path.display().to_string(),
            dir.join("probe-keys.jsonl").display().to_string(),
            "--repeat".into(),
            spec.repeat.max(1).to_string(),
            "--profile".into(),
            spec.profile_level.to_string(),
        ];
        run_probe(&spec, &dir, "probe-keys", &a)?;
    }
    let kp = dir.join("probe-keys.jsonl");
    let text = std::fs::read_to_string(&kp).map_err(|e| format!("{}: {e}", kp.display()))?;
    let (kheader, keys): (KeysHeader, Vec<ProbeKey>) =
        rc::parse_probe(&text, rc::PROBE_KEYS_SCHEMA)?;
    // 4. aggregate, join
    let mut costs = rc::aggregate(&keys, support);
    if spec.keys.is_some() && !targets.is_empty() {
        costs.retain(|id, _| targets.contains(id));
    }
    let profile = match &spec.profile {
        Some(p) => {
            let b = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
            let v: Value =
                serde_json::from_slice(&b).map_err(|e| format!("{}: {e}", p.display()))?;
            rc::profile_rules(&v, &spec.owner)?
        }
        None => BTreeMap::new(),
    };
    let (table, payload_map) = match &spec.table {
        Some(p) => {
            let (t, m) = table_sizes(p, &spec.table_to_presentation)?;
            (t, Some(m))
        }
        None => (BTreeMap::new(), None),
    };
    let rows = rows(&shapes, &costs, &profile, &table, &targets);
    let errors = keys.iter().filter(|k| k.error.is_some()).count();
    let off_target = keys
        .iter()
        .filter(|k| {
            k.rule()
                .is_some_and(|r| !targets.is_empty() && !targets.contains(&r))
        })
        .count();
    // audit 2026-11-19 (a1): the profile's PAYLOAD positions are bound to the table's RECORD indices through the
    // importer's ordinary-rule order; without a table nothing is bound (no model), unmapped positions are listed
    let bound = rc::calls_from_profile(
        &profile,
        &spec.table_to_presentation,
        header.arity,
        payload_map.as_ref(),
    );
    let report = json!({
        "schema": rc::REPORT_SCHEMA, "spec_sha256": sha256_hex(&sb), "probe_sha256": spec.probe.sha256,
        "package": spec.package, "owner": spec.owner, "rules_header": header, "keys_header": kheader,
        "keys": keys.len(), "key_errors": errors, "keys_on_other_rules": off_target,
        "profile": spec.profile, "rows": rows,
        "profile_binding": if payload_map.is_some() { "payload positions -> table record indices (native importer order, slot aliases apart)" } else { "no table: profile calls not bound, no model written" },
        "profile_unmapped": bound.unmapped,
    });
    let rb = serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("report.json"), rb).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("report.md"), markdown(&rows)).map_err(|e| e.to_string())?;
    // a ready `cost.rule_cost` for improve (interfaces/rule-cost.md 4): the measured default weights, the run's calls
    // per rule in the table's chart form (`table_to_presentation`), uniform calls for the rest
    if !profile.is_empty() && payload_map.is_some() {
        let model = rc::RuleCostModel {
            calls: bound.calls.clone(),
            ..rc::RuleCostModel::default()
        };
        let mb = serde_json::to_vec_pretty(&model).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("rule_cost_model.json"), mb).map_err(|e| e.to_string())?;
    }
    Ok(format!(
        "rule-cost: {} keys ({} errors, {} on other rules), {} rules measured -> {}",
        keys.len(),
        errors,
        off_target,
        costs.len(),
        dir.join("report.json").display()
    ))
}

/// `rr_campaign rule-score --table T.jsonl [--model M.json] [--include I,J,..] [--out O.jsonl]
/// [--keys K.jsonl [--table-to-presentation P0,P1,..] [--one-level]]`: the `rule_cost`
/// features and scores of table records (interfaces/rule-cost.md 4), one JSON line per rule: today's predicted ns per
/// application (per-term specialisation, the measured defaults or `M`), the ESTIMATE for an engine with a
/// per-application coefficient cache ([`rc::RuleCostModel::cached`]), calls and score. With `--keys` (lines
/// `{"key": [..]}` in the OWNER PRESENTATION's slots, mapped to the table's slots by `--table-to-presentation`, identity
/// when absent): each key goes to the FIRST rule of the table (file order) that serves it ([`improve::serves_key`]:
/// face, equations, exception branches), and each rule line counts `keys_served`; the summary gives the mean predicted
/// ns per served key, today and cached. `--one-level` also prices the SAME-SECTOR children of each key's rule at that
/// key (the keys a step hands back to the same owner: NEW keys unless already pending) at the predicted cost of the
/// rule that first-matches each child (ONE level: the children's own children are not followed; merges are ignored);
/// a second pass over the table. Reads the table line by line; runs no child. Returns a one-line summary.
pub fn rule_score_cmd(args: &[String]) -> Result<String, String> {
    let Some(table) = arg(args, "--table") else {
        return Err(
            "usage: rule-score --table T.jsonl [--model M.json] [--include I,J,..] [--out O.jsonl]"
                .into(),
        );
    };
    let model: rc::RuleCostModel = match arg(args, "--model") {
        Some(p) => {
            let b = std::fs::read(&p).map_err(|e| format!("{p}: {e}"))?;
            serde_json::from_slice(&b).map_err(|e| format!("model {p}: {e}"))?
        }
        None => rc::RuleCostModel::default(),
    };
    let cached = model.cached();
    let include: Option<std::collections::BTreeSet<i64>> = match arg(args, "--include") {
        Some(list) => Some(
            list.split(',')
                .map(|x| x.trim().parse().map_err(|_| format!("--include {x}")))
                .collect::<Result<_, String>>()?,
        ),
        None => None,
    };
    let t2p: Vec<usize> = match arg(args, "--table-to-presentation") {
        Some(list) => list
            .split(',')
            .map(|x| {
                x.trim()
                    .parse()
                    .map_err(|_| format!("--table-to-presentation {x}"))
            })
            .collect::<Result<_, String>>()?,
        None => vec![],
    };
    let keys: Vec<Vec<i32>> = match arg(args, "--keys") {
        Some(p) => {
            let text = std::fs::read_to_string(&p).map_err(|e| format!("{p}: {e}"))?;
            let mut keys = Vec::new();
            for l in text.lines().filter(|l| !l.trim().is_empty()) {
                let v: Value = serde_json::from_str(l).map_err(|e| format!("{p}: {e}"))?;
                let pres: Vec<i32> = v
                    .get("key")
                    .and_then(Value::as_array)
                    .ok_or_else(|| format!("{p}: a line without key"))?
                    .iter()
                    .map(|x| {
                        x.as_i64()
                            .map(|n| n as i32)
                            .ok_or("a key entry is not an integer")
                    })
                    .collect::<Result<_, _>>()?;
                let key: Vec<i32> = (0..pres.len())
                    .map(|i| {
                        pres.get(t2p.get(i).copied().unwrap_or(i))
                            .copied()
                            .unwrap_or(0)
                    })
                    .collect();
                keys.push(key);
            }
            keys
        }
        None => vec![],
    };
    let levels: usize = match arg(args, "--levels") {
        Some(v) => v.parse().map_err(|_| format!("--levels {v}"))?,
        None => 0,
    };
    let one_level = levels > 0 || args.iter().any(|a| a == "--one-level");
    if one_level && keys.is_empty() {
        return Err("--one-level / --levels need --keys".into());
    }
    // per key: its rule's row and predicted ns (today, cached); the same-sector children of that rule at the key
    let mut key_row: Vec<Option<usize>> = vec![None; keys.len()];
    let mut key_own: Vec<(f64, f64)> = vec![(0.0, 0.0); keys.len()];
    let mut children: Vec<(usize, Vec<i32>)> = Vec::new();
    let mut open: Vec<usize> = (0..keys.len()).collect();
    let (mut served_today, mut served_cached, mut served) = (0f64, 0f64, 0usize);
    let f = std::fs::File::open(&table).map_err(|e| format!("{table}: {e}"))?;
    let mut out_lines = Vec::new();
    let (mut n, mut today_sum, mut cached_sum) = (0u64, 0f64, 0f64);
    for line in std::io::BufRead::lines(std::io::BufReader::new(f)) {
        let line = line.map_err(|e| format!("{table}: {e}"))?;
        let Some(r) = TableRule::from_line(&line)? else {
            continue;
        };
        let mut keys_served = 0usize;
        let mut served_here = Vec::new();
        if !open.is_empty() {
            let v: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
            let mut still = Vec::with_capacity(open.len());
            for k in open {
                if rustred_campaign_core::economy::improve::serves_key(&v, &keys[k])? {
                    keys_served += 1;
                    served_here.push(k);
                    if one_level {
                        for c in rc::same_sector_children(&v, &keys[k]).unwrap_or_default() {
                            children.push((k, c));
                        }
                    }
                } else {
                    still.push(k);
                }
            }
            open = still;
        }
        if include.as_ref().is_some_and(|s| !s.contains(&r.index)) {
            continue;
        }
        let fe = r.cost.clone().unwrap_or_default();
        let today = model.per_application_ns(r.monomials, r.cost.as_ref());
        let cache = cached.per_application_ns(r.monomials, r.cost.as_ref());
        let calls = model.calls_of(&r.chart, r.index);
        for &k in &served_here {
            key_row[k] = Some(out_lines.len());
            key_own[k] = (today, cache);
        }
        served += keys_served;
        served_today += keys_served as f64 * today;
        served_cached += keys_served as f64 * cache;
        n += 1;
        today_sum += calls * today;
        cached_sum += calls * cache;
        out_lines.push(
            json!({"index": r.index, "chart": r.chart, "target": r.target, "case": r.case, "bytes": r.bytes,
                "rhs_terms": fe.rhs_terms, "children_same_sector": fe.children_same_sector,
                "children_lower": fe.children_lower, "unread_terms": fe.unread_terms,
                "coefficient_monomials": fe.coefficient_monomials, "denominator_monomials": fe.denominator_monomials,
                "distinct_coefficients": fe.distinct_coefficients,
                "sharing": fe.rhs_terms as f64 / fe.distinct_coefficients.max(1) as f64,
                "distinct_coefficient_monomials": fe.distinct_coefficient_monomials,
                "distinct_denominators": fe.distinct_denominators,
                "distinct_denominator_monomials": fe.distinct_denominator_monomials,
                "max_coefficient_bytes": fe.max_coefficient_bytes, "distinct_coefficient_bytes": fe.distinct_coefficient_bytes,
                "today_ns": today, "cached_ns_estimate": cache, "calls": calls, "keys_served": keys_served,
                "score_today": calls * today, "score_cached_estimate": calls * cache})
            .to_string(),
        );
    }
    // pass 2 (--one-level): each same-sector child to its first serving rule, priced at that rule's prediction
    let mut child_cost: Vec<(f64, f64, usize)> = vec![(0.0, 0.0, 0); keys.len()];
    let mut unserved_children = 0usize;
    if one_level && !children.is_empty() {
        let mut open: Vec<usize> = (0..children.len()).collect();
        let f = std::fs::File::open(&table).map_err(|e| format!("{table}: {e}"))?;
        for line in std::io::BufRead::lines(std::io::BufReader::new(f)) {
            if open.is_empty() {
                break;
            }
            let line = line.map_err(|e| format!("{table}: {e}"))?;
            let Some(r) = TableRule::from_line(&line)? else {
                continue;
            };
            let v: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
            let (today, cache) = (
                model.per_application_ns(r.monomials, r.cost.as_ref()),
                cached.per_application_ns(r.monomials, r.cost.as_ref()),
            );
            let mut still = Vec::with_capacity(open.len());
            for c in open {
                let (parent, child) = &children[c];
                let child_frame: Vec<i32> = child.clone();
                if rustred_campaign_core::economy::improve::serves_key(&v, &child_frame)? {
                    let e = &mut child_cost[*parent];
                    e.0 += today;
                    e.1 += cache;
                    e.2 += 1;
                } else {
                    still.push(c);
                }
            }
            open = still;
        }
        unserved_children = open.len();
    }
    if one_level {
        // per row: mean one-level ns over the keys it served
        let mut per_row: BTreeMap<usize, (f64, f64, f64, usize)> = BTreeMap::new();
        for k in 0..keys.len() {
            if let Some(row) = key_row[k] {
                let e = per_row.entry(row).or_default();
                e.0 += key_own[k].0 + child_cost[k].0;
                e.1 += key_own[k].1 + child_cost[k].1;
                e.2 += child_cost[k].2 as f64;
                e.3 += 1;
            }
        }
        for (row, (t, c, ch, n)) in per_row {
            let mut v: Value = serde_json::from_str(&out_lines[row]).map_err(|e| e.to_string())?;
            v["one_level_today_ns_per_key"] = json!(t / n as f64);
            v["one_level_cached_ns_estimate_per_key"] = json!(c / n as f64);
            v["same_sector_children_per_key"] = json!(ch / n as f64);
            out_lines[row] = v.to_string();
        }
    }
    // --levels N: the same-sector sub-tree N levels deep, each DISTINCT key stepped once (a key met again is a merge into
    // a pending key, not a new step), each step at its first-matching rule's prediction
    let mut level_lines = Vec::new();
    if levels > 0 {
        let mut seen: std::collections::BTreeSet<Vec<i32>> = keys.iter().cloned().collect();
        let mut frontier: Vec<Vec<i32>> = Vec::new();
        for (_, c) in &children {
            if seen.insert(c.clone()) {
                frontier.push(c.clone());
            }
        }
        let merged0 = children.len() - frontier.len();
        let (mut total_today, mut total_cached) = (served_today, served_cached);
        for level in 1..=levels {
            if frontier.is_empty() {
                break;
            }
            let mut open: Vec<usize> = (0..frontier.len()).collect();
            let mut next: Vec<Vec<i32>> = Vec::new();
            let (mut lt, mut lc, mut stepped, mut produced) = (0f64, 0f64, 0usize, 0usize);
            let f = std::fs::File::open(&table).map_err(|e| format!("{table}: {e}"))?;
            for line in std::io::BufRead::lines(std::io::BufReader::new(f)) {
                if open.is_empty() {
                    break;
                }
                let line = line.map_err(|e| format!("{table}: {e}"))?;
                let Some(r) = TableRule::from_line(&line)? else {
                    continue;
                };
                let v: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
                let mut still = Vec::with_capacity(open.len());
                let mut here = Vec::new();
                for k in open {
                    if rustred_campaign_core::economy::improve::serves_key(&v, &frontier[k])? {
                        here.push(k);
                    } else {
                        still.push(k);
                    }
                }
                open = still;
                if here.is_empty() {
                    continue;
                }
                let (today, cache) = (
                    model.per_application_ns(r.monomials, r.cost.as_ref()),
                    cached.per_application_ns(r.monomials, r.cost.as_ref()),
                );
                for k in here {
                    stepped += 1;
                    lt += today;
                    lc += cache;
                    if level < levels {
                        for c in rc::same_sector_children(&v, &frontier[k]).unwrap_or_default() {
                            produced += 1;
                            if seen.insert(c.clone()) {
                                next.push(c);
                            }
                        }
                    }
                }
            }
            total_today += lt;
            total_cached += lc;
            level_lines.push(format!(
                "level {level}: {stepped} distinct keys stepped ({} with no rule), predicted {:.3} s today / {:.3} s cached; {produced} same-sector children produced, {} new",
                open.len(),
                lt * 1e-9,
                lc * 1e-9,
                next.len()
            ));
            frontier = next;
        }
        level_lines.insert(
            0,
            format!(
                "LEVELS {levels}: {} keys at level 0 ({merged0} level-1 children merged into keys already met); total predicted {:.3} s today, {:.3} s cached",
                keys.len(),
                total_today * 1e-9,
                total_cached * 1e-9
            ),
        );
    }
    let text = out_lines.join("\n") + if out_lines.is_empty() { "" } else { "\n" };
    match arg(args, "--out") {
        Some(o) => std::fs::write(&o, &text).map_err(|e| format!("{o}: {e}"))?,
        None => print!("{text}"),
    }
    let mut per_key = if keys.is_empty() {
        String::new()
    } else {
        format!(
            "; keys {} served {served} unserved {}: mean predicted per served key today {:.4} s, cached estimate {:.4} s",
            keys.len(),
            open.len(),
            served_today * 1e-9 / served.max(1) as f64,
            served_cached * 1e-9 / served.max(1) as f64
        )
    };
    if one_level {
        let (t, c, n): (f64, f64, usize) = child_cost
            .iter()
            .fold((0.0, 0.0, 0), |a, x| (a.0 + x.0, a.1 + x.1, a.2 + x.2));
        let s = served.max(1) as f64;
        per_key.push_str(&format!(
            "; ONE LEVEL (own + same-sector children at their first-matching rule): today {:.4} s, cached estimate {:.4} s per served key, {:.1} same-sector children per key ({unserved_children} children with no rule)",
            (served_today + t) * 1e-9 / s,
            (served_cached + c) * 1e-9 / s,
            n as f64 / s
        ));
    }
    if !level_lines.is_empty() {
        per_key.push('\n');
        per_key.push_str(&level_lines.join("\n"));
    }
    Ok(format!(
        "rule-score: {n} rules, score today {:.3} s, cached estimate {:.3} s (calls x ns per application){per_key}",
        today_sum * 1e-9,
        cached_sum * 1e-9
    ))
}
