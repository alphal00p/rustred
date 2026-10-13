//! Keep historical empty requests in provenance, not in executable unions.
use super::*;
use rustred::solver::DomainPowerSummary;

pub(super) fn recorded_domains<const N: usize>(
    text: &str,
    arity: usize,
) -> Result<Vec<SavedApplicationDomain<N>>, AppError> {
    entry::recorded_regions::<N>(text, arity)?
        .into_iter()
        .map(|(id, region)| {
            let summary = DomainPowerSummary::try_new(
                region.support,
                &region.lower,
                &region.upper,
                region.rank,
                region.powers,
            )
            .map_err(failure)?;
            Ok(SavedApplicationDomain {
                id,
                physical_arity: arity,
                is_empty: summary.is_empty(),
                region,
            })
        })
        .collect()
}

pub(super) fn execution_domain<const N: usize>(
    text: &str,
    arity: usize,
) -> Result<(RequestedEntryDomain<N>, usize), AppError> {
    let domains = recorded_domains::<N>(text, arity)?;
    let empty: BTreeSet<_> = domains
        .iter()
        .filter(|row| row.is_empty)
        .map(|row| row.id.as_str())
        .collect();
    if empty.len() == domains.len() {
        return Err(AppError::input(
            "saved application has no nonempty starting domain",
        ));
    }
    if empty.is_empty() {
        return Ok((RequestedEntryDomain::parse_with_arity(text, arity)?, 0));
    }
    let mut document: Value = serde_json::from_str(text).map_err(failure)?;
    document["queries"]
        .as_array_mut()
        .expect("native parser validated query array")
        .retain(|row| !empty.contains(row["id"].as_str().expect("native parser validated ID")));
    if let Some(roles) = document.get_mut("query_roles") {
        for role in ["required", "auxiliary"] {
            if let Some(ids) = roles[role].as_array_mut() {
                ids.retain(|id| {
                    !empty.contains(id.as_str().expect("native parser validated role ID"))
                });
            }
        }
    }
    let domain = RequestedEntryDomain::parse_with_arity(
        &serde_json::to_string(&document).map_err(failure)?,
        arity,
    )?;
    Ok((domain, empty.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn queries() -> Value {
        json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
            {"id":"empty-scalar", "owner":"11", "lower":[0,0], "upper":[2,2],
                "max_numerator_rank":0, "power_bounds":{"max_power_difference":1}},
            {"id":"nonempty", "owner":"10", "lower":[0,0], "upper":[2,2],
                "max_numerator_rank":2, "power_bounds":{"min_power_difference":1,"max_power_difference":1}}
        ],"query_roles":{"required":["empty-scalar","nonempty"],"auxiliary":[]}})
    }

    #[test]
    fn vacuous_history_is_preserved_but_admits_no_inputs() {
        let text = queries().to_string();
        let records = recorded_domains::<2>(&text, 2).unwrap();
        assert_eq!(records.len(), 2);
        assert!(records[0].is_empty);
        assert!(!records[1].is_empty);
        let (execution, empty) = execution_domain::<2>(&text, 2).unwrap();
        assert_eq!(empty, 1);
        assert_eq!(execution.regions().len(), 1);
        execution
            .validate(&IntegralKey::try_new([2, -1]).unwrap())
            .unwrap();
        assert!(
            execution
                .validate(&IntegralKey::try_new([1, 1]).unwrap())
                .is_err()
        );
        assert!(
            execution
                .validate(&IntegralKey::try_new([2, 0]).unwrap())
                .is_err()
        );
    }

    #[test]
    fn entirely_empty_scope_is_not_misreported_as_usable() {
        let mut document = queries();
        document["queries"].as_array_mut().unwrap().pop();
        document["query_roles"]["required"] = json!(["empty-scalar"]);
        assert!(
            execution_domain::<2>(&document.to_string(), 2)
                .unwrap_err()
                .to_string()
                .contains("no nonempty starting domain")
        );
    }

    #[test]
    fn malformed_geometry_is_not_silently_dropped() {
        let mut document = queries();
        document["queries"][0]["lower"] = json!([3, 0]);
        assert!(recorded_domains::<2>(&document.to_string(), 2).is_err());
    }
}
