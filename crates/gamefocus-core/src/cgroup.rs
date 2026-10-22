//! Resolve Linux cgroup paths to Docker / Podman / systemd-user targets.

use std::process::Command;

/// Supervisor target inferred from `/proc/<pid>/cgroup`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CgroupTarget {
    Docker { id: String },
    Podman { id: String },
    SystemdUser { unit: String },
}

/// Parse cgroup file contents into a best-effort actionable target.
pub fn parse_cgroup_target(cgroup: &str) -> Option<CgroupTarget> {
    for line in cgroup.lines() {
        let path = line_path(line)?;
        if let Some(id) = extract_after(path, "docker-") {
            let id = id.trim_end_matches(".scope").to_string();
            if !id.is_empty() {
                return Some(CgroupTarget::Docker { id });
            }
        }
        if let Some(id) = extract_docker_slash(path) {
            return Some(CgroupTarget::Docker { id });
        }
        if let Some(id) = extract_after(path, "libpod-") {
            let id = id.trim_end_matches(".scope").to_string();
            if !id.is_empty() {
                return Some(CgroupTarget::Podman { id });
            }
        }
    }
    // Prefer systemd user unit on the unified hierarchy line.
    for line in cgroup.lines() {
        let path = line_path(line)?;
        if path.contains("user@") || path.contains("user.slice") {
            if let Some(unit) = last_service_unit(path) {
                return Some(CgroupTarget::SystemdUser { unit });
            }
        }
    }
    None
}

fn line_path(line: &str) -> Option<&str> {
    // "0::/system.slice/docker-abc.scope" or "12:pids:/docker/abc"
    let path = line.split(':').next_back()?.trim();
    if path.is_empty() {
        return None;
    }
    Some(path)
}

fn extract_after<'a>(path: &'a str, marker: &str) -> Option<&'a str> {
    let idx = path.find(marker)?;
    Some(&path[idx + marker.len()..])
}

fn extract_docker_slash(path: &str) -> Option<String> {
    let idx = path.find("/docker/")?;
    let rest = &path[idx + "/docker/".len()..];
    let id = rest.split('/').next()?.trim();
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

fn last_service_unit(path: &str) -> Option<String> {
    path.split('/')
        .rev()
        .find(|s| s.ends_with(".service") && !s.starts_with("user@"))
        .map(|s| s.to_string())
}

/// Human label for warnings, e.g. `Docker: life-copilot-llama-server`.
pub fn format_cgroup_label(target: &CgroupTarget) -> String {
    match target {
        CgroupTarget::Docker { id } => {
            let name = inspect_container_name("docker", id).unwrap_or_else(|| short_id(id));
            format!("Docker: {name}")
        }
        CgroupTarget::Podman { id } => {
            let name = inspect_container_name("podman", id).unwrap_or_else(|| short_id(id));
            format!("Podman: {name}")
        }
        CgroupTarget::SystemdUser { unit } => format!("systemd-user: {unit}"),
    }
}

fn short_id(id: &str) -> String {
    id.chars().take(12).collect()
}

fn inspect_container_name(engine: &str, id: &str) -> Option<String> {
    let out = Command::new(engine)
        .args(["inspect", "--format", "{{.Name}}", id])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let name = String::from_utf8_lossy(&out.stdout).trim().trim_start_matches('/').to_string();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// Read `/proc/<pid>/cgroup` and format a label when possible (Linux only).
pub fn resolve_pid_supervisor_label(pid: u32) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let text = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).ok()?;
        let target = parse_cgroup_target(&text)?;
        Some(format_cgroup_label(&target))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_docker_scope() {
        let cgroup = "0::/system.slice/docker-d54564359db3dddb713a289251c5c6679ccbd74bcf0e6ba338fd83340b95b200.scope\n";
        assert_eq!(
            parse_cgroup_target(cgroup),
            Some(CgroupTarget::Docker {
                id: "d54564359db3dddb713a289251c5c6679ccbd74bcf0e6ba338fd83340b95b200".into()
            })
        );
    }

    #[test]
    fn parses_docker_slash_path() {
        let cgroup = "12:pids:/docker/abcdef0123456789\n";
        assert_eq!(
            parse_cgroup_target(cgroup),
            Some(CgroupTarget::Docker {
                id: "abcdef0123456789".into()
            })
        );
    }

    #[test]
    fn parses_libpod_scope() {
        let cgroup =
            "0::/user.slice/user-1000.slice/user@1000.service/app.slice/libpod-deadbeefcafe.scope\n";
        assert_eq!(
            parse_cgroup_target(cgroup),
            Some(CgroupTarget::Podman {
                id: "deadbeefcafe".into()
            })
        );
    }

    #[test]
    fn parses_systemd_user_unit() {
        let cgroup =
            "0::/user.slice/user-1000.slice/user@1000.service/app.slice/ollama-ailocal.service\n";
        assert_eq!(
            parse_cgroup_target(cgroup),
            Some(CgroupTarget::SystemdUser {
                unit: "ollama-ailocal.service".into()
            })
        );
    }

    #[test]
    fn docker_wins_over_systemd_when_both_present() {
        let cgroup = "\
12:pids:/docker/abc123
0::/user.slice/user-1000.slice/user@1000.service/app.slice/something.service
";
        assert!(matches!(
            parse_cgroup_target(cgroup),
            Some(CgroupTarget::Docker { .. })
        ));
    }

    #[test]
    fn formats_systemd_label_without_inspect() {
        let label = format_cgroup_label(&CgroupTarget::SystemdUser {
            unit: "ollama.service".into(),
        });
        assert_eq!(label, "systemd-user: ollama.service");
    }
}
