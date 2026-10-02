//! Typed runtime input for the shared compiled mathematical order.
use rustred::order::{
    CompiledOrder, CoordinateGroups, DegreeRow, Direction, Limits, OrderDescriptor,
};
use rustred::sector::OrderingPolicy;
use serde::{Deserialize, Serialize};

use super::FamilyCandidatesRequest;
use crate::AppError;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateIntegralOrder {
    pub version: u32,
    pub support_weights: Vec<u64>,
    pub support_priority: Vec<usize>,
    /// Absolute physical degree rows evaluated before all support priorities.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pre_support_degree_rows: Vec<CandidateDegreeRow>,
    pub degree_rows: Vec<CandidateDegreeRow>,
    pub coordinate_priority: Vec<usize>,
    pub coordinate_groups: CandidateCoordinateGroups,
    pub active_direction: CandidateOrderDirection,
    pub inactive_direction: CandidateOrderDirection,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateDegreeRow {
    pub active: Vec<u64>,
    pub inactive: Vec<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CandidateCoordinateGroups {
    ActiveFirst,
    InactiveFirst,
    Interleaved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CandidateOrderDirection {
    Ascending,
    Descending,
}

impl CandidateIntegralOrder {
    /// Parse bounded declarative metadata, then compile once outside solver loops.
    pub fn from_json(text: &str) -> Result<CompiledOrder, AppError> {
        if text.len() > crate::MAX_INPUT_BYTES {
            return Err(AppError::limit(
                "integral-order JSON exceeds the input byte limit",
            ));
        }
        let descriptor: Self = serde_json::from_str(text).map_err(|error| {
            AppError::schema(format!("invalid integral-order descriptor: {error}"))
        })?;
        descriptor.compile(Limits::default())
    }

    pub fn compile(self, limits: Limits) -> Result<CompiledOrder, AppError> {
        if self.version != 1 {
            return Err(AppError::schema(
                "unsupported integral-order descriptor version",
            ));
        }
        let direction = |value| match value {
            CandidateOrderDirection::Ascending => Direction::Ascending,
            CandidateOrderDirection::Descending => Direction::Descending,
        };
        CompiledOrder::compile(
            OrderDescriptor {
                pre_support_degree_rows: self
                    .pre_support_degree_rows
                    .into_iter()
                    .map(|row| DegreeRow {
                        active: row.active,
                        inactive: row.inactive,
                    })
                    .collect(),
                support_weights: self.support_weights,
                support_priority: self.support_priority,
                degree_rows: self
                    .degree_rows
                    .into_iter()
                    .map(|row| DegreeRow {
                        active: row.active,
                        inactive: row.inactive,
                    })
                    .collect(),
                coordinate_priority: self.coordinate_priority,
                coordinate_groups: match self.coordinate_groups {
                    CandidateCoordinateGroups::ActiveFirst => CoordinateGroups::ActiveFirst,
                    CandidateCoordinateGroups::InactiveFirst => CoordinateGroups::InactiveFirst,
                    CandidateCoordinateGroups::Interleaved => CoordinateGroups::Interleaved,
                },
                active_direction: direction(self.active_direction),
                inactive_direction: direction(self.inactive_direction),
            },
            limits,
        )
        .map_err(|error| AppError::input(error.to_string()))
    }
}

pub(super) fn request_policy(
    request: &FamilyCandidatesRequest,
    arity: usize,
) -> Result<OrderingPolicy, AppError> {
    if let Some(program) = &request.integral_order {
        if program.arity() != arity || request.permutation.is_some() {
            return Err(AppError::input(
                "integral-order program requires matching family arity and no legacy --permutation",
            ));
        }
        OrderingPolicy::try_programmed(program.clone())
            .map_err(|error| AppError::input(error.to_string()))
    } else {
        super::load::candidate_ordering(arity, request.permutation.as_deref())
    }
}

pub(super) fn saved_policy(
    record: &super::model::ProgramRecord,
) -> Result<OrderingPolicy, AppError> {
    let policy = bound_policy(
        &record.integral_order,
        record.root_sector.len(),
        record.permutation.as_deref(),
    )?;
    if policy.program().is_some() && record.schema == super::model::LEGACY_CANDIDATE_BUNDLE_SCHEMA {
        return Err(AppError::schema(
            "legacy candidate schema cannot carry a programmed integral order",
        ));
    }
    Ok(policy)
}

pub(super) fn bound_policy(
    identity: &str,
    arity: usize,
    permutation: Option<&[usize]>,
) -> Result<OrderingPolicy, AppError> {
    let policy = OrderingPolicy::try_from_stable_id(identity)
        .map_err(|error| AppError::schema(error.to_string()))?;
    let legacy = super::load::candidate_ordering(arity, permutation)?;
    if let Some(program) = policy.program() {
        if program.arity() != arity || permutation.is_some() {
            return Err(AppError::schema(
                "candidate integral-order binding is incompatible with its schema, arity or permutation",
            ));
        }
    } else if policy != legacy {
        return Err(AppError::schema(
            "candidate legacy ordering differs from its coordinate priority",
        ));
    }
    Ok(policy)
}
