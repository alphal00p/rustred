//! Canonical wide source-parent sharing and exact mathematical cell payloads.
//! These decoded descriptions carry no replay or installation authority.
use std::collections::BTreeMap;

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial};
use crate::foundry::cell::{FixedIndexRestriction, RuleCell, SourceViewConstruction};
use crate::foundry::completion::LatticeBox;
use crate::identity::{IndexShift, IntegralShift, TranslatedSourceRequest};

use super::super::super::error::ArtifactPersistenceError;
use super::super::super::model::ClosedArtifact;
use super::super::binary::{Reader, Writer, check_limit, try_vec};
use super::super::coefficient::{
    decode_indexed_coefficient, decode_indexed_polynomial, encode_indexed_coefficient,
    encode_indexed_polynomial,
};
use super::super::semantic::{
    decode_bool_vec, decode_i64_vec, encode_bool_slice, encode_i64_slice,
};

pub(super) const COMBINED_ORIGINAL_PLAN: u16 = 0x701;

pub(super) struct ParentPlan {
    pub fixed: Vec<FixedIndexRestriction>,
    pub requests: Vec<(TranslatedSourceRequest, IndexedCoefficient)>,
    pub conditions: Vec<IndexedPolynomial>,
}

pub(super) struct CellPlan {
    pub parent: usize,
    pub sector: Vec<bool>,
    pub application: LatticeBox,
    pub rhs: Vec<(IndexShift, IndexedCoefficient)>,
}

fn invalid(field: &'static str) -> ArtifactPersistenceError {
    ArtifactPersistenceError::SemanticMismatch { field }
}

// A cheap, exact structural key locates candidates. Native weight and guard
// equality resolves its buckets; no hash, Arc address or completion order is
// ever persisted or used as semantic identity.
type ParentKey = (Vec<(usize, i64)>, Vec<(usize, Vec<i64>)>);

fn parent_key(
    artifact: &ClosedArtifact,
    cell: &RuleCell,
) -> Result<ParentKey, ArtifactPersistenceError> {
    let rule = cell.rule();
        let evidence = rule
            .replay_evidence()
            .combined_original_domain()
            .ok_or_else(|| invalid("combined source parent evidence"))?;
        if evidence.affine_application_domain().is_some() {
            return Err(invalid("affine combined-domain evidence is not serializable yet"));
        }
    if evidence.affine_application_domain().is_some() {
        return Err(invalid("affine combined domains require the affine codec"));
    }
    if !matches!(
        cell.sources().construction(),
        SourceViewConstruction::Direct
    ) || evidence.application_boxes().len() != 1
        || cell.sources().relations().len() != rule.source_combination().len()
    {
        return Err(invalid("combined original source parent shape"));
    }
    let fixed = cell
        .fixed_restrictions()
        .iter()
        .map(|item| (item.position(), item.value()))
        .collect();
    let mut requests = Vec::with_capacity(rule.source_combination().len());
    for (ordinal, contribution) in rule.source_combination().iter().enumerate() {
        if contribution.source_ordinal() != ordinal {
            return Err(invalid("combined original source chronology"));
        }
        let provenance = cell
            .sources()
            .provenance()
            .get(ordinal)
            .ok_or_else(|| invalid("combined original source provenance"))?
            .translated();
        let original = artifact
            .source_relations()
            .get(provenance.source_ordinal())
            .ok_or_else(|| invalid("combined canonical source ordinal"))?;
        if original.row_id() != provenance.source_row()
            || cell.sources().relations()[ordinal].row_id() != contribution.row_id()
        {
            return Err(invalid("combined canonical source RowId"));
        }
        requests.push((
            provenance.source_ordinal(),
            provenance.offset().values().to_vec(),
        ));
    }
    Ok((fixed, requests))
}

fn same_parent(left: &RuleCell, right: &RuleCell) -> bool {
    left.rule()
        .source_combination()
        .iter()
        .map(|term| term.coefficient())
        .eq(right
            .rule()
            .source_combination()
            .iter()
            .map(|term| term.coefficient()))
        && left
            .rule()
            .nonzero_guards()
            .iter()
            .map(|guard| guard.polynomial())
            .eq(right
                .rule()
                .nonzero_guards()
                .iter()
                .map(|guard| guard.polynomial()))
}

pub(super) fn encode(
    writer: &mut Writer,
    artifact: &ClosedArtifact,
) -> Result<(), ArtifactPersistenceError> {
    let mut buckets: BTreeMap<ParentKey, Vec<usize>> = BTreeMap::new();
    let mut parents: Vec<(&RuleCell, ParentKey)> = Vec::new();
    let mut cells = Vec::with_capacity(artifact.rule_cells().len());
    check_limit(
        "combined cells",
        artifact.rule_cells().len(),
        writer.limits().max_collection_entries,
    )?;
    for cell in artifact.rule_cells() {
        let key = parent_key(artifact, cell)?;
        let existing = buckets.get(&key).and_then(|indices| {
            indices
                .iter()
                .copied()
                .find(|index| same_parent(parents[*index].0, cell))
        });
        let ordinal = if let Some(ordinal) = existing {
            ordinal
        } else {
            let ordinal = parents.len();
            buckets.entry(key.clone()).or_default().push(ordinal);
            parents.push((cell, key));
            ordinal
        };
        cells.push(ordinal);
    }
    writer.u16(COMBINED_ORIGINAL_PLAN)?;
    writer.usize(parents.len(), "combined source parents")?;
    for (cell, (fixed, requests)) in &parents {
        writer.usize(fixed.len(), "combined fixed target coordinates")?;
        for (axis, value) in fixed {
            writer.usize(*axis, "fixed target axis")?;
            writer.i64(*value)?;
        }
        writer.usize(requests.len(), "combined original source requests")?;
        for ((ordinal, offset), contribution) in
            requests.iter().zip(cell.rule().source_combination())
        {
            writer.usize(*ordinal, "original source ordinal")?;
            encode_i64_slice(writer, offset)?;
            encode_indexed_coefficient(writer, contribution.coefficient())?;
        }
        writer.usize(
            cell.rule().nonzero_guards().len(),
            "combined retained conditions",
        )?;
        for guard in cell.rule().nonzero_guards() {
            encode_indexed_polynomial(writer, guard.polynomial())?;
        }
    }
    writer.usize(cells.len(), "combined cells")?;
    for (cell, parent) in artifact.rule_cells().iter().zip(cells) {
        writer.usize(parent, "combined cell parent")?;
        encode_bool_slice(writer, cell.rule().sector().active_bits())?;
        let evidence = cell
            .rule()
            .replay_evidence()
            .combined_original_domain()
            .ok_or_else(|| invalid("combined cell evidence"))?;
        if evidence.affine_application_domain().is_some() {
            return Err(invalid("affine combined-domain evidence is not serializable yet"));
        }
        if evidence.affine_application_domain().is_some() {
            return Err(invalid("affine combined domains require the affine codec"));
        }
        encode_box(writer, &evidence.application_boxes()[0])?;
        writer.usize(cell.rule().right_hand_side().len(), "combined RHS terms")?;
        for term in cell.rule().right_hand_side() {
            encode_i64_slice(writer, term.shift().values())?;
            encode_indexed_coefficient(writer, term.coefficient())?;
        }
    }
    Ok(())
}

pub(super) fn encode_box(
    writer: &mut Writer,
    piece: &LatticeBox,
) -> Result<(), ArtifactPersistenceError> {
    writer.usize(piece.arity(), "mathematical box arity")?;
    for (lower, upper) in piece.lower().iter().zip(piece.upper()) {
        writer.u64(*lower)?;
        match upper {
            None => writer.u8(0)?,
            Some(upper) => {
                writer.u8(1)?;
                writer.u64(*upper)?;
            }
        }
    }
    Ok(())
}

fn decode_box(
    reader: &mut Reader<'_>,
    arity: usize,
) -> Result<LatticeBox, ArtifactPersistenceError> {
    if reader.count("mathematical box arity")? != arity {
        return Err(invalid("mathematical box arity"));
    }
    let mut lower = try_vec(arity, "mathematical lower endpoints")?;
    let mut upper = try_vec(arity, "mathematical upper endpoints")?;
    for _ in 0..arity {
        lower.push(reader.u64()?);
        upper.push(match reader.u8()? {
            0 => None,
            1 => Some(reader.u64()?),
            _ => return Err(invalid("mathematical infinity tag")),
        });
    }
    LatticeBox::try_new(lower, upper).map_err(|_| invalid("mathematical box bounds"))
}

pub(super) fn decode(
    reader: &mut Reader<'_>,
    context: &IndexedCoefficientContext,
    original_count: usize,
) -> Result<(Vec<ParentPlan>, Vec<CellPlan>), ArtifactPersistenceError> {
    if reader.u16()? != COMBINED_ORIGINAL_PLAN {
        return Err(invalid("combined original plan tag"));
    }
    let arity = context.index_count();
    let parent_count = reader.count("combined source parents")?;
    if parent_count == 0 {
        return Err(invalid("empty combined source parents"));
    }
    let mut parents = try_vec(parent_count, "combined source parents")?;
    let mut aggregate_requests = 0usize;
    for _ in 0..parent_count {
        let count = reader.count("combined fixed target coordinates")?;
        check_limit(
            "combined fixed target coordinates",
            count,
            reader.limits().rule_cells.max_fixed_restrictions,
        )?;
        let mut fixed: Vec<FixedIndexRestriction> =
            try_vec(count, "combined fixed target coordinates")?;
        for _ in 0..count {
            let axis = reader.count("fixed target axis")?;
            let value = reader.i64()?;
            if axis >= arity || fixed.last().is_some_and(|last| last.position() >= axis) {
                return Err(invalid("combined fixed target coordinate order"));
            }
            fixed.push(FixedIndexRestriction::new(axis, value));
        }
        let count = reader.count("combined original source requests")?;
        if count == 0 {
            return Err(invalid("empty combined source request"));
        }
        aggregate_requests = aggregate_requests.checked_add(count).ok_or(
            ArtifactPersistenceError::ResourceCountOverflow {
                resource: "aggregate combined source requests",
            },
        )?;
        check_limit(
            "aggregate combined source requests",
            aggregate_requests,
            reader
                .limits()
                .translated_sources
                .max_requested_source_translations,
        )?;
        check_limit(
            "combined source weights",
            count,
            reader.limits().rule_derivation.max_source_combination_terms,
        )?;
        let mut requests: Vec<(TranslatedSourceRequest, IndexedCoefficient)> =
            try_vec(count, "combined original source requests")?;
        for _ in 0..count {
            let ordinal = reader.count("original source ordinal")?;
            if ordinal >= original_count {
                return Err(invalid("original source ordinal"));
            }
            let offset = decode_i64_vec(reader, "original source offset")?;
            if offset.len() != arity {
                return Err(invalid("original source offset arity"));
            }
            let request = TranslatedSourceRequest::new(
                ordinal,
                IntegralShift::try_new(offset).map_err(|_| invalid("original source offset"))?,
            );
            if requests
                .last()
                .is_some_and(|(previous, _)| previous >= &request)
            {
                return Err(invalid("original source request order"));
            }
            let weight = decode_indexed_coefficient(reader, context, "original source weight")?;
            if weight.is_zero() {
                return Err(invalid("zero original source weight"));
            }
            requests.push((request, weight));
        }
        let count = reader.count("combined retained conditions")?;
        check_limit(
            "combined retained conditions",
            count,
            reader.limits().rule_cells.max_guards,
        )?;
        let mut conditions = try_vec(count, "combined retained conditions")?;
        for _ in 0..count {
            conditions.push(decode_indexed_polynomial(
                reader,
                context,
                "combined retained condition",
            )?);
        }
        parents.push(ParentPlan {
            fixed,
            requests,
            conditions,
        });
    }
    let count = reader.count("combined cells")?;
    let geometry = reader.limits().cover_replay;
    check_limit(
        "combined mathematical boxes",
        count,
        geometry.max_requested_boxes,
    )?;
    let coordinate_cells = count
        .checked_mul(arity)
        .and_then(|value| value.checked_mul(2))
        .ok_or(ArtifactPersistenceError::ResourceCountOverflow {
            resource: "combined box coordinate cells",
        })?;
    check_limit(
        "combined box coordinate cells",
        coordinate_cells,
        geometry.max_requested_box_coordinate_cells,
    )?;
    let mut cells = try_vec(count, "combined cells")?;
    let mut used = try_vec(parent_count, "combined parent references")?;
    used.resize(parent_count, false);
    let mut next_first_parent = 0usize;
    for _ in 0..count {
        let parent = reader.count("combined cell parent")?;
        if parent >= parents.len() {
            return Err(invalid("combined cell parent reference"));
        }
        if !used[parent] {
            if parent != next_first_parent {
                return Err(invalid("combined parent encounter order"));
            }
            used[parent] = true;
            next_first_parent += 1;
        }
        let sector = decode_bool_vec(reader, "combined sector")?;
        if sector.len() != arity {
            return Err(invalid("combined sector arity"));
        }
        let application = decode_box(reader, arity)?;
        let term_count = reader.count("combined RHS terms")?;
        check_limit(
            "combined RHS terms",
            term_count,
            reader.limits().rule_cells.max_retained_terms,
        )?;
        let mut rhs = try_vec(term_count, "combined RHS terms")?;
        for _ in 0..term_count {
            let shift = decode_i64_vec(reader, "combined RHS shift")?;
            let shift = IndexShift::try_new(shift, arity)
                .map_err(|_| invalid("combined RHS shift arity"))?;
            let coefficient =
                decode_indexed_coefficient(reader, context, "combined RHS coefficient")?;
            rhs.push((shift, coefficient));
        }
        cells.push(CellPlan {
            parent,
            sector,
            application,
            rhs,
        });
    }
    if used.iter().any(|value| !*value) {
        return Err(invalid("unused combined source parent"));
    }
    Ok((parents, cells))
}
