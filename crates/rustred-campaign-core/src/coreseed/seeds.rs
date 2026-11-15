//! Explicit seed lists of the core rules (interfaces/coreseed.md 2-3): generic centres resolved per key, shells
//! `S<p>m<q>` in the engine's HARDER unit (+1 on a line, -1 on a numerator; a lowered slot moves the other way) over ALL
//! slots, no floor; the list is written as OFFSETS FROM THE KEY (one centre per seed, shell S0) so that one recipe serves
//! every key of a face (the engine's sector filter drops a seed outside the chart's top sector at each key). Generic in
//! arity and chart (L8).

use std::collections::BTreeSet;

use serde_json::{Value, json};

use super::spec::CentreSpec;
use crate::termmin::steps::parse_shell;

pub type Offset = Vec<i32>;

/// The support signature of a key: (dotted lines = lines at power >= 2, nonzero numerators).
pub fn signature(key: &[i32], lines: &[bool]) -> (Vec<usize>, Vec<usize>) {
    let dotted = (0..key.len()).filter(|&i| lines[i] && key[i] >= 2).collect();
    let nums = (0..key.len()).filter(|&i| !lines[i] && key[i] < 0).collect();
    (dotted, nums)
}

/// Dots + rank of a key.
pub fn dots_rank(key: &[i32], lines: &[bool]) -> i64 {
    key.iter()
        .zip(lines)
        .map(|(&v, &l)| if l { (v - 1).max(0) as i64 } else { (-v).max(0) as i64 })
        .sum()
}

/// One harder step on slot `i` (+1 on a line, -1 on a numerator).
pub fn harder(lines: &[bool], i: usize) -> i32 {
    if lines[i] { 1 } else { -1 }
}

/// The seed `key + off` lies in the chart's top sector (every line >= 1, every numerator <= 0).
pub fn valid_at(key: &[i32], off: &[i32], lines: &[bool]) -> bool {
    key.iter()
        .zip(off)
        .zip(lines)
        .all(|((&k, &o), &l)| if l { k + o >= 1 } else { k + o <= 0 })
}

/// The centres of `specs` at `key`, as (label, offset) in spec order (first occurrence wins). `lowest[s]` = the
/// minimum of slot `s` over the core keys (for `line_up_at_lowest`).
pub fn resolve_centres(
    specs: &[CentreSpec],
    key: &[i32],
    lines: &[bool],
    lowest: &[i32],
) -> Vec<(String, Offset)> {
    let n = key.len();
    let (dotted, nums) = signature(key, lines);
    let mut out: Vec<(String, Offset)> = vec![];
    let mut push = |label: String, o: Offset| {
        if !out.iter().any(|(_, x)| *x == o) {
            out.push((label, o));
        }
    };
    let unit = |i: usize, s: i32| -> Offset {
        let mut o = vec![0; n];
        o[i] = s;
        o
    };
    for c in specs {
        match c {
            CentreSpec::Offset(v) if v.len() == n => push(format!("offset{v:?}"), v.clone()),
            CentreSpec::Offset(_) => {}
            CentreSpec::Token(t) => match t.as_str() {
                "key" => push("key".into(), vec![0; n]),
                "dotted_line_down" => {
                    for &i in &dotted {
                        push(format!("line{i}_down"), unit(i, -harder(lines, i)));
                    }
                }
                "numerator_up" => {
                    for &i in &nums {
                        push(format!("num{i}_up"), unit(i, -harder(lines, i)));
                    }
                }
                "numerator_pairs_up" => {
                    for (a, &i) in nums.iter().enumerate() {
                        for &j in &nums[a + 1..] {
                            let mut o = vec![0; n];
                            o[i] = -harder(lines, i);
                            o[j] = -harder(lines, j);
                            push(format!("num{i}_{j}_up"), o);
                        }
                    }
                }
                "line_up_at_lowest" => {
                    for &i in &dotted {
                        if key[i] == lowest[i] {
                            push(format!("line{i}_up"), unit(i, harder(lines, i)));
                        }
                    }
                }
                "floor_of_free" => {
                    let mut o = vec![0; n];
                    for &i in dotted.iter().chain(&nums) {
                        o[i] = -harder(lines, i);
                    }
                    if o.iter().any(|&x| x != 0) {
                        push("floor_of_free".into(), o);
                    }
                }
                _ => {}
            },
        }
    }
    out
}

/// Non-decreasing slot sequences of length `k` over `n` slots, lexicographic.
fn multisets(n: usize, k: u32) -> Vec<Vec<usize>> {
    fn rec(n: usize, k: u32, from: usize, cur: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if cur.len() as u32 == k {
            out.push(cur.clone());
            return;
        }
        for i in from..n {
            cur.push(i);
            rec(n, k, i, cur, out);
            cur.pop();
        }
    }
    let mut out = vec![];
    rec(n, k, 0, &mut vec![], &mut out);
    out
}

/// The offsets of `shells` around `centre` (generation order: shells in list order, raised multiset outer, lowered
/// inner, both lexicographic; raised and lowered slots disjoint), WITHOUT the sector filter.
pub fn shell_offsets(centre: &[i32], lines: &[bool], shells: &[String]) -> Result<Vec<Offset>, String> {
    let n = centre.len();
    let mut out = vec![];
    for s in shells {
        let (p, q) = parse_shell(s)?;
        let ups = multisets(n, p);
        let downs = multisets(n, q);
        for u in &ups {
            for d in &downs {
                if d.iter().any(|x| u.contains(x)) {
                    continue;
                }
                let mut off = centre.to_vec();
                for &i in u {
                    off[i] += harder(lines, i);
                }
                for &i in d {
                    off[i] -= harder(lines, i);
                }
                out.push(off);
            }
        }
    }
    Ok(out)
}

/// The explicit list of a rule (centres x shells) for a GROUP of keys: generation order over the centres resolved at
/// each key (keys in the given order), first occurrence wins, keeping only offsets valid at some key of the group.
pub fn rule_offsets(
    keys: &[Vec<i32>],
    lines: &[bool],
    centres: &[CentreSpec],
    shells: &[String],
    lowest: &[i32],
) -> Result<Vec<Offset>, String> {
    let mut seen: BTreeSet<Offset> = BTreeSet::new();
    let mut out = vec![];
    for k in keys {
        for (_, c) in resolve_centres(centres, k, lines, lowest) {
            for o in shell_offsets(&c, lines, shells)? {
                if !seen.contains(&o) && keys.iter().any(|x| valid_at(x, &o, lines)) {
                    seen.insert(o.clone());
                    out.push(o);
                }
            }
        }
    }
    Ok(out)
}

/// The list in the minimisation order: `distance` = stable sort by the L1 norm of the offset (the distance of the
/// seed from the key; ties keep generation order); `outermost_first` = reverse generation order.
pub fn ordered(list: &[Offset], order: &str) -> Vec<Offset> {
    let mut v = list.to_vec();
    match order {
        "outermost_first" => v.reverse(),
        _ => v.sort_by_key(|o| o.iter().map(|x| x.unsigned_abs() as u64).sum::<u64>()),
    }
    v
}

/// Restrict `list` (kept in its order) to the offsets in `keep`.
pub fn restrict(list: &[Offset], keep: &BTreeSet<Offset>) -> Vec<Offset> {
    list.iter().filter(|o| keep.contains(*o)).cloned().collect()
}

/// The face domain of a signature group (recipe `domain`): slots outside the signature fixed at the frame base (line
/// 1, numerator 0); signature slots free with the bound that keeps the signature (line >= 2, numerator <= -1).
pub fn face_domain(sig: &(Vec<usize>, Vec<usize>), lines: &[bool]) -> Value {
    let n = lines.len();
    let free: BTreeSet<usize> = sig.0.iter().chain(&sig.1).copied().collect();
    let fixed: Vec<Value> = (0..n)
        .map(|i| {
            if free.contains(&i) {
                Value::Null
            } else {
                json!(if lines[i] { 1 } else { 0 })
            }
        })
        .collect();
    let min: Vec<Value> = (0..n)
        .map(|i| if sig.0.contains(&i) { json!(2) } else { Value::Null })
        .collect();
    let max: Vec<Value> = (0..n)
        .map(|i| if sig.1.contains(&i) { json!(-1) } else { Value::Null })
        .collect();
    json!({"fixed": fixed, "equations": [], "exceptions": [], "bounds": {"min": min, "max": max}})
}

/// The explicit `rustred.recipe.v1` document of a list: the offsets as centres in list order (= the elimination's
/// seed order), shell S0, no floor, sector filter on, the face domain; `relation_rule` from the measure; with
/// `engine_minimise` the line-7 `elimination.minimise` member (backend `engine`). Returns (document, canonical text).
pub fn recipe_doc(
    id: &str,
    lines: &[bool],
    domain: &Value,
    list: &[Offset],
    relation_rule: &str,
    provenance: Value,
    engine_minimise: Option<Value>,
) -> Result<(Value, String), String> {
    let chart: String = lines.iter().map(|&l| if l { '1' } else { '0' }).collect();
    let mut elim = json!({"convention": "rustred.recipe-elim.v1"});
    if let Some(m) = engine_minimise {
        elim["minimise"] = m;
    }
    let doc = json!({"schema": "rustred.recipe.v1", "id": id, "arity": lines.len(), "chart": chart,
        "domain": domain,
        "seeds": {"unit": "harder", "centres": list, "generator": {"shells": ["S0"]}, "sector_filter": true},
        "symmetry_rows": false, "relation_rule": relation_rule, "elimination": elim,
        "provenance": provenance});
    let mut text = crate::schema::canon::canonical_json(&doc, false)?;
    text.push('\n');
    Ok((doc, text))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(n_lines: usize, n: usize) -> Vec<bool> {
        (0..n).map(|i| i < n_lines).collect()
    }

    #[test]
    fn the_generic_rule_matches_the_mixed_shell_family_around_one_centre() {
        // 20 slots: the generic six shells around the key over all slots = 4,431 offsets before the sector filter
        let l = lines(9, 20);
        let shells: Vec<String> = ["S0", "S1", "Sm1", "S1m1", "S1m2", "Sm2"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let all = shell_offsets(&[0; 20], &l, &shells).unwrap();
        assert_eq!(all.len(), 4431);
        // with the centres {key, line 0 down} the list at K* is termmin2's 458 (= the helper's K* list), as a SET
        let k: Vec<i32> = vec![2, 1, 1, 1, 1, 1, 1, 1, 1, -2, -1, 0, -1, 0, 0, 0, 0, 0, 0, -1];
        let c = vec![CentreSpec::token("key"), CentreSpec::Offset({
            let mut o = vec![0; 20];
            o[0] = -1;
            o
        })];
        let ours = rule_offsets(std::slice::from_ref(&k), &l, &c, &shells, &k).unwrap();
        let theirs = crate::termmin::steps::mixed_shell_offsets(&k, &l, &shells).unwrap();
        let a: BTreeSet<_> = ours.iter().cloned().collect();
        let b: BTreeSet<_> = theirs.iter().cloned().collect();
        assert_eq!(a, b);
        assert_eq!(ours.len(), 458);
        // the same through the token dotted_line_down (K* has one dotted line)
        let c2 = vec![CentreSpec::token("key"), CentreSpec::token("dotted_line_down")];
        assert_eq!(rule_offsets(std::slice::from_ref(&k), &l, &c2, &shells, &k).unwrap(), ours);
    }

    #[test]
    fn centres_are_generic_tokens() {
        let l = vec![true, true, true, false, false];
        let k = vec![3, 2, 1, -2, 0];
        let lowest = vec![2, 2, 1, -3, 0];
        let r = resolve_centres(
            &[
                CentreSpec::token("key"),
                CentreSpec::token("dotted_line_down"),
                CentreSpec::token("numerator_up"),
                CentreSpec::token("line_up_at_lowest"),
                CentreSpec::token("floor_of_free"),
            ],
            &k,
            &l,
            &lowest,
        );
        let labels: Vec<&str> = r.iter().map(|x| x.0.as_str()).collect();
        assert_eq!(
            labels,
            vec!["key", "line0_down", "line1_down", "num3_up", "line1_up", "floor_of_free"]
        );
        assert_eq!(r[3].1, vec![0, 0, 0, 1, 0]);
        assert_eq!(r[4].1, vec![0, 1, 0, 0, 0]);
        assert_eq!(r[5].1, vec![-1, -1, 0, 1, 0]);
        assert!(valid_at(&k, &[0, 0, -1, 0, 0], &l) == false);
        assert_eq!(dots_rank(&k, &l), 5);
        assert_eq!(signature(&k, &l), (vec![0, 1], vec![3]));
    }

    #[test]
    fn distance_order_is_stable_and_the_recipe_is_readable() {
        let l = vec![true, false];
        let list = vec![vec![1, -1], vec![0, 0], vec![1, 0], vec![0, -1]];
        assert_eq!(
            ordered(&list, "distance"),
            vec![vec![0, 0], vec![1, 0], vec![0, -1], vec![1, -1]]
        );
        assert_eq!(ordered(&list, "outermost_first")[0], vec![0, -1]);
        let dom = face_domain(&(vec![0], vec![]), &l);
        let (_, text) = recipe_doc("t", &l, &dom, &list, "first_admissible", json!({}), None).unwrap();
        let r = crate::worklist::recipe::Recipe::parse(&text).expect("the acceptance chain reads it");
        assert_eq!(r.relation_rule.as_deref(), Some("first_admissible"));
        assert_eq!(r.case_domain().unwrap().arity, 2);
    }
}
