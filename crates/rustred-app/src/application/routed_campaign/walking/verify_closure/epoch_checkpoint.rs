//! Read-only CP6 adapter for the independent closure oracle.
//!
//! Select latest exactly once, authenticate every referenced immutable file
//! (record payloads during their subsequent oracle consumption),
//! and decode raw images/ledger/edges rather than calling runtime::open (which
//! adopts a new session). No locks, writes, previous-generation fallback,
//! reconstructed lookup index or engine closure calculation are used here.
use super::super::checkpoint::RawCheckpoint;
use super::super::queue::{CompactDomain, Domain, Phase};
use super::epoch_export::{AnchorRow, EpochSections};
use rustred::solver::DomainPowerBounds;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read};
use std::path::{Component, Path};

#[cfg(test)]
mod tests;

const FORMAT: &str = "RUSTRED-WALK-CP6";
const MANIFEST_LIMIT: u64 = 64 << 10;

/// Compare scheduling scalars authenticated by the selected CP6 manifest with
/// the command. An omitted window inherits the saved bound, just as restore
/// does. Only explicit cut options are compared here: historical diagnostic
/// environment-only cuts were not command-bound, and cold proof checking need
/// not recreate that process environment. New public custom cuts are also
/// bound by the request digest, so omitting one still fails overall binding.
pub(super) fn schedule_matches_request(
    manifest: &Value,
    request: &super::super::OwnerDomainWalkRequest,
) -> bool {
    if manifest["format"] != FORMAT {
        return true; // Historical non-resumable exports have no CP6 scalars.
    }
    let rolling = manifest["epoch_rolling"].as_bool();
    let window = manifest["epoch_base_window"].as_u64();
    rolling == Some(request.epoch_rolling)
        && manifest["epoch_result_escrow_jobs"].as_u64()
            == Some(request.epoch_result_escrow_jobs as u64)
        && manifest["epoch_result_escrow_bytes"] == json!(request.epoch_result_escrow_bytes)
        && manifest["epoch_publication_order"].as_str()
            == Some(request.epoch_publication_order.name())
        && request.epoch_cut_size.is_none_or(|n| {
            Some(n as u64)
                == if request.epoch_rolling {
                    manifest["epoch_cut_size"].as_u64()
                } else {
                    window
                }
        })
        && request
            .epoch_window
            .is_none_or(|n| window == Some(n as u64))
}

// Field order is the public canonical manifest digest encoding.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format: String,
    schema: u32,
    generation: u64,
    arity: usize,
    walk_semantics_version: u32,
    resumable: bool,
    files: Vec<FileRef>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileRef {
    key: String,
    file: String,
    count: u64,
    bytes: u64,
    blake3: [u8; 32],
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    manifest: Manifest,
    blake3: [u8; 32],
}

/// Detection only; the actual reader repeats all identity/digest checks.
/// The canonical envelope puts format first, within this bounded prefix.
pub(super) fn present(directory: &Path) -> bool {
    let mut prefix = [0u8; 256];
    File::open(directory.join("latest.json"))
        .and_then(|mut file| file.read(&mut prefix))
        .is_ok_and(|n| String::from_utf8_lossy(&prefix[..n]).contains(FORMAT))
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn number(value: &Value, key: &str) -> Result<u64, String> {
    value[key]
        .as_u64()
        .ok_or_else(|| format!("CP6 missing integer {key}"))
}
fn count(value: &Value, key: &str) -> Result<usize, String> {
    usize::try_from(number(value, key)?).map_err(|_| format!("CP6 {key} exceeds usize"))
}

/// Exact public row shape and ordered one-to-one source-frontier mapping.
/// Query geometry/roles and whether a frontier is actually required are then
/// checked independently against the request/prepared registry by the oracle.
fn input_inventory(inputs: &[Value], frontiers: &[Value]) -> Result<(), String> {
    let mut next_frontier = 0;
    for row in inputs {
        let object = row.as_object().ok_or("CP6 input row object")?;
        let amended = match object.get("amendment") {
            None => false,
            Some(value)
                if value.as_u64().is_some_and(|n| n > 0)
                    && matches!(row["role"].as_str(), Some("required" | "auxiliary"))
                    && row["role_declared"] == true =>
            {
                true
            }
            _ => return Err("CP6 amendment row shape".into()),
        };
        if !row["id"].is_string()
            || !matches!(row["role"].as_str(), Some("required" | "auxiliary"))
            || !row["role_declared"].is_boolean()
        {
            return Err("CP6 input row identity/role shape".into());
        }
        match object.get("domain") {
            Some(Value::Number(id)) if id.as_u64().is_some_and(|n| n < u32::MAX as u64) => {
                if object.len() != 4 + usize::from(amended)
                    || object.contains_key("source_validity_unresolved")
                {
                    return Err("CP6 mapped input has unresolved or unknown fields".into());
                }
            }
            Some(Value::Null) if object.len() == 5 && row["source_validity_unresolved"] == true => {
                if frontiers
                    .get(next_frontier)
                    .is_none_or(|f| f["id"] != row["id"])
                {
                    return Err("CP6 unresolved input/frontier correspondence differs".into());
                }
                next_frontier += 1;
            }
            _ => return Err("CP6 input domain/source obligation shape".into()),
        }
    }
    if next_frontier != frontiers.len() {
        return Err("CP6 input frontier has no unresolved row".into());
    }
    Ok(())
}

pub(super) fn source_frontier_matches<const N: usize>(
    query: &crate::application::routed_campaign::matching::input::Query,
    frontier: &Value,
) -> bool {
    let Ok(owner) = <&[bool; N]>::try_from(query.owner.as_slice()) else {
        return false;
    };
    frontier
        == &json!({"id":query.id,"kind":"initial_route_source_validity_obligation",
        "owner":super::super::mask::<N>(owner),"lower":query.lower,"upper":query.upper,
        "rank":query.rank,"power_bounds":super::super::power_bounds_json(query.powers),
        "reached_missing_rule_claim":false})
}
fn regular(path: &Path) -> io::Result<File> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(invalid("CP6 input is not a regular non-symlink file"));
    }
    File::open(path)
}

/// Constant-buffer authenticated reader. Length/digest are checked on the
/// same open file consumed by the decoder, including skipped sections.
pub(super) struct Input {
    reader: BufReader<File>,
    reference: FileRef,
    read: u64,
    hash: blake3::Hasher,
}
impl Read for Input {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let n = self.reader.read(output)?;
        self.read = self
            .read
            .checked_add(n as u64)
            .ok_or_else(|| invalid("CP6 length overflow"))?;
        if self.read > self.reference.bytes {
            return Err(invalid("CP6 file exceeds authenticated length"));
        }
        self.hash.update(&output[..n]);
        // Record consumers finish through BufRead::lines rather than finish().
        // Authenticate the very bytes parsed, including an empty sidecar. A
        // zero-length destination is not evidence of EOF.
        if n == 0 && !output.is_empty() {
            self.check_digest()?;
        }
        Ok(n)
    }
}
impl Input {
    fn open(directory: &Path, reference: &FileRef) -> io::Result<Self> {
        let mut components = Path::new(&reference.file).components();
        if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
            return Err(invalid("CP6 section path is not one filename"));
        }
        let file = regular(&directory.join(&reference.file))?;
        if file.metadata()?.len() != reference.bytes {
            return Err(invalid("CP6 section length differs"));
        }
        Ok(Self {
            reader: BufReader::with_capacity(32 << 10, file),
            reference: reference.clone(),
            read: 0,
            hash: blake3::Hasher::new(),
        })
    }
    fn read_array<const M: usize>(&mut self) -> io::Result<[u8; M]> {
        let mut bytes = [0; M];
        self.read_exact(&mut bytes)?;
        Ok(bytes)
    }
    fn u8(&mut self) -> io::Result<u8> {
        Ok(self.read_array::<1>()?[0])
    }
    fn u16(&mut self) -> io::Result<u16> {
        Ok(u16::from_le_bytes(self.read_array()?))
    }
    fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.read_array()?))
    }
    fn u64(&mut self) -> io::Result<u64> {
        Ok(u64::from_le_bytes(self.read_array()?))
    }
    fn finish(mut self) -> io::Result<()> {
        if self.read(&mut [0])? != 0 {
            return Err(invalid(
                "CP6 section trailing bytes, length or digest differs",
            ));
        }
        self.check_digest()
    }
    fn check_digest(&self) -> io::Result<()> {
        if self.read != self.reference.bytes
            || self.hash.finalize().as_bytes() != &self.reference.blake3
        {
            return Err(invalid("CP6 section length or digest differs"));
        }
        Ok(())
    }
    fn drain(mut self) -> io::Result<()> {
        io::copy(&mut self, &mut io::sink())?;
        self.finish()
    }
    fn header(&mut self, arity: usize, section: u32) -> io::Result<usize> {
        if &self.read_array::<8>()? != b"EPC6PART"
            || self.u32()? != 1
            || self.u32()? as usize != arity
            || self.u32()? != section
        {
            return Err(invalid("CP6 section framing differs"));
        }
        let count = self.u64()?;
        if count != self.reference.count {
            return Err(invalid("CP6 section count differs"));
        }
        usize::try_from(count).map_err(|_| invalid("CP6 section count exceeds usize"))
    }
    fn fixed(&self, count: usize, width: usize) -> io::Result<()> {
        if (count as u64)
            .checked_mul(width as u64)
            .and_then(|n| n.checked_add(28))
            != Some(self.reference.bytes)
        {
            return Err(invalid("CP6 fixed section width differs"));
        }
        Ok(())
    }
}

/// Captured from the authenticated selected generation, never from a later
/// latest.json. Open one sidecar at a time; load must consume it through EOF.
pub(super) struct RecordRef(FileRef);
impl RecordRef {
    pub(super) fn open(&self, path: &Path, count: usize) -> io::Result<Input> {
        if path.file_name().and_then(|s| s.to_str()) != Some(self.0.file.as_str())
            || count as u64 != self.0.count
        {
            return Err(invalid("CP6 captured record reference differs"));
        }
        Input::open(
            path.parent().ok_or_else(|| invalid("CP6 record path"))?,
            &self.0,
        )
    }
}
fn reserve<T>(count: usize) -> io::Result<Vec<T>> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| invalid("CP6 reader allocation"))?;
    Ok(result)
}
fn json_file(directory: &Path, reference: &FileRef, limit: Option<u64>) -> io::Result<Value> {
    if limit.is_some_and(|limit| reference.bytes > limit) {
        return Err(invalid("CP6 bounded metadata exceeds limit"));
    }
    let mut input = Input::open(directory, reference)?;
    let value = serde_json::from_reader(&mut input).map_err(io::Error::other)?;
    input.finish()?;
    Ok(value)
}
fn rows<T: serde::de::DeserializeOwned>(
    directory: &Path,
    reference: &FileRef,
) -> io::Result<Vec<T>> {
    let input = Input::open(directory, reference)?;
    let mut buffered = BufReader::with_capacity(32 << 10, input);
    let mut result = Vec::new();
    let mut line = String::new();
    while buffered.read_line(&mut line)? != 0 {
        if result.len() as u64 >= reference.count || line.trim().is_empty() {
            return Err(invalid("CP6 row count differs"));
        }
        result
            .try_reserve(1)
            .map_err(|_| invalid("CP6 row allocation"))?;
        result.push(serde_json::from_str(&line).map_err(io::Error::other)?);
        line.clear();
    }
    if result.len() as u64 != reference.count {
        return Err(invalid("CP6 row count differs"));
    }
    buffered.into_inner().finish()?;
    Ok(result)
}
fn image<const N: usize>(input: &mut Input, wire_arity: usize) -> io::Result<CompactDomain<N>> {
    let phase = match input.u8()? {
        0 => Phase::Apply,
        1 => Phase::Route,
        _ => return Err(invalid("CP6 domain phase")),
    };
    let mask = input.u32()?;
    if N < 32 && mask >> N != 0 {
        return Err(invalid("CP6 owner high bits"));
    }
    let rank_flag = input.u8()?;
    let rank_word = input.u32()?;
    if rank_flag > 1 || rank_flag == 0 && rank_word != 0 {
        return Err(invalid("CP6 rank option encoding"));
    }
    let mut lower = reserve(wire_arity)?;
    let mut upper = reserve(wire_arity)?;
    for _ in 0..wire_arity {
        lower.push(u64::from(input.u16()?));
    }
    for _ in 0..wire_arity {
        let word = input.u16()?;
        upper.push((word != u16::MAX).then_some(u64::from(word)));
    }
    let mut option = || -> io::Result<Option<u64>> {
        let flag = input.u8()?;
        let word = input.u64()?;
        if flag > 1 || flag == 0 && word != 0 {
            return Err(invalid("CP6 power option encoding"));
        }
        Ok((flag == 1).then_some(word))
    };
    let powers = DomainPowerBounds {
        max_positive_power: option()?,
        min_power_difference: option()?.map(|v| v as i64),
        max_power_difference: option()?.map(|v| v as i64),
    };
    CompactDomain::restore(&Domain {
        phase,
        owner: std::array::from_fn(|i| mask >> i & 1 != 0),
        lower,
        upper,
        rank: (rank_flag == 1).then_some(rank_word),
        powers,
    })
    .map_err(io::Error::other)
}

pub(super) fn read_raw<const N: usize>(
    directory: &Path,
) -> Result<(RawCheckpoint<N>, EpochSections, Vec<RecordRef>), String> {
    read_inner::<N>(directory).map_err(|error| format!("CP6 raw checkpoint: {error}"))
}
fn read_inner<const N: usize>(
    directory: &Path,
) -> Result<(RawCheckpoint<N>, EpochSections, Vec<RecordRef>), String> {
    let start = std::time::Instant::now();
    let io = |error: io::Error| error.to_string();
    let meta = fs::symlink_metadata(directory).map_err(io)?;
    if !meta.is_dir()
        || meta.file_type().is_symlink()
        || directory.join("epoch-poison").exists()
        || directory.join("epoch-export.json").exists()
        || directory.join("epoch-internal-latest.json").exists()
    {
        return Err("invalid, poisoned or ambiguous CP6 directory".into());
    }
    let mut bytes = Vec::new();
    regular(&directory.join("latest.json"))
        .map_err(io)?
        .take(MANIFEST_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(io)?;
    if bytes.len() as u64 > MANIFEST_LIMIT {
        return Err("manifest too large".into());
    }
    let envelope: Envelope = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let manifest = envelope.manifest;
    if manifest.format != FORMAT
        || manifest.schema != 3
        || manifest.generation == 0
        || !super::super::super::storage::compatible_width(manifest.arity, N)
        || manifest.walk_semantics_version != 4
        || !manifest.resumable
        || manifest.files.len()
            != 15 + usize::from(manifest.files.iter().any(|file| file.key == "rescue"))
        || *blake3::hash(&serde_json::to_vec(&manifest).map_err(|e| e.to_string())?).as_bytes()
            != envelope.blake3
    {
        return Err("manifest identity or digest differs".into());
    }
    let mut names: Vec<String> = (1..=9)
        .map(|n| format!("state-{n}"))
        .chain(
            [
                "meta",
                "owners",
                "inputs",
                "input-frontiers",
                "record-segments",
                "orthants",
            ]
            .map(str::to_owned),
        )
        .collect();
    let rescue_profile = manifest.files.iter().any(|file| file.key == "rescue");
    if rescue_profile {
        names.push("rescue".into());
    }
    let file = |key: &str| -> Result<&FileRef, String> {
        manifest
            .files
            .iter()
            .find(|f| f.key == key)
            .ok_or_else(|| format!("missing {key}"))
    };
    for key in &names {
        let entry = file(key)?;
        let suffix = key.strip_prefix("state-").unwrap_or(key);
        if entry.file != format!("epoch-{:020}-{suffix}.part", manifest.generation) {
            return Err(format!("noncanonical section path {key}"));
        }
    }
    let scalar = json_file(directory, file("meta")?, Some(1 << 20)).map_err(io)?;
    let total = count(&scalar, "watermark")?;
    let p0 = count(&scalar, "p0")?;
    let processed = count(&scalar, "processed_queries")?;
    let query_total = count(&scalar, "total_queries")?;
    let amendments: Vec<super::super::rescue::AmendmentRef> =
        serde_json::from_value(scalar["amendments"].clone()).map_err(|e| e.to_string())?;
    if amendments.len() > super::super::rescue::MAX_AMENDMENTS
        || amendments.is_empty() == rescue_profile
    {
        return Err("CP6 rescue profile differs".into());
    }
    let mut amended_queries = 0u64;
    let mut previous_domain = p0 as u64;
    let mut previous_generation = 0;
    for (index, amendment) in amendments.iter().enumerate() {
        if amendment.sequence != index as u64 + 1
            || amendment.first_input != query_total as u64 + amended_queries
            || amendment.first_domain < previous_domain
            || amendment.first_domain > total as u64
            || amendment.quarantined > amendment.first_domain
            || amendment.resumed_generation == 0
            || amendment.resumed_generation >= manifest.generation
            || amendment.resumed_generation < previous_generation
        {
            return Err("CP6 amendment cursor inventory differs".into());
        }
        previous_domain = amendment.first_domain;
        previous_generation = amendment.resumed_generation;
        amended_queries = amended_queries
            .checked_add(amendment.queries)
            .ok_or("CP6 amendment count overflow")?;
    }
    let rolling = match scalar.get("epoch_rolling") {
        None => false,
        Some(value) => value.as_bool().ok_or("invalid rolling policy")?,
    };
    match scalar
        .get("epoch_publication_order")
        .and_then(Value::as_str)
    {
        None if scalar.get("epoch_publication_order").is_none() => {}
        Some("oldest-prefix") if rolling => {}
        Some("oldest-ready") if rolling => {}
        _ => return Err("invalid epoch publication order".into()),
    }
    if rolling {
        if !(1..=4096).contains(&count(&scalar, "epoch_cut_size")?) {
            return Err("invalid rolling cut size".into());
        }
    } else if scalar.get("epoch_cut_size").is_some() {
        return Err("lockstep scalar has rolling cut size".into());
    }
    let admission = scalar["initial_admission"]
        .as_str()
        .ok_or("missing admission state")?;
    let base_window = count(&scalar, "epoch_base_window")?;
    let escrow_jobs = count(&scalar, "epoch_result_escrow_jobs")?;
    let escrow_bytes = scalar
        .get("epoch_result_escrow_bytes")
        .ok_or("missing epoch escrow byte admission budget")?;
    if !(1..=4096).contains(&base_window)
        || base_window.checked_add(escrow_jobs) != Some(count(&scalar, "lockstep_b")?)
        || (if escrow_jobs == 0 {
            !escrow_bytes.is_null()
        } else {
            !rolling
                || scalar["epoch_publication_order"]
                    .as_str()
                    .is_some_and(|s| s != "oldest-prefix")
                || escrow_bytes
                    .as_u64()
                    .is_none_or(|n| n == 0 || usize::try_from(n).is_err())
        })
    {
        return Err("invalid epoch base/escrow bounds".into());
    }
    if scalar["schema"] != 5
        || scalar["walk_semantics_version"] != 4
        || scalar["record_schema"] != 1
        || scalar["preparation"]
            .as_object()
            .is_none_or(|p| p.len() != 3)
        || scalar["preparation"]["helpers"].as_u64().is_none()
        || ["obligations", "retirements"].iter().any(|key| {
            scalar["preparation"][key]
                .as_u64()
                .is_none_or(|n| n == 0 || n > u64::from(u32::MAX))
        })
        || p0 > total
        || total >= u32::MAX as usize
        || processed > query_total
        || total > count(&scalar, "max_domains")?
        || !(1..=4096).contains(&count(&scalar, "lockstep_b")?)
        || number(&scalar, "k")? >= 1u64 << 48
        || !matches!(admission, "complete" | "in_progress")
        || admission == "complete" && processed != query_total
        || !matches!(scalar["g2"].as_str(), Some("off" | "union"))
        || scalar["g2"] == "off" && scalar["walk"]["g2_records"] != 0
        || scalar["imported_prefix"] != 0
        || scalar["engine_certification_void"] != false
        || count(&scalar, "quarantined")? > total
        || count(&scalar, "abandoned_obligations")? > count(&scalar, "quarantined")?
    {
        return Err("scalar identity or supported scope differs".into());
    }
    let owners: Vec<String> = rows(directory, file("owners")?).map_err(io)?;
    if owners.len() != count(&scalar, "owner_count")?
        || serde_json::to_value(
            blake3::hash(&serde_json::to_vec(&owners).map_err(|e| e.to_string())?).as_bytes(),
        )
        .map_err(|e| e.to_string())?
            != scalar["owners_digest"]
    {
        return Err("owner inventory digest differs".into());
    }
    let inputs: Vec<Value> = rows(directory, file("inputs")?).map_err(io)?;
    let input_frontiers: Vec<Value> = rows(directory, file("input-frontiers")?).map_err(io)?;
    if inputs.len() as u64 != processed as u64 + amended_queries
        || input_frontiers.len() != count(&scalar, "input_frontiers")?
    {
        return Err("input prefix inventory differs".into());
    }
    input_inventory(&inputs, &input_frontiers)?;
    let section = |n: u32| -> Result<(Input, usize), String> {
        let mut input = Input::open(directory, file(&format!("state-{n}"))?).map_err(io)?;
        let count = input.header(manifest.arity, n).map_err(io)?;
        Ok((input, count))
    };
    let (mut domains_input, n) = section(1)?;
    domains_input
        .fixed(n, 37 + 4 * manifest.arity)
        .map_err(io)?;
    if n != total {
        return Err("domain watermark differs".into());
    }
    let mut domains = reserve(n).map_err(io)?;
    for _ in 0..n {
        domains.push(image(&mut domains_input, manifest.arity).map_err(io)?);
    }
    domains_input.finish().map_err(io)?;
    let (mut nodes_input, n) = section(2)?;
    nodes_input.fixed(n, 1).map_err(io)?;
    if n != total {
        return Err("node count differs".into());
    }
    let mut flags = reserve(n).map_err(io)?;
    for _ in 0..n {
        flags.push(nodes_input.u8().map_err(io)?);
    }
    nodes_input.finish().map_err(io)?;
    if flags.iter().any(|flag| flag & !27 != 0) {
        return Err("unsupported CP6 node flag".into());
    }
    let (mut closure_input, n) = section(8)?;
    closure_input.fixed(n, 1).map_err(io)?;
    if n != total && !(n == 0 && scalar["closure"]["unavailable"].is_string()) {
        return Err("closure flag count differs".into());
    }
    for flag in flags.iter_mut().take(n) {
        let closure = closure_input.u8().map_err(io)?;
        if closure & !7 != 0 || *flag & 3 != closure & 3 {
            return Err("closure/node flags differ".into());
        }
        *flag |= closure & 4;
    }
    closure_input.finish().map_err(io)?;
    let (mut ledger_input, n) = section(4)?;
    ledger_input.fixed(n, 8).map_err(io)?;
    if n != total {
        return Err("ledger count differs".into());
    }
    let mut ledger = reserve(n).map_err(io)?;
    for _ in 0..n {
        ledger.push(ledger_input.u64().map_err(io)?);
    }
    ledger_input.finish().map_err(io)?;
    // Decode the rescue bitsets independently; runtime's helper is deliberately
    // not called by the cold authority reader.
    let mut abandoned_bits = Vec::new();
    if !amendments.is_empty() {
        let reference = file("rescue")?;
        if reference.count != total as u64 || reference.bytes != 16 + total.div_ceil(64) as u64 * 16
        {
            return Err("CP6 rescue byte inventory differs".into());
        }
        let mut input = Input::open(directory, reference).map_err(io)?;
        if &input.read_array::<8>().map_err(io)? != b"ERSC0001"
            || input.u64().map_err(io)? != total as u64
        {
            return Err("CP6 rescue bitset header differs".into());
        }
        let words = total.div_ceil(64);
        let mut quarantine = reserve(words).map_err(io)?;
        abandoned_bits = reserve(words).map_err(io)?;
        let mut qcount = 0u64;
        let mut acount = 0u64;
        for _ in 0..words {
            let word = input.u64().map_err(io)?;
            qcount += u64::from(word.count_ones());
            quarantine.push(word);
        }
        for (index, &qword) in quarantine.iter().enumerate() {
            let word = input.u64().map_err(io)?;
            if word & !qword != 0
                || index + 1 == words && total % 64 != 0 && qword >> (total % 64) != 0
            {
                return Err("CP6 rescue bitset subset or padding differs".into());
            }
            acount += u64::from(word.count_ones());
            abandoned_bits.push(word);
        }
        input.finish().map_err(io)?;
        if qcount != number(&scalar, "quarantined")?
            || acount != number(&scalar, "abandoned_obligations")?
        {
            return Err("CP6 rescue bitset count differs".into());
        }
    } else if number(&scalar, "quarantined")? != 0 || number(&scalar, "abandoned_obligations")? != 0
    {
        return Err("CP6 rescue counts without chain".into());
    }
    for (id, &word) in ledger.iter().enumerate() {
        let tag = word >> 61;
        let abandoned = abandoned_bits
            .get(id / 64)
            .is_some_and(|word| word >> (id % 64) & 1 != 0);
        if tag == 7 && !abandoned || abandoned && !matches!(tag, 0 | 1 | 6 | 7) {
            return Err("CP6 abandoned authority differs from ledger".into());
        }
    }
    let (mut edge_input, n) = section(5)?;
    if n != count(&scalar, "edge_runs")?
        || n as u64 > edge_input.reference.bytes.saturating_sub(28) / 8
    {
        return Err("edge run inventory differs".into());
    }
    let mut runs = reserve(n).map_err(io)?;
    let mut edges = Vec::new();
    for _ in 0..n {
        let source = edge_input.u32().map_err(io)?;
        let length = edge_input.u32().map_err(io)? as usize;
        if length > total
            || length as u64 > edge_input.reference.bytes.saturating_sub(edge_input.read) / 4
        {
            return Err("edge target count exceeds section bounds".into());
        }
        let mut targets = reserve(length).map_err(io)?;
        edges
            .try_reserve(length)
            .map_err(|_| "edge pair allocation")?;
        for _ in 0..length {
            let target = edge_input.u32().map_err(io)?;
            targets.push(target);
            edges.push((source, target));
        }
        runs.push((source, targets));
    }
    edge_input.finish().map_err(io)?;
    if edges.len() != count(&scalar, "edges")? {
        return Err("edge count differs".into());
    }
    let (mut anchor_input, n) = section(6)?;
    if anchor_input.u16().map_err(io)? != 2
        || anchor_input.u64().map_err(io)? != n as u64
        || n > total
    {
        return Err("anchor inventory differs".into());
    }
    let mut anchors: Vec<AnchorRow> = reserve(n).map_err(io)?;
    for _ in 0..n {
        let node = anchor_input.u32().map_err(io)?;
        let kind = anchor_input.u8().map_err(io)?;
        if anchor_input.read_array::<3>().map_err(io)? != [0; 3] {
            return Err("unsupported anchor shape".into());
        }
        let count = anchor_input.u32().map_err(io)? as usize;
        let scope_len = anchor_input.u32().map_err(io)? as usize;
        let dispatch = anchor_input.u64().map_err(io)?;
        if count == 0
            || count > total
            || count > 1 << 18
            || kind > 2
            || scope_len > 4 + (1 << 18) * (18 + 4 * N)
            || (count as u64)
                .checked_mul(16)
                .and_then(|n| n.checked_add(scope_len as u64))
                .is_none_or(|n| {
                    n > anchor_input
                        .reference
                        .bytes
                        .saturating_sub(anchor_input.read)
                })
            || kind == 0 && (count != 1 || scope_len != 8)
            || kind != 0 && scalar["g2"] != "union"
        {
            return Err("anchor counts, mode or scope exceed bounds".into());
        }
        let mut list = reserve(count).map_err(io)?;
        for _ in 0..count {
            let target = anchor_input.u32().map_err(io)?;
            let lent = anchor_input.u8().map_err(io)?;
            if anchor_input.read_array::<3>().map_err(io)? != [0; 3] {
                return Err("anchor padding".into());
            }
            list.push((target, lent, anchor_input.u64().map_err(io)?));
        }
        let mut scope = reserve(scope_len).map_err(io)?;
        for _ in 0..scope_len {
            scope.push(anchor_input.u8().map_err(io)?);
        }
        anchors.push((node, kind, dispatch, list, scope));
    }
    anchor_input.finish().map_err(io)?;
    // Resume-only auxiliary state is authenticated and framing checked, but is
    // not used as authority for graph coverage or independent native inspection.
    for (number, width) in [(3, Some(8)), (7, None), (9, Some(8))] {
        let (input, n) = section(number)?;
        if let Some(width) = width {
            input.fixed(n, width).map_err(io)?;
        }
        if number == 3 && n != total.div_ceil(64) || number == 9 && n > total {
            return Err("resume auxiliary section inventory differs".into());
        }
        input.drain().map_err(io)?;
    }
    let reference = file("orthants")?;
    let mut orthants = Input::open(directory, reference).map_err(io)?;
    if &orthants.read_array::<8>().map_err(io)? != b"EPORTH01"
        || orthants.u64().map_err(io)? != reference.count
        || reference.count > total as u64
        || reference
            .count
            .checked_mul(4)
            .and_then(|n| n.checked_add(16))
            != Some(reference.bytes)
    {
        return Err("orthant section framing or count differs".into());
    }
    for _ in 0..reference.count {
        let id = orthants.u32().map_err(io)?;
        if id != u32::MAX && id as usize >= total {
            return Err("orthant ID beyond watermark".into());
        }
    }
    orthants.finish().map_err(io)?;
    let segments = json_file(directory, file("record-segments")?, None).map_err(io)?;
    let segments = segments.as_array().ok_or("record segment list")?;
    if segments.len() as u64 != file("record-segments")?.count {
        return Err("record segment count".into());
    }
    let mut records = Vec::new();
    let mut record_refs = Vec::new();
    let mut seen = BTreeSet::new();
    let mut first = 0u64;
    for segment in segments {
        let name = segment["file"].as_str().ok_or("record segment file")?;
        let generation = name
            .strip_prefix("records-")
            .and_then(|s| s.strip_suffix(".bin"))
            .filter(|s| s.len() == 20 && s.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or("noncanonical record segment name")?;
        if generation == 0
            || generation > manifest.generation
            || number(segment, "generation")? != generation
            || number(segment, "first")? != first
            || !seen.insert(generation)
        {
            return Err("record segment generation differs".into());
        }
        let digest = segment["blake3"].as_str().ok_or("record segment digest")?;
        let digest = blake3::Hash::from_hex(digest).map_err(|e| e.to_string())?;
        let reference = FileRef {
            key: "records".into(),
            file: name.into(),
            bytes: number(segment, "bytes")?,
            count: number(segment, "count")?,
            blake3: *digest.as_bytes(),
        };
        first = first
            .checked_add(reference.count)
            .ok_or("record count overflow")?;
        records.push((directory.join(name), count(segment, "count")?));
        record_refs.push(RecordRef(reference));
    }
    let counts = scalar["ledger_counts"]
        .as_array()
        .filter(|a| a.len() == 8)
        .ok_or("ledger counts")?;
    let mut normalized = json!({"format":FORMAT,"schema":3,"generation":manifest.generation,
        "initial_admission":admission,"total_queries":query_total,"processed_queries":processed,
        "epoch_rolling":rolling,"epoch_cut_size":scalar["epoch_cut_size"],
        "epoch_publication_order":scalar.get("epoch_publication_order").cloned().unwrap_or(json!("oldest-prefix")),
        "lockstep_b":scalar["lockstep_b"],
        "epoch_base_window":scalar["epoch_base_window"],
        "epoch_result_escrow_jobs":scalar["epoch_result_escrow_jobs"],
        "epoch_result_escrow_bytes":scalar["epoch_result_escrow_bytes"],
        "k":scalar["k"],"p0":p0,"edge_digest":scalar["edge_digest"],"records_digest":scalar["records_digest"],
        "ledger6_counts":{}});
    for (name, value) in [
        "pending",
        "reserved",
        "native",
        "native_frontier",
        "native_error",
        "alias",
        "exhausted",
        "abandoned",
    ]
    .iter()
    .zip(counts)
    {
        if !value.is_u64() {
            return Err("ledger count type".into());
        }
        normalized["ledger6_counts"][*name] = value.clone();
    }
    let merged = counts[2..=5]
        .iter()
        .chain(std::iter::once(&counts[7]))
        .try_fold(0u64, |sum, v| sum.checked_add(v.as_u64()?));
    if merged != Some(first) {
        return Err("record/ledger inventory differs".into());
    }
    if admission == "in_progress"
        && (scalar["k"] != 0
            || p0 != total
            || counts[0].as_u64() != Some(total as u64)
            || !records.is_empty()
            || !edges.is_empty()
            || scalar["walk"]["dispatched"] != 0)
    {
        return Err("incomplete admission has issued work".into());
    }
    let walk = &scalar["walk"];
    let raw = RawCheckpoint {
        generation: manifest.generation,
        request: scalar["request"]
            .as_str()
            .ok_or("request digest")?
            .to_owned(),
        publication_policy: "epoch".into(),
        walk_semantics_version: 4,
        // CP6 binds request/owner identities, not executable bytes. Launchers pin
        // the executable independently; do not invent a native binary binding.
        executable: String::new(),
        owners,
        counters: [
            count(walk, "events")?,
            count(walk, "successors")?,
            count(walk, "conditional")?,
            count(walk, "known_reuse")?
                .checked_add(count(walk, "job_duplicates")?)
                .ok_or("reuse counter overflow")?,
            0,
            count(walk, "frontiers")?,
            count(walk, "completed")?,
            count(walk, "natives")?,
            count(walk, "routed")?,
            count(walk, "route_masks")?,
            p0,
            count(walk, "initial_inspected")?,
        ],
        closure: scalar["closure"].clone(),
        inputs,
        input_frontiers,
        uncommitted: Vec::new(),
        flags,
        edges,
        domains,
        records,
        verify_seconds: start.elapsed().as_secs_f64(),
        amendments,
        pending_frontiers: Vec::new(),
        g2_activation: None,
    };
    Ok((
        raw,
        EpochSections {
            ledger,
            runs,
            anchors,
            manifest: normalized,
        },
        record_refs,
    ))
}
