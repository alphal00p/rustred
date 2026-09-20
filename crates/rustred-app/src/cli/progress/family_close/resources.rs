//! Best-effort process resources, not a process-tree or scheduler memory limit.

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Resources {
    pub(super) rss: Option<u64>,
    pub(super) peak_rss: Option<u64>,
}

impl Resources {
    pub(super) fn sample() -> Self {
        #[cfg(target_os = "linux")]
        {
            std::fs::read_to_string("/proc/self/status")
                .map(|text| Self::parse(&text))
                .unwrap_or_default()
        }
        #[cfg(not(target_os = "linux"))]
        {
            Self::default()
        }
    }

    #[cfg(any(target_os = "linux", test))]
    pub(super) fn parse(text: &str) -> Self {
        let read = |name: &str| {
            let line = text.lines().find(|line| line.starts_with(name))?;
            let mut words = line.split_ascii_whitespace();
            if words.next()? != name {
                return None;
            }
            let value = words.next()?.parse::<u64>().ok()?;
            if words.next()? != "kB" {
                return None;
            }
            value.checked_mul(1_024)
        };
        Self {
            rss: read("VmRSS:"),
            peak_rss: read("VmHWM:"),
        }
    }

    pub(super) fn line(self) -> String {
        let display = |value: Option<u64>| {
            value.map_or_else(
                || "unavailable".to_owned(),
                |bytes| format!("{:.1} MiB", bytes as f64 / 1_048_576.0),
            )
        };
        format!(
            "process RSS={} peak={} | CPU=unavailable | not process-tree memory",
            display(self.rss),
            display(self.peak_rss)
        )
    }
}
