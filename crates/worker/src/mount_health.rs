use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

use cluster_protocol::{MountFailureReason, MountHealth, MountProbe};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountIdentity {
    pub mount_point: PathBuf,
    pub export: String,
    pub filesystem_id: String,
    pub read_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ownership {
    pub uid: u32,
    pub gid: u32,
}

/// Injectable boundary around host mount and filesystem inspection.
pub trait MountInspector {
    fn metadata_ownership(&self, path: &Path) -> io::Result<Ownership>;
    fn mount_identity(&self, path: &Path) -> io::Result<Option<MountIdentity>>;
    fn filesystem_id(&self, path: &Path) -> io::Result<String>;
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;
    fn verify_writable(&self, path: &Path) -> io::Result<()>;
}

#[derive(Debug, Clone)]
pub struct MountHealthChecker {
    shared_root: PathBuf,
    expected_export: String,
    expected_ownership: Ownership,
}

impl MountHealthChecker {
    pub fn new(
        shared_root: impl Into<PathBuf>,
        expected_export: impl Into<String>,
        expected_uid: u32,
        expected_gid: u32,
    ) -> Self {
        Self {
            shared_root: shared_root.into(),
            expected_export: expected_export.into(),
            expected_ownership: Ownership {
                uid: expected_uid,
                gid: expected_gid,
            },
        }
    }

    pub fn check(&self, probe: &MountProbe, inspector: &impl MountInspector) -> MountHealth {
        self.try_check(probe, inspector)
            .unwrap_or_else(|failure| MountHealth::Unhealthy {
                reason: failure.reason,
                message: failure.message,
            })
    }

    fn try_check(
        &self,
        probe: &MountProbe,
        inspector: &impl MountInspector,
    ) -> Result<MountHealth, Failure> {
        let ownership = inspector
            .metadata_ownership(&self.shared_root)
            .map_err(|error| Failure::io("inspect shared root", error))?;
        if ownership != self.expected_ownership {
            return Err(Failure::new(
                MountFailureReason::OwnershipMismatch,
                format!(
                    "shared root is owned by {}:{}, expected {}:{}",
                    ownership.uid,
                    ownership.gid,
                    self.expected_ownership.uid,
                    self.expected_ownership.gid
                ),
            ));
        }

        let mount = inspector
            .mount_identity(&self.shared_root)
            .map_err(|error| Failure::io("inspect mount table", error))?
            .ok_or_else(|| {
                Failure::new(
                    MountFailureReason::LocalFallback,
                    "shared root is not backed by a mounted filesystem",
                )
            })?;
        if mount.export != self.expected_export {
            return Err(Failure::new(
                MountFailureReason::LocalFallback,
                format!(
                    "shared root export is {:?}, expected {:?}",
                    mount.export, self.expected_export
                ),
            ));
        }
        let actual_filesystem = inspector
            .filesystem_id(&self.shared_root)
            .map_err(|error| Failure::io("inspect filesystem identity", error))?;
        if actual_filesystem != mount.filesystem_id {
            return Err(Failure::new(
                MountFailureReason::WrongFilesystem,
                format!(
                    "mount table filesystem {} does not match path filesystem {}",
                    mount.filesystem_id, actual_filesystem
                ),
            ));
        }
        if mount.read_only {
            return Err(Failure::new(
                MountFailureReason::ReadOnly,
                "shared root mount is read-only",
            ));
        }
        inspector
            .verify_writable(&self.shared_root)
            .map_err(|error| {
                Failure::new(
                    MountFailureReason::ReadOnly,
                    format!("shared root is not writable: {error}"),
                )
            })?;

        let relative_probe = Path::new(&probe.relative_path);
        if relative_probe.is_absolute()
            || relative_probe
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(Failure::new(
                MountFailureReason::ProbeNotVisible,
                "coordinator probe path is not a safe relative path",
            ));
        }
        let probe_path = self.shared_root.join(relative_probe);
        let bytes = inspector.read(&probe_path).map_err(|error| {
            Failure::new(
                MountFailureReason::ProbeNotVisible,
                format!("coordinator probe is not visible: {error}"),
            )
        })?;
        let digest = format!("{:x}", Sha256::digest(&bytes));
        if digest != probe.expected_contents_digest {
            return Err(Failure::new(
                MountFailureReason::ProbeNotVisible,
                format!(
                    "coordinator probe digest is {digest}, expected {}",
                    probe.expected_contents_digest
                ),
            ));
        }
        let probe_ownership = inspector
            .metadata_ownership(&probe_path)
            .map_err(|error| Failure::io("inspect probe ownership", error))?;
        if probe_ownership != self.expected_ownership {
            return Err(Failure::new(
                MountFailureReason::OwnershipMismatch,
                format!(
                    "probe is owned by {}:{}, expected {}:{}",
                    probe_ownership.uid,
                    probe_ownership.gid,
                    self.expected_ownership.uid,
                    self.expected_ownership.gid
                ),
            ));
        }

        Ok(MountHealth::Healthy {
            // Report the configured export identity, which is stable across
            // hosts. Linux device major/minor values are only meaningful for
            // verifying this host's mount-table entry against this path.
            filesystem_id: mount.export,
            probe_id: probe.id.clone(),
        })
    }
}

struct Failure {
    reason: MountFailureReason,
    message: String,
}

impl Failure {
    fn new(reason: MountFailureReason, message: impl Into<String>) -> Self {
        Self {
            reason,
            message: message.into(),
        }
    }

    fn io(operation: &str, error: io::Error) -> Self {
        let reason = if error.kind() == io::ErrorKind::NotFound {
            MountFailureReason::Missing
        } else {
            MountFailureReason::IoError
        };
        Self::new(reason, format!("failed to {operation}: {error}"))
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemMountInspector;

impl MountInspector for SystemMountInspector {
    fn metadata_ownership(&self, path: &Path) -> io::Result<Ownership> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = fs::metadata(path)?;
            Ok(Ownership {
                uid: metadata.uid(),
                gid: metadata.gid(),
            })
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "filesystem ownership checks require Unix",
            ))
        }
    }

    fn mount_identity(&self, path: &Path) -> io::Result<Option<MountIdentity>> {
        // macOS has no /proc: ask the kernel which mount backs the path.
        #[cfg(target_os = "macos")]
        {
            macos::mount_identity(path)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let canonical = fs::canonicalize(path)?;
            let contents = fs::read_to_string("/proc/self/mountinfo")?;
            Ok(parse_mountinfo(&contents)
                .into_iter()
                .filter(|entry| canonical.starts_with(&entry.mount_point))
                .max_by_key(|entry| entry.mount_point.as_os_str().len()))
        }
    }

    fn filesystem_id(&self, path: &Path) -> io::Result<String> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Ok(device_id(fs::metadata(path)?.dev()))
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "filesystem identity checks require Unix",
            ))
        }
    }

    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        fs::read(path)
    }

    fn verify_writable(&self, path: &Path) -> io::Result<()> {
        let test_path = path.join(format!(".vk-write-probe-{}", Uuid::new_v4()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&test_path)?;
        file.write_all(b"mount-health")?;
        file.sync_all()?;
        drop(file);
        fs::remove_file(test_path)
    }
}

#[cfg_attr(target_os = "macos", allow(dead_code))]
fn parse_mountinfo(contents: &str) -> Vec<MountIdentity> {
    contents
        .lines()
        .filter_map(|line| {
            let (before, after) = line.split_once(" - ")?;
            let fields: Vec<_> = before.split_whitespace().collect();
            let trailing: Vec<_> = after.split_whitespace().collect();
            if fields.len() < 6 || trailing.len() < 2 {
                return None;
            }
            Some(MountIdentity {
                mount_point: PathBuf::from(unescape_mount_field(fields[4])),
                export: unescape_mount_field(trailing[1]),
                filesystem_id: fields[2].to_owned(),
                read_only: fields[5].split(',').any(|option| option == "ro"),
            })
        })
        .collect()
}

fn unescape_mount_field(value: &str) -> String {
    value
        .replace("\\040", " ")
        .replace("\\011", "\t")
        .replace("\\012", "\n")
        .replace("\\134", "\\")
}

/// Device identity in the form the Linux mount table uses (`major:minor`), so a
/// path's device can be compared with its mount's.
#[cfg(unix)]
fn device_id(dev: u64) -> String {
    format!("{}:{}", linux_major(dev), linux_minor(dev))
}

/// Mount identity as reported by `statfs(2)` on macOS, kept free of FFI so the
/// conversion is tested on every platform.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn mount_identity_from_statfs(
    mount_on: &[u8],
    mount_from: &[u8],
    read_only: bool,
    mount_point_dev: u64,
) -> MountIdentity {
    MountIdentity {
        mount_point: PathBuf::from(c_field(mount_on)),
        export: c_field(mount_from),
        filesystem_id: device_id(mount_point_dev),
        read_only,
    }
}

/// A fixed-size, NUL-terminated C string field as text.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn c_field(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

#[cfg(target_os = "macos")]
mod macos {
    use std::{
        ffi::CString,
        fs, io,
        os::unix::{ffi::OsStrExt, fs::MetadataExt},
        path::Path,
    };

    use super::{MountIdentity, mount_identity_from_statfs};

    /// The mount backing `path`, from `statfs(2)`: `f_mntonname` is where it is
    /// mounted (symlinks such as a synthetic `/srv` resolved), `f_mntfromname`
    /// is the source (`server:/export` for NFS).
    pub(super) fn mount_identity(path: &Path) -> io::Result<Option<MountIdentity>> {
        let canonical = fs::canonicalize(path)?;
        let c_path = CString::new(canonical.as_os_str().as_bytes())
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
        // SAFETY: `statfs` writes only into `buffer`, which is a properly sized,
        // zero-initialised `libc::statfs`, and reads the NUL-terminated path.
        let mut buffer: libc::statfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statfs(c_path.as_ptr(), &mut buffer) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let mount_on: Vec<u8> = buffer.f_mntonname.iter().map(|&c| c as u8).collect();
        let mount_from: Vec<u8> = buffer.f_mntfromname.iter().map(|&c| c as u8).collect();
        let read_only = buffer.f_flags & (libc::MNT_RDONLY as u32) != 0;
        let mount_point_dev = fs::metadata(super::c_field(&mount_on))?.dev();
        Ok(Some(mount_identity_from_statfs(
            &mount_on,
            &mount_from,
            read_only,
            mount_point_dev,
        )))
    }
}

#[cfg(target_os = "linux")]
fn linux_major(dev: u64) -> u64 {
    ((dev >> 8) & 0xfff) | ((dev >> 32) & !0xfff)
}

#[cfg(target_os = "linux")]
fn linux_minor(dev: u64) -> u64 {
    (dev & 0xff) | ((dev >> 12) & !0xff)
}

#[cfg(all(unix, not(target_os = "linux")))]
fn linux_major(dev: u64) -> u64 {
    dev
}

#[cfg(all(unix, not(target_os = "linux")))]
fn linux_minor(_dev: u64) -> u64 {
    0
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, collections::HashMap};

    use super::*;

    struct FixtureInspector {
        ownership: Ownership,
        probe_ownership: Ownership,
        mount: Option<MountIdentity>,
        filesystem_id: String,
        files: HashMap<PathBuf, Vec<u8>>,
        writable: bool,
        write_checks: Cell<usize>,
    }

    impl MountInspector for FixtureInspector {
        fn metadata_ownership(&self, path: &Path) -> io::Result<Ownership> {
            if path == Path::new("/shared") {
                Ok(self.ownership)
            } else {
                Ok(self.probe_ownership)
            }
        }
        fn mount_identity(&self, _path: &Path) -> io::Result<Option<MountIdentity>> {
            Ok(self.mount.clone())
        }
        fn filesystem_id(&self, _path: &Path) -> io::Result<String> {
            Ok(self.filesystem_id.clone())
        }
        fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
            self.files
                .get(path)
                .cloned()
                .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
        }
        fn verify_writable(&self, _path: &Path) -> io::Result<()> {
            self.write_checks.set(self.write_checks.get() + 1);
            self.writable
                .then_some(())
                .ok_or_else(|| io::Error::from(io::ErrorKind::PermissionDenied))
        }
    }

    fn fixture() -> (MountHealthChecker, MountProbe, FixtureInspector) {
        let contents = b"coordinator challenge".to_vec();
        let digest = format!("{:x}", Sha256::digest(&contents));
        let ownership = Ownership {
            uid: 1000,
            gid: 100,
        };
        (
            MountHealthChecker::new("/shared", "server:/shared", 1000, 100),
            MountProbe {
                id: "probe-7".into(),
                relative_path: ".probes/probe-7".into(),
                expected_contents_digest: digest,
            },
            FixtureInspector {
                ownership,
                probe_ownership: ownership,
                mount: Some(MountIdentity {
                    mount_point: "/shared".into(),
                    export: "server:/shared".into(),
                    filesystem_id: "0:42".into(),
                    read_only: false,
                }),
                filesystem_id: "0:42".into(),
                files: HashMap::from([(PathBuf::from("/shared/.probes/probe-7"), contents)]),
                writable: true,
                write_checks: Cell::new(0),
            },
        )
    }

    fn reason(health: MountHealth) -> MountFailureReason {
        match health {
            MountHealth::Unhealthy { reason, .. } => reason,
            MountHealth::Healthy { .. } => panic!("expected unhealthy mount"),
        }
    }

    #[test]
    fn statfs_fields_become_a_mount_identity() {
        let mut mount_on = [0u8; 32];
        let mount_point = b"/System/Volumes/Data/srv";
        mount_on[..mount_point.len()].copy_from_slice(mount_point);
        let mut mount_from = [0u8; 64];
        let export = b"172.16.0.99:/var/nfs/shared/VibeKanban";
        mount_from[..export.len()].copy_from_slice(export);

        let identity = mount_identity_from_statfs(&mount_on, &mount_from, false, 42);
        assert_eq!(
            identity.mount_point,
            PathBuf::from("/System/Volumes/Data/srv")
        );
        assert_eq!(identity.export, "172.16.0.99:/var/nfs/shared/VibeKanban");
        assert_eq!(identity.filesystem_id, device_id(42));
        assert!(!identity.read_only);
        assert!(mount_identity_from_statfs(&mount_on, &mount_from, true, 42).read_only);
    }

    #[test]
    fn c_field_stops_at_the_first_nul_and_tolerates_none() {
        assert_eq!(c_field(b"abc\0def"), "abc");
        assert_eq!(c_field(b"abc"), "abc");
        assert_eq!(c_field(b"\0"), "");
    }

    #[test]
    fn accepts_matching_export_filesystem_probe_and_ownership() {
        let (checker, probe, inspector) = fixture();
        assert_eq!(
            checker.check(&probe, &inspector),
            MountHealth::Healthy {
                filesystem_id: "server:/shared".into(),
                probe_id: "probe-7".into(),
            }
        );
        assert_eq!(inspector.write_checks.get(), 1);
    }

    #[test]
    fn rejects_local_fallback_and_wrong_filesystem() {
        let (checker, probe, mut inspector) = fixture();
        inspector.mount.as_mut().unwrap().export = "local-disk".into();
        assert_eq!(
            reason(checker.check(&probe, &inspector)),
            MountFailureReason::LocalFallback
        );

        inspector.mount.as_mut().unwrap().export = "server:/shared".into();
        inspector.filesystem_id = "0:99".into();
        assert_eq!(
            reason(checker.check(&probe, &inspector)),
            MountFailureReason::WrongFilesystem
        );
    }

    #[test]
    fn rejects_read_only_missing_or_changed_probe() {
        let (checker, probe, mut inspector) = fixture();
        inspector.mount.as_mut().unwrap().read_only = true;
        assert_eq!(
            reason(checker.check(&probe, &inspector)),
            MountFailureReason::ReadOnly
        );

        inspector.mount.as_mut().unwrap().read_only = false;
        inspector.files.clear();
        assert_eq!(
            reason(checker.check(&probe, &inspector)),
            MountFailureReason::ProbeNotVisible
        );

        inspector.files.insert(
            PathBuf::from("/shared/.probes/probe-7"),
            b"stale challenge".to_vec(),
        );
        assert_eq!(
            reason(checker.check(&probe, &inspector)),
            MountFailureReason::ProbeNotVisible
        );
    }

    #[test]
    fn rejects_root_and_probe_ownership_mismatches() {
        let (checker, probe, mut inspector) = fixture();
        inspector.ownership.uid = 2000;
        assert_eq!(
            reason(checker.check(&probe, &inspector)),
            MountFailureReason::OwnershipMismatch
        );

        inspector.ownership.uid = 1000;
        inspector.probe_ownership.gid = 200;
        assert_eq!(
            reason(checker.check(&probe, &inspector)),
            MountFailureReason::OwnershipMismatch
        );
    }

    #[test]
    fn mountinfo_fixture_selects_export_identity_and_options() {
        let entries = parse_mountinfo(
            "36 25 0:42 / /shared rw,relatime - nfs4 server:/shared rw,vers=4.2\n\
             37 36 0:43 / /shared/read-only ro,relatime - nfs4 server:/archive ro",
        );
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].export, "server:/shared");
        assert_eq!(entries[0].filesystem_id, "0:42");
        assert!(!entries[0].read_only);
        assert!(entries[1].read_only);
    }
}
