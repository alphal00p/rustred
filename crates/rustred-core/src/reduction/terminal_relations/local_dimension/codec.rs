//! Native exact claims for local fixed-dimension recurrence source plans.
//!
//! Symbolica owns the state and atom bytes. The structural record only refers
//! to exact coefficients by table ID. Cold loading regenerates the sealed plan
//! from its family, original keys and direction policy; serialized source
//! weights and equations are claims to compare, never publication authority.

use super::*;
use crate::algebra::Coefficient;
use crate::persistence::{
    BinaryIoError, BinaryIoLimits, BinaryProgramKind, BinarySection, CoefficientId,
    CoefficientTableBuilder, DecodedCoefficientTable, EncodedCoefficientTable, SectionTag,
    encode_program, inspect_program,
};

const SCHEMA: u32 = 1;
const RECIPE: &str = "rustred.terminal-local-dimension.fixed-d-exact-source-v1";

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
struct WeightedSourceRecord {
    ordinal: u64,
    seed: Vec<i64>,
    weight: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
struct EquationRecord {
    terms: Vec<(u64, u32)>,
    conditions: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
struct WitnessRecord {
    source: u64,
    direction: Vec<u32>,
    spectator_rank: u64,
    sources: Vec<WeightedSourceRecord>,
    equation: EquationRecord,
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
struct SkippedRecord {
    source: u64,
    direction: Vec<u32>,
    reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
struct Record {
    schema: u32,
    recipe: String,
    family: [u8; 32],
    keys: Vec<Vec<i64>>,
    inventory: Vec<Vec<i64>>,
    witnesses: Vec<WitnessRecord>,
    skips: Vec<SkippedRecord>,
    statistics: [u64; 9],
    skipped_reasons: Vec<(String, u64)>,
}

impl LocalDimensionRecurrencePlan {
    /// Store exact fixed-d source claims using Symbolica's binary coefficient
    /// table. Operational algebra grants are deliberately not mathematical
    /// policy: a cold reader supplies its own grants to regenerate the plan.
    pub fn encode_native(&self, io: BinaryIoLimits) -> Result<Vec<u8>, BinaryIoError> {
        let mut table = CoefficientTableBuilder::new(io);
        let keys = self.input.iter().collect::<Vec<_>>();
        let inventory = self.inventory.iter().collect::<Vec<_>>();
        grant("local dimension input keys", keys.len(), io)?;
        grant("local dimension inventory keys", inventory.len(), io)?;
        grant("local dimension witnesses", self.witnesses.len(), io)?;
        grant("local dimension skipped geometries", self.skips.len(), io)?;
        if !self.input.is_subset(&self.inventory) {
            return Err(invalid("local dimension originals absent from inventory"));
        }
        for key in &inventory {
            grant("local dimension key coordinates", key.powers().len(), io)?;
        }
        let witnesses = self
            .witnesses
            .iter()
            .map(|witness| {
                grant(
                    "local dimension direction coordinates",
                    witness.direction.len(),
                    io,
                )?;
                grant(
                    "local dimension weighted sources",
                    witness.contributions.len(),
                    io,
                )?;
                let source = keys
                    .binary_search(&&witness.source)
                    .map_err(|_| invalid("local dimension witness lacks original source"))?;
                let direction = witness
                    .direction
                    .iter()
                    .map(|value| index(table.intern(value)?))
                    .collect::<Result<_, _>>()?;
                let sources = witness
                    .contributions
                    .iter()
                    .map(|source| {
                        grant(
                            "local dimension weighted seed coordinates",
                            source.seed.powers().len(),
                            io,
                        )?;
                        Ok(WeightedSourceRecord {
                            ordinal: u64::try_from(source.ordinal).map_err(native)?,
                            seed: source.seed.powers().to_vec(),
                            weight: index(table.intern(&source.weight)?)?,
                        })
                    })
                    .collect::<Result<_, BinaryIoError>>()?;
                let equation = encode_equation(&witness.equation, &inventory, &mut table, io)?;
                Ok(WitnessRecord {
                    source: u64::try_from(source).map_err(native)?,
                    direction,
                    spectator_rank: u64::try_from(witness.spectator_rank).map_err(native)?,
                    sources,
                    equation,
                })
            })
            .collect::<Result<_, BinaryIoError>>()?;
        let skips = self
            .skips
            .iter()
            .map(|skip| {
                grant(
                    "local dimension skipped direction coordinates",
                    skip.direction.len(),
                    io,
                )?;
                let source = keys.binary_search(&&skip.source).map_err(|_| {
                    invalid("local dimension skipped witness lacks original source")
                })?;
                Ok(SkippedRecord {
                    source: u64::try_from(source).map_err(native)?,
                    direction: skip
                        .direction
                        .iter()
                        .map(|value| index(table.intern(value)?))
                        .collect::<Result<_, _>>()?,
                    reason: skip.reason.clone(),
                })
            })
            .collect::<Result<_, BinaryIoError>>()?;
        grant(
            "local dimension skipped reasons",
            self.statistics.skipped_reasons.len(),
            io,
        )?;
        let record = Record {
            schema: SCHEMA,
            recipe: RECIPE.into(),
            family: family_digest(self.family_fingerprint()),
            keys: keys.iter().map(|key| key.powers().to_vec()).collect(),
            inventory: inventory.iter().map(|key| key.powers().to_vec()).collect(),
            witnesses,
            skips,
            statistics: encode_statistics(&self.statistics)?,
            skipped_reasons: self
                .statistics
                .skipped_reasons
                .iter()
                .map(|(reason, count)| Ok((reason.clone(), u64::try_from(*count).map_err(native)?)))
                .collect::<Result<_, BinaryIoError>>()?,
        };
        encode_record(&record, table.finish()?, io)
    }

    /// Regenerate every nominated direction, geometry, shifted ordinary
    /// source and exact equation. The serialized weights are never authority.
    pub fn from_native_bytes(
        bytes: &[u8],
        family: Arc<IntegralFamily>,
        options: LocalDimensionRecurrenceOptions,
        io: BinaryIoLimits,
    ) -> Result<Self, Error> {
        Self::from_native_bytes_with_control(
            bytes,
            family,
            options,
            io,
            &LocalDimensionRecurrenceControl::default(),
        )
    }

    /// Cancellable cold source replay. Like ordinary exact preparation,
    /// cancellation is observed between native operations rather than inside
    /// an indivisible Symbolica call.
    pub fn from_native_bytes_with_control(
        bytes: &[u8],
        family: Arc<IntegralFamily>,
        options: LocalDimensionRecurrenceOptions,
        io: BinaryIoLimits,
        control: &LocalDimensionRecurrenceControl<'_>,
    ) -> Result<Self, Error> {
        control.checkpoint()?;
        let (keys, inventory, witnesses, skips, statistics) =
            decode_claims(bytes, &family, io).map_err(|error| Error::Binary(error.to_string()))?;
        control.checkpoint()?;
        let replayed = Self::prepare(family, &keys, options, control)?;
        if replayed.input != keys
            || replayed.inventory != inventory
            || replayed.witnesses != witnesses
            || replayed.skips != skips
            || replayed.statistics != statistics
        {
            return Err(Error::Binary(
                "local dimension claims failed complete exact source replay".into(),
            ));
        }
        Ok(replayed)
    }
}

type Claims = (
    BTreeSet<IntegralKey>,
    BTreeSet<IntegralKey>,
    Vec<Witness>,
    Vec<model::SkippedWitness>,
    LocalDimensionRecurrenceStatistics,
);

fn decode_claims(
    bytes: &[u8],
    family: &IntegralFamily,
    io: BinaryIoLimits,
) -> Result<Claims, BinaryIoError> {
    let envelope = inspect_program(bytes, io)?;
    if envelope.kind() != BinaryProgramKind::TerminalNormalization
        || envelope.sections().iter().map(|section| section.tag).ne([
            SectionTag::SYMBOLICA_STATE,
            SectionTag::COEFFICIENTS,
            SectionTag::PROGRAM,
        ])
    {
        return Err(invalid("not a local dimension exact-source capsule"));
    }
    let payload = envelope
        .section(SectionTag::PROGRAM)
        .expect("checked section");
    let (record, consumed): (Record, usize) =
        bincode::decode_from_slice(payload, bincode::config::standard()).map_err(native)?;
    if consumed != payload.len() || record.schema != SCHEMA || record.recipe != RECIPE {
        return Err(invalid("unsupported or trailing local dimension records"));
    }
    if record.family != family_digest(family.fingerprint()) {
        return Err(invalid("local dimension family identity differs"));
    }
    let keys = decode_keys(record.keys, family, "local dimension input keys", io)?;
    let inventory = decode_keys(
        record.inventory,
        family,
        "local dimension inventory keys",
        io,
    )?;
    let input_set = keys.iter().cloned().collect::<BTreeSet<_>>();
    let inventory_set = inventory.iter().cloned().collect::<BTreeSet<_>>();
    if !input_set.is_subset(&inventory_set) {
        return Err(invalid("local dimension originals absent from inventory"));
    }
    grant("local dimension witnesses", record.witnesses.len(), io)?;
    grant("local dimension skipped geometries", record.skips.len(), io)?;
    let table = DecodedCoefficientTable::import_generated(
        envelope
            .section(SectionTag::SYMBOLICA_STATE)
            .expect("checked section"),
        envelope
            .section(SectionTag::COEFFICIENTS)
            .expect("checked section"),
        io,
    )?;
    for index in 0..table.len() {
        family
            .coefficient_context()
            .validate_with_limits(
                table.coefficient(CoefficientId::try_from_index(index)?)?,
                io.exact_algebra,
            )
            .map_err(native)?;
    }
    let mut used = BTreeSet::new();
    let mut witnesses = Vec::with_capacity(record.witnesses.len());
    for witness in record.witnesses {
        let source = keys
            .get(usize::try_from(witness.source).map_err(native)?)
            .ok_or(invalid(
                "local dimension original source index out of bounds",
            ))?
            .clone();
        grant(
            "local dimension direction coordinates",
            witness.direction.len(),
            io,
        )?;
        if witness.direction.len() != family.loop_momenta().len() {
            return Err(invalid("local dimension direction has wrong loop arity"));
        }
        let direction = witness
            .direction
            .into_iter()
            .map(|id| coefficient(&table, &mut used, id))
            .collect::<Result<Vec<_>, _>>()?;
        if direction.iter().all(Coefficient::is_zero) {
            return Err(invalid("local dimension direction is zero"));
        }
        grant(
            "local dimension weighted sources",
            witness.sources.len(),
            io,
        )?;
        let contributions = witness
            .sources
            .into_iter()
            .map(|source| {
                let seed = decode_key(source.seed, family, io)?;
                let weight = coefficient(&table, &mut used, source.weight)?;
                if weight.is_zero() {
                    return Err(invalid(
                        "local dimension weighted source is explicitly zero",
                    ));
                }
                Ok(WeightedSource {
                    ordinal: usize::try_from(source.ordinal).map_err(native)?,
                    seed,
                    weight,
                })
            })
            .collect::<Result<_, BinaryIoError>>()?;
        let equation = decode_equation(witness.equation, &inventory, &table, &mut used, io)?;
        witnesses.push(Witness {
            source,
            direction,
            spectator_rank: usize::try_from(witness.spectator_rank).map_err(native)?,
            contributions,
            equation,
        });
    }
    let mut skips = Vec::with_capacity(record.skips.len());
    for skip in record.skips {
        let source = keys
            .get(usize::try_from(skip.source).map_err(native)?)
            .ok_or(invalid(
                "local dimension skipped source index out of bounds",
            ))?
            .clone();
        grant(
            "local dimension skipped direction coordinates",
            skip.direction.len(),
            io,
        )?;
        // A nomination-level unsupported geometry has no direction yet. It
        // is retained explicitly, not silently dropped; complete reprepare
        // below must reproduce the same empty-direction refusal and reason.
        // Once a direction exists, its usual loop arity/nonzero checks apply.
        if !skip.direction.is_empty() && skip.direction.len() != family.loop_count() {
            return Err(invalid(
                "local dimension skipped direction has wrong loop arity",
            ));
        }
        let direction = skip
            .direction
            .into_iter()
            .map(|id| coefficient(&table, &mut used, id))
            .collect::<Result<Vec<_>, _>>()?;
        if (!direction.is_empty() && direction.iter().all(Coefficient::is_zero))
            || skip.reason.is_empty()
        {
            return Err(invalid(
                "local dimension skipped geometry has zero direction or empty reason",
            ));
        }
        skips.push(model::SkippedWitness {
            source,
            direction,
            reason: skip.reason,
        });
    }
    if used.len() != table.len() {
        return Err(invalid("unused local dimension coefficient records"));
    }
    grant(
        "local dimension skipped reasons",
        record.skipped_reasons.len(),
        io,
    )?;
    let mut skipped_reasons = BTreeMap::new();
    for (reason, count) in record.skipped_reasons {
        if reason.is_empty()
            || skipped_reasons
                .keys()
                .next_back()
                .is_some_and(|previous| previous >= &reason)
        {
            return Err(invalid(
                "local dimension skipped reasons are not uniquely ordered",
            ));
        }
        let count = usize::try_from(count).map_err(native)?;
        if count == 0 {
            return Err(invalid("local dimension skipped reason count is zero"));
        }
        skipped_reasons.insert(reason, count);
    }
    let mut statistics = decode_statistics(record.statistics)?;
    statistics.skipped_reasons = skipped_reasons;
    Ok((input_set, inventory_set, witnesses, skips, statistics))
}

fn encode_equation(
    equation: &TerminalEquation,
    inventory: &[&IntegralKey],
    table: &mut CoefficientTableBuilder,
    io: BinaryIoLimits,
) -> Result<EquationRecord, BinaryIoError> {
    grant("local dimension equation terms", equation.terms.len(), io)?;
    grant(
        "local dimension equation conditions",
        equation.nonzero_conditions.len(),
        io,
    )?;
    let terms = equation
        .terms
        .iter()
        .map(|(key, value)| {
            if value.is_zero() {
                return Err(invalid("local dimension equation contains explicit zero"));
            }
            let target = inventory
                .binary_search(&key)
                .map_err(|_| invalid("local dimension endpoint lacks inventory"))?;
            Ok((
                u64::try_from(target).map_err(native)?,
                index(table.intern(value)?)?,
            ))
        })
        .collect::<Result<_, BinaryIoError>>()?;
    let conditions = equation
        .nonzero_conditions
        .iter()
        .map(|value| index(table.intern(value)?))
        .collect::<Result<_, _>>()?;
    Ok(EquationRecord { terms, conditions })
}

fn decode_equation(
    record: EquationRecord,
    inventory: &[IntegralKey],
    table: &DecodedCoefficientTable,
    used: &mut BTreeSet<u32>,
    io: BinaryIoLimits,
) -> Result<TerminalEquation, BinaryIoError> {
    grant("local dimension equation terms", record.terms.len(), io)?;
    grant(
        "local dimension equation conditions",
        record.conditions.len(),
        io,
    )?;
    let mut terms = BTreeMap::new();
    let mut previous = None;
    for (target, id) in record.terms {
        let target = usize::try_from(target).map_err(native)?;
        if previous.is_some_and(|previous| previous >= target) {
            return Err(invalid(
                "local dimension equation terms are not uniquely ordered",
            ));
        }
        previous = Some(target);
        let key = inventory.get(target).ok_or(invalid(
            "local dimension equation target index out of bounds",
        ))?;
        let value = coefficient(table, used, id)?;
        if value.is_zero() {
            return Err(invalid("local dimension equation contains explicit zero"));
        }
        terms.insert(key.clone(), value);
    }
    let nonzero_conditions = record
        .conditions
        .into_iter()
        .map(|id| {
            let value = coefficient(table, used, id)?;
            if value.is_zero() {
                return Err(invalid("local dimension equation condition is zero"));
            }
            Ok(value)
        })
        .collect::<Result<_, BinaryIoError>>()?;
    Ok(TerminalEquation {
        terms,
        nonzero_conditions,
    })
}

fn decode_keys(
    records: Vec<Vec<i64>>,
    family: &IntegralFamily,
    resource: &'static str,
    io: BinaryIoLimits,
) -> Result<Vec<IntegralKey>, BinaryIoError> {
    grant(resource, records.len(), io)?;
    let mut keys = Vec::with_capacity(records.len());
    for record in records {
        let key = decode_key(record, family, io)?;
        if keys.last().is_some_and(|previous| previous >= &key) {
            return Err(invalid("local dimension keys are not ordered uniquely"));
        }
        keys.push(key);
    }
    Ok(keys)
}

fn decode_key(
    powers: Vec<i64>,
    family: &IntegralFamily,
    io: BinaryIoLimits,
) -> Result<IntegralKey, BinaryIoError> {
    grant("local dimension key coordinates", powers.len(), io)?;
    if powers.len() != family.denominator_count() {
        return Err(invalid("local dimension key has wrong family arity"));
    }
    IntegralKey::try_new(powers).map_err(native)
}

fn encode_statistics(
    statistics: &LocalDimensionRecurrenceStatistics,
) -> Result<[u64; 9], BinaryIoError> {
    let counts = [
        statistics.input_keys,
        statistics.nominated_directions,
        statistics.supported_geometries,
        statistics.degenerate_grams,
        statistics.skipped_geometries,
        statistics.weighted_source_rows,
        statistics.equations,
        statistics.output_terms,
        statistics.inventory_keys,
    ];
    let mut encoded = [0; 9];
    for (target, value) in encoded.iter_mut().zip(counts) {
        *target = u64::try_from(value).map_err(native)?;
    }
    Ok(encoded)
}

fn decode_statistics(
    values: [u64; 9],
) -> Result<LocalDimensionRecurrenceStatistics, BinaryIoError> {
    let mut counts = [0; 9];
    for (target, value) in counts.iter_mut().zip(values) {
        *target = usize::try_from(value).map_err(native)?;
    }
    Ok(LocalDimensionRecurrenceStatistics {
        input_keys: counts[0],
        nominated_directions: counts[1],
        supported_geometries: counts[2],
        degenerate_grams: counts[3],
        skipped_geometries: counts[4],
        weighted_source_rows: counts[5],
        equations: counts[6],
        output_terms: counts[7],
        inventory_keys: counts[8],
        skipped_reasons: BTreeMap::new(),
    })
}

fn family_digest(fingerprint: &str) -> [u8; 32] {
    *blake3::hash(fingerprint.as_bytes()).as_bytes()
}

fn invalid(message: &'static str) -> BinaryIoError {
    BinaryIoError::Invalid(message)
}

fn native(error: impl std::fmt::Display) -> BinaryIoError {
    BinaryIoError::Native(error.to_string())
}

fn grant(
    resource: &'static str,
    requested: usize,
    io: BinaryIoLimits,
) -> Result<(), BinaryIoError> {
    if requested > io.max_collection_entries {
        Err(BinaryIoError::Limit {
            resource,
            requested,
            limit: io.max_collection_entries,
        })
    } else {
        Ok(())
    }
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

fn encode_record(
    record: &impl bincode::Encode,
    table: EncodedCoefficientTable,
    io: BinaryIoLimits,
) -> Result<Vec<u8>, BinaryIoError> {
    let program = bincode::encode_to_vec(record, bincode::config::standard()).map_err(native)?;
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

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
