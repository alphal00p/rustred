//! The on-disk p-library format `rustred.p-owner-library.v1` (owner: M2-T8b rr-package).
//!
//! A package is one directory, `p-library-<a>-<b>/` by convention, or the same
//! files embedded as `(relative name, bytes)` pairs:
//!
//! ```text
//! library.json                manifest: schema, identity, every p_versions() entry, the
//!                             Symbolica save format, the file list, presentations (index,
//!                             kind, family file, node key hex, slot momenta), DAG nodes,
//!                             edge maps, terminals, owners (class label, lane, presentation,
//!                             roles, payload file, terminals, lp_count, residuals), counts
//! families/p-families.json    the families file of the scope: the tops, in order
//! families/<name>.rrbin       one IntegralFamily per family name, see `encode_family_file`
//! topology/p-topologies.json  the reduction DAG, schema rustred.p-topologies.v1
//! census.rrbin                the census as bincode records, see `encode_census`
//! census.json                 the same census, schema rustred.p-census.v1 (readable)
//! owners/<class label>.rrbin  a Custom owner's payload: its candidate bundle, verbatim
//! layer/p-layer.json          the basis layer (M2-T9), when the library has one
//! ```
//!
//! The DTOs below mirror the public fields of the core types; no serde or
//! bincode derive is added to a core type. Everything is versioned by
//! constants, never by a digest: the schema string [`P_LIBRARY_SCHEMA`] names
//! the format of every file, the `versions` map holds every `p_versions()`
//! entry, and `symbolica_save_format` the format of the Symbolica state inside
//! every `.rrbin` file (Symbolica's state export header; 6 at the vendored pin).
//! A byte length per binary file guards against truncation; it is no identity.
//! The producer block is informational and never compared.
//!
//! [`P_LIBRARY_SCHEMA`]: rustred::propagator::P_LIBRARY_SCHEMA
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use rustred::family::{IntegralFamily, IntegralFamilyLimits, IntegralKey};
use rustred::persistence::{
    BinaryIoError, BinaryIoLimits, CoefficientTableBuilder, DecodedCoefficientTable,
    NativeFamilyRecord,
};
use rustred::propagator::census::{PCensus, PClassRecord, PSectorFate, SignedAffineLoopMap};
use rustred::propagator::class::OwnerClassKey;
use rustred::propagator::error::PError;
use rustred::propagator::lane::{
    BubbleRoles, CarpetRoles, DiamondRoles, LaneRoles, OwnerLane, TriangleRoles,
};
use rustred::propagator::library::{CustomOwnerMeta, PTerminal};
use rustred::propagator::presentation::{
    EdgeKind, EdgeMap, NodeRef, Presentation, PresentationKind, Slot, SlotIsp, SlotLine,
};
use rustred::propagator::topology::{
    Action, CarpetMatch, Dag, DagEdge, DagEdgeKind, DiaMatch, Fate, GraphKey, Ins, NodeInfo,
    P_TOPOLOGIES_SCHEMA, P_TOPOLOGY_VERSION, PTopo, TriMatch, topologies_json,
};
use rustred::propagator::{NodeIndex, OwnerClassId, PresentationId};
use serde::{Deserialize, Serialize};

/// The manifest.
pub const LIBRARY_FILE: &str = "library.json";
/// The families file of the scope.
pub const FAMILIES_FILE: &str = "families/p-families.json";
pub const TOPOLOGY_FILE: &str = "topology/p-topologies.json";
pub const CENSUS_FILE: &str = "census.rrbin";
pub const CENSUS_JSON_FILE: &str = "census.json";
pub const LAYER_FILE: &str = "layer/p-layer.json";

/// Name of the manifest entry holding the Symbolica save format.
pub const SYMBOLICA_SAVE_FORMAT: &str = "SYMBOLICA_SAVE_FORMAT";

const FAMILY_MAGIC: &[u8; 8] = b"RRPFAM1\n";
const CENSUS_MAGIC: &[u8; 8] = b"RRPCEN1\n";
/// Magic of a Symbolica state export (`symbolica::state::SYMBOLICA_MAGIC`).
const SYMBOLICA_STATE_MAGIC: u32 = 0x3787_1367;

/// `families/<name>.rrbin`.
pub fn family_file(name: &str) -> Result<String, PError> {
    file_name_part(name, "family name")?;
    Ok(format!("families/{name}.rrbin"))
}

/// `owners/<class label>.rrbin`.
pub fn owner_file(label: &str) -> Result<String, PError> {
    file_name_part(label, "class label")?;
    Ok(format!("owners/{label}.rrbin"))
}

/// Longest family name or class label in a file name: `<name>.rrbin` stays
/// within the usual 255-byte limit of a path component.
const MAX_NAME_BYTES: usize = 240;

/// A name that is one plain path component on every platform.
fn file_name_part(name: &str, what: &str) -> Result<(), PError> {
    let plain = !name.is_empty()
        && name.len() <= MAX_NAME_BYTES
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'));
    if plain {
        Ok(())
    } else {
        Err(malformed(format!(
            "the {what} {name:?} is no plain file name"
        )))
    }
}

pub(crate) fn malformed(detail: impl Into<String>) -> PError {
    PError::MalformedLibrary(detail.into())
}

fn binary(what: &str) -> impl Fn(BinaryIoError) -> PError + '_ {
    move |error| malformed(format!("{what}: {error}"))
}

// ---------------------------------------------------------------------------
// Symbolica save format
// ---------------------------------------------------------------------------

/// The format version in the header of a Symbolica state export, if `state`
/// starts with one.
pub fn state_save_format(state: &[u8]) -> Option<u16> {
    let magic = u32::from_le_bytes(state.get(..4)?.try_into().ok()?);
    let version = u16::from_le_bytes(state.get(4..6)?.try_into().ok()?);
    (magic == SYMBOLICA_STATE_MAGIC).then_some(version)
}

/// The Symbolica save format this build writes and reads: the header of an
/// (empty) state export, so it follows the linked Symbolica.
pub fn symbolica_save_format() -> Result<u16, PError> {
    static FORMAT: OnceLock<Result<u16, String>> = OnceLock::new();
    FORMAT
        .get_or_init(|| {
            let table = CoefficientTableBuilder::new(BinaryIoLimits::default())
                .finish()
                .map_err(|error| error.to_string())?;
            state_save_format(&table.state)
                .ok_or_else(|| "a state export without the Symbolica header".to_owned())
        })
        .clone()
        .map_err(|error| malformed(format!("Symbolica save format: {error}")))
}

/// `found` is the save format of this build, else the typed refusal.
pub fn check_save_format(found: u16, what: &str) -> Result<(), PError> {
    let expected = symbolica_save_format()?;
    if found == expected {
        return Ok(());
    }
    Err(PError::VersionMismatch {
        name: format!("{SYMBOLICA_SAVE_FORMAT} ({what})"),
        found: found.to_string(),
        expected: expected.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Length-framed sections
// ---------------------------------------------------------------------------

fn push_section(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

/// `count` length-framed sections after `magic`, nothing after them.
fn sections<'a>(
    bytes: &'a [u8],
    magic: &[u8; 8],
    count: usize,
    what: &str,
) -> Result<Vec<&'a [u8]>, PError> {
    let fail = |detail: &str| malformed(format!("{what}: {detail}"));
    let mut rest = bytes
        .strip_prefix(magic.as_slice())
        .ok_or_else(|| fail("wrong magic, not a file of this kind"))?;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let (length, tail) = rest
            .split_first_chunk::<8>()
            .ok_or_else(|| fail("truncated section length"))?;
        let length = usize::try_from(u64::from_le_bytes(*length))
            .map_err(|_| fail("section length exceeds the address space"))?;
        if tail.len() < length {
            return Err(fail("truncated section"));
        }
        let (section, tail) = tail.split_at(length);
        out.push(section);
        rest = tail;
    }
    if !rest.is_empty() {
        return Err(fail("trailing bytes"));
    }
    Ok(out)
}

fn bincode_encode<T: bincode::Encode>(value: &T, what: &str) -> Result<Vec<u8>, PError> {
    bincode::encode_to_vec(value, bincode::config::standard())
        .map_err(|error| malformed(format!("{what}: {error}")))
}

fn bincode_decode<T: bincode::Decode<()>>(bytes: &[u8], what: &str) -> Result<T, PError> {
    let (value, used) = bincode::decode_from_slice(bytes, bincode::config::standard())
        .map_err(|error| malformed(format!("{what}: {error}")))?;
    if used != bytes.len() {
        return Err(malformed(format!("{what}: trailing bytes")));
    }
    Ok(value)
}

// ---------------------------------------------------------------------------
// families/<name>.rrbin
// ---------------------------------------------------------------------------

/// `families/<name>.rrbin`: magic `RRPFAM1\n`, then three length-framed
/// sections: the Symbolica state, the coefficient atoms and the bincode
/// `NativeFamilyRecord` (rustred persistence, `persistence::family`). Bytes
/// depend on the Symbolica symbol table of the writing process: the same
/// writer in a fresh process writes the same bytes.
pub fn encode_family_file(family: &IntegralFamily) -> Result<Vec<u8>, PError> {
    let what = format!("family {}", family.name());
    let mut table = CoefficientTableBuilder::new(BinaryIoLimits::default());
    let record = NativeFamilyRecord::from_family(family, &mut table).map_err(binary(&what))?;
    let table = table.finish().map_err(binary(&what))?;
    let record = bincode_encode(&record, &what)?;
    let mut out = FAMILY_MAGIC.to_vec();
    push_section(&mut out, &table.state);
    push_section(&mut out, &table.atoms);
    push_section(&mut out, &record);
    Ok(out)
}

/// The Symbolica save format of a family file.
pub fn family_file_save_format(bytes: &[u8], what: &str) -> Result<u16, PError> {
    let parts = sections(bytes, FAMILY_MAGIC, 3, what)?;
    state_save_format(parts[0])
        .ok_or_else(|| malformed(format!("{what}: no Symbolica state header")))
}

/// Inverse of [`encode_family_file`]; the save format must be this build's.
pub fn decode_family_file(bytes: &[u8], what: &str) -> Result<IntegralFamily, PError> {
    let parts = sections(bytes, FAMILY_MAGIC, 3, what)?;
    let format = state_save_format(parts[0])
        .ok_or_else(|| malformed(format!("{what}: no Symbolica state header")))?;
    check_save_format(format, what)?;
    let limits = BinaryIoLimits::default();
    let table = DecodedCoefficientTable::import_generated(parts[0], parts[1], limits)
        .map_err(binary(what))?;
    let record: NativeFamilyRecord = bincode_decode(parts[2], what)?;
    record
        .to_family(&table, IntegralFamilyLimits::default(), limits)
        .map_err(binary(what))
}

// ---------------------------------------------------------------------------
// census.rrbin
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
struct CensusRecord {
    classes: Vec<ClassRecord>,
    sectors: Vec<SectorRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
struct ClassRecord {
    id: u64,
    /// `OwnerClassKey::canonical`; its first three bytes are loops, lines, eps lines.
    canonical: Vec<u8>,
    owner_presentation: u32,
    owner_support: u64,
    node: Option<u32>,
    nodes: Vec<u32>,
    t: u8,
    eps_lines: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
struct SectorRecord {
    presentation: u32,
    support: u64,
    /// `None`: a zero sector.
    route: Option<RouteRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
struct RouteRecord {
    class: u64,
    loops_in: u8,
    loops_out: u8,
    loop_linear: Vec<i64>,
    loop_external: Vec<i64>,
    external_sign: i8,
}

/// `census.rrbin`: magic `RRPCEN1\n` and one length-framed section, the
/// bincode records of every class and sector.
pub fn encode_census(census: &PCensus) -> Result<Vec<u8>, PError> {
    let record = CensusRecord {
        classes: census
            .classes
            .iter()
            .map(|(id, record)| ClassRecord {
                id: id.0,
                canonical: record.key.canonical.to_vec(),
                owner_presentation: record.owner.0.0,
                owner_support: record.owner.1,
                node: record.node.map(|node| node.0),
                nodes: record.nodes.iter().map(|node| node.0).collect(),
                t: record.t,
                eps_lines: record.eps_lines,
            })
            .collect(),
        sectors: census
            .sectors
            .iter()
            .map(|(&(presentation, support), fate)| SectorRecord {
                presentation: presentation.0,
                support,
                route: match fate {
                    PSectorFate::Zero => None,
                    PSectorFate::Owner { class, map } => Some(RouteRecord {
                        class: class.0,
                        loops_in: map.loops_in,
                        loops_out: map.loops_out,
                        loop_linear: map.loop_linear.to_vec(),
                        loop_external: map.loop_external.to_vec(),
                        external_sign: map.external_sign,
                    }),
                },
            })
            .collect(),
    };
    let mut out = CENSUS_MAGIC.to_vec();
    push_section(&mut out, &bincode_encode(&record, CENSUS_FILE)?);
    Ok(out)
}

/// Inverse of [`encode_census`]: every class under the id of its canonical
/// bytes, no class or sector twice.
pub fn decode_census(bytes: &[u8]) -> Result<PCensus, PError> {
    let parts = sections(bytes, CENSUS_MAGIC, 1, CENSUS_FILE)?;
    let record: CensusRecord = bincode_decode(parts[0], CENSUS_FILE)?;
    let fail = |detail: String| malformed(format!("{CENSUS_FILE}: {detail}"));
    let mut census = PCensus::default();
    for class in record.classes {
        let &[loops, lines, eps_lines, ..] = class.canonical.as_slice() else {
            return Err(fail(format!("class {:016x}: canonical bytes", class.id)));
        };
        let key = OwnerClassKey {
            loops,
            lines,
            eps_lines,
            canonical: class.canonical.into(),
        };
        if key.id() != OwnerClassId(class.id) {
            return Err(fail(format!(
                "class {:016x} holds the key {}",
                class.id,
                key.label()
            )));
        }
        let record = PClassRecord {
            key,
            owner: (
                PresentationId(class.owner_presentation),
                class.owner_support,
            ),
            node: class.node.map(NodeIndex),
            nodes: class.nodes.into_iter().map(NodeIndex).collect(),
            t: class.t,
            eps_lines: class.eps_lines,
        };
        if census
            .classes
            .insert(OwnerClassId(class.id), record)
            .is_some()
        {
            return Err(fail(format!("class {:016x} appears twice", class.id)));
        }
    }
    for sector in record.sectors {
        let fate = match sector.route {
            None => PSectorFate::Zero,
            Some(route) => PSectorFate::Owner {
                class: OwnerClassId(route.class),
                map: SignedAffineLoopMap {
                    loops_in: route.loops_in,
                    loops_out: route.loops_out,
                    loop_linear: route.loop_linear.into(),
                    loop_external: route.loop_external.into(),
                    external_sign: route.external_sign,
                },
            },
        };
        let key = (PresentationId(sector.presentation), sector.support);
        if census.sectors.insert(key, fate).is_some() {
            return Err(fail(format!(
                "sector ({}, {:#x}) appears twice",
                sector.presentation, sector.support
            )));
        }
    }
    Ok(census)
}

// ---------------------------------------------------------------------------
// topology/p-topologies.json
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TopologiesFile {
    schema: String,
    p_topology_version: u32,
    nodes: Vec<TopologyNode>,
    edges: Vec<TopologyEdge>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TopologyNode {
    index: u32,
    key: String,
    topology: TopologyGraph,
    loops: u8,
    t: u8,
    eps_lines: u8,
    action: TopologyAction,
    series_parallel: bool,
    children: Vec<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TopologyGraph {
    n: u8,
    s: u8,
    t: u8,
    lines: Vec<[u8; 3]>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum TopologyAction {
    Trivial,
    Insertion {
        keep: u8,
        drop: u8,
    },
    Carpet {
        r: u8,
        third: Option<u8>,
        blob: Vec<u8>,
        blob_loops: u8,
        blob_eps: u8,
    },
    Triangle {
        a1: u8,
        a2: u8,
        b: u8,
        c1: u8,
        c2: u8,
        apex: u8,
        v_a: u8,
        v_b: u8,
    },
    Diamond {
        a: Vec<u8>,
        b: Vec<u8>,
        c: Vec<u8>,
        spect: Vec<u8>,
        top: u8,
        bottom: u8,
    },
    Custom,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TopologyEdge {
    parent: u32,
    child: u32,
    kind: String,
    #[serde(default)]
    line: Option<u8>,
    fates: Vec<Fate>,
}

/// `topology/p-topologies.json`: [`topologies_json`], pretty, with a final newline.
pub fn encode_topology(dag: &Dag) -> Result<Vec<u8>, PError> {
    pretty_json(&topologies_json(dag), TOPOLOGY_FILE)
}

/// The DAG of [`encode_topology`]: the schema and topology version of this
/// build, nodes in order with valid graphs, and nothing that
/// [`topologies_json`] would not write back unchanged. Keys are read, never
/// recomputed (plan M2 section 3.1).
pub fn decode_topology(bytes: &[u8]) -> Result<Dag, PError> {
    let fail = |detail: String| malformed(format!("{TOPOLOGY_FILE}: {detail}"));
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|error| fail(error.to_string()))?;
    let schema = value.get("schema").and_then(serde_json::Value::as_str);
    if schema != Some(P_TOPOLOGIES_SCHEMA) {
        return Err(PError::VersionMismatch {
            name: "P_TOPOLOGIES_SCHEMA".to_owned(),
            found: schema.unwrap_or("(absent)").to_owned(),
            expected: P_TOPOLOGIES_SCHEMA.to_owned(),
        });
    }
    let version = value
        .get("p_topology_version")
        .and_then(serde_json::Value::as_u64);
    if version != Some(u64::from(P_TOPOLOGY_VERSION)) {
        return Err(PError::VersionMismatch {
            name: "P_TOPOLOGY_VERSION".to_owned(),
            found: version.map_or("(absent)".to_owned(), |v| v.to_string()),
            expected: P_TOPOLOGY_VERSION.to_string(),
        });
    }
    let file: TopologiesFile =
        serde_json::from_value(value.clone()).map_err(|error| fail(error.to_string()))?;
    debug_assert_eq!(file.schema, P_TOPOLOGIES_SCHEMA);
    debug_assert_eq!(file.p_topology_version, P_TOPOLOGY_VERSION);
    let mut dag = Dag::default();
    for (position, node) in file.nodes.into_iter().enumerate() {
        if node.index as usize != position {
            return Err(fail(format!("node {} at position {position}", node.index)));
        }
        let key = GraphKey::from_hex(&node.key)
            .ok_or_else(|| fail(format!("node {}: key {:?}", node.index, node.key)))?;
        let graph = node.topology;
        let vertex = |v: u8| v < graph.n;
        if !vertex(graph.s)
            || !vertex(graph.t)
            || !graph.lines.iter().all(|&[u, v, _]| vertex(u) && vertex(v))
        {
            return Err(fail(format!("node {}: vertex out of range", node.index)));
        }
        let topo = PTopo {
            n: graph.n,
            s: graph.s,
            t: graph.t,
            lines: graph.lines.iter().map(|&[u, v, e]| (u, v, e)).collect(),
        };
        dag.nodes.push(NodeInfo {
            index: node.index,
            key,
            topo,
            loops: node.loops,
            t: node.t,
            eps_lines: node.eps_lines,
            action: node.action.into_action(),
            series_parallel: node.series_parallel,
            children: node.children,
        });
    }
    let nodes = dag.nodes.len();
    for edge in file.edges {
        let kind = match (edge.kind.as_str(), edge.line) {
            ("rule", None) => DagEdgeKind::Rule,
            ("contract", Some(line)) => DagEdgeKind::Contract(line),
            (kind, line) => return Err(fail(format!("edge kind {kind:?} with line {line:?}"))),
        };
        if edge.parent as usize >= nodes || edge.child as usize >= nodes {
            return Err(fail(format!("edge {} -> {}", edge.parent, edge.child)));
        }
        dag.edges.push(DagEdge {
            parent: edge.parent,
            child: edge.child,
            kind,
            fates: edge.fates,
        });
    }
    if dag
        .nodes
        .iter()
        .any(|node| node.children.iter().any(|&child| child as usize >= nodes))
    {
        return Err(fail("a child index out of range".to_owned()));
    }
    if topologies_json(&dag) != value {
        return Err(fail("the file does not round-trip".to_owned()));
    }
    Ok(dag)
}

impl TopologyAction {
    fn into_action(self) -> Action {
        match self {
            Self::Trivial => Action::Trivial,
            Self::Insertion { keep, drop } => Action::Insertion(Ins { keep, drop }),
            Self::Carpet {
                r,
                third,
                blob,
                blob_loops,
                blob_eps,
            } => Action::Carpet(CarpetMatch {
                r,
                third,
                blob: blob.into_iter().collect(),
                blob_loops,
                blob_eps,
            }),
            Self::Triangle {
                a1,
                a2,
                b,
                c1,
                c2,
                apex,
                v_a,
                v_b,
            } => Action::Triangle(TriMatch {
                a1,
                a2,
                b,
                c1,
                c2,
                apex,
                v_a,
                v_b,
            }),
            Self::Diamond {
                a,
                b,
                c,
                spect,
                top,
                bottom,
            } => Action::Diamond(DiaMatch {
                a: a.into_iter().collect(),
                b: b.into_iter().collect(),
                c: c.into_iter().collect(),
                spect: spect.into_iter().collect(),
                top,
                bottom,
            }),
            Self::Custom => Action::Custom,
        }
    }
}

/// Pretty JSON with a final newline.
pub fn pretty_json<T: Serialize>(value: &T, what: &str) -> Result<Vec<u8>, PError> {
    let mut bytes =
        serde_json::to_vec_pretty(value).map_err(|error| malformed(format!("{what}: {error}")))?;
    bytes.push(b'\n');
    Ok(bytes)
}

// ---------------------------------------------------------------------------
// library.json
// ---------------------------------------------------------------------------

/// The manifest `library.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LibraryManifest {
    /// [`P_LIBRARY_SCHEMA`](rustred::propagator::P_LIBRARY_SCHEMA): the format of every file.
    pub schema: String,
    /// `PLibrary::identity()`.
    pub identity: String,
    /// First and last loop count of the scope.
    pub scope: [u8; 2],
    /// Every `p_versions()` entry by name.
    pub versions: BTreeMap<String, String>,
    /// The Symbolica save format of the state inside every `.rrbin` file.
    pub symbolica_save_format: u16,
    /// Informational only, never compared.
    pub producer: ProducerRecord,
    pub files: FilesRecord,
    pub presentations: Vec<PresentationRecord>,
    /// What assembly made of each DAG node, in DAG order (informational,
    /// checked against the presentations).
    pub nodes: Vec<NodeRecord>,
    pub edge_maps: Vec<EdgeMapRecord>,
    /// In terminal-id order; the first is `unit`.
    pub terminals: Vec<TerminalRecord>,
    /// In class-id order.
    pub owners: Vec<OwnerRecord>,
    pub counts: CountsRecord,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProducerRecord {
    pub name: String,
    pub rustred_version: String,
    pub symbolica_version: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilesRecord {
    pub families: String,
    /// One file per family name, sorted by name.
    pub family_files: Vec<FamilyFileRecord>,
    pub topology: String,
    pub census: String,
    pub census_json: String,
    pub layer: Option<String>,
    /// p artefact v2: the rule ordering file (`ordering.json`), if any
    /// (`super::ordering`); absent in older packages and in `rustred
    /// p-package` output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ordering: Option<String>,
    /// p artefact v2: saved route verifications (`routes.rrbin`,
    /// `rustred::propagator::saved_routes`), if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routes: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyFileRecord {
    /// The family name.
    pub name: String,
    pub file: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PayloadRecord {
    pub file: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationRecord {
    pub index: u32,
    pub kind: KindRecord,
    pub loops: u8,
    /// The family file (`families/<family name>.rrbin`).
    pub family: String,
    pub node: Option<NodeRefRecord>,
    pub slots: Vec<SlotRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum KindRecord {
    Top { name: String },
    Induced { top: u32, mask: u64 },
    InsertionAdapted,
    CarpetAdapted,
    TriangleAdapted,
    DiamondAdapted,
    SchemeAdapted,
    EpsNode,
    Caller,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeRefRecord {
    pub index: u32,
    /// Graph key bytes, lower-case hex.
    pub key: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum SlotRecord {
    Line { momentum: Vec<i64>, eps_units: u8 },
    Isp { left: Vec<i64>, right: Vec<i64> },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeRecord {
    pub index: u32,
    pub lane: LaneRecord,
    pub trivial: bool,
    pub presentation: Option<u32>,
    pub fallback: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaneRecord {
    Zero,
    SeriesParallel,
    Insertion,
    Carpet,
    Triangle,
    Diamond,
    Scheme,
    Custom,
    Product,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum RolesRecord {
    None,
    Insertion {
        lines: [u16; 2],
        loop_index: u8,
    },
    Carpet {
        r: u16,
        third: Option<u16>,
        outer_loop: u8,
    },
    Triangle {
        b: u16,
        a: [u16; 2],
        c: [u16; 2],
        k_loop: u8,
    },
    /// M3-T5b: path `i` is `a[i]`, `b[i]`, `c[i]`.
    Diamond {
        a: Vec<u16>,
        b: Vec<u16>,
        c: Vec<u16>,
        spectator: Option<u16>,
    },
    /// M5-R0 A1.
    Scheme {
        scheme: u8,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoopMapRecord {
    pub loops_in: u8,
    pub loops_out: u8,
    pub loop_linear: Vec<i64>,
    pub loop_external: Vec<i64>,
    pub external_sign: i8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKindRecord {
    Insertion,
    Carpet,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EdgeMapRecord {
    pub parent: u32,
    pub child: u32,
    pub kind: EdgeKindRecord,
    pub integrated_loop: u8,
    pub outer_map: LoopMapRecord,
    pub line_slots: Vec<Option<u16>>,
    pub merged_slot: Option<u16>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum TerminalRecord {
    Unit,
    /// A residual of the owner with this class label, at `key`.
    Residual {
        owner: String,
        key: Vec<i64>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomRecord {
    pub lp_count: u32,
    pub residuals: u32,
    pub two_sided_axes: u64,
    pub charts: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerRecord {
    /// Class label `L<l>T<t>-<id hex>`.
    pub class: String,
    pub lane: LaneRecord,
    pub presentation: u32,
    pub node: Option<u32>,
    pub roles: RolesRecord,
    pub edge_maps: Vec<usize>,
    pub terminals: Vec<u32>,
    pub payload: Option<PayloadRecord>,
    pub custom: Option<CustomRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CountsRecord {
    pub presentations: usize,
    pub tops: usize,
    pub dag_nodes: usize,
    pub dag_edges: usize,
    pub classes: usize,
    pub sectors: usize,
    pub zero_sectors: usize,
    pub edge_maps: usize,
    pub terminals: usize,
    /// Owners per lane.
    pub owners: BTreeMap<LaneRecord, usize>,
}

impl LaneRecord {
    pub fn of(lane: OwnerLane) -> Self {
        match lane {
            OwnerLane::Zero => Self::Zero,
            OwnerLane::SeriesParallel => Self::SeriesParallel,
            OwnerLane::Insertion => Self::Insertion,
            OwnerLane::Carpet => Self::Carpet,
            OwnerLane::Triangle => Self::Triangle,
            OwnerLane::Diamond => Self::Diamond,
            OwnerLane::Scheme => Self::Scheme,
            OwnerLane::Custom => Self::Custom,
            OwnerLane::Product => Self::Product,
        }
    }

    pub fn lane(self) -> OwnerLane {
        match self {
            Self::Zero => OwnerLane::Zero,
            Self::SeriesParallel => OwnerLane::SeriesParallel,
            Self::Insertion => OwnerLane::Insertion,
            Self::Carpet => OwnerLane::Carpet,
            Self::Triangle => OwnerLane::Triangle,
            Self::Diamond => OwnerLane::Diamond,
            Self::Scheme => OwnerLane::Scheme,
            Self::Custom => OwnerLane::Custom,
            Self::Product => OwnerLane::Product,
        }
    }
}

impl RolesRecord {
    pub fn of(roles: &LaneRoles) -> Self {
        match *roles {
            LaneRoles::None => Self::None,
            LaneRoles::Insertion(BubbleRoles { lines, loop_index }) => {
                Self::Insertion { lines, loop_index }
            }
            LaneRoles::Carpet(CarpetRoles {
                r,
                third,
                outer_loop,
            }) => Self::Carpet {
                r,
                third,
                outer_loop,
            },
            LaneRoles::Triangle(TriangleRoles { b, a, c, k_loop }) => {
                Self::Triangle { b, a, c, k_loop }
            }
            LaneRoles::Diamond(roles) => Self::Diamond {
                a: roles.a().to_vec(),
                b: roles.b().to_vec(),
                c: roles.c().to_vec(),
                spectator: roles.spectator,
            },
            LaneRoles::Scheme(roles) => Self::Scheme {
                scheme: roles.scheme,
            },
        }
    }

    pub fn roles(&self) -> LaneRoles {
        match *self {
            Self::None => LaneRoles::None,
            Self::Insertion { lines, loop_index } => {
                LaneRoles::Insertion(BubbleRoles { lines, loop_index })
            }
            Self::Carpet {
                r,
                third,
                outer_loop,
            } => LaneRoles::Carpet(CarpetRoles {
                r,
                third,
                outer_loop,
            }),
            Self::Triangle { b, a, c, k_loop } => {
                LaneRoles::Triangle(TriangleRoles { b, a, c, k_loop })
            }
            // A diamond record of no diamond (path counts differ, too few or too
            // many paths) reads as no roles: its owner then has no program and
            // the library is refused.
            Self::Diamond {
                ref a,
                ref b,
                ref c,
                spectator,
            } => DiamondRoles::new(a, b, c, spectator).map_or(LaneRoles::None, LaneRoles::Diamond),
            Self::Scheme { scheme } => {
                LaneRoles::Scheme(rustred::propagator::lane::SchemeRoles { scheme })
            }
        }
    }
}

impl KindRecord {
    pub fn of(kind: &PresentationKind) -> Self {
        match kind {
            PresentationKind::Top { name } => Self::Top { name: name.clone() },
            PresentationKind::Induced { top, mask } => Self::Induced {
                top: top.0,
                mask: *mask,
            },
            PresentationKind::InsertionAdapted => Self::InsertionAdapted,
            PresentationKind::CarpetAdapted => Self::CarpetAdapted,
            PresentationKind::TriangleAdapted => Self::TriangleAdapted,
            PresentationKind::DiamondAdapted => Self::DiamondAdapted,
            PresentationKind::SchemeAdapted => Self::SchemeAdapted,
            PresentationKind::EpsNode => Self::EpsNode,
            PresentationKind::Caller => Self::Caller,
        }
    }

    pub fn kind(&self) -> PresentationKind {
        match self {
            Self::Top { name } => PresentationKind::Top { name: name.clone() },
            Self::Induced { top, mask } => PresentationKind::Induced {
                top: PresentationId(*top),
                mask: *mask,
            },
            Self::InsertionAdapted => PresentationKind::InsertionAdapted,
            Self::CarpetAdapted => PresentationKind::CarpetAdapted,
            Self::TriangleAdapted => PresentationKind::TriangleAdapted,
            Self::DiamondAdapted => PresentationKind::DiamondAdapted,
            Self::SchemeAdapted => PresentationKind::SchemeAdapted,
            Self::EpsNode => PresentationKind::EpsNode,
            Self::Caller => PresentationKind::Caller,
        }
    }
}

impl SlotRecord {
    pub fn of(slot: &Slot) -> Self {
        match slot {
            Slot::Line(line) => Self::Line {
                momentum: line.momentum.clone(),
                eps_units: line.eps_units,
            },
            Slot::Isp(isp) => Self::Isp {
                left: isp.left.clone(),
                right: isp.right.clone(),
            },
        }
    }

    pub fn slot(&self) -> Slot {
        match self {
            Self::Line {
                momentum,
                eps_units,
            } => Slot::Line(SlotLine {
                momentum: momentum.clone(),
                eps_units: *eps_units,
            }),
            Self::Isp { left, right } => Slot::Isp(SlotIsp {
                left: left.clone(),
                right: right.clone(),
            }),
        }
    }
}

impl NodeRefRecord {
    pub fn of(node: &NodeRef) -> Self {
        Self {
            index: node.index.0,
            key: GraphKey(Arc::clone(&node.graph_key)).to_hex(),
        }
    }

    pub fn node(&self) -> Result<NodeRef, PError> {
        let key = GraphKey::from_hex(&self.key)
            .ok_or_else(|| malformed(format!("node {}: key {:?}", self.index, self.key)))?;
        Ok(NodeRef {
            index: NodeIndex(self.index),
            graph_key: key.0,
        })
    }
}

impl PresentationRecord {
    pub fn of(presentation: &Presentation, family: String) -> Self {
        Self {
            index: presentation.id.0,
            kind: KindRecord::of(&presentation.kind),
            loops: presentation.loops,
            family,
            node: presentation.node.as_ref().map(NodeRefRecord::of),
            slots: presentation.slots.iter().map(SlotRecord::of).collect(),
        }
    }

    /// The presentation of this record over `family`; not yet validated.
    pub fn presentation(&self, family: Arc<IntegralFamily>) -> Result<Presentation, PError> {
        Ok(Presentation {
            id: PresentationId(self.index),
            kind: self.kind.kind(),
            loops: self.loops,
            slots: self.slots.iter().map(SlotRecord::slot).collect(),
            family,
            node: self.node.as_ref().map(NodeRefRecord::node).transpose()?,
        })
    }
}

impl LoopMapRecord {
    pub fn of(map: &SignedAffineLoopMap) -> Self {
        Self {
            loops_in: map.loops_in,
            loops_out: map.loops_out,
            loop_linear: map.loop_linear.to_vec(),
            loop_external: map.loop_external.to_vec(),
            external_sign: map.external_sign,
        }
    }

    pub fn map(&self) -> SignedAffineLoopMap {
        SignedAffineLoopMap {
            loops_in: self.loops_in,
            loops_out: self.loops_out,
            loop_linear: self.loop_linear.clone().into(),
            loop_external: self.loop_external.clone().into(),
            external_sign: self.external_sign,
        }
    }
}

impl EdgeMapRecord {
    pub fn of(map: &EdgeMap) -> Self {
        Self {
            parent: map.parent.0,
            child: map.child.0,
            kind: match map.kind {
                EdgeKind::Insertion => EdgeKindRecord::Insertion,
                EdgeKind::Carpet => EdgeKindRecord::Carpet,
            },
            integrated_loop: map.integrated_loop,
            outer_map: LoopMapRecord::of(&map.outer_map),
            line_slots: map.line_slots.clone(),
            merged_slot: map.merged_slot,
        }
    }

    pub fn edge_map(&self) -> EdgeMap {
        EdgeMap {
            parent: PresentationId(self.parent),
            child: PresentationId(self.child),
            kind: match self.kind {
                EdgeKindRecord::Insertion => EdgeKind::Insertion,
                EdgeKindRecord::Carpet => EdgeKind::Carpet,
            },
            integrated_loop: self.integrated_loop,
            outer_map: self.outer_map.map(),
            line_slots: self.line_slots.clone(),
            merged_slot: self.merged_slot,
        }
    }
}

impl TerminalRecord {
    /// `labels`: class id to class label.
    pub fn of(
        terminal: &PTerminal,
        labels: &BTreeMap<OwnerClassId, String>,
    ) -> Result<Self, PError> {
        Ok(match terminal {
            PTerminal::Unit => Self::Unit,
            PTerminal::Residual { owner, key } => Self::Residual {
                owner: labels.get(owner).cloned().ok_or_else(|| {
                    malformed(format!("a residual of the unknown class {:016x}", owner.0))
                })?,
                key: key.powers().to_vec(),
            },
        })
    }

    /// `classes`: class label to class id.
    pub fn terminal(&self, classes: &BTreeMap<String, OwnerClassId>) -> Result<PTerminal, PError> {
        Ok(match self {
            Self::Unit => PTerminal::Unit,
            Self::Residual { owner, key } => PTerminal::Residual {
                owner: *classes
                    .get(owner)
                    .ok_or_else(|| malformed(format!("a residual of the unknown class {owner}")))?,
                key: IntegralKey::try_new(key.iter().copied())
                    .map_err(|error| malformed(format!("a residual key of {owner}: {error}")))?,
            },
        })
    }
}

impl CustomRecord {
    pub fn of(meta: &CustomOwnerMeta) -> Self {
        Self {
            lp_count: meta.lp_count,
            residuals: meta.residuals,
            two_sided_axes: meta.two_sided_axes,
            charts: meta.charts.clone(),
        }
    }

    pub fn meta(&self) -> CustomOwnerMeta {
        CustomOwnerMeta {
            lp_count: self.lp_count,
            residuals: self.residuals,
            two_sided_axes: self.two_sided_axes,
            charts: self.charts.clone(),
        }
    }
}
