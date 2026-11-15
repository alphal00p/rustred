//! The spec section of the composed pole profiles (`rustred.campaign.epsdepth-spec.v1`; interfaces/epsdepth.md 8).
//! Every field optional with a documented default; unknown members are REFUSED (a typo must not fall back silently).
//!
//! ```json
//! {"d0": "4", "target_exponent": 0,
//!  "budgets": {"max_depth": 32, "max_keys": null, "max_seconds": null},
//!  "keys": {"mode": "all", "sample": {"n": 2000, "seed": 0}, "max_dots_rank": null},
//!  "terminal_set": "not_certified",
//!  "report": {"top_keys": 5, "top_blame": 10},
//!  "pole_objective": {"measures": ["eps_box_excess", "max_pole_order", "sum_pole_orders", "singular_terms", "size"],
//!                     "max_size_ratio": "unbounded"},
//!  "eps_box": null}
//! ```

use serde_json::{Map, Value, json};

use super::epsbox::{EpsBox, EpsBoxChecked};
use crate::poles::{MaxSizeRatio, PoleObjective};

/// Which box keys get a composed profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyMode {
    All,
    /// `n` keys drawn deterministically (splitmix64 with `seed`) from the sorted box keys.
    Sample { n: usize, seed: u64 },
    /// Box keys with dots + rank <= the bound (dots / rank from the node's line mask).
    Layer { max_dots_rank: i64 },
}

/// What is known of the terminal set at d0 (lane poles' B2 record; TRICKS 16: `inherent` only on a certified set).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalSetStatus {
    CertifiedRegular,
    Degenerate,
    NotCertified,
}

impl TerminalSetStatus {
    pub fn word(&self) -> &'static str {
        match self {
            TerminalSetStatus::CertifiedRegular => "certified_regular",
            TerminalSetStatus::Degenerate => "degenerate",
            TerminalSetStatus::NotCertified => "not_certified",
        }
    }
}

/// Budgets of one composition run.
#[derive(Clone, Debug, PartialEq)]
pub struct Budgets {
    /// Cap of R(k) - target per key (section 4); a capped key may leave box keys `undecided(budget:depth)`.
    pub max_depth: i64,
    /// Cap of the cone size (keys); keys beyond it are `undecided(budget:keys)`.
    pub max_keys: Option<usize>,
    /// Wall cap; results stopped by it are labelled time-decided.
    pub max_seconds: Option<f64>,
}

/// The parsed spec.
#[derive(Clone, Debug, PartialEq)]
pub struct EpsDepthSpec {
    pub d0: String,
    pub target_exponent: i64,
    pub budgets: Budgets,
    pub keys: KeyMode,
    pub terminal_set: TerminalSetStatus,
    pub top_keys: usize,
    pub top_blame: usize,
    pub objective: PoleObjective,
    pub eps_box: Option<EpsBoxChecked>,
}

impl Default for EpsDepthSpec {
    fn default() -> Self {
        EpsDepthSpec {
            d0: "4".into(),
            target_exponent: 0,
            budgets: Budgets { max_depth: 32, max_keys: None, max_seconds: None },
            keys: KeyMode::All,
            terminal_set: TerminalSetStatus::NotCertified,
            top_keys: 5,
            top_blame: 10,
            objective: PoleObjective::default(),
            eps_box: None,
        }
    }
}

fn obj<'a>(v: &'a Value, at: &str, allowed: &[&str]) -> Result<&'a Map<String, Value>, String> {
    let o = v.as_object().ok_or_else(|| format!("{at} must be an object"))?;
    for k in o.keys() {
        if !allowed.contains(&k.as_str()) {
            return Err(format!("{at}: unknown member {k:?} (allowed: {allowed:?})"));
        }
    }
    Ok(o)
}

fn uint(v: &Value, at: &str) -> Result<u64, String> {
    v.as_u64().ok_or_else(|| format!("{at} must be an unsigned integer"))
}

fn int(v: &Value, at: &str) -> Result<i64, String> {
    v.as_i64().ok_or_else(|| format!("{at} must be an integer"))
}

/// `a` or `a/b` (optional sign, digits, b != 0): the d0 text (lane poles' `D0::parse` normalises it in the algebra crate).
fn rational_text(t: &str) -> bool {
    let (a, b) = t.split_once('/').unwrap_or((t, "1"));
    let a = a.strip_prefix('-').unwrap_or(a);
    let dig = |s: &str| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit());
    dig(a) && dig(b) && b.bytes().any(|c| c != b'0')
}

impl EpsDepthSpec {
    /// Parse the section (`null` or absent members = defaults).
    pub fn from_value(v: &Value) -> Result<EpsDepthSpec, String> {
        let mut s = EpsDepthSpec::default();
        if v.is_null() {
            return Ok(s);
        }
        let o = obj(
            v,
            "epsdepth",
            &["d0", "target_exponent", "budgets", "keys", "terminal_set", "report", "pole_objective", "eps_box"],
        )?;
        if let Some(d) = o.get("d0") {
            let t = d.as_str().ok_or("epsdepth.d0 must be a text a or a/b")?;
            if !rational_text(t) {
                return Err(format!("epsdepth.d0 {t:?} is not a rational a or a/b"));
            }
            s.d0 = t.to_string();
        }
        if let Some(t) = o.get("target_exponent") {
            s.target_exponent = int(t, "epsdepth.target_exponent")?;
        }
        if let Some(b) = o.get("budgets") {
            let b = obj(b, "epsdepth.budgets", &["max_depth", "max_keys", "max_seconds"])?;
            if let Some(x) = b.get("max_depth") {
                s.budgets.max_depth = i64::try_from(uint(x, "epsdepth.budgets.max_depth")?)
                    .map_err(|_| "epsdepth.budgets.max_depth too large".to_string())?;
            }
            if let Some(x) = b.get("max_keys").filter(|x| !x.is_null()) {
                s.budgets.max_keys = Some(uint(x, "epsdepth.budgets.max_keys")? as usize);
            }
            if let Some(x) = b.get("max_seconds").filter(|x| !x.is_null()) {
                let f = x.as_f64().filter(|f| *f > 0.0).ok_or("epsdepth.budgets.max_seconds must be > 0")?;
                s.budgets.max_seconds = Some(f);
            }
        }
        if let Some(k) = o.get("keys") {
            let k = obj(k, "epsdepth.keys", &["mode", "sample", "max_dots_rank"])?;
            let mode = k.get("mode").map(|m| m.as_str().unwrap_or("?")).unwrap_or("all");
            let (mut n, mut seed) = (2000usize, 0u64);
            if let Some(sm) = k.get("sample") {
                let sm = obj(sm, "epsdepth.keys.sample", &["n", "seed"])?;
                if let Some(x) = sm.get("n") {
                    n = uint(x, "epsdepth.keys.sample.n")? as usize;
                }
                if let Some(x) = sm.get("seed") {
                    seed = uint(x, "epsdepth.keys.sample.seed")?;
                }
            }
            s.keys = match mode {
                "all" => KeyMode::All,
                "sample" => KeyMode::Sample { n, seed },
                "layer" => {
                    let b = k
                        .get("max_dots_rank")
                        .filter(|x| !x.is_null())
                        .ok_or("epsdepth.keys.mode layer needs max_dots_rank")?;
                    KeyMode::Layer { max_dots_rank: int(b, "epsdepth.keys.max_dots_rank")? }
                }
                m => return Err(format!("epsdepth.keys.mode {m:?} is not all / sample / layer")),
            };
        }
        if let Some(t) = o.get("terminal_set") {
            s.terminal_set = match t.as_str() {
                Some("certified_regular") => TerminalSetStatus::CertifiedRegular,
                Some("degenerate") => TerminalSetStatus::Degenerate,
                Some("not_certified") => TerminalSetStatus::NotCertified,
                _ => return Err(format!("epsdepth.terminal_set {t} is not certified_regular / degenerate / not_certified")),
            };
        }
        if let Some(r) = o.get("report") {
            let r = obj(r, "epsdepth.report", &["top_keys", "top_blame"])?;
            if let Some(x) = r.get("top_keys") {
                s.top_keys = uint(x, "epsdepth.report.top_keys")? as usize;
            }
            if let Some(x) = r.get("top_blame") {
                s.top_blame = uint(x, "epsdepth.report.top_blame")? as usize;
            }
        }
        if let Some(p) = o.get("pole_objective") {
            s.objective = PoleObjective::from_value(p, MaxSizeRatio::Unbounded).map_err(|e| e.to_string())?;
        }
        if let Some(b) = o.get("eps_box").filter(|b| !b.is_null()) {
            s.eps_box = Some(EpsBox::from_value(b)?);
        }
        Ok(s)
    }

    /// The effective spec with every default filled in (written into report headers).
    pub fn to_json(&self) -> Value {
        let keys = match &self.keys {
            KeyMode::All => json!({"mode": "all"}),
            KeyMode::Sample { n, seed } => json!({"mode": "sample", "sample": {"n": n, "seed": seed}}),
            KeyMode::Layer { max_dots_rank } => json!({"mode": "layer", "max_dots_rank": max_dots_rank}),
        };
        json!({"schema": "rustred.campaign.epsdepth-spec.v1", "d0": self.d0, "target_exponent": self.target_exponent,
            "budgets": {"max_depth": self.budgets.max_depth, "max_keys": self.budgets.max_keys,
                        "max_seconds": self.budgets.max_seconds},
            "keys": keys, "terminal_set": self.terminal_set.word(),
            "report": {"top_keys": self.top_keys, "top_blame": self.top_blame},
            "pole_objective": self.objective.to_json(),
            "eps_box": self.eps_box.as_ref().map(|b| serde_json::to_value(&b.spec).unwrap_or(Value::Null))})
    }

    /// Select box keys by the key mode. `keys` sorted or not: the result is sorted; `dots_rank` maps a key to its
    /// dots + rank. Deterministic.
    pub fn select_keys(&self, keys: &[Vec<i32>], dots_rank: impl Fn(&[i32]) -> i64) -> Vec<Vec<i32>> {
        let mut all: Vec<Vec<i32>> = keys.to_vec();
        all.sort();
        all.dedup();
        match &self.keys {
            KeyMode::All => all,
            KeyMode::Layer { max_dots_rank } => all.into_iter().filter(|k| dots_rank(k) <= *max_dots_rank).collect(),
            KeyMode::Sample { n, seed } => {
                if *n >= all.len() {
                    return all;
                }
                // partial Fisher-Yates with splitmix64: the first n of a seeded permutation, then sorted
                let mut st = *seed;
                let mut next = || {
                    st = st.wrapping_add(0x9E37_79B9_7F4A_7C15);
                    let mut z = st;
                    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
                    z ^ (z >> 31)
                };
                let len = all.len();
                for i in 0..*n {
                    let j = i + (next() % (len - i) as u64) as usize;
                    all.swap(i, j);
                }
                all.truncate(*n);
                all.sort();
                all
            }
        }
    }
}

/// Dots + rank of a key under a line mask (dots = sum over lines of (power - 1), rank = sum of |numerator powers|).
pub fn dots_rank(key: &[i32], lines: &[bool]) -> i64 {
    key.iter()
        .zip(lines)
        .map(|(&v, &l)| if l { i64::from(v) - 1 } else { i64::from(v).abs() })
        .sum()
}
