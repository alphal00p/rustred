use std::collections::BTreeSet;

use rustred::solver::DomainPowerBounds;
use serde_json::Value;

use crate::AppError;

#[derive(Debug)]
pub(in crate::application::routed_campaign) struct Query {
    pub id: String,
    pub owner: Vec<bool>,
    pub lower: Vec<u64>,
    pub upper: Vec<Option<u64>>,
    pub rank: Option<u32>,
    pub powers: DomainPowerBounds,
}

pub(in crate::application::routed_campaign) fn power_bounds_json(p: DomainPowerBounds) -> Value {
    serde_json::json!({"max_positive_power":p.max_positive_power,
        "min_power_difference":p.min_power_difference,"max_power_difference":p.max_power_difference})
}

fn only_fields(value: &Value, allowed: &[&str], context: &str) -> Result<(), AppError> {
    if value
        .as_object()
        .is_none_or(|object| object.keys().any(|k| !allowed.contains(&k.as_str())))
    {
        return Err(AppError::input(format!(
            "unknown field or non-object {context}"
        )));
    }
    Ok(())
}

fn parse_powers(value: Option<&Value>) -> Result<DomainPowerBounds, AppError> {
    let Some(value) = value else {
        return Ok(DomainPowerBounds::default());
    };
    only_fields(
        value,
        &[
            "max_positive_power",
            "min_power_difference",
            "max_power_difference",
        ],
        "power_bounds",
    )?;
    let signed = |key| match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_i64().map(Some).ok_or_else(|| {
            AppError::input(format!("{key} must be a signed 64-bit integer or null"))
        }),
    };
    let powers = DomainPowerBounds {
        max_positive_power: match value.get("max_positive_power") {
            None | Some(Value::Null) => None,
            Some(v) => Some(v.as_u64().ok_or_else(|| {
                AppError::input("max_positive_power must be an unsigned 64-bit integer or null")
            })?),
        },
        min_power_difference: signed("min_power_difference")?,
        max_power_difference: signed("max_power_difference")?,
    };
    powers
        .validate()
        .map_err(|e| AppError::input(e.to_string()))?;
    Ok(powers)
}

/// Validate every query before loading native rule programs.
pub(in crate::application::routed_campaign) fn parse(
    text: &str,
    arity: usize,
    limit: usize,
) -> Result<Vec<Query>, AppError> {
    if text.len() > 1024 * 1024 {
        return Err(AppError::input("owner-domain query input exceeds 1 MiB"));
    }
    let document: Value = serde_json::from_str(text)
        .map_err(|error| AppError::input(format!("owner-domain query JSON: {error}")))?;
    if document["schema"] != "rustred.owner-domain-queries.json.v2" {
        return Err(AppError::input("unsupported owner-domain query schema"));
    }
    only_fields(&document, &["schema", "queries"], "query document")?;
    let rows = document["queries"]
        .as_array()
        .ok_or_else(|| AppError::input("queries must be an array"))?;
    if rows.is_empty() || rows.len() > limit {
        return Err(AppError::input(
            "query count must be positive and within its allowance",
        ));
    }
    let mut ids = BTreeSet::new();
    rows.iter()
        .map(|row| {
            only_fields(
                row,
                &[
                    "id",
                    "owner",
                    "lower",
                    "upper",
                    "max_numerator_rank",
                    "power_bounds",
                ],
                "query",
            )?;
            let id = row["id"]
                .as_str()
                .filter(|id| !id.is_empty() && id.len() <= 128)
                .ok_or_else(|| AppError::input("query id must contain 1..=128 UTF-8 bytes"))?;
            if !ids.insert(id) {
                return Err(AppError::input("query ids must be unique"));
            }
            let owner = row["owner"]
                .as_str()
                .filter(|bits| bits.len() == arity && bits.bytes().all(|b| b == b'0' || b == b'1'))
                .ok_or_else(|| AppError::input("query owner must be an arity-sized binary mask"))?;
            let array = |name: &str| {
                row[name]
                    .as_array()
                    .filter(|a| a.len() == arity)
                    .ok_or_else(|| AppError::input(format!("query {name} must have family arity")))
            };
            let lower: Vec<_> = array("lower")?
                .iter()
                .map(|n| {
                    n.as_u64().ok_or_else(|| {
                        AppError::input("query lower bounds must be unsigned integers")
                    })
                })
                .collect::<Result<_, _>>()?;
            let upper: Vec<_> = array("upper")?
                .iter()
                .map(|n| {
                    if n.is_null() {
                        Ok(None)
                    } else {
                        n.as_u64().map(Some).ok_or_else(|| {
                            AppError::input("query upper bounds must be unsigned integers or null")
                        })
                    }
                })
                .collect::<Result<_, _>>()?;
            if lower
                .iter()
                .zip(&upper)
                .any(|(&lo, &hi)| hi.is_some_and(|hi| hi < lo))
            {
                return Err(AppError::input("query lower bound exceeds upper bound"));
            }
            let rank = match row.get("max_numerator_rank") {
                Some(Value::Null) => None,
                Some(value) => Some(
                    value
                        .as_u64()
                        .and_then(|n| u32::try_from(n).ok())
                        .ok_or_else(|| {
                            AppError::input("query rank must be an unsigned 32-bit integer or null")
                        })?,
                ),
                None => {
                    return Err(AppError::input(
                        "query rank must be explicit (null means unbounded)",
                    ));
                }
            };
            Ok(Query {
                id: id.into(),
                owner: owner.bytes().map(|b| b == b'1').collect(),
                lower,
                upper,
                rank,
                powers: parse_powers(row.get("power_bounds"))?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn power_predicates_are_admitted_exactly_and_unknown_fields_fail_closed() {
        let base = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
            {"id":"q", "owner":"10", "lower":[0,0], "upper":[null,null],
             "max_numerator_rank":11, "power_bounds":{"max_positive_power":24,
             "min_power_difference":-3,"max_power_difference":10}}]});
        let row = parse(&base.to_string(), 2, 1).unwrap().remove(0);
        assert_eq!(
            row.powers,
            DomainPowerBounds {
                max_positive_power: Some(24),
                min_power_difference: Some(-3),
                max_power_difference: Some(10)
            }
        );
        assert_eq!(
            power_bounds_json(row.powers),
            base["queries"][0]["power_bounds"]
        );
        for (field, value) in [
            ("max_positive_power", json!(-1)),
            ("min_power_difference", json!(11)),
            ("max_power_difference", json!(-4)),
            ("min_power_difference", json!(u64::MAX)),
            ("unexpected", json!(1)),
        ] {
            let mut doc = base.clone();
            doc["queries"][0]["power_bounds"][field] = value;
            assert!(parse(&doc.to_string(), 2, 1).is_err(), "{field}");
        }
        let mut unknown = base.clone();
        unknown["queries"][0]["max_positive_power"] = json!(24);
        assert!(parse(&unknown.to_string(), 2, 1).is_err());
        let mut old = base.clone();
        old["schema"] = json!("rustred.owner-domain-queries.json.v1");
        assert!(parse(&old.to_string(), 2, 1).is_err());
        let mut absent = base;
        absent["queries"][0]
            .as_object_mut()
            .unwrap()
            .remove("power_bounds");
        assert!(
            parse(&absent.to_string(), 2, 1).unwrap()[0]
                .powers
                .is_unconstrained()
        );
    }

    #[test]
    fn queries_retain_unbounded_positive_bounds_and_actual_rank() {
        let doc = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
            {"id":"above-entry", "owner":"10", "lower":[0,11], "upper":[null,11], "max_numerator_rank":11}]});
        let rows = parse(&doc.to_string(), 2, 1).unwrap();
        assert_eq!(rows[0].lower, [0, 11]);
        assert_eq!(rows[0].upper, [None, Some(11)]);
        assert_eq!(rows[0].rank, Some(11));
    }

    #[test]
    fn invalid_queries_fail_before_native_preparation() {
        let base = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
            {"id":"q", "owner":"10", "lower":[0,0], "upper":[null,0], "max_numerator_rank":0}]});
        for (field, value) in [
            ("id", json!("")),
            ("owner", json!("11x")),
            ("lower", json!([-1, 0])),
            ("upper", json!([null])),
            ("max_numerator_rank", json!(-1)),
            ("max_numerator_rank", json!(4294967296u64)),
        ] {
            let mut doc = base.clone();
            doc["queries"][0][field] = value;
            assert!(parse(&doc.to_string(), 2, 1).is_err(), "{field}");
        }
        let mut duplicate = base.clone();
        duplicate["queries"]
            .as_array_mut()
            .unwrap()
            .push(base["queries"][0].clone());
        assert!(parse(&duplicate.to_string(), 2, 2).is_err());
        assert!(parse(&base.to_string(), 2, 0).is_err());
        let mut missing = base.clone();
        missing["queries"][0]
            .as_object_mut()
            .unwrap()
            .remove("max_numerator_rank");
        assert!(parse(&missing.to_string(), 2, 1).is_err());
        let mut inverted = base.clone();
        inverted["queries"][0]["lower"] = json!([0, 1]);
        assert!(parse(&inverted.to_string(), 2, 1).is_err());
        let mut unbounded = base;
        unbounded["queries"][0]["max_numerator_rank"] = Value::Null;
        assert_eq!(parse(&unbounded.to_string(), 2, 1).unwrap()[0].rank, None);
    }
}
