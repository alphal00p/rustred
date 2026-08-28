use std::collections::{BTreeMap, BTreeSet};

use rustred::{
    CoefficientLocation, GuardOrigin, IntegralFamily, ParametricNonZeroCondition,
    ParametricRelation, ParametricRowId, ScalarProductCoordinate,
};
use serde::Serialize;
use symbolica::prelude::AtomCore;

use crate::application::model::MetadataValue;
use crate::application::producer::ProducerOutputV1;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct DeriveOutputV1 {
    pub(super) schema: &'static str,
    pub(super) status: &'static str,
    pub(super) equation_convention: &'static str,
    pub(super) relation_selection: &'static str,
    pub(super) target_disposition: &'static str,
    pub(super) producer: ProducerOutputV1,
    pub(super) provenance: ProvenanceOutputV1,
    pub(super) family: FamilyOutputV1,
    pub(super) target: TargetOutputV1,
    pub(super) coordinates: Vec<CoordinateOutputV1>,
    pub(super) denominators: Vec<DenominatorOutputV1>,
    pub(super) external_gram: Vec<ExternalGramOutputV1>,
    pub(super) domain_conditions: Vec<ConditionOutputV1>,
    pub(super) relation_counts: RelationCountsOutputV1,
    pub(super) relations: Vec<RelationOutputV1>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct ProvenanceOutputV1 {
    pub(super) requested_input_format: &'static str,
    pub(super) detected_input_form: &'static str,
    pub(super) input_schema: String,
    pub(super) parameter_source: String,
    pub(super) input_parameters: Vec<String>,
    pub(super) canonical_integral: String,
    pub(super) metadata: BTreeMap<String, MetadataValue>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct FamilyOutputV1 {
    pub(super) name: String,
    pub(super) fingerprint: String,
    pub(super) parametric_context_fingerprint: String,
    pub(super) parameters: Vec<String>,
    pub(super) dimension: String,
    pub(super) loop_momenta: Vec<String>,
    pub(super) external_momenta: Vec<String>,
    pub(super) denominator_count: usize,
    pub(super) index_symbols: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct TargetOutputV1 {
    pub(super) present: bool,
    pub(super) disposition: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) powers: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) numerator: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct CoordinateOutputV1 {
    pub(super) ordinal: usize,
    pub(super) kind: &'static str,
    pub(super) left: String,
    pub(super) right: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct DenominatorOutputV1 {
    pub(super) ordinal: usize,
    pub(super) id: String,
    pub(super) source_expression: String,
    pub(super) normalized_expression: String,
    pub(super) power_shift: String,
    pub(super) constant: String,
    pub(super) coefficients: Vec<AffineCoefficientOutputV1>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct AffineCoefficientOutputV1 {
    pub(super) coordinate: usize,
    pub(super) value: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct ExternalGramOutputV1 {
    pub(super) row: usize,
    pub(super) column: usize,
    pub(super) left: String,
    pub(super) right: String,
    pub(super) value: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ConditionOutputV1 {
    pub(super) expression: String,
    pub(super) sources: Vec<String>,
    pub(super) origins: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct RelationCountsOutputV1 {
    pub(super) generated_ordinary: usize,
    pub(super) generated_li: usize,
    pub(super) emitted_ordinary: usize,
    pub(super) emitted_li: usize,
    pub(super) emitted_total: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct RelationOutputV1 {
    pub(super) ordinal: usize,
    pub(super) stable_id: String,
    pub(super) id: RowIdOutputV1,
    pub(super) terms: Vec<RelationTermOutputV1>,
    pub(super) nonzero_conditions: Vec<ConditionOutputV1>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct RowIdOutputV1 {
    pub(super) kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) contraction_momentum: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) differentiated_loop: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) first_external: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) second_external: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) label: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct RelationTermOutputV1 {
    pub(super) shift: Vec<i64>,
    pub(super) coefficient: String,
}

pub(super) fn target_output(normalized: &rustred::NormalizedProjectInputV1) -> TargetOutputV1 {
    let target = normalized.target();
    TargetOutputV1 {
        present: true,
        disposition: target.derive_disposition(),
        powers: Some(target.powers().to_vec()),
        numerator: Some(target.numerator().to_canonical_string()),
    }
}

pub(super) fn coordinate_outputs(family: &IntegralFamily) -> Vec<CoordinateOutputV1> {
    family
        .coordinates()
        .iter()
        .copied()
        .enumerate()
        .map(|(ordinal, coordinate)| match coordinate {
            ScalarProductCoordinate::LoopLoop { left, right } => {
                let left_label = family.loop_momenta()[left].clone();
                let right_label = family.loop_momenta()[right].clone();
                CoordinateOutputV1 {
                    ordinal,
                    kind: "loop_loop",
                    left: left_label,
                    right: right_label,
                }
            }
            ScalarProductCoordinate::LoopExternal {
                loop_index,
                external_index,
            } => {
                let left_label = family.loop_momenta()[loop_index].clone();
                let right_label = family.external_momenta()[external_index].clone();
                CoordinateOutputV1 {
                    ordinal,
                    kind: "loop_external",
                    left: left_label,
                    right: right_label,
                }
            }
        })
        .collect()
}

pub(super) fn denominator_outputs(
    family: &IntegralFamily,
    records: &[rustred::LoweredSymbolicaDenominatorV1],
) -> Vec<DenominatorOutputV1> {
    records
        .iter()
        .zip(family.denominators())
        .zip(family.power_shifts())
        .enumerate()
        .map(
            |(ordinal, ((record, denominator), power_shift))| DenominatorOutputV1 {
                ordinal,
                id: record.id().to_owned(),
                source_expression: record.source().to_canonical_string(),
                normalized_expression: record.normalized_expression().to_canonical_string(),
                power_shift: power_shift.to_expression().to_canonical_string(),
                constant: denominator.constant().to_expression().to_canonical_string(),
                coefficients: denominator
                    .coefficients()
                    .iter()
                    .enumerate()
                    .map(|(coordinate, coefficient)| AffineCoefficientOutputV1 {
                        coordinate,
                        value: coefficient.to_expression().to_canonical_string(),
                    })
                    .collect(),
            },
        )
        .collect()
}

pub(super) fn external_gram_outputs(family: &IntegralFamily) -> Vec<ExternalGramOutputV1> {
    let mut output = Vec::new();
    for (row, values) in family.external_gram().iter().enumerate() {
        for (column, value) in values.iter().enumerate() {
            output.push(ExternalGramOutputV1 {
                row,
                column,
                left: family.external_momenta()[row].clone(),
                right: family.external_momenta()[column].clone(),
                value: value.to_expression().to_canonical_string(),
            });
        }
    }
    output
}

pub(super) fn family_domain_outputs(family: &IntegralFamily) -> Vec<ConditionOutputV1> {
    let mut merged: BTreeMap<String, (BTreeSet<String>, BTreeSet<String>)> = BTreeMap::new();
    for condition in family.domain().conditions() {
        let expression = condition.polynomial().to_expression().to_canonical_string();
        let entry = merged.entry(expression).or_default();
        entry.0.insert(coefficient_location(&condition.source()));
        entry
            .1
            .extend(condition.origins().iter().map(GuardOrigin::stable_string));
    }
    merged
        .into_iter()
        .map(|(expression, (sources, origins))| ConditionOutputV1 {
            expression,
            sources: sources.into_iter().collect(),
            origins: origins.into_iter().collect(),
        })
        .collect()
}

fn coefficient_location(location: &CoefficientLocation) -> String {
    location.stable_string()
}

pub(super) fn relation_output(ordinal: usize, relation: &ParametricRelation) -> RelationOutputV1 {
    RelationOutputV1 {
        ordinal,
        stable_id: relation.row_id().stable_string(),
        id: row_id_output(relation.row_id()),
        terms: relation
            .terms()
            .iter()
            .map(|(shift, coefficient)| RelationTermOutputV1 {
                shift: shift.values().to_vec(),
                coefficient: coefficient.to_expression().to_canonical_string(),
            })
            .collect(),
        nonzero_conditions: relation_conditions(relation.guarded_nonzero_conditions()),
    }
}

fn row_id_output(row: &ParametricRowId) -> RowIdOutputV1 {
    match row {
        ParametricRowId::OrdinaryIbp {
            contraction_momentum,
            differentiated_loop,
        } => RowIdOutputV1 {
            kind: "ordinary_ibp",
            contraction_momentum: Some(*contraction_momentum),
            differentiated_loop: Some(*differentiated_loop),
            first_external: None,
            second_external: None,
            label: None,
        },
        ParametricRowId::LorentzInvariance {
            first_external,
            second_external,
        } => RowIdOutputV1 {
            kind: "lorentz_invariance",
            contraction_momentum: None,
            differentiated_loop: None,
            first_external: Some(*first_external),
            second_external: Some(*second_external),
            label: None,
        },
        ParametricRowId::Derived { label } => RowIdOutputV1 {
            kind: "derived",
            contraction_momentum: None,
            differentiated_loop: None,
            first_external: None,
            second_external: None,
            label: Some(label.to_string()),
        },
    }
}

fn relation_conditions(conditions: &[ParametricNonZeroCondition]) -> Vec<ConditionOutputV1> {
    let mut merged: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for condition in conditions {
        merged
            .entry(condition.polynomial().to_expression().to_canonical_string())
            .or_default()
            .extend(condition.origins().iter().map(GuardOrigin::stable_string));
    }
    merged
        .into_iter()
        .map(|(expression, origins)| ConditionOutputV1 {
            expression,
            sources: Vec::new(),
            origins: origins.into_iter().collect(),
        })
        .collect()
}
