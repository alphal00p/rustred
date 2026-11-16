//! The NODE REPORT of the composed profiles (interfaces/epsdepth.md 5, 9): verdicts per (box key, terminal), per
//! terminal the maximal composed pole order ("spurious coefficients needed of this terminal"), the keys responsible, the
//! blamed relations, and (with the user's eps box) the required eps depth against the box.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::compose::{Composition, KeyResult, Order, PairResult};
use super::epsbox::BoxStatus;
use super::spec::{EpsDepthSpec, TerminalSetStatus};

/// Schema of the report header.
pub const REPORT_SCHEMA: &str = "rustred.campaign.epsdepth-report.v1";
/// Schema of a row (one selected box key).
pub const ROW_SCHEMA: &str = "rustred.campaign.epsdepth-row.v1";

/// The verdict of one (box key, terminal) pair (TRICKS 16 and the implementer's ruling of 2026-11-15).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PairVerdict {
    /// p = 0 and no local pole in the key's cone.
    Regular,
    /// p = 0 although local poles occur in the cone: they cancel.
    Spurious,
    /// p > 0 on a terminal set CERTIFIED regular at d0 (T = R, or the vector re-expressed on R by lane regbasis).
    Inherent(u32),
    /// p > 0 on the closure's terminals T, no regular terminal set R known (POLES_OBJECTIVE 2B: T can be degenerate at
    /// d0; a pole on T is not a defect of a relation): never called inherent.
    OnTOnly(u32),
    Undecided(String),
}

impl PairVerdict {
    pub fn word(&self) -> String {
        match self {
            PairVerdict::Regular => "regular".into(),
            PairVerdict::Spurious => "spurious".into(),
            PairVerdict::Inherent(p) => format!("inherent({p})"),
            PairVerdict::OnTOnly(p) => format!("on_t_only({p})"),
            PairVerdict::Undecided(r) => format!("undecided({r})"),
        }
    }
    fn class(&self) -> &'static str {
        match self {
            PairVerdict::Regular => "regular",
            PairVerdict::Spurious => "spurious",
            PairVerdict::Inherent(_) => "inherent",
            PairVerdict::OnTOnly(_) => "on_t_only",
            PairVerdict::Undecided(_) => "undecided",
        }
    }
}

/// The decided pole order of a pair (`Zero` counts as order 0), `None` when undecided.
pub fn decided_order(p: &PairResult) -> Option<u32> {
    match &p.order {
        Order::Zero => Some(0),
        Order::Pole(o) => Some(*o),
        Order::Undecided(_) => None,
    }
}

/// The verdict of a pair.
pub fn verdict(p: &PairResult, local_pole_in_cone: bool, set: TerminalSetStatus) -> PairVerdict {
    match &p.order {
        Order::Undecided(r) => PairVerdict::Undecided(r.clone()),
        Order::Zero | Order::Pole(0) => {
            if local_pole_in_cone {
                PairVerdict::Spurious
            } else {
                PairVerdict::Regular
            }
        }
        Order::Pole(o) => match set {
            TerminalSetStatus::CertifiedRegular => PairVerdict::Inherent(*o),
            _ => PairVerdict::OnTOnly(*o),
        },
    }
}

#[derive(Default)]
struct TermAgg {
    max_order: Option<u32>,
    histogram: BTreeMap<u32, u64>,
    zero: u64,
    argmax: Vec<(u32, Vec<i32>)>,
    bound_max: Option<i64>,
    verdicts: BTreeMap<&'static str, u64>,
    blame: BTreeMap<String, (u64, u32)>,
    max_required: Option<i64>,
    setting: u64,
    exceeding: u64,
    max_excess: i64,
}

/// Build the report header and one row per selected key. `terminals` = the terminal keys (component order), `key_of`
/// maps a source key index to its key, `lines` = the node's line mask (dots / rank of rows), `node` = a free name.
pub fn node_report(
    comp: &Composition,
    spec: &EpsDepthSpec,
    terminals: &[Vec<i32>],
    key_of: &dyn Fn(usize) -> Vec<i32>,
    lines: &[bool],
    node: &str,
) -> Result<(Value, Vec<Value>), String> {
    if terminals.len() != comp.terminals {
        return Err(format!(
            "{} terminal keys for {} components",
            terminals.len(),
            comp.terminals
        ));
    }
    if let Some(b) = &spec.eps_box {
        b.covers(terminals)?;
    }
    let nt = terminals.len();
    let mut aggs: Vec<TermAgg> = (0..nt).map(|_| TermAgg::default()).collect();
    let mut totals: BTreeMap<&'static str, u64> = BTreeMap::new();
    let mut rows = Vec::with_capacity(comp.results.len());
    for r in &comp.results {
        let key = key_of(r.key);
        rows.push(row(
            r,
            &key,
            spec,
            terminals,
            lines,
            &mut aggs,
            &mut totals,
        )?);
    }
    let per_terminal: Vec<Value> = aggs
        .into_iter()
        .zip(terminals)
        .map(|(mut a, t)| {
            a.argmax.sort_by(|x, y| y.0.cmp(&x.0).then_with(|| x.1.cmp(&y.1)));
            a.argmax.truncate(spec.top_keys);
            let mut blame: Vec<(String, (u64, u32))> = a.blame.into_iter().collect();
            blame.sort_by(|x, y| y.1.0.cmp(&x.1.0).then_with(|| y.1.1.cmp(&x.1.1)).then_with(|| x.0.cmp(&y.0)));
            let n_blamed = blame.len();
            blame.truncate(spec.top_blame);
            let eps = spec.eps_box.as_ref().map(|b| {
                json!({"available": b.available(t).ok(), "needed_order": b.needed_order(),
                       "max_required": a.max_required, "keys_setting_it": a.setting,
                       "exceeding": a.exceeding, "max_excess": a.max_excess})
            });
            json!({"terminal": t, "max_order": a.max_order,
                   "spurious_coefficients_needed": a.max_order,
                   "histogram": a.histogram.iter().map(|(k, v)| (k.to_string(), json!(v))).collect::<serde_json::Map<_, _>>(),
                   "exact_zero": a.zero,
                   "argmax": a.argmax.iter().map(|(o, k)| json!({"key": k, "order": o})).collect::<Vec<_>>(),
                   "bound_max": a.bound_max, "verdicts": a.verdicts,
                   "blame": blame.iter().map(|(s, (n, o))| json!({"server": s, "pairs": n, "max_order": o})).collect::<Vec<_>>(),
                   "blamed_relations": n_blamed, "eps_box": eps})
        })
        .collect();
    let st = &comp.stats;
    let header = json!({
        "schema": REPORT_SCHEMA, "node": node, "d0": spec.d0, "target_exponent": spec.target_exponent,
        "terminal_set": {"keys": terminals, "status": spec.terminal_set.word()},
        "spec": spec.to_json(),
        "keys": {"selected": st.selected},
        "cone": {"keys": st.cone_keys, "edges": st.edges, "singular_edges": st.singular_edges,
                 "singular_relations": st.singular_relations, "max_edge_pole": st.max_edge_pole,
                 "max_height": st.max_height, "max_required": st.max_required, "capped_keys": st.capped_keys,
                 "truncated_keys": st.truncated_keys, "open_keys": st.open_keys, "failed_keys": st.failed_keys,
                 "expansions": st.expansions},
        "time_decided": st.time_decided,
        "seconds": {"discover": st.seconds_discover, "compose": st.seconds_compose},
        "verdicts": totals,
        "per_terminal": per_terminal,
        "profile_used": "composed",
    });
    Ok((header, rows))
}

fn row(
    r: &KeyResult,
    key: &[i32],
    spec: &EpsDepthSpec,
    terminals: &[Vec<i32>],
    lines: &[bool],
    aggs: &mut [TermAgg],
    totals: &mut BTreeMap<&'static str, u64>,
) -> Result<Value, String> {
    let (mut dots, mut rank) = (0i64, 0i64);
    for (&v, &l) in key.iter().zip(lines) {
        if l {
            dots += i64::from(v) - 1;
        } else {
            rank += i64::from(v).abs();
        }
    }
    let mut orders = vec![];
    let mut bounds = vec![];
    let mut verdicts = vec![];
    let mut blames = vec![];
    let mut eps = vec![];
    let mut leading = vec![];
    for (j, p) in r.pairs.iter().enumerate() {
        let v = verdict(p, r.local_pole_in_cone, spec.terminal_set);
        let a = &mut aggs[j];
        *a.verdicts.entry(v.class()).or_default() += 1;
        *totals.entry(v.class()).or_default() += 1;
        let o = decided_order(p);
        if let Some(o) = o {
            *a.histogram.entry(o).or_default() += 1;
            if matches!(p.order, Order::Zero) {
                a.zero += 1;
            }
            match a.max_order {
                Some(m) if m > o => {}
                Some(m) if m == o => a.argmax.push((o, key.to_vec())),
                _ => {
                    a.max_order = Some(o);
                    a.argmax = vec![(o, key.to_vec())];
                }
            }
            if o > 0
                && let Some(s) = &p.blame_server
            {
                let e = a.blame.entry(s.clone()).or_insert((0, 0));
                e.0 += 1;
                e.1 = e.1.max(o);
            }
        }
        if let Some(b) = p.bound {
            a.bound_max = Some(a.bound_max.map_or(b, |m| m.max(b)));
        }
        let e = match (&spec.eps_box, o) {
            (Some(bx), Some(o)) => {
                let req = bx.required(o);
                match a.max_required {
                    Some(m) if m > req => {}
                    Some(m) if m == req => a.setting += 1,
                    _ => {
                        a.max_required = Some(req);
                        a.setting = 1;
                    }
                }
                let s = bx.status(&terminals[j], o)?;
                if let BoxStatus::Exceeds(k) = s {
                    a.exceeding += 1;
                    a.max_excess = a.max_excess.max(k);
                }
                json!(s.word())
            }
            (Some(_), None) => json!("undecided"),
            (None, _) => Value::Null,
        };
        eps.push(e);
        orders.push(o.map_or(Value::Null, |o| json!(o)));
        bounds.push(p.bound.map_or(Value::Null, |b| json!(b)));
        verdicts.push(v.word());
        blames.push(p.blame_server.clone().map_or(Value::Null, Value::String));
        leading.push(p.leading.clone().map_or(Value::Null, Value::String));
    }
    Ok(
        json!({"schema": ROW_SCHEMA, "key": key, "D": dots, "r": rank, "orders": orders, "bounds": bounds,
              "verdicts": verdicts, "blame": blames, "eps": eps, "leading": leading,
              "local": {"pole_order": r.local_pole, "singular_terms": r.local_singular,
                        "verdict": if r.local_pole == 0 { "regular_here".to_string() } else { format!("singular_here({})", r.local_pole) },
                        "pole_in_cone": r.local_pole_in_cone}}),
    )
}
