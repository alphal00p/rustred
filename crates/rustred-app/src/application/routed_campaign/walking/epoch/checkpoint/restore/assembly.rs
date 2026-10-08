//! Bind the private manifest, scalars and owner inventory before assembling
//! provisional runtime arrays. This is intentionally NOT a resume entry:
//! variable sections, records/roots and complete cross-state checks still
//! precede any construction of EpochState or dispatch/session reservation.
use super::super::super::anchors::AnchorMap;
use super::super::metadata::{Identity, OwnedScalars};
use super::super::publication::{self, FileRef, Manifest};
use super::super::{Digest, Section, SectionReceipt, invalid};
use super::{
    CheckedRead, EdgeStore, FixedSection, Ledger6, Store, auxiliary, cross_state, dispatch_state,
    lookup, record_body, roots,
};
use std::io::{self, Read};
use std::path::Path;

pub(super) struct Provisional<const N: usize> {
    pub manifest: Manifest,
    pub scalars: OwnedScalars,
    pub store: Store<N>,
    pub ledger: Ledger6,
    pub nodes: Vec<u8>,
    pub live: Vec<u64>,
    pub edges: EdgeStore,
    pub closure_flags: Vec<u8>,
    pub anchors: AnchorMap,
    pub frontier_counts: std::collections::BTreeMap<u32, u32>,
    pub dispatch: dispatch_state::SavedDispatch,
    pub rescue: Option<super::super::super::rescue::State>,
    pub record_segments:
        Vec<crate::application::routed_campaign::walking::checkpoint::manifest::Segment>,
}

impl<const N: usize> Provisional<N> {
    /// Owner preparation must have authenticated the same identity first.
    /// This is still not runnable state: final closure,
    /// session reservation and unfinished-batch replay remain separate gates.
    pub fn read_roots(
        &self,
        directory: &Path,
        identity: &Identity<'_>,
        reducer: &rustred::solver::RoutedCandidateReducer<N>,
    ) -> io::Result<roots::Roots> {
        identity.validate_saved(&self.scalars, self.scalars.lockstep_b)?;
        roots::read(
            directory,
            file(&self.manifest, "inputs")?,
            file(&self.manifest, "input-frontiers")?,
            identity,
            reducer,
            &self.store,
            self.scalars.p0,
        )
    }
}

fn file<'a>(manifest: &'a Manifest, key: &str) -> io::Result<&'a FileRef> {
    manifest
        .files
        .iter()
        .find(|file| file.key == key)
        .ok_or_else(|| invalid("epoch private manifest is missing a required section"))
}

fn checked(directory: &Path, file: &FileRef) -> io::Result<CheckedRead> {
    CheckedRead::open(directory, &file.file, file.bytes, file.blake3)
}

fn fixed<const N: usize>(
    directory: &Path,
    manifest: &Manifest,
    section: Section,
) -> io::Result<FixedSection<N>> {
    let file = file(manifest, &format!("state-{}", section as u32))?;
    FixedSection::open_with_arity(
        directory,
        &SectionReceipt {
            generation: manifest.generation,
            section,
            digest: Digest {
                bytes: file.bytes,
                blake3: file.blake3,
            },
        },
        file.count,
        manifest.arity,
    )
}

fn owners(directory: &Path, file: &FileRef, identity: &Identity<'_>) -> io::Result<()> {
    if file.count != identity.owners().len() as u64
        || file.count.checked_mul(67) != Some(file.bytes)
    {
        return Err(invalid("epoch owner section count/length differs"));
    }
    let mut reader = checked(directory, file)?;
    let mut line = [0u8; 67]; // canonical JSON string: quote, 64 hex digits, quote, newline
    for owner in identity.owners() {
        reader.read_exact(&mut line)?;
        if line[0] != b'"' || &line[1..65] != owner.as_bytes() || line[65..] != [b'"', b'\n'] {
            return Err(invalid("epoch owner fingerprint inventory differs"));
        }
    }
    reader.finish()
}

/// Preflight cross-section scalar counts before any domain-sized allocation.
/// File metadata/digests are subsequently checked by each streamed decoder.
fn inventories(manifest: &Manifest, saved: &OwnedScalars) -> io::Result<()> {
    let watermark = u64::from(saved.watermark);
    let closure = if saved.closure.unavailable.is_some() {
        0
    } else {
        watermark
    };
    for (section, count, width) in [
        (Section::Domains, watermark, 37 + 4 * manifest.arity as u64),
        (Section::Nodes, watermark, 1),
        (Section::Live, watermark.div_ceil(64), 8),
        (Section::Ledger, watermark, 8),
        (Section::ClosureFlags, closure, 1),
    ] {
        let file = file(manifest, &format!("state-{}", section as u32))?;
        if file.count != count
            || count
                .checked_mul(width)
                .and_then(|body| body.checked_add(28))
                != Some(file.bytes)
        {
            return Err(invalid(
                "epoch manifest/scalar fixed-section inventory differs",
            ));
        }
    }
    let edges = file(manifest, "state-5")?;
    if edges.count != saved.edge_runs
        || saved
            .edge_runs
            .checked_mul(2)
            .and_then(|words| words.checked_add(saved.edges))
            .and_then(|words| words.checked_mul(4))
            .and_then(|body| body.checked_add(28))
            != Some(edges.bytes)
        || file(manifest, "state-6")?.count > watermark
        || file(manifest, "state-7")?.count
            != saved.ledger_counts[super::super::super::ledger6::Tag::Reserved as usize]
        || file(manifest, "state-9")?.count > watermark
        || file(manifest, "orthants")?.count > watermark
        || file(manifest, "inputs")?.count
            != saved.processed_queries as u64
                + saved.amendments.iter().map(|a| a.queries).sum::<u64>()
        || file(manifest, "input-frontiers")?.count != saved.input_frontiers as u64
    {
        return Err(invalid(
            "epoch manifest/scalar variable-section inventory differs",
        ));
    }
    Ok(())
}

pub(super) fn read<const N: usize>(
    directory: &Path,
    identity: &Identity<'_>,
    lockstep_b: usize,
) -> io::Result<Provisional<N>> {
    let manifest = publication::read_manifest(&directory.join(publication::LATEST))?;
    read_manifest(directory, identity, lockstep_b, manifest)
}

pub(super) fn read_manifest<const N: usize>(
    directory: &Path,
    identity: &Identity<'_>,
    lockstep_b: usize,
    manifest: Manifest,
) -> io::Result<Provisional<N>> {
    if N > 32 || !crate::application::routed_campaign::storage::compatible_width(manifest.arity, N)
    {
        return Err(invalid("epoch private manifest arity differs"));
    }
    let meta = file(&manifest, "meta")?;
    if meta.count != 1 || meta.bytes > publication::MAX_META_BYTES {
        return Err(invalid("epoch scalar metadata count/byte limit"));
    }
    let mut reader = checked(directory, meta)?;
    let scalars: OwnedScalars = serde_json::from_reader(&mut reader).map_err(io::Error::other)?;
    reader.finish()?;
    // Worker width is operational. A rolling resume retains the authenticated
    // saved window; policy/cut size and the full request binding are still
    // checked independently before any campaign-sized allocation.
    let lockstep_b = if identity.epoch_rolling() {
        scalars.lockstep_b
    } else {
        lockstep_b
    };
    identity.validate_saved(&scalars, lockstep_b)?;
    if scalars
        .amendments
        .iter()
        .any(|a| a.resumed_generation >= manifest.generation)
    {
        return Err(invalid(
            "epoch amendment resumed generation is not historical",
        ));
    }
    inventories(&manifest, &scalars)?;
    owners(directory, file(&manifest, "owners")?, identity)?;
    let rescue_active = !scalars.amendments.is_empty();
    let mut store =
        fixed::<N>(directory, &manifest, Section::Domains)?.domains_with_rescue(rescue_active)?;
    let rescue = if rescue_active {
        let entry = file(&manifest, "rescue")?;
        if entry.count != u64::from(scalars.watermark)
            || entry.bytes != 16 + u64::from(scalars.watermark).div_ceil(64) * 16
        {
            return Err(invalid("epoch rescue byte inventory"));
        }
        let mut reader = checked(directory, entry)?;
        let (quarantine, abandoned) =
            super::super::super::rescue::read(&mut reader, scalars.watermark)?;
        reader.finish()?;
        if super::super::super::rescue::count(&quarantine) != scalars.quarantined
            || super::super::super::rescue::count(&abandoned) != scalars.abandoned_obligations
        {
            return Err(invalid("epoch rescue scalar inventory"));
        }
        store.install_quarantine(quarantine).map_err(invalid)?;
        Some(super::super::super::rescue::State {
            amendments: scalars.amendments.clone(),
            abandoned,
        })
    } else {
        if manifest.files.iter().any(|file| file.key == "rescue") {
            return Err(invalid("unexpected epoch rescue state"));
        }
        None
    };
    let mut nodes = fixed::<N>(directory, &manifest, Section::Nodes)?.flags(false)?;
    let live = fixed::<N>(directory, &manifest, Section::Live)?.live(scalars.watermark)?;
    let ledger = fixed::<N>(directory, &manifest, Section::Ledger)?.ledger()?;
    for id in 0..scalars.watermark {
        let marked = rescue
            .as_ref()
            .is_some_and(|r| super::super::super::rescue::contains(&r.abandoned, id));
        let tag = ledger.tag(id);
        use super::super::super::ledger6::Tag;
        if (tag == Some(Tag::Abandoned) && !marked)
            || marked
                && !matches!(
                    tag,
                    Some(Tag::Pending | Tag::Reserved | Tag::Exhausted | Tag::Abandoned)
                )
        {
            return Err(invalid("epoch abandoned bitset/ledger differs"));
        }
    }
    if ledger.counts().0 != scalars.ledger_counts {
        return Err(invalid("epoch scalar ledger tag counts differ"));
    }
    let mut edges = fixed::<N>(directory, &manifest, Section::Edges)?.edges(scalars.watermark)?;
    if edges.runs() != scalars.edge_runs
        || edges.edges() != scalars.edges
        || edges.self_edges() != scalars.self_edges
        || edges.edge_digest() != scalars.edge_digest
    {
        return Err(invalid("epoch scalar edge inventory/digest differs"));
    }
    let closure_flags = fixed::<N>(directory, &manifest, Section::ClosureFlags)?.flags(true)?;
    let anchor_file = file(&manifest, "state-6")?;
    let anchors = auxiliary::anchors_with_arity::<N>(
        directory,
        &SectionReceipt {
            generation: manifest.generation,
            section: Section::Anchors,
            digest: Digest {
                bytes: anchor_file.bytes,
                blake3: anchor_file.blake3,
            },
        },
        anchor_file.count,
        scalars.watermark,
        scalars.p0,
        scalars.k,
        manifest.arity,
    )?;
    let frontier_counts =
        fixed::<N>(directory, &manifest, Section::Frontiers)?.frontiers(scalars.watermark)?;
    let dispatch_file = file(&manifest, "state-7")?;
    let dispatch = dispatch_state::read_with_arity::<N>(
        directory,
        &SectionReceipt {
            generation: manifest.generation,
            section: Section::Dispatch,
            digest: Digest {
                bytes: dispatch_file.bytes,
                blake3: dispatch_file.blake3,
            },
        },
        dispatch_file.count,
        dispatch_state::Binding {
            ledger: &ledger,
            nodes: &nodes,
            k: scalars.k,
            p0: scalars.p0,
            lockstep_b: scalars.lockstep_b,
            adaptive: scalars.adaptive_dispatch.as_ref(),
        },
        manifest.arity,
    )?;
    let orthants = file(&manifest, "orthants")?;
    let store = lookup::rebuild(
        directory,
        manifest.generation,
        &Digest {
            bytes: orthants.bytes,
            blake3: orthants.blake3,
        },
        orthants.count,
        store,
        &live,
    )?;
    cross_state::validate(cross_state::View {
        store: &store,
        ledger: &ledger,
        nodes: &mut nodes,
        live: &live,
        edges: &mut edges,
        anchors: &anchors,
        frontier_counts: &frontier_counts,
        closure_flags: scalars
            .closure
            .unavailable
            .is_none()
            .then_some(closure_flags.as_slice()),
        walk: &scalars.walk,
        k: scalars.k,
        p0: scalars.p0,
        input_frontiers: scalars.input_frontiers,
        records_digest: &scalars.records_digest,
    })?;
    let record_segments = record_body::read(
        directory,
        file(&manifest, "record-segments")?,
        manifest.generation,
        record_body::View {
            store: &store,
            ledger: &ledger,
            edges: &edges,
            anchors: &anchors,
            frontier_counts: &frontier_counts,
            counters: &scalars.walk,
            k: scalars.k,
            input_frontiers: scalars.input_frontiers,
        },
    )?;
    Ok(Provisional {
        manifest,
        scalars,
        store,
        ledger,
        nodes,
        live,
        edges,
        closure_flags,
        anchors,
        frontier_counts,
        dispatch,
        rescue,
        record_segments,
    })
}

#[cfg(test)]
mod tests;
