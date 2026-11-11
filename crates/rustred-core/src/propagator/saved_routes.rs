//! Saved verified route maps of a p library (p artefact v2,
//! notes/artifact-ordering.SPEC.md item 4; owner: M5 m5-1l3l).
//!
//! Every Route key of a routed sector is transported onto its owner through a
//! verified momentum map (`engine::compile_route`: `symmetry::verify`, then
//! `integral_transport::compile`). The verification is exact algebra and
//! was a quarter of a cold 1L-3L request (M5 1L-3L profile). A package can
//! carry the verifications once, recorded when it is written
//! ([`export_saved_routes`], file `routes.rrbin`): the engine then restores
//! the map of a sector on its first use (`SavedVerifiedMap::restore`, a
//! trusted restore that checks shapes and table membership, not the algebra)
//! and compiles the same transport. A sector without a record, or a record
//! that does not restore, is verified as before, so a missing or partial file
//! changes nothing but time. The records are bound to the library by
//! [`P_SAVED_ROUTES_SCHEMA`] and `SAVED_VERIFIED_MAP_SEMANTICS` (a mismatch is
//! refused at load: [`PError::VersionMismatch`]) and by the package files
//! (`SHA256SUMS`, vakint's listed-file check). Loading with `verify_descent`
//! ignores them (`load_p_library`).
use std::collections::BTreeMap;

use crate::sector::symmetry::VerifiedMap;
use crate::sector::symmetry::saved::{
    SAVED_VERIFIED_MAP_SEMANTICS, SavedValueTable, SavedValueTableBuilder, SavedVerifiedMap,
};

use super::PresentationId;
use super::census::SignedAffineLoopMap;
use super::engine::{routed_sectors, verify_route};
use super::error::PError;
use super::library::PLibrary;
use super::presentation::Presentation;

/// Schema of `routes.rrbin`; bump it when its record layout changes.
pub const P_SAVED_ROUTES_SCHEMA: &str = "rustred.p-saved-routes.v2";

#[derive(bincode::Encode, bincode::Decode)]
struct RoutesFile {
    schema: String,
    semantics: String,
    entries: Vec<RouteEntry>,
}

#[derive(bincode::Encode, bincode::Decode)]
struct RouteEntry {
    presentation: u32,
    support: u64,
    /// What the record was verified for: the owner's presentation and
    /// support and the census map. A record is restored only for exactly
    /// these (a package whose census routes the sector otherwise verifies it).
    target: u32,
    target_support: u64,
    map: SavedLoopMap,
    /// `RouteBlob`, decoded on the sector's first use.
    blob: Vec<u8>,
}

#[derive(Clone, PartialEq, Eq, bincode::Encode, bincode::Decode)]
struct SavedLoopMap {
    loops_in: u8,
    loops_out: u8,
    loop_linear: Vec<i64>,
    loop_external: Vec<i64>,
    external_sign: i8,
}

impl SavedLoopMap {
    fn of(map: &SignedAffineLoopMap) -> Self {
        Self {
            loops_in: map.loops_in,
            loops_out: map.loops_out,
            loop_linear: map.loop_linear.to_vec(),
            loop_external: map.loop_external.to_vec(),
            external_sign: map.external_sign,
        }
    }
}

/// A record restorable for one routed sector.
struct SavedRoute {
    target: (PresentationId, u64),
    map: SavedLoopMap,
    blob: Vec<u8>,
}

#[derive(bincode::Encode, bincode::Decode)]
struct RouteBlob {
    table: SavedValueTable,
    map: SavedVerifiedMap,
}

/// The saved route verifications of a library, by routed sector.
#[derive(Default)]
pub struct PSavedRoutes {
    routes: BTreeMap<(PresentationId, u64), SavedRoute>,
}

impl std::fmt::Debug for PSavedRoutes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PSavedRoutes({} sectors)", self.routes.len())
    }
}

fn malformed(detail: impl std::fmt::Display) -> PError {
    PError::MalformedLibrary(format!("saved routes: {detail}"))
}

impl PSavedRoutes {
    /// Read `routes.rrbin`: its schema and the verified-map semantics first.
    pub fn decode(bytes: &[u8]) -> Result<Self, PError> {
        let (file, used): (RoutesFile, usize) =
            bincode::decode_from_slice(bytes, bincode::config::standard()).map_err(malformed)?;
        if used != bytes.len() {
            return Err(malformed("trailing bytes"));
        }
        for (name, found, expected) in [
            ("P_SAVED_ROUTES_SCHEMA", &file.schema, P_SAVED_ROUTES_SCHEMA),
            (
                "SAVED_VERIFIED_MAP_SEMANTICS",
                &file.semantics,
                SAVED_VERIFIED_MAP_SEMANTICS,
            ),
        ] {
            if found != expected {
                return Err(PError::VersionMismatch {
                    name: name.to_owned(),
                    found: found.clone(),
                    expected: expected.to_owned(),
                });
            }
        }
        let mut routes = BTreeMap::new();
        for entry in file.entries {
            let route = SavedRoute {
                target: (PresentationId(entry.target), entry.target_support),
                map: entry.map,
                blob: entry.blob,
            };
            if routes
                .insert((PresentationId(entry.presentation), entry.support), route)
                .is_some()
            {
                return Err(malformed(format!(
                    "sector {:#x} of presentation {} twice",
                    entry.support, entry.presentation
                )));
            }
        }
        Ok(Self { routes })
    }

    pub fn len(&self) -> usize {
        self.routes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }

    /// The saved map of sector `support` of `source` onto sector
    /// `target_support` of `target` through `map`, restored; `None` without a
    /// record for exactly these, or when it does not restore.
    pub(crate) fn restore(
        &self,
        source: &Presentation,
        target: &Presentation,
        support: u64,
        target_support: u64,
        map: &SignedAffineLoopMap,
    ) -> Option<VerifiedMap> {
        let route = self.routes.get(&(source.id, support))?;
        if route.target != (target.id, target_support) || route.map != SavedLoopMap::of(map) {
            return None;
        }
        let (blob, _): (RouteBlob, usize) =
            bincode::decode_from_slice(&route.blob, bincode::config::standard()).ok()?;
        let values = blob
            .table
            .materialize(source.family.coefficient_context())
            .ok()?;
        blob.map
            .restore(&source.family, &target.family, &values)
            .ok()
    }
}

/// Verify the route of every routed sector of `library` and record it:
/// `(routes.rrbin bytes, sectors saved, sectors left to run-time
/// verification because their map has a non-constant coefficient)`.
pub fn export_saved_routes(library: &PLibrary) -> Result<(Vec<u8>, usize, usize), PError> {
    use rayon::prelude::*;
    // Independent exact verifications: on the caller's rayon pool, in census
    // order (the file does not depend on the thread count).
    let records: Vec<Result<Option<RouteEntry>, PError>> = routed_sectors(library)
        .into_par_iter()
        .map(|(presentation, support, target_id, target_support, map)| {
            let source = library.presentation(presentation);
            let target = library.presentation(target_id);
            let verified = verify_route(source, target, map, support)?;
            let mut table = SavedValueTableBuilder::new();
            let Ok(saved) = SavedVerifiedMap::capture(&verified, &mut table) else {
                return Ok(None);
            };
            let blob = RouteBlob {
                table: table.finish(),
                map: saved,
            };
            Ok(Some(RouteEntry {
                presentation: presentation.0,
                support,
                target: target_id.0,
                target_support,
                map: SavedLoopMap::of(map),
                blob: bincode::encode_to_vec(&blob, bincode::config::standard())
                    .map_err(malformed)?,
            }))
        })
        .collect();
    let mut entries = Vec::with_capacity(records.len());
    let mut skipped = 0;
    for record in records {
        match record? {
            Some(entry) => entries.push(entry),
            None => skipped += 1,
        }
    }
    let saved = entries.len();
    let file = RoutesFile {
        schema: P_SAVED_ROUTES_SCHEMA.to_owned(),
        semantics: SAVED_VERIFIED_MAP_SEMANTICS.to_owned(),
        entries,
    };
    let bytes = bincode::encode_to_vec(&file, bincode::config::standard()).map_err(malformed)?;
    Ok((bytes, saved, skipped))
}
