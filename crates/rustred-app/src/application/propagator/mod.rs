//! The p library on disk and in vakint: assembly, format, package, loader and
//! custom-owner generation (plan M2 section 4; owner of this file: M2-I0
//! rr-iface). Each submodule names its owner task.
pub mod assemble;
pub mod format;
pub mod generate;
pub mod loader;
pub mod package;
