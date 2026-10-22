//! GPU VRAM preflight: warn when Focus cannot free CUDA/DirectX buffers.

use crate::profile::{Matcher, Profile, WhitelistEntry};
use std::path::Path;
use std::process::Command;
use tracing::warn;

/// Warn when a non-whitelisted consumer holds at least this much GPU memory.
pub const VRAM_WARN_MIB: u64 = 500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VramConsumer {
    pub pid: u32,
    pub process_name: String,
    pub used_mib: u64,
}

/// Parse `nvidia-smi --query-compute-apps=pid,process_name,used_gpu_memory --format=csv,noheader,nounits`.
///
/// Lines look like: `2281, /app/llama-server, 6526` or with `MiB` suffix if nounits omitted.
pub fn parse_nvidia_smi_compute_apps(csv: &str) -> Vec<VramConsumer> {
    let mut out = Vec::new();
    for line in csv.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("pid") {
            continue;
        }
        let parts: Vec<&str> = line.splitn(3, ',').map(|p| p.trim()).collect();
        if parts.len() < 3 {
            continue;
        }
        let Ok(pid) = parts[0].parse::<u32>() else {
            continue;
        };
        let process_name = parts[1].to_string();
        let used_mib = parse_mib(parts[2]);
        if used_mib == 0 && parts[2].is_empty() {
            continue;
        }
        out.push(VramConsumer {
            pid,
            process_name,
            used_mib,
        });
    }
    out
}

fn parse_mib(raw: &str) -> u64 {
    let cleaned = raw
        .trim()
        .trim_end_matches("MiB")
        .trim_end_matches("miB")
        .trim();
    cleaned.parse::<u64>().unwrap_or(0)
}

fn basename_of(process_name: &str) -> &str {
    Path::new(process_name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(process_name)
}

fn name_is_whitelisted(basename: &str, profile: &Profile) -> bool {
    let on_whitelist = profile.whitelist.iter().any(|entry| match entry {
        WhitelistEntry::ProcessName(pat) => crate::session::process_name_matches(basename, pat),
        _ => false,
    });
    let on_matchers = profile.matchers.iter().any(|m| match m {
        Matcher::ProcessName(pat) => crate::session::process_name_matches(basename, pat),
        _ => false,
    });
    on_whitelist || on_matchers
}

/// Consumers above threshold that are not protected by profile whitelist/matchers.
pub fn vram_warn_targets<'a>(
    consumers: &'a [VramConsumer],
    profile: &Profile,
    threshold_mib: u64,
) -> Vec<&'a VramConsumer> {
    consumers
        .iter()
        .filter(|c| c.used_mib >= threshold_mib)
        .filter(|c| !name_is_whitelisted(basename_of(&c.process_name), profile))
        .collect()
}

pub fn format_vram_warning(c: &VramConsumer, resolved: Option<&str>) -> String {
    let who = resolved.unwrap_or_else(|| basename_of(&c.process_name));
    format!(
        "PID {} ({}) is holding {} MiB VRAM. Suspending will NOT free GPU memory.",
        c.pid, who, c.used_mib
    )
}

/// Best-effort probe via `nvidia-smi`. Returns empty on any failure.
pub fn probe_nvidia_compute_apps() -> Vec<VramConsumer> {
    let output = Command::new("nvidia-smi")
        .args([
            "--query-compute-apps=pid,process_name,used_gpu_memory",
            "--format=csv,noheader,nounits",
        ])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            parse_nvidia_smi_compute_apps(&String::from_utf8_lossy(&out.stdout))
        }
        _ => Vec::new(),
    }
}

/// Log WARN lines for heavy non-whitelisted GPU consumers. Never fails the session.
pub fn warn_heavy_vram_users(profile: &Profile) {
    let consumers = probe_nvidia_compute_apps();
    for c in vram_warn_targets(&consumers, profile, VRAM_WARN_MIB) {
        let resolved = crate::cgroup::resolve_pid_supervisor_label(c.pid);
        warn!("{}", format_vram_warning(c, resolved.as_deref()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{BoostPolicy, FocusPolicy, Profile};

    fn empty_profile() -> Profile {
        Profile {
            name: "t".into(),
            description: String::new(),
            matchers: Vec::new(),
            park: Vec::new(),
            whitelist: Vec::new(),
            focus: FocusPolicy::default(),
            boost: BoostPolicy::default(),
        }
    }

    #[test]
    fn parses_nvidia_smi_csv_nounits() {
        let csv = "\
2281, /app/llama-server, 6526
4699, /usr/bin/kwin_wayland, 7
";
        let apps = parse_nvidia_smi_compute_apps(csv);
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0].pid, 2281);
        assert_eq!(apps[0].process_name, "/app/llama-server");
        assert_eq!(apps[0].used_mib, 6526);
        assert_eq!(apps[1].used_mib, 7);
    }

    #[test]
    fn parses_mib_suffix() {
        let csv = "1234, chrome, 900 MiB\n";
        let apps = parse_nvidia_smi_compute_apps(csv);
        assert_eq!(apps[0].used_mib, 900);
    }

    #[test]
    fn warns_only_above_threshold_and_not_whitelisted() {
        let mut profile = empty_profile();
        profile.whitelist.push(WhitelistEntry::ProcessName("steam".into()));
        let consumers = vec![
            VramConsumer {
                pid: 1,
                process_name: "/app/llama-server".into(),
                used_mib: 6526,
            },
            VramConsumer {
                pid: 2,
                process_name: "kwin_wayland".into(),
                used_mib: 7,
            },
            VramConsumer {
                pid: 3,
                process_name: "/home/x/.steam/steam".into(),
                used_mib: 800,
            },
        ];
        let hits = vram_warn_targets(&consumers, &profile, 500);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].pid, 1);
        let msg = format_vram_warning(hits[0], None);
        assert!(msg.contains("6526"));
        assert!(msg.contains("llama-server"));
        assert!(msg.contains("NOT free GPU memory"));
        let resolved = format_vram_warning(hits[0], Some("Docker: life-copilot-llama-server"));
        assert!(resolved.contains("Docker: life-copilot-llama-server"));
    }
}
