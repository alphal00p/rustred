use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use crate::MAX_INPUT_BYTES;
use crate::cli::args::StreamPath;
use crate::cli::error::CliError;

const MAX_TEMPORARY_NAME_ATTEMPTS: u32 = 1_024;

pub(crate) fn read_input(source: &StreamPath) -> Result<String, CliError> {
    let bytes = match source {
        StreamPath::Stdio => read_bounded(io::stdin().lock(), "standard input")?,
        StreamPath::File(path) => {
            let file = File::open(path).map_err(|error| {
                CliError::InputIo(format!("cannot open input {}: {error}", path.display()))
            })?;
            read_bounded(file, &format!("input {}", path.display()))?
        }
    };
    String::from_utf8(bytes).map_err(|error| {
        CliError::Input(format!(
            "input is not UTF-8 (invalid byte at offset {})",
            error.utf8_error().valid_up_to()
        ))
    })
}

fn read_bounded(mut reader: impl Read, label: &str) -> Result<Vec<u8>, CliError> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| CliError::InputIo(format!("cannot read {label}: {error}")))?;
        if read == 0 {
            break;
        }
        let requested = bytes.len().checked_add(read).ok_or_else(|| {
            CliError::Input(format!(
                "input length overflowed the {MAX_INPUT_BYTES}-byte CLI limit"
            ))
        })?;
        if requested > MAX_INPUT_BYTES {
            return Err(CliError::Input(format!(
                "input exceeds the {MAX_INPUT_BYTES}-byte CLI limit"
            )));
        }
        bytes.try_reserve(read).map_err(|_| {
            CliError::InputIo(format!(
                "cannot reserve {requested} bytes while reading {label}"
            ))
        })?;
        bytes.extend_from_slice(&buffer[..read]);
    }
    Ok(bytes)
}

pub(crate) fn write_output(
    destination: &StreamPath,
    contents: &[u8],
    force: bool,
) -> Result<(), CliError> {
    match destination {
        StreamPath::Stdio => {
            let mut stdout = io::stdout().lock();
            stdout
                .write_all(contents)
                .and_then(|()| stdout.flush())
                .map_err(|error| {
                    CliError::OutputIo(format!("cannot write standard output: {error}"))
                })
        }
        StreamPath::File(path) => write_file_atomically(path, contents, force),
    }
}

fn write_file_atomically(path: &Path, contents: &[u8], force: bool) -> Result<(), CliError> {
    if path.file_name().is_none() {
        return Err(CliError::OutputIo(format!(
            "output path {} has no file name",
            path.display()
        )));
    }
    if !force && path.exists() {
        return Err(CliError::OutputIo(format!(
            "output {} already exists; use --force to replace it",
            path.display()
        )));
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
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
                return Err(CliError::OutputIo(format!(
                    "cannot create an atomic output beside {}: {error}",
                    path.display()
                )));
            }
        }
    }
    let Some((temporary_path, mut temporary_file)) = temporary else {
        return Err(CliError::OutputIo(format!(
            "cannot acquire a temporary output name beside {}",
            path.display()
        )));
    };
    let result = (|| {
        temporary_file.write_all(contents).map_err(|error| {
            CliError::OutputIo(format!("cannot write output {}: {error}", path.display()))
        })?;
        temporary_file.sync_all().map_err(|error| {
            CliError::OutputIo(format!("cannot sync output {}: {error}", path.display()))
        })?;
        drop(temporary_file);

        if force {
            fs::rename(&temporary_path, path).map_err(|error| {
                CliError::OutputIo(format!(
                    "cannot atomically install output {}: {error}",
                    path.display()
                ))
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
                CliError::OutputIo(format!(
                    "cannot atomically install output {}: {detail}",
                    path.display()
                ))
            })?;
            fs::remove_file(&temporary_path).map_err(|error| {
                CliError::OutputIo(format!(
                    "output {} was installed but its staging link could not be removed: {error}",
                    path.display()
                ))
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

fn sync_parent_directory(parent: &Path, path: &Path) -> Result<(), CliError> {
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            CliError::OutputIo(format!(
                "output {} was installed but its directory could not be synced: {error}",
                path.display()
            ))
        })
}
