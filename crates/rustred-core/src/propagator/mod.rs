//! Massless propagator-type ("p") integral library: one library program per
//! release scope over vakint's families and Forcer's rule cascade (plan M2
//! sections 3-4). Every module is registered here once; each file names its
//! owner task.

// Interfaces and their self-contained implementations (M2-I0; class and
// census bodies: M2-T1 and M2-T2; library bodies: M2-T6; programs body: M2-T8a).
pub mod census;
pub mod class;
pub mod error;
pub mod families;
pub mod lane;
pub mod library;
pub(crate) mod orbit;
pub mod path_census;
pub mod presentation;
pub mod programs;
pub mod saved_orbits;
pub mod saved_routes;
pub mod step;
pub mod tag;
pub mod terminal;

// Ordered adapter and test support (M2-T6).
pub mod engine;
pub mod testing;

// Kernels and the Insertion, SeriesParallel and Carpet lanes (M2-T4).
pub mod carpet;
pub mod insertion;
pub mod kernel;
pub mod series_parallel;
// rr-alloc: per-thread step coefficient memos (RUSTRED_P_STEP_MEMO).
pub(crate) mod step_memo;

// Lane decision and presentations (M2-T3).
pub mod decide;
pub mod present;

// Triangle lane (M2-T5).
pub mod triangle;

// Diamond lane (M3-T5b).
pub mod diamond;

// Scheme lane (M5 A1 schemes): Forcer's hand schemes as data-driven templates.
pub mod scheme;

// Custom owners (M2-T7c).
pub mod custom;
// The candidate bundle's wire records, reader and restore (M3-T7d).
pub mod bundle_wire;
// table_dump.v1 rule tables as Custom payloads (p5l-converter).
pub mod table_import;

// Descent certificate (M2-T8a).
pub mod descent;

// Basis layer onto the Forcer masters (M2-T9).
pub mod basis;

// Static-score route-map choice (M5 maps, p5l-maps-rs).
pub mod route_choice;

// The topology crate: signatures below name its types, and the app crate
// reaches them through this path. Its version enters `p_versions()`.
pub use rustred_topology as topology;
pub use rustred_topology::P_TOPOLOGY_VERSION;

/// Generated content and the meaning of a recorded step.
pub const P_OWNER_LIBRARY_SEMANTICS: &str = "rustred-p-owner-library-2026-11-05";
pub const P_CLASS_KEY_VERSION: u32 = 1;
/// 2 (M5 combined 4L package, route maps (a); p5l-maps-rs in M5 batch 2): a
/// package assembly stores, for every routed sector, the static-score route map
/// `T M` (the first route composed with an automorphism of the owner sector,
/// fewest routed terms by a traffic-free static score, verified by
/// `class::verify_route`; `census::PRouteMaps::StaticScore`, chosen inside the
/// assembly since batch 2); the engine follows the stored maps strictly.
pub const P_CENSUS_VERSION: u32 = 2;
/// The `present` and `edge_map` algorithms. 2 (M5 combined 4L package, ISP
/// choice (b)): the embedded `present::P_ISP_PREFERENCES` re-complete the ISPs
/// of three scope 1-4 Insertion owners.
pub const P_PRESENTATION_VERSION: u32 = 2;
/// The content of `data/p/p-families.v1.json`.
pub const P_FAMILIES_VERSION: u32 = 1;
pub const P_GAMMA_TAG_VERSION: u32 = 1;
pub const P_GAMMA_TAG_MAX_M: i8 = 8;
pub const P_GAMMA_TAG_WIDTH: usize = 16;
pub const P_INSERTION_KERNEL_VERSION: u32 = 1;
pub const P_CARPET_KERNEL_VERSION: u32 = 1;
pub const P_TRIANGLE_TEMPLATE_VERSION: u32 = 1;
/// The Diamond template and its lane (M3-T5b).
pub const P_DIAMOND_TEMPLATE_VERSION: u32 = 1;
/// The Scheme lane (M5 A1 schemes): the meaning of the embedded scheme set
/// (`scheme::EMBEDDED_SCHEMES`: rules, their stored order, order key and
/// verification). Bump it with every change of an embedded scheme file.
/// 2 (M5 combined 4L package, user decision 1): no1, haha and bubu are on by
/// default (`"default": true`), so a library assembled without
/// `RUSTRED_P_SCHEMES` puts these three owners on the Scheme lane; the
/// rank-gated Custom no1 of B2 is the fallback (`RUSTRED_P_SCHEMES=none`).
/// 3 (M5 batch 2, m5-schemes-2): permutation rules (`"kind": "permutation"`),
/// `eq:`/`distinct` conditions and `"dispatch": "terms"`; no2 is embedded and on
/// by default (Forcer's SchemeZno2 with its three relabelling rules); haha and
/// bubu carry their parametric certificates (45/51, 36/99). no6 is not embedded
/// (keys outside Forcer's reach need a fallback): its Custom owner stays.
/// M5 no6: no6 is embedded with the shipped Custom no6 rules as the fallback
/// of the keys outside its reach (`"fallback": "custom"`). 4 (M5 batch 2,
/// coordinator 15:38Z): no6 is on by default (Scheme lane: no1, haha, bubu,
/// no2, no6).
pub const P_SCHEME_TEMPLATE_VERSION: u32 = 4;
/// Custom-owner generation options and the false-knock-out guard. 2 (M5 B2):
/// a payload order may be rank-gated (an ISP-rank row before the support,
/// `descent::is_rank_gated`), and the scope 1-4 package ships such owners.
pub const P_CUSTOM_OWNER_VERSION: u32 = 2;
/// p artefact v2 (notes/artifact-ordering.SPEC.md): the format and semantics
/// of an explicit rule ordering file (`ordering.json`, schema
/// [`P_ORDERING_SCHEMA`]). Not a `p_versions()` entry: a library without an
/// ordering keeps its identity; one with an ordering appends
/// `,P_ORDERING_VERSION=<v>:<heuristic>` (`PLibrary::identity`).
pub const P_ORDERING_VERSION: u32 = 1;
/// The content of `data/p/p-families-5L.v1.json` (the 64 five-loop tops,
/// [`families::P_FAMILIES_5L_V1_JSON`]) and the 5L families built from it (gap
/// G9 of p5l-converter, M5 batch 3). Not a `p_versions()` entry, so every
/// scope 1-4 package (identity, manifest, census versions) is unchanged: a
/// library whose scope reaches 5 loops appends `,P_FAMILIES_5L_VERSION=<v>`
/// to its identity (`PLibrary::identity`). Bump it with every change of the
/// 5L families file or of how its tops are presented.
pub const P_FAMILIES_5L_VERSION: u32 = 1;
pub const P_ORDERING_SCHEMA: &str = "rustred.p-ordering.v1";
pub const P_LIBRARY_SCHEMA: &str = "rustred.p-owner-library.v1";
pub const P_FAMILIES_SCHEMA: &str = "rustred.p-families.v1";
pub const P_BASIS_LAYER_SCHEMA: &str = "rustred.p-basis-layer.v1";
/// Orbit canonicalisation of Apply keys (M5 R0 A2): the automorphism search,
/// its authentication and the choice of representative. It enters
/// [`library::PLibrary::step_identity`] only while canonicalisation is on, so a
/// library with it off keeps the step caches it had before; the library
/// identity ([`library::PLibrary::identity`]) never carries it.
pub const P_ORBIT_CANON_VERSION: u32 = 3;
/// Whether a new [`library::PLibrary`] canonicalises keys under orbits
/// ([`library::PLibrary::set_orbit_canonicalization`]). Off: the reduction
/// path, the step identity and the step records are the ones without A2.
pub const P_ORBIT_CANON_DEFAULT: bool = false;
/// Whether a new [`library::PLibrary`] that carries saved automorphism
/// groups (`orbits.rrbin`, [`library::PLibraryBuilder::set_saved_orbits`])
/// canonicalises keys under orbits. On (M5 batch 2, measured 2026-11-11 on the
/// combined 1-4 package, 12 cells, values identical): 16 threads -17 % cold and
/// -15 % warm wall, peak RSS -1..-14 %; 1 thread -5.7 % instructions. A
/// library without saved groups keeps [`P_ORBIT_CANON_DEFAULT`] (the run-time
/// group search costs more cold time than it saves on small requests), so
/// the step identity, and the step caches, of such a library do not change.
pub const P_ORBIT_CANON_WITH_SAVED_ORBITS: bool = true;

const _: () = assert!(P_GAMMA_TAG_WIDTH == 2 * P_GAMMA_TAG_MAX_M as usize);

/// Every version constant, in a fixed order. With the scope it is the cache
/// identity of a library program ([`library::PLibrary::identity`]): bump the
/// constant whose semantics change, there is no digest behind it.
pub fn p_versions() -> Vec<(&'static str, String)> {
    vec![
        (
            "P_OWNER_LIBRARY_SEMANTICS",
            P_OWNER_LIBRARY_SEMANTICS.to_owned(),
        ),
        ("P_CLASS_KEY_VERSION", P_CLASS_KEY_VERSION.to_string()),
        ("P_CENSUS_VERSION", P_CENSUS_VERSION.to_string()),
        ("P_PRESENTATION_VERSION", P_PRESENTATION_VERSION.to_string()),
        ("P_FAMILIES_VERSION", P_FAMILIES_VERSION.to_string()),
        ("P_GAMMA_TAG_VERSION", P_GAMMA_TAG_VERSION.to_string()),
        ("P_GAMMA_TAG_MAX_M", P_GAMMA_TAG_MAX_M.to_string()),
        ("P_GAMMA_TAG_WIDTH", P_GAMMA_TAG_WIDTH.to_string()),
        (
            "P_INSERTION_KERNEL_VERSION",
            P_INSERTION_KERNEL_VERSION.to_string(),
        ),
        (
            "P_CARPET_KERNEL_VERSION",
            P_CARPET_KERNEL_VERSION.to_string(),
        ),
        (
            "P_TRIANGLE_TEMPLATE_VERSION",
            P_TRIANGLE_TEMPLATE_VERSION.to_string(),
        ),
        (
            "P_DIAMOND_TEMPLATE_VERSION",
            P_DIAMOND_TEMPLATE_VERSION.to_string(),
        ),
        (
            "P_SCHEME_TEMPLATE_VERSION",
            P_SCHEME_TEMPLATE_VERSION.to_string(),
        ),
        ("P_CUSTOM_OWNER_VERSION", P_CUSTOM_OWNER_VERSION.to_string()),
        ("P_LIBRARY_SCHEMA", P_LIBRARY_SCHEMA.to_owned()),
        ("P_FAMILIES_SCHEMA", P_FAMILIES_SCHEMA.to_owned()),
        ("P_BASIS_LAYER_SCHEMA", P_BASIS_LAYER_SCHEMA.to_owned()),
        ("P_TOPOLOGY_VERSION", P_TOPOLOGY_VERSION.to_string()),
    ]
}

/// Library-local index of a presentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PresentationId(pub u32);

/// Name of an owner class: see [`class::OwnerClassKey::id`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OwnerClassId(pub u64);

/// Library-local index of a terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PTerminalId(pub u32);

/// Index of a node of the reduction DAG.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeIndex(pub u32);
