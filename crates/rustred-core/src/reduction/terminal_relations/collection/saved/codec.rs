//! Native Symbolica sidecar. It stores exact predecessor maps and provider
//! bindings, original diagonal sources, source weights, guards and final maps.
//! Import validates composition against supplied sessions and replays the
//! finite collection once, following the native normalization-sidecar policy.
use super::*;
use crate::persistence::{
    BinaryProgramKind, BinarySection, CoefficientId, CoefficientTableBuilder,
    DecodedCoefficientTable, NativeFamilyRecord, SectionTag, encode_program, inspect_program,
};

const MAX_RECORD_BYTES: usize = 1 << 30;

type Key = (u32, Vec<i64>);
type NativeRow = Vec<(Key, u32)>;
type LocalRow = Vec<(Vec<i64>, u32)>;
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
struct PredecessorRecord {
    family: u32,
    assistance_binding: Option<String>,
    normalized: u64,
    remaining: Vec<Vec<i64>>,
    conditions: Vec<u32>,
    maps: Vec<(Vec<i64>, LocalRow)>,
}
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
struct OrdinaryRecord {
    contraction: u64,
    differentiated_loop: u64,
    terms: LocalRow,
    conditions: Vec<u32>,
}
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
struct ProofRecord {
    inputs: Vec<(u32, Vec<Vec<i64>>)>,
    sources: Vec<(u32, Vec<i64>, Vec<OrdinaryRecord>)>,
    equations: Vec<(NativeRow, Vec<(u64, u32)>, Vec<u32>)>,
}
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
struct Record {
    schema: u32,
    families: Vec<(String, NativeFamilyRecord)>,
    predecessors: Vec<PredecessorRecord>,
    proofs: Vec<ProofRecord>,
    layer_ends: Vec<u64>,
    maps: Vec<(Key, NativeRow, Vec<u32>)>,
}
fn binary(e: impl std::fmt::Display) -> VacuumCollectionError {
    VacuumCollectionError::Binary(e.to_string())
}

struct Intern {
    native: CoefficientTableBuilder,
}
impl Intern {
    fn new(io: BinaryIoLimits) -> Self {
        Self {
            native: CoefficientTableBuilder::new(io),
        }
    }
    fn value(&mut self, value: &Coefficient) -> Result<u32> {
        let id = self.native.intern(value).map_err(binary)?;
        // Family records intern through the same builder. Reconstructing the
        // entire expected dictionary below also covers their earlier entries.
        Ok(id.index() as u32)
    }
    fn local(&mut self, row: &TerminalRelationRow) -> Result<LocalRow> {
        row.iter()
            .map(|(key, value)| Ok((key.powers().to_vec(), self.value(value)?)))
            .collect()
    }
    fn conditions(&mut self, values: &[Coefficient]) -> Result<Vec<u32>> {
        values.iter().map(|v| self.value(v)).collect()
    }
    fn row(&mut self, row: &Row, families: &BTreeMap<&str, u32>) -> Result<NativeRow> {
        row.iter()
            .map(|(key, value)| {
                Ok((
                    (
                        families[key.family_fingerprint()],
                        key.integral().powers().to_vec(),
                    ),
                    self.value(value)?,
                ))
            })
            .collect()
    }
}

fn record(plan: &TerminalCollectionPlan, io: BinaryIoLimits) -> Result<(Record, Intern)> {
    check(
        "collection records",
        plan.raw.len(),
        io.max_collection_entries,
    )?;
    let mut table = Intern::new(io);
    let ids: BTreeMap<_, _> = plan
        .families
        .keys()
        .enumerate()
        .map(|(i, f)| (f.as_str(), i as u32))
        .collect();
    let families = plan
        .families
        .iter()
        .map(|(fingerprint, family)| {
            Ok((
                fingerprint.clone(),
                NativeFamilyRecord::from_family(family, &mut table.native).map_err(binary)?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let predecessors = plan
        .predecessors
        .iter()
        .map(|(family, parent)| {
            Ok(PredecessorRecord {
                family: ids[family.as_str()],
                assistance_binding: parent.assistance_binding.clone(),
                normalized: parent.normalized as u64,
                remaining: parent
                    .remaining
                    .iter()
                    .map(|k| k.powers().to_vec())
                    .collect(),
                conditions: table.conditions(&parent.conditions)?,
                maps: parent
                    .maps
                    .iter()
                    .map(|(key, row)| Ok((key.powers().to_vec(), table.local(row)?)))
                    .collect::<Result<_>>()?,
            })
        })
        .collect::<Result<_>>()?;
    let proofs = plan
        .proofs
        .iter()
        .map(|proof| {
            let inputs = proof
                .families()
                .map(|family| {
                    (
                        ids[family.fingerprint()],
                        proof
                            .raw_terminals()
                            .iter()
                            .filter(|key| key.family_fingerprint() == family.fingerprint())
                            .map(|key| key.integral().powers().to_vec())
                            .collect(),
                    )
                })
                .collect();
            let sources = proof
                .sources()
                .iter()
                .map(|source| {
                    let rows = source
                        .rows()
                        .iter()
                        .map(|row| {
                            let RowId::OrdinaryIbp {
                                contraction_momentum,
                                differentiated_loop,
                            } = row.row_id()
                            else {
                                return Err(VacuumCollectionError::ReplayFailed);
                            };
                            Ok(OrdinaryRecord {
                                contraction: *contraction_momentum as u64,
                                differentiated_loop: *differentiated_loop as u64,
                                terms: table.local(&row.equation().terms)?,
                                conditions: table.conditions(&row.equation().nonzero_conditions)?,
                            })
                        })
                        .collect::<Result<_>>()?;
                    Ok((
                        ids[source.family_fingerprint()],
                        source.corner().powers().to_vec(),
                        rows,
                    ))
                })
                .collect::<Result<_>>()?;
            let equations = proof
                .equations()
                .iter()
                .map(|equation| {
                    Ok((
                        table.row(equation.terms(), &ids)?,
                        equation
                            .source_weights()
                            .iter()
                            .map(|(source, value)| Ok((*source as u64, table.value(value)?)))
                            .collect::<Result<_>>()?,
                        table.conditions(equation.nonzero_conditions())?,
                    ))
                })
                .collect::<Result<_>>()?;
            Ok(ProofRecord {
                inputs,
                sources,
                equations,
            })
        })
        .collect::<Result<_>>()?;
    let mut maps = Vec::new();
    let mut terms = 0usize;
    for (family, rows) in &plan.reductions {
        for (key, row) in rows {
            terms = terms
                .saturating_add(row.terms().len())
                .saturating_add(row.nonzero_conditions().len());
            check("collection entries", terms, io.max_collection_entries)?;
            maps.push((
                (ids[family.as_str()], key.powers().to_vec()),
                table.row(row.terms(), &ids)?,
                table.conditions(row.nonzero_conditions())?,
            ));
        }
    }
    Ok((
        Record {
            schema: 1,
            families,
            predecessors,
            proofs,
            layer_ends: plan.layer_ends.iter().map(|&n| n as u64).collect(),
            maps,
        },
        table,
    ))
}

pub(super) fn encode(plan: &TerminalCollectionPlan, io: BinaryIoLimits) -> Result<Vec<u8>> {
    let (record, table) = record(plan, io)?;
    let records = bincode::encode_to_vec(record, bincode::config::standard()).map_err(binary)?;
    check(
        "collection structural bytes",
        records.len(),
        io.max_program_bytes.min(MAX_RECORD_BYTES),
    )?;
    let table = table.native.finish().map_err(binary)?;
    encode_program(
        BinaryProgramKind::TerminalCollection,
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
                bytes: &records,
            },
        ],
        io,
    )
    .map_err(binary)
}

pub(super) fn decode(
    bytes: &[u8],
    sessions: &[&TerminalRelationSession],
    limits: TerminalCollectionLimits,
    io: BinaryIoLimits,
) -> Result<TerminalCollectionPlan> {
    let envelope = inspect_program(bytes, io).map_err(binary)?;
    if envelope.kind() != BinaryProgramKind::TerminalCollection
        || envelope.sections().iter().map(|s| s.tag).ne([
            SectionTag::SYMBOLICA_STATE,
            SectionTag::COEFFICIENTS,
            SectionTag::PROGRAM,
        ])
    {
        return Err(binary("not a terminal collection sidecar"));
    }
    let bytes = envelope.section(SectionTag::PROGRAM).expect("checked");
    check(
        "collection structural bytes",
        bytes.len(),
        io.max_program_bytes.min(MAX_RECORD_BYTES),
    )?;
    let (stored, used): (Record, usize) = bincode::decode_from_slice(
        bytes,
        bincode::config::standard().with_limit::<MAX_RECORD_BYTES>(),
    )
    .map_err(binary)?;
    if used != bytes.len() || stored.schema != 1 {
        return Err(binary("unsupported or trailing collection records"));
    }
    TerminalCollectionPlan::validate_sessions(sessions, limits)?;
    check(
        "collection families",
        stored.families.len(),
        limits.diagonal.aliases.max_families,
    )?;
    if stored.families.len() != sessions.len() {
        return Err(binary("collection session family inventory differs"));
    }
    check(
        "collection proof groups",
        stored.proofs.len(),
        limits
            .diagonal
            .aliases
            .max_families
            .saturating_mul(limits.max_proof_layers),
    )?;
    check(
        "collection proof layers",
        stored.layer_ends.len(),
        limits.max_proof_layers,
    )?;
    let layer_ends = stored
        .layer_ends
        .iter()
        .map(|&n| usize::try_from(n).map_err(binary))
        .collect::<Result<Vec<_>>>()?;
    let families: BTreeMap<_, _> = sessions
        .iter()
        .map(|s| (s.family_owner().fingerprint(), s.family_owner()))
        .collect();
    let owner = |index: u32| -> Result<Arc<IntegralFamily>> {
        let (fingerprint, _) = stored
            .families
            .get(index as usize)
            .ok_or_else(|| binary("invalid family index"))?;
        families
            .get(fingerprint.as_str())
            .map(|f| Arc::clone(f))
            .ok_or(VacuumCollectionError::WrongFamily)
    };
    let mut proofs = Vec::new();
    for proof in &stored.proofs {
        check(
            "proof family inventories",
            proof.inputs.len(),
            limits.diagonal.aliases.max_families,
        )?;
        let mut inputs = Vec::new();
        let mut total = 0usize;
        for (index, keys) in &proof.inputs {
            total = total.saturating_add(keys.len());
            check(
                "proof input keys",
                total,
                limits.diagonal.aliases.max_terminals,
            )?;
            let family = owner(*index)?;
            let mut inventory = BTreeSet::new();
            for powers in keys {
                if powers.len() != family.denominator_count() {
                    return Err(VacuumCollectionError::WrongArity);
                }
                let key = IntegralKey::try_new(powers.clone()).map_err(binary)?;
                if !inventory.insert(key) {
                    return Err(binary("duplicate proof input key"));
                }
            }
            inputs.push((family, inventory));
        }
        proofs.push(Arc::new(VacuumDiagonalCollectionPlan::prepare(
            &inputs,
            limits.diagonal,
        )?));
    }
    let plan = TerminalCollectionPlan::compose(sessions, proofs, layer_ends, limits)?;
    let (expected, table) = record(&plan, io)?;
    if stored != expected {
        return Err(binary(
            "stored maps, provenance, guards or session binding failed exact replay",
        ));
    }
    // First-occurrence IDs are deterministic. Compare every native coefficient
    // after state-aware import rather than comparing process-dependent bytes.
    let actual = DecodedCoefficientTable::import_generated(
        envelope
            .section(SectionTag::SYMBOLICA_STATE)
            .expect("checked"),
        envelope.section(SectionTag::COEFFICIENTS).expect("checked"),
        io,
    )
    .map_err(binary)?;
    let expected = table.native.finish().map_err(binary)?;
    let expected = DecodedCoefficientTable::import_generated(&expected.state, &expected.atoms, io)
        .map_err(binary)?;
    if actual.len() != expected.len() {
        return Err(binary("collection coefficient dictionary differs"));
    }
    for i in 0..actual.len() {
        let id = CoefficientId::try_from_index(i).map_err(binary)?;
        if actual.coefficient(id).map_err(binary)? != expected.coefficient(id).map_err(binary)? {
            return Err(binary("collection coefficient failed exact replay"));
        }
    }
    Ok(plan)
}
