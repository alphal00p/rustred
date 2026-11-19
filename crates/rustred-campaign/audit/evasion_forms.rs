//! procfix7 round 6 (gate10 m10-4): EVERY known form that collapses a filesystem error into "absent" / "not a file" /
//! "no content" / an incomplete listing. NOT compiled (audit/ is no module of the crate). tests/procfix9_tripwire.rs runs
//! `audit/fs_sites.py --scan` over this file as PRODUCTION code: every line marked EXPECT must be an unreviewed candidate.
//! Lines 1-23 of the evasion forms: gate10 scripts/gate10/clippytest/lib.rs (verbatim); then further forms.
use std::path::{Path, PathBuf};
pub fn a_baseline_exists(p: &Path) -> bool { p.exists() } // EXPECT
pub fn b_closure(ps: &[&Path]) -> usize { ps.iter().filter(|p| p.exists()).count() } // EXPECT
pub fn c_ufcs(ps: Vec<&Path>) -> usize { ps.into_iter().filter(|p| Path::exists(p)).count() } // EXPECT
pub fn d_fn_path(ps: Vec<&Path>) -> usize { ps.into_iter().map(Path::exists).filter(|b| *b).count() } // EXPECT
macro_rules! ex { ($p:expr) => { $p.exists() } } // EXPECT
pub fn e_local_macro(p: &Path) -> bool { ex!(p) } // EXPECT
pub fn f_metadata_is_ok(p: &Path) -> bool { std::fs::metadata(p).is_ok() } // EXPECT
pub fn g_file_open_is_ok(p: &Path) -> bool { std::fs::File::open(p).is_ok() } // EXPECT
pub fn h_canonicalize_is_ok(p: &Path) -> bool { p.canonicalize().is_ok() } // EXPECT
pub fn i_read_link_is_ok(p: &Path) -> bool { std::fs::read_link(p).is_ok() } // EXPECT
pub fn j_pathbuf_deref(p: &PathBuf) -> bool { p.exists() } // EXPECT
pub fn k_read_dir_flatten(d: &Path) -> usize { std::fs::read_dir(d).map(|r| r.flatten().count()).unwrap_or(0) } // EXPECT
pub fn l_openoptions(p: &Path) -> bool { std::fs::OpenOptions::new().read(true).open(p).is_ok() } // EXPECT
pub fn m_matches_symlink_metadata(p: &Path) -> bool { matches!(std::fs::symlink_metadata(p), Ok(_)) } // EXPECT
pub fn n_dirent_metadata(e: std::fs::DirEntry) -> bool { e.metadata().is_ok() } // EXPECT
pub fn o_dirent_file_type(e: &std::fs::DirEntry) -> bool { e.file_type().map(|t| t.is_file()).unwrap_or(false) } // EXPECT
pub fn p_fs_exists(p: &Path) -> bool { std::fs::exists(p).unwrap_or(false) } // EXPECT
pub fn q_read_ok(p: &Path) -> Vec<u8> { std::fs::read(p).unwrap_or_default() } // EXPECT
pub fn r_try_exists(p: &Path) -> bool { p.try_exists().unwrap_or(false) } // EXPECT
pub fn s_fn_pointer() -> fn(&Path) -> bool { Path::is_dir } // EXPECT
pub fn t_trait_obj(p: &dyn AsRef<Path>) -> bool { p.as_ref().is_file() } // EXPECT
pub fn u_canonicalize_fs(p: &Path) -> bool { std::fs::canonicalize(p).is_ok() } // EXPECT
pub fn v_read_to_string_ok(p: &Path) -> Option<String> { std::fs::read_to_string(p).ok() } // EXPECT
pub fn w_metadata_method_in_iter(ps: &[PathBuf]) -> usize { ps.iter().filter_map(|p| p.metadata().ok()).count() } // EXPECT
pub fn x1_matches_file_open(p: &Path) -> bool { matches!(std::fs::File::open(p), Ok(_)) } // EXPECT
pub fn x2_file_open_map_default(p: &Path) -> bool { std::fs::File::open(p).map(|_| true).unwrap_or(false) } // EXPECT
pub fn x3_read_link_ok(p: &Path) -> Option<PathBuf> { std::fs::read_link(p).ok() } // EXPECT
pub fn x4_if_let_metadata(p: &Path) -> bool { if let Ok(m) = std::fs::metadata(p) { m.is_dir() } else { false } } // EXPECT
pub fn x5_openoptions_ok_some(p: &Path) -> bool { std::fs::OpenOptions::new().write(true).open(p).ok().is_some() } // EXPECT
pub fn x6_path_read_dir_is_ok(p: &Path) -> bool { p.read_dir().is_ok() } // EXPECT
pub fn x7_fs_read_link_fn_value() -> fn(&Path) -> std::io::Result<PathBuf> { Path::read_link } // EXPECT
pub fn x8_canonicalize_fallback(p: &Path) -> PathBuf { std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf()) } // EXPECT
