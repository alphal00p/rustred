//! Campaign supervisor library (no engine linkage: engine programs are child processes under the contract crate).
//! Layering: `atomic`, `lock`, `clock`, `ledger`, `state`, `progress` are pure file/logic modules (package "ledger");
//! `resources`, `launch`, `signals`, `cpu_policy`, `admission` are the process layer (package "proc").
//! The ladder, acceptance and certificate logic does NOT live here: it is library code without any supervisor or
//! ledger assumption (requirement R22), in `rustred-campaign-core`.
pub mod adapters;
pub mod admission;
pub mod atomic;
pub mod cli;
pub mod clock;
pub mod cpu_policy;
pub mod export;
pub mod launch;
pub mod ledger;
pub mod lock;
// lane tadicjob's request (REQUEST TO I-INT): rr_campaign tadic-check / pole-report
pub mod polereport;
pub mod progress;
pub mod publish;
pub mod resources;
pub mod signals;
pub mod state;
pub mod supervisor;
pub mod tui;
// lane closeloop (interfaces/closeloop.md)
pub mod closeloop;
// lane gencheck (interfaces/emission-check.md): the emission path (granted by the implementer 2026-11-18)
pub mod emission;
