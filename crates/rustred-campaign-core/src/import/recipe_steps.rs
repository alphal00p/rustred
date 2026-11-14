//! Recipe STEPS that choose the seed set per key (the independent checker's default step sequence, e.g. T266's
//! `d0 > d1_cnone_sfirst > d2 {first,last,num,union} x {fon,foff} M1C > d3_cfirst_foff > d3_cunion_fon`) mapped onto
//! explicit `rustred.recipe.v1` recipes (interfaces/import.md section 14, coord-recipe-solve.md v1.1 + v1.2 section 13).
//!
//! A step name `d<k>_c<centre>_f<on|off>[_s<sym>][_m<rule>]` is evaluated AT EACH KEY (its centres and floor depend on
//! the key); keys with the same explicit seed set are grouped into ONE recipe. Grammar and semantics (the checker's,
//! as the engine-line translation `SCRATCHPAD/engine-line/tools/mk_k5.py` reads them; nothing node-specific here: the
//! line / numerator priorities are INPUT):
//! - `d<k>`: up-shells `S0..Sk` around every centre (`d0` = `S0`; seeds 1 / 21 / 231 / 1771 at arity 20 for k = 0..3);
//! - centre `none` = the key; `first` = one step toward the floor on the first off-floor line in the line priority
//!   (else the first off-floor numerator); `last` = the last off-floor line (else the last numerator); `num` = the first
//!   off-floor numerator in the numerator priority (else the first line); `union` = one centre per off-floor slot;
//!   off-floor = a line > 1 or a numerator < 0; a step = -1 on a line, +1 on a numerator;
//! - floor `on` = `min_offset -1` on every line > 1 at the key; `off` = none;
//! - `s<..>` (symmetry rows at the key, placed first) -> `symmetry_rows: true` (an APPROXIMATION: the engine adds the
//!   group's rows at every seed after its IBP rows; exact form needs the proposed v1.3 `symmetry_rows: "key_first"`);
//! - `m<..>` (`M1C`: an M1 relation, else the comparator's) -> v1.2 `relation_rule: "m1_first"`; absent ->
//!   `first_admissible` (v1.1's only rule).
//!
//! Output forms: `V1_2` = one recipe per group with `domain.keys` and `relation_rule`; `V1_1` = one POINT recipe per key
//! (`domain.fixed` = the key; v1.1 has neither `domain.keys` nor `relation_rule`: an `m` step is then refused, since
//! v1.1 cannot state it and silently solving with another rule would not reproduce the step).

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Value, json};

use super::error::ImportError;

/// The centre rule of a step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Centre {
    None,
    First,
    Last,
    Num,
    Union,
}

/// A parsed step name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Step {
    pub name: String,
    pub depth: u32,
    pub centre: Centre,
    pub floor: bool,
    pub symmetry_first: bool,
    /// `Some("M1C")` etc. (an M1-first relation rule)
    pub relation: Option<String>,
}

/// Parse `d<k>[_c<centre>][_f<on|off>][_s<..>][_m<..>]` (parts after `d<k>` in any order; unknown parts refused).
pub fn parse_step(name: &str) -> Result<Step, ImportError> {
    let bad = |m: &str| ImportError::Spec(format!("recipe step {name:?}: {m}"));
    let mut parts = name.split('_');
    let d = parts.next().ok_or_else(|| bad("empty"))?;
    let depth: u32 = d
        .strip_prefix('d')
        .and_then(|x| x.parse().ok())
        .ok_or_else(|| bad("must start with d<depth>"))?;
    let mut st = Step {
        name: name.to_string(),
        depth,
        centre: Centre::None,
        floor: false,
        symmetry_first: false,
        relation: None,
    };
    for p in parts {
        match p.split_at(1) {
            ("c", c) => {
                st.centre = match c {
                    "none" => Centre::None,
                    "first" => Centre::First,
                    "last" => Centre::Last,
                    "num" => Centre::Num,
                    "union" => Centre::Union,
                    _ => return Err(bad(&format!("unknown centre rule {c:?}"))),
                }
            }
            ("f", "on") => st.floor = true,
            ("f", "off") => st.floor = false,
            ("s", _) => st.symmetry_first = true,
            ("m", m) if !m.is_empty() => st.relation = Some(m.to_string()),
            _ => return Err(bad(&format!("unknown part {p:?}"))),
        }
    }
    Ok(st)
}

/// The recipe form to write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecipeForm {
    V1_1,
    V1_2,
}

/// The explicit seed set of a step at a key: (centres as offsets from the key, floored line slots, shells, symmetry).
pub fn explicit_seeds(
    st: &Step,
    key: &[i32],
    lines: &[bool],
    line_priority: &[usize],
    num_priority: &[usize],
) -> (Vec<Vec<i32>>, Vec<usize>, Vec<String>, bool) {
    let n = key.len();
    let off_lines: Vec<usize> = line_priority
        .iter()
        .copied()
        .filter(|&i| lines[i] && key[i] > 1)
        .collect();
    let off_nums: Vec<usize> = num_priority
        .iter()
        .copied()
        .filter(|&j| !lines[j] && key[j] < 0)
        .collect();
    let step = |i: usize| -> Vec<i32> {
        let mut v = vec![0; n];
        v[i] = if lines[i] { -1 } else { 1 };
        v
    };
    let zero = vec![0; n];
    let centres = match st.centre {
        Centre::None => vec![zero],
        Centre::First => vec![
            off_lines
                .first()
                .or(off_nums.first())
                .map(|&i| step(i))
                .unwrap_or(zero),
        ],
        Centre::Last => vec![
            off_lines
                .last()
                .or(off_nums.last())
                .map(|&i| step(i))
                .unwrap_or(zero),
        ],
        Centre::Num => vec![
            off_nums
                .first()
                .or(off_lines.first())
                .map(|&i| step(i))
                .unwrap_or(zero),
        ],
        Centre::Union => {
            let mut slots: Vec<usize> = (0..n)
                .filter(|&i| (lines[i] && key[i] > 1) || (!lines[i] && key[i] < 0))
                .collect();
            slots.sort_unstable();
            let v: Vec<Vec<i32>> = slots.into_iter().map(step).collect();
            if v.is_empty() { vec![zero] } else { v }
        }
    };
    let floor: Vec<usize> = if st.floor {
        (0..n).filter(|&i| lines[i] && key[i] > 1).collect()
    } else {
        vec![]
    };
    let shells: Vec<String> = (0..=st.depth).map(|k| format!("S{k}")).collect();
    (centres, floor, shells, st.symmetry_first)
}

/// One generated recipe and the keys it is assigned.
#[derive(Clone, Debug)]
pub struct GeneratedRecipe {
    pub id: String,
    pub step: String,
    pub recipe: Value,
    pub keys: Vec<Vec<i32>>,
}

/// Map (step name, key) assignments onto explicit recipes: one per (step, explicit seed set) in `V1_2` (with
/// `domain.keys` and the relation rule), one point recipe per key in `V1_1`. `id_prefix` prefixes every recipe id.
pub fn recipes_for_steps(
    assignments: &[(String, Vec<i32>)],
    lines: &[bool],
    line_priority: &[usize],
    num_priority: &[usize],
    form: RecipeForm,
    id_prefix: &str,
) -> Result<Vec<GeneratedRecipe>, ImportError> {
    let n = lines.len();
    let chart: String = lines.iter().map(|&l| if l { '1' } else { '0' }).collect();
    type GroupKey = (String, String, String, String, bool);
    let mut groups: BTreeMap<GroupKey, (Step, Vec<Vec<i32>>)> = BTreeMap::new();
    let mut order: Vec<GroupKey> = vec![];
    for (name, key) in assignments {
        if key.len() != n {
            return Err(ImportError::Spec(format!(
                "key {key:?} of arity {} for {n}",
                key.len()
            )));
        }
        let st = parse_step(name)?;
        if form == RecipeForm::V1_1 && st.relation.is_some() {
            return Err(ImportError::Unsupported(format!(
                "step {name}: relation rule {:?} needs recipe v1.2 (relation_rule)",
                st.relation
            )));
        }
        let (c, f, s, sym) = explicit_seeds(&st, key, lines, line_priority, num_priority);
        let gk: GroupKey = (
            name.clone(),
            serde_json::to_string(&c).unwrap_or_default(),
            serde_json::to_string(&f).unwrap_or_default(),
            s.join("+"),
            sym,
        );
        let gk = match form {
            RecipeForm::V1_2 => gk,
            // one point recipe per key
            RecipeForm::V1_1 => (format!("{}@{:?}", gk.0, key), gk.1, gk.2, gk.3, gk.4),
        };
        if !groups.contains_key(&gk) {
            order.push(gk.clone());
        }
        groups
            .entry(gk)
            .or_insert_with(|| (st, vec![]))
            .1
            .push(key.clone());
    }
    let mut out = vec![];
    for (gi, gk) in order.iter().enumerate() {
        let (st, keys) = &groups[gk];
        let (c, f, s, sym) = explicit_seeds(st, &keys[0], lines, line_priority, num_priority);
        let id = format!("{id_prefix}{}/g{gi:05}", st.name);
        let min_offset: serde_json::Map<String, Value> =
            f.iter().map(|i| (i.to_string(), json!(-1))).collect();
        let domain = match form {
            RecipeForm::V1_2 => json!({"keys": keys}),
            RecipeForm::V1_1 => json!({"fixed": keys[0], "equations": [], "exceptions": []}),
        };
        let mut r = json!({"schema": "rustred.recipe.v1", "id": id, "arity": n, "chart": chart,
            "domain": domain,
            "seeds": {"unit": "harder", "centres": c, "generator": {"shells": s},
                      "min_offset": min_offset, "sector_filter": true},
            "symmetry_rows": sym,
            "elimination": {"convention": "rustred.recipe-elim.v1"},
            "provenance": {"step": st.name, "approximation": if sym {
                Value::String("symmetry rows at every seed after the IBP rows (step: at the key only, first; v1.3 key_first)".into())
            } else { Value::Null }}});
        if form == RecipeForm::V1_2 {
            r["relation_rule"] = json!(if st.relation.is_some() {
                "m1_first"
            } else {
                "first_admissible"
            });
        }
        out.push(GeneratedRecipe {
            id,
            step: st.name.clone(),
            recipe: r,
            keys: keys.clone(),
        });
    }
    Ok(out)
}

/// The per-key assignment side file (interfaces/import.md 14) of generated recipes: one line per recipe.
pub fn assignment_lines(recipes: &[GeneratedRecipe]) -> String {
    recipes
        .iter()
        .map(|g| format!("{}\n", json!({"recipe": g.id, "keys": g.keys})))
        .collect()
}
