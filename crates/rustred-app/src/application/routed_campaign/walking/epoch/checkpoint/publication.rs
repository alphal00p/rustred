//! Atomic CP6 publication. Resumable denotes the supported validated restore
//! format, never independent cold verification of this freshly saved state.
use super::super::ledger6::Tag;
use super::super::record_store::Sidecar;
use super::metadata::Inputs;
use super::{Digest, MergeBoundary, Observed, Section, Stream, invalid};
use crate::application::atomic_file::write_file_atomically_with;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

pub(crate) const FORMAT: &str = "RUSTRED-WALK-CP6";
pub(crate) const SCHEMA: u32 = 3;
pub(super) const LATEST: &str = "latest.json";
pub(super) const PREVIOUS: &str = "previous.json";
const MAX_MANIFEST_BYTES: usize = 64 * 1024;
pub(super) const MAX_META_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FileRef {
    pub key: String,
    pub file: String,
    pub count: u64,
    pub bytes: u64,
    pub blake3: [u8; 32],
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Manifest {
    format: String,
    schema: u32,
    pub generation: u64,
    pub arity: usize,
    walk_semantics_version: u32,
    resumable: bool,
    pub files: Vec<FileRef>,
}

#[derive(Serialize)]
struct Envelope<'a> {
    manifest: &'a Manifest,
    blake3: [u8; 32],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadEnvelope {
    manifest: Manifest,
    blake3: [u8; 32],
}

pub(super) struct Receipt {
    pub generation: u64,
    pub manifest: PathBuf,
    /// Digest of the authenticated manifest object, not the envelope file.
    pub manifest_blake3: [u8; 32],
    /// Warnings after latest is already durable cannot turn success into
    /// failure. No cleanup is attempted by this first private publisher.
    pub warnings: Vec<String>,
}

pub(super) struct Store {
    directory: PathBuf,
    _lock: File,
    next: u64,
    latest: Option<Manifest>,
    failed: bool,
    session: u64,
    #[cfg(test)]
    fail: Option<FailPoint>,
}

#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum FailPoint {
    AfterSections,
    BeforeLatest,
    BeforePrevious,
}

impl Store {
    #[cfg(test)]
    pub(super) fn fail_at(&mut self, point: FailPoint) {
        self.fail = Some(point);
    }

    /// Sticky C5 authority, including failure outside P3. Never publish another
    /// generation after this call; a failed fsync does not claim durability.
    pub(super) fn poison(&mut self, merge: u64, reason: &str) -> io::Result<()> {
        self.failed = true;
        write_file_atomically_with(&self.directory.join("epoch-poison"), true, |output| {
            writeln!(output, "epoch engine-fatal at merge {merge}")
                .map_err(|error| error.to_string())?;
            let bytes = reason.as_bytes();
            output
                .write_all(&bytes[..bytes.len().min(4096)])
                .map_err(|error| error.to_string())
        })
        .map_err(io::Error::other)
    }

    /// Fresh-only internal store: no CP5, S2 export or old private snapshot
    /// is imported. The restore constructor is a later independently gated
    /// implementation; this one requires a genuinely empty directory.
    pub fn fresh(directory: PathBuf) -> io::Result<Self> {
        match fs::symlink_metadata(&directory) {
            Ok(metadata) => {
                if !metadata.is_dir()
                    || metadata.file_type().is_symlink()
                    || fs::read_dir(&directory)?.next().is_some()
                {
                    return Err(invalid(
                        "new epoch internal checkpoint directory must be empty",
                    ));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => fs::create_dir(&directory)?,
            Err(error) => return Err(error),
        }
        let parent = directory
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        File::open(parent)?.sync_all()?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(directory.join("checkpoint.lock"))?;
        lock.try_lock()
            .map_err(|error| io::Error::other(error.to_string()))?;
        let session = super::session::initial(&directory)?.number();
        Ok(Self {
            directory,
            _lock: lock,
            next: 1,
            latest: None,
            failed: false,
            session,
            #[cfg(test)]
            fail: None,
        })
    }

    /// Lock first, then validate all checkpoint data before `adopt` reserves
    /// another session. This does not enable a public resume/import entry.
    pub(super) fn open(directory: PathBuf) -> io::Result<Self> {
        let metadata = fs::symlink_metadata(&directory)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(invalid(
                "epoch checkpoint directory is not a real directory",
            ));
        }
        if directory.join("epoch-internal-latest.json").exists()
            || directory.join("epoch-internal.lock").exists()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "old private epoch snapshots are not public CP6; fresh state required",
            ));
        }
        let lock_path = directory.join("checkpoint.lock");
        let metadata = fs::symlink_metadata(&lock_path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(invalid("epoch checkpoint lock is not a regular file"));
        }
        let lock = OpenOptions::new().read(true).write(true).open(lock_path)?;
        lock.try_lock()
            .map_err(|error| io::Error::other(error.to_string()))?;
        let session = super::session::read(&directory)?;
        // One streaming directory pass, including orphan state/record tails:
        // never collide with a generation used before a crash. No deletion.
        let mut greatest = 0;
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if name.starts_with("poison-") || name == "epoch-poison" {
                return Err(invalid("epoch checkpoint is permanently poisoned"));
            }
            let digits = name
                .strip_prefix("epoch-")
                .filter(|rest| rest.as_bytes().get(20) == Some(&b'-'))
                .or_else(|| {
                    name.strip_prefix("records-")
                        .filter(|rest| rest.get(20..) == Some(".bin"))
                })
                .and_then(|rest| rest.get(..20));
            if let Some(digits) = digits.filter(|text| text.bytes().all(|b| b.is_ascii_digit())) {
                let generation = digits
                    .parse::<u64>()
                    .map_err(|_| invalid("epoch orphan generation range"))?;
                greatest = greatest.max(generation);
            }
        }
        let next = greatest
            .checked_add(1)
            .ok_or_else(|| invalid("epoch generation exhausted"))?;
        Ok(Self {
            directory,
            _lock: lock,
            next,
            latest: None,
            failed: false,
            session,
            #[cfg(test)]
            fail: None,
        })
    }

    pub(super) fn directory(&self) -> &Path {
        &self.directory
    }
    pub(super) fn next_generation(&self) -> u64 {
        self.next
    }
    pub(super) fn current_generation(&self) -> Option<u64> {
        self.latest.as_ref().map(|manifest| manifest.generation)
    }

    /// Called only after full state/root validation. A successful reservation
    /// is durable even if the subsequent restored-state assembly is interrupted.
    pub(super) fn adopt(
        &mut self,
        manifest: Manifest,
        saved_session: u64,
    ) -> io::Result<super::Session> {
        if self.failed || self.latest.is_some() || manifest.generation >= self.next {
            return Err(invalid("epoch publisher cannot adopt this generation"));
        }
        let reservation = super::session::reserve(&self.directory, saved_session);
        match reservation {
            Ok(session) => {
                self.session = session.number();
                self.latest = Some(manifest);
                Ok(session)
            }
            Err(error) => {
                self.failed = true;
                Err(error)
            }
        }
    }

    /// One state-bound orchestration writes every section itself. It never
    /// accepts arbitrary section receipts from another state or generation.
    /// Err is sticky, but is not a rollback guarantee: the shared atomic
    /// helper can install latest and then fail directory fsync. In that case
    /// durability is uncertain and latest may already name the new generation.
    pub fn save<const N: usize>(
        &mut self,
        boundary: &MergeBoundary<'_, N>,
        inputs: &Inputs<'_>,
        records: &mut Sidecar,
    ) -> io::Result<Receipt> {
        self.save_observed(boundary, inputs, records, &mut || {})
    }

    pub(super) fn save_observed<const N: usize>(
        &mut self,
        boundary: &MergeBoundary<'_, N>,
        inputs: &Inputs<'_>,
        records: &mut Sidecar,
        progress: &mut dyn FnMut(),
    ) -> io::Result<Receipt> {
        if self.failed {
            return Err(io::Error::other("epoch internal store already failed"));
        }
        inputs.validate(boundary)?;
        if boundary.dispatch.session != self.session {
            return Err(invalid("epoch save does not own the reserved session"));
        }
        if records.directory() != self.directory || records.generation() != self.next {
            return Err(invalid(
                "epoch record tail belongs to another directory or generation",
            ));
        }
        let counts = boundary.state.ledger.counts();
        let record_count = counts.get(Tag::Native)
            + counts.get(Tag::NativeFrontier)
            + counts.get(Tag::NativeError)
            + counts.get(Tag::Alias)
            + counts.get(Tag::Abandoned);
        if record_count != records.total() as u64 {
            return Err(invalid("epoch record/ledger inventory differs"));
        }
        let result = self.save_inner(boundary, inputs, records, progress);
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn save_inner<const N: usize>(
        &mut self,
        boundary: &MergeBoundary<'_, N>,
        inputs: &Inputs<'_>,
        records: &mut Sidecar,
        progress: &mut dyn FnMut(),
    ) -> io::Result<Receipt> {
        let generation = self.next;
        self.next = self
            .next
            .checked_add(1)
            .ok_or_else(|| invalid("epoch generation overflow"))?;
        // Sealed records are durable before any authority names them. The
        // sidecar advances its tail generation even if later section I/O fails.
        records
            .seal(generation, self.next)
            .map_err(io::Error::other)?;
        progress();
        let mut files = Vec::new();
        files
            .try_reserve_exact(16)
            .map_err(|_| io::Error::other("epoch manifest allocation"))?;
        for section in Section::ALL {
            let receipt = boundary.write_new_section_observed(
                &self.directory,
                generation,
                section,
                progress,
            )?;
            files.push(FileRef {
                key: format!("state-{}", section as u32),
                file: section.filename(generation),
                count: boundary.section_count(section),
                bytes: receipt.digest.bytes,
                blake3: receipt.digest.blake3,
            });
        }
        files.push(self.aux(generation, "meta", 1, progress, |file| {
            let limited = Limit::new(file, MAX_META_BYTES);
            let (limited, digest) = inputs.write_scalars(boundary, limited)?;
            Ok((limited.output, digest))
        })?);
        files.push(self.aux(
            generation,
            "owners",
            inputs.owner_count() as u64,
            progress,
            |file| inputs.write_owners(file),
        )?);
        files.push(self.aux(
            generation,
            "inputs",
            inputs.rows.len() as u64,
            progress,
            |file| inputs.write_rows(file, false),
        )?);
        files.push(self.aux(
            generation,
            "input-frontiers",
            inputs.frontiers.len() as u64,
            progress,
            |file| inputs.write_rows(file, true),
        )?);
        files.push(self.aux(
            generation,
            "record-segments",
            records.closed().len() as u64,
            progress,
            |file| json_stream(file, records.closed()),
        )?);
        files.push(self.aux(
            generation,
            "orthants",
            boundary.state.store.buckets.len() as u64,
            progress,
            |file| write_orthants(boundary, file),
        )?);
        if let Some(rescue) = &boundary.state.rescue {
            files.push(self.aux(
                generation,
                "rescue",
                u64::from(boundary.state.watermark()),
                progress,
                |file| {
                    super::super::rescue::write(
                        boundary.state.watermark(),
                        &boundary.state.store.quarantine,
                        &rescue.abandoned,
                        file,
                    )
                },
            )?);
        }
        #[cfg(test)]
        self.at(FailPoint::AfterSections)?;
        File::open(&self.directory)?.sync_all()?;
        let manifest = Manifest {
            format: FORMAT.into(),
            schema: SCHEMA,
            generation,
            arity: N,
            walk_semantics_version: super::super::EPOCH_WALK_SEMANTICS_VERSION,
            resumable: true,
            files,
        };
        #[cfg(test)]
        self.at(FailPoint::BeforeLatest)?;
        let manifest_blake3 = manifest_digest(&manifest)?;
        write_manifest(&self.directory.join(LATEST), &manifest)?;
        // From this point latest is durable. Never return Err because the
        // previous pointer or best-effort housekeeping cannot be advanced.
        let previous = self.latest.replace(manifest);
        let mut warnings = Vec::new();
        if let Some(previous) = previous {
            let advance = (|| {
                #[cfg(test)]
                self.at(FailPoint::BeforePrevious)?;
                write_manifest(&self.directory.join(PREVIOUS), &previous)
            })();
            if let Err(error) = advance {
                warnings.push(format!(
                    "latest is durable; previous pointer not advanced: {error}"
                ));
            }
        }
        Ok(Receipt {
            generation,
            manifest: self.directory.join(LATEST),
            manifest_blake3,
            warnings,
        })
    }

    fn aux<'a>(
        &self,
        generation: u64,
        key: &str,
        count: u64,
        progress: &'a mut dyn FnMut(),
        write: impl FnOnce(Observed<'a, File>) -> io::Result<(Observed<'a, File>, Digest)>,
    ) -> io::Result<FileRef> {
        let name = format!("epoch-{generation:020}-{key}.part");
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.directory.join(&name))?;
        let (file, digest) = write(Observed {
            output: file,
            progress,
        })?;
        file.output.sync_all()?;
        Ok(FileRef {
            key: key.into(),
            file: name,
            count,
            bytes: digest.bytes,
            blake3: digest.blake3,
        })
    }

    #[cfg(test)]
    fn at(&self, point: FailPoint) -> io::Result<()> {
        if self.fail == Some(point) {
            Err(io::Error::other("injected epoch publication failure"))
        } else {
            Ok(())
        }
    }
}

fn json_stream<W: Write, T: Serialize + ?Sized>(output: W, value: &T) -> io::Result<(W, Digest)> {
    let mut stream = Stream::new(output);
    serde_json::to_writer(&mut stream, value).map_err(io::Error::other)?;
    stream.finish()
}

/// Historical dominant orthants may be retired from the live lookup set.
/// Store bucket ordinals are rebuilt from all images in insertion order;
/// record the actual slot independently of live bits for restore validation.
pub(super) fn write_orthants<const N: usize, W: Write>(
    boundary: &MergeBoundary<'_, N>,
    output: W,
) -> io::Result<(W, Digest)> {
    let mut stream = Stream::new(output);
    stream.write_all(b"EPORTH01")?;
    stream.write_all(&(boundary.state.store.buckets.len() as u64).to_le_bytes())?;
    for bucket in &boundary.state.store.buckets {
        stream.write_all(&bucket.orthant.unwrap_or(u32::MAX).to_le_bytes())?;
    }
    stream.finish()
}

struct Limit<W> {
    output: W,
    remaining: u64,
}
impl<W> Limit<W> {
    fn new(output: W, limit: u64) -> Self {
        Self {
            output,
            remaining: limit,
        }
    }
}
impl<W: Write> Write for Limit<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() as u64 > self.remaining {
            return Err(invalid("epoch JSON byte limit"));
        }
        let written = self.output.write(bytes)?;
        self.remaining -= written as u64;
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.output.flush()
    }
}

fn manifest_digest(manifest: &Manifest) -> io::Result<[u8; 32]> {
    let (_, digest) = json_stream(Limit::new(io::sink(), MAX_MANIFEST_BYTES as u64), manifest)?;
    Ok(digest.blake3)
}

fn write_manifest(path: &Path, manifest: &Manifest) -> io::Result<()> {
    let envelope = Envelope {
        manifest,
        blake3: manifest_digest(manifest)?,
    };
    write_file_atomically_with(path, true, |file| {
        json_stream(Limit::new(file, MAX_MANIFEST_BYTES as u64), &envelope)
            .map(|_| ())
            .map_err(|error| error.to_string())
    })
    .map_err(io::Error::other)
}

/// Bounded private-manifest reader for corruption/failure tests and later
/// restore plumbing. It verifies the envelope, NOT the referenced state.
pub(super) fn read_manifest(path: &Path) -> io::Result<Manifest> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(invalid("epoch manifest is not a regular file"));
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(MAX_MANIFEST_BYTES + 1)
        .map_err(|_| io::Error::other("epoch manifest read allocation"))?;
    File::open(path)?
        .take(MAX_MANIFEST_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(invalid("epoch manifest byte limit"));
    }
    // Discriminate an explicitly foreign identity before strict CP6 decoding.
    // Older private envelopes contain removed fields; CP5/S2 are flat objects.
    // Their shape errors must not become corruption errors that permit runtime
    // fallback to an unrelated valid previous CP6 generation. This bounded
    // probe grants no authority: strict shape, digest and inventory checks follow.
    let shape: serde_json::Value = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
    let identity = shape.get("manifest").unwrap_or(&shape);
    if identity
        .get("format")
        .is_some_and(|value| value.as_str() != Some(FORMAT))
        || identity
            .get("schema")
            .is_some_and(|value| value.as_u64() != Some(u64::from(SCHEMA)))
        || identity.get("walk_semantics_version").is_some_and(|value| {
            value.as_u64() != Some(u64::from(super::super::EPOCH_WALK_SEMANTICS_VERSION))
        })
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unsupported public CP6 identity; no private/CP5/S2 import",
        ));
    }
    drop(shape);
    let envelope: ReadEnvelope = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
    let manifest = envelope.manifest;
    if manifest.format != FORMAT
        || manifest.schema != SCHEMA
        || manifest.walk_semantics_version != super::super::EPOCH_WALK_SEMANTICS_VERSION
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unsupported public CP6 identity; no private/CP5/S2 import",
        ));
    }
    if manifest.generation == 0
        || !manifest.resumable
        || manifest.files.len()
            != 15 + usize::from(manifest.files.iter().any(|file| file.key == "rescue"))
        || manifest_digest(&manifest)? != envelope.blake3
    {
        return Err(invalid("epoch CP6 manifest identity or digest"));
    }
    // Exact fixed inventory also rejects duplicates and path traversal;
    // section content/shape verification remains the restore layer's job.
    for section in Section::ALL {
        let key = format!("state-{}", section as u32);
        if !manifest
            .files
            .iter()
            .any(|file| file.key == key && file.file == section.filename(manifest.generation))
        {
            return Err(invalid("epoch private manifest state path inventory"));
        }
    }
    for key in [
        "meta",
        "owners",
        "inputs",
        "input-frontiers",
        "record-segments",
        "orthants",
    ] {
        let name = format!("epoch-{:020}-{key}.part", manifest.generation);
        if !manifest
            .files
            .iter()
            .any(|file| file.key == key && file.file == name)
        {
            return Err(invalid("epoch private manifest metadata path inventory"));
        }
    }
    if manifest.files.iter().any(|file| {
        file.key == "rescue"
            && file.file != format!("epoch-{:020}-rescue.part", manifest.generation)
    }) {
        return Err(invalid("epoch rescue inventory path"));
    }
    Ok(manifest)
}
