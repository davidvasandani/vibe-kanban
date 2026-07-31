use std::{
    fs, io,
    path::{Path, PathBuf},
};

use thiserror::Error;

/// Authorizes existing workspace paths against one canonical shared root.
#[derive(Debug, Clone)]
pub struct PathAuthority {
    shared_root: PathBuf,
}

#[derive(Debug, Error)]
pub enum PathAuthorityError {
    #[error("shared root must be an absolute path: {0}")]
    RelativeSharedRoot(PathBuf),
    #[error("workspace path must be absolute: {0}")]
    RelativeWorkspacePath(PathBuf),
    #[error("failed to resolve shared root {path}: {source}")]
    SharedRootUnavailable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to resolve workspace path {path}: {source}")]
    WorkspacePathUnavailable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("workspace path {path} resolves outside shared root {shared_root}")]
    OutsideSharedRoot { path: PathBuf, shared_root: PathBuf },
}

impl PathAuthority {
    pub fn new(shared_root: impl AsRef<Path>) -> Result<Self, PathAuthorityError> {
        let shared_root = shared_root.as_ref();
        if !shared_root.is_absolute() {
            return Err(PathAuthorityError::RelativeSharedRoot(
                shared_root.to_owned(),
            ));
        }

        let canonical_root = fs::canonicalize(shared_root).map_err(|source| {
            PathAuthorityError::SharedRootUnavailable {
                path: shared_root.to_owned(),
                source,
            }
        })?;

        Ok(Self {
            shared_root: canonical_root,
        })
    }

    pub fn shared_root(&self) -> &Path {
        &self.shared_root
    }

    /// Returns the canonical path when the existing target resolves within the
    /// configured shared root.
    pub fn authorize_workspace_path(
        &self,
        workspace_path: impl AsRef<Path>,
    ) -> Result<PathBuf, PathAuthorityError> {
        let workspace_path = workspace_path.as_ref();
        if !workspace_path.is_absolute() {
            return Err(PathAuthorityError::RelativeWorkspacePath(
                workspace_path.to_owned(),
            ));
        }

        let canonical_path = fs::canonicalize(workspace_path).map_err(|source| {
            PathAuthorityError::WorkspacePathUnavailable {
                path: workspace_path.to_owned(),
                source,
            }
        })?;

        if !canonical_path.starts_with(&self.shared_root) {
            return Err(PathAuthorityError::OutsideSharedRoot {
                path: canonical_path,
                shared_root: self.shared_root.clone(),
            });
        }

        Ok(canonical_path)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn authorizes_existing_workspace_and_returns_its_canonical_path() {
        let fixture = TempDir::new().unwrap();
        let shared_root = fixture.path().join("shared");
        let workspace = shared_root.join("projects").join("workspace");
        fs::create_dir_all(&workspace).unwrap();
        let authority = PathAuthority::new(&shared_root).unwrap();

        assert_eq!(
            authority.authorize_workspace_path(&workspace).unwrap(),
            fs::canonicalize(workspace).unwrap()
        );
    }

    #[test]
    fn rejects_relative_and_missing_workspace_paths() {
        let fixture = TempDir::new().unwrap();
        let shared_root = fixture.path().join("shared");
        fs::create_dir(&shared_root).unwrap();
        let authority = PathAuthority::new(&shared_root).unwrap();

        assert!(matches!(
            authority.authorize_workspace_path("workspace"),
            Err(PathAuthorityError::RelativeWorkspacePath(_))
        ));
        assert!(matches!(
            authority.authorize_workspace_path(shared_root.join("missing")),
            Err(PathAuthorityError::WorkspacePathUnavailable { .. })
        ));
    }

    #[test]
    fn rejects_paths_outside_the_shared_root() {
        let fixture = TempDir::new().unwrap();
        let shared_root = fixture.path().join("shared");
        let outside = fixture.path().join("outside");
        fs::create_dir(&shared_root).unwrap();
        fs::create_dir(&outside).unwrap();
        let authority = PathAuthority::new(&shared_root).unwrap();

        assert!(matches!(
            authority.authorize_workspace_path(outside),
            Err(PathAuthorityError::OutsideSharedRoot { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_that_escape_the_shared_root() {
        use std::os::unix::fs::symlink;

        let fixture = TempDir::new().unwrap();
        let shared_root = fixture.path().join("shared");
        let outside = fixture.path().join("outside");
        fs::create_dir(&shared_root).unwrap();
        fs::create_dir(&outside).unwrap();
        let escape = shared_root.join("escape");
        symlink(&outside, &escape).unwrap();
        let authority = PathAuthority::new(&shared_root).unwrap();

        assert!(matches!(
            authority.authorize_workspace_path(escape),
            Err(PathAuthorityError::OutsideSharedRoot { .. })
        ));
    }
}
