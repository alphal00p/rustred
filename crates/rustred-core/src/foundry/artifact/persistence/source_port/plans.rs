//! Canonical wide source-parent sharing and exact mathematical cell payloads.
//! These decoded descriptions carry no replay or installation authority.
use std::collections::BTreeMap;
use std::sync::Arc;

use symbolica::prelude::{IntegerRing, Matrix};

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial};
use crate::foundry::cell::{FixedIndexRestriction, RuleCell, SourceViewConstruction};
use crate::foundry::completion::LatticeBox;
use crate::foundry::parametric::AffineApplicationDomain;
use crate::identity::{IndexShift, IntegralShift, TranslatedSourceRequest};

use super::super::super::error::ArtifactPersistenceError;
use super::super::super::model::ClosedArtifact;
use super::super::binary::{Reader, Writer, check_limit, try_vec};
use super::super::coefficient::{
    decode_base_polynomial, decode_indexed_coefficient, decode_indexed_polynomial, decode_integer,
    encode_base_polynomial, encode_indexed_coefficient, encode_indexed_polynomial, encode_integer,
};
use super::super::semantic::{
    decode_bool_vec, decode_i64_vec, encode_bool_slice, encode_i64_slice,
};

// V4 retains the caller-declared root-sector scope as well as exact affine
// targets and exclusions. A
// distinct plan tag is intentional: old source-port payloads are rejected at
// the byte boundary rather than being misread with shifted fields.
pub(super) const COMBINED_ORIGINAL_PLAN: u16 = 0x704;
const AFFINE_DOMAIN_ABSENT: u8 = 0;
const AFFINE_DOMAIN_PRESENT: u8 = 1;

pub(super) struct ParentPlan {
    pub fixed: Vec<FixedIndexRestriction>,
    pub requests: Vec<(TranslatedSourceRequest, IndexedCoefficient)>,
    pub conditions: Vec<IndexedPolynomial>,
    pub affine: Option<Arc<AffineApplicationDomain>>,
    pub affine_exclusions: Arc<[Arc<AffineApplicationDomain>]>,
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

pub(super) fn encode_affine_domain(
    writer: &mut Writer,
    domain: &AffineApplicationDomain,
) -> Result<(), ArtifactPersistenceError> {
    if !domain.is_authenticated()
        || domain.sector().len() != domain.fixed().len()
        || domain.indices().len() != domain.sector().len()
    {
        return Err(invalid("authenticated affine domain"));
    }
    encode_bool_slice(writer, domain.sector())?;
    writer.usize(domain.fixed().len(), "affine fixed coordinates")?;
    for fixed in domain.fixed() {
        match fixed {
            None => writer.u8(0)?,
            Some(value) => {
                writer.u8(1)?;
                writer.i64(i64::from(*value))?;
            }
        }
    }
    writer.usize(domain.indices().len(), "affine index-variable positions")?;
    for &index in domain.indices() {
        writer.usize(index, "affine index-variable position")?;
    }
    writer.usize(domain.equations().len(), "affine equations")?;
    for equation in domain.equations() {
        encode_base_polynomial(writer, equation)?;
    }
    let matrix = domain
        .primitive_matrix()
        .ok_or_else(|| invalid("affine primitive matrix"))?;
    writer.usize(matrix.nrows(), "affine primitive matrix rows")?;
    writer.usize(matrix.ncols(), "affine primitive matrix columns")?;
    for row in matrix.row_iter() {
        for value in row {
            encode_integer(writer, value)?;
        }
    }
    writer.u8(if domain.has_integral_chart() == Some(true) {
        1
    } else {
        0
    })?;
    Ok(())
}

fn decode_affine_domain(
    reader: &mut Reader<'_>,
    context: &IndexedCoefficientContext,
    arity: usize,
) -> Result<Arc<AffineApplicationDomain>, ArtifactPersistenceError> {
    let sector = decode_bool_vec(reader, "affine sector")?;
    if sector.len() != arity {
        return Err(invalid("affine sector arity"));
    }
    let fixed_count = reader.count("affine fixed coordinates")?;
    if fixed_count != arity {
        return Err(invalid("affine fixed-coordinate arity"));
    }
    let mut fixed = try_vec(arity, "affine fixed coordinates")?;
    for axis in 0..arity {
        let value = match reader.u8()? {
            0 => None,
            1 => Some(i16::try_from(reader.i64()?).map_err(|_| invalid("affine fixed value"))?),
            _ => return Err(invalid("affine fixed-coordinate tag")),
        };
        if value.is_some_and(|value| (value > 0) != sector[axis]) {
            return Err(invalid("affine fixed-coordinate sector"));
        }
        fixed.push(value);
    }
    let index_count = reader.count("affine index-variable positions")?;
    if index_count != arity {
        return Err(invalid("affine index-variable arity"));
    }
    let mut indices = try_vec(arity, "affine index-variable positions")?;
    for _ in 0..arity {
        let index = reader.count("affine index-variable position")?;
        let variable_count = context.one().raw().get_variables().len();
        let first_index = variable_count
            .checked_sub(arity)
            .ok_or_else(|| invalid("affine index-variable positions"))?;
        let expected = first_index
            .checked_add(indices.len())
            .ok_or_else(|| invalid("affine index-variable positions"))?;
        if index != expected {
            return Err(invalid("affine index-variable positions"));
        }
        indices.push(index);
    }
    let equation_count = reader.count("affine equations")?;
    check_limit(
        "affine equations",
        equation_count,
        reader.limits().rule_cells.max_guards,
    )?;
    if equation_count == 0 {
        return Err(invalid("empty affine equations"));
    }
    let template = context.one();
    let variables = template.raw().get_variables();
    let mut equations = try_vec(equation_count, "affine equations")?;
    for _ in 0..equation_count {
        equations.push(decode_base_polynomial(
            reader,
            variables,
            "affine equation",
        )?);
    }
    let rows = reader.count("affine primitive matrix rows")?;
    let columns = reader.count("affine primitive matrix columns")?;
    if rows == 0 || columns != arity + 1 {
        return Err(invalid("affine primitive matrix shape"));
    }
    let entries =
        rows.checked_mul(columns)
            .ok_or(ArtifactPersistenceError::ResourceCountOverflow {
                resource: "affine primitive matrix entries",
            })?;
    check_limit(
        "affine primitive matrix entries",
        entries,
        reader.limits().max_collection_entries,
    )?;
    let mut values = try_vec(entries, "affine primitive matrix entries")?;
    for _ in 0..entries {
        values.push(decode_integer(reader, "affine primitive matrix entry")?);
    }
    let rows = u32::try_from(rows).map_err(|_| invalid("affine primitive matrix rows"))?;
    let columns = u32::try_from(columns).map_err(|_| invalid("affine primitive matrix columns"))?;
    let matrix = Matrix::from_linear(values, rows, columns, IntegerRing)
        .map_err(|_| invalid("affine primitive matrix shape"))?;
    let integral_chart = match reader.u8()? {
        0 => false,
        1 => true,
        _ => return Err(invalid("affine integral-chart tag")),
    };
    let domain = AffineApplicationDomain::from_persisted(
        sector.into_boxed_slice(),
        fixed.into_boxed_slice(),
        indices.into_boxed_slice(),
        equations.into_boxed_slice(),
        matrix,
        integral_chart,
    )
    .map_err(|_| invalid("authenticated affine domain"))?;
    if domain
        .equations()
        .iter()
        .any(|equation| equation.variables() != variables)
    {
        return Err(invalid("affine equation variable map"));
    }
    Ok(Arc::new(domain))
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
        && left
            .rule()
            .replay_evidence()
            .combined_original_domain()
            .and_then(|evidence| evidence.affine_application_domain())
            == right
                .rule()
                .replay_evidence()
                .combined_original_domain()
                .and_then(|evidence| evidence.affine_application_domain())
        && match (
            left.rule().replay_evidence().combined_original_domain(),
            right.rule().replay_evidence().combined_original_domain(),
        ) {
            (Some(left), Some(right)) => left.affine_exclusions() == right.affine_exclusions(),
            (None, None) => true,
            _ => false,
        }
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
    let root = super::super::super::source_port::scope::root_from_bounds(
        artifact.supported_root_power_bounds(),
        artifact.arity(),
    )?;
    encode_bool_slice(writer, root.active_bits())?;
    writer.usize(parents.len(), "combined source parents")?;
    for (cell, (fixed, requests)) in &parents {
        writer.usize(fixed.len(), "combined fixed target coordinates")?;
        for (axis, value) in fixed {
            writer.usize(*axis, "fixed target axis")?;
            writer.i64(*value)?;
        }
        let evidence = cell
            .rule()
            .replay_evidence()
            .combined_original_domain()
            .ok_or_else(|| invalid("combined source parent evidence"))?;
        match evidence.affine_application_domain() {
            None => writer.u8(AFFINE_DOMAIN_ABSENT)?,
            Some(domain) => {
                writer.u8(AFFINE_DOMAIN_PRESENT)?;
                encode_affine_domain(writer, domain)?;
            }
        }
        writer.usize(evidence.affine_exclusions().len(), "affine exclusions")?;
        for excluded in evidence.affine_exclusions() {
            encode_affine_domain(writer, excluded)?;
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
) -> Result<(crate::sector::Mask, Vec<ParentPlan>, Vec<CellPlan>), ArtifactPersistenceError> {
    decode_with_scope(reader, context, original_count, false)
}

pub(super) fn decode_root(
    reader: &mut Reader<'_>,
    arity: usize,
) -> Result<crate::sector::Mask, ArtifactPersistenceError> {
    if reader.u16()? != COMBINED_ORIGINAL_PLAN {
        return Err(invalid("combined original plan tag"));
    }
    let root = decode_bool_vec(reader, "combined root sector")?;
    if root.len() != arity {
        return Err(invalid("combined root-sector arity"));
    }
    crate::sector::Mask::try_new(root).map_err(|_| invalid("combined root sector"))
}

pub(super) fn decode_with_scope(
    reader: &mut Reader<'_>,
    context: &IndexedCoefficientContext,
    original_count: usize,
    bounded: bool,
) -> Result<(crate::sector::Mask, Vec<ParentPlan>, Vec<CellPlan>), ArtifactPersistenceError> {
    let arity = context.index_count();
    let root = decode_root(reader, arity)?;
    let parent_count = reader.count("combined source parents")?;
    if parent_count == 0 && !bounded {
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
        let affine = match reader.u8()? {
            AFFINE_DOMAIN_ABSENT => None,
            AFFINE_DOMAIN_PRESENT => Some(decode_affine_domain(reader, context, arity)?),
            _ => return Err(invalid("affine-domain presence tag")),
        };
        let count = reader.count("affine exclusions")?;
        check_limit(
            "affine exclusions",
            count,
            reader.limits().rule_cells.max_guards,
        )?;
        let mut affine_exclusions = try_vec(count, "affine exclusions")?;
        for _ in 0..count {
            affine_exclusions.push(decode_affine_domain(reader, context, arity)?);
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
            affine,
            affine_exclusions: affine_exclusions.into(),
        });
    }
    let count = reader.count("combined cells")?;
    if parent_count == 0 && count != 0 {
        return Err(invalid("empty combined source parents with cells"));
    }
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
        if let Some(affine) = &parents[parent].affine {
            let parent_fixed: Vec<_> = (0..arity)
                .map(|axis| {
                    parents[parent]
                        .fixed
                        .iter()
                        .find(|fixed| fixed.position() == axis)
                        .map(|fixed| fixed.value())
                })
                .collect();
            let affine_fixed: Vec<_> = affine
                .fixed()
                .iter()
                .map(|value| value.map(i64::from))
                .collect();
            if affine.sector() != sector.as_slice() || affine_fixed != parent_fixed {
                return Err(invalid("affine parent/cell binding"));
            }
        }
        if parents[parent]
            .affine_exclusions
            .iter()
            .any(|excluded| excluded.sector() != sector.as_slice())
        {
            return Err(invalid("affine exclusion/cell binding"));
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
    Ok((root, parents, cells))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algebra::{CoefficientContext, IndexedCoefficientContext};
    use crate::solver::{AffineCase, AffineIntersection, CoordinateCase};

    fn fixture() -> (IndexedCoefficientContext, Arc<AffineApplicationDomain>) {
        let base = CoefficientContext::new(["d"]);
        let context = IndexedCoefficientContext::try_new(&base, "affine-persistence-test", 2)
            .expect("indexed context");
        let n0 = context.index(0).expect("n0");
        let n1 = context.index(1).expect("n1");
        let equation = context
            .sub(&n0, &n1)
            .expect("equation")
            .raw()
            .numerator
            .clone();
        let case = AffineCase::from_coordinate(
            &CoordinateCase::generic(),
            &[equation],
            &[1, 2],
            &[true, true],
        )
        .expect("affine case");
        let AffineIntersection::Affine(case) = case else {
            panic!("fixture must be coupled");
        };
        let domain = AffineApplicationDomain::from_case(&case, &[true, true]).unwrap();
        (context, Arc::new(domain))
    }

    #[test]
    fn authenticated_affine_domain_round_trips_byte_identically() {
        let (context, domain) = fixture();
        let mut writer = Writer::new(Default::default());
        encode_affine_domain(&mut writer, &domain).unwrap();
        let (bytes, table) = writer.finish_for_test().unwrap();
        let mut reader = Reader::with_table(&bytes, Default::default(), table).unwrap();
        let decoded = decode_affine_domain(&mut reader, &context, 2).unwrap();
        reader.finish().unwrap();
        assert_eq!(*decoded, *domain);
        let mut replay = Writer::new(Default::default());
        encode_affine_domain(&mut replay, &decoded).unwrap();
        assert_eq!(replay.finish(), bytes);
    }

    #[test]
    fn affine_domain_codec_rejects_duplicate_index_positions() {
        let (context, domain) = fixture();
        let mut writer = Writer::new(Default::default());
        encode_affine_domain(&mut writer, &domain).unwrap();
        let (mut bytes, table) = writer.finish_for_test().unwrap();
        // sector length + two sector bytes + fixed length + two fixed tags +
        // index length place the two u64 positions at offsets 28 and 36.
        bytes[36..44].copy_from_slice(&1u64.to_le_bytes());
        let mut reader = Reader::with_table(&bytes, Default::default(), table).unwrap();
        assert_eq!(
            decode_affine_domain(&mut reader, &context, 2).unwrap_err(),
            ArtifactPersistenceError::SemanticMismatch {
                field: "affine index-variable positions"
            }
        );
    }
}
