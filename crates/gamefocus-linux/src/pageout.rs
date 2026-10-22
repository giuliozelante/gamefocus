//! Best-effort RSS reclaim after SIGSTOP via `process_madvise(..., MADV_PAGEOUT)`.

#[cfg(target_os = "linux")]
use tracing::debug;

/// Anonymous private `rw` VMA from `/proc/<pid>/maps`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnonRange {
    pub start: u64,
    pub end: u64,
}

/// Parse `/proc/pid/maps` lines; keep anonymous private read-write mappings.
pub fn parse_anon_rw_ranges(maps: &str) -> Vec<AnonRange> {
    let mut out = Vec::new();
    for line in maps.lines() {
        // 55b8a8a00000-55b8a8c00000 rw-p 00000000 00:00 0
        let mut parts = line.split_whitespace();
        let Some(range) = parts.next() else {
            continue;
        };
        let Some(perms) = parts.next() else {
            continue;
        };
        let Some(_offset) = parts.next() else {
            continue;
        };
        let Some(dev) = parts.next() else {
            continue;
        };
        if !perms.starts_with("rw") || !perms.contains('p') {
            continue;
        }
        if dev != "00:00" {
            continue;
        }
        let Some((start_s, end_s)) = range.split_once('-') else {
            continue;
        };
        let Ok(start) = u64::from_str_radix(start_s, 16) else {
            continue;
        };
        let Ok(end) = u64::from_str_radix(end_s, 16) else {
            continue;
        };
        if end > start {
            out.push(AnonRange { start, end });
        }
    }
    out
}

/// After SIGSTOP: hint kernel to page out anonymous RSS for `pid`. Soft-fails always.
pub fn page_out_after_stop(pid: u32) {
    #[cfg(target_os = "linux")]
    {
        if let Err(err) = page_out_linux(pid) {
            debug!(pid, %err, "process_madvise pageout skipped");
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
    }
}

#[cfg(target_os = "linux")]
fn page_out_linux(pid: u32) -> Result<(), String> {
    let maps = std::fs::read_to_string(format!("/proc/{pid}/maps"))
        .map_err(|e| format!("read maps: {e}"))?;
    let ranges = parse_anon_rw_ranges(&maps);
    if ranges.is_empty() {
        return Ok(());
    }

    let pidfd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid as libc::pid_t, 0_i32) };
    if pidfd < 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let pidfd = pidfd as i32;

    let mut advised = 0u64;
    for r in ranges {
        let len = (r.end - r.start) as usize;
        let iov = libc::iovec {
            iov_base: r.start as *mut _,
            iov_len: len,
        };
        let rc = unsafe {
            libc::syscall(
                libc::SYS_process_madvise,
                pidfd,
                &iov as *const libc::iovec,
                1_usize,
                libc::MADV_PAGEOUT,
                0_u32,
            )
        };
        if rc >= 0 {
            advised = advised.saturating_add(len as u64);
        }
    }

    unsafe {
        libc::close(pidfd);
    }
    debug!(pid, advised, "process_madvise PAGEOUT done");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_anon_rw_and_skips_file_backed() {
        let maps = "\
55b8a8a00000-55b8a8c00000 rw-p 00000000 00:00 0\n\
7f8a00000000-7f8a00200000 r--p 00000000 08:01 12345 /usr/lib/libc.so.6\n\
7ffc00000000-7ffc00200000 rw-p 00000000 00:00 0 [stack]\n\
ffffffffff600000-ffffffffff601000 --xp 00000000 00:00 0 [vsyscall]\n";
        let ranges = parse_anon_rw_ranges(maps);
        assert_eq!(
            ranges,
            vec![
                AnonRange {
                    start: 0x55b8_a8a0_0000,
                    end: 0x55b8_a8c0_0000,
                },
                AnonRange {
                    start: 0x7ffc_0000_0000,
                    end: 0x7ffc_0020_0000,
                },
            ]
        );
    }
}
