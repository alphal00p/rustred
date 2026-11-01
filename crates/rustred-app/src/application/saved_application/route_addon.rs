//! Saved route preparation: a versioned add-on file of a saved application.
//!
//! Loading a saved application verifies every momentum route of its
//! selection exactly (`symmetry::verify` and the transport compiler). That
//! work depends only on the immutable application, so it can be done once:
//! [`write_saved_application_route_addon`] records the verified maps of all
//! transport routes into one file, and the loader rebuilds the identical
//! routes from it instead of verifying them again.
//!
//! Validity is versioning only, no source or program fingerprint: the file
//! carries its schema ([`SAVED_ROUTE_PREPARATION_SCHEMA`]), the hand-bumped
//! verified-map semantics version
//! ([`rustred::sector::symmetry::SAVED_VERIFIED_MAP_SEMANTICS`]) and the
//! identity of the application it belongs to (family fingerprint, the BLAKE3
//! of its `selection.json` as declared in `application.json`, the route
//! counts). A trailing BLAKE3 of the file's own bytes detects corruption.
//! Any mismatch rejects the file loudly (stderr and
//! [`SavedRoutePreparation::rejected`]) and the loader verifies the routes as
//! without the add-on; a rejected add-on never fails a load.
//!
//! The bytes are deterministic: routes in selection order, values in
//! first-occurrence order, exact decimal rationals (no Symbolica state).
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::{Value, json};

use super::package::read_bounded;
use super::*;
use crate::application::atomic_file::write_file_atomically;
use crate::application::routed_campaign::prepare::SavedRoutes;

/// File schema; bump it when the layout below changes.
pub const SAVED_ROUTE_PREPARATION_SCHEMA: &str = "rustred.saved-application.route-preparation.v1";
/// Default file name, next to `application.json` (picked up automatically).
pub const SAVED_ROUTE_PREPARATION_FILE: &str = "route-preparation.rrbin";
const MAGIC: &[u8; 16] = b"RR-SAVED-ROUTES1";
const CHECKSUM_BYTES: usize = 32;

/// Which route preparation add-on a load uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SavedRouteAddon<'a> {
    /// [`SAVED_ROUTE_PREPARATION_FILE`] next to `application.json`, when present.
    #[default]
    Auto,
    /// Verify every route; ignore any add-on.
    Disabled,
    /// This file; a missing or mismatched file is rejected loudly.
    File(&'a Path),
}

/// Where the routes of a loaded application came from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SavedRoutePreparationSource {
    /// Exact verification of every route (no add-on, or a rejected one).
    #[default]
    Verified,
    /// Restored from a matching route preparation add-on.
    Addon,
}

/// Route preparation report of one load.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SavedRoutePreparation {
    pub source: SavedRoutePreparationSource,
    /// The add-on that was used or rejected.
    pub addon: Option<PathBuf>,
    /// Why the add-on was rejected (the routes were verified instead).
    pub rejected: Option<String>,
}

impl SavedRoutePreparation {
    pub fn to_json(&self) -> Value {
        json!({
            "source": match self.source {
                SavedRoutePreparationSource::Verified => "verified",
                SavedRoutePreparationSource::Addon => "addon",
            },
            "addon": self.addon.as_ref().map(|path| path.display().to_string()),
            "rejected": self.rejected,
        })
    }

    pub(super) fn reject(&mut self, path: &Path, reason: String) {
        eprintln!(
            "rustred: route preparation add-on {} rejected ({reason}); verifying all routes",
            path.display()
        );
        self.source = SavedRoutePreparationSource::Verified;
        self.addon = Some(path.to_path_buf());
        self.rejected = Some(reason);
    }
}

/// Identity of the saved application an add-on belongs to.
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(super) struct AddonIdentity {
    family_fingerprint: String,
    selection_blake3: String,
}

impl AddonIdentity {
    /// Cheap identity fields of an `application.json` report.
    pub(super) fn of_application(report: &Value) -> Result<Self, AppError> {
        let field = |value: &Value, name: &str| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| AppError::input(format!("saved application missing {name}")))
        };
        Ok(Self {
            family_fingerprint: field(
                &report["metadata"]["family_fingerprint"],
                "family_fingerprint",
            )?,
            selection_blake3: field(&report["selection"]["blake3"], "selection blake3")?,
        })
    }
}

#[derive(bincode::Encode, bincode::Decode)]
struct Header {
    schema: String,
    map_semantics: String,
    identity: AddonIdentity,
    transport_routes: u64,
    values: u64,
}

fn encode(identity: &AddonIdentity, routes: &SavedRoutes) -> Result<Vec<u8>, AppError> {
    let config = bincode::config::standard();
    let header = Header {
        schema: SAVED_ROUTE_PREPARATION_SCHEMA.into(),
        map_semantics: rustred::sector::symmetry::SAVED_VERIFIED_MAP_SEMANTICS.into(),
        identity: identity.clone(),
        transport_routes: routes.routes.len() as u64,
        values: routes.values.len() as u64,
    };
    let header = bincode::encode_to_vec(&header, config).map_err(failure)?;
    let body = bincode::encode_to_vec(routes, config).map_err(failure)?;
    let mut bytes = Vec::with_capacity(MAGIC.len() + 16 + header.len() + body.len() + 32);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&(header.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&header);
    bytes.extend_from_slice(&(body.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&body);
    let checksum = blake3::hash(&bytes);
    bytes.extend_from_slice(checksum.as_bytes());
    Ok(bytes)
}

fn section<'a>(bytes: &mut &'a [u8]) -> Result<&'a [u8], String> {
    if bytes.len() < 8 {
        return Err("truncated file".into());
    }
    let (length, rest) = bytes.split_at(8);
    let length = usize::try_from(u64::from_le_bytes(length.try_into().expect("8 bytes")))
        .map_err(|_| "section length exceeds usize".to_owned())?;
    if rest.len() < length {
        return Err("truncated file".into());
    }
    let (section, rest) = rest.split_at(length);
    *bytes = rest;
    Ok(section)
}

/// Decode an add-on and check it against the application identity. The error
/// is the human-readable rejection reason.
pub(super) fn decode(bytes: &[u8], expected: &AddonIdentity) -> Result<SavedRoutes, String> {
    if bytes.len() < MAGIC.len() + CHECKSUM_BYTES || &bytes[..MAGIC.len()] != MAGIC {
        return Err("not a route preparation add-on".into());
    }
    let (content, checksum) = bytes.split_at(bytes.len() - CHECKSUM_BYTES);
    if blake3::hash(content).as_bytes() != checksum {
        return Err("file checksum mismatch (corrupt or truncated)".into());
    }
    let config = bincode::config::standard();
    let mut rest = &content[MAGIC.len()..];
    let header_bytes = section(&mut rest)?;
    let body = section(&mut rest)?;
    if !rest.is_empty() {
        return Err("trailing bytes".into());
    }
    let (header, used): (Header, usize) = bincode::decode_from_slice(header_bytes, config)
        .map_err(|e| format!("unreadable header: {e}"))?;
    if used != header_bytes.len() {
        return Err("trailing header bytes".into());
    }
    if header.schema != SAVED_ROUTE_PREPARATION_SCHEMA {
        return Err(format!(
            "schema {} differs from {SAVED_ROUTE_PREPARATION_SCHEMA}",
            header.schema
        ));
    }
    if header.map_semantics != rustred::sector::symmetry::SAVED_VERIFIED_MAP_SEMANTICS {
        return Err(format!(
            "verified-map semantics {} differs from {}",
            header.map_semantics,
            rustred::sector::symmetry::SAVED_VERIFIED_MAP_SEMANTICS
        ));
    }
    if header.identity.family_fingerprint != expected.family_fingerprint {
        return Err("written for another integral family".into());
    }
    if header.identity.selection_blake3 != expected.selection_blake3 {
        return Err(format!(
            "written for selection {}, this application has {}",
            header.identity.selection_blake3, expected.selection_blake3
        ));
    }
    let (routes, used): (SavedRoutes, usize) =
        bincode::decode_from_slice(body, config).map_err(|e| format!("unreadable routes: {e}"))?;
    if used != body.len()
        || routes.routes.len() as u64 != header.transport_routes
        || routes.values.len() as u64 != header.values
    {
        return Err("route records differ from the header".into());
    }
    Ok(routes)
}

/// Locate and read the add-on a load uses. `Err((path, reason))` is a rejection.
pub(super) fn read_for_load(
    directory: &Path,
    addon: SavedRouteAddon<'_>,
    identity: &AddonIdentity,
    maximum: usize,
) -> Result<Option<(PathBuf, SavedRoutes)>, (PathBuf, String)> {
    let path = match addon {
        SavedRouteAddon::Disabled => return Ok(None),
        SavedRouteAddon::Auto => {
            let path = directory.join(SAVED_ROUTE_PREPARATION_FILE);
            if !path.is_file() {
                return Ok(None);
            }
            path
        }
        SavedRouteAddon::File(path) => path.to_path_buf(),
    };
    let bytes = match read_bounded(&path, maximum) {
        Ok(bytes) => bytes,
        Err(error) => return Err((path, error.to_string())),
    };
    match decode(&bytes, identity) {
        Ok(routes) => Ok(Some((path, routes))),
        Err(reason) => Err((path, reason)),
    }
}

/// Verify every route of a saved application (as a load without add-on does)
/// and record the verified maps into a new add-on file at `destination`
/// (never replaced). The application itself is not modified.
pub fn write_saved_application_route_addon<const N: usize>(
    application: &Path,
    destination: &Path,
    options: SavedApplicationOptions,
) -> Result<Value, AppError> {
    let started = Instant::now();
    let prepared =
        package::load_with_addon::<N>(application, options, None, SavedRouteAddon::Disabled)?;
    let load_seconds = started.elapsed().as_secs_f64();
    let (selection, _, _) = Selection::parse_with_byte_limit(
        &prepared.inputs.selection,
        package::metadata_limit(None)?,
    )?;
    let routes = prepare::capture_saved_routes::<N>(&selection, &prepared.application)?;
    let identity = AddonIdentity::of_application(&package::application_report(application)?)?;
    let bytes = encode(&identity, &routes)?;
    // Self-check: the written bytes decode to the same records.
    if decode(&bytes, &identity).as_ref() != Ok(&routes) {
        return Err(AppError::execution(
            "route preparation add-on does not decode to its records",
        ));
    }
    write_file_atomically(destination, &bytes, false).map_err(failure)?;
    Ok(json!({
        "schema": SAVED_ROUTE_PREPARATION_SCHEMA,
        "map_semantics": rustred::sector::symmetry::SAVED_VERIFIED_MAP_SEMANTICS,
        "path": destination.display().to_string(),
        "bytes": bytes.len(),
        "blake3": blake3::hash(&bytes).to_hex().to_string(),
        "family_fingerprint_blake3": blake3::hash(identity.family_fingerprint.as_bytes()).to_hex().to_string(),
        "selection_blake3": identity.selection_blake3,
        "transport_routes": routes.routes.len(),
        "values": routes.values.len(),
        "preparation_workers": options.preparation_workers,
        "verifying_load_seconds": load_seconds,
        "wall_seconds": started.elapsed().as_secs_f64(),
    }))
}

/// Load `application` twice, verifying and from `addon`, and require
/// identical routes (owners, roots, family identities and complete verified
/// maps). The returned report contains both load times.
pub fn check_saved_application_route_addon<const N: usize>(
    application: &Path,
    addon: &Path,
    options: SavedApplicationOptions,
) -> Result<Value, AppError> {
    let started = Instant::now();
    let verified =
        package::load_with_addon::<N>(application, options, None, SavedRouteAddon::Disabled)?;
    let verified_seconds = started.elapsed().as_secs_f64();
    let started = Instant::now();
    let restored =
        package::load_with_addon::<N>(application, options, None, SavedRouteAddon::File(addon))?;
    let restored_seconds = started.elapsed().as_secs_f64();
    if restored.route_preparation.source != SavedRoutePreparationSource::Addon {
        return Err(AppError::input(format!(
            "route preparation add-on was not used: {}",
            restored
                .route_preparation
                .rejected
                .as_deref()
                .unwrap_or("unknown reason")
        )));
    }
    let left = verified.application.routes().collect::<Vec<_>>();
    let right = restored.application.routes().collect::<Vec<_>>();
    if left.len() != right.len() {
        return Err(AppError::execution("restored route count differs"));
    }
    for ((left_key, left), (right_key, right)) in left.iter().zip(&right) {
        let (a, b) = (&left.transport, &right.transport);
        if left_key != right_key
            || left.owner_sector != right.owner_sector
            || a.source_root() != b.source_root()
            || a.target_root() != b.target_root()
            || a.source_family_fingerprint() != b.source_family_fingerprint()
            || a.target_family().fingerprint() != b.target_family().fingerprint()
            || a.verified_map() != b.verified_map()
        {
            return Err(AppError::execution(
                "a restored route differs from its verified route",
            ));
        }
    }
    Ok(json!({
        "routes": left.len(),
        "identical": true,
        "verified": {
            "wall_seconds": verified_seconds,
            "route_preparation_seconds": verified.preparation_statistics.route_preparation.as_secs_f64(),
        },
        "addon": {
            "wall_seconds": restored_seconds,
            "route_preparation_seconds": restored.preparation_statistics.route_preparation.as_secs_f64(),
        },
        "preparation_workers": options.preparation_workers,
    }))
}

fn failure(error: impl std::fmt::Display) -> AppError {
    AppError::execution(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustred::sector::symmetry::SavedValueTableBuilder;

    fn identity() -> AddonIdentity {
        AddonIdentity {
            family_fingerprint: "family".into(),
            selection_blake3: "a".repeat(64),
        }
    }

    fn routes() -> SavedRoutes {
        SavedRoutes {
            values: SavedValueTableBuilder::new().finish(),
            routes: Vec::new(),
        }
    }

    #[test]
    fn decode_checks_magic_checksum_versions_and_identity() {
        let bytes = encode(&identity(), &routes()).unwrap();
        assert_eq!(encode(&identity(), &routes()).unwrap(), bytes);
        assert_eq!(decode(&bytes, &identity()).unwrap(), routes());
        let mut other = identity();
        other.selection_blake3 = "b".repeat(64);
        assert!(
            decode(&bytes, &other)
                .unwrap_err()
                .contains("written for selection")
        );
        other = identity();
        other.family_fingerprint = "other".into();
        assert!(
            decode(&bytes, &other)
                .unwrap_err()
                .contains("another integral family")
        );
        let mut corrupt = bytes.clone();
        corrupt[20] ^= 1;
        assert!(
            decode(&corrupt, &identity())
                .unwrap_err()
                .contains("checksum")
        );
        assert!(
            decode(&bytes[..bytes.len() - 1], &identity())
                .unwrap_err()
                .contains("checksum")
        );
        assert!(decode(b"not an add-on", &identity()).is_err());
        // A different version is rejected even with a valid checksum.
        let mut header = Header {
            schema: SAVED_ROUTE_PREPARATION_SCHEMA.into(),
            map_semantics: "rustred-verified-map-1999-01-01".into(),
            identity: identity(),
            transport_routes: 0,
            values: 0,
        };
        for semantics in [false, true] {
            if semantics {
                header.schema = "rustred.saved-application.route-preparation.v0".into();
                header.map_semantics =
                    rustred::sector::symmetry::SAVED_VERIFIED_MAP_SEMANTICS.into();
            }
            let config = bincode::config::standard();
            let header = bincode::encode_to_vec(&header, config).unwrap();
            let body = bincode::encode_to_vec(routes(), config).unwrap();
            let mut bytes = MAGIC.to_vec();
            bytes.extend_from_slice(&(header.len() as u64).to_le_bytes());
            bytes.extend_from_slice(&header);
            bytes.extend_from_slice(&(body.len() as u64).to_le_bytes());
            bytes.extend_from_slice(&body);
            let checksum = blake3::hash(&bytes);
            bytes.extend_from_slice(checksum.as_bytes());
            let reason = decode(&bytes, &identity()).unwrap_err();
            assert!(reason.contains("differs from"), "{reason}");
        }
    }
}
