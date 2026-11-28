use command_group::AsyncGroupChild;
#[cfg(unix)]
use tokio::time::Duration;

pub async fn kill_process_group(child: &mut AsyncGroupChild) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        // Use command_group's UnixChildExt::signal() which calls killpg()
        // with the pgid captured at spawn time. This works even after the
        // group leader has exited, unlike getpgid() which would fail.
        use command_group::{Signal, UnixChildExt};

        for sig in [Signal::SIGINT, Signal::SIGTERM, Signal::SIGKILL] {
            tracing::info!("Sending {:?} to process group", sig);
            if let Err(e) = child.signal(sig) {
                // break if the group does not exist anymore
                if e.raw_os_error() == Some(nix::libc::ESRCH) {
                    break;
                }
                tracing::warn!("Failed to send signal {:?} to process group: {}", sig, e);
            }
            if sig != Signal::SIGKILL {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }

    let _ = child.kill().await;
    let _ = child.wait().await;
    Ok(())
}

/// Kill an orphaned process group left behind by a previous server instance
/// (e.g. after a crash or SIGKILL during a deploy). Returns true if a signal
/// was delivered to the group.
///
/// `expected_age_secs` is the age of the execution's DB record. The group is
/// only signalled when a live process group leader with a matching pgid has
/// roughly that age, which guards against killing an unrelated process that
/// happens to reuse the pid.
#[cfg(unix)]
pub async fn kill_orphan_process_group(pgid: i32, expected_age_secs: i64) -> bool {
    if !process_group_leader_matches(pgid, expected_age_secs).await {
        return false;
    }
    kill_process_group_by_pgid(pgid).await
}

/// Whether any process in the group is still alive.
#[cfg(unix)]
pub fn process_group_alive(pgid: i32) -> bool {
    use nix::{sys::signal::killpg, unistd::Pid};
    killpg(Pid::from_raw(pgid), None).is_ok()
}

/// Whether the group has a member that can still run (or is blocked in I/O):
/// any process in it that is not a zombie. Unlike [`process_group_alive`],
/// an exited-but-unreaped orphan (a zombie its reaper never collected) does
/// not count. Linux reads `/proc`; elsewhere falls back to
/// [`process_group_alive`].
#[cfg(unix)]
pub fn process_group_has_running_members(pgid: i32) -> bool {
    #[cfg(target_os = "linux")]
    {
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return process_group_alive(pgid);
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if !name.bytes().all(|b| b.is_ascii_digit()) {
                continue;
            }
            let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else {
                continue;
            };
            // `pid (comm) state ppid pgrp …`; `comm` may contain spaces and
            // parentheses, so split after the last `)`.
            let Some((_, rest)) = stat.rsplit_once(") ") else {
                continue;
            };
            let mut fields = rest.split_whitespace();
            let (Some(state), Some(_ppid), Some(pgrp)) =
                (fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            if state != "Z" && state != "X" && pgrp.parse::<i32>() == Ok(pgid) {
                return true;
            }
        }
        false
    }
    #[cfg(not(target_os = "linux"))]
    {
        process_group_alive(pgid)
    }
}

/// Send SIGTERM then SIGKILL to a process group identified only by its pgid
/// (e.g. one adopted from a previous server instance, with no child handle).
/// Returns true if a signal was delivered.
#[cfg(unix)]
pub async fn kill_process_group_by_pgid(pgid: i32) -> bool {
    use nix::{
        errno::Errno,
        sys::signal::{Signal, killpg},
        unistd::Pid,
    };

    let pid = Pid::from_raw(pgid);
    let mut signalled = false;
    for sig in [Signal::SIGTERM, Signal::SIGKILL] {
        tracing::info!("Sending {:?} to process group {}", sig, pgid);
        match killpg(pid, sig) {
            Ok(()) => signalled = true,
            Err(Errno::ESRCH) => break,
            Err(e) => {
                tracing::warn!("Failed to send {:?} to process group {}: {}", sig, pgid, e);
            }
        }
        if sig != Signal::SIGKILL {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
    signalled
}

/// SIGKILL every process in group `pgid` at once, without the SIGTERM grace
/// period of [`kill_process_group_by_pgid`]. For a child that was spawned with
/// `process_group(0)` and has **not yet been reaped**, so the pgid cannot have
/// been recycled. Returns whether a signal was delivered.
#[cfg(unix)]
pub fn kill_process_group_now(pgid: u32) -> bool {
    use nix::{
        sys::signal::{Signal, killpg},
        unistd::Pid,
    };
    let Ok(pgid) = i32::try_from(pgid) else {
        return false;
    };
    killpg(Pid::from_raw(pgid), Signal::SIGKILL).is_ok()
}

/// Check that the process with the given pid is still the leader of its own
/// process group and started around when the execution record was created
/// (guards against acting on a recycled pid).
#[cfg(unix)]
pub async fn process_group_leader_matches(pgid: i32, expected_age_secs: i64) -> bool {
    let output = match tokio::process::Command::new("ps")
        .args(["-o", "pgid=,etime=", "-p", &pgid.to_string()])
        .output()
        .await
    {
        Ok(output) => output,
        Err(e) => {
            tracing::warn!(
                "Failed to run ps to verify orphaned process {}: {}",
                pgid,
                e
            );
            return false;
        }
    };
    if !output.status.success() {
        // Process no longer exists
        return false;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut fields = stdout.split_whitespace();
    let (Some(ps_pgid), Some(etime)) = (fields.next(), fields.next()) else {
        return false;
    };
    if ps_pgid.parse::<i32>() != Ok(pgid) {
        // The pid was recycled by a process in a different group
        return false;
    }
    let Some(age_secs) = parse_etime_seconds(etime) else {
        return false;
    };

    // The child was spawned moments after its DB record was created (spawn has
    // a 30s timeout), so a genuine orphan is slightly younger than the record.
    // A recycled pid belongs to a process started after the orphan died and
    // would be much younger.
    let age_gap_secs = expected_age_secs - age_secs as i64;
    (-60..=300).contains(&age_gap_secs)
}

/// Parse a `ps` etime value ("mm:ss", "hh:mm:ss" or "dd-hh:mm:ss") to seconds.
#[cfg(unix)]
fn parse_etime_seconds(etime: &str) -> Option<u64> {
    let (days, rest) = match etime.split_once('-') {
        Some((days, rest)) => (days.parse::<u64>().ok()?, rest),
        None => (0, etime),
    };
    let fields = rest
        .split(':')
        .map(|f| f.parse::<u64>().ok())
        .collect::<Option<Vec<_>>>()?;
    let (hours, minutes, seconds) = match fields[..] {
        [minutes, seconds] => (0, minutes, seconds),
        [hours, minutes, seconds] => (hours, minutes, seconds),
        _ => return None,
    };
    Some(((days * 24 + hours) * 60 + minutes) * 60 + seconds)
}

#[cfg(all(test, unix))]
mod tests {
    use super::parse_etime_seconds;
    use crate::command_ext::GroupSpawnNoWindowExt;

    #[tokio::test]
    async fn detects_and_kills_live_process_group() {
        let mut child = tokio::process::Command::new("sleep")
            .arg("30")
            .group_spawn_no_window()
            .expect("spawn sleep");
        let pgid = child.id().expect("child pid") as i32;

        assert!(super::process_group_alive(pgid));
        assert!(super::kill_process_group_by_pgid(pgid).await);

        // Reap the child so the group is fully gone, then verify
        let _ = child.wait().await;
        assert!(!super::process_group_alive(pgid));
    }

    #[test]
    fn parses_minutes_and_seconds() {
        assert_eq!(parse_etime_seconds("05:42"), Some(5 * 60 + 42));
    }

    #[test]
    fn parses_hours_minutes_seconds() {
        assert_eq!(
            parse_etime_seconds("03:05:42"),
            Some(3 * 3600 + 5 * 60 + 42)
        );
    }

    #[test]
    fn parses_days_prefix() {
        assert_eq!(
            parse_etime_seconds("2-03:05:42"),
            Some(2 * 86400 + 3 * 3600 + 5 * 60 + 42)
        );
    }

    #[test]
    fn rejects_malformed_values() {
        assert_eq!(parse_etime_seconds(""), None);
        assert_eq!(parse_etime_seconds("42"), None);
        assert_eq!(parse_etime_seconds("a:b"), None);
        assert_eq!(parse_etime_seconds("1:2:3:4"), None);
    }
}

#[cfg(all(test, target_os = "linux"))]
mod group_member_tests {
    use std::os::unix::process::CommandExt;

    use super::*;

    #[test]
    fn running_members_are_seen_and_a_reaped_group_is_not() {
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .process_group(0)
            .spawn()
            .unwrap();
        let pgid = child.id() as i32;
        assert!(process_group_has_running_members(pgid));
        kill_process_group_now(child.id());
        child.wait().unwrap();
        assert!(!process_group_has_running_members(pgid));
    }

    #[test]
    fn a_zombie_is_not_a_running_member() {
        let mut child = std::process::Command::new("true")
            .process_group(0)
            .spawn()
            .unwrap();
        let pgid = child.id() as i32;
        // Exited but not yet reaped: a zombie.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while std::fs::read_to_string(format!("/proc/{pgid}/stat"))
            .is_ok_and(|s| !s.rsplit_once(") ").is_some_and(|(_, r)| r.starts_with('Z')))
        {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(process_group_alive(pgid), "signal 0 still sees the zombie");
        assert!(!process_group_has_running_members(pgid));
        child.wait().unwrap();
    }
}
