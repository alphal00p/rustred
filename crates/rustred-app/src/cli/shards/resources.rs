//! Linux process identities, disjoint CPU assignments and measured resource
//! samples. Worker reservations are never reported as measured CPU use.
use super::{CliError, bad, io_error};
use serde::Serialize;
use std::{collections::BTreeSet, fs, io, process::Command};

#[derive(Clone, Debug, Serialize)]
pub(super) struct ProcessSample {
    pub pid: u32,
    pub start_ticks: u64,
    pub group: u32,
    pub cpu_ticks: u64,
    pub rss_bytes: u64,
}
pub(super) fn parse_stat(pid: u32, text: &str, page_size: u64) -> Option<ProcessSample> {
    // comm may itself contain spaces and ')' characters.
    let fields: Vec<_> = text
        .get(text.rfind(')')? + 2..)?
        .split_whitespace()
        .collect();
    Some(ProcessSample {
        pid,
        start_ticks: fields.get(19)?.parse().ok()?,
        group: fields.get(2)?.parse().ok()?,
        cpu_ticks: fields
            .get(11)?
            .parse::<u64>()
            .ok()?
            .checked_add(fields.get(12)?.parse::<u64>().ok()?)?,
        rss_bytes: fields
            .get(21)?
            .parse::<u64>()
            .ok()?
            .checked_mul(page_size)?,
    })
}
#[cfg(target_os = "linux")]
pub(super) fn sample(pid: u32) -> Option<ProcessSample> {
    let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if page <= 0 {
        return None;
    }
    parse_stat(
        pid,
        &fs::read_to_string(format!("/proc/{pid}/stat")).ok()?,
        page as u64,
    )
}
#[cfg(not(target_os = "linux"))]
pub(super) fn sample(_pid: u32) -> Option<ProcessSample> {
    None
}

pub(super) fn same_process(pid: u32, start: u64) -> bool {
    sample(pid).is_some_and(|s| s.start_ticks == start)
}

#[cfg(target_os = "linux")]
pub(super) fn allowed_cpus() -> Result<BTreeSet<usize>, CliError> {
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        if libc::sched_getaffinity(0, std::mem::size_of_val(&set), &mut set) != 0 {
            return Err(io_error(io::Error::last_os_error()));
        }
        Ok((0..libc::CPU_SETSIZE as usize)
            .filter(|&i| libc::CPU_ISSET(i, &set))
            .collect())
    }
}
#[cfg(not(target_os = "linux"))]
pub(super) fn allowed_cpus() -> Result<BTreeSet<usize>, CliError> {
    Err(bad(
        "independent-root supervision currently requires Linux /proc and CPU affinity",
    ))
}

#[cfg(target_os = "linux")]
pub(super) fn configure(
    command: &mut Command,
    cpus: &[usize],
    master_lock: Option<&fs::File>,
) -> Result<(), CliError> {
    use std::os::fd::AsRawFd;
    use std::os::unix::process::CommandExt;
    let inherited_lock = master_lock.map(AsRawFd::as_raw_fd);
    let mut set: libc::cpu_set_t = unsafe { std::mem::zeroed() };
    for &cpu in cpus {
        if cpu >= libc::CPU_SETSIZE as usize {
            return Err(bad("CPU index exceeds native affinity capacity"));
        }
        unsafe { libc::CPU_SET(cpu, &mut set) };
    }
    // Only async-signal-safe syscalls in the child before exec; inputs are
    // prepared in the parent, before Command forks.
    unsafe {
        command.pre_exec(move || {
            // Retain the campaign lock across exec. If the supervisor dies
            // between fork and persisting this PID, an orphan still prevents
            // another supervisor from launching duplicate work.
            if let Some(fd) = inherited_lock {
                let flags = libc::fcntl(fd, libc::F_GETFD);
                if flags < 0 || libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
                    return Err(io::Error::last_os_error());
                }
            }
            if libc::setsid() < 0 {
                return Err(io::Error::last_os_error());
            }
            if libc::sched_setaffinity(0, std::mem::size_of_val(&set), &set) != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    Ok(())
}
#[cfg(not(target_os = "linux"))]
pub(super) fn configure(
    _command: &mut Command,
    _cpus: &[usize],
    _master_lock: Option<&fs::File>,
) -> Result<(), CliError> {
    Err(bad("Linux is required"))
}

#[cfg(target_os = "linux")]
pub(super) fn ticks_per_second() -> f64 {
    (unsafe { libc::sysconf(libc::_SC_CLK_TCK) }) as f64
}
#[cfg(not(target_os = "linux"))]
pub(super) fn ticks_per_second() -> f64 {
    1.0
}

/// Include child process groups, including any subprocesses, and supervisor
/// RSS. Only explicitly owned root identities can be signalled.
pub(super) fn group_samples(groups: &BTreeSet<u32>) -> Result<Vec<ProcessSample>, CliError> {
    let mut result = vec![];
    for entry in fs::read_dir("/proc").map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        if let Some(row) = sample(pid) {
            if groups.contains(&row.group) || pid == std::process::id() {
                result.push(row);
            }
        }
    }
    Ok(result)
}
pub(super) fn host_available() -> Result<u64, CliError> {
    let text = fs::read_to_string("/proc/meminfo").map_err(io_error)?;
    let host = text
        .lines()
        .find_map(|line| {
            line.strip_prefix("MemAvailable:")
                .and_then(|s| s.split_whitespace().next())
                .and_then(|s| s.parse::<u64>().ok())
        })
        .and_then(|n| n.checked_mul(1024))
        .ok_or_else(|| bad("MemAvailable missing or malformed"))?;
    // Respect cgroup-v2 headroom when available; host RAM alone can substantially
    // overstate available memory in a container.
    let mut available = host;
    if let Ok(cgroups) = fs::read_to_string("/proc/self/cgroup") {
        if let Some(group) = cgroups.lines().find_map(|s| s.strip_prefix("0::")) {
            let mut path =
                std::path::PathBuf::from("/sys/fs/cgroup").join(group.trim_start_matches('/'));
            loop {
                if let (Ok(max), Ok(current)) = (
                    fs::read_to_string(path.join("memory.max")),
                    fs::read_to_string(path.join("memory.current")),
                ) {
                    if let (Ok(max), Ok(current)) =
                        (max.trim().parse::<u64>(), current.trim().parse::<u64>())
                    {
                        available = available.min(max.saturating_sub(current));
                    }
                }
                if path == std::path::Path::new("/sys/fs/cgroup") || !path.pop() {
                    break;
                }
            }
        }
    }
    Ok(available)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MemoryAction {
    Continue,
    Save,
    Emergency,
}
pub(super) fn memory_action(rss: u64, available: u64, maximum: u64, reserve: u64) -> MemoryAction {
    if rss >= maximum || available <= reserve / 4 {
        MemoryAction::Emergency
    } else if rss >= maximum - maximum / 20 || available <= reserve {
        MemoryAction::Save
    } else {
        MemoryAction::Continue
    }
}
#[cfg(target_os = "linux")]
pub(super) fn emergency_kill(pid: u32, start: u64) -> Result<(), CliError> {
    if !same_process(pid, start) {
        return Ok(());
    }
    // setsid establishes a unique group led by this verified owned process.
    if unsafe { libc::kill(-(pid as i32), libc::SIGKILL) } != 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::ESRCH) {
            return Err(io_error(error));
        }
    }
    Ok(())
}
#[cfg(not(target_os = "linux"))]
pub(super) fn emergency_kill(_pid: u32, _start: u64) -> Result<(), CliError> {
    Err(bad("Linux is required"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_soft_and_hard_ram_thresholds() {
        assert_eq!(memory_action(94, 1000, 100, 20), MemoryAction::Continue);
        assert_eq!(memory_action(95, 1000, 100, 20), MemoryAction::Save);
        assert_eq!(memory_action(100, 1000, 100, 20), MemoryAction::Emergency);
        assert_eq!(memory_action(10, 20, 100, 20), MemoryAction::Save);
        assert_eq!(memory_action(10, 5, 100, 20), MemoryAction::Emergency);
    }
    #[test]
    fn process_stat_handles_parenthesized_names() {
        let mut fields = vec!["0"; 22];
        fields[0] = "S";
        fields[2] = "123";
        fields[11] = "7";
        fields[12] = "8";
        fields[19] = "999";
        fields[21] = "4";
        let sample = parse_stat(
            123,
            &format!("123 (owner worker (2)) {}", fields.join(" ")),
            4096,
        )
        .unwrap();
        assert_eq!(
            (sample.start_ticks, sample.cpu_ticks, sample.rss_bytes),
            (999, 15, 16384)
        );
    }
}
