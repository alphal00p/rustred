use super::super::matching::input::{self, Query};
use crate::AppError;
use serde_json::Value;

pub(super) struct GuardedQuery {
    pub query: Query,
    pub batch: usize,
    pub rule: usize,
}

pub(super) fn parse(text: &str, arity: usize, limit: usize) -> Result<Vec<GuardedQuery>, AppError> {
    if text.len() > 1024 * 1024 {
        return Err(AppError::input("guarded queries exceed 1 MiB"));
    }
    let mut doc: Value = serde_json::from_str(text)
        .map_err(|e| AppError::input(format!("guarded query JSON: {e}")))?;
    if doc["schema"] != "rustred.owner-guarded-rule-queries.json.v1" {
        return Err(AppError::input("unsupported guarded-rule query schema"));
    }
    if doc.as_object().is_none_or(|o| {
        o.keys()
            .any(|k| !["schema", "queries"].contains(&k.as_str()))
    }) {
        return Err(AppError::input("unknown guarded query document field"));
    }
    let rows = doc["queries"]
        .as_array()
        .ok_or_else(|| AppError::input("queries must be an array"))?;
    if rows.is_empty() || rows.len() > limit {
        return Err(AppError::input(
            "guarded query count exceeds the positive allowance",
        ));
    }
    let selectors = rows
        .iter()
        .map(|row| {
            let ordinal = |key| {
                row[key]
                    .as_u64()
                    .and_then(|v| usize::try_from(v).ok())
                    .ok_or_else(|| {
                        AppError::input(format!("query {key} must be an explicit unsigned ordinal"))
                    })
            };
            Ok((ordinal("batch")?, ordinal("rule")?))
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    // Reuse the existing local-coordinate/rank admission; no caller guard is
    // accepted or converted into native applicability authority.
    for row in rows {
        let object = row
            .as_object()
            .ok_or_else(|| AppError::input("query must be an object"))?;
        if object.keys().any(|k| {
            ![
                "id",
                "owner",
                "lower",
                "upper",
                "max_numerator_rank",
                "batch",
                "rule",
            ]
            .contains(&k.as_str())
        }) {
            return Err(AppError::input(
                "unknown guarded query field (guards cannot be supplied)",
            ));
        }
    }
    // This explicitly unconstrained adapter rejects power_bounds above. Strip
    // selectors before the strict general-domain parser, not unknown fields.
    for row in doc["queries"].as_array_mut().expect("validated rows") {
        let object = row.as_object_mut().expect("validated object");
        object.remove("batch");
        object.remove("rule");
    }
    doc["schema"] = Value::String("rustred.owner-domain-queries.json.v2".into());
    // Guarded display retains its independent 1 MiB/10k admission policy.
    let queries = input::parse(&doc.to_string(), arity, limit, 1024 * 1024)?;
    Ok(queries
        .into_iter()
        .zip(selectors)
        .map(|(query, (batch, rule))| GuardedQuery { query, batch, rule })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn guarded_query_selectors_and_actual_rank_are_explicit() {
        let mut value = json!({"schema":"rustred.owner-guarded-rule-queries.json.v1","queries":[
            {"id":"q","owner":"10","batch":0,"rule":239,"lower":[1,11],"upper":[null,11],"max_numerator_rank":11}]});
        let out = parse(&value.to_string(), 2, 1).unwrap();
        assert_eq!(
            (out[0].batch, out[0].rule, out[0].query.rank),
            (0, 239, Some(11))
        );
        value["queries"][0]["max_numerator_rank"] = Value::Null;
        assert_eq!(parse(&value.to_string(), 2, 1).unwrap()[0].query.rank, None);
        for (key, bad) in [
            ("batch", json!(-1)),
            ("rule", Value::Null),
            ("lower", json!([-1, 0])),
            ("guards", json!([])),
        ] {
            let mut v = value.clone();
            v["queries"][0][key] = bad;
            assert!(parse(&v.to_string(), 2, 1).is_err(), "{key}");
        }
    }
}
