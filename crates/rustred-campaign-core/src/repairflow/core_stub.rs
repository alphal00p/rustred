//! STUB of the IBP-core call (interfaces/coreseed.md: lane coreseed offers `solve_core(keys, frame_lines, policy,
//! &mut dyn CoreSolver) -> CoreReport`). Until it lands, the keys a face hands to its IBP core (no symmetry piece
//! serves them) go to the EXISTING face-recipe escalation of the closing loop (`recipes.escalation`, mixed-shell step
//! first in the default): this function returns that request per face, with the core keys it is meant for. Pure.

use std::collections::BTreeMap;

use super::policy::RepairPolicy;
use super::recipes::{RecipeContext, face_recipe};
use crate::worklist::items::Face;
use crate::worklist::recipe::Recipe;

/// What the IBP core of one face is asked to do (stub form).
#[derive(Clone, Debug)]
pub struct CoreRequest {
    pub face: Face,
    /// The face's core keys (no acceptable symmetry choice).
    pub keys: Vec<Vec<i32>>,
    /// The recipe escalation in order (`recipes.escalation`), first certifying step wins per key.
    pub steps: Vec<(String, Result<Recipe, String>)>,
    /// `stub: face-recipe escalation` until lane coreseed's solve_core is wired.
    pub route: &'static str,
}

/// The stub: group the core keys by face, one escalation request per face.
pub fn solve_core(
    core_keys: &[Vec<i32>],
    lines: &[bool],
    policy: &RepairPolicy,
    line_priority: &[usize],
    num_priority: &[usize],
    ctx: &RecipeContext<'_>,
) -> Vec<CoreRequest> {
    let mut by: BTreeMap<String, (Face, Vec<Vec<i32>>)> = BTreeMap::new();
    for k in core_keys {
        if let Some(f) = Face::of_key(k, lines) {
            by.entry(f.label())
                .or_insert_with(|| (f.clone(), vec![]))
                .1
                .push(k.clone());
        }
    }
    by.into_values()
        .map(|(face, keys)| {
            let steps = policy
                .recipes
                .escalation
                .iter()
                .map(|s| {
                    (
                        s.clone(),
                        face_recipe(&face, s, lines, line_priority, num_priority, ctx),
                    )
                })
                .collect();
            CoreRequest {
                face,
                keys,
                steps,
                route: "stub: face-recipe escalation (lane coreseed's solve_core not wired)",
            }
        })
        .collect()
}
