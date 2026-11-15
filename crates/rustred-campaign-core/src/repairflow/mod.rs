//! The REPAIR FLOW (interfaces/repairflow.md): per uncovered face of a node's closure, a CHEAP symbolic attempt
//! (small deterministic budget, lift pre-screen with the size caps, children returned not searched), judged by the
//! per-rule caps, the keys it serves and a crossover cost model, then the node size budget; every face gets its recipe
//! escalation (face-domain recipes solved at the keys by the closing loop); what nothing serves becomes a typed
//! terminal proposal. Pure library: no process, file, clock or ledger. Every threshold is policy DATA ([`policy`]).

pub mod composite;
pub mod decide;
pub mod face_format;
pub mod faces;
pub mod judge;
pub mod minimise;
pub mod policy;
pub mod recipes;
pub mod symflow;
pub mod symrel;

pub use decide::{
    CandidateAnswer, FaceAnswer, FaceDecision, NodeDecisions, decide, extension_sources, s_plan,
};
pub use faces::{
    AttributionStats, ExceptionRef, FaceGroup, ShadowClass, case_line, exception_faces,
    group_open_keys, s_args, shadow_face,
};
pub use judge::{Class, Judgement, Measures, cost, judge, measures_of, served_keys};
pub use policy::{POLICY_SCHEMA, PolicyFile, RepairPolicy};
pub use recipes::{RecipeContext, face_recipe, face_tag};
