//! Assembly BY FORM for recipe rules (R48; interface section 10.4): `<node>.recipes.jsonl` with one record per
//! certified recipe, the certified key-set files (closure.md key-set format) and relation-set files referenced by
//! digest, and the economy by form (bytes, estimated application cost = sum of recorded per-key solve costs). An
//! assembled set holding recipes is NOT engine-loadable today (the coordinator's recipe record type and dispatcher
//! do not exist yet): `engine_loadable = false`.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::json;

use crate::accept::Position;
use crate::accept::recipe::{Recipe, RecipeVerdict};
use crate::closure::{KeyCodec, KeySet};

pub const RECIPE_RECORD_SCHEMA: &str = "rustred.campaign.recipe-record.v1";

/// One certified recipe to ship.
pub struct RecipeToShip<'a> {
    pub recipe: &'a Recipe,
    pub position: Position,
    pub verdict: &'a RecipeVerdict,
}

/// Economy of an assembled set by form (bytes; recipes: estimated application cost).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct FormEconomy {
    pub symbolic_rules: u64,
    pub point_rules: u64,
    pub table_bytes: u64,
    pub recipes: u64,
    pub recipe_certified_keys: u64,
    pub recipes_bytes: u64,
    pub keysets_bytes: u64,
    pub relation_sets_bytes: u64,
    pub recipe_cost_ms: u64,
    pub engine_loadable: bool,
}

/// Write the recipe files into `out_dir`; `table` (the assembled symbolic + point table) is measured for the economy.
pub fn write(
    node: &str,
    out_dir: &Path,
    table: Option<&Path>,
    recipes: &[RecipeToShip],
) -> Result<(PathBuf, FormEconomy), String> {
    std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    let mut eco = FormEconomy::default();
    if let Some(t) = table {
        let text = std::fs::read(t).map_err(|e| e.to_string())?;
        eco.table_bytes = text.len() as u64;
        for l in text.split(|&b| b == b'\n').filter(|l| l.starts_with(br#"{"type":"rule""#)) {
            let v: serde_json::Value = match serde_json::from_slice(l) {
                Ok(v) => v,
                Err(e) => return Err(e.to_string()),
            };
            if v["fixed"].as_array().is_some_and(|a| a.iter().all(|x| !x.is_null())) {
                eco.point_rules += 1;
            } else {
                eco.symbolic_rules += 1;
            }
        }
    }
    let mut out = String::new();
    let mut sorted: Vec<&RecipeToShip> = recipes.iter().collect();
    sorted.sort_by(|a, b| a.position.cmp(&b.position));
    for r in sorted {
        let RecipeVerdict::Certified {
            recipe_digest,
            certified,
            certified_digest,
            relation_set_digest,
            relation_set,
            binary_sha256,
            determinism_contract,
            cost_ms,
            ..
        } = r.verdict
        else {
            return Err(format!("recipe {} is not certified", r.recipe.digest));
        };
        let arity = r.recipe.domain.arity;
        let maxv = certified.iter().flatten().map(|v| v.unsigned_abs()).max().unwrap_or(0);
        let bits = (4..=16).find(|b| (maxv as u64) < (1u64 << (b - 1))).ok_or("key values too large")?;
        let ks = KeySet::from_keys(KeyCodec::new(arity, bits).map_err(|e| format!("{e:?}"))?, certified.iter().map(|k| k.as_slice()))
            .map_err(|e| format!("{e:?}"))?;
        let tag = &recipe_digest[7..23];
        let kf = out_dir.join(format!("{node}.recipe-{tag}.keys"));
        let mut buf = Vec::new();
        let d = ks.write(&mut buf).map_err(|e| e.to_string())?;
        assert_eq!(&d, certified_digest, "key-set digest rule");
        std::fs::write(&kf, &buf).map_err(|e| e.to_string())?;
        let rf = out_dir.join(format!("{node}.recipe-{tag}.relations.jsonl"));
        std::fs::write(&rf, relation_set).map_err(|e| e.to_string())?;
        eco.keysets_bytes += buf.len() as u64;
        eco.relation_sets_bytes += relation_set.len() as u64;
        eco.recipes += 1;
        eco.recipe_certified_keys += certified.len() as u64;
        eco.recipe_cost_ms += cost_ms;
        let line = json!({"schema": RECIPE_RECORD_SCHEMA, "recipe_digest": recipe_digest, "recipe": r.recipe.body,
            "position": r.position, "form": "recipe",
            "certified_keys": {"file": kf.file_name().map(|f| f.to_string_lossy().to_string()), "digest": certified_digest, "count": certified.len()},
            "relation_set": {"file": rf.file_name().map(|f| f.to_string_lossy().to_string()), "digest": relation_set_digest},
            "binary_sha256": binary_sha256, "determinism_contract": determinism_contract, "cost_ms": cost_ms,
            "dispatch": "matches only keys in domain AND certified set; otherwise falls through"});
        out.push_str(&line.to_string());
        out.push('\n');
    }
    let p = out_dir.join(format!("{node}.recipes.jsonl"));
    eco.recipes_bytes = out.len() as u64;
    std::fs::write(&p, out).map_err(|e| e.to_string())?;
    eco.engine_loadable = eco.recipes == 0;
    Ok((p, eco))
}
