//! Structural scope proposals. Only the final cell installer can seal them.

use crate::sector::{Mask, OrderingPolicy};

use super::super::source_port::scope::sector_count;
use super::super::{ArtifactError, ArtifactPersistenceError, TotalExcessProofScope};
use super::binary::{Reader, Writer, check_limit, try_vec};
use super::semantic::{decode_bool_vec_checked, encode_bool_slice};
use super::{ArtifactLoadLimits, ArtifactSchemaVersion, SOURCE_PORT_ALGORITHM_ID};

const VERSION: u32 = 1;
const TOTAL_EXCESS: u8 = 1;

pub(super) struct Proposal {
    pub root: Mask,
    pub entry_degree: u64,
    pub degrees: Vec<(Mask, u64)>,
}

fn invalid(field: &'static str) -> ArtifactPersistenceError {
    ArtifactPersistenceError::SemanticMismatch { field }
}

pub(super) fn encode(
    writer: &mut Writer,
    scope: &TotalExcessProofScope,
) -> Result<(), ArtifactPersistenceError> {
    writer.u32(VERSION)?;
    writer.u8(TOTAL_EXCESS)?;
    writer.u64(scope.max_entry_total_excess_degree())?;
    encode_bool_slice(writer, scope.root_sector().active_bits())?;
    writer.usize(scope.successor_degrees().len(), "bounded successor sectors")?;
    for (sector, degree) in scope.successor_degrees() {
        encode_bool_slice(writer, sector.active_bits())?;
        writer.u64(*degree)?;
    }
    Ok(())
}

pub(super) fn decode(
    bytes: &[u8],
    limits: ArtifactLoadLimits,
) -> Result<Proposal, ArtifactPersistenceError> {
    let mut reader = Reader::root(bytes, limits)?;
    if reader.u32()? != VERSION || reader.u8()? != TOTAL_EXCESS {
        return Err(invalid("bounded scope version or degree convention"));
    }
    let entry_degree = reader.u64()?;
    let root = Mask::try_new(decode_bool_vec_checked(
        &mut reader,
        "bounded root sector",
        |arity| {
            check_limit("bounded scope arity", arity, limits.cover_replay.max_arity)?;
            check_limit(
                "bounded scope coordinate cells",
                arity,
                limits.cover_replay.max_requested_box_coordinate_cells,
            )
        },
    )?)
    .map_err(ArtifactError::from)?;
    let count = reader.count("bounded successor sectors")?;
    // Match the consuming producer's current support: a zero-only root is not
    // an admitted source-port program. Empty cells remain valid when explicit
    // terminals cover a nonempty nonzero-sector envelope exactly.
    if count == 0 {
        return Err(invalid(
            "zero-only bounded source-port roots are unsupported",
        ));
    }
    check_limit(
        "bounded successor sectors",
        count,
        limits.cover_replay.max_requested_boxes,
    )?;
    let census = sector_count(&root)?;
    check_limit(
        "bounded root census",
        census,
        limits.cover_replay.max_requested_boxes,
    )?;
    let census_coordinates = census.checked_mul(root.arity()).ok_or(
        ArtifactPersistenceError::ResourceCountOverflow {
            resource: "bounded root census coordinates",
        },
    )?;
    check_limit(
        "bounded root census coordinates",
        census_coordinates,
        limits.cover_replay.max_requested_box_coordinate_cells,
    )?;
    if count > census {
        return Err(invalid("bounded successor census"));
    }
    let coordinates = count
        .checked_add(1)
        .and_then(|n| n.checked_mul(root.arity()))
        .ok_or(ArtifactPersistenceError::ResourceCountOverflow {
            resource: "bounded scope coordinate cells",
        })?;
    check_limit(
        "bounded scope coordinate cells",
        coordinates,
        limits.cover_replay.max_requested_box_coordinate_cells,
    )?;
    let mut degrees: Vec<(Mask, u64)> = try_vec(count, "bounded successor sectors")?;
    for _ in 0..count {
        let sector = Mask::try_new(decode_bool_vec_checked(
            &mut reader,
            "bounded successor sector",
            |arity| {
                if arity != root.arity() {
                    Err(invalid("bounded successor degree map"))
                } else {
                    Ok(())
                }
            },
        )?)
        .map_err(ArtifactError::from)?;
        let degree = reader.u64()?;
        if sector.arity() != root.arity()
            || !sector.is_subsector_of(&root).map_err(ArtifactError::from)?
            || degrees
                .last()
                .is_some_and(|(previous, _)| previous >= &sector)
            || degree < entry_degree
        {
            return Err(invalid("bounded successor degree map"));
        }
        degrees.push((sector, degree));
    }
    reader.finish()?;
    Ok(Proposal {
        root,
        entry_degree,
        degrees,
    })
}

/// Reject structurally decidable scope/body mismatches before importing State.
/// This uses the same framing cursor and source-plan root decoder as replay;
/// no coefficient-bearing recipe is interpreted here.
pub(super) fn preflight_body(
    bytes: &[u8],
    scope: &Proposal,
    limits: ArtifactLoadLimits,
) -> Result<(), ArtifactPersistenceError> {
    let mut reader = Reader::root(bytes, limits)?;
    if reader.fixed(super::MAGIC.len())? != super::MAGIC {
        return Err(ArtifactPersistenceError::InvalidMagic);
    }
    let schema = reader.u32()?;
    if schema != ArtifactSchemaVersion::CURRENT.as_u32() {
        return Err(ArtifactPersistenceError::UnsupportedSchema { actual: schema });
    }
    if reader.u32()? != super::SECTION_COUNT {
        return Err(invalid("section count"));
    }
    let metadata = reader.section(super::METADATA_SECTION)?;
    reader.section(super::FAMILY_SECTION)?;
    reader.section(super::SOURCES_SECTION)?;
    let rules = reader.section(super::RULES_SECTION)?;
    reader.section(super::TERMINALS_SECTION)?;
    reader.finish()?;
    let mut metadata = reader.child(metadata);
    if metadata.string("algorithm identifier")? != SOURCE_PORT_ALGORITHM_ID {
        return Err(invalid("bounded source-port algorithm"));
    }
    if metadata.count("artifact arity")? != scope.root.arity() {
        return Err(invalid("bounded scope/body arity"));
    }
    metadata.string("family fingerprint")?;
    metadata.string("context fingerprint")?;
    let ordering =
        OrderingPolicy::try_from_stable_id(metadata.string("artifact ordering identifier")?)
            .map_err(ArtifactError::from)?;
    ordering
        .require_arity(scope.root.arity())
        .map_err(ArtifactError::from)?;
    if !ordering.is_spired() {
        return Err(invalid("bounded scope ordering"));
    }
    metadata.finish()?;
    let mut rules = reader.child(rules);
    if super::source_port::decode_root(&mut rules, scope.root.arity())? != scope.root {
        return Err(invalid("bounded scope/body root"));
    }
    Ok(())
}
