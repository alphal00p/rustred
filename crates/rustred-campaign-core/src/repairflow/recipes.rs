//! Face-domain recipes from escalation step names (interfaces/repairflow.md section 5).
//!
//! A step `d<k>_c<centre>_f<on|off>[_s..][_m..]` (import.md 14 grammar) is read ON THE FACE: the centre is chosen among
//! the face's own off-floor slots in the line / numerator priority, so the seed set is uniform over every key of the
//! face; the domain is the PROPER face (face lines >= 2, face numerators <= -1, everything else at its floor): a key of a
//! smaller face is outside it and dispatch falls through. Written as the core `worklist::recipe::Recipe` (the engine's
//! `rustred.recipe.v1` file in its v1.1 form, which the acceptance chain reads): v1.1 has no `relation_rule`, the
//! engine's default (M1 first) applies, so a step WITHOUT `m` is refused here (it could not be reproduced).

use std::collections::BTreeMap;

use crate::import::recipe_steps::{Centre, parse_step};
use crate::worklist::items::Face;
use crate::worklist::recipe::{
    RECIPE_ELIMINATION, RECIPE_SCHEMA, RECIPE_UNIT, Recipe, RecipeBall, RecipeBounds, RecipeDomain,
    RecipeElimination, RecipeGenerator, RecipeSeeds,
};

/// Identity fields written into the recipe's provenance (echoed by the engine, part of its digest).
#[derive(Clone, Debug)]
pub struct RecipeContext<'a> {
    pub node: &'a str,
    pub node_order: &'a str,
    pub policy_digest: &'a str,
    /// Shells of the `mixed_shell` step (harder unit; policy `recipes.mixed_shells`).
    pub mixed_shells: Vec<String>,
}

/// The step name of the MIXED-SHELL family (lane termmin2: centres = the key and each dotted line one step down,
/// shells in the harder unit, an EXPLICIT offset list since `rustred.recipe.v1` has no S1m2 shell).
pub const MIXED_SHELL: &str = "mixed_shell";

/// The face recipe of the mixed-shell step: the offsets of `termmin::steps::mixed_shell_offsets` at the face's LOWEST key
/// (face lines at 2, face numerators at -1): they depend on the key only through its dotted lines, so they are uniform
/// over the face; one centre per seed, shell S0, no floor; domain = the proper face.
pub fn mixed_shell_recipe(
    face: &Face,
    lines: &[bool],
    shells: &[String],
    ctx: &RecipeContext<'_>,
) -> Result<Recipe, String> {
    let n = lines.len();
    let rep: Vec<i32> = (0..n)
        .map(|i| {
            if face.sd.contains(&i) {
                2
            } else if face.sn.contains(&i) {
                -1
            } else if lines[i] {
                1
            } else {
                0
            }
        })
        .collect();
    let offs = crate::termmin::steps::mixed_shell_offsets(&rep, lines, shells)?;
    let slots = face.slots();
    let fixed: Vec<Option<i32>> = (0..n)
        .map(|i| (!slots.contains(&i)).then_some(if lines[i] { 1 } else { 0 }))
        .collect();
    let bounds = (!slots.is_empty()).then(|| RecipeBounds {
        min: (0..n).map(|i| face.sd.contains(&i).then_some(2)).collect(),
        max: (0..n).map(|i| face.sn.contains(&i).then_some(-1)).collect(),
    });
    let r = Recipe {
        schema: RECIPE_SCHEMA.into(),
        id: format!("rf/{}/{MIXED_SHELL}", face_tag(face)),
        arity: n,
        chart: lines.iter().map(|&l| if l { '1' } else { '0' }).collect(),
        domain: RecipeDomain {
            fixed,
            equations: vec![],
            exceptions: vec![],
            bounds,
        },
        seeds: RecipeSeeds {
            unit: RECIPE_UNIT.into(),
            centres: offs,
            generator: RecipeGenerator::Shells(vec!["S0".into()]),
            min_offset: BTreeMap::new(),
            max_offset: BTreeMap::new(),
            layer_cap: None,
            sector_filter: true,
            sector_slots: None,
        },
        symmetry_rows: false,
        elimination: RecipeElimination {
            convention: RECIPE_ELIMINATION.into(),
        },
        provenance: Some(serde_json::json!({
            "node": ctx.node, "node_order": ctx.node_order, "face": face.label(), "step": MIXED_SHELL,
            "shells_harder_unit": shells, "producer": "repairflow", "policy": ctx.policy_digest,
        })),
    };
    r.validate()?;
    Ok(r)
}

/// A safe file-name component of a face label + step (`{6,7}x{11}` -> `d6.7_n11`).
pub fn face_tag(face: &Face) -> String {
    let j = |v: &[usize]| {
        v.iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join(".")
    };
    format!("d{}_n{}", j(&face.sd), j(&face.sn))
}

/// The face recipe of one step.
pub fn face_recipe(
    face: &Face,
    step_name: &str,
    lines: &[bool],
    line_priority: &[usize],
    num_priority: &[usize],
    ctx: &RecipeContext<'_>,
) -> Result<Recipe, String> {
    if step_name == MIXED_SHELL {
        return mixed_shell_recipe(face, lines, &ctx.mixed_shells, ctx);
    }
    let st = parse_step(step_name).map_err(|e| e.to_string())?;
    if st.relation.is_none() {
        return Err(format!(
            "step {step_name}: without an `m` part the step asks first_admissible, which a v1.1 recipe cannot state"
        ));
    }
    let n = lines.len();
    let off_lines: Vec<usize> = line_priority
        .iter()
        .copied()
        .filter(|i| face.sd.contains(i))
        .collect();
    let off_nums: Vec<usize> = num_priority
        .iter()
        .copied()
        .filter(|j| face.sn.contains(j))
        .collect();
    let step = |i: usize| -> Vec<i32> {
        let mut v = vec![0; n];
        v[i] = if lines[i] { -1 } else { 1 };
        v
    };
    let centres: Vec<Vec<i32>> = match st.centre {
        Centre::None => vec![],
        Centre::First => off_lines
            .first()
            .or(off_nums.first())
            .map(|&i| vec![step(i)])
            .unwrap_or_default(),
        Centre::Last => off_lines
            .last()
            .or(off_nums.last())
            .map(|&i| vec![step(i)])
            .unwrap_or_default(),
        Centre::Num => off_nums
            .first()
            .or(off_lines.first())
            .map(|&i| vec![step(i)])
            .unwrap_or_default(),
        Centre::Union => {
            let mut s = face.slots();
            s.sort_unstable();
            s.into_iter().map(step).collect()
        }
    };
    let mut min_offset = BTreeMap::new();
    if st.floor {
        for &i in &face.sd {
            min_offset.insert(i, -1);
        }
    }
    let generator = if st.depth <= 3 {
        RecipeGenerator::Shells((0..=st.depth).map(|k| format!("S{k}")).collect())
    } else {
        RecipeGenerator::Ball(RecipeBall {
            radius: st.depth as u8,
            down: vec![],
        })
    };
    let slots = face.slots();
    let fixed: Vec<Option<i32>> = (0..n)
        .map(|i| {
            if slots.contains(&i) {
                None
            } else {
                Some(if lines[i] { 1 } else { 0 })
            }
        })
        .collect();
    let bounds = if slots.is_empty() {
        None
    } else {
        Some(RecipeBounds {
            min: (0..n).map(|i| face.sd.contains(&i).then_some(2)).collect(),
            max: (0..n).map(|i| face.sn.contains(&i).then_some(-1)).collect(),
        })
    };
    let r = Recipe {
        schema: RECIPE_SCHEMA.into(),
        id: format!("rf/{}/{}", face_tag(face), st.name),
        arity: n,
        chart: lines.iter().map(|&l| if l { '1' } else { '0' }).collect(),
        domain: RecipeDomain {
            fixed,
            equations: vec![],
            exceptions: vec![],
            bounds,
        },
        seeds: RecipeSeeds {
            unit: RECIPE_UNIT.into(),
            centres,
            generator,
            min_offset,
            max_offset: BTreeMap::new(),
            layer_cap: None,
            sector_filter: true,
            sector_slots: None,
        },
        symmetry_rows: st.symmetry_first,
        elimination: RecipeElimination {
            convention: RECIPE_ELIMINATION.into(),
        },
        provenance: Some(serde_json::json!({
            "node": ctx.node, "node_order": ctx.node_order, "face": face.label(), "step": st.name,
            "producer": "repairflow", "policy": ctx.policy_digest,
        })),
    };
    r.validate()?;
    Ok(r)
}
