//! The judge of a symbolic candidate and the crossover cost model (interfaces/repairflow.md section 3).
//! Pure: the measures come from the candidate's own record line; nothing is estimated from a clock.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::faces::Key;
use super::policy::RepairPolicy;
use crate::key::NodeFrame;

/// Size measures of a candidate rule, read from its engine record (table_dump.v1 rule line).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Measures {
    /// Length of the record line (bytes, without the newline).
    pub bytes: u64,
    pub rhs_terms: u64,
    /// The engine's `monomials` (coefficient monomials of the lifted rule; 0 when absent).
    pub monomials: u64,
    pub trace_rows: u64,
    /// `seconds_search + lift_seconds` as recorded (information only, never a decision input).
    pub seconds: f64,
    /// Free directions of the rule's case: symbolic slots (`fixed` null) minus its equations.
    #[serde(default)]
    pub free_dims: u64,
}

/// The measures of a rule line.
pub fn measures_of(line: &str) -> Result<Measures, String> {
    let line = line.trim_end_matches('\n');
    let v: Value = serde_json::from_str(line).map_err(|e| format!("candidate record: {e}"))?;
    if v.get("type").and_then(Value::as_str) != Some("rule") {
        return Err("candidate record: not a rule line".into());
    }
    let rhs = match v.get("rhs") {
        Some(Value::Array(a)) => a.len() as u64,
        Some(Value::String(s)) => serde_json::from_str::<Value>(s)
            .ok()
            .and_then(|x| x.as_array().map(|a| a.len() as u64))
            .ok_or("candidate record: rhs string is not an array")?,
        _ => return Err("candidate record: no rhs".into()),
    };
    let u = |k: &str| v.get(k).and_then(Value::as_u64).unwrap_or(0);
    let free = v
        .get("fixed")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter(|x| x.is_null()).count() as u64)
        .unwrap_or(0);
    let eqs = v
        .get("equations")
        .and_then(Value::as_array)
        .map(|a| a.len() as u64)
        .unwrap_or(0);
    let f = |k: &str| v.get(k).and_then(Value::as_f64).unwrap_or(0.0);
    Ok(Measures {
        bytes: line.len() as u64,
        rhs_terms: rhs,
        monomials: u("monomials"),
        trace_rows: u("trace_rows"),
        seconds: f("seconds_search") + f("lift_seconds"),
        free_dims: free.saturating_sub(eqs),
    })
}

/// The open keys of the face at which the candidate's case domain APPLIES (chart, fixed slots, equations, every
/// exception branch, witness poles: the core dispatcher's semantics). A record whose domain cannot be built is an
/// error (the class names it).
pub fn served_keys(line: &str, frame: &NodeFrame, open: &[Key]) -> Result<Vec<Key>, String> {
    let rt = crate::accept::rule::RuleText::parse(line.as_bytes(), frame)
        .map_err(|e| format!("candidate record: {e:?}"))?;
    let d = rt
        .domain
        .ok_or_else(|| format!("candidate domain: {}", rt.domain_error.unwrap_or_default()))?;
    Ok(open
        .iter()
        .filter(|k| d.applies(k).applies())
        .cloned()
        .collect())
}

/// Why a face's symbolic outcome is what it is (first applicable class, in this order).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "class")]
pub enum Class {
    /// The candidate is kept (subject to the node budget).
    Keep,
    /// Symbolic step off by policy.
    SymbolicOff,
    /// Beyond `symbolic.max_faces`.
    FaceCap,
    /// The shadowing test proved the face matched by earlier records / empty.
    Shadowed,
    EmptyDomain,
    /// The job gave no candidate: budget stop, refusal (screen, exhausted shells, open obligation), error.
    NoCandidate {
        why: String,
    },
    /// The generated table is not in the node's frame.
    FrameMismatch {
        why: String,
    },
    /// The candidate's domain could not be built.
    DomainError {
        why: String,
    },
    /// A per-rule cap is exceeded.
    OverCap {
        field: String,
        value: u64,
        cap: u64,
    },
    /// It serves too few open keys of its face.
    TooFewKeys {
        served: u64,
        needed: u64,
    },
    /// Following the recipe at the served keys is cheaper (cost model).
    CostlierThanRecipe,
    /// Kept by the judge, but the node budget was exhausted (`order` of the budget decided).
    OverNodeBudget {
        running_bytes: u64,
        running_rules: u64,
        max_bytes: u64,
        max_rules: u64,
    },
}

/// The numbers of the cost model for one candidate (ns).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Cost {
    pub uses: f64,
    pub c_table_ns: f64,
    pub c_recipe_ns: f64,
}

/// C_table and C_recipe of a candidate serving `served` keys (interface section 3).
pub fn cost(m: &Measures, served: u64, p: &RepairPolicy) -> Cost {
    let c = &p.cost;
    let uses =
        served as f64 * c.uses_per_served_key + m.free_dims as f64 * c.unbounded_direction_uses;
    let c_table = c.table_weight
        * (c.load_ns_per_byte * m.bytes as f64
            + uses * (c.apply_ns_base + c.apply_ns_per_monomial * m.monomials as f64));
    let h = c.cache_hit_fraction;
    let c_recipe = uses
        * ((1.0 - h) * c.recipe_solve_s * 1e9
            + h * (c.cached_ns_base + c.cached_ns_per_rhs_term * m.rhs_terms as f64));
    Cost {
        uses,
        c_table_ns: c_table,
        c_recipe_ns: c_recipe,
    }
}

/// The judge of one candidate (the face's open keys, the keys its domain applies at).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Judgement {
    pub class: Class,
    pub measures: Option<Measures>,
    pub served: u64,
    pub left: u64,
    pub cost: Option<Cost>,
}

/// Judge a candidate with measures `m` serving `served` of the face's `open` open keys.
pub fn judge(m: &Measures, served: u64, open: u64, p: &RepairPolicy) -> Judgement {
    let k = &p.keep;
    let over = [
        ("rhs_terms", m.rhs_terms, k.max_rhs_terms),
        ("monomials", m.monomials, k.max_monomials),
        ("bytes", m.bytes, k.max_bytes),
        ("trace_rows", m.trace_rows, k.max_trace_rows),
    ]
    .into_iter()
    .find(|(_, v, c)| v > c);
    let cst = cost(m, served, p);
    let needed_frac = (k.min_served_fraction * open as f64).ceil() as u64;
    let needed = k.min_keys_served.max(needed_frac);
    let class = if let Some((f, v, c)) = over {
        Class::OverCap {
            field: f.into(),
            value: v,
            cap: c,
        }
    } else if served < needed {
        Class::TooFewKeys { served, needed }
    } else if p.cost.enabled && cst.c_table_ns > cst.c_recipe_ns {
        Class::CostlierThanRecipe
    } else {
        Class::Keep
    };
    Judgement {
        class,
        measures: Some(m.clone()),
        served,
        left: open.saturating_sub(served),
        cost: Some(cst),
    }
}

/// A judgement without a candidate.
pub fn without_candidate(class: Class, open: u64) -> Judgement {
    Judgement {
        class,
        measures: None,
        served: 0,
        left: open,
        cost: None,
    }
}
