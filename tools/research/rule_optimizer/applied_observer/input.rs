use rustred::{
    algebra::IndexedGuardLimits,
    solver::{
        DomainPowerBounds, OwnerAppliedCellRefinement, OwnerAppliedLimits, OwnerDomainMatchLimits,
        OwnerDomainRefinementAxes,
    },
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
    num::NonZeroUsize,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema: String,
    pub selection: PathBuf,
    pub queries: PathBuf,
    pub preparation_targets_csv: PathBuf,
    pub owner_base: PathBuf,
    pub workers: usize,
    pub native_limits: NativeLimits,
    pub recorder_limits: RecorderLimits,
}
impl Request {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != "rustred.owner-applied-observer.request.v1"
            || !(1..=50).contains(&self.workers)
        {
            return Err("invalid schema/workers".into());
        }
        self.native_limits.build()?;
        self.recorder_limits.validate()
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeLimits {
    pub applied: BTreeMap<String, usize>,
    pub matching: BTreeMap<String, usize>,
    pub guard: BTreeMap<String, usize>,
    pub refinement_axes: String,
    pub cell_refinement_max_cardinality: Option<usize>,
}
impl NativeLimits {
    pub fn build(&self) -> Result<OwnerAppliedLimits, String> {
        // Every numeric field is mandatory: no unnoticed default/refinement drift.
        macro_rules! fields { ($map:expr,$t:ident,$($field:ident),+ $(,)?) => {{
            let map = $map;
            let names = [$(stringify!($field)),+];
            if map.len() != names.len() || names.iter().any(|name| !map.contains_key(*name)) {
                return Err(format!("incomplete or unknown {} numeric limits", stringify!($t)));
            }
            $t { $($field:map[stringify!($field)]),+, ..Default::default() }
        }}; }
        let guard = fields!(
            &self.guard,
            IndexedGuardLimits,
            max_input_terms,
            max_coefficient_equations,
            max_univariate_degree,
            max_factor_variables,
            max_factor_total_degree,
            max_factor_dense_slots,
            max_total_integer_bits,
            max_factor_recombination_subsets,
            max_gcd_factor_work,
            max_factor_terms,
            max_exact_hyperplane_replay_substitutions,
            max_exact_hyperplane_replay_terms,
            max_exact_hyperplane_replay_work
        );
        let mut matching = fields!(
            &self.matching,
            OwnerDomainMatchLimits,
            max_rules,
            max_terminal_checks,
            max_predicates,
            max_pieces,
            max_cells,
            max_split_operations,
            max_coordinate_cells,
            max_bounded_refinement_cells
        );
        matching.guard_algebra = guard;
        matching.refinement_axes = match self.refinement_axes.as_str() {
            "inactive-only" => OwnerDomainRefinementAxes::InactiveOnly,
            "finite-axes" => OwnerDomainRefinementAxes::FiniteAxes,
            _ => return Err("unknown refinement_axes".into()),
        };
        let mut result = fields!(
            &self.applied,
            OwnerAppliedLimits,
            max_term_visits,
            max_shift_groups,
            max_boundary_cells,
            max_sign_splits,
            max_native_operations,
            max_events,
            max_scratch_terms,
            max_scratch_boxes,
            max_scratch_coordinate_cells
        );
        result.matching = matching;
        result.cell_refinement = match self.cell_refinement_max_cardinality {
            None => OwnerAppliedCellRefinement::Off,
            Some(n) => OwnerAppliedCellRefinement::SingleFiniteAxis {
                max_cardinality: NonZeroUsize::new(n).ok_or("zero refinement cardinality")?,
            },
        };
        Ok(result)
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecorderLimits {
    pub max_input_bytes: usize,
    pub max_queries: usize,
    pub max_events: usize,
    /// All recorder JSON files, including the prewritten binding and final
    /// result. Native coefficient state/atoms have separate caps below.
    pub max_json_bytes: usize,
    pub max_record_bytes: usize,
    pub max_coefficients: usize,
    pub max_atom_bytes: usize,
    pub max_total_atom_bytes: usize,
    pub max_state_bytes: usize,
    pub max_error_bytes: usize,
}
impl RecorderLimits {
    fn validate(&self) -> Result<(), String> {
        if [
            self.max_input_bytes,
            self.max_queries,
            self.max_events,
            self.max_json_bytes,
            self.max_record_bytes,
            self.max_coefficients,
            self.max_atom_bytes,
            self.max_total_atom_bytes,
            self.max_state_bytes,
            self.max_error_bytes,
        ]
        .contains(&0)
            || self.max_record_bytes > self.max_json_bytes
            || self.max_error_bytes > self.max_record_bytes / 4
        {
            return Err("invalid recorder limits".into());
        }
        self.max_total_atom_bytes
            .checked_add(self.max_state_bytes)
            .ok_or("coefficient byte cap overflow")?;
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Powers {
    pub max_positive_power: Option<u64>,
    pub min_power_difference: Option<i64>,
    pub max_power_difference: Option<i64>,
}
impl Powers {
    pub fn native(self) -> DomainPowerBounds {
        DomainPowerBounds {
            max_positive_power: self.max_positive_power,
            min_power_difference: self.min_power_difference,
            max_power_difference: self.max_power_difference,
        }
    }
    pub fn from_native(p: DomainPowerBounds) -> Self {
        Self {
            max_positive_power: p.max_positive_power,
            min_power_difference: p.min_power_difference,
            max_power_difference: p.max_power_difference,
        }
    }
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    pub id: String,
    pub owner: String,
    pub lower: Vec<u64>,
    pub upper: Vec<Option<u64>>,
    pub max_numerator_rank: Option<u32>,
    pub power_bounds: Powers,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Queries {
    pub schema: String,
    pub queries: Vec<Query>,
    pub query_roles: Option<QueryRoles>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryRoles {
    pub required: Vec<String>,
    pub auxiliary: Vec<String>,
}
impl Queries {
    pub fn validate(&self, max: usize) -> Result<usize, String> {
        if self.schema != "rustred.owner-domain-queries.json.v2"
            || self.queries.is_empty()
            || self.queries.len() > max
        {
            return Err("invalid query schema/count".into());
        }
        let n = self.queries[0].owner.len();
        if !(1..=16).contains(&n) {
            return Err("query arity outside 1..16".into());
        }
        let mut ids = BTreeSet::new();
        for q in &self.queries {
            if q.id.is_empty()
                || q.id.len() > 1024
                || !ids.insert(&q.id)
                || q.owner.len() != n
                || q.owner.bytes().any(|b| b != b'0' && b != b'1')
                || q.lower.len() != n
                || q.upper.len() != n
                || q.lower
                    .iter()
                    .zip(&q.upper)
                    .any(|(lo, hi)| hi.is_some_and(|hi| hi < *lo))
            {
                return Err("invalid query identity/box".into());
            }
            q.power_bounds
                .native()
                .validate()
                .map_err(|e| e.to_string())?;
        }
        if let Some(roles) = &self.query_roles {
            let declared: Vec<_> = roles.required.iter().chain(&roles.auxiliary).collect();
            let unique: BTreeSet<_> = declared.iter().copied().collect();
            if unique.len() != declared.len() || unique != ids {
                return Err("query roles must partition exactly all query IDs".into());
            }
        }
        Ok(n)
    }
}
pub fn read_bounded(path: &Path, cap: usize) -> Result<String, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > cap as u64 {
        return Err("input byte cap".into());
    }
    let mut text = String::new();
    file.take(cap.saturating_add(1) as u64)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    if text.len() > cap {
        return Err("input byte cap".into());
    }
    Ok(text)
}
