//! THE CONE-PRICING ACCEPTANCE of a replacement (emission contract section 17; round 6 J4; design of lane rulecost 2026-11-19T13:25Z,
//! agreed by the coordinator). BINDING ORDER: the d = 4 regularity guard (section 13) comes first; a candidate worse at d = 4 is
//! refused before it is priced; a relation regular at d = 4 wins whatever its size.
//!
//! PURPOSE: refuse or rank a candidate by the WORK IT HANDS DOWN on the real request, counted on merged receiver keys, not by its
//! bytes. An A/B partial record pass of the same request (old arm: the old table / input; new arm: the candidate), each stopped
//! after a saved phase, writes the engine's owner statistics (`RUSTRED_P_OWNER_STATS`: per phase, per owner class: steps, terms,
//! dests = children per receiving owner, shapes (lines, dots, rank, keys)). Read TWO FULL LEVELS from the entrance (the Route
//! phase where the replaced relation's children first arrive): Route, Apply, Route, Apply. The verdict is read at the SECOND
//! Apply:
//! - NOT WORSE iff the candidate's children there are <= the old ones AND no receiving owner grows by more than a factor
//!   (default 1.25; owners with fewer than `min_children` old children are compared only if the candidate gives them at least
//!   that many);
//! - an ENTRANCE-ONLY WIN is REFUSED ('absorbed'): the candidate wins at the entrance (by more than the tolerance) but not at
//!   the second Apply (gain there below the tolerance, default 4 %);
//! - a third level is reported when the arms carry it and the second-level difference is still moving (above the tolerance).
//! RANKING among accepted candidates: children at the second Apply, then the high-shell population (keys stepped at the second
//! Apply whose D + r exceeds the request's), then the arms' seconds through the second Apply. PURE: parses and decides; the
//! process part (rr_campaign cone-price --run) runs the engine.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const SCHEMA: &str = "rustred.emission.cone-price.v1";

/// One phase of one arm, summed over owner groups.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PhaseSum {
    pub phase: i64,
    /// keys stepped in the phase (Route: keys routed; Apply: keys applied = live keys)
    pub steps: u64,
    /// children handed down (sum of the groups' dests)
    pub children: u64,
    pub seconds: f64,
    /// lines of the keys stepped (max over the groups' shapes)
    pub lines: Option<i64>,
    /// keys stepped whose D + r exceeds the request's (from the groups' shapes)
    pub high_shell: u64,
    /// children per receiving owner (dest label -> children)
    pub per_owner: BTreeMap<String, u64>,
    /// the SHELL HISTOGRAM of the keys stepped: "lines/dots/rank" -> keys (from the groups' shapes; tagged keys as the engine
    /// steps them)
    pub shells: BTreeMap<String, u64>,
    /// per owner class stepping in the phase (at an Apply: the RECEIVING owner): keys stepped, seconds, children handed down
    pub per_class: BTreeMap<String, ClassSum>,
}

/// One owner class in one phase.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ClassSum {
    pub keys: u64,
    pub seconds: f64,
    pub children: u64,
}

/// Owner statistics of one arm, by phase.
#[derive(Clone, Debug, Default)]
pub struct Arm {
    pub phases: BTreeMap<i64, PhaseSum>,
    /// D + r of the request (max over the phase-0 shapes)
    pub request_dr: i64,
}

/// Parse an owner-statistics file (`{"groups": [{phase, class, steps, seconds, terms, shapes: [[lines, dots, rank, keys]], dests:
/// [[label, children]]}], "phases": [[phase, seconds, ...]]}`). `request_dr` = D + r of the request (None: from phase 0).
pub fn parse_arm(text: &str, request_dr: Option<i64>) -> Result<Arm, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("owner stats: {e}"))?;
    let groups = v["groups"].as_array().ok_or("owner stats without groups")?;
    let dr0 = groups
        .iter()
        .filter(|g| g["phase"].as_i64() == Some(0))
        .flat_map(|g| g["shapes"].as_array().cloned().unwrap_or_default())
        .filter_map(|s| Some(s[1].as_i64()? + s[2].as_i64()?))
        .max()
        .unwrap_or(0);
    let request_dr = request_dr.unwrap_or(dr0);
    let mut phases: BTreeMap<i64, PhaseSum> = BTreeMap::new();
    for g in groups {
        let ph = g["phase"].as_i64().ok_or("group without phase")?;
        let p = phases.entry(ph).or_insert_with(|| PhaseSum { phase: ph, ..Default::default() });
        let (steps, secs) = (g["steps"].as_u64().unwrap_or(0), g["seconds"].as_f64().unwrap_or(0.0));
        p.steps += steps;
        p.seconds += secs;
        for s in g["shapes"].as_array().into_iter().flatten() {
            let (l, d, r, k) = (s[0].as_i64(), s[1].as_i64().unwrap_or(0), s[2].as_i64().unwrap_or(0), s[3].as_u64().unwrap_or(0));
            p.lines = p.lines.max(l);
            if d + r > request_dr {
                p.high_shell += k;
            }
            *p.shells.entry(format!("{}/{d}/{r}", l.unwrap_or(0))).or_default() += k;
        }
        let mut gchildren = 0u64;
        for d in g["dests"].as_array().into_iter().flatten() {
            let (label, n) = (d[0].as_str().unwrap_or("?").to_string(), d[1].as_u64().unwrap_or(0));
            p.children += n;
            gchildren += n;
            *p.per_owner.entry(label).or_default() += n;
        }
        let c = p.per_class.entry(g["class"].as_str().unwrap_or("?").to_string()).or_default();
        c.keys += steps;
        c.seconds += secs;
        c.children += gchildren;
    }
    Ok(Arm { phases, request_dr })
}

/// The decision's knobs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rules {
    /// relative tolerance of "equal" (entrance win, second-level gain, still moving)
    pub tolerance: f64,
    /// no receiving owner may grow by more than this factor at the second Apply
    pub owner_growth: f64,
    /// owners below this many old children are compared only if the candidate gives them at least that many
    pub min_children: u64,
}

impl Default for Rules {
    fn default() -> Self {
        Rules { tolerance: 0.04, owner_growth: 1.25, min_children: 100 }
    }
}

/// The verdict (side file `cone/<sha256>.json`; a replacement record names it in `replaces.cone_sha256`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Verdict {
    pub schema: String,
    pub old_body_sha256: Option<String>,
    pub new_body_sha256: Vec<String>,
    pub request: Value,
    pub entrance_phase: i64,
    pub phases_read: Vec<i64>,
    /// per phase read: [old, new] steps / children / high shell / seconds
    pub levels: Vec<Value>,
    pub entrance_ratio_old_over_new: Option<f64>,
    pub second_apply_ratio_old_over_new: Option<f64>,
    pub owners_grown: Vec<Value>,
    /// THE SHELL HISTOGRAM AT THE SECOND APPLY (coordinator after LIT-AUDIT-SZ-CL, 2026-11-19): keys per (lines, dots, rank), old
    /// vs new, with totals by rank and by dots, mean dots / rank and keys weighted x4 per rank step (single-key model): a candidate
    /// that lowers dots but raises rank downstream (a no-raise relation, Smith-Zeng 2.3) shows here
    #[serde(default)]
    pub second_apply_shells: Value,
    /// keys, seconds and children per RECEIVING owner at the second Apply, old vs new
    #[serde(default)]
    pub second_apply_owners: Vec<Value>,
    /// what the partial pass compares: work, not values
    #[serde(default)]
    pub values_note: String,
    pub third_level: Option<Value>,
    pub rules: Rules,
    pub accepted: bool,
    pub reason: Option<String>,
    pub rank_key: Option<Vec<f64>>,
}

fn ratio(a: u64, b: u64) -> Option<f64> {
    (b > 0).then(|| a as f64 / b as f64)
}

/// Price a candidate: `old` / `new` arms, `entrance` = the Route phase where the replaced relation's children first arrive.
pub fn price(old: &Arm, new: &Arm, entrance: i64, rules: &Rules, request: Value, old_body: Option<String>, new_bodies: Vec<String>) -> Result<Verdict, String> {
    if entrance % 2 != 0 {
        return Err(format!("the entrance phase {entrance} is not a Route phase (Route = even phase: 2 (14 - lines))"));
    }
    let read = [entrance, entrance + 1, entrance + 2, entrance + 3];
    for p in read {
        if !old.phases.contains_key(&p) || !new.phases.contains_key(&p) {
            return Err(format!("phase {p} missing in an arm (stop after phase {} at least)", entrance + 3));
        }
    }
    let lvl = |p: i64| {
        let (o, n) = (&old.phases[&p], &new.phases[&p]);
        json!({"phase": p, "stage": if p % 2 == 0 { "Route" } else { "Apply" }, "lines": o.lines.or(n.lines),
               "steps": [o.steps, n.steps], "children": [o.children, n.children], "high_shell": [o.high_shell, n.high_shell],
               "seconds": [o.seconds, n.seconds]})
    };
    let levels: Vec<Value> = read.iter().map(|&p| lvl(p)).collect();
    let (oe, ne) = (&old.phases[&entrance], &new.phases[&entrance]);
    let (oa, na) = (&old.phases[&(entrance + 3)], &new.phases[&(entrance + 3)]);
    let entrance_ratio = ratio(oe.steps, ne.steps);
    let second_ratio = ratio(oa.children, na.children);
    let t = rules.tolerance;
    // the shell histogram at the second Apply
    let shells = {
        let keys: std::collections::BTreeSet<&String> = oa.shells.keys().chain(na.shells.keys()).collect();
        let mut rows = vec![];
        let (mut by_rank, mut by_dots): (BTreeMap<i64, [u64; 2]>, BTreeMap<i64, [u64; 2]>) = Default::default();
        let (mut w, mut sd, mut sr) = ([0f64; 2], [0f64; 2], [0f64; 2]);
        for k in keys {
            let p: Vec<i64> = k.split('/').map(|x| x.parse().unwrap_or(0)).collect();
            let (o, n) = (oa.shells.get(k).copied().unwrap_or(0), na.shells.get(k).copied().unwrap_or(0));
            rows.push(json!({"lines": p[0], "dots": p[1], "rank": p[2], "old": o, "new": n}));
            for (i, x) in [o, n].into_iter().enumerate() {
                by_rank.entry(p[2]).or_default()[i] += x;
                by_dots.entry(p[1]).or_default()[i] += x;
                w[i] += x as f64 * 4f64.powi(p[2] as i32);
                sd[i] += (x * p[1] as u64) as f64;
                sr[i] += (x * p[2] as u64) as f64;
            }
        }
        let tot = [oa.shells.values().sum::<u64>() as f64, na.shells.values().sum::<u64>() as f64];
        let mean = |s: [f64; 2]| json!([if tot[0] > 0.0 { s[0] / tot[0] } else { 0.0 }, if tot[1] > 0.0 { s[1] / tot[1] } else { 0.0 }]);
        json!({"phase": entrance + 3, "unit": "tagged keys stepped at the second Apply, per (lines, dots, rank); [old, new]",
               "rows": rows, "by_rank": by_rank, "by_dots": by_dots, "mean_dots": mean(sd), "mean_rank": mean(sr),
               "keys_weighted_x4_per_rank": w,
               "weight_note": "x4 per rank step: the single-key model (FEASIBILITY-5L S2); information, not a refusal rule"})
    };
    let owners_cmp: Vec<Value> = {
        let cls: std::collections::BTreeSet<&String> = oa.per_class.keys().chain(na.per_class.keys()).collect();
        cls.into_iter()
            .map(|c| {
                let (a, b) = (oa.per_class.get(c).cloned().unwrap_or_default(), na.per_class.get(c).cloned().unwrap_or_default());
                json!({"owner": c, "keys": [a.keys, b.keys], "seconds": [a.seconds, b.seconds], "children": [a.children, b.children]})
            })
            .collect()
    };
    // per receiving owner at the second Apply: keys received (per owner class) and children handed down (per destination)
    let mut grown = vec![];
    for (c, b) in &na.per_class {
        let a = oa.per_class.get(c).map_or(0, |x| x.keys);
        if b.keys >= rules.min_children && (b.keys as f64) > rules.owner_growth * (a.max(1) as f64) {
            grown.push(json!({"owner": c, "by": "keys received", "old": a, "new": b.keys,
                              "factor": if a > 0 { json!(b.keys as f64 / a as f64) } else { Value::Null }}));
        }
    }
    let owners: std::collections::BTreeSet<&String> = oa.per_owner.keys().chain(na.per_owner.keys()).collect();
    for o in owners {
        let (a, b) = (oa.per_owner.get(o).copied().unwrap_or(0), na.per_owner.get(o).copied().unwrap_or(0));
        if a < rules.min_children && b < rules.min_children {
            continue;
        }
        if (b as f64) > rules.owner_growth * (a.max(1) as f64) && b >= rules.min_children {
            grown.push(json!({"owner": o, "by": "children handed down", "old": a, "new": b,
                              "factor": if a > 0 { json!(b as f64 / a as f64) } else { Value::Null }}));
        }
    }
    let worse = na.children > oa.children;
    let entrance_win = entrance_ratio.is_some_and(|r| r > 1.0 + t);
    let second_gain = second_ratio.unwrap_or(1.0);
    let absorbed = entrance_win && second_gain < 1.0 + t;
    let third = (second_gain - 1.0).abs() > t && old.phases.contains_key(&(entrance + 5)) && new.phases.contains_key(&(entrance + 5));
    let third_level = third.then(|| lvl(entrance + 5));
    let reason = if worse {
        Some(format!(
            "cone_worse: {} children at the second Apply (phase {}) vs {} today ({:+.2} %)",
            na.children, entrance + 3, oa.children, 100.0 * (na.children as f64 / oa.children.max(1) as f64 - 1.0)
        ))
    } else if !grown.is_empty() {
        Some(format!("cone_owner_grows: {} receiving owner(s) grow by more than {}x at the second Apply", grown.len(), rules.owner_growth))
    } else if absorbed {
        Some(format!(
            "cone_absorbed: entrance-only win ({:.2}x at phase {entrance}, {:.4}x at the second Apply, below 1 + {t})",
            entrance_ratio.unwrap_or(0.0), second_gain
        ))
    } else {
        None
    };
    let accepted = reason.is_none();
    Ok(Verdict {
        schema: SCHEMA.into(),
        old_body_sha256: old_body,
        new_body_sha256: new_bodies,
        request,
        entrance_phase: entrance,
        phases_read: read.to_vec(),
        levels,
        entrance_ratio_old_over_new: entrance_ratio,
        second_apply_ratio_old_over_new: second_ratio,
        owners_grown: grown,
        second_apply_shells: shells,
        second_apply_owners: owners_cmp,
        values_note: "the partial pass prices WORK (keys, children, shells, seconds) and stops before the terminals: it compares no \
                      values. The candidate's exactness is its derivation's check; a full A/B must compare tagged terminal VALUES, \
                      not terminal expansions (different rule choices may give different, equal-valued expansions: CL Theorem 2.4)"
            .into(),
        third_level,
        rules: rules.clone(),
        accepted,
        reason,
        rank_key: accepted.then(|| {
            let secs: f64 = read.iter().map(|p| new.phases[p].seconds).sum();
            vec![na.children as f64, na.high_shell as f64, secs]
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arm(rows: &[(i64, u64, &[(&str, u64)])]) -> Arm {
        let groups: Vec<Value> = rows
            .iter()
            .map(|(p, steps, dests)| {
                json!({"phase": p, "class": "c", "steps": steps, "seconds": 1.0, "terms": 0, "shapes": [[12, 1, 0, steps]],
                       "dests": dests.iter().map(|(l, n)| json!([l, n])).collect::<Vec<_>>()})
            })
            .collect();
        parse_arm(&json!({"groups": groups}).to_string(), Some(1)).unwrap()
    }

    #[test]
    fn worse_absorbed_grown_and_accepted() {
        let old = arm(&[(4, 327, &[("a", 1039)]), (5, 579, &[("a", 20000), ("b", 3780)]), (6, 14300, &[("a", 57429)]),
                        (7, 27819, &[("a", 800000), ("b", 105807)])]);
        // the T269 shape: entrance 327 -> 12, the second Apply 1.8 % worse
        let glued = arm(&[(4, 12, &[("a", 12)]), (5, 231, &[("a", 20000), ("b", 3886)]), (6, 14222, &[("a", 63271)]),
                          (7, 28859, &[("a", 810000), ("b", 112379)])]);
        let v = price(&old, &glued, 4, &Rules::default(), json!({}), None, vec![]).unwrap();
        assert!(!v.accepted && v.reason.as_deref().unwrap().starts_with("cone_worse"), "{v:?}");
        // equal within the tolerance at the second Apply, entrance win: absorbed
        let eq = arm(&[(4, 12, &[("a", 12)]), (5, 231, &[("a", 23780)]), (6, 14222, &[("a", 57000)]), (7, 28000, &[("a", 800000), ("b", 100000)])]);
        let v = price(&old, &eq, 4, &Rules::default(), json!({}), None, vec![]).unwrap();
        assert!(!v.accepted && v.reason.as_deref().unwrap().starts_with("cone_absorbed"), "{v:?}");
        // a real gain two levels down: accepted
        let good = arm(&[(4, 12, &[("a", 12)]), (5, 231, &[("a", 20000)]), (6, 9000, &[("a", 40000)]), (7, 20000, &[("a", 600000), ("b", 90000)])]);
        let v = price(&old, &good, 4, &Rules::default(), json!({}), None, vec![]).unwrap();
        assert!(v.accepted, "{v:?}");
        // the shell histogram at the second Apply: every key in a (lines, dots, rank) row, old vs new
        assert_eq!(v.second_apply_shells["rows"], json!([{"lines": 12, "dots": 1, "rank": 0, "old": 27819, "new": 20000}]));
        assert_eq!(v.second_apply_shells["by_rank"]["0"], json!([27819, 20000]));
        assert_eq!(v.second_apply_owners[0]["keys"], json!([27819, 20000]));
        // fewer children overall but one receiving owner grows 1.5x: refused
        let grow = arm(&[(4, 300, &[("a", 1000)]), (5, 500, &[("a", 20000)]), (6, 14000, &[("a", 50000)]), (7, 27000, &[("a", 700000), ("b", 158000)])]);
        let v = price(&old, &grow, 4, &Rules::default(), json!({}), None, vec![]).unwrap();
        assert!(!v.accepted && v.reason.as_deref().unwrap().starts_with("cone_owner_grows"), "{v:?}");
        // an Apply phase as the entrance, or a missing phase: an error
        assert!(price(&old, &good, 5, &Rules::default(), json!({}), None, vec![]).is_err());
        assert!(price(&old, &good, 6, &Rules::default(), json!({}), None, vec![]).is_err());
    }
}
