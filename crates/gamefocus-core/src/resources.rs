//! Lightweight host resource probes for session impact summaries.

/// Best-effort "memory available to apps" in bytes (Linux MemAvailable; else None).
pub fn mem_available_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let text = std::fs::read_to_string("/proc/meminfo").ok()?;
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("MemAvailable:") {
                let kb: u64 = rest.split_whitespace().next()?.parse().ok()?;
                return Some(kb.saturating_mul(1024));
            }
        }
        None
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// Human summary after Focus/Boost apply.
pub fn format_session_impact(
    parked: usize,
    boost_applied: bool,
    before_avail: Option<u64>,
    after_avail: Option<u64>,
) -> String {
    let mut lines = vec!["Session active".to_string()];
    let mut focus = format!("Focus: {parked} apps paused");
    if let (Some(before), Some(after)) = (before_avail, after_avail) {
        if after >= before {
            let freed_mib = (after - before) / (1024 * 1024);
            focus.push_str(&format!(" (MemAvailable +{freed_mib} MiB)"));
        } else {
            let lost_mib = (before - after) / (1024 * 1024);
            focus.push_str(&format!(" (MemAvailable -{lost_mib} MiB)"));
        }
    }
    lines.push(focus);
    if boost_applied {
        lines.push("Boost: applied".into());
    } else {
        lines.push("Boost: off or soft-failed".into());
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_positive_mem_delta() {
        let s = format_session_impact(
            12,
            true,
            Some(2 * 1024 * 1024 * 1024),
            Some(5 * 1024 * 1024 * 1024 + 400 * 1024 * 1024),
        );
        assert!(s.contains("12 apps paused"));
        // (5GiB + 400MiB) - 2GiB = 3488 MiB
        assert!(s.contains("+3488 MiB"), "{s}");
        assert!(s.contains("Boost: applied"));
    }

    #[test]
    fn formats_without_mem_probe() {
        let s = format_session_impact(3, false, None, None);
        assert!(s.contains("3 apps paused"));
        assert!(!s.contains("MemAvailable"));
        assert!(s.contains("Boost: off"));
    }
}
