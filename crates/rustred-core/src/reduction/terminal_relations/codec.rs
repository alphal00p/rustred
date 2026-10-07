//! Portable native checkpoints. Structural integers use fixed RustRed wire
//! types; all mathematical coefficients use the shared Symbolica atom table.
use super::*;
use crate::family::IntegralFamilyLimits;
use crate::persistence::{
    BinaryProgramKind, BinarySection, CoefficientId, CoefficientTableBuilder,
    DecodedCoefficientTable, NativeFamilyRecord, SectionTag, encode_program, inspect_program,
};
use symbolica::tensors::sparse::SparseMatrix;

// This bounds structural decoder allocations, not native algebra. Native atom
// sections retain their independent caller-owned limits. The matching writer
// refuses the same metadata envelope so it never emits an unreadable snapshot.
const MAX_STRUCTURAL_BYTES: usize = 1 << 30;

#[derive(bincode::Encode, bincode::Decode)]
struct Record {
    schema: u32,
    family: NativeFamilyRecord,
    fingerprint: String,
    raw: Vec<Vec<i64>>,
    normalization: Vec<u8>,
    seeds: Vec<Vec<i64>>,
    seed_cursor: u64,
    source_cursor: u64,
    source_count: u64,
    seed_depth: u32,
    columns: Vec<Vec<i64>>,
    alias_keys: Vec<Vec<i64>>,
    column_normalized: bool,
    row_ptrs: Vec<u64>,
    column_ids: Vec<u32>,
    coefficients: Vec<u32>,
    pivots: Vec<Option<u32>>,
    conditions: Vec<u32>,
    rebuild: Vec<Vec<(Vec<i64>, u32)>>,
    rebuild_cursor: u64,
    completed_rebuild_rows: u64,
}
fn binary(error: impl fmt::Display) -> TerminalRelationError {
    TerminalRelationError::Binary(error.to_string())
}

pub(super) fn encode(
    session: &TerminalRelationSession,
    limits: BinaryIoLimits,
) -> Result<Vec<u8>, TerminalRelationError> {
    let mut table = CoefficientTableBuilder::new(limits);
    let family = NativeFamilyRecord::from_family(&session.family, &mut table).map_err(binary)?;
    let mut intern = |c: &Coefficient| table.intern(c).map(|id| id.index() as u32).map_err(binary);
    let u = session.reducer.u();
    let coefficients = u
        .values()
        .iter()
        .map(&mut intern)
        .collect::<Result<Vec<_>, _>>()?;
    let conditions = session
        .conditions
        .iter()
        .map(&mut intern)
        .collect::<Result<Vec<_>, _>>()?;
    let rebuild = session
        .rebuild
        .iter()
        .map(|row| {
            row.iter()
                .map(|(k, c)| Ok((k.powers().to_vec(), intern(c)?)))
                .collect::<Result<Vec<_>, TerminalRelationError>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let record = Record {
        schema: 1,
        family,
        fingerprint: session.family.fingerprint().to_owned(),
        raw: session.raw.iter().map(|k| k.powers().to_vec()).collect(),
        normalization: session
            .normalization
            .encode_native(limits)
            .map_err(binary)?,
        seeds: session.seeds.iter().map(|k| k.powers().to_vec()).collect(),
        seed_cursor: session.seed_cursor as u64,
        source_cursor: session.source_cursor as u64,
        source_count: session.sources.len() as u64,
        seed_depth: session.seed_depth,
        columns: session
            .columns
            .iter()
            .map(|k| k.powers().to_vec())
            .collect(),
        alias_keys: session
            .alias_keys
            .iter()
            .map(|k| k.powers().to_vec())
            .collect(),
        column_normalized: session.column_normalized,
        row_ptrs: u.row_ptrs().iter().map(|n| *n as u64).collect(),
        column_ids: u.col_idcs().clone(),
        coefficients,
        pivots: session.reducer.pivots().clone(),
        conditions,
        rebuild,
        rebuild_cursor: session.rebuild_cursor as u64,
        completed_rebuild_rows: session.completed_rebuild_rows,
    };
    let records = bincode::encode_to_vec(record, bincode::config::standard()).map_err(binary)?;
    check(
        "checkpoint structural bytes",
        records.len(),
        limits.max_program_bytes.min(MAX_STRUCTURAL_BYTES),
    )?;
    let table = table.finish().map_err(binary)?;
    encode_program(
        BinaryProgramKind::TerminalRelations,
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
        limits,
    )
    .map_err(binary)
}

pub(super) fn decode(
    bytes: &[u8],
    limits: TerminalRelationLimits,
    io: BinaryIoLimits,
) -> Result<TerminalRelationSession, TerminalRelationError> {
    let envelope = inspect_program(bytes, io).map_err(binary)?;
    if envelope.kind() != BinaryProgramKind::TerminalRelations
        || envelope.sections().iter().map(|s| s.tag).ne([
            SectionTag::SYMBOLICA_STATE,
            SectionTag::COEFFICIENTS,
            SectionTag::PROGRAM,
        ])
    {
        return Err(binary("not a terminal-relations checkpoint"));
    }
    let data = envelope
        .section(SectionTag::PROGRAM)
        .expect("checked section");
    let (record, used): (Record, usize) = bincode::decode_from_slice(
        data,
        bincode::config::standard().with_limit::<MAX_STRUCTURAL_BYTES>(),
    )
    .map_err(binary)?;
    if used != data.len() || record.schema != 1 {
        return Err(binary("unsupported or trailing terminal-relations records"));
    }
    check("seeds", record.seeds.len(), limits.max_seeds)?;
    check("columns", record.columns.len(), limits.max_columns)?;
    check(
        "stored nonzeros",
        record.coefficients.len(),
        limits.max_nonzeros,
    )?;
    check(
        "independent rows",
        record.row_ptrs.len().saturating_sub(1),
        limits.max_rows,
    )?;
    check("rebuild rows", record.rebuild.len(), limits.max_rows)?;
    check(
        "raw terminals",
        record.raw.len(),
        limits.normalization.max_terminals,
    )?;
    check(
        "generated alias keys",
        record.alias_keys.len(),
        limits
            .max_columns
            .saturating_add(limits.normalization.max_terminals),
    )?;
    let rebuild_terms = record
        .rebuild
        .iter()
        .try_fold(0usize, |n, row| n.checked_add(row.len()))
        .ok_or_else(|| binary("rebuild nonzero count overflow"))?;
    check("rebuild nonzeros", rebuild_terms, limits.max_nonzeros)?;
    check(
        "source conditions",
        record.conditions.len(),
        io.max_collection_entries,
    )?;
    record
        .family
        .validate_shape(IntegralFamilyLimits::default(), io)
        .map_err(binary)?;
    let table = DecodedCoefficientTable::import_generated(
        envelope.section(SectionTag::SYMBOLICA_STATE).unwrap(),
        envelope.section(SectionTag::COEFFICIENTS).unwrap(),
        io,
    )
    .map_err(binary)?;
    let family = Arc::new(
        record
            .family
            .to_family(&table, IntegralFamilyLimits::default(), io)
            .map_err(binary)?,
    );
    if family.fingerprint() != record.fingerprint {
        return Err(binary("family fingerprint changed"));
    }
    let arity = family.denominator_count();
    let key = |v: Vec<i64>| {
        if v.len() != arity {
            return Err(binary("integral-key arity mismatch"));
        }
        IntegralKey::try_new(v).map_err(binary)
    };
    let raw_count = record.raw.len();
    let raw = record
        .raw
        .into_iter()
        .map(&key)
        .collect::<Result<BTreeSet<_>, _>>()?;
    if raw.len() != raw_count {
        return Err(binary("duplicate raw terminals"));
    }
    let normalization = TerminalNormalizationPlan::decode_generated(
        &record.normalization,
        &family,
        &raw,
        OrderingPolicy::SpiredUncutV1,
        limits.normalization,
        io,
    )
    .map_err(binary)?;
    let seeds = record
        .seeds
        .into_iter()
        .map(&key)
        .collect::<Result<Vec<_>, _>>()?;
    if seeds.iter().collect::<BTreeSet<_>>().len() != seeds.len() {
        return Err(binary("duplicate completed-work seeds"));
    }
    let columns = record
        .columns
        .into_iter()
        .map(&key)
        .collect::<Result<Vec<_>, _>>()?;
    if columns.iter().collect::<BTreeSet<_>>().len() != columns.len() {
        return Err(binary("duplicate integral columns"));
    }
    let alias_keys = record
        .alias_keys
        .into_iter()
        .map(&key)
        .collect::<Result<BTreeSet<_>, _>>()?;
    let sources = sources::Sources::new(&family)?;
    let seed_cursor = usize::try_from(record.seed_cursor).map_err(binary)?;
    let source_cursor = usize::try_from(record.source_cursor).map_err(binary)?;
    if record.source_count != sources.len() as u64
        || seed_cursor > seeds.len()
        || source_cursor >= sources.len()
        || (seed_cursor == seeds.len() && source_cursor != 0)
    {
        return Err(binary("source work cursor is inconsistent"));
    }
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
    let conditions = record
        .conditions
        .into_iter()
        .map(&coefficient)
        .collect::<Result<Vec<_>, _>>()?;
    if conditions.iter().any(Coefficient::is_zero) {
        return Err(binary("zero nonzero condition"));
    }
    let values = record
        .coefficients
        .into_iter()
        .map(&coefficient)
        .collect::<Result<Vec<_>, _>>()?;
    let row_ptrs = record
        .row_ptrs
        .into_iter()
        .map(usize::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(binary)?;
    let nrows = row_ptrs
        .len()
        .checked_sub(1)
        .ok_or_else(|| binary("empty CSR pointer array"))?;
    let u = SparseMatrix::try_from_csr(
        u32::try_from(nrows).map_err(binary)?,
        u32::try_from(columns.len()).map_err(binary)?,
        values,
        row_ptrs,
        record.column_ids,
        RationalPolynomialField::new(Z),
    )
    .map_err(binary)?;
    if record.pivots.len() != columns.len() {
        return Err(binary("pivot array width mismatch"));
    }
    let mut checked = vec![None; columns.len()];
    for (row, p) in u.row_ptrs().windows(2).enumerate() {
        if p[0] == p[1]
            || u.values()[p[0]..p[1]].iter().any(Coefficient::is_zero)
            || u.values()[p[0]] != family.coefficient_context().one()
        {
            return Err(binary("invalid normalized sparse pivot row"));
        }
        let pivot = u.col_idcs()[p[0]] as usize;
        if checked[pivot].replace(row as u32).is_some() {
            return Err(binary("duplicate sparse pivot"));
        }
    }
    if checked != record.pivots {
        return Err(binary("pivot array differs from sparse leading columns"));
    }
    let rebuild = record
        .rebuild
        .into_iter()
        .map(|row| {
            let count = row.len();
            let output = row
                .into_iter()
                .map(|(k, c)| Ok((key(k)?, coefficient(c)?)))
                .collect::<Result<TerminalRelationRow, TerminalRelationError>>()?;
            if count != output.len() {
                return Err(binary("duplicate rebuild-row key"));
            }
            Ok(output)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let rebuild_cursor = usize::try_from(record.rebuild_cursor).map_err(binary)?;
    if rebuild_cursor > rebuild.len() {
        return Err(binary("rebuild cursor exceeds row inventory"));
    }
    let terminals = normalization.canonical_terminals().clone();
    let mut session = TerminalRelationSession {
        family,
        raw,
        normalization,
        sources,
        seeds,
        seed_cursor,
        source_cursor,
        seed_depth: record.seed_depth,
        columns,
        terminals,
        alias_keys,
        aliases: BTreeMap::new(),
        column_normalized: record.column_normalized,
        terminal_relations: 0,
        reducer: SparseRowReducer::from_upper_triangular_matrix(u, record.pivots),
        conditions,
        rebuild,
        rebuild_cursor,
        completed_rebuild_rows: record.completed_rebuild_rows,
        limits,
    };
    if !session.alias_keys.is_empty() {
        let (aliases, terminals) = session.make_aliases(&session.alias_keys)?;
        session.aliases = aliases;
        session.terminals = terminals;
    }
    let column_set: BTreeSet<_> = session.columns.iter().cloned().collect();
    if !session.terminals.is_subset(&column_set) {
        return Err(binary("terminal block absent from columns"));
    }
    let mut seen_terminal = false;
    for column in &session.columns {
        if session.terminals.contains(column) {
            seen_terminal = true;
        } else if seen_terminal {
            return Err(binary("auxiliary column occurs after terminal block"));
        }
    }
    session.terminal_relations = session.terminal_relation_count();
    Ok(session)
}
