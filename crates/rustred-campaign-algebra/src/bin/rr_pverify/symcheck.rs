//! Exactness of ONE-TERM symmetry rules I(target) = c I(image) against the engine's authenticated automorphism
//! group of the chart (`rr_node symmetry` output): the rule is exact when c = 1 and some group element maps the
//! target pattern slot by slot onto the image pattern (a slot without a single-slot action must carry the constant
//! 0 in the target). Patterns are compared symbolically, so a face rule is checked on its whole domain.

use serde_json::{Value, json};

use crate::table::{Pat, Table, for_each_line, parse_rule};
use crate::{Args, die};

fn image(p: &Pat, action: &[Value]) -> Option<Pat> {
    let n = p.len();
    let mut out: Pat = vec![(None, 0); n];
    let mut set = vec![false; n];
    for i in 0..n {
        let a = &action[i];
        if a.is_null() {
            if p[i] != (None, 0) {
                return None;
            }
            continue;
        }
        let j = a[0].as_u64()? as usize;
        if a[1].as_bool()? {
            return None; // flips (two-sided axes) are not handled
        }
        out[j] = p[i];
        set[j] = true;
    }
    // unset image slots must be zero
    for j in 0..n {
        if !set[j] {
            out[j] = (None, 0);
        }
    }
    Some(out)
}

/// `symcheck --table T --symmetry SYM.json --rules I,J,..`
pub fn cmd(mut a: Args) {
    let tp = a.need("--table");
    let sp = a.need("--symmetry");
    let rules: Vec<usize> = a
        .need("--rules")
        .split(',')
        .map(|x| x.parse().unwrap())
        .collect();
    a.done();
    let sym: Value = serde_json::from_str(&std::fs::read_to_string(&sp).unwrap()).unwrap();
    let elems = sym["group"]["elements"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let n = Table::load(&tp).unwrap_or_else(|e| die(&e)).n;
    let mut res = vec![];
    let mut pos = 0usize;
    for_each_line(&tp, |_, _, v| {
        if v["type"].as_str() != Some("rule") {
            return Ok(());
        }
        let r = parse_rule(&v, n, pos)?;
        pos += 1;
        if !rules.contains(&r.pos) {
            return Ok(());
        }
        let rhs = v["rhs"].as_array().unwrap();
        if rhs.len() != 1 {
            res.push(json!({"rule": r.pos, "exact": false, "why": format!("{} terms (not a one-term rule)", rhs.len())}));
            return Ok(());
        }
        let coef = rhs[0][1].as_str().unwrap_or("").trim().trim_start_matches('(').trim_end_matches(')').to_string();
        let target = &r.target;
        let img = &r.rhs_pats[0];
        let mut found = vec![];
        for e in &elems {
            let act = e["action"].as_array().cloned().unwrap_or_default();
            if act.len() != n {
                continue;
            }
            if image(target, &act).as_ref() == Some(img) {
                found.push(e["element"].clone());
            }
        }
        res.push(json!({"rule": r.pos, "coefficient": coef, "group_elements_mapping_target_to_image": found,
            "exact": coef == "1" && !found.is_empty()}));
        Ok(())
    })
    .unwrap_or_else(|e| die(&e));
    let ok = res.iter().filter(|x| x["exact"] == true).count();
    println!("{}", serde_json::to_string_pretty(&json!({"schema": "rr_pverify.symcheck.v1", "group_order": sym["group"]["group_order"],
        "verified_generators": sym["group"]["verified_generators"], "elements_listed": elems.len(), "rules": res, "exact": ok})).unwrap());
}
