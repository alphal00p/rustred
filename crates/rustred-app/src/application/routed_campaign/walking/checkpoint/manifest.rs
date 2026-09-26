//! CP5 manifest: one small JSON authority over sectioned files. Every file it
//! references carries its byte length and blake3 digest; segmented sections
//! list per-generation segments that must tile `[0, total)` exactly.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub(super) const SCHEMA: u32 = 5;
pub(super) const FORMAT: &str = "RUSTRED-WALK-CP5";
pub const MAX_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;
pub(super) const FRESH_CAMPAIGN: &str =
    "unsupported checkpoint generation; complete dependency history requires a fresh CP5 campaign";
pub(super) const TILING: &str = "checkpoint segments do not tile the section total";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) enum Section {
    Meta,
    Nodes,
    Ledger,
    Index,
    Domains,
    Edges,
    Records,
}
impl Section {
    pub const ALL: [Section; 7] = [
        Section::Meta,
        Section::Nodes,
        Section::Ledger,
        Section::Index,
        Section::Domains,
        Section::Edges,
        Section::Records,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Meta => "meta",
            Self::Nodes => "nodes",
            Self::Ledger => "ledger",
            Self::Index => "index",
            Self::Domains => "domains",
            Self::Edges => "edges",
            Self::Records => "records",
        }
    }
    pub fn extension(self) -> &'static str {
        match self {
            Self::Meta => "json",
            Self::Records => "jsonl",
            _ => "bin",
        }
    }
    pub fn segmented(self) -> bool {
        matches!(self, Self::Domains | Self::Edges | Self::Records)
    }
    pub fn file_name(self, generation: u64) -> String {
        format!("{}-{generation:020}.{}", self.name(), self.extension())
    }
    /// Exactly `<section>-<20 digits>.<extension>`; anything else is not ours.
    pub fn parse(name: &str) -> Option<(Section, u64)> {
        let (stem, extension) = name.rsplit_once('.')?;
        let (section, digits) = stem.rsplit_once('-')?;
        if digits.len() != 20 || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let section = Self::ALL
            .into_iter()
            .find(|s| s.name() == section && s.extension() == extension)?;
        Some((section, digits.parse().ok()?))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct SectionRef {
    pub file: String,
    pub bytes: u64,
    pub blake3: String,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct Segment {
    pub generation: u64,
    pub file: String,
    pub first: u64,
    pub count: u64,
    pub bytes: u64,
    pub blake3: String,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct Segmented {
    pub total: u64,
    pub segments: Vec<Segment>,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct Sections {
    #[serde(default)]
    pub meta: Option<SectionRef>,
    #[serde(default)]
    pub nodes: Option<SectionRef>,
    #[serde(default)]
    pub ledger: Option<SectionRef>,
    #[serde(default)]
    pub index: Option<SectionRef>,
    #[serde(default)]
    pub domains: Option<Segmented>,
    #[serde(default)]
    pub edges: Option<Segmented>,
    #[serde(default)]
    pub records: Option<Segmented>,
}
impl Sections {
    pub fn plain(&self, section: Section) -> Option<&SectionRef> {
        match section {
            Section::Meta => self.meta.as_ref(),
            Section::Nodes => self.nodes.as_ref(),
            Section::Ledger => self.ledger.as_ref(),
            Section::Index => self.index.as_ref(),
            _ => None,
        }
    }
    pub fn segmented(&self, section: Section) -> Option<&Segmented> {
        match section {
            Section::Domains => self.domains.as_ref(),
            Section::Edges => self.edges.as_ref(),
            Section::Records => self.records.as_ref(),
            _ => None,
        }
    }
    pub fn segmented_mut(&mut self, section: Section) -> &mut Option<Segmented> {
        match section {
            Section::Domains => &mut self.domains,
            Section::Edges => &mut self.edges,
            Section::Records => &mut self.records,
            _ => unreachable!("plain section"),
        }
    }
    pub fn plain_mut(&mut self, section: Section) -> &mut Option<SectionRef> {
        match section {
            Section::Meta => &mut self.meta,
            Section::Nodes => &mut self.nodes,
            Section::Ledger => &mut self.ledger,
            Section::Index => &mut self.index,
            _ => unreachable!("segmented section"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct Manifest {
    pub schema: u32,
    pub format: String,
    pub kind: String,
    pub generation: u64,
    pub walk_semantics_version: u32,
    pub arity: u32,
    pub publication_policy: String,
    pub request: String,
    pub owners: Vec<String>,
    /// Digest of the executable that wrote this generation (informational).
    pub executable: String,
    /// Digest of the executable that bootstrapped the campaign.
    pub executable_first: String,
    pub sections: Sections,
    pub metadata: Value,
}
/// One referenced file: name, byte length and blake3 hex digest.
pub(super) struct FileRef<'a> {
    pub file: &'a str,
    pub bytes: u64,
    pub blake3: &'a str,
}
impl Manifest {
    pub fn files(&self) -> Vec<FileRef<'_>> {
        let mut out = Vec::new();
        for section in Section::ALL {
            if let Some(plain) = self.sections.plain(section) {
                out.push(FileRef {
                    file: &plain.file,
                    bytes: plain.bytes,
                    blake3: &plain.blake3,
                });
            }
            if let Some(segmented) = self.sections.segmented(section) {
                out.extend(segmented.segments.iter().map(|s| FileRef {
                    file: &s.file,
                    bytes: s.bytes,
                    blake3: &s.blake3,
                }));
            }
        }
        out
    }
    pub fn total_bytes(&self) -> u64 {
        self.files().iter().map(|f| f.bytes).sum()
    }
    pub fn validate_structure(&self) -> Result<(), String> {
        if self.schema != SCHEMA || self.format != FORMAT {
            return Err(FRESH_CAMPAIGN.into());
        }
        if !matches!(self.kind.as_str(), "bootstrap" | "state")
            || self.generation == 0
            || self.executable.is_empty()
            || self.executable_first.is_empty()
        {
            return Err("invalid checkpoint manifest kind or identity".into());
        }
        let valid_file = |section: Section, file: &str, generation: u64| {
            Section::parse(file) == Some((section, generation))
                && generation >= 1
                && generation <= self.generation
        };
        let valid_digest = |digest: &str| {
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        };
        for section in Section::ALL {
            if let Some(plain) = self.sections.plain(section) {
                if !valid_file(section, &plain.file, self.generation)
                    || !valid_digest(&plain.blake3)
                {
                    return Err(format!(
                        "checkpoint manifest references an invalid {} section",
                        section.name()
                    ));
                }
            }
            if let Some(segmented) = self.sections.segmented(section) {
                let mut next = 0u64;
                let mut last_generation = 0u64;
                for segment in &segmented.segments {
                    if !valid_file(section, &segment.file, segment.generation)
                        || !valid_digest(&segment.blake3)
                        || segment.generation <= last_generation
                    {
                        return Err(format!(
                            "checkpoint manifest references an invalid {} segment",
                            section.name()
                        ));
                    }
                    if segment.first != next
                        || segment.count == 0
                        || next.checked_add(segment.count).is_none()
                    {
                        return Err(TILING.into());
                    }
                    next += segment.count;
                    last_generation = segment.generation;
                }
                if next != segmented.total {
                    return Err(TILING.into());
                }
            }
        }
        let s = &self.sections;
        let present =
            |section: Section| s.plain(section).is_some() || s.segmented(section).is_some();
        match self.kind.as_str() {
            "bootstrap" => {
                if !present(Section::Meta) || Section::ALL[1..].iter().any(|&x| present(x)) {
                    return Err("bootstrap manifest carries state sections".into());
                }
            }
            _ => {
                if self.owners.is_empty() {
                    return Err("state manifest has no owner binding".into());
                }
                if Section::ALL
                    .iter()
                    .any(|&x| x != Section::Ledger && !present(x))
                {
                    return Err("state manifest is missing sections".into());
                }
            }
        }
        Ok(())
    }
}

/// Bounded read with a lenient pre-check so CP1/CP3/CP4 manifests and
/// unknown formats are refused with the fresh-campaign message, never a
/// serde error about missing fields.
pub(super) fn read(path: &Path) -> Result<Manifest, String> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|e| format!("cannot open checkpoint manifest: {e}"))?
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("cannot read checkpoint manifest: {e}"))?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err("checkpoint manifest exceeds bound".into());
    }
    let lenient: Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("invalid checkpoint manifest: {e}"))?;
    if lenient["schema"] != SCHEMA || lenient["format"] != FORMAT {
        return Err(FRESH_CAMPAIGN.into());
    }
    let manifest: Manifest =
        serde_json::from_value(lenient).map_err(|e| format!("invalid checkpoint manifest: {e}"))?;
    manifest.validate_structure()?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn section_names_round_trip_and_reject_foreign_files() {
        for section in Section::ALL {
            let name = section.file_name(42);
            assert_eq!(Section::parse(&name), Some((section, 42)));
        }
        for name in [
            "state-00000000000000000001.bin",
            "meta-1.json",
            "meta-00000000000000000001.bin",
            "domains-0000000000000000000a.bin",
            "latest.json",
            "checkpoint.lock",
            ".meta-00000000000000000001.json.rustred-tmp-1-0",
        ] {
            assert_eq!(Section::parse(name), None, "{name}");
        }
    }
    #[test]
    fn structure_validation_requires_tiling_and_kind_sections() {
        let segment = |generation: u64, first: u64, count: u64| Segment {
            generation,
            file: Section::Domains.file_name(generation),
            first,
            count,
            bytes: 1,
            blake3: "0".repeat(64),
        };
        let plain = |section: Section| SectionRef {
            file: section.file_name(3),
            bytes: 1,
            blake3: "a".repeat(64),
        };
        let mut manifest = Manifest {
            schema: SCHEMA,
            format: FORMAT.into(),
            kind: "state".into(),
            generation: 3,
            walk_semantics_version: 1,
            arity: 1,
            publication_policy: "ordered".into(),
            request: "r".into(),
            owners: vec!["o".into()],
            executable: "e".into(),
            executable_first: "e".into(),
            sections: Sections {
                meta: Some(plain(Section::Meta)),
                nodes: Some(plain(Section::Nodes)),
                ledger: None,
                index: Some(plain(Section::Index)),
                domains: Some(Segmented {
                    total: 5,
                    segments: vec![segment(2, 0, 3), segment(3, 3, 2)],
                }),
                edges: Some(Segmented::default()),
                records: Some(Segmented::default()),
            },
            metadata: json!({}),
        };
        manifest.validate_structure().unwrap();
        let mut gap = manifest.clone();
        gap.sections.domains.as_mut().unwrap().segments[1].first = 4;
        assert_eq!(gap.validate_structure().unwrap_err(), TILING);
        let mut short = manifest.clone();
        short.sections.domains.as_mut().unwrap().total = 6;
        assert_eq!(short.validate_structure().unwrap_err(), TILING);
        let mut future = manifest.clone();
        future.sections.domains.as_mut().unwrap().segments[1].generation = 4;
        assert!(future.validate_structure().is_err());
        let mut missing = manifest.clone();
        missing.sections.index = None;
        assert!(missing.validate_structure().is_err());
        let mut old = manifest.clone();
        old.schema = 4;
        assert_eq!(old.validate_structure().unwrap_err(), FRESH_CAMPAIGN);
        manifest.kind = "bootstrap".into();
        assert!(manifest.validate_structure().is_err());
        manifest.generation = 1; // Rewritten sections carry the manifest generation.
        manifest.sections = Sections {
            meta: Some(SectionRef {
                file: Section::Meta.file_name(1),
                bytes: 1,
                blake3: "a".repeat(64),
            }),
            ..Default::default()
        };
        manifest.validate_structure().unwrap();
    }
}
