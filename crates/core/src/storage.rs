//! Filesystem-backed runtime object storage.
//!
//! Static frontend assets are embedded in the executable and are deliberately
//! read-only. Runtime objects therefore live below an application-owned root
//! and are addressed by opaque, validated keys rather than client paths.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use tokio::{fs, io};

/// A cloneable handle to the application's writable storage root.
#[derive(Clone, Debug)]
pub struct RuntimeStorage {
    root: Arc<PathBuf>,
}

impl RuntimeStorage {
    /// Configure a runtime storage root without touching the filesystem.
    ///
    /// [`Self::initialize`] performs the first write. This split keeps metadata
    /// export free of filesystem side effects.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: Arc::new(root.into()),
        }
    }

    /// Return the configured root.
    pub fn root(&self) -> &Path {
        self.root.as_path()
    }

    /// Create the root after the application has left metadata-export mode.
    pub async fn initialize(&self) -> Result<(), StorageError> {
        fs::create_dir_all(self.root()).await?;
        Ok(())
    }

    /// Resolve an object to `<root>/<namespace>/objects/<shard>/<key>`.
    ///
    /// Both segments accept only URL-safe opaque identifiers. They can never
    /// introduce a separator or traversal component.
    pub fn object_path(&self, namespace: &str, key: &str) -> Result<PathBuf, StorageError> {
        validate_segment("namespace", namespace, 1)?;
        validate_segment("object key", key, 2)?;
        let shard = &key[..2];
        Ok(self
            .root()
            .join(namespace)
            .join("objects")
            .join(shard)
            .join(key))
    }

    /// Copy an existing file into storage without overwriting another object.
    ///
    /// Callers publish their database reference only after this completes, so a
    /// partially copied object is never reachable through the application.
    pub async fn store_file(
        &self,
        namespace: &str,
        key: &str,
        source: impl AsRef<Path>,
    ) -> Result<PathBuf, StorageError> {
        let destination = self.object_path(namespace, key)?;
        let parent = destination
            .parent()
            .ok_or_else(|| StorageError::InvalidSegment {
                kind: "object key",
                value: key.to_string(),
            })?;
        fs::create_dir_all(parent).await?;

        let mut input = fs::File::open(source.as_ref()).await?;
        let mut output = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
            .await
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(StorageError::AlreadyExists(key.to_string()));
            }
            Err(error) => return Err(error.into()),
        };

        if let Err(error) = async {
            io::copy(&mut input, &mut output).await?;
            output.sync_all().await
        }
        .await
        {
            drop(output);
            if let Err(cleanup_error) = fs::remove_file(&destination).await {
                if cleanup_error.kind() != std::io::ErrorKind::NotFound {
                    tracing::warn!(
                        path = %destination.display(),
                        %cleanup_error,
                        "Could not remove an incomplete runtime object"
                    );
                }
            }
            return Err(error.into());
        }

        Ok(destination)
    }

    /// Open a stored object for streaming.
    pub async fn open_file(&self, namespace: &str, key: &str) -> Result<fs::File, StorageError> {
        let path = self.object_path(namespace, key)?;
        fs::File::open(path).await.map_err(StorageError::from)
    }

    /// Remove an object. Missing objects are already deleted and return false.
    pub async fn remove_file(&self, namespace: &str, key: &str) -> Result<bool, StorageError> {
        let path = self.object_path(namespace, key)?;
        match fs::remove_file(path).await {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }
}

fn validate_segment(
    kind: &'static str,
    value: &str,
    minimum_len: usize,
) -> Result<(), StorageError> {
    if value.len() < minimum_len
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(StorageError::InvalidSegment {
            kind,
            value: value.to_string(),
        });
    }
    Ok(())
}

/// Runtime object storage failure.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("invalid {kind} `{value}`")]
    InvalidSegment { kind: &'static str, value: String },
    #[error("runtime object `{0}` already exists")]
    AlreadyExists(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stores_opens_and_removes_a_sharded_object() {
        let root = tempfile::tempdir().unwrap();
        let source_dir = tempfile::tempdir().unwrap();
        let source = source_dir.path().join("source.bin");
        fs::write(&source, b"runtime bytes").await.unwrap();
        let storage = RuntimeStorage::new(root.path());

        storage.initialize().await.unwrap();
        let path = storage
            .store_file("media", "abcdef012345", &source)
            .await
            .unwrap();

        assert_eq!(
            path,
            root.path()
                .join("media")
                .join("objects")
                .join("ab")
                .join("abcdef012345")
        );
        assert_eq!(fs::read(path).await.unwrap(), b"runtime bytes");
        assert!(storage.remove_file("media", "abcdef012345").await.unwrap());
        assert!(!storage.remove_file("media", "abcdef012345").await.unwrap());
    }

    #[test]
    fn refuses_paths_and_short_shard_keys() {
        let storage = RuntimeStorage::new("storage");

        for namespace in ["", "../media", "media/files"] {
            assert!(storage.object_path(namespace, "abcdef").is_err());
        }
        for key in ["a", "../secret", "ab/cd", "ab.cd"] {
            assert!(storage.object_path("media", key).is_err());
        }
    }

    #[tokio::test]
    async fn never_overwrites_an_existing_object() {
        let root = tempfile::tempdir().unwrap();
        let source_dir = tempfile::tempdir().unwrap();
        let first = source_dir.path().join("first.bin");
        let second = source_dir.path().join("second.bin");
        fs::write(&first, b"first").await.unwrap();
        fs::write(&second, b"second").await.unwrap();
        let storage = RuntimeStorage::new(root.path());

        storage.store_file("media", "abcdef", first).await.unwrap();
        assert!(matches!(
            storage.store_file("media", "abcdef", second).await,
            Err(StorageError::AlreadyExists(_))
        ));
        assert_eq!(
            fs::read(storage.object_path("media", "abcdef").unwrap())
                .await
                .unwrap(),
            b"first"
        );
    }
}
