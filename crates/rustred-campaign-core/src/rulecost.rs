//! APPLICATION COST as a measured, first-class quality of a rule (lane rulecost; WS/plan/auto-campaign/interfaces/
//! rule-cost.md). PURE: no process, no file, no clock. It holds
//!
//! * the records of the engine probe `p_rule_cost` (rules: [`ProbeRule`]; keys: [`ProbeKey`]), schema-checked;
//! * a deterministic SAMPLER of candidate keys on a rule's face, per shell (dots, rank) ([`face_keys`]): the probe's
//!   own dispatch decides which candidates are first-match keys of the rule;
//! * the per-rule AGGREGATION ([`aggregate`]): cost per application (median, p90 of the per-key minimum over the
//!   repeats), children per application and per receiving owner, size of the specialised row, the profiled split;
//! * the JOIN with a run profile ([`profile_rules`]: calls and seconds per rule from the owner statistics of a run);
//! * the RULE-COST QUALITY SCORE ([`RuleCostModel`], [`RuleFeatures`]) used by the improve flow when its measure is
//!   `rule_cost` (improve.rs; default stays monomials / bytes: nothing changes unless a spec asks for it).
//!
//! Nothing here names a node, a line count or an arity: slots, supports, charts and shells come from the probe's
//! records and the spec (L8).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Header schema of `p_rule_cost rules`.
pub const PROBE_RULES_SCHEMA: &str = "rustred.rule-cost.rules.v1";
/// Header schema of `p_rule_cost keys`.
pub const PROBE_KEYS_SCHEMA: &str = "rustred.rule-cost.keys.v1";
/// Spec schema of `rr_campaign rule-cost`.
pub const SPEC_SCHEMA: &str = "rustred.campaign.rule-cost.v1";
/// Schema of the report `rr_campaign rule-cost` writes.
pub const REPORT_SCHEMA: &str = "rustred.campaign.rule-cost-report.v1";

/// Size of one polynomial or of several (terms summed, maxima kept), as the probe writes it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PolyShape {
    pub terms: u64,
    pub max_degree_base: u32,
    pub max_degree_index: u32,
    pub max_total_degree_indices: u32,
    pub max_bits: u64,
}

/// Header line of `p_rule_cost rules`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RulesHeader {
    pub tool: String,
    pub schema: String,
    pub package: String,
    pub class: String,
    pub presentation: u32,
    pub arity: usize,
    /// Owner support (hex).
    pub support: String,
    pub charts: Vec<String>,
    pub rules: usize,
    pub prepare_seconds: f64,
    pub rss_kb: u64,
}

/// One prepared rule, as the probe reads it (rustred `propagator::custom::rule_cost::CustomRuleShape`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProbeRule {
    /// Raw chart (hex) and position in its batch: the rule's name in owner statistics and in [`ProbeKey::via`].
    pub chart: String,
    pub batch: usize,
    pub position: usize,
    pub ordinal: usize,
    pub partition: bool,
    pub fixed: Vec<(usize, i64)>,
    pub equalities: usize,
    pub exception_branches: usize,
    pub exception_conditions: usize,
    pub rhs_terms: usize,
    pub distinct_coefficients: usize,
    pub distinct_original_denominators: usize,
    pub base_variables: usize,
    pub numerator: PolyShape,
    pub denominator: PolyShape,
    pub original_denominator: PolyShape,
    pub shifts: Vec<Vec<i64>>,
    /// The DISTINCT coefficients of the rule (TRICKS 72), when the probe could align its payload's coefficient table.
    pub coefficients: Option<DistinctCoefficients>,
}

/// A rule's distinct coefficients (what a per-application cache would specialise once): count, sharing (rhs terms
/// per distinct coefficient), summed sizes over the distinct ones, the largest one, distinct denominators, and the
/// canonical text bytes when the probe printed them (`--detail`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DistinctCoefficients {
    pub rhs_terms: u64,
    pub distinct: u64,
    pub sharing: f64,
    pub numerator: PolyShape,
    pub denominator: PolyShape,
    pub max_terms: u64,
    pub distinct_denominators: u64,
    pub distinct_denominator_terms: u64,
    pub text_bytes: Option<u64>,
    pub max_text_bytes: Option<u64>,
}

/// Monomial VISITS of one application by today's step (interfaces/rule-cost.md 1): every rhs term's coefficient is
/// preflighted and substituted (numerator + denominator: twice its monomials) and every term's original denominator is
/// substituted once more in the applicability check.
pub fn visits_per_term(rule: &ProbeRule) -> u64 {
    2 * rule.coefficient_monomials() + rule.original_denominator.terms
}

/// The same with a per-application cache: each DISTINCT coefficient once, each distinct denominator once (None
/// without the distinct sizes).
pub fn visits_distinct(rule: &ProbeRule) -> Option<u64> {
    let c = rule.coefficients.as_ref()?;
    Some(2 * (c.numerator.terms + c.denominator.terms) + c.distinct_denominator_terms)
}

impl ProbeRule {
    /// `(chart, position)` with the chart as a number.
    pub fn id(&self) -> Result<RuleId, String> {
        Ok(RuleId {
            chart: hex(&self.chart)?,
            position: self.position,
        })
    }

    /// Monomials of the coefficients (numerators + denominators, summed over the rhs terms).
    pub fn coefficient_monomials(&self) -> u64 {
        self.numerator.terms + self.denominator.terms
    }
}

/// A rule of one owner: its raw chart and its position in the chart's rule list (what the engine's owner statistics
/// and the probe call `(chart, position)`; a table's `index` within its chart under the native importer's order rule).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RuleId {
    pub chart: u64,
    pub position: usize,
}

impl std::fmt::Display for RuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:x}:{}", self.chart, self.position)
    }
}

/// A hex number with or without `0x` and leading zeros.
pub fn hex(text: &str) -> Result<u64, String> {
    let t = text.trim().trim_start_matches("0x");
    u64::from_str_radix(t, 16).map_err(|_| format!("{text:?} is not a hex number"))
}

/// Header line of `p_rule_cost keys`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeysHeader {
    pub tool: String,
    pub schema: String,
    pub package: String,
    pub class: String,
    pub presentation: u32,
    pub keys: usize,
    pub repeat: usize,
    pub profile_level: u8,
    pub profile_compiled: bool,
    pub prepare_seconds: f64,
    pub rss_kb_loaded: u64,
    pub rss_kb_prepared: u64,
    pub peak_reset: bool,
}

/// Children of one application that go to one receiving owner (`owner` = class label, `same-owner`'s hex id,
/// `zero` or `unmapped`), with the receiver's lane and the children's line count.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Receiver {
    pub owner: String,
    pub lane: String,
    pub lines: u32,
    pub children: u64,
}

/// The specialised row's size (coefficients in the base variables only).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Row {
    pub numerator: PolyShape,
    pub denominator: PolyShape,
}

/// One key of `p_rule_cost keys`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProbeKey {
    pub key: Vec<i64>,
    pub error: Option<String>,
    /// `"terminal"`, `"uncovered"` or `{"chart": hex, "position": n}`.
    pub via: Value,
    pub positioned_before: Option<usize>,
    pub dispatch_differs: bool,
    pub seconds: Vec<f64>,
    pub seconds_min: f64,
    pub seconds_median: f64,
    pub peak_rss_delta_kb: u64,
    pub children: u64,
    pub children_same_owner: u64,
    pub receivers: Vec<Receiver>,
    pub conditions: u64,
    pub row: Row,
    pub profile: Value,
}

impl ProbeKey {
    /// The rule applied at the key, if one was.
    pub fn rule(&self) -> Option<RuleId> {
        let chart = hex(self.via.get("chart")?.as_str()?).ok()?;
        let position = usize::try_from(self.via.get("position")?.as_u64()?).ok()?;
        Some(RuleId { chart, position })
    }
}

/// Parse a probe output: the header (checked against `schema`) and the body lines.
pub fn parse_probe<H: serde::de::DeserializeOwned, T: serde::de::DeserializeOwned>(
    text: &str,
    schema: &str,
) -> Result<(H, Vec<T>), String> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let first = lines.next().ok_or("empty probe output")?;
    let hv: Value = serde_json::from_str(first).map_err(|e| format!("probe header: {e}"))?;
    if hv.get("schema").and_then(Value::as_str) != Some(schema) {
        return Err(format!(
            "probe header schema {:?} is not {schema}",
            hv.get("schema")
        ));
    }
    let header = serde_json::from_value(hv).map_err(|e| format!("probe header: {e}"))?;
    let mut body = Vec::new();
    for (n, l) in lines.enumerate() {
        body.push(serde_json::from_str(l).map_err(|e| format!("probe line {}: {e}", n + 2))?);
    }
    Ok((header, body))
}

// ---------------------------------------------------------------------------------------------------------------
// sampler
// ---------------------------------------------------------------------------------------------------------------

/// Candidate keys per shell.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct SamplerSpec {
    /// Shells `[dots, rank]` (dots = sum over the support's lines of power - 1, rank = sum of |power| elsewhere).
    pub shells: Vec<[u32; 2]>,
    /// Candidate keys per shell and rule (fewer when the shell has fewer keys on the face).
    pub per_shell: usize,
    pub seed: u64,
}

impl Default for SamplerSpec {
    fn default() -> Self {
        SamplerSpec {
            shells: vec![],
            per_shell: 4,
            seed: 1,
        }
    }
}

/// SplitMix64 (deterministic, no dependency).
struct Mix(u64);

impl Mix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// `(dots, rank)` of `key` on an owner of support `support` (line slots = its set bits).
pub fn shell_of(key: &[i64], support: u64) -> [u32; 2] {
    let (mut dots, mut rank) = (0u32, 0u32);
    for (slot, &n) in key.iter().enumerate() {
        if slot < 64 && support >> slot & 1 == 1 {
            dots += u32::try_from((n - 1).max(0)).unwrap_or(u32::MAX);
        } else {
            rank += u32::try_from(n.unsigned_abs()).unwrap_or(u32::MAX);
        }
    }
    [dots, rank]
}

/// Candidate keys on the face `fixed` of an owner (`arity` slots, `support` = its line slots): fixed slots at their
/// values, every other line >= 1, every other slot <= 0, the shell's dots and rank shared out at random over the free
/// slots. Deterministic in (spec.seed, salt, shell); at most `per_shell` distinct keys per shell; a shell the face
/// cannot reach (its fixed part already exceeds it, or no free slot to take the rest) gives none.
pub fn face_keys(
    arity: usize,
    support: u64,
    fixed: &[(usize, i64)],
    spec: &SamplerSpec,
    salt: u64,
) -> Vec<(Vec<i64>, [u32; 2])> {
    let is_line = |slot: usize| slot < 64 && support >> slot & 1 == 1;
    let fixed_at: BTreeMap<usize, i64> = fixed.iter().copied().collect();
    let (mut fixed_dots, mut fixed_rank) = (0i64, 0i64);
    for (&slot, &v) in &fixed_at {
        if slot >= arity || (is_line(slot) && v < 1) || (!is_line(slot) && v > 0) {
            return vec![];
        }
        if is_line(slot) {
            fixed_dots += v - 1;
        } else {
            fixed_rank += -v;
        }
    }
    let free_lines: Vec<usize> = (0..arity)
        .filter(|s| is_line(*s) && !fixed_at.contains_key(s))
        .collect();
    let free_other: Vec<usize> = (0..arity)
        .filter(|s| !is_line(*s) && !fixed_at.contains_key(s))
        .collect();
    let mut out = Vec::new();
    for (k, &[dots, rank]) in spec.shells.iter().enumerate() {
        let (dd, dr) = (i64::from(dots) - fixed_dots, i64::from(rank) - fixed_rank);
        if dd < 0
            || dr < 0
            || (dd > 0 && free_lines.is_empty())
            || (dr > 0 && free_other.is_empty())
        {
            continue;
        }
        let mut rng =
            Mix(spec.seed ^ salt.rotate_left(17) ^ (k as u64).wrapping_mul(0xA24B_AED4_963E_E407));
        let mut seen = BTreeSet::new();
        let tries = spec.per_shell.saturating_mul(32).max(32);
        for _ in 0..tries {
            if seen.len() >= spec.per_shell {
                break;
            }
            let mut key: Vec<i64> = (0..arity)
                .map(|slot| match fixed_at.get(&slot) {
                    Some(&v) => v,
                    None if is_line(slot) => 1,
                    None => 0,
                })
                .collect();
            for _ in 0..dd {
                key[free_lines[rng.below(free_lines.len())]] += 1;
            }
            for _ in 0..dr {
                key[free_other[rng.below(free_other.len())]] -= 1;
            }
            seen.insert(key);
        }
        out.extend(seen.into_iter().map(|key| (key, [dots, rank])));
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// aggregation
// ---------------------------------------------------------------------------------------------------------------

/// Mean children per application that go to one receiver.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReceiverMean {
    pub owner: String,
    pub lane: String,
    pub lines: u32,
    pub per_application: f64,
}

/// Median seconds per application at one shell.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ShellCost {
    pub shell: [u32; 2],
    pub applications: usize,
    pub seconds_median: f64,
}

/// The measured application cost of one rule.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RuleCost {
    pub rule: Option<RuleId>,
    pub applications: usize,
    /// Over the keys, of each key's minimum over the repeats.
    pub seconds_median: f64,
    pub seconds_p90: f64,
    pub seconds_mean: f64,
    pub seconds_max: f64,
    pub children_median: f64,
    pub children_mean: f64,
    pub same_owner_mean: f64,
    pub receivers: Vec<ReceiverMean>,
    pub row_terms_median: f64,
    pub row_max_degree_base: u32,
    pub row_max_bits: u64,
    pub peak_rss_delta_kb_max: u64,
    /// Keys where a positioned alias / recipe would take the key before the rule (the probe does not emulate it).
    pub dispatch_differs: usize,
    /// Profiled split: per timer category, mean seconds per application (profiled keys only), and its share of the
    /// profiled wall.
    pub profile_applications: usize,
    pub profile_seconds: BTreeMap<String, f64>,
    pub profile_share: BTreeMap<String, f64>,
    pub by_shell: Vec<ShellCost>,
}

/// The `q`-quantile (nearest rank, `q` in [0, 1]) of `values`; 0 for none.
pub fn quantile(values: &[f64], q: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    let rank = ((q * v.len() as f64).ceil() as usize).clamp(1, v.len());
    v[rank - 1]
}

fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        0.5 * (v[n / 2 - 1] + v[n / 2])
    }
}

/// Per rule applied at the keys (error, terminal and uncovered keys skipped), on an owner of support `support`.
pub fn aggregate(keys: &[ProbeKey], support: u64) -> BTreeMap<RuleId, RuleCost> {
    let mut by_rule: BTreeMap<RuleId, Vec<&ProbeKey>> = BTreeMap::new();
    for k in keys.iter().filter(|k| k.error.is_none()) {
        if let Some(r) = k.rule() {
            by_rule.entry(r).or_default().push(k);
        }
    }
    let mut out = BTreeMap::new();
    for (rule, ks) in by_rule {
        let n = ks.len();
        let secs: Vec<f64> = ks.iter().map(|k| k.seconds_min).collect();
        let children: Vec<f64> = ks.iter().map(|k| k.children as f64).collect();
        let mut receivers: BTreeMap<(String, String, u32), u64> = BTreeMap::new();
        for k in &ks {
            for r in &k.receivers {
                *receivers
                    .entry((r.owner.clone(), r.lane.clone(), r.lines))
                    .or_default() += r.children;
            }
        }
        let mut profile_seconds: BTreeMap<String, f64> = BTreeMap::new();
        let (mut profiled, mut wall) = (0usize, 0.0f64);
        for k in &ks {
            let Some(cats) = k.profile.get("categories").and_then(Value::as_object) else {
                continue;
            };
            profiled += 1;
            wall += k
                .profile
                .get("wall_s")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            for (cat, v) in cats {
                *profile_seconds.entry(cat.clone()).or_default() +=
                    v.get("s").and_then(Value::as_f64).unwrap_or(0.0);
            }
        }
        let profile_share = profile_seconds
            .iter()
            .map(|(c, s)| (c.clone(), if wall > 0.0 { s / wall } else { 0.0 }))
            .collect();
        if profiled > 0 {
            for s in profile_seconds.values_mut() {
                *s /= profiled as f64;
            }
        }
        let mut shells: BTreeMap<[u32; 2], Vec<f64>> = BTreeMap::new();
        for k in &ks {
            shells
                .entry(shell_of(&k.key, support))
                .or_default()
                .push(k.seconds_min);
        }
        out.insert(
            rule,
            RuleCost {
                rule: Some(rule),
                applications: n,
                seconds_median: median(&secs),
                seconds_p90: quantile(&secs, 0.9),
                seconds_mean: secs.iter().sum::<f64>() / n as f64,
                seconds_max: secs.iter().copied().fold(0.0, f64::max),
                children_median: median(&children),
                children_mean: children.iter().sum::<f64>() / n as f64,
                same_owner_mean: ks.iter().map(|k| k.children_same_owner as f64).sum::<f64>()
                    / n as f64,
                receivers: receivers
                    .into_iter()
                    .map(|((owner, lane, lines), c)| ReceiverMean {
                        owner,
                        lane,
                        lines,
                        per_application: c as f64 / n as f64,
                    })
                    .collect(),
                row_terms_median: median(
                    &ks.iter()
                        .map(|k| (k.row.numerator.terms + k.row.denominator.terms) as f64)
                        .collect::<Vec<_>>(),
                ),
                row_max_degree_base: ks
                    .iter()
                    .map(|k| {
                        k.row
                            .numerator
                            .max_degree_base
                            .max(k.row.denominator.max_degree_base)
                    })
                    .max()
                    .unwrap_or(0),
                row_max_bits: ks
                    .iter()
                    .map(|k| k.row.numerator.max_bits.max(k.row.denominator.max_bits))
                    .max()
                    .unwrap_or(0),
                peak_rss_delta_kb_max: ks.iter().map(|k| k.peak_rss_delta_kb).max().unwrap_or(0),
                dispatch_differs: ks.iter().filter(|k| k.dispatch_differs).count(),
                profile_applications: profiled,
                profile_seconds,
                profile_share,
                by_shell: shells
                    .into_iter()
                    .map(|(shell, v)| ShellCost {
                        shell,
                        applications: v.len(),
                        seconds_median: median(&v),
                    })
                    .collect(),
            },
        );
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// run profile
// ---------------------------------------------------------------------------------------------------------------

/// Calls and step seconds of one rule in a run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RunRule {
    pub calls: u64,
    pub seconds: f64,
}

/// The rules of owner `class` (hex id or label `...-<hex>`) in an owner-statistics document (`RUSTRED_P_OWNER_STATS`:
/// `{"owners": [{"class": hex, "rules": [[chart hex, position, calls, seconds], ...]}]}`); empty when the owner did
/// not step.
pub fn profile_rules(doc: &Value, class: &str) -> Result<BTreeMap<RuleId, RunRule>, String> {
    let want = hex(class.rsplit('-').next().unwrap_or(class))?;
    let owners = doc
        .get("owners")
        .and_then(Value::as_array)
        .ok_or("owner statistics: no owners array")?;
    let mut out = BTreeMap::new();
    for o in owners {
        let Some(c) = o.get("class").and_then(Value::as_str) else {
            continue;
        };
        let Ok(id) = hex(c.rsplit('-').next().unwrap_or(c)) else {
            continue;
        };
        if id != want {
            continue;
        }
        for r in o
            .get("rules")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let a = r
                .as_array()
                .ok_or("owner statistics: a rule entry is not an array")?;
            let (Some(chart), Some(position), Some(calls), Some(seconds)) = (
                a.first().and_then(Value::as_str),
                a.get(1).and_then(Value::as_u64),
                a.get(2).and_then(Value::as_u64),
                a.get(3).and_then(Value::as_f64),
            ) else {
                return Err(format!("owner statistics: rule entry {r}"));
            };
            out.insert(
                RuleId {
                    chart: hex(chart)?,
                    position: position as usize,
                },
                RunRule { calls, seconds },
            );
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------------------------
// the quality score
// ---------------------------------------------------------------------------------------------------------------

/// What the score needs of a rule, read from its table record ([`RuleFeatures::from_record`]).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RuleFeatures {
    pub rhs_terms: u64,
    /// Additive terms of the coefficient texts at ANY parenthesis depth (expanded numerators and denominators: about
    /// their monomials, +1 per coefficient); improve's `monomials` counts top-level terms only.
    pub coefficient_monomials: u64,
    /// Additive terms of the denominator texts (after the last top-level `/`), summed over the rhs terms: what the
    /// applicability check substitutes once per term.
    pub denominator_monomials: u64,
    /// Distinct coefficient texts, their additive terms summed over the distinct ones, their bytes and the largest.
    pub distinct_coefficients: u64,
    pub distinct_coefficient_monomials: u64,
    pub distinct_coefficient_bytes: u64,
    pub max_coefficient_bytes: u64,
    /// Additive terms of the distinct denominator texts.
    pub distinct_denominator_monomials: u64,
    /// Distinct denominators among the coefficient texts (the text after the last top-level `/`; `1` when none).
    pub distinct_denominators: u64,
    /// Children that stay in the rule's sector at a generic key of its case.
    pub children_same_sector: u64,
    /// Children that drop `k` lines at a generic key of the case: `k -> count`.
    pub children_lower: BTreeMap<u32, u64>,
    /// Some rhs integral could not be read (the children counts are then incomplete).
    pub unread_terms: u64,
}

/// One slot of an rhs integral text: `n<k>`, `n<k>+s`, `n<k>-s` or an integer.
fn slot_value(text: &str, fixed: &[Option<i64>]) -> Option<SlotValue> {
    let t = text.trim();
    if let Ok(v) = t.parse::<i64>() {
        return Some(SlotValue::Known(v));
    }
    let rest = t.strip_prefix('n')?;
    let split = rest.find(['+', '-']).unwrap_or(rest.len());
    let (k, off) = rest.split_at(split);
    let k: usize = k.parse().ok()?;
    let off: i64 = if off.is_empty() {
        0
    } else {
        off.replace('+', "").parse().ok()?
    };
    Some(match fixed.get(k).copied().flatten() {
        Some(v) => SlotValue::Known(v + off),
        None => SlotValue::Free,
    })
}

enum SlotValue {
    Known(i64),
    Free,
}

/// Additive terms of a coefficient text at any depth: per sum (the whole text and each parenthesised group) its binary
/// `+` / `-` separators plus one.
fn terms_any_depth(coef: &str) -> u64 {
    let mut total = 1u64;
    let mut prev: Option<char> = None;
    for ch in coef.chars().filter(|c| !c.is_whitespace()) {
        match ch {
            '(' => total += 1,
            '+' | '-'
                if prev.is_some_and(|p| p.is_ascii_alphanumeric() || p == ')' || p == '_') =>
            {
                total += 1
            }
            _ => {}
        }
        prev = Some(ch);
    }
    total
}

/// The denominator part of a coefficient text: what follows the last top-level `/` (None when there is none).
fn denominator_text(coef: &str) -> Option<&str> {
    let mut depth = 0i32;
    let mut at = None;
    for (i, ch) in coef.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            '/' if depth == 0 => at = Some(i),
            _ => {}
        }
    }
    at.map(|i| coef[i + 1..].trim())
}

impl RuleFeatures {
    /// From one `table_dump.v1` rule record: `chart` (`1` = a line slot), `fixed`, `rhs` = `[[integral, coefficient]]`.
    /// A free slot counts as a kept line (generic key of the case); a fixed one is evaluated.
    pub fn from_record(v: &Value) -> Option<RuleFeatures> {
        let rhs = v.get("rhs")?.as_array()?;
        let chart: Vec<bool> = v
            .get("chart")?
            .as_str()?
            .chars()
            .map(|c| c == '1')
            .collect();
        let fixed: Vec<Option<i64>> = v
            .get("fixed")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(Value::as_i64).collect())
            .unwrap_or_default();
        let mut f = RuleFeatures {
            rhs_terms: rhs.len() as u64,
            ..RuleFeatures::default()
        };
        let mut dens: BTreeSet<&str> = BTreeSet::new();
        let mut coefs: BTreeSet<&str> = BTreeSet::new();
        for term in rhs {
            let (Some(integral), coef) = (
                term.get(0).and_then(Value::as_str),
                term.get(1).and_then(Value::as_str).unwrap_or("1"),
            ) else {
                f.unread_terms += 1;
                continue;
            };
            if dens.insert(denominator_text(coef).unwrap_or("1")) {
                f.distinct_denominator_monomials +=
                    terms_any_depth(denominator_text(coef).unwrap_or("1"));
            }
            f.coefficient_monomials += terms_any_depth(coef);
            f.denominator_monomials += terms_any_depth(denominator_text(coef).unwrap_or("1"));
            if coefs.insert(coef) {
                f.distinct_coefficient_monomials += terms_any_depth(coef);
                f.distinct_coefficient_bytes += coef.len() as u64;
                f.max_coefficient_bytes = f.max_coefficient_bytes.max(coef.len() as u64);
            }
            let inner = integral
                .trim()
                .strip_prefix("I(")
                .and_then(|s| s.strip_suffix(')'));
            let Some(inner) = inner else {
                f.unread_terms += 1;
                continue;
            };
            let slots: Vec<&str> = inner.split(',').collect();
            let mut dropped = 0u32;
            let mut ok = slots.len() == chart.len();
            for (slot, text) in slots.iter().enumerate() {
                match slot_value(text, &fixed) {
                    Some(SlotValue::Known(v)) if chart.get(slot) == Some(&true) && v <= 0 => {
                        dropped += 1
                    }
                    Some(_) => {}
                    None => ok = false,
                }
            }
            if !ok {
                f.unread_terms += 1;
            } else if dropped == 0 {
                f.children_same_sector += 1;
            } else {
                *f.children_lower.entry(dropped).or_default() += 1;
            }
        }
        f.distinct_denominators = dens.len() as u64;
        f.distinct_coefficients = coefs.len() as u64;
        Some(f)
    }

    /// All children (read ones).
    pub fn children(&self) -> u64 {
        self.children_same_sector + self.children_lower.values().sum::<u64>()
    }
}

/// How a sub-table that replaces one rule shares that rule's calls among its pieces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubtableCalls {
    /// Every call at the most expensive piece (conservative: a sub-table must be cheaper at its worst piece).
    #[default]
    Max,
    /// The calls split evenly over the pieces (a guess: the real split needs the run's keys re-dispatched).
    Uniform,
}

/// The RULE-COST score of improve's `rule_cost` measure (interfaces/rule-cost.md 4). Every number is DATA: the
/// per-application terms are fitted from `rr_campaign rule-cost` measurements, the calls come from a run profile,
/// the child weights from the receiving owners' step cost; the defaults are documented guesses (interfaces 4).
///
/// score(rule) = calls(rule) x [ apply_ns_base + apply_ns_per_term x terms + apply_ns_per_monomial x monomials
///                               + apply_ns_per_denominator x distinct denominators
///                               + child_ns_same_sector x same-sector children
///                               + sum_k child_ns_lower(k) x children dropping k lines ]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct RuleCostModel {
    pub apply_ns_base: f64,
    pub apply_ns_per_term: f64,
    pub apply_ns_per_monomial: f64,
    pub apply_ns_per_denominator: f64,
    /// Per monomial of the DENOMINATOR texts over all terms (the per-term applicability check).
    pub apply_ns_per_denominator_monomial: f64,
    /// Per monomial of the DISTINCT coefficients, and per distinct coefficient (an engine that specialises each
    /// distinct coefficient once per application: TRICKS 72).
    pub apply_ns_per_distinct_monomial: f64,
    pub apply_ns_per_distinct_coefficient: f64,
    /// Per monomial of the DISTINCT denominators (an engine that checks / specialises each distinct denominator once).
    pub apply_ns_per_distinct_denominator_monomial: f64,
    pub child_ns_same_sector: f64,
    /// Default cost of a child that drops lines; `child_ns_by_lines_dropped` overrides it per number dropped.
    pub child_ns_lower: f64,
    pub child_ns_by_lines_dropped: BTreeMap<u32, f64>,
    /// Calls per rule, `"<chart hex>:<position>"` (a run profile; [`profile_rules`]); absent = `default_calls`.
    pub calls: BTreeMap<String, f64>,
    /// The calls of a rule the profile does not name (uniform weights without a profile).
    pub default_calls: f64,
    pub subtable: SubtableCalls,
}

/// The defaults are MEASURED on today's step (engine 12b, reports/RULE-COST.md 5: 120 rules of four eleven- and
/// nine-line owners, single thread, shared host): 29 us per application, 70 ns per monomial visit, where one
/// application visits every rhs term's coefficient twice (preflight + substitution: 140 ns per all-term coefficient
/// monomial) and every term's original denominator once (70 ns per all-term denominator monomial), 2.6 us per child
/// inside the step + about 1 us of routing. Distinct-coefficient terms are 0: today's step does not share coefficients
/// within an application; an engine that does (TRICKS 72) moves the 140 ns to `apply_ns_per_distinct_monomial` (DATA).
/// Child weights hold no downstream cost: a NEW key's later steps depend on its receiver and are not measured yet.
impl Default for RuleCostModel {
    fn default() -> Self {
        RuleCostModel {
            apply_ns_base: 29_000.0,
            apply_ns_per_term: 0.0,
            apply_ns_per_monomial: 140.0,
            apply_ns_per_denominator: 0.0,
            apply_ns_per_denominator_monomial: 70.0,
            apply_ns_per_distinct_monomial: 0.0,
            apply_ns_per_distinct_coefficient: 0.0,
            apply_ns_per_distinct_denominator_monomial: 0.0,
            child_ns_same_sector: 3_600.0,
            child_ns_lower: 3_600.0,
            child_ns_by_lines_dropped: BTreeMap::new(),
            calls: BTreeMap::new(),
            default_calls: 1.0,
            subtable: SubtableCalls::Max,
        }
    }
}

impl RuleCostModel {
    /// The same weights for an engine with a per-application coefficient cache (TRICKS 72; ESTIMATE): the per-term
    /// coefficient and denominator weights move to the distinct coefficients and distinct denominators.
    pub fn cached(&self) -> RuleCostModel {
        RuleCostModel {
            apply_ns_per_monomial: 0.0,
            apply_ns_per_denominator_monomial: 0.0,
            apply_ns_per_distinct_monomial: self.apply_ns_per_distinct_monomial
                + self.apply_ns_per_monomial,
            apply_ns_per_distinct_denominator_monomial: self
                .apply_ns_per_distinct_denominator_monomial
                + self.apply_ns_per_denominator_monomial,
            ..self.clone()
        }
    }

    /// The calls of the rule at `(chart, index)`, the chart as the table record writes it (a `0/1` slot string).
    pub fn calls_of(&self, chart: &str, index: i64) -> f64 {
        self.calls
            .get(&format!("{chart}:{index}"))
            .copied()
            .unwrap_or(self.default_calls)
    }

    /// Nanoseconds of ONE application (with its children) of a rule of `monomials` coefficient monomials.
    /// With features, their any-depth `coefficient_monomials` replace `monomials` (the record's own count).
    pub fn per_application_ns(&self, monomials: u64, f: Option<&RuleFeatures>) -> f64 {
        let m = f.map_or(monomials, |f| f.coefficient_monomials.max(monomials));
        let mut ns = self.apply_ns_base + self.apply_ns_per_monomial * m as f64;
        if let Some(f) = f {
            ns += self.apply_ns_per_term * f.rhs_terms as f64
                + self.apply_ns_per_denominator * f.distinct_denominators as f64
                + self.apply_ns_per_denominator_monomial * f.denominator_monomials as f64
                + self.apply_ns_per_distinct_monomial * f.distinct_coefficient_monomials as f64
                + self.apply_ns_per_distinct_coefficient * f.distinct_coefficients as f64
                + self.apply_ns_per_distinct_denominator_monomial
                    * f.distinct_denominator_monomials as f64
                + self.child_ns_same_sector * f.children_same_sector as f64;
            for (&k, &n) in &f.children_lower {
                let w = self
                    .child_ns_by_lines_dropped
                    .get(&k)
                    .copied()
                    .unwrap_or(self.child_ns_lower);
                ns += w * n as f64;
            }
        }
        ns
    }
}

/// Audit 2026-11-19 (a1): the native importer's PAYLOAD ordinal of every rule record of a table. Per chart, in file
/// order, a rule record whose rhs keys name another slot's index (`n<k>` at slot `j != k`: a slot-permuted ALIAS,
/// `table_import.rs` `permuted_rhs`) is kept apart as a slot alias and gets no payload position; every other rule record
/// takes the next ordinary position of its chart. Profiles (owner statistics, probe positions) count ordinary payload
/// positions; tables count records: the two differ after the first alias (T162-final: 19 aliases, source 1246 =
/// payload 1245).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PayloadMap {
    /// (table chart string, record index) -> ordinary payload position (aliases absent).
    pub by_source: BTreeMap<(String, i64), usize>,
    /// (table chart string, payload position) -> record index.
    pub by_payload: BTreeMap<(String, usize), i64>,
    /// The alias records (table chart string, record index).
    pub aliases: Vec<(String, i64)>,
}

/// Is a rule record's rhs slot-permuted (any rhs key slot `j` written `n<k>` or `n<k>+-c` with `k != j`)?
pub fn is_slot_alias(v: &Value) -> bool {
    v.get("rhs").and_then(Value::as_array).is_some_and(|rhs| {
        rhs.iter().any(|t| {
            t.get(0)
                .and_then(Value::as_str)
                .and_then(|k| k.trim().strip_prefix("I(")?.strip_suffix(')'))
                .is_some_and(|inner| {
                    inner.split(',').enumerate().any(|(j, s)| {
                        let s = s.trim();
                        s.strip_prefix('n').is_some_and(|rest| {
                            let end = rest.find(['+', '-']).unwrap_or(rest.len());
                            rest[..end].parse::<usize>().is_ok_and(|k| k != j)
                        })
                    })
                })
        })
    })
}

impl PayloadMap {
    /// From a table's lines (any record kind; rule records by `type`, chart and index).
    pub fn from_lines<'a>(lines: impl IntoIterator<Item = &'a str>) -> Result<PayloadMap, String> {
        let mut m = PayloadMap::default();
        let mut next: BTreeMap<String, usize> = BTreeMap::new();
        for (n, l) in lines.into_iter().enumerate() {
            if l.trim().is_empty() || !l.contains("\"rule\"") {
                continue;
            }
            let v: Value = serde_json::from_str(l).map_err(|e| format!("line {}: {e}", n + 1))?;
            if v.get("type").and_then(Value::as_str) != Some("rule") {
                continue;
            }
            let chart = v
                .get("chart")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("line {}: rule without chart", n + 1))?
                .to_string();
            let index = v
                .get("index")
                .and_then(Value::as_i64)
                .ok_or_else(|| format!("line {}: rule without index", n + 1))?;
            if is_slot_alias(&v) {
                m.aliases.push((chart, index));
                continue;
            }
            let p = next.entry(chart.clone()).or_insert(0);
            m.by_source.insert((chart.clone(), index), *p);
            m.by_payload.insert((chart, *p), index);
            *p += 1;
        }
        Ok(m)
    }
}

/// The improve model's calls from a profile, bound to SOURCE records (audit 2026-11-19, a1).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProfileCalls {
    /// `"<table chart>:<record index>"` -> calls (the key improve's `rule_cost` looks up).
    pub calls: BTreeMap<String, f64>,
    /// Profile positions with no source record (`"<table chart>:payload <position>"`): reported, never guessed.
    pub unmapped: Vec<String>,
}

/// The `calls` of a [`RuleCostModel`] from a run profile: each rule's chart (owner-presentation mask) in the table's slot
/// order (`table_to_presentation`, identity when empty) and its PAYLOAD position translated to the table's record index
/// through `map`. Without a map nothing is bound (every position is unmapped): a payload ordinal is never used as a
/// record index.
pub fn calls_from_profile(
    profile: &BTreeMap<RuleId, RunRule>,
    table_to_presentation: &[usize],
    arity: usize,
    map: Option<&PayloadMap>,
) -> ProfileCalls {
    let mut out = ProfileCalls::default();
    for (id, run) in profile {
        let chart: String = (0..arity)
            .map(|i| {
                let p = table_to_presentation.get(i).copied().unwrap_or(i);
                if p < 64 && id.chart >> p & 1 == 1 {
                    '1'
                } else {
                    '0'
                }
            })
            .collect();
        match map.and_then(|m| m.by_payload.get(&(chart.clone(), id.position))) {
            Some(index) => {
                out.calls
                    .insert(format!("{chart}:{index}"), run.calls as f64);
            }
            None => out
                .unmapped
                .push(format!("{chart}:payload {}", id.position)),
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// one level of same-sector children (the score's NEW-key term, interfaces/rule-cost.md 4)
// ---------------------------------------------------------------------------------------------------------------

/// The value at `key` of one rhs integral slot text (`n<k>`, `n<k>+s`, `n<k>-s` or an integer); None if unreadable.
fn slot_at(text: &str, key: &[i32]) -> Option<i32> {
    let t = text.trim();
    if let Ok(v) = t.parse::<i32>() {
        return Some(v);
    }
    let rest = t.strip_prefix('n')?;
    let split = rest.find(['+', '-']).unwrap_or(rest.len());
    let (k, off) = rest.split_at(split);
    let k: usize = k.parse().ok()?;
    let off: i32 = if off.is_empty() {
        0
    } else {
        off.replace('+', "").parse().ok()?
    };
    Some(key.get(k)? + off)
}

/// The children of the rule record `v` (table_dump.v1: `chart` 0/1 slot string, `rhs` = `[[integral, coefficient]]`)
/// at `key` that stay in the rule's sector (every chart line >= 1): the keys a step would hand back to the SAME owner.
/// Unreadable rhs integrals are skipped; None when the record has no rhs or chart.
pub fn same_sector_children(v: &Value, key: &[i32]) -> Option<Vec<Vec<i32>>> {
    let rhs = v.get("rhs")?.as_array()?;
    let chart: Vec<bool> = v
        .get("chart")?
        .as_str()?
        .chars()
        .map(|c| c == '1')
        .collect();
    let mut out = Vec::new();
    for term in rhs {
        let Some(inner) = term
            .get(0)
            .and_then(Value::as_str)
            .and_then(|s| s.trim().strip_prefix("I("))
            .and_then(|s| s.strip_suffix(')'))
        else {
            continue;
        };
        let child: Option<Vec<i32>> = inner.split(',').map(|x| slot_at(x, key)).collect();
        let Some(child) = child else { continue };
        if child.len() == chart.len() && child.iter().zip(&chart).all(|(&n, &line)| !line || n >= 1)
        {
            out.push(child);
        }
    }
    Some(out)
}
