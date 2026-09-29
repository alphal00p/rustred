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
    lookup,
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
    FixedSection::open(
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
        || file(manifest, "inputs")?.count != saved.processed_queries as u64
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
    if N > 32 || manifest.arity != N {
        return Err(invalid("epoch private manifest arity differs"));
    }
    let meta = file(&manifest, "meta")?;
    if meta.count != 1 || meta.bytes > publication::MAX_META_BYTES {
        return Err(invalid("epoch scalar metadata count/byte limit"));
    }
    let mut reader = checked(directory, meta)?;
    let scalars: OwnedScalars = serde_json::from_reader(&mut reader).map_err(io::Error::other)?;
    reader.finish()?;
    identity.validate_saved(&scalars, lockstep_b)?;
    inventories(&manifest, &scalars)?;
    owners(directory, file(&manifest, "owners")?, identity)?;
    let store = fixed::<N>(directory, &manifest, Section::Domains)?.domains()?;
    let mut nodes = fixed::<N>(directory, &manifest, Section::Nodes)?.flags(false)?;
    let live = fixed::<N>(directory, &manifest, Section::Live)?.live(scalars.watermark)?;
    let ledger = fixed::<N>(directory, &manifest, Section::Ledger)?.ledger()?;
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
    let anchors = auxiliary::anchors::<N>(
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
    )?;
    let frontier_counts =
        fixed::<N>(directory, &manifest, Section::Frontiers)?.frontiers(scalars.watermark)?;
    let dispatch_file = file(&manifest, "state-7")?;
    let dispatch = dispatch_state::read::<N>(
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
        },
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
    })
}

#[cfg(test)]
mod tests;
