//! gate10 (a)(1): which collapsing std forms does crates/rustred-campaign/clippy.toml (disallowed-methods) catch?
//! Each function is one evasion attempt; `cargo clippy` lists the warned ones (the rest compile silently).
#![allow(dead_code)]
use std::path::{Path, PathBuf};
pub fn a_baseline_exists(p: &Path) -> bool { p.exists() }
pub fn b_closure(ps: &[&Path]) -> usize { ps.iter().filter(|p| p.exists()).count() }
pub fn c_ufcs(ps: Vec<&Path>) -> usize { ps.into_iter().filter(|p| Path::exists(p)).count() }
pub fn d_fn_path(ps: Vec<&Path>) -> usize { ps.into_iter().map(Path::exists).filter(|b| *b).count() }
macro_rules! ex { ($p:expr) => { $p.exists() } }
pub fn e_local_macro(p: &Path) -> bool { ex!(p) }
pub fn f_metadata_is_ok(p: &Path) -> bool { std::fs::metadata(p).is_ok() }
pub fn g_file_open_is_ok(p: &Path) -> bool { std::fs::File::open(p).is_ok() }
pub fn h_canonicalize_is_ok(p: &Path) -> bool { p.canonicalize().is_ok() }
pub fn i_read_link_is_ok(p: &Path) -> bool { std::fs::read_link(p).is_ok() }
pub fn j_pathbuf_deref(p: &PathBuf) -> bool { p.exists() }
pub fn k_read_dir_flatten(d: &Path) -> usize { std::fs::read_dir(d).map(|r| r.flatten().count()).unwrap_or(0) }
pub fn l_openoptions(p: &Path) -> bool { std::fs::OpenOptions::new().read(true).open(p).is_ok() }
pub fn m_matches_symlink_metadata(p: &Path) -> bool { matches!(std::fs::symlink_metadata(p), Ok(_)) }
pub fn n_dirent_metadata(e: std::fs::DirEntry) -> bool { e.metadata().is_ok() }
pub fn o_dirent_file_type(e: &std::fs::DirEntry) -> bool { e.file_type().map(|t| t.is_file()).unwrap_or(false) }
pub fn p_fs_exists(p: &Path) -> bool { std::fs::exists(p).unwrap_or(false) }
pub fn q_read_ok(p: &Path) -> Vec<u8> { std::fs::read(p).unwrap_or_default() }
pub fn r_try_exists(p: &Path) -> bool { p.try_exists().unwrap_or(false) }
pub fn s_fn_pointer() -> fn(&Path) -> bool { Path::is_dir }
pub fn t_trait_obj(p: &dyn AsRef<Path>) -> bool { p.as_ref().is_file() }
pub fn u_canonicalize_fs(p: &Path) -> bool { std::fs::canonicalize(p).is_ok() }
pub fn v_read_to_string_ok(p: &Path) -> Option<String> { std::fs::read_to_string(p).ok() }
pub fn w_metadata_method_in_iter(ps: &[PathBuf]) -> usize { ps.iter().filter_map(|p| p.metadata().ok()).count() }
