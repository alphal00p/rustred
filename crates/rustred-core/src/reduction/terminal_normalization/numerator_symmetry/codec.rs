//! Generated native witness capsules. Cold loading verifies the stored maps
//! and transports complete numerators again; it does not rediscover symmetries.
//! Historical proposal diagnostics are not mathematical authority.
use super::*;
use crate::algebra::Coefficient;
use crate::persistence::{
    BinaryIoError, BinaryIoLimits, BinaryProgramKind, BinarySection, CoefficientId,
    CoefficientTableBuilder, DecodedCoefficientTable, SectionTag, encode_program, inspect_program,
};
use crate::sector::symmetry::CoefficientMatrix;

const SCHEMA: u32 = 1;
const RECIPE: &str = "rustred.terminal-numerator-symmetry.exact-map-v1";

#[derive(Clone, bincode::Encode, bincode::Decode)]
struct MatrixRecord {
    rows: u64,
    columns: u64,
    entries: Vec<u32>,
}
#[derive(Clone, bincode::Encode, bincode::Decode)]
struct WitnessRecord {
    source: u64,
    maps: [MatrixRecord; 3],
    terms: Vec<(Vec<i64>, u32)>,
    conditions: Vec<u32>,
}
#[derive(Clone, bincode::Encode, bincode::Decode)]
struct Record {
    schema: u32,
    recipe: String,
    family: [u8; 32],
    keys: Vec<Vec<i64>>,
    witnesses: Vec<WitnessRecord>,
    statistics: StatisticsRecord,
}
#[derive(Clone, bincode::Encode, bincode::Decode)]
struct StatisticsRecord {
    counts: [u64; 10],
    skipped: Vec<(u8, Option<u8>, u64)>,
}

impl TerminalNumeratorSymmetryEquations {
    /// Encode native Symbolica coefficients with their state context. The
    /// structural section stores integer powers and exact momentum matrices,
    /// never expression strings or a discovery recipe standing in for proof.
    pub fn encode_native(&self, io: BinaryIoLimits) -> Result<Vec<u8>, BinaryIoError> {
        let mut table = CoefficientTableBuilder::new(io);
        let keys: Vec<_> = self.input_keys.iter().cloned().collect();
        grant("numerator symmetry input keys", keys.len(), io)?;
        for key in &keys {
            grant(
                "numerator symmetry coordinate entries",
                key.powers().len(),
                io,
            )?;
        }
        let mut witnesses = Vec::new();
        for (source, rows) in &self.rows {
            let proofs = self
                .witnesses
                .get(source)
                .ok_or(invalid("missing numerator witnesses"))?;
            if rows.len() != proofs.len() {
                return Err(invalid("numerator witness/row count mismatch"));
            }
            let source_id = keys
                .binary_search(source)
                .map_err(|_| invalid("undeclared numerator source"))?;
            for (row, witness) in rows.iter().zip(proofs) {
                if witness.source() != source
                    || witness.equation() != row
                    || witness.family_fingerprint() != self.family_fingerprint()
                    || row.terms.is_empty()
                {
                    return Err(invalid("numerator witness differs from its provider row"));
                }
                let map = witness.verified_map().momentum();
                grant("numerator symmetry row entries", row.terms.len(), io)?;
                grant(
                    "numerator symmetry conditions",
                    row.nonzero_conditions.len(),
                    io,
                )?;
                witnesses.push(WitnessRecord {
                    source: u64::try_from(source_id).map_err(native)?,
                    maps: [
                        encode_matrix(map.loop_linear(), &mut table, io)?,
                        encode_matrix(map.loop_external(), &mut table, io)?,
                        encode_matrix(map.external_linear(), &mut table, io)?,
                    ],
                    terms: row
                        .terms
                        .iter()
                        .map(|(key, coefficient)| {
                            Ok((key.powers().to_vec(), index(table.intern(coefficient)?)?))
                        })
                        .collect::<Result<_, BinaryIoError>>()?,
                    conditions: row
                        .nonzero_conditions
                        .iter()
                        .map(|coefficient| index(table.intern(coefficient)?))
                        .collect::<Result<_, _>>()?,
                });
                grant("numerator symmetry witnesses", witnesses.len(), io)?;
            }
        }
        if self.witnesses.len() != self.rows.len() {
            return Err(invalid("orphan numerator witnesses"));
        }
        let record = Record {
            schema: SCHEMA,
            recipe: RECIPE.into(),
            family: *blake3::hash(self.family_fingerprint().as_bytes()).as_bytes(),
            keys: keys.iter().map(|key| key.powers().to_vec()).collect(),
            witnesses,
            statistics: encode_statistics(&self.statistics)?,
        };
        encode_record(&record, table.finish()?, io)
    }

    /// Authenticate a generated capsule against the supplied family. Every
    /// stored exact map is independently verified and its complete negative-
    /// power numerator is transported, coalesced and compared with the stored
    /// equation and guards. No heuristic symmetry search or IBP generation runs.
    pub fn from_native_bytes(
        bytes: &[u8],
        family: Arc<IntegralFamily>,
        limits: TerminalNumeratorSymmetryLimits,
        io: BinaryIoLimits,
    ) -> Result<Self, Error> {
        let envelope = inspect_program(bytes, io)?;
        if envelope.kind() != BinaryProgramKind::TerminalNormalization
            || envelope.sections().iter().map(|section| section.tag).ne([
                SectionTag::SYMBOLICA_STATE,
                SectionTag::COEFFICIENTS,
                SectionTag::PROGRAM,
            ])
        {
            return Err(invalid("not a numerator symmetry capsule").into());
        }
        // Matching trusted generated-native boundary: caller byte/count grants
        // apply, without bincode's unrelated fixed decoded-allocation ceiling.
        let payload = envelope
            .section(SectionTag::PROGRAM)
            .expect("checked section");
        let (record, consumed): (Record, usize) =
            crate::persistence::decode_structure(payload).map_err(native)?;
        if consumed != payload.len() || record.schema != SCHEMA || record.recipe != RECIPE {
            return Err(invalid("unsupported or trailing numerator symmetry records").into());
        }
        if record.family != *blake3::hash(family.fingerprint().as_bytes()).as_bytes() {
            return Err(invalid("numerator symmetry family identity differs").into());
        }
        grant("numerator symmetry input keys", record.keys.len(), io)?;
        grant("numerator symmetry witnesses", record.witnesses.len(), io)?;
        let mut keys = Vec::with_capacity(record.keys.len());
        for powers in record.keys {
            grant("numerator symmetry coordinate entries", powers.len(), io)?;
            let key = IntegralKey::try_new(powers).map_err(native)?;
            validate_key(&family, &key)?;
            if keys.last().is_some_and(|previous| previous >= &key) {
                return Err(
                    invalid("numerator symmetry input keys are not ordered uniquely").into(),
                );
            }
            keys.push(key);
        }
        let table = DecodedCoefficientTable::import_generated(
            envelope
                .section(SectionTag::SYMBOLICA_STATE)
                .expect("checked section"),
            envelope
                .section(SectionTag::COEFFICIENTS)
                .expect("checked section"),
            io,
        )?;
        for i in 0..table.len() {
            family
                .coefficient_context()
                .validate_with_limits(
                    table.coefficient(CoefficientId::try_from_index(i)?)?,
                    io.exact_algebra,
                )
                .map_err(native)?;
        }
        let mut used = BTreeSet::new();
        let mut rows: BTreeMap<IntegralKey, Vec<TerminalEquation>> = BTreeMap::new();
        let mut witnesses: BTreeMap<IntegralKey, Vec<TerminalNumeratorSymmetryWitness>> =
            BTreeMap::new();
        let mut last_source = None;
        for record in record.witnesses {
            let source_id = usize::try_from(record.source).map_err(native)?;
            if last_source.is_some_and(|previous| previous > source_id) {
                return Err(invalid("numerator witnesses are not source ordered").into());
            }
            last_source = Some(source_id);
            let source = keys
                .get(source_id)
                .ok_or(invalid("numerator source index out of bounds"))?;
            let [a, b, c] = record.maps;
            let map = MomentumMap::new(
                decode_matrix(a, &table, &mut used, limits, io)?,
                decode_matrix(b, &table, &mut used, limits, io)?,
                decode_matrix(c, &table, &mut used, limits, io)?,
            );
            grant("numerator symmetry row entries", record.terms.len(), io)?;
            grant("numerator symmetry conditions", record.conditions.len(), io)?;
            let mut terms = BTreeMap::new();
            let mut last_key = None;
            for (powers, id) in record.terms {
                let key = IntegralKey::try_new(powers).map_err(native)?;
                validate_key(&family, &key)?;
                if last_key.as_ref().is_some_and(|previous| previous >= &key) {
                    return Err(invalid("numerator equation is not uniquely coalesced").into());
                }
                last_key = Some(key.clone());
                let value = coefficient(&table, &mut used, id)?;
                if value.is_zero() {
                    return Err(invalid("numerator equation contains an explicit zero").into());
                }
                terms.insert(key, value);
            }
            let nonzero_conditions = record
                .conditions
                .into_iter()
                .map(|id| coefficient(&table, &mut used, id))
                .collect::<Result<Vec<_>, _>>()?;
            let equation = TerminalEquation {
                terms,
                nonzero_conditions,
            };
            if equation.terms.is_empty() {
                return Err(invalid("empty stored numerator symmetry equation").into());
            }
            let replayed =
                TerminalNumeratorSymmetryWitness::verify(family.clone(), source, map, limits)?;
            if replayed.equation() != &equation {
                return Err(invalid(
                    "numerator symmetry equation or guards failed full exact replay",
                )
                .into());
            }
            let target_rows = rows.entry(source.clone()).or_default();
            if target_rows.contains(&equation) {
                return Err(invalid("duplicate numerator symmetry equation").into());
            }
            target_rows.push(equation);
            witnesses.entry(source.clone()).or_default().push(replayed);
        }
        if used.len() != table.len() {
            return Err(invalid("unused numerator symmetry coefficient records").into());
        }
        let statistics = decode_statistics(record.statistics)?;
        let equations = rows.values().map(Vec::len).sum::<usize>();
        let output_terms = rows
            .values()
            .flatten()
            .map(|row| row.terms.len())
            .try_fold(0usize, |count, terms| {
                count
                    .checked_add(terms)
                    .ok_or(invalid("numerator output count overflow"))
            })?;
        if statistics.input_keys != keys.len()
            || statistics.equation_targets != rows.len()
            || statistics.equations != equations
            || statistics.output_terms != output_terms
            || statistics.verified_generators > statistics.proposed_generators
            || (!rows.is_empty() && statistics.verified_generators == 0)
        {
            return Err(
                invalid("numerator symmetry diagnostic counts differ from exact rows").into(),
            );
        }
        Ok(Self {
            family_fingerprint: family.fingerprint_owner(),
            input_keys: keys.into_iter().collect(),
            rows,
            witnesses,
            statistics,
        })
    }
}

fn invalid(message: &'static str) -> BinaryIoError {
    BinaryIoError::Invalid(message)
}
fn native(error: impl std::fmt::Display + 'static) -> BinaryIoError {
    crate::persistence::native_error(error)
}
fn grant(
    resource: &'static str,
    requested: usize,
    io: BinaryIoLimits,
) -> Result<(), BinaryIoError> {
    io.check_count(resource, requested, io.max_collection_entries)
}
fn index(id: CoefficientId) -> Result<u32, BinaryIoError> {
    u32::try_from(id.index()).map_err(native)
}
fn coefficient(
    table: &DecodedCoefficientTable,
    used: &mut BTreeSet<u32>,
    id: u32,
) -> Result<Coefficient, BinaryIoError> {
    let value = table
        .coefficient(CoefficientId::try_from_index(id as usize)?)?
        .clone();
    used.insert(id);
    Ok(value)
}
fn encode_matrix(
    matrix: &CoefficientMatrix,
    table: &mut CoefficientTableBuilder,
    io: BinaryIoLimits,
) -> Result<MatrixRecord, BinaryIoError> {
    grant(
        "numerator symmetry matrix entries",
        matrix.entries().len(),
        io,
    )?;
    Ok(MatrixRecord {
        rows: u64::try_from(matrix.rows()).map_err(native)?,
        columns: u64::try_from(matrix.columns()).map_err(native)?,
        entries: matrix
            .entries()
            .iter()
            .map(|value| index(table.intern(value)?))
            .collect::<Result<_, _>>()?,
    })
}
fn decode_matrix(
    record: MatrixRecord,
    table: &DecodedCoefficientTable,
    used: &mut BTreeSet<u32>,
    limits: TerminalNumeratorSymmetryLimits,
    io: BinaryIoLimits,
) -> Result<CoefficientMatrix, BinaryIoError> {
    grant(
        "numerator symmetry matrix entries",
        record.entries.len(),
        io,
    )?;
    let entries = record
        .entries
        .into_iter()
        .map(|id| coefficient(table, used, id))
        .collect::<Result<Vec<_>, _>>()?;
    let rows = usize::try_from(record.rows).map_err(native)?;
    let columns = usize::try_from(record.columns).map_err(native)?;
    if limits.verification.diagnostics_enabled {
        CoefficientMatrix::try_new_with_max_entries(
            rows,
            columns,
            entries,
            limits.verification.max_matrix_entries,
        )
    } else {
        CoefficientMatrix::try_new(rows, columns, entries)
    }
    .map_err(native)
}
fn encode_record(
    record: &Record,
    table: crate::persistence::EncodedCoefficientTable,
    io: BinaryIoLimits,
) -> Result<Vec<u8>, BinaryIoError> {
    let program =
        crate::persistence::encode_structure(&record, io.max_program_bytes).map_err(native)?;
    encode_program(
        BinaryProgramKind::TerminalNormalization,
        &[
            BinarySection {
                tag: SectionTag::SYMBOLICA_STATE,
                bytes: &table.state,
            },
            BinarySection {
                tag: SectionTag::COEFFICIENTS,
                bytes: &table.atoms,
            },
            BinarySection {
                tag: SectionTag::PROGRAM,
                bytes: &program,
            },
        ],
        io,
    )
}

fn encode_statistics(
    s: &TerminalNumeratorSymmetryStatistics,
) -> Result<StatisticsRecord, BinaryIoError> {
    Ok(StatisticsRecord {
        counts: [
            s.input_keys,
            s.analyzed_supports,
            s.analyzed_denominators,
            s.proposed_generators,
            s.verified_generators,
            s.equation_targets,
            s.equations,
            s.output_terms,
            s.trivial_equations,
            s.rejected_power_permutations,
        ]
        .map(|value| u64::try_from(value).map_err(native))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| invalid("statistics arity"))?,
        skipped: s
            .skipped
            .iter()
            .map(|(reason, count)| {
                let (kind, geometry) = skip_code(*reason);
                Ok((kind, geometry, u64::try_from(*count).map_err(native)?))
            })
            .collect::<Result<_, BinaryIoError>>()?,
    })
}
fn decode_statistics(
    record: StatisticsRecord,
) -> Result<TerminalNumeratorSymmetryStatistics, BinaryIoError> {
    let counts = record
        .counts
        .map(|value| usize::try_from(value).map_err(native))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let mut skipped = BTreeMap::new();
    for (kind, geometry, count) in record.skipped {
        if skipped
            .insert(
                skip_reason(kind, geometry)?,
                usize::try_from(count).map_err(native)?,
            )
            .is_some()
        {
            return Err(invalid("duplicate numerator diagnostic reason"));
        }
    }
    Ok(TerminalNumeratorSymmetryStatistics {
        input_keys: counts[0],
        analyzed_supports: counts[1],
        analyzed_denominators: counts[2],
        proposed_generators: counts[3],
        verified_generators: counts[4],
        equation_targets: counts[5],
        equations: counts[6],
        output_terms: counts[7],
        trivial_equations: counts[8],
        rejected_power_permutations: counts[9],
        skipped,
    })
}
fn skip_code(reason: TerminalNumeratorSymmetrySkipReason) -> (u8, Option<u8>) {
    use TerminalNumeratorSymmetrySkipReason::*;
    match reason {
        ExternalMomenta => (0, None),
        AnalyticPowerShifts => (1, None),
        NoNumerator => (2, None),
        MomentumGeometry(reason) => (3, Some(geometry_code(reason))),
        RankDeficientSupport => (4, None),
        NoProposedSymmetry => (5, None),
    }
}
fn skip_reason(
    kind: u8,
    geometry: Option<u8>,
) -> Result<TerminalNumeratorSymmetrySkipReason, BinaryIoError> {
    use TerminalNumeratorSymmetrySkipReason::*;
    Ok(match (kind, geometry) {
        (0, None) => ExternalMomenta,
        (1, None) => AnalyticPowerShifts,
        (2, None) => NoNumerator,
        (3, Some(code)) => MomentumGeometry(geometry_reason(code)?),
        (4, None) => RankDeficientSupport,
        (5, None) => NoProposedSymmetry,
        _ => return Err(invalid("unknown numerator diagnostic reason")),
    })
}
fn geometry_code(reason: ProductSkipReason) -> u8 {
    use ProductSkipReason::*;
    match reason {
        ExternalMomenta => 0,
        AnalyticPowerShifts => 1,
        NumeratorPowers => 2,
        ActiveLineCount => 3,
        NonUnitMass => 4,
        NonIntegerQuadratic => 5,
        NotLinearSquare => 6,
        SingularMomentumBasis => 7,
        NonUnimodularMomentumBasis => 8,
        ConditionalMomentumMap => 9,
        NonIntegralMomentumMap => 10,
        NonUnimodularMomentumMap => 11,
        ParametricPreparationLimit => 12,
    }
}
fn geometry_reason(code: u8) -> Result<ProductSkipReason, BinaryIoError> {
    use ProductSkipReason::*;
    Ok(match code {
        0 => ExternalMomenta,
        1 => AnalyticPowerShifts,
        2 => NumeratorPowers,
        3 => ActiveLineCount,
        4 => NonUnitMass,
        5 => NonIntegerQuadratic,
        6 => NotLinearSquare,
        7 => SingularMomentumBasis,
        8 => NonUnimodularMomentumBasis,
        9 => ConditionalMomentumMap,
        10 => NonIntegralMomentumMap,
        11 => NonUnimodularMomentumMap,
        12 => ParametricPreparationLimit,
        _ => return Err(invalid("unknown numerator geometry diagnostic")),
    })
}

#[cfg(test)]
mod tests;
