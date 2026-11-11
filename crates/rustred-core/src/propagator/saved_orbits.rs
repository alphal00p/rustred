//! Saved verified automorphism groups of a p library (p artefact v2, next to
//! the saved routes; M5 batch 2, m5-a2-canon).
//!
//! Orbit canonicalisation (module `orbit`, M5 R0 A2) needs, per owner sector
//! and per presentation (its whole line set), the group of automorphisms that
//! `symmetry::verify` authenticated. Searching and authenticating them at run
//! time cost 1-4 s per process (A2 report, section 3.2). A package can carry
//! them once, recorded when it is written ([`export_saved_orbits`], file
//! `orbits.rrbin`): every group element is stored as its integer momentum
//! matrix; at run time the engine restores a sector's group on its first use
//! and recomputes the slot actions from the presentation (integer algebra, no
//! search, no `verify`). As for `routes.rrbin`, this is a trusted restore:
//! the record binds to the presentation id, the support, and a fingerprint of
//! the presentation's slots, loops and eps units (any mismatch: the group is
//! searched and authenticated as without the file), and to the package files
//! (`SHA256SUMS`, vakint's listed-file check). The restored group must also
//! pass cheap structural checks (shape, `|det| = 1` on the loop block, `q`
//! fixed up to sign, the identity present). Loading with `verify_descent`
//! ignores the file. Groups change no value: canonicalisation replaces a key
//! by an exact image, and the file is not part of the library identity.
use std::collections::{BTreeMap, BTreeSet};

use super::PresentationId;
use super::error::PError;
use super::library::PLibrary;
use super::orbit::{OwnerOrbit, orbit_of_group, owner_orbit};
use super::presentation::{Presentation, Slot};

/// Schema of `orbits.rrbin`; bump it when its record layout or the meaning of
/// a saved group changes (with `P_ORBIT_CANON_VERSION` when the search does).
pub const P_SAVED_ORBITS_SCHEMA: &str = "rustred.p-saved-orbits.v1";

#[derive(bincode::Encode, bincode::Decode)]
struct OrbitsFile {
    schema: String,
    /// `P_ORBIT_CANON_VERSION` of the search that produced the groups.
    canon_version: u32,
    entries: Vec<OrbitEntry>,
}

#[derive(bincode::Encode, bincode::Decode)]
struct OrbitEntry {
    presentation: u32,
    support: u64,
    /// [`fingerprint`] of the presentation the group was authenticated on.
    fingerprint: u64,
    candidates: u32,
    verified: u32,
    /// `n = L + 1`.
    n: u8,
    /// The group, `n x n` row-major per element.
    group: Vec<Vec<i64>>,
}

struct SavedOrbit {
    fingerprint: u64,
    candidates: usize,
    verified: usize,
    n: usize,
    group: Vec<Vec<i64>>,
}

/// The saved automorphism groups of a library, by `(presentation, support)`.
#[derive(Default)]
pub struct PSavedOrbits {
    orbits: BTreeMap<(PresentationId, u64), SavedOrbit>,
}

impl std::fmt::Debug for PSavedOrbits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PSavedOrbits({} sectors)", self.orbits.len())
    }
}

fn malformed(detail: impl std::fmt::Display) -> PError {
    PError::MalformedLibrary(format!("saved orbits: {detail}"))
}

/// FNV-1a over the loops, and per slot its kind, momenta and eps units: what
/// the search and the slot actions read from a presentation.
pub(crate) fn fingerprint(presentation: &Presentation) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut eat = |value: i64| {
        for byte in value.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    };
    eat(i64::from(presentation.loops));
    eat(presentation.slots.len() as i64);
    for slot in &presentation.slots {
        match slot {
            Slot::Line(line) => {
                eat(1);
                eat(i64::from(line.eps_units));
                eat(line.momentum.len() as i64);
                line.momentum.iter().for_each(|&c| eat(c));
            }
            Slot::Isp(isp) => {
                eat(2);
                eat(isp.left.len() as i64);
                isp.left.iter().for_each(|&c| eat(c));
                eat(isp.right.len() as i64);
                isp.right.iter().for_each(|&c| eat(c));
            }
        }
    }
    hash
}

impl PSavedOrbits {
    /// Read `orbits.rrbin`: its schema and canonicalisation version first.
    pub fn decode(bytes: &[u8]) -> Result<Self, PError> {
        let (file, used): (OrbitsFile, usize) =
            bincode::decode_from_slice(bytes, bincode::config::standard()).map_err(malformed)?;
        if used != bytes.len() {
            return Err(malformed("trailing bytes"));
        }
        if file.schema != P_SAVED_ORBITS_SCHEMA {
            return Err(PError::VersionMismatch {
                name: "P_SAVED_ORBITS_SCHEMA".to_owned(),
                found: file.schema,
                expected: P_SAVED_ORBITS_SCHEMA.to_owned(),
            });
        }
        if file.canon_version != super::P_ORBIT_CANON_VERSION {
            return Err(PError::VersionMismatch {
                name: "P_ORBIT_CANON_VERSION".to_owned(),
                found: file.canon_version.to_string(),
                expected: super::P_ORBIT_CANON_VERSION.to_string(),
            });
        }
        let mut orbits = BTreeMap::new();
        for entry in file.entries {
            let saved = SavedOrbit {
                fingerprint: entry.fingerprint,
                candidates: entry.candidates as usize,
                verified: entry.verified as usize,
                n: usize::from(entry.n),
                group: entry.group,
            };
            if orbits
                .insert((PresentationId(entry.presentation), entry.support), saved)
                .is_some()
            {
                return Err(malformed(format!(
                    "sector {:#x} of presentation {} twice",
                    entry.support, entry.presentation
                )));
            }
        }
        Ok(Self { orbits })
    }

    pub fn len(&self) -> usize {
        self.orbits.len()
    }

    pub fn is_empty(&self) -> bool {
        self.orbits.is_empty()
    }

    /// The saved group of sector `support` of `presentation`, restored as an
    /// orbit; `None` without a record for exactly this presentation content,
    /// or when the record fails the structural checks.
    pub(crate) fn restore(&self, presentation: &Presentation, support: u64) -> Option<OwnerOrbit> {
        let saved = self.orbits.get(&(presentation.id, support))?;
        let n = usize::from(presentation.loops) + 1;
        if saved.fingerprint != fingerprint(presentation) {
            return None;
        }
        if saved.group.is_empty() {
            // The search found no basis (or no loop): no automorphism at all.
            return Some(OwnerOrbit {
                candidates: saved.candidates,
                verified: saved.verified,
                ..OwnerOrbit::default()
            });
        }
        if saved.n != n {
            return None;
        }
        let unit: Vec<i64> = (0..n * n)
            .map(|entry| i64::from(entry % (n + 1) == 0))
            .collect();
        let mut has_unit = false;
        let mut distinct = BTreeSet::new();
        for t in &saved.group {
            if t.len() != n * n || !structurally_sound(t, n) || !distinct.insert(t) {
                return None;
            }
            has_unit |= *t == unit;
        }
        if !has_unit {
            return None;
        }
        let group = saved
            .group
            .iter()
            .map(|t| t.iter().map(|&c| i128::from(c)).collect())
            .collect();
        Some(orbit_of_group(
            presentation,
            support,
            group,
            saved.candidates,
            saved.verified,
        ))
    }
}

/// `q` fixed up to sign (`t[L][..L] = 0`, `t[L][L] = +-1`) and `|det A| = 1`
/// for the loop block `A`.
fn structurally_sound(t: &[i64], n: usize) -> bool {
    let loops = n - 1;
    if (0..loops).any(|column| t[loops * n + column] != 0) || t[loops * n + loops].abs() != 1 {
        return false;
    }
    let a: Vec<Vec<i128>> = (0..loops)
        .map(|row| {
            (0..loops)
                .map(|column| i128::from(t[row * n + column]))
                .collect()
        })
        .collect();
    super::orbit::determinant_of(&a).abs() == 1
}

/// Every sector whose group orbit canonicalisation asks for: the whole line
/// set of every presentation, and the sector of every owner class.
pub(crate) fn orbit_sectors(library: &PLibrary) -> Vec<(PresentationId, u64)> {
    let mut sectors: BTreeSet<(PresentationId, u64)> = library
        .presentations()
        .iter()
        .map(|presentation| {
            let lines = presentation
                .slots
                .iter()
                .enumerate()
                .filter(|(_, slot)| matches!(slot, Slot::Line(_)))
                .fold(0_u64, |mask, (slot, _)| mask | 1 << slot);
            (presentation.id, lines)
        })
        .collect();
    sectors.extend(library.census().classes.values().map(|record| record.owner));
    sectors.into_iter().collect()
}

/// Search and authenticate the group of every sector of [`orbit_sectors`] and
/// record it: `(orbits.rrbin bytes, sectors saved, sectors with a nontrivial
/// group)`. Every generator is proved here by `symmetry::verify`; only the
/// record is trusted when a package loads.
pub fn export_saved_orbits(library: &PLibrary) -> Result<(Vec<u8>, usize, usize), PError> {
    use rayon::prelude::*;
    let records: Vec<Result<OrbitEntry, PError>> = orbit_sectors(library)
        .into_par_iter()
        .map(|(presentation, support)| {
            let presentation = library.presentation(presentation);
            let orbit = owner_orbit(presentation, support)?;
            let n = usize::from(presentation.loops) + 1;
            let group = orbit
                .group
                .iter()
                .map(|t| {
                    t.iter()
                        .map(|&c| {
                            i64::try_from(c).map_err(|_| malformed("group entry overflows i64"))
                        })
                        .collect::<Result<Vec<i64>, PError>>()
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(OrbitEntry {
                presentation: presentation.id.0,
                support,
                fingerprint: fingerprint(presentation),
                candidates: u32::try_from(orbit.candidates).map_err(malformed)?,
                verified: u32::try_from(orbit.verified).map_err(malformed)?,
                n: u8::try_from(n).map_err(malformed)?,
                group,
            })
        })
        .collect();
    let mut entries = Vec::with_capacity(records.len());
    for record in records {
        entries.push(record?);
    }
    let saved = entries.len();
    let nontrivial = entries.iter().filter(|entry| entry.group.len() > 2).count();
    let file = OrbitsFile {
        schema: P_SAVED_ORBITS_SCHEMA.to_owned(),
        canon_version: super::P_ORBIT_CANON_VERSION,
        entries,
    };
    let bytes = bincode::encode_to_vec(&file, bincode::config::standard()).map_err(malformed)?;
    Ok((bytes, saved, nontrivial))
}
