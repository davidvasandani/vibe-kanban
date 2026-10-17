//! Extension traits to suppress console windows on Windows.
//!
//! On Windows, spawned child processes open a visible console window by
//! default.  Call `.no_window()` before `.spawn()` or `.output()` to set
//! the `CREATE_NO_WINDOW` creation flag and prevent this.
//!
//! On non-Windows platforms the methods are no-ops.

use command_group::{AsyncCommandGroup, AsyncGroupChild};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Adds a `.no_window()` builder method that suppresses the console window
/// on Windows.  No-op on other platforms.
pub trait NoWindowExt {
    fn no_window(&mut self) -> &mut Self;
}

impl NoWindowExt for std::process::Command {
    #[cfg(windows)]
    fn no_window(&mut self) -> &mut Self {
        use std::os::windows::process::CommandExt;
        self.creation_flags(CREATE_NO_WINDOW)
    }

    #[cfg(not(windows))]
    fn no_window(&mut self) -> &mut Self {
        self
    }
}

impl NoWindowExt for tokio::process::Command {
    #[cfg(windows)]
    fn no_window(&mut self) -> &mut Self {
        use std::os::windows::process::CommandExt;
        self.creation_flags(CREATE_NO_WINDOW)
    }

    #[cfg(not(windows))]
    fn no_window(&mut self) -> &mut Self {
        self
    }
}

/// Adds a `.group_spawn_no_window()` helper for command-group spawns that
/// suppresses the console window on Windows. No-op on other platforms.
pub trait GroupSpawnNoWindowExt {
    fn group_spawn_no_window(&mut self) -> std::io::Result<AsyncGroupChild>;
}

impl GroupSpawnNoWindowExt for tokio::process::Command {
    fn group_spawn_no_window(&mut self) -> std::io::Result<AsyncGroupChild> {
        place_in_spawn_cgroup(self);
        let mut group = self.group();
        #[cfg(windows)]
        group.creation_flags(CREATE_NO_WINDOW);
        group.spawn()
    }
}

/// `cgroup.procs` of the cgroup every child spawned through
/// [`GroupSpawnNoWindowExt`] joins before `exec`. Set once, by the cluster
/// worker, so agents are charged to its limited `jobs` cgroup from their first
/// page instead of to the worker's own control-plane cgroup.
#[cfg(unix)]
static SPAWN_CGROUP_PROCS: std::sync::OnceLock<std::ffi::CString> = std::sync::OnceLock::new();

/// Register the cgroup (by its `cgroup.procs` path) that spawned children
/// join. Returns `false` if one is already registered or the path is invalid.
#[cfg(unix)]
pub fn set_spawn_cgroup(procs: &std::path::Path) -> bool {
    use std::os::unix::ffi::OsStrExt;

    std::ffi::CString::new(procs.as_os_str().as_bytes())
        .is_ok_and(|procs| SPAWN_CGROUP_PROCS.set(procs).is_ok())
}

/// Make `command`'s child join the registered spawn cgroup between `fork` and
/// `exec`. A no-op when none is registered. Failure to join is not a spawn
/// failure: the worker's sweeper still moves the process afterwards.
#[cfg(unix)]
pub fn place_in_spawn_cgroup(command: &mut tokio::process::Command) {
    let Some(procs) = SPAWN_CGROUP_PROCS.get() else {
        return;
    };
    // SAFETY: after `fork` only async-signal-safe calls are allowed. The hook
    // uses open/write/close on a `'static` C string and allocates nothing.
    unsafe {
        command.pre_exec(move || {
            let fd = libc::open(procs.as_ptr(), libc::O_WRONLY | libc::O_CLOEXEC);
            if fd >= 0 {
                // Writing "0" moves the calling (child) process.
                libc::write(fd, b"0".as_ptr().cast(), 1);
                libc::close(fd);
            }
            Ok(())
        });
    }
}

#[cfg(not(unix))]
pub fn place_in_spawn_cgroup(_command: &mut tokio::process::Command) {}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    /// The hook runs in the child between fork and exec, so the write it makes
    /// is observable in the registered file once the child has started. A
    /// regular file stands in for `cgroup.procs`; the kernel reads "0" as
    /// "the writing process".
    #[tokio::test]
    async fn spawned_children_join_the_registered_cgroup_before_exec() {
        let dir = tempfile::tempdir().unwrap();
        let procs = dir.path().join("cgroup.procs");
        std::fs::write(&procs, "").unwrap();
        assert!(set_spawn_cgroup(&procs));
        assert!(
            !set_spawn_cgroup(&procs),
            "registration is once per process"
        );

        let mut child = tokio::process::Command::new("true")
            .group_spawn_no_window()
            .unwrap();
        child.wait().await.unwrap();

        assert_eq!(std::fs::read_to_string(&procs).unwrap(), "0");
    }
}
