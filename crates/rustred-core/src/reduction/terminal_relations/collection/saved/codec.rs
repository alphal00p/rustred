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
    feedbacks: Vec<FeedbackRecord>,
}
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
struct FeedbackRecord {
    after_layer: u64,
    local: Vec<(u32, Option<String>, Vec<u32>, Vec<LocalRow>, Vec<Vec<i64>>)>,
    inputs: Vec<Key>,
    equations: Vec<(NativeRow, Vec<(u64, u32)>, Vec<u32>)>,
}
// Existing published sidecars remain valid baseline input for explicit refine.
#[derive(bincode::Encode, bincode::Decode)]
struct LegacyRecord {
    schema: u32,
    families: Vec<(String, NativeFamilyRecord)>,
    predecessors: Vec<PredecessorRecord>,
    proofs: Vec<ProofRecord>,
    layer_ends: Vec<u64>,
    maps: Vec<(Key, NativeRow, Vec<u32>)>,
}

#[cfg(test)]
mod compatibility_tests {
    use super::*;
    use crate::family::AffineDenominator;

    #[test]
    fn schema_one_diagonal_sidecar_remains_valid_refinement_input() {
        let context = CoefficientContext::new(["d"]);
        let family = Arc::new(
            IntegralFamily::new(
                "feedback-legacy-schema",
                vec!["q".into()],
                vec![],
                context.clone(),
                context.parameter("d").unwrap(),
                vec![AffineDenominator::new(
                    context.integer(-1),
                    vec![context.one()],
                )],
                vec![],
                vec![context.zero()],
            )
            .unwrap(),
        );
        let session = TerminalRelationSession::new(
            family,
            BTreeSet::from([
                IntegralKey::try_new([1]).unwrap(),
                IntegralKey::try_new([2]).unwrap(),
            ]),
            0,
            Default::default(),
        )
        .unwrap();
        let plan = TerminalCollectionPlan::prepare(&[&session], Default::default()).unwrap();
        let (record, table) = record(&plan, Default::default()).unwrap();
        let legacy = LegacyRecord {
            schema: 1,
            families: record.families,
            predecessors: record.predecessors,
            proofs: record.proofs,
            layer_ends: record.layer_ends,
            maps: record.maps,
        };
        let structural = bincode::encode_to_vec(legacy, bincode::config::standard()).unwrap();
        let table = table.native.finish().unwrap();
        let bytes = encode_program(
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
                    bytes: &structural,
                },
            ],
            Default::default(),
        )
        .unwrap();
        let loaded = TerminalCollectionPlan::from_native_bytes(
            &bytes,
            &[&session],
            Default::default(),
            Default::default(),
        )
        .unwrap();
        assert_eq!(loaded.statistics(), plan.statistics());
        let refined = loaded
            .refine_with_finite_feedback(&[&session], Default::default())
            .unwrap();
        assert_eq!(refined.remaining_terminals(), plan.remaining_terminals());
        assert_eq!(refined.proofs().len(), plan.proofs().len());
    }
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
    let feedbacks = plan
        .feedbacks
        .iter()
        .map(|feedback| {
            let local = feedback
                .local
                .iter()
                .map(|(id, source)| {
                    Ok((
                        ids[id.as_str()],
                        source.binding.clone(),
                        table.conditions(&source.conditions)?,
                        source
                            .rows
                            .iter()
                            .map(|row| table.local(row))
                            .collect::<Result<_>>()?,
                        source
                            .columns
                            .iter()
                            .map(|key| key.powers().to_vec())
                            .collect(),
                    ))
                })
                .collect::<Result<_>>()?;
            let inputs = feedback
                .result
                .raw
                .iter()
                .map(|key| {
                    (
                        ids[key.family_fingerprint()],
                        key.integral().powers().to_vec(),
                    )
                })
                .collect();
            let equations = feedback
                .result
                .equations
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
            Ok(FeedbackRecord {
                after_layer: feedback.after_layer as u64,
                local,
                inputs,
                equations,
            })
        })
        .collect::<Result<_>>()?;
    Ok((
        Record {
            schema: 2,
            families,
            predecessors,
            proofs,
            layer_ends: plan.layer_ends.iter().map(|&n| n as u64).collect(),
            maps,
            feedbacks,
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
    let (schema, _): (u32, usize) =
        bincode::decode_from_slice(bytes, bincode::config::standard()).map_err(binary)?;
    let (stored, used) = if schema == 1 {
        let (legacy, used): (LegacyRecord, usize) = bincode::decode_from_slice(
            bytes,
            bincode::config::standard().with_limit::<MAX_RECORD_BYTES>(),
        )
        .map_err(binary)?;
        if legacy.schema != 1 {
            return Err(binary("unsupported collection schema"));
        }
        (
            Record {
                schema: 2,
                families: legacy.families,
                predecessors: legacy.predecessors,
                proofs: legacy.proofs,
                layer_ends: legacy.layer_ends,
                maps: legacy.maps,
                feedbacks: Vec::new(),
            },
            used,
        )
    } else {
        bincode::decode_from_slice::<Record, _>(
            bytes,
            bincode::config::standard().with_limit::<MAX_RECORD_BYTES>(),
        )
        .map_err(binary)?
    };
    if used != bytes.len() || stored.schema != 2 {
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
    let actual = DecodedCoefficientTable::import_generated(
        envelope
            .section(SectionTag::SYMBOLICA_STATE)
            .expect("checked"),
        envelope.section(SectionTag::COEFFICIENTS).expect("checked"),
        io,
    )
    .map_err(binary)?;
    check(
        "finite feedback layers",
        stored.feedbacks.len(),
        limits.max_proof_layers,
    )?;
    let all_families: BTreeMap<_, _> = families
        .iter()
        .map(|(id, f)| ((*id).to_owned(), Arc::clone(f)))
        .collect();
    let coefficient = |family: &IntegralFamily, id: u32| -> Result<Coefficient> {
        let value = actual
            .coefficient(CoefficientId::try_from_index(id as usize).map_err(binary)?)
            .map_err(binary)?;
        family
            .coefficient_context()
            .validate_with_limits(value, limits.finite_feedback.algebra.exact_algebra)
            .map_err(algebra)?;
        Ok(value.clone())
    };
    let mut feedbacks = Vec::new();
    for feedback in &stored.feedbacks {
        let after_layer = usize::try_from(feedback.after_layer).map_err(binary)?;
        if after_layer > layer_ends.len() {
            return Err(binary("invalid feedback layer position"));
        }
        let mut local = BTreeMap::new();
        let mut total_rows = 0usize;
        let mut total_terms = 0usize;
        for (id, binding, guards, stored_rows, column_order) in &feedback.local {
            let family = owner(*id)?;
            total_rows = total_rows.saturating_add(stored_rows.len());
            check(
                "finite feedback source rows",
                total_rows,
                limits.finite_feedback.max_source_rows,
            )?;
            check(
                "finite feedback source guards",
                guards.len(),
                limits.finite_feedback.max_conditions,
            )?;
            let conditions = guards
                .iter()
                .map(|&id| coefficient(&family, id))
                .collect::<Result<Vec<_>>>()?;
            let mut rows = Vec::new();
            for row in stored_rows {
                total_terms = total_terms.saturating_add(row.len());
                check(
                    "finite feedback source terms",
                    total_terms,
                    limits.finite_feedback.max_source_terms,
                )?;
                let mut terms = BTreeMap::new();
                for (powers, id) in row {
                    if powers.len() != family.denominator_count() {
                        return Err(VacuumCollectionError::WrongArity);
                    }
                    let key = IntegralKey::try_new(powers.clone()).map_err(binary)?;
                    let value = coefficient(&family, *id)?;
                    if value.is_zero() || terms.insert(key, value).is_some() {
                        return Err(binary("noncanonical feedback source row"));
                    }
                }
                rows.push(terms);
            }
            check(
                "finite feedback ordered columns",
                column_order.len(),
                limits.finite_feedback.max_columns,
            )?;
            let columns = column_order
                .iter()
                .map(|powers| {
                    if powers.len() != family.denominator_count() {
                        return Err(VacuumCollectionError::WrongArity);
                    }
                    IntegralKey::try_new(powers.clone()).map_err(binary)
                })
                .collect::<Result<Vec<_>>>()?;
            if columns.iter().collect::<BTreeSet<_>>().len() != columns.len() {
                return Err(binary("duplicate feedback column"));
            }
            if local
                .insert(
                    family.fingerprint().to_owned(),
                    feedback::LocalRows {
                        binding: binding.clone(),
                        conditions,
                        rows,
                        columns,
                    },
                )
                .is_some()
            {
                return Err(binary("duplicate feedback source family"));
            }
        }
        check(
            "finite feedback targets",
            feedback.inputs.len(),
            limits.finite_feedback.max_columns,
        )?;
        let mut inputs = BTreeSet::new();
        for (id, powers) in &feedback.inputs {
            let family = owner(*id)?;
            if powers.len() != family.denominator_count() {
                return Err(VacuumCollectionError::WrongArity);
            }
            let key = IntegralKey::try_new(powers.clone()).map_err(binary)?;
            if !inputs.insert(VacuumIntegralKey::from_family(&family, key)) {
                return Err(binary("duplicate feedback input"));
            }
        }
        let restored = Arc::new(feedback::FiniteFeedback::replay(
            after_layer,
            local,
            inputs,
            &all_families,
            &proofs,
            &layer_ends,
            &feedbacks,
            limits,
            None,
        )?);
        feedback::authenticate(std::slice::from_ref(&restored), sessions, limits)?;
        feedbacks.push(restored);
    }
    let plan = TerminalCollectionPlan::compose(sessions, proofs, layer_ends, feedbacks, limits)?;
    let (expected, table) = record(&plan, io)?;
    if stored != expected {
        return Err(binary(
            "stored maps, provenance, guards or session binding failed exact replay",
        ));
    }
    // First-occurrence IDs are deterministic. Compare every native coefficient
    // after state-aware import rather than comparing process-dependent bytes.
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
