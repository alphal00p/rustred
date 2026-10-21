//! Separate discovery checkpoint. Only the immutable old exact baseline is
//! inherited authority; new terminal maps are rebuilt from exactly replayed
//! circuits. Numeric U and its structural ancestry are discovery evidence only.
use super::*;
use crate::persistence::{
    BinaryProgramKind, BinarySection, CoefficientId, CoefficientTableBuilder,
    DecodedCoefficientTable, SectionTag, encode_program, inspect_program,
};
use crate::solver::SectorExecutor;
use symbolica::domains::Ring;
use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};
use symbolica::prelude::Integer;
use symbolica::tensors::sparse::SparseMatrix;

const VERSION: u32 = 1001;

#[derive(bincode::Encode, bincode::Decode, PartialEq, Eq, PartialOrd, Ord)]
enum Source {
    Inherited(u64),
    Ordinary(u64, u64),
}
#[derive(bincode::Encode, bincode::Decode)]
struct Options {
    batch: u64,
    samples: u64,
    candidates: u64,
    exponent: u16,
    polynomial_terms: u64,
    operations: u64,
    degree: u16,
    reconstruction: [u64; 10],
}
#[derive(bincode::Encode, bincode::Decode)]
struct Circuit {
    scanned_sources: u64,
    sources: Vec<Source>,
    weights: Vec<(u64, u32)>,
    image: Vec<(Vec<i64>, u32)>,
    conditions: Vec<u32>,
}
#[derive(bincode::Encode, bincode::Decode)]
enum Status {
    Pending,
    Rejected(String),
    Accepted(u64),
}
#[derive(bincode::Encode, bincode::Decode)]
struct Proposal {
    target: Vec<i64>,
    sources: Vec<u64>,
    status: Status,
}
#[derive(bincode::Encode, bincode::Decode)]
struct Image {
    prime: u64,
    sample: u64,
    point: Vec<u64>,
    columns: Vec<Vec<i64>>,
    pointers: Vec<u64>,
    column_ids: Vec<u32>,
    values: Vec<u64>,
    accepted_sources: Vec<u64>,
    parents: Vec<Vec<u64>>,
    pivots: Vec<u32>,
    processed: u64,
}
#[derive(bincode::Encode, bincode::Decode)]
struct Record {
    version: u32,
    baseline_blake3: [u8; 32],
    options: Options,
    initial_basis: u64,
    start: u64,
    next: u64,
    ordinary_row_ids: Vec<String>,
    sources: Vec<Source>,
    image: Image,
    proposals: Vec<Proposal>,
    circuits: Vec<Circuit>,
    misses: u64,
}

fn binary(error: impl fmt::Display) -> TerminalRelationError {
    TerminalRelationError::Binary(error.to_string())
}
fn wire(value: usize) -> Result<u64, TerminalRelationError> {
    u64::try_from(value).map_err(binary)
}
fn host(value: u64) -> Result<usize, TerminalRelationError> {
    usize::try_from(value).map_err(binary)
}
// Operational unlimited grants have one architecture-independent wire marker;
// physical source/count records still use strict checked host conversions.
fn grant_wire(value: usize) -> Result<u64, TerminalRelationError> {
    if value == usize::MAX {
        Ok(u64::MAX)
    } else {
        wire(value)
    }
}
fn grant_host(value: u64) -> Result<usize, TerminalRelationError> {
    if value == u64::MAX {
        Ok(usize::MAX)
    } else {
        host(value)
    }
}
fn source(value: &TerminalRelationModularSource) -> Result<Source, TerminalRelationError> {
    Ok(match *value {
        TerminalRelationModularSource::InheritedBasis { ordinal } => {
            Source::Inherited(wire(ordinal)?)
        }
        TerminalRelationModularSource::Ordinary {
            seed_ordinal,
            source_ordinal,
        } => Source::Ordinary(wire(seed_ordinal)?, wire(source_ordinal)?),
    })
}
fn native_source(value: Source) -> Result<TerminalRelationModularSource, TerminalRelationError> {
    Ok(match value {
        Source::Inherited(ordinal) => TerminalRelationModularSource::InheritedBasis {
            ordinal: host(ordinal)?,
        },
        Source::Ordinary(seed, ordinal) => TerminalRelationModularSource::Ordinary {
            seed_ordinal: host(seed)?,
            source_ordinal: host(ordinal)?,
        },
    })
}

fn options(value: TerminalRelationModularOptions) -> Result<Options, TerminalRelationError> {
    value.validate()?;
    let r = value.reconstruction;
    let numbers = [
        r.max_rows,
        r.max_columns,
        r.max_input_nonzeros,
        r.max_coefficient_terms,
        r.max_probes,
        r.max_attempts,
        r.max_primes,
        r.max_cached_images,
        r.max_cached_values,
        r.max_weight_slots,
    ];
    let mut reconstruction = [0; 10];
    for (to, from) in reconstruction.iter_mut().zip(numbers) {
        *to = grant_wire(from)?;
    }
    Ok(Options {
        batch: grant_wire(value.batch_rows)?,
        samples: grant_wire(value.discovery_samples)?,
        candidates: grant_wire(value.max_candidates)?,
        exponent: r.arithmetic.max_exponent,
        polynomial_terms: grant_wire(r.arithmetic.max_polynomial_terms)?,
        operations: grant_wire(r.arithmetic.max_term_operations)?,
        degree: r.max_degree,
        reconstruction,
    })
}
fn native_options(value: Options) -> Result<TerminalRelationModularOptions, TerminalRelationError> {
    let n = value.reconstruction;
    let options = TerminalRelationModularOptions {
        batch_rows: grant_host(value.batch)?,
        discovery_samples: grant_host(value.samples)?,
        max_candidates: grant_host(value.candidates)?,
        reconstruction: ProjectedSourceWeightLimits {
            arithmetic: ExactAlgebraLimits {
                max_exponent: value.exponent,
                max_polynomial_terms: grant_host(value.polynomial_terms)?,
                max_term_operations: grant_host(value.operations)?,
            },
            max_rows: grant_host(n[0])?,
            max_columns: grant_host(n[1])?,
            max_input_nonzeros: grant_host(n[2])?,
            max_coefficient_terms: grant_host(n[3])?,
            max_degree: value.degree,
            max_probes: grant_host(n[4])?,
            max_attempts: grant_host(n[5])?,
            max_primes: grant_host(n[6])?,
            max_cached_images: grant_host(n[7])?,
            max_cached_values: grant_host(n[8])?,
            max_weight_slots: grant_host(n[9])?,
        },
    };
    options.validate()?;
    Ok(options)
}

pub(super) fn encode(
    value: &TerminalRelationModularSession,
    io: BinaryIoLimits,
) -> Result<Vec<u8>, TerminalRelationError> {
    let mut table = CoefficientTableBuilder::new(io);
    let mut intern = |c: &Coefficient| table.intern(c).map(|id| id.index() as u32).map_err(binary);
    let circuits = value
        .circuits
        .iter()
        .map(|c| {
            Ok(Circuit {
                scanned_sources: wire(c.scanned_sources)?,
                sources: c.sources.iter().map(source).collect::<Result<_, _>>()?,
                weights: c
                    .weights
                    .iter()
                    .map(|(&slot, c)| Ok((wire(slot)?, intern(c)?)))
                    .collect::<Result<_, TerminalRelationError>>()?,
                image: c
                    .image
                    .iter()
                    .map(|(k, c)| Ok((k.powers().to_vec(), intern(c)?)))
                    .collect::<Result<_, TerminalRelationError>>()?,
                conditions: c
                    .nonzero_conditions
                    .iter()
                    .map(&mut intern)
                    .collect::<Result<_, _>>()?,
            })
        })
        .collect::<Result<_, TerminalRelationError>>()?;
    let image = &value.image;
    let field = Zp64::new(image.prime);
    let record = Record {
        version: VERSION,
        baseline_blake3: *blake3::hash(&value.baseline).as_bytes(),
        options: options(value.options)?,
        initial_basis: wire(value.initial_basis_rows)?,
        start: wire(value.start_ordinal)?,
        next: wire(value.next_ordinal)?,
        ordinary_row_ids: (0..value.session.sources.len())
            .map(|i| {
                value
                    .session
                    .sources
                    .row_id(i)
                    .expect("validated source ordinal")
                    .stable_string()
            })
            .collect(),
        sources: value
            .rows
            .iter()
            .map(|r| source(&r.identity))
            .collect::<Result<_, _>>()?,
        image: Image {
            prime: image.prime,
            sample: image.sample,
            point: image.point.clone(),
            columns: image.columns.iter().map(|k| k.powers().to_vec()).collect(),
            pointers: image
                .reducer
                .u()
                .row_ptrs()
                .iter()
                .copied()
                .map(wire)
                .collect::<Result<_, _>>()?,
            column_ids: image.reducer.u().col_idcs().clone(),
            values: image
                .reducer
                .u()
                .values()
                .iter()
                .map(|v| field.from_element(v))
                .collect(),
            accepted_sources: image
                .accepted_sources
                .iter()
                .copied()
                .map(wire)
                .collect::<Result<_, _>>()?,
            parents: image
                .parents
                .iter()
                .map(|r| r.iter().copied().map(wire).collect())
                .collect::<Result<_, _>>()?,
            pivots: image.pivot_columns.clone(),
            processed: wire(image.rows_processed)?,
        },
        proposals: value
            .candidates
            .iter()
            .map(|c| {
                Ok(Proposal {
                    target: c.target.powers().to_vec(),
                    sources: c
                        .sources
                        .iter()
                        .copied()
                        .map(wire)
                        .collect::<Result<_, _>>()?,
                    status: match &c.status {
                        TerminalRelationModularProposalStatus::Pending => Status::Pending,
                        TerminalRelationModularProposalStatus::Rejected { reason } => {
                            Status::Rejected(reason.clone())
                        }
                        TerminalRelationModularProposalStatus::Accepted { circuit } => {
                            Status::Accepted(wire(*circuit)?)
                        }
                    },
                })
            })
            .collect::<Result<_, TerminalRelationError>>()?,
        circuits,
        misses: wire(value.reconstruction_misses)?,
    };
    let program = bincode::encode_to_vec(record, bincode::config::standard()).map_err(binary)?;
    let coefficients = table.finish().map_err(binary)?;
    encode_program(
        BinaryProgramKind::TerminalRelations,
        &[
            BinarySection {
                tag: SectionTag::SYMBOLICA_STATE,
                bytes: &coefficients.state,
            },
            BinarySection {
                tag: SectionTag::COEFFICIENTS,
                bytes: &coefficients.atoms,
            },
            BinarySection {
                tag: SectionTag::PROGRAM,
                bytes: &program,
            },
            BinarySection {
                tag: SectionTag::VALUES,
                bytes: &value.baseline,
            },
        ],
        io,
    )
    .map_err(binary)
}

fn row_index(
    identity: &TerminalRelationModularSource,
    initial: usize,
    start: usize,
    next: usize,
    sources: usize,
) -> Result<usize, TerminalRelationError> {
    match *identity {
        TerminalRelationModularSource::InheritedBasis { ordinal } if ordinal < initial => {
            Ok(ordinal)
        }
        TerminalRelationModularSource::Ordinary {
            seed_ordinal,
            source_ordinal,
        } if source_ordinal < sources => {
            let ordinal = seed_ordinal
                .checked_mul(sources)
                .and_then(|n| n.checked_add(source_ordinal))
                .ok_or_else(|| binary("circuit source overflow"))?;
            if ordinal < start || ordinal >= next {
                return Err(binary(
                    "circuit source is outside the scanned ordinary inventory",
                ));
            }
            initial
                .checked_add(ordinal - start)
                .ok_or_else(|| binary("circuit registry overflow"))
        }
        _ => Err(binary(
            "circuit source identity is outside the immutable baseline",
        )),
    }
}

pub(super) fn decode(
    bytes: &[u8],
    limits: TerminalRelationLimits,
    io: BinaryIoLimits,
) -> Result<TerminalRelationModularSession, TerminalRelationError> {
    let envelope = inspect_program(bytes, io).map_err(binary)?;
    if envelope.kind() != BinaryProgramKind::TerminalRelations
        || envelope.sections().iter().map(|s| s.tag).ne([
            SectionTag::SYMBOLICA_STATE,
            SectionTag::COEFFICIENTS,
            SectionTag::PROGRAM,
            SectionTag::VALUES,
        ])
    {
        return Err(binary("not a modular terminal-circuit checkpoint"));
    }
    let data = envelope.section(SectionTag::PROGRAM).unwrap();
    // Outer framing is caller-bounded. As with the native coefficient import,
    // these are generated checkpoint records, not a hostile-input sandbox.
    let (record, used): (Record, usize) =
        bincode::decode_from_slice(data, bincode::config::standard()).map_err(binary)?;
    if record.version != VERSION || used != data.len() {
        return Err(binary("unsupported or trailing modular checkpoint records"));
    }
    let baseline = envelope.section(SectionTag::VALUES).unwrap();
    if blake3::hash(baseline).as_bytes() != &record.baseline_blake3 {
        return Err(binary("modular exact baseline digest differs"));
    }
    let mut session = TerminalRelationSession::from_native_bytes(baseline, limits, io)?;
    if session.assistance.is_some() || session.rebuild_cursor != session.rebuild.len() {
        return Err(binary(
            "modular baseline has unsupported pending exact work",
        ));
    }
    let initial = host(record.initial_basis)?;
    let start = host(record.start)?;
    let next = host(record.next)?;
    let source_count = session.sources.len();
    if record.ordinary_row_ids.len() != source_count
        || record
            .ordinary_row_ids
            .iter()
            .enumerate()
            .any(|(i, saved)| {
                session
                    .sources
                    .row_id(i)
                    .is_none_or(|id| id.stable_string() != *saved)
            })
    {
        return Err(binary(
            "ordinary source RowId ordering differs from its checkpoint",
        ));
    }
    let total = session
        .seeds
        .len()
        .checked_mul(source_count)
        .ok_or_else(|| binary("modular source inventory overflow"))?;
    let expected_start = session
        .seed_cursor
        .checked_mul(source_count)
        .and_then(|n| n.checked_add(session.source_cursor))
        .ok_or_else(|| binary("modular baseline cursor overflow"))?;
    if initial != session.reducer.u().nrows() as usize
        || start != expected_start
        || next < start
        || next > total
    {
        return Err(binary("modular cursor/baseline binding differs"));
    }
    let count = initial
        .checked_add(next - start)
        .ok_or_else(|| binary("modular registry count overflow"))?;
    if record.sources.len() != count {
        return Err(binary(
            "modular source count differs from its exact cursor interval",
        ));
    }
    check(
        "modular source identities",
        count,
        io.max_collection_entries,
    )?;
    let mut rows: Vec<_> = session
        .basis_rows()
        .into_iter()
        .enumerate()
        .map(|(ordinal, terms)| SourceRow {
            identity: TerminalRelationModularSource::InheritedBasis { ordinal },
            terms,
            conditions: Vec::new(),
        })
        .collect();
    for ordinal in start..next {
        let (raw, conditions) = session.sources.row(
            &session.seeds[ordinal / source_count],
            ordinal % source_count,
            session.limits.algebra,
        )?;
        let terms = session.normalized_row(&raw)?;
        rows.push(SourceRow {
            identity: TerminalRelationModularSource::Ordinary {
                seed_ordinal: ordinal / source_count,
                source_ordinal: ordinal % source_count,
            },
            terms,
            conditions,
        });
    }
    for (saved, row) in record.sources.into_iter().zip(&rows) {
        if native_source(saved)? != row.identity {
            return Err(binary("modular source registry changed order or identity"));
        }
    }
    let options = native_options(record.options)?;
    let table = DecodedCoefficientTable::import_generated(
        envelope.section(SectionTag::SYMBOLICA_STATE).unwrap(),
        envelope.section(SectionTag::COEFFICIENTS).unwrap(),
        io,
    )
    .map_err(binary)?;
    let family = session.family.clone();
    let coefficient = |id: u32| -> Result<Coefficient, TerminalRelationError> {
        let value = table
            .coefficient(CoefficientId::try_from_index(id as usize).map_err(binary)?)
            .map_err(binary)?;
        family
            .coefficient_context()
            .validate_with_limits(value, io.exact_algebra)
            .map_err(binary)?;
        Ok(value.clone())
    };
    let key = |values: Vec<i64>| -> Result<IntegralKey, TerminalRelationError> {
        if values.len() != family.denominator_count() {
            return Err(binary("modular integral arity differs from its baseline"));
        }
        IntegralKey::try_new(values).map_err(binary)
    };
    let mut circuits = Vec::with_capacity(record.circuits.len());
    for encoded in record.circuits {
        let scanned_sources = host(encoded.scanned_sources)?;
        if scanned_sources < initial || scanned_sources > rows.len() {
            return Err(binary("circuit acceptance prefix is out of range"));
        }
        let sources = encoded
            .sources
            .into_iter()
            .map(native_source)
            .collect::<Result<Vec<_>, _>>()?;
        let indices = sources
            .iter()
            .map(|s| row_index(s, initial, start, next, source_count))
            .collect::<Result<Vec<_>, _>>()?;
        if indices.iter().any(|&i| i >= scanned_sources) || indices.windows(2).any(|p| p[0] >= p[1])
        {
            return Err(binary(
                "circuit sources are not a distinct ordered scanned frame",
            ));
        }
        let mut weights = BTreeMap::new();
        for (slot, id) in encoded.weights {
            if weights.insert(host(slot)?, coefficient(id)?).is_some() {
                return Err(binary("duplicate circuit weight slot"));
            }
        }
        let mut image = TerminalRelationRow::new();
        for (powers, id) in encoded.image {
            if image.insert(key(powers)?, coefficient(id)?).is_some() {
                return Err(binary("duplicate circuit physical image column"));
            }
        }
        let nonzero_conditions = encoded
            .conditions
            .into_iter()
            .map(&coefficient)
            .collect::<Result<Vec<_>, _>>()?;
        circuits.push(TerminalRelationModularCircuit {
            scanned_sources,
            sources,
            weights,
            image,
            nonzero_conditions,
        });
    }
    // Decode closures own only the immutable family, not the mutable session.
    // Rebuild maps from exact full-column products, never from numeric pivots.
    let mut last_scanned = initial;
    for circuit in &circuits {
        if circuit.scanned_sources < last_scanned {
            return Err(binary("circuit acceptance chronology regressed"));
        }
        last_scanned = circuit.scanned_sources;
        let indices = circuit
            .sources
            .iter()
            .map(|s| row_index(s, initial, start, next, source_count))
            .collect::<Result<Vec<_>, _>>()?;
        let selected: Vec<_> = indices.iter().map(|&i| &rows[i]).collect();
        let mut required = session.conditions.clone();
        for row in &rows[..circuit.scanned_sources] {
            for guard in &row.conditions {
                if !required.contains(guard) {
                    required.push(guard.clone());
                }
            }
            for c in row.terms.values() {
                if !c.denominator.is_constant() {
                    let guard: Coefficient = c.denominator.clone().into();
                    if !required.contains(&guard) {
                        required.push(guard);
                    }
                }
            }
        }
        validation::validate_circuit(&session, &selected, &required, circuit)?;
        let mut targets: Vec<_> = circuit.image.keys().cloned().collect();
        elimination::sort_keys(&mut targets);
        let column = session.columns.len() as u32 - 1 - session.column_offsets[&targets[0]];
        if session.reducer.pivots()[column as usize].is_some() {
            return Err(binary("accepted circuit target was already an exact pivot"));
        }
        let before = session.terminal_relations;
        session.add_row(circuit.image.clone())?;
        if session.terminal_relations != before + 1 {
            return Err(binary("accepted circuit has no new exact terminal pivot"));
        }
        for guard in &circuit.nonzero_conditions {
            if !session.conditions.contains(guard) {
                session.conditions.push(guard.clone());
            }
        }
    }
    let image = decode_image(record.image, &session, &rows, io)?;
    let mut candidates = Vec::with_capacity(record.proposals.len());
    let mut accepted = vec![0usize; circuits.len()];
    for encoded in record.proposals {
        let target = key(encoded.target)?;
        if !session.terminals.contains(&target) {
            return Err(binary("modular proposal targets an unknown terminal"));
        }
        let sources = encoded
            .sources
            .into_iter()
            .map(host)
            .collect::<Result<Vec<_>, _>>()?;
        if sources.is_empty()
            || sources.iter().any(|&s| s >= rows.len())
            || sources.windows(2).any(|p| p[0] >= p[1])
        {
            return Err(binary("modular proposal source frame is invalid"));
        }
        let status = match encoded.status {
            Status::Pending => TerminalRelationModularProposalStatus::Pending,
            Status::Rejected(reason) => TerminalRelationModularProposalStatus::Rejected { reason },
            Status::Accepted(slot) => {
                let slot = host(slot)?;
                let circuit = circuits
                    .get(slot)
                    .ok_or_else(|| binary("modular accepted proposal has no exact circuit"))?;
                let mut keys: Vec<_> = circuit.image.keys().cloned().collect();
                elimination::sort_keys(&mut keys);
                if keys.first() != Some(&target) {
                    return Err(binary(
                        "accepted proposal target differs from its exact circuit",
                    ));
                }
                if sources.len() != circuit.sources.len()
                    || sources.iter().zip(&circuit.sources).any(|(&index, id)| {
                        index >= circuit.scanned_sources || &rows[index].identity != id
                    })
                {
                    return Err(binary(
                        "accepted proposal sources differ from its exact circuit",
                    ));
                }
                accepted[slot] += 1;
                TerminalRelationModularProposalStatus::Accepted { circuit: slot }
            }
        };
        candidates.push(Candidate {
            target,
            sources,
            status,
        });
    }
    if accepted.iter().any(|&n| n != 1) {
        return Err(binary(
            "exact modular circuits are not in bijection with accepted proposals",
        ));
    }
    Ok(TerminalRelationModularSession {
        baseline: baseline.to_vec(),
        session,
        options,
        initial_basis_rows: initial,
        start_ordinal: start,
        next_ordinal: next,
        rows,
        image,
        candidates,
        circuits,
        reconstruction_misses: host(record.misses)?,
        executor: SectorExecutor::new(1).map_err(invalid)?,
    })
}

fn decode_image(
    encoded: Image,
    session: &TerminalRelationSession,
    rows: &[SourceRow],
    io: BinaryIoLimits,
) -> Result<discovery::DiscoveryImage, TerminalRelationError> {
    if encoded.prime <= 5 || encoded.prime >= 1 << 63 || !Integer::from(encoded.prime).is_prime(0) {
        return Err(binary(
            "modular checkpoint field is not an admitted native prime",
        ));
    }
    if encoded.point.len() != session.family.coefficient_context().variables().len()
        || encoded.point.iter().any(|&n| n >= encoded.prime)
    {
        return Err(binary("modular generic point has invalid coordinates"));
    }
    let columns = encoded
        .columns
        .into_iter()
        .map(|values| {
            if values.len() != session.family.denominator_count() {
                return Err(binary("modular column arity differs"));
            }
            IntegralKey::try_new(values).map_err(binary)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let unique: BTreeSet<_> = columns.iter().collect();
    if unique.len() != columns.len()
        || session.columns.iter().any(|k| !unique.contains(k))
        || rows
            .iter()
            .flat_map(|r| r.terms.keys())
            .any(|k| !unique.contains(k))
    {
        return Err(binary(
            "modular columns omit or duplicate physical source support",
        ));
    }
    check(
        "modular column records",
        columns.len(),
        io.max_collection_entries,
    )?;
    check(
        "modular nonzero records",
        encoded.values.len(),
        io.max_collection_entries,
    )?;
    let width = u32::try_from(columns.len())
        .map_err(binary)?
        .checked_add(1)
        .ok_or_else(|| binary("modular sentinel overflow"))?;
    let field = Zp64::new(encoded.prime);
    if encoded.values.iter().any(|&n| n == 0 || n >= encoded.prime)
        || encoded.column_ids.iter().any(|&i| i >= width - 1)
    {
        return Err(binary(
            "modular U contains noncanonical residues or a nonzero sentinel",
        ));
    }
    let pointers = encoded
        .pointers
        .into_iter()
        .map(host)
        .collect::<Result<Vec<_>, _>>()?;
    let count = pointers
        .len()
        .checked_sub(1)
        .ok_or_else(|| binary("missing modular CSR pointers"))?;
    let matrix = SparseMatrix::try_from_csr(
        u32::try_from(count).map_err(binary)?,
        width,
        encoded
            .values
            .into_iter()
            .map(|n| field.to_element(n))
            .collect(),
        pointers,
        encoded.column_ids,
        field.clone(),
    )
    .map_err(binary)?;
    let accepted_sources = encoded
        .accepted_sources
        .into_iter()
        .map(host)
        .collect::<Result<Vec<_>, _>>()?;
    let parents = encoded
        .parents
        .into_iter()
        .map(|r| r.into_iter().map(host).collect())
        .collect::<Result<Vec<Vec<_>>, _>>()?;
    if accepted_sources.len() != count
        || parents.len() != count
        || encoded.pivots.len() != count
        || accepted_sources.iter().any(|&s| s >= rows.len())
        || accepted_sources.windows(2).any(|p| p[0] >= p[1])
        || host(encoded.processed)? != rows.len()
    {
        return Err(binary("modular discovery graph/source counts disagree"));
    }
    let mut reducer = SparseRowReducer::new(width, field, LuLMode::Pattern);
    for (row, span) in matrix.row_ptrs().windows(2).enumerate() {
        let range = span[0]..span[1];
        if range.is_empty()
            || matrix.col_idcs()[range.start] != encoded.pivots[row]
            || !matrix.field().is_one(&matrix.values()[range.start])
            || parents[row]
                .iter()
                .any(|&p| p >= row || encoded.pivots[p] >= encoded.pivots[row])
            || parents[row].iter().collect::<BTreeSet<_>>().len() != parents[row].len()
        {
            return Err(binary("modular U pivot or ancestry is malformed/cyclic"));
        }
        if reducer.add_row(&matrix.values()[range.clone()], &matrix.col_idcs()[range])
            != Some(encoded.pivots[row])
        {
            return Err(binary(
                "modular triangular replay changed its pivot chronology",
            ));
        }
    }
    if reducer.u() != &matrix {
        return Err(binary(
            "native modular triangular replay changed its matrix",
        ));
    }
    let column_offsets = elimination::column_offsets(&columns);
    Ok(discovery::DiscoveryImage {
        prime: encoded.prime,
        sample: encoded.sample,
        point: encoded.point,
        columns,
        column_offsets,
        reducer,
        accepted_sources,
        parents,
        pivot_columns: encoded.pivots,
        rows_processed: host(encoded.processed)?,
    })
}

#[cfg(test)]
pub(super) fn with_changed_ordinary_row_id(bytes: &[u8]) -> Result<Vec<u8>, TerminalRelationError> {
    let io = BinaryIoLimits::default();
    let envelope = inspect_program(bytes, io).map_err(binary)?;
    let data = envelope.section(SectionTag::PROGRAM).unwrap();
    let (mut record, _): (Record, usize) =
        bincode::decode_from_slice(data, bincode::config::standard()).map_err(binary)?;
    record.ordinary_row_ids[0].push_str("-mutated");
    let program = bincode::encode_to_vec(record, bincode::config::standard()).map_err(binary)?;
    encode_program(
        BinaryProgramKind::TerminalRelations,
        &[
            BinarySection {
                tag: SectionTag::SYMBOLICA_STATE,
                bytes: envelope.section(SectionTag::SYMBOLICA_STATE).unwrap(),
            },
            BinarySection {
                tag: SectionTag::COEFFICIENTS,
                bytes: envelope.section(SectionTag::COEFFICIENTS).unwrap(),
            },
            BinarySection {
                tag: SectionTag::PROGRAM,
                bytes: &program,
            },
            BinarySection {
                tag: SectionTag::VALUES,
                bytes: envelope.section(SectionTag::VALUES).unwrap(),
            },
        ],
        io,
    )
    .map_err(binary)
}
