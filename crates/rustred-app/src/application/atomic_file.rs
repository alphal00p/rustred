//! Atomic native-output installation shared by frontend and checkpoint I/O.
//!
//! The caller owns path selection and maps these contextual errors to its API.

use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const MAX_TEMPORARY_NAME_ATTEMPTS: u32 = 1_024;

pub(crate) fn write_file_atomically(
    path: &Path,
    contents: &[u8],
    force: bool,
) -> Result<(), String> {
    write_file_atomically_with(path, force, |file| {
        file.write_all(contents)
            .map_err(|error| format!("cannot write output {}: {error}", path.display()))
    })
}

/// Stream a potentially large state without retaining a second full image.
pub(crate) fn write_file_atomically_with(
    path: &Path,
    force: bool,
    write: impl FnOnce(&mut File) -> Result<(), String>,
) -> Result<(), String> {
    if path.file_name().is_none() {
        return Err(format!("output path {} has no file name", path.display()));
    }
    if !force && path.exists() {
        return Err(format!(
            "output {} already exists; use --force to replace it",
            path.display()
        ));
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = None;
    for attempt in 0..MAX_TEMPORARY_NAME_ATTEMPTS {
        let candidate = temporary_path(parent, path.file_name().unwrap(), attempt);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => {
                temporary = Some((candidate, file));
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(format!(
                    "cannot create an atomic output beside {}: {error}",
                    path.display()
                ));
            }
        }
    }
    let Some((temporary_path, mut temporary_file)) = temporary else {
        return Err(format!(
            "cannot acquire a temporary output name beside {}",
            path.display()
        ));
    };
    let result = (|| {
        write(&mut temporary_file)?;
        temporary_file
            .sync_all()
            .map_err(|error| format!("cannot sync output {}: {error}", path.display()))?;
        drop(temporary_file);

        if force {
            fs::rename(&temporary_path, path).map_err(|error| {
                format!(
                    "cannot atomically install output {}: {error}",
                    path.display()
                )
            })?;
        } else {
            // A hard link is the stable-std create-if-absent primitive.  It
            // installs the already synced inode atomically and cannot replace
            // a path which appears during the write.
            fs::hard_link(&temporary_path, path).map_err(|error| {
                let detail = if error.kind() == io::ErrorKind::AlreadyExists {
                    "the destination appeared while it was being prepared".to_owned()
                } else {
                    error.to_string()
                };
                format!(
                    "cannot atomically install output {}: {detail}",
                    path.display()
                )
            })?;
            fs::remove_file(&temporary_path).map_err(|error| {
                format!(
                    "output {} was installed but its staging link could not be removed: {error}",
                    path.display()
                )
            })?;
        }
        sync_parent_directory(parent, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
}

fn temporary_path(parent: &Path, file_name: &OsStr, attempt: u32) -> PathBuf {
    let mut name = OsString::from(".");
    name.push(file_name);
    name.push(".rustred-tmp-");
    name.push(std::process::id().to_string());
    name.push("-");
    name.push(attempt.to_string());
    parent.join(name)
}

fn sync_parent_directory(parent: &Path, path: &Path) -> Result<(), String> {
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            format!(
                "output {} was installed but its directory could not be synced: {error}",
                path.display()
            )
        })
}
