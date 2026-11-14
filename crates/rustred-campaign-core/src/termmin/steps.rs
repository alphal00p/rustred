//! POINT RECIPES at one terminal key (interfaces/termmin.md section 2): a recipe step of the import.md 14 grammar
//! (`import::recipe_steps`) evaluated AT THE KEY, written as an explicit `rustred.recipe.v1` file with `domain.keys =
//! [key]`, and its EXPLICIT SEED SET enumerated exactly as the verb builds it (centres in order, up-shells S0..Sk, the
//! floor, the chart's sector filter; first occurrence wins), so that the seed cap, the per-terminal seed budget and the
//! "same seed set as an earlier step" skip are decided before any solve. Generic: arity, chart and priorities are input.

use std::collections::BTreeSet;

use serde::Serialize;
use serde_json::Value;

use crate::import::recipe_steps::{RecipeForm, Step, explicit_seeds, recipes_for_steps};

/// One point recipe at a key.
#[derive(Clone, Debug, Serialize)]
pub struct PointRecipe {
    pub id: String,
    pub step: String,
    /// The recipe document (`rustred.recipe.v1`, `domain.keys = [key]`).
    pub recipe: Value,
    /// Canonical JSON text of the recipe + "\n" (the bytes of the recipe file).
    pub text: String,
    /// sha256 (hex) of `text`.
    pub sha256: String,
    /// Number of seeds of the explicit seed set.
    pub seeds: u64,
    /// sha256 (hex) of the sorted explicit seed set (the "same seed set" test).
    pub seed_set_digest: String,
    pub centres: Vec<Vec<i32>>,
    pub floor: Vec<usize>,
    pub shells: Vec<String>,
    pub symmetry_rows: bool,
    pub relation_rule: String,
}

fn sha256_hex(b: &[u8]) -> String {
    let mut h = crate::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

/// Key text `I(a,b,..)`.
pub fn key_text(key: &[i32]) -> String {
    format!(
        "I({})",
        key.iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(",")
    )
}

/// The explicit seed set of a recipe step at `key`: for each centre (offset from the key) the up-shells S0..S`depth`
/// (an up-move is +1 on a line, -1 on a numerator), then the floor (`min_offset -1` on `floor` slots: a seed with
/// `seed[s] - key[s] < -1` is dropped) and the sector filter (a seed with a line <= 0 or a numerator > 0 is dropped).
pub fn enumerate_seeds(
    key: &[i32],
    centres: &[Vec<i32>],
    floor: &[usize],
    depth: u32,
    lines: &[bool],
) -> BTreeSet<Vec<i32>> {
    let n = key.len();
    let mut out = BTreeSet::new();
    let up = |s: &mut Vec<i32>, i: usize| {
        if lines[i] { s[i] += 1 } else { s[i] -= 1 }
    };
    for c in centres {
        let centre: Vec<i32> = key.iter().zip(c).map(|(k, o)| k + o).collect();
        // multisets of up-moves of size m <= depth: non-decreasing slot sequences
        let mut stack: Vec<(Vec<i32>, usize, u32)> = vec![(centre, 0, 0)];
        while let Some((s, from, m)) = stack.pop() {
            let keep = floor.iter().all(|&f| s[f] - key[f] >= -1)
                && (0..n).all(|i| if lines[i] { s[i] >= 1 } else { s[i] <= 0 });
            if keep {
                out.insert(s.clone());
            }
            if m < depth {
                for i in from..n {
                    let mut t = s.clone();
                    up(&mut t, i);
                    stack.push((t, i, m + 1));
                }
            }
        }
    }
    out
}

/// The point recipe of `step` at `key` (one recipe, `domain.keys = [key]`, v1.2 form with the step's relation rule).
pub fn point_recipe(
    step: &Step,
    key: &[i32],
    lines: &[bool],
    line_priority: &[usize],
    num_priority: &[usize],
) -> Result<PointRecipe, String> {
    let prefix = format!("termmin/{}/", key_text(key));
    let g = recipes_for_steps(
        &[(step.name.clone(), key.to_vec())],
        lines,
        line_priority,
        num_priority,
        RecipeForm::V1_2,
        &prefix,
    )
    .map_err(|e| format!("recipe step {}: {e}", step.name))?;
    let [one] = g.as_slice() else {
        return Err(format!(
            "recipe step {}: {} recipes for one key",
            step.name,
            g.len()
        ));
    };
    let (centres, floor, shells, sym) =
        explicit_seeds(step, key, lines, line_priority, num_priority);
    let seeds = enumerate_seeds(key, &centres, &floor, step.depth, lines);
    // the form every reader accepts today (the acceptance chain's `worklist::recipe::Recipe` is the v1.1 file: no
    // `relation_rule`, no `domain.keys`): the point domain as `domain.fixed`, the relation rule left to the engine
    // (v1.2's default is m1_first) and recorded in the provenance; depth > 3 as an up-only L1 ball (the verb's named
    // shells stop at S3; `ball {radius k, down []}` = the up-shells S0..Sk)
    let relation_rule = one.recipe["relation_rule"]
        .as_str()
        .unwrap_or("first_admissible")
        .to_string();
    let mut doc = one.recipe.clone();
    if let Some(o) = doc.as_object_mut() {
        o.remove("relation_rule");
        o.insert(
            "domain".into(),
            serde_json::json!({"fixed": key, "equations": [], "exceptions": []}),
        );
        if step.depth > 3 {
            o["seeds"]["generator"] =
                serde_json::json!({"ball": {"radius": step.depth, "down": []}});
        }
        o["provenance"]["relation_rule"] = Value::String(relation_rule.clone());
        o["provenance"]["termmin_key"] = serde_json::json!(key);
    }
    let mut sd = String::new();
    for s in &seeds {
        sd.push_str(&format!("{s:?}\n"));
    }
    let mut text = crate::schema::canon::canonical_json(&doc, false)?;
    text.push('\n');
    Ok(PointRecipe {
        id: one.id.clone(),
        step: step.name.clone(),
        recipe: doc,
        sha256: sha256_hex(text.as_bytes()),
        text,
        seeds: seeds.len() as u64,
        seed_set_digest: sha256_hex(sd.as_bytes()),
        centres,
        floor,
        shells,
        symmetry_rows: sym,
        relation_rule,
    })
}

/// Default priorities: lines (resp. numerators) in the order of `coordinate_priority` when given, else slot order.
pub fn priorities(
    lines: &[bool],
    coordinate_priority: Option<&[usize]>,
) -> (Vec<usize>, Vec<usize>) {
    let n = lines.len();
    let order: Vec<usize> = match coordinate_priority {
        Some(p)
            if p.len() == n && {
                let s: BTreeSet<usize> = p.iter().copied().collect();
                s.len() == n && s.iter().all(|&x| x < n)
            } =>
        {
            p.to_vec()
        }
        _ => (0..n).collect(),
    };
    (
        order.iter().copied().filter(|&i| lines[i]).collect(),
        order.iter().copied().filter(|&i| !lines[i]).collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::recipe_steps::parse_step;

    #[test]
    fn up_shell_counts_are_binomial() {
        let lines = vec![true; 10]
            .into_iter()
            .chain(vec![false; 10])
            .collect::<Vec<_>>();
        let key: Vec<i32> = (0..20).map(|i| if i < 10 { 1 } else { 0 }).collect();
        let z = vec![vec![0; 20]];
        let c: Vec<usize> = (0..=3)
            .map(|d| enumerate_seeds(&key, &z, &[], d, &lines).len())
            .collect();
        assert_eq!(c, vec![1, 21, 231, 1771]);
    }

    #[test]
    fn point_recipe_has_one_key_and_counts_its_seeds() {
        let lines = [true, true, false];
        let st = parse_step("d2_clast_fon_mM1C").unwrap();
        let r = point_recipe(&st, &[2, 1, 0], &lines, &[0, 1], &[2]).unwrap();
        assert_eq!(r.recipe["domain"]["fixed"], serde_json::json!([2, 1, 0]));
        assert!(r.recipe.get("relation_rule").is_none());
        crate::worklist::recipe::Recipe::parse(&r.text).expect("the acceptance chain reads it");
        assert_eq!(r.relation_rule, "m1_first");
        assert_eq!(r.centres, vec![vec![-1, 0, 0]]);
        assert_eq!(r.floor, vec![0]);
        // centre [1,1,0], up-shells to depth 2 over 3 slots: 1 + 3 + 6
        assert_eq!(r.seeds, 10);
        let u = point_recipe(
            &parse_step("d2_cunion_fon_mM1C").unwrap(),
            &[2, 1, 0],
            &lines,
            &[0, 1],
            &[2],
        )
        .unwrap();
        assert_eq!(u.seed_set_digest, r.seed_set_digest);
        assert_ne!(u.sha256, r.sha256);
        let d4 = point_recipe(
            &parse_step("d4_cfirst_foff").unwrap(),
            &[2, 1, 0],
            &lines,
            &[0, 1],
            &[2],
        )
        .unwrap();
        assert_eq!(
            d4.recipe["seeds"]["generator"],
            serde_json::json!({"ball": {"radius": 4, "down": []}})
        );
        crate::worklist::recipe::Recipe::parse(&d4.text).expect("ball form");
    }
}
