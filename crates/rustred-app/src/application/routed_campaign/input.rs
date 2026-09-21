//! Bounded external steering, not source or momentum-map authority.
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use rustred::family::IntegralKey;
use rustred::sector::Mask;
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, Visitor},
};

use crate::{AppError, CandidateOwnerLoadLimits, MAX_CANDIDATE_BUNDLE_BYTES};

pub(super) const MAX_STEERING_BYTES: usize = 16 * 1024 * 1024;
pub(super) const MAX_ROUTES: usize = 100_000;

#[derive(Debug, Deserialize)]
pub(super) struct Selection {
    pub family_fingerprint: String,
    pub owners: Vec<Owner>,
    pub initial_frontier_routes: Vec<Route>,
    #[serde(default, deserialize_with = "unique_limits")]
    pub load_limits: BTreeMap<String, usize>,
}

fn unique_limits<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, usize>, D::Error> {
    struct Limits;
    impl<'de> Visitor<'de> for Limits {
        type Value = BTreeMap<String, usize>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("unique positive integer load limits")
        }
        fn visit_map<M: MapAccess<'de>>(self, mut access: M) -> Result<Self::Value, M::Error> {
            let mut result = BTreeMap::new();
            while let Some((key, value)) = access.next_entry::<String, usize>()? {
                if result.insert(key, value).is_some() {
                    return Err(de::Error::custom("duplicate load limit"));
                }
            }
            Ok(result)
        }
    }
    deserializer.deserialize_map(Limits)
}

#[derive(Debug, Deserialize)]
pub(super) struct Owner {
    pub path: PathBuf,
    pub bytes: u64,
    pub mask: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct Route {
    pub source_mask: String,
    pub owner_mask: String,
    pub requires_transport: bool,
    pub source_to_representative: Vec<Vec<String>>,
    pub owner_to_representative: Vec<Vec<String>>,
}

pub(super) fn mask(text: &str, n: usize) -> Result<Mask, AppError> {
    if text.len() != n || text.bytes().any(|b| b != b'0' && b != b'1') {
        return Err(AppError::input("invalid original-coordinate sector mask"));
    }
    Mask::try_new(text.bytes().map(|b| b == b'1')).map_err(|e| AppError::input(e.to_string()))
}

impl Selection {
    pub fn parse(text: &str) -> Result<(Self, usize, CandidateOwnerLoadLimits), AppError> {
        if text.len() > MAX_STEERING_BYTES {
            return Err(AppError::limit("owner selection exceeds 16 MiB"));
        }
        let selection: Self =
            serde_json::from_str(text).map_err(|e| AppError::input(e.to_string()))?;
        let limits = selection.limits()?;
        let n = selection
            .owners
            .first()
            .ok_or_else(|| AppError::input("no owners"))?
            .mask
            .len();
        if !(1..=16).contains(&n) {
            return Err(AppError::input("owner arity outside supported 1..=16"));
        }
        if selection.owners.len() > limits.bundle.max_collection_entries
            || selection.initial_frontier_routes.len() > MAX_ROUTES
        {
            return Err(AppError::limit(
                "owner or route count exceeds admission limit",
            ));
        }
        let mut owners = BTreeSet::new();
        let mut total = 0usize;
        for owner in &selection.owners {
            mask(&owner.mask, n)?;
            if !owners.insert(&owner.mask) {
                return Err(AppError::input("duplicate owner mask"));
            }
            let size =
                usize::try_from(owner.bytes).map_err(|_| AppError::limit("owner size overflow"))?;
            total = total
                .checked_add(size)
                .ok_or_else(|| AppError::limit("input size overflow"))?;
            if size > limits.bundle.max_bundle_bytes || total > limits.max_total_input_bytes {
                return Err(AppError::limit(
                    "owner bytes exceed declared ingress limits",
                ));
            }
        }
        let mut sources = BTreeSet::new();
        for route in &selection.initial_frontier_routes {
            mask(&route.source_mask, n)?;
            mask(&route.owner_mask, n)?;
            if !owners.contains(&route.owner_mask) || !sources.insert(&route.source_mask) {
                return Err(AppError::input("route owner missing or duplicate source"));
            }
            if !route.requires_transport && route.source_mask != route.owner_mask {
                return Err(AppError::input(
                    "nonidentity route requires native transport",
                ));
            }
            let loops = route.source_to_representative.len();
            if loops == 0 || loops > n || route.owner_to_representative.len() != loops {
                return Err(AppError::input("invalid route matrix dimensions"));
            }
            for matrix in [
                &route.source_to_representative,
                &route.owner_to_representative,
            ] {
                for row in matrix {
                    if row.len() != loops || row.iter().any(|x| x.parse::<i64>().is_err()) {
                        return Err(AppError::input(
                            "route matrices require square signed-integer entries",
                        ));
                    }
                }
            }
        }
        Ok((selection, n, limits))
    }

    fn limits(&self) -> Result<CandidateOwnerLoadLimits, AppError> {
        let mut limits = CandidateOwnerLoadLimits::default();
        for (name, &value) in &self.load_limits {
            if value == 0 {
                return Err(AppError::input("load limits must be positive"));
            }
            match name.as_str() {
                "max_bundle_bytes" => limits.bundle.max_bundle_bytes = value,
                "max_total_input_bytes" => limits.max_total_input_bytes = value,
                "max_total_coefficient_bytes" => limits.bundle.max_total_coefficient_bytes = value,
                "max_collection_entries" => limits.bundle.max_collection_entries = value,
                "max_total_symbolica_state_bytes" => limits.max_total_symbolica_state_bytes = value,
                "max_zero_sector_visits" => limits.max_zero_sector_visits = value,
                "max_coefficient_bytes" => limits.bundle.max_coefficient_bytes = value,
                _ => return Err(AppError::input(format!("unknown load limit: {name}"))),
            }
        }
        if limits.bundle.max_bundle_bytes > MAX_CANDIDATE_BUNDLE_BYTES {
            return Err(AppError::limit(
                "per-owner bundle exceeds the 1 GiB hard ceiling",
            ));
        }
        Ok(limits)
    }
}

pub(super) fn targets(text: &str, n: usize, max: usize) -> Result<Vec<IntegralKey>, AppError> {
    if text.len() > MAX_STEERING_BYTES {
        return Err(AppError::limit("target CSV exceeds 16 MiB"));
    }
    let mut result = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        if result.len() >= max {
            return Err(AppError::limit("target count limit"));
        }
        let powers = line
            .split(',')
            .map(|s| s.trim().parse::<i64>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::input(format!("target CSV: {e}")))?;
        if powers.len() != n {
            return Err(AppError::input("target arity mismatch"));
        }
        result.push(IntegralKey::try_new(powers).map_err(|e| AppError::input(e.to_string()))?);
    }
    if result.is_empty() {
        return Err(AppError::input("no concrete targets"));
    }
    Ok(result)
}
