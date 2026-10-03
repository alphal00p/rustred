//! Display-only trace compaction. Native candidates and proof inputs never enter
//! this module: it receives already rendered attempt records after checking.
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Detail {
    Full,
    Summary,
}

pub fn detail(request: &Value) -> Result<Detail, String> {
    match request.get("trace_detail") {
        None => Ok(Detail::Full),
        Some(Value::String(value)) if value == "full" => Ok(Detail::Full),
        Some(Value::String(value)) if value == "summary" => Ok(Detail::Summary),
        _ => Err("trace_detail must be full or summary".into()),
    }
}

fn counts(attempt: &mut Value) {
    for (payload, count) in [
        ("normalized_full_product", "full_product_term_count"),
        ("conditions", "retained_condition_count"),
        ("ordinary_contributions", "ordinary_contribution_count"),
    ] {
        attempt[count] = json!(attempt.get(payload).and_then(Value::as_array).map(Vec::len));
    }
}

fn compact(attempt: &mut Value) {
    // This is the exact displayed u*A entry, not a parsed/recomputed RHS
    // coefficient. The rule RHS has the opposite sign of this product tail.
    if attempt["proof_error"]["kind"] == "UNPROVED_DESCENT_OBLIGATION" {
        let term = attempt["proof_error"]
            .get("shift")
            .and_then(|shift| {
                attempt["normalized_full_product"]
                    .as_array()
                    .and_then(|terms| terms.iter().find(|term| &term["shift"] == shift))
            })
            .cloned();
        let Some(term) = term else {
            // Fail closed for diagnostic retention: never drop the circuit if
            // its native typed witness cannot be bound to the rendered product.
            attempt["trace_compaction_refused"] = json!("missing failed-shift product term");
            return;
        };
        attempt["failed_shift_full_product_term"] = term;
    }
    let object = attempt
        .as_object_mut()
        .expect("attempt records are objects");
    for key in [
        "normalized_full_product",
        "conditions",
        "ordinary_contributions",
    ] {
        object.remove(key);
    }
    object.insert("trace_compacted".into(), json!(true));
}

pub fn append(attempts: &mut Vec<Value>, mut attempt: Value, detail: Detail) {
    if detail == Detail::Summary {
        counts(&mut attempt);
        if attempt.get("normalized_full_product").is_some() {
            // Compact a circuit only when another actual circuit replaces it.
            // A terminal NoTarget/budget record must keep the last circuit full
            // as well as its own final record, for independent inspection.
            if let Some(previous) = attempts
                .iter_mut()
                .rev()
                .find(|previous| previous.get("normalized_full_product").is_some())
            {
                compact(previous);
            }
        }
    }
    attempts.push(attempt);
}
