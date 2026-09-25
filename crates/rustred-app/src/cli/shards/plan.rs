use super::{CliError, bad};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Shard {
    pub id: usize,
    pub owners: Vec<String>,
    pub query_indices: Vec<usize>,
}

pub(super) fn partition(
    selection: &Value,
    queries: &Value,
    count: usize,
) -> Result<Vec<Shard>, CliError> {
    let owners = selection["owners"]
        .as_array()
        .ok_or_else(|| bad("selection owners must be an array"))?;
    let mut known = BTreeSet::new();
    for row in owners {
        let mask = row["mask"]
            .as_str()
            .ok_or_else(|| bad("owner mask must be a string"))?;
        if mask.is_empty() || mask.bytes().any(|x| x != b'0' && x != b'1') || !known.insert(mask) {
            return Err(bad("owner masks must be unique binary strings"));
        }
    }
    if queries["schema"] != "rustred.owner-domain-queries.json.v2" {
        return Err(bad("unsupported owner query schema"));
    }
    if queries
        .as_object()
        .is_none_or(|fields| fields.keys().any(|key| key != "schema" && key != "queries"))
    {
        return Err(bad("query document contains an unknown field"));
    }
    let rows = queries["queries"]
        .as_array()
        .ok_or_else(|| bad("queries must be an array"))?;
    let mut groups = BTreeMap::<String, Vec<usize>>::new();
    let mut ids = BTreeSet::new();
    for (index, row) in rows.iter().enumerate() {
        let owner = row["owner"]
            .as_str()
            .ok_or_else(|| bad("query owner must be a string"))?;
        let id = row["id"]
            .as_str()
            .ok_or_else(|| bad("query id must be a string"))?;
        if !known.contains(owner) || id.is_empty() || id.len() > 128 || !ids.insert(id) {
            return Err(bad(
                "queries require selected owners and unique nonempty IDs of at most 128 bytes",
            ));
        }
        groups.entry(owner.to_owned()).or_default().push(index);
    }
    let count = if count == 0 { groups.len() } else { count };
    if count == 0 || count > groups.len() {
        return Err(bad(
            "shards must not exceed the number of distinct starting owners",
        ));
    }
    let mut shards: Vec<_> = (0..count)
        .map(|id| Shard {
            id,
            owners: vec![],
            query_indices: vec![],
        })
        .collect();
    for (index, (owner, indices)) in groups.into_iter().enumerate() {
        let shard = &mut shards[index % count];
        shard.owners.push(owner);
        shard.query_indices.extend(indices);
    }
    for shard in &mut shards {
        shard.query_indices.sort_unstable();
    }
    Ok(shards)
}

pub(super) fn shard_queries(queries: &Value, shard: &Shard) -> Value {
    let rows = Value::Array(
        shard
            .query_indices
            .iter()
            .map(|&i| queries["queries"][i].clone())
            .collect(),
    );
    serde_json::json!({"schema":queries["schema"],"queries":rows})
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn owners_keep_auxiliaries_and_exact_query_union() {
        let s = json!({"owners":[{"mask":"01"},{"mask":"10"},{"mask":"11"}]});
        let q = json!({"schema":"rustred.owner-domain-queries.json.v2","queries":[{"id":"a","owner":"01"},{"id":"b","owner":"10"},{"id":"aux-a","owner":"01"},{"id":"c","owner":"11"}]});
        let shards = partition(&s, &q, 2).unwrap();
        let queue = partition(&s, &q, 0).unwrap();
        assert_eq!(queue.len(), 3);
        assert_eq!(queue[0].query_indices, vec![0, 2]);
        assert!(queue.iter().all(|job| job.owners.len() == 1));
        assert_eq!(shards[0].query_indices, vec![0, 2, 3]);
        assert_eq!(shards[1].query_indices, vec![1]);
        assert_eq!(
            shards.iter().map(|s| s.query_indices.len()).sum::<usize>(),
            4
        );
        assert!(partition(&s, &q, 4).is_err());
    }
}
