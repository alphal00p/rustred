//! Atomic native-output installation shared by frontend and checkpoint I/O.
//!
//! The caller owns path selection and maps these contextual errors to its API.

use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

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

/// Legacy string-error boundary. Resource-owned callers use the typed helper
/// below so their original budget failure survives the public error mapping.
pub(crate) fn write_file_atomically_with(
    path: &Path,
    force: bool,
    write: impl FnOnce(&mut File) -> Result<(), String>,
) -> Result<(), String> {
    write_with(path, force, write, || Ok(()), |error| error)
}

/// Cache callers retain the typed original RAM/time failure. Once publication
/// begins no resource poll can turn a committed write into a resource error.
pub(crate) fn write_file_atomically_with_resources(
    path: &Path,
    force: bool,
    write: impl FnOnce(&mut File) -> Result<(), crate::AppError>,
) -> Result<(), crate::AppError> {
    write_with(
        path,
        force,
        write,
        || symbolica::utils::resources::checkpoint().map_err(crate::AppError::resource),
        crate::AppError::input,
    )
}

pub(crate) fn write_file_atomically_resource(
    path: &Path,
    contents: &[u8],
    force: bool,
) -> Result<(), crate::AppError> {
    write_file_atomically_with_resources(path, force, |file| {
        for bytes in contents.chunks(64 * 1024) {
            symbolica::utils::resources::checkpoint().map_err(crate::AppError::resource)?;
            file.write_all(bytes)
                .map_err(|error| crate::AppError::input(error.to_string()))?;
        }
        Ok(())
    })
}

fn write_with<E>(
    path: &Path,
    force: bool,
    write: impl FnOnce(&mut File) -> Result<(), E>,
    mut checkpoint: impl FnMut() -> Result<(), E>,
    error: impl Fn(String) -> E,
) -> Result<(), E> {
    checkpoint()?;
    if path.file_name().is_none() {
        return Err(error(format!(
            "output path {} has no file name",
            path.display()
        )));
    }
    if !force && path.exists() {
        return Err(error(format!(
            "output {} already exists; use --force to replace it",
            path.display()
        )));
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut attempt = 0u64;
    let (temporary_path, mut temporary_file) = loop {
        checkpoint()?;
        let candidate = temporary_path(parent, path.file_name().unwrap(), attempt);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => break (candidate, file),
            Err(cause) if cause.kind() == io::ErrorKind::AlreadyExists => {
                attempt = attempt
                    .checked_add(1)
                    .ok_or_else(|| error("atomic temporary-name space exhausted".into()))?;
            }
            Err(cause) => {
                return Err(error(format!(
                    "cannot create an atomic output beside {}: {cause}",
                    path.display()
                )));
            }
        }
    };
    let result = (|| {
        write(&mut temporary_file)?;
        temporary_file
            .sync_all()
            .map_err(|cause| error(format!("cannot sync output {}: {cause}", path.display())))?;
        checkpoint()?;
        drop(temporary_file);
        if force {
            fs::rename(&temporary_path, path).map_err(|cause| {
                error(format!(
                    "cannot atomically install output {}: {cause}",
                    path.display()
                ))
            })?;
        } else {
            fs::hard_link(&temporary_path, path).map_err(|cause| {
                let detail = if cause.kind() == io::ErrorKind::AlreadyExists {
                    "the destination appeared while it was being prepared".to_owned()
                } else {
                    cause.to_string()
                };
                error(format!(
                    "cannot atomically install output {}: {detail}",
                    path.display()
                ))
            })?;
            fs::remove_file(&temporary_path).map_err(|cause| {
                error(format!(
                    "output {} was installed but its staging link could not be removed: {cause}",
                    path.display()
                ))
            })?;
        }
        sync_parent_directory(parent, path).map_err(&error)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
}

fn temporary_path(parent: &Path, file_name: &OsStr, attempt: u64) -> PathBuf {
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

#[cfg(test)]
mod tests {
    use super::*;
    use symbolica::utils::resources::{ResourceBudget, ResourceLimitError, ResourceLimits};

    #[test]
    fn resource_refusal_after_staging_preserves_the_previous_committed_file() {
        struct Scratch(PathBuf);
        impl Drop for Scratch {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let scratch = Scratch(std::env::temp_dir().join(format!(
            "rustred-atomic-resource-{}-{nonce}",
            std::process::id()
        )));
        fs::create_dir(&scratch.0).unwrap();
        let output = scratch.0.join("output");
        fs::write(&output, b"old").unwrap();
        let directory = scratch.0.clone();
        let budget = ResourceBudget::with_memory_monitor(
            ResourceLimits {
                max_process_rss_bytes: Some(1),
                ..Default::default()
            },
            move || {
                let staged = fs::read_dir(&directory)
                    .map_err(|error| error.to_string())?
                    .try_fold(false, |found, entry| {
                        let entry = entry?;
                        Ok::<_, io::Error>(
                            found
                                || entry
                                    .file_name()
                                    .to_string_lossy()
                                    .starts_with(".output.rustred-tmp-"),
                        )
                    })
                    .map_err(|error| error.to_string())?;
                Ok(if staged { 2 } else { 0 })
            },
        );
        let error = budget
            .run(|| {
                write_file_atomically_with_resources(&output, true, |file| {
                    file.write_all(b"new")
                        .map_err(|error| crate::AppError::input(error.to_string()))
                })
            })
            .unwrap_err();
        assert!(matches!(
            error.resource_limit_error(),
            Some(ResourceLimitError::Memory { .. })
        ));
        assert_eq!(fs::read(&output).unwrap(), b"old");
        assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 1);
        write_file_atomically_resource(&output, b"new", true).unwrap();
        assert_eq!(fs::read(&output).unwrap(), b"new");
    }
}
