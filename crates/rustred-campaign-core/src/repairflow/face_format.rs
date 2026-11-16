//! Writer of the common face record `face_recipe.v1` (/common/ben/sym-investigation/w13/FACE_RECIPE_FORMAT.md) for the
//! faces of a node's decisions: the base fields we can fill (null where the campaign has no data) plus OUR SEVEN
//! ADDITIVE FIELDS (`route`, `evidence_layers`, `bound_to`, `orbit`, `cut_by`, `exception_loci[].status`,
//! `cost_per_key`; notes of lane repairflow, first section). Reading the format is not implemented.

use serde_json::{Value, json};

use super::decide::{FaceDecision, NodeDecisions};
use super::judge::Class;

/// What binds the records (section `bound_to`).
#[derive(Clone, Debug, Default)]
pub struct Binding {
    pub node_identity: Option<String>,
    pub table_set_digest: Option<String>,
    pub engine_binary_sha256: Option<String>,
    pub policy_digest: String,
}

fn route_of(f: &FaceDecision) -> Value {
    let class = serde_json::to_value(&f.symbolic.class).unwrap_or(Value::Null);
    let decision = if f.kept.is_some() {
        "symbolic_rule"
    } else if !f.recipe_steps.is_empty() {
        "face_recipe"
    } else {
        "none"
    };
    let status = match (&f.kept, &f.symbolic.class) {
        (Some(_), _) => json!("closed_symbolic_all_layers"),
        (
            None,
            Class::OverCap { .. } | Class::CostlierThanRecipe | Class::OverNodeBudget { .. },
        ) => {
            json!({"candidate_larger_budget_symbolic": class,
                   "note": "large with the current engine (unminimised seeds)"})
        }
        _ => json!("pointwise_or_open (see the closing loop's statement)"),
    };
    json!({"decision": decision, "numbers": class, "candidate": f.symbolic.measures,
           "seed_setting": "policy symbolic.centre (default union_floored)", "status": status})
}

/// One `face_recipe.v1` record of a face.
pub fn face_recipe_record(node: &str, f: &FaceDecision, b: &Binding) -> Value {
    let keys: u64 = f.layers.iter().filter_map(|l| l["observed"].as_u64()).sum();
    let layers: Vec<Value> = f
        .layers
        .iter()
        .map(|l| json!([l["d"], l["r"], l["observed"], l["capacity"]]))
        .collect();
    let cut_by = f.exception.as_ref().map(|x| {
        json!([{"rule": format!("{}#{}", x.source, x.label), "record_sha256": x.record_sha256,
                "branch": format!("exception[{}]", x.branch), "condition": x.case, "keys": f.open_keys}])
    });
    let domain = match &f.exception {
        Some(x) => json!({"case": x.case, "free_dims": x.free_dims, "constrained": x.constrained}),
        None => json!({"signature_only": true}),
    };
    json!({
        "schema": "face_recipe.v1", "node": node, "face_id": f.face,
        "signature": {"dotted": f.sd, "numerators": f.sn},
        "domain": domain,
        "keys_known": {"keys": keys, "layers": layers, "source": "the closure's unserved keys (repairflow P1)"},
        "recipe": {"escalation": f.recipe_steps, "note": f.recipe_note},
        "generic_relation": Value::Null, "exception_loci": [], "solved": Value::Null, "tested": Value::Null,
        "rhs": Value::Null, "coefficient_degree": {"measured": false, "reason": "not computed by the campaign"},
        "cost": Value::Null,
        "produced": {"lane": "implementer repairflow"},
        // OUR ADDITIVE FIELDS
        "route": route_of(f),
        "evidence_layers": f.layers.iter().map(|l| json!({"layer": [l["d"], l["r"]], "capacity": l["capacity"],
            "solved": Value::Null, "proved": Value::Null, "proof_tool": Value::Null, "proof_tool_digest": Value::Null,
            "sampled": Value::Null, "run_time": true})).collect::<Vec<_>>(),
        "bound_to": {"node_identity": b.node_identity, "table_set_digest": b.table_set_digest,
            "engine_binary_sha256": b.engine_binary_sha256, "recipe_engine_digest": Value::Null,
            "recipe_file_sha256": Value::Null, "policy_digest": b.policy_digest},
        "orbit": Value::Null,
        "cut_by": cut_by,
        "cost_per_key": {"apply_ms": Value::Null, "solve_s": Value::Null, "cached_ms": Value::Null,
            "escalation_s": Value::Null, "source": "not measured per face (lane timing table: notes)"},
    })
}

/// The records of every searched face of a node (plain faces explained by exception faces are left out).
pub fn face_recipe_records(d: &NodeDecisions, b: &Binding) -> Vec<Value> {
    d.faces
        .iter()
        .filter(|f| f.symbolic.class != Class::PlainFaceExplained)
        .map(|f| face_recipe_record(&d.node, f, b))
        .collect()
}

/// What a `face_recipe.v1` record says, as the campaign READS it (base identity + our additive fields; unknown fields
/// are kept in `raw`): the round trip of [`face_recipe_record`].
#[derive(Clone, Debug, PartialEq)]
pub struct FaceRecord {
    pub node: Option<String>,
    pub face_id: String,
    pub dotted: Vec<usize>,
    pub numerators: Vec<usize>,
    pub route_decision: Option<String>,
    pub route_status: Value,
    pub cut_by: Value,
    pub bound_to: Value,
    pub evidence_layers: Vec<Value>,
    pub raw: Value,
}

/// Read one `face_recipe.v1` line (ours or the helper's: their records lack our additive fields, which read as null).
pub fn read_face_record(line: &str) -> Result<FaceRecord, String> {
    let v: Value = serde_json::from_str(line).map_err(|e| format!("face record: {e}"))?;
    if v.get("schema")
        .and_then(Value::as_str)
        .is_some_and(|s| s != "face_recipe.v1")
    {
        return Err(format!("face record: schema {}", v["schema"]));
    }
    let slots = |k: &str| -> Result<Vec<usize>, String> {
        serde_json::from_value(v["signature"][k].clone())
            .map_err(|e| format!("face record: signature.{k}: {e}"))
    };
    Ok(FaceRecord {
        node: v["node"].as_str().map(str::to_string),
        face_id: v["face_id"]
            .as_str()
            .or(v["id"].as_str())
            .unwrap_or_default()
            .to_string(),
        dotted: slots("dotted")?,
        numerators: slots("numerators")?,
        route_decision: v["route"]["decision"].as_str().map(str::to_string),
        route_status: v["route"]["status"].clone(),
        cut_by: v["cut_by"].clone(),
        bound_to: v["bound_to"].clone(),
        evidence_layers: v["evidence_layers"].as_array().cloned().unwrap_or_default(),
        raw: v,
    })
}
