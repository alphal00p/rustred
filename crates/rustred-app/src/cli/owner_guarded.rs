use super::{
    args::{OwnerGuardedApplyArgs, StreamPath},
    error::CliError,
    io::{preflight_output_destination, read_bounded, read_input, write_output},
    progress::RoutedProgress,
};
use crate::{OwnerGuardedApplyRequest, OwnerGuardedApplyResult, owner_guarded_apply_with_progress};
use serde_json::json;
use std::{
    fs::{File, OpenOptions},
    io::{self, IsTerminal, Write},
    sync::{Arc, atomic::AtomicBool},
};

pub(super) fn run(args: OwnerGuardedApplyArgs) -> Result<(), CliError> {
    super::routed::preflight_inner_pools()?;
    let output = StreamPath::File(args.output.clone());
    preflight_output_destination(&output, false)?;
    if let Some(path) = &args.events {
        preflight_output_destination(&StreamPath::File(path.clone()), false)?;
    }
    if args.events.as_ref() == Some(&args.output)
        || args.stop_file.as_ref() == Some(&args.output)
        || args.events.is_some() && args.events == args.stop_file
    {
        return Err(CliError::Input(
            "event, result and stop paths must differ".into(),
        ));
    }
    let query_bytes = read_bounded(
        File::open(&args.queries)
            .map_err(|e| CliError::InputIo(format!("{}: {e}", args.queries.display())))?,
        "guarded queries",
        1024 * 1024,
    )?;
    let queries = String::from_utf8(query_bytes)
        .map_err(|_| CliError::Input("guarded queries must be UTF-8".into()))?;
    let mut request =
        OwnerGuardedApplyRequest::new(read_input(&StreamPath::File(args.manifest))?, queries);
    request.owner_base = args.owner_base;
    request.max_queries = args.max_queries;
    request.max_report_events = args.max_report_events;
    request.max_report_bytes = args.max_report_bytes;
    request.max_expression_bytes = args.max_expression_bytes;
    if let Some(path) = args.work_limits {
        let bytes = read_bounded(
            File::open(&path).map_err(|e| CliError::InputIo(format!("{}: {e}", path.display())))?,
            "guarded work limits",
            16 * 1024,
        )?;
        let text = String::from_utf8(bytes)
            .map_err(|_| CliError::Input("guarded work limits must be UTF-8".into()))?;
        request.limits = crate::application::guarded_limits_from_json(&text)?;
    }
    let events: Box<dyn Write + Send> = match args.events {
        Some(path) => Box::new(
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(|e| CliError::OutputIo(format!("{}: {e}", path.display())))?,
        ),
        None => Box::new(io::stdout()),
    };
    let cancellation = Arc::new(AtomicBool::new(
        args.stop_file.as_ref().is_some_and(|p| p.exists()),
    ));
    let monitor = RoutedProgress::start(
        events,
        io::stderr().is_terminal() && !args.no_progress,
        Arc::clone(&cancellation),
        args.stop_file,
    );
    let (mut document, outcome) =
        match owner_guarded_apply_with_progress(request, &cancellation, |event| {
            monitor.observe(event)
        }) {
            Ok(result) => (
                result.document,
                diagnostic_outcome(result.diagnostic_complete),
            ),
            Err(error) => (
                OwnerGuardedApplyResult::preparation_error_document(&error),
                Err(CliError::from(error)),
            ),
        };
    document["operation"] = json!("owner_guarded_apply");
    monitor.observe(OwnerGuardedApplyResult::completion_progress(&document));
    let presentation = monitor.finish();
    let mut bytes = BoundedBytes {
        bytes: Vec::new(),
        limit: args.max_report_bytes,
    };
    serde_json::to_writer(&mut bytes, &document)
        .map_err(|e| CliError::OutputIo(format!("bounded guarded report: {e}")))?;
    write_output(&output, &bytes.bytes, false)?;
    presentation.map_err(|e| CliError::OutputIo(format!("event journal: {e}")))?;
    outcome
}
/// Completion is deliberately not applicability/closure: native Problem and
/// Residual events can be retained by a successfully finished diagnostic.
fn diagnostic_outcome(complete: bool) -> Result<(), CliError> {
    if complete {
        Ok(())
    } else {
        Err(CliError::Input(
            "guarded diagnostic incomplete; see saved result".into(),
        ))
    }
}
struct BoundedBytes {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write for BoundedBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(io::Error::other("guarded report byte limit"));
        }
        self.bytes
            .try_reserve(bytes.len())
            .map_err(io::Error::other)?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn guarded_diagnostic_exit_and_output_writer_are_bounded() {
        assert!(diagnostic_outcome(true).is_ok());
        assert_eq!(diagnostic_outcome(false).unwrap_err().exit_code(), 4);
        let mut b = BoundedBytes {
            bytes: Vec::new(),
            limit: 3,
        };
        b.write_all(b"abc").unwrap();
        assert!(b.write_all(b"d").is_err());
        assert_eq!(b.bytes, b"abc");
    }
}
