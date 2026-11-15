//! COPY (not a move) of pieces of rustred-core/src/node/replay/mod.rs at engine commit b9f06de7 (`Pat`, `parse_integral`, `key_at`, `power_decode`, `Rule`, `rule_from`, `seed_at`) for the
//! campaign's independent A2 row check (plan 3.1 (e)); changes: items made `pub` (and the ones marked "campaign copy").

use serde_json::Value;

use super::algebra::Unknown;

/// The engine's key type of the replay.
pub type Key = Vec<i64>;

/// One integral-key coordinate: (Some(axis), offset) symbolic, (None, value) numeric.
pub type Pat = Vec<(Option<usize>, i64)>;

pub fn parse_integral(text: &str, n: usize) -> Result<Pat, Unknown> {
    let inner = text
        .strip_prefix("I(")
        .and_then(|s| s.strip_suffix(')'))
        .ok_or_else(|| format!("unsupported integral key: {text}"))?;
    let parts: Vec<&str> = inner.split(',').collect();
    if parts.len() != n {
        return Err("truncated physical key".into());
    }
    parts
        .iter()
        .enumerate()
        .map(|(i, x)| {
            if let Some(rest) = x.strip_prefix('n') {
                let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
                let axis: usize = digits
                    .parse()
                    .map_err(|_| format!("unsupported integral key: {text}"))?;
                if axis != i {
                    return Err(format!(
                        "unsupported integral key: key axis permuted {text}"
                    ));
                }
                let off = &rest[digits.len()..];
                let off: i64 = if off.is_empty() {
                    0
                } else {
                    off.trim_start_matches('+')
                        .parse()
                        .map_err(|_| format!("unsupported integral key: {text}"))?
                };
                Ok((Some(i), off))
            } else {
                Ok((
                    None,
                    x.parse()
                        .map_err(|_| format!("unsupported integral key: {text}"))?,
                ))
            }
        })
        .collect()
}

pub fn key_at(pat: &Pat, point: &[i64]) -> Key {
    pat.iter()
        .map(|(ax, off)| ax.map_or(*off, |a| point[a] + off))
        .collect()
}

pub fn power_decode(b: i64) -> Result<(bool, i64), Unknown> {
    if !(0..256).contains(&b) {
        return Err("invalid Power byte".into());
    }
    let a = b & 127;
    Ok((b & 128 != 0, if a >= 64 { a - 128 } else { a }))
}

#[derive(Clone, Debug)]
pub struct Rule {
    pub raw: Value,
    pub chart: String,
    pub index: usize,
    pub fixed: Vec<Option<i64>>,
    pub equations: Vec<String>,
    pub exceptions: Vec<Vec<String>>,
    pub target: String,
    pub rhs: Vec<(String, String)>,
    pub seeds: Vec<String>,
    pub direct_hit: bool,
}

pub fn rule_from(v: &Value) -> Result<Rule, Unknown> {
    let s = |x: &Value| {
        x.as_str()
            .map(str::to_string)
            .ok_or_else(|| "malformed rule record".to_string())
    };
    Ok(Rule {
        raw: v.clone(),
        chart: s(&v["chart"])?,
        index: v["index"].as_u64().ok_or("rule index")? as usize,
        fixed: v["fixed"]
            .as_array()
            .ok_or("rule fixed")?
            .iter()
            .map(|x| x.as_i64())
            .collect(),
        equations: v
            .get("equations")
            .and_then(|e| e.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        exceptions: v["exceptions"]
            .as_array()
            .ok_or("rule exceptions")?
            .iter()
            .map(|b| {
                b.as_array()
                    .map(|ps| {
                        ps.iter()
                            .filter_map(|p| p.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default()
            })
            .collect(),
        target: s(&v["target"])?,
        rhs: v["rhs"]
            .as_array()
            .ok_or("rule rhs")?
            .iter()
            .map(|t| Ok((s(&t[0])?, s(&t[1])?)))
            .collect::<Result<_, Unknown>>()?,
        seeds: v
            .get("seeds")
            .and_then(|e| e.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        direct_hit: v
            .get("direct_hit")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

pub fn seed_at(record: &Value, point: &[i64], recenter: &[i64], n: usize) -> Result<Key, Unknown> {
    let text = record["seed"]
        .as_str()
        .ok_or("unsupported native Seed encoding")?;
    let inner = text
        .strip_prefix("Seed { integral: Integral([")
        .and_then(|s| s.strip_suffix(" }"))
        .ok_or("unsupported native Seed encoding")?;
    let (powers, shifts) = inner
        .split_once("]), shifts: [")
        .ok_or("unsupported native Seed encoding")?;
    let shifts = shifts
        .strip_suffix(']')
        .ok_or("unsupported native Seed encoding")?;
    let raw: Vec<i64> = powers
        .split(", ")
        .map(|p| {
            p.strip_prefix("Power(")
                .and_then(|x| x.strip_suffix(')'))
                .and_then(|x| x.parse().ok())
                .ok_or_else(|| "truncated Seed Power vector".to_string())
        })
        .collect::<Result<_, _>>()?;
    if raw.len() != n {
        return Err("truncated Seed Power vector".into());
    }
    let shifts: Vec<i64> = shifts
        .split(',')
        .map(|x| x.trim().parse().map_err(|_| "seed shifts".to_string()))
        .collect::<Result<_, _>>()?;
    if shifts.len() != n {
        return Err("seed shifts length".into());
    }
    let mut y = Vec::with_capacity(n);
    for (i, &x) in raw.iter().enumerate() {
        let (sym, off) = power_decode(x)?;
        if sym {
            if off != shifts[i] {
                return Err("Power vs Seed shift mismatch".into());
            }
            y.push(point[i] + off + recenter[i]);
        } else {
            if shifts[i] != 0 || recenter[i] != 0 {
                return Err("numeric seed coordinate cannot recenter".into());
            }
            y.push(off);
        }
    }
    Ok(y)
}
