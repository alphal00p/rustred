//! Streaming reader of a `table_dump.v1` node table, written from the table format and the ENGINE's loader semantics
//! (rustred-core node/mastermap.rs `Table::load` and node/bounds.rs `bound_exclusions` at f9132a56): which records
//! are rules, `dispatch` other than absent / "Partition" = skipped, the optional `domain_bounds` as extra exception
//! branches, residual records = masters. No code of the producer is used.

use std::collections::HashSet;
use std::io::{BufRead, BufReader};

use serde_json::Value;

use crate::expr::Expr;
use crate::rat::Lin;

/// Pattern argument of a rule integral: `Some(axis) + offset`, or a constant.
pub type Pat = Vec<(Option<usize>, i64)>;

pub fn parse_pat(text: &str, n: usize) -> Result<Pat, String> {
    let inner = text
        .trim()
        .strip_prefix("I(")
        .and_then(|t| t.strip_suffix(')'))
        .ok_or_else(|| format!("bad integral {text}"))?;
    let v: Pat = inner
        .split(',')
        .map(|a| {
            let a = a.trim();
            if let Some(rest) = a.strip_prefix('n') {
                let end = rest.find(['+', '-']).unwrap_or(rest.len());
                let ax: usize = rest[..end].parse().map_err(|_| format!("bad arg {a}"))?;
                let off: i64 = if end == rest.len() {
                    0
                } else {
                    rest[end..]
                        .trim_start_matches('+')
                        .parse()
                        .map_err(|_| format!("bad arg {a}"))?
                };
                if ax >= n {
                    return Err(format!("axis out of range {a}"));
                }
                Ok((Some(ax), off))
            } else {
                Ok((None, a.parse().map_err(|_| format!("bad arg {a}"))?))
            }
        })
        .collect::<Result<_, String>>()?;
    if v.len() != n {
        return Err(format!("arity {text}"));
    }
    Ok(v)
}

pub fn at(p: &Pat, point: &[i64]) -> Vec<i64> {
    p.iter()
        .map(|(a, o)| a.map_or(*o, |a| point[a] + o))
        .collect()
}

/// One literal of a case (equation or exception member).
#[derive(Clone, Debug)]
pub struct Member {
    pub text: String,
    pub expr: Expr,
    /// d-free affine form, if the literal is affine.
    pub lin: Option<Lin>,
    /// fast exact test for `c + a * n_v` (i128).
    pub fast: Option<(i128, usize, i128)>,
    pub uses_d: bool,
}

impl Member {
    pub fn new(text: &str, n: usize) -> Result<Member, String> {
        let expr = Expr::parse(text, n)?;
        let uses_d = expr.uses_d();
        let lin = if uses_d {
            None
        } else {
            Lin::from_node(&expr.root, n)
        };
        let fast = lin.as_ref().and_then(|l| l.single_var_i128());
        Ok(Member {
            text: text.to_string(),
            expr,
            lin,
            fast,
            uses_d,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Rule {
    pub pos: usize,
    pub label: Option<u64>,
    pub fixed: Vec<Option<i64>>,
    pub target: Pat,
    pub equations: Vec<Member>,
    /// record branches, then the `domain_bounds` branches (engine order)
    pub exceptions: Vec<Vec<Member>>,
    pub n_record_branches: usize,
    pub rhs_pats: Vec<Pat>,
    pub skip: bool,
    pub origin: Value,
    pub witness_loci: Option<Vec<String>>,
    pub witness_poles: Option<u64>,
    /// the record's chart (line slots true)
    pub chart: Vec<bool>,
}

#[derive(Debug)]
pub struct Table {
    pub n: usize,
    pub mask: Vec<bool>,
    pub zero: HashSet<Vec<bool>>,
    pub rules: Vec<Rule>,
    pub masters: Vec<Vec<i64>>,
    pub header: Value,
    pub has_end: bool,
    pub record_lines: usize,
}

fn eq_text(i: usize, v: i64) -> String {
    match v {
        0 => format!("n{i}"),
        v if v > 0 => format!("-{v}+n{i}"),
        v => format!("{}+n{i}", -v),
    }
}

/// `domain_bounds` -> extra one-member exception branches (semantics of the engine's node/bounds.rs, re-implemented).
pub fn bound_branches(rec: &Value) -> Result<Vec<String>, String> {
    let Some(b) = rec.get("domain_bounds").filter(|b| !b.is_null()) else {
        return Ok(vec![]);
    };
    let o = b.as_object().ok_or("domain_bounds: not an object")?;
    if let Some(k) = o.keys().find(|k| *k != "min" && *k != "max") {
        return Err(format!("domain_bounds: unknown field {k}"));
    }
    let chart: Vec<bool> = rec["chart"]
        .as_str()
        .ok_or("domain_bounds without chart")?
        .chars()
        .map(|c| c == '1')
        .collect();
    let n = chart.len();
    let list = |k: &str| -> Result<Vec<Option<i64>>, String> {
        match b.get(k) {
            None | Some(Value::Null) => Ok(vec![None; n]),
            Some(Value::Array(a)) if a.len() == n => a
                .iter()
                .map(|x| match x {
                    Value::Null => Ok(None),
                    x => x
                        .as_i64()
                        .map(Some)
                        .ok_or("bound not an integer".to_string()),
                })
                .collect(),
            _ => Err("domain_bounds: arity".into()),
        }
    };
    let (mn, mx) = (list("min")?, list("max")?);
    let fixed: Vec<Option<i64>> = rec["fixed"]
        .as_array()
        .map(|a| a.iter().map(Value::as_i64).collect())
        .unwrap_or_else(|| vec![None; n]);
    let mut out = vec![];
    for i in 0..n {
        if chart[i] && mx[i].is_some() {
            return Err(format!("domain_bounds: max on line slot {i}"));
        }
        if !chart[i] && mn[i].is_some() {
            return Err(format!("domain_bounds: min on non-line slot {i}"));
        }
        if let Some(f) = fixed.get(i).copied().flatten() {
            if mn[i].is_some_and(|m| f < m) || mx[i].is_some_and(|m| f > m) {
                out.push(eq_text(i, f));
            }
            continue;
        }
        if chart[i] {
            if let Some(m) = mn[i] {
                if m - 1 > 64 {
                    return Err("domain_bounds: min too far".into());
                }
                for v in 1..m {
                    out.push(eq_text(i, v));
                }
            }
        } else if let Some(m) = mx[i] {
            if -m > 64 {
                return Err("domain_bounds: max too far".into());
            }
            for v in (m + 1)..=0 {
                out.push(eq_text(i, v));
            }
        }
    }
    Ok(out)
}

pub fn parse_rule(v: &Value, n: usize, pos: usize) -> Result<Rule, String> {
    let fixed: Vec<Option<i64>> = v["fixed"]
        .as_array()
        .ok_or("rule without fixed")?
        .iter()
        .map(Value::as_i64)
        .collect();
    if fixed.len() != n {
        return Err("fixed arity".into());
    }
    let equations = v
        .get("equations")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|x| Member::new(x.as_str().unwrap_or(""), n))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    let mut exceptions = v["exceptions"]
        .as_array()
        .ok_or("rule without exceptions")?
        .iter()
        .map(|b| {
            b.as_array()
                .ok_or("exception conjunction")?
                .iter()
                .map(|x| Member::new(x.as_str().unwrap_or(""), n))
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, String>>()?;
    let n_record_branches = exceptions.len();
    for t in bound_branches(v)? {
        exceptions.push(vec![Member::new(&t, n)?]);
    }
    let mut rhs_pats = vec![];
    for term in v["rhs"].as_array().ok_or("rule without rhs")? {
        rhs_pats.push(parse_pat(term[0].as_str().unwrap_or(""), n)?);
    }
    let skip = !matches!(
        v.get("dispatch").and_then(Value::as_str),
        None | Some("Partition")
    );
    Ok(Rule {
        pos,
        label: v["index"].as_u64(),
        fixed,
        target: parse_pat(v["target"].as_str().unwrap_or(""), n)?,
        equations,
        exceptions,
        n_record_branches,
        rhs_pats,
        skip,
        origin: v.get("origin").cloned().unwrap_or(Value::Null),
        witness_loci: v.get("witness_loci").and_then(Value::as_array).map(|a| {
            a.iter()
                .map(|x| x.as_str().unwrap_or("").to_string())
                .collect()
        }),
        witness_poles: v.get("witness_poles").and_then(Value::as_u64),
        chart: v["chart"]
            .as_str()
            .ok_or("rule without chart")?
            .chars()
            .map(|c| c == '1')
            .collect(),
    })
}

/// Stream every non-empty line: (line number, raw bytes without newline, parsed JSON).
pub fn for_each_line(
    path: &str,
    mut f: impl FnMut(usize, &str, Value) -> Result<(), String>,
) -> Result<(), String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{path}: {e}"))?;
    let mut r = BufReader::with_capacity(1 << 24, file);
    let mut line = String::new();
    let mut no = 0usize;
    loop {
        line.clear();
        let k = r.read_line(&mut line).map_err(|e| format!("{path}: {e}"))?;
        if k == 0 {
            break;
        }
        let t = line.strip_suffix('\n').unwrap_or(&line);
        if !t.trim().is_empty() {
            let v: Value = serde_json::from_str(t).map_err(|e| format!("{path} line {no}: {e}"))?;
            f(no, t, v)?;
        }
        no += 1;
    }
    Ok(())
}

impl Table {
    /// Load the dispatch skeleton (no coefficient texts).
    pub fn load(path: &str) -> Result<Table, String> {
        let mut t = Table {
            n: 0,
            mask: vec![],
            zero: HashSet::new(),
            rules: vec![],
            masters: vec![],
            header: Value::Null,
            has_end: false,
            record_lines: 0,
        };
        for_each_line(path, |_, _, v| t.absorb(v))?;
        if t.n == 0 {
            return Err("no header".into());
        }
        Ok(t)
    }

    pub fn absorb(&mut self, v: Value) -> Result<(), String> {
        self.record_lines += 1;
        match v["type"].as_str().unwrap_or("") {
            "header" => {
                self.n = v["denominators"]
                    .as_u64()
                    .ok_or("header without denominators")? as usize;
                self.mask = v["mask"]
                    .as_str()
                    .ok_or("header without mask")?
                    .chars()
                    .map(|c| c == '1')
                    .collect();
                if v["power_shifts"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|x| x.as_str() != Some("0")))
                {
                    return Err("eps family not supported".into());
                }
                let mut h = v;
                if let Some(o) = h.as_object_mut() {
                    o.remove("family_source");
                    o.remove("original_family_source");
                }
                self.header = h;
            }
            "zero_sectors" => {
                for m in v["masks"].as_array().ok_or("zero_sectors without masks")? {
                    self.zero
                        .insert(m.as_str().unwrap_or("").chars().map(|c| c == '1').collect());
                }
            }
            "chart" => {
                let c: Vec<bool> = v["chart"]
                    .as_str()
                    .unwrap_or("")
                    .chars()
                    .map(|c| c == '1')
                    .collect();
                if c != self.mask {
                    return Err("chart other than the node mask".into());
                }
                let g: Vec<&str> = v["guards"]
                    .as_array()
                    .map(|a| a.iter().filter_map(Value::as_str).collect())
                    .unwrap_or_default();
                if g.iter()
                    .enumerate()
                    .any(|(i, g)| (self.mask[i] && *g != "n>=1") || (!self.mask[i] && *g != "n<=0"))
                {
                    return Err("unsupported chart guards".into());
                }
            }
            "rule" => {
                let pos = self.rules.len();
                let r = parse_rule(&v, self.n, pos)?;
                self.rules.push(r);
            }
            "sym" | "sym_zero" => return Err("sym records are not handled by this verifier".into()),
            "residual" => {
                let p = parse_pat(v["key"].as_str().unwrap_or(""), self.n)?;
                self.masters.push(at(&p, &[]));
            }
            "end" => self.has_end = true,
            _ => {}
        }
        Ok(())
    }

    /// Node order data of the header: (support priority, coordinate priority).
    pub fn order_data(&self) -> Result<(Vec<usize>, Vec<usize>), String> {
        let o = &self.header["order"];
        let sp: Vec<usize> = o["support_priority"]
            .as_array()
            .ok_or("header order without support_priority")?
            .iter()
            .map(|x| x.as_u64().map(|x| x as usize).ok_or("bad priority"))
            .collect::<Result<_, _>>()?;
        let cp: Vec<usize> = o["coordinate_priority"]
            .as_array()
            .ok_or("header order without coordinate_priority")?
            .iter()
            .map(|x| x.as_u64().map(|x| x as usize).ok_or("bad priority"))
            .collect::<Result<_, _>>()?;
        if o["two_sided"].as_array().is_some_and(|a| !a.is_empty()) {
            return Err("two-sided order axes are not handled".into());
        }
        let np = &self.header["node_order"]["coordinate_priority"];
        if !np.is_null() && np.as_array().map(|a| a.len()) != Some(cp.len()) {
            return Err("node_order priority differs from order priority".into());
        }
        if let Some(a) = np.as_array() {
            for (x, y) in a.iter().zip(&cp) {
                if x.as_u64() != Some(*y as u64) {
                    return Err("node_order priority differs from order priority".into());
                }
            }
        }
        if !self.header["node_order"]["slot_permutation"].is_null() {
            return Err("slot permutations are not handled".into());
        }
        Ok((sp, cp))
    }
}

/// A rule's raw integral-pattern identity check: target slot j is `n_j` (offset 0) or the constant of a fixed slot j.
pub fn target_is_canonical(r: &Rule) -> bool {
    r.target.iter().enumerate().all(|(j, (a, o))| match a {
        Some(a) => *a == j && *o == 0 && r.fixed[j].is_none(),
        None => r.fixed[j] == Some(*o),
    })
}
