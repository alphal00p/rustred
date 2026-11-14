//! vk5l WP6: the census of an earlier package as the resume census of a new
//! assembly (`RUSTRED_P_ASSEMBLE_CENSUS_SEED=<package dir>`).
//!
//! A scope 1-5 assembly spends most of its time in the census routes stage
//! (FULL2, 37 tops: 11.2 M sectors, about 25 h on 16 threads). A package with
//! more tops shares almost every presentation with an earlier one, so its
//! census can start from the earlier census: [`census_seed`] translates the
//! earlier `census.rrbin` onto the new presentation ids (a presentation is
//! matched by its family fingerprint, kind, DAG graph key and slots; an
//! `Induced` kind by its mask and its top's fingerprint) and
//! `census::p_census_journaled` reuses a sector only when its class has the
//! same key and the same owner sector in both. Everything else is routed
//! afresh, and a seeded census is always checked completely afterwards
//! (`RUSTRED_P_ASSEMBLE_CENSUS_CHECK` cannot skip it): a translation mistake
//! is a refused assembly, never a wrong package.
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use rustred::propagator::PresentationId;
use rustred::propagator::census::{PCensus, PClassRecord};
use rustred::propagator::error::PError;
use rustred::propagator::presentation::{Presentation, PresentationKind};

use super::format::{
    CENSUS_FILE, KindRecord, LIBRARY_FILE, LibraryManifest, PresentationRecord, SlotRecord,
    decode_census, decode_family_file,
};

fn malformed(detail: String) -> PError {
    PError::MalformedLibrary(format!("census seed: {detail}"))
}

/// The matching key of a presentation: family fingerprint, kind (an Induced
/// kind by mask and top fingerprint), graph key and slots.
fn match_key(
    fingerprint: &str,
    kind: &KindRecord,
    top_fingerprint: Option<&str>,
    node: Option<&str>,
    slots: &[SlotRecord],
) -> String {
    let kind = match kind {
        KindRecord::Induced { mask, .. } => {
            format!("Induced({mask:#x},{})", top_fingerprint.unwrap_or("?"))
        }
        other => format!("{other:?}"),
    };
    format!("{fingerprint}|{kind}|{}|{slots:?}", node.unwrap_or("-"))
}

/// Seed statistics, for the assembly log.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CensusSeedReport {
    pub old_presentations: usize,
    pub matched_presentations: usize,
    pub old_sectors: usize,
    pub translated_sectors: usize,
    pub old_classes: usize,
    pub translated_classes: usize,
}

/// The census of the package in `directory` translated onto `presentations`
/// (the presentations of the assembly being made). Unmatched presentations,
/// and classes whose owner presentation is unmatched, are dropped.
pub fn census_seed(
    directory: &Path,
    presentations: &[Arc<Presentation>],
) -> Result<(PCensus, CensusSeedReport), PError> {
    let read = |name: &str| {
        std::fs::read(directory.join(name))
            .map_err(|error| malformed(format!("reading {}/{name}: {error}", directory.display())))
    };
    let manifest: LibraryManifest = serde_json::from_slice(&read(LIBRARY_FILE)?)
        .map_err(|error| malformed(format!("{LIBRARY_FILE}: {error}")))?;
    let mut fingerprints: BTreeMap<String, String> = BTreeMap::new();
    for record in &manifest.files.family_files {
        let family = decode_family_file(&read(&record.file)?, &record.file)?;
        fingerprints.insert(record.file.clone(), family.fingerprint().to_owned());
    }
    let fingerprint_of = |record: &PresentationRecord| {
        fingerprints
            .get(&record.family)
            .cloned()
            .ok_or_else(|| malformed(format!("the family file {} is not listed", record.family)))
    };
    let by_index: BTreeMap<u32, &PresentationRecord> = manifest
        .presentations
        .iter()
        .map(|p| (p.index, p))
        .collect();

    // New presentations by key; a key held twice is ambiguous and not used.
    let new_fingerprint: BTreeMap<PresentationId, String> = presentations
        .iter()
        .map(|p| (p.id, p.family.fingerprint().to_owned()))
        .collect();
    let mut new_keys: BTreeMap<String, Option<PresentationId>> = BTreeMap::new();
    for p in presentations {
        let record = PresentationRecord::of(p, String::new());
        let top = match &p.kind {
            PresentationKind::Induced { top, .. } => new_fingerprint.get(top).map(String::as_str),
            _ => None,
        };
        let key = match_key(
            p.family.fingerprint(),
            &record.kind,
            top,
            record.node.as_ref().map(|n| n.key.as_str()),
            &record.slots,
        );
        new_keys
            .entry(key)
            .and_modify(|slot| *slot = None)
            .or_insert(Some(p.id));
    }
    let mut ids: BTreeMap<PresentationId, PresentationId> = BTreeMap::new();
    for record in &manifest.presentations {
        let top = match &record.kind {
            KindRecord::Induced { top, .. } => by_index
                .get(top)
                .map(|top| fingerprint_of(top))
                .transpose()?,
            _ => None,
        };
        let key = match_key(
            &fingerprint_of(record)?,
            &record.kind,
            top.as_deref(),
            record.node.as_ref().map(|n| n.key.as_str()),
            &record.slots,
        );
        if let Some(Some(id)) = new_keys.get(&key) {
            ids.insert(PresentationId(record.index), *id);
        }
    }

    let old = decode_census(&read(CENSUS_FILE)?)?;
    let mut seed = PCensus::default();
    for (id, record) in &old.classes {
        let (owner, support) = record.owner;
        if let Some(&owner) = ids.get(&owner) {
            seed.classes.insert(
                *id,
                PClassRecord {
                    key: record.key.clone(),
                    owner: (owner, support),
                    node: None,
                    nodes: Vec::new(),
                    t: record.t,
                    eps_lines: record.eps_lines,
                },
            );
        }
    }
    for (&(id, support), fate) in &old.sectors {
        if let Some(&id) = ids.get(&id) {
            seed.sectors.insert((id, support), fate.clone());
        }
    }
    let report = CensusSeedReport {
        old_presentations: manifest.presentations.len(),
        matched_presentations: ids.len(),
        old_sectors: old.sectors.len(),
        translated_sectors: seed.sectors.len(),
        old_classes: old.classes.len(),
        translated_classes: seed.classes.len(),
    };
    Ok((seed, report))
}
