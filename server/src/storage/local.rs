use super::{ByteStream, OpenResult, StorageError};
use futures_util::StreamExt;
use tokio::fs::OpenOptions;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom};
use tokio_util::io::ReaderStream;

fn available_space(path: &Path) -> u64 {
    fs2::available_space(path).unwrap_or(0)
}

pub struct LocalStorage {
    root: PathBuf,
    min_free_bytes: u64,
    space_check: Box<dyn Fn(&Path) -> u64 + Send + Sync>,
}

impl LocalStorage {
    pub fn new(root: PathBuf, min_free_bytes: u64) -> Result<Self, std::io::Error> {
        std::fs::create_dir_all(&root)?;
        Ok(Self {
            root,
            min_free_bytes,
            space_check: Box::new(available_space),
        })
    }

    pub fn with_space_checker(
        root: PathBuf,
        min_free_bytes: u64,
        check: Box<dyn Fn(&Path) -> u64 + Send + Sync>,
    ) -> Self {
        Self {
            root,
            min_free_bytes,
            space_check: check,
        }
    }

    pub fn path(&self, key: &str) -> PathBuf {
        self.root.join(key)
    }

    pub fn check_free(&self) -> Result<(), StorageError> {
        let free = (self.space_check)(&self.root);
        if free < self.min_free_bytes {
            return Err(StorageError::LowSpace(free));
        }
        Ok(())
    }

    pub async fn create(&self, key: &str) -> Result<(), StorageError> {
        File::create(self.path(key)).await?;
        Ok(())
    }

    pub async fn append(&self, key: &str, bytes: &[u8]) -> Result<(), StorageError> {
        let mut f = OpenOptions::new().append(true).open(self.path(key)).await?;
        f.write_all(bytes).await?;
        f.flush().await?;
        Ok(())
    }

    pub async fn delete(&self, key: &str) -> Result<(), StorageError> {
        match tokio::fs::remove_file(self.path(key)).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Durability point: fsync before the row flips to ready.
    pub async fn complete(&self, key: &str) -> Result<(), StorageError> {
        let f = OpenOptions::new().append(true).open(self.path(key)).await?;
        f.sync_all().await?;
        Ok(())
    }

    pub async fn open_range(
        &self,
        key: &str,
        range: Option<(u64, u64)>,
    ) -> Result<OpenResult, StorageError> {
        let mut f = File::open(self.path(key)).await?;
        let total = f.metadata().await?.len();
        if total == 0 {
            return Err(StorageError::NoSuchKey);
        }
        let (start, end) = match range {
            // Clamp both bounds into [0, total-1]; never allow start > end.
            Some((s, e)) => {
                let start = s.min(total - 1);
                let end = e.min(total - 1).max(start);
                (start, end)
            }
            None => (0, total - 1),
        };
        f.seek(SeekFrom::Start(start)).await?;
        let len = end - start + 1;
        let stream: ByteStream = Box::pin(
            ReaderStream::new(f.take(len)).map(|r| r.map_err(std::io::Error::from)),
        );
        Ok(OpenResult {
            total,
            start,
            end,
            stream,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;
    use tempfile::tempdir;

    fn ls(root: &std::path::Path) -> LocalStorage {
        LocalStorage::with_space_checker(root.to_path_buf(), 100, Box::new(|_| 1_000))
    }

    #[tokio::test]
    async fn append_create_delete_and_complete() {
        let dir = tempdir().unwrap();
        let s = ls(dir.path());
        s.create("k1").await.unwrap();
        s.append("k1", b"hello ").await.unwrap();
        s.append("k1", b"world").await.unwrap();
        s.complete("k1").await.unwrap();
        let content = tokio::fs::read(s.path("k1")).await.unwrap();
        assert_eq!(content, b"hello world");
        s.delete("k1").await.unwrap();
        assert!(!s.path("k1").exists());
    }

    #[tokio::test]
    async fn free_space_check_rejects_low() {
        let dir = tempdir().unwrap();
        let s = LocalStorage::with_space_checker(dir.path().to_path_buf(), 1000, Box::new(|_| 500));
        assert!(matches!(s.check_free(), Err(StorageError::LowSpace(500))));
    }

    #[tokio::test]
    async fn open_range_full_file() {
        let dir = tempdir().unwrap();
        let s = ls(dir.path());
        s.create("k2").await.unwrap();
        s.append("k2", b"0123456789").await.unwrap();
        let res = s.open_range("k2", None).await.unwrap();
        assert_eq!(res.total, 10);
        assert_eq!(res.start, 0);
        assert_eq!(res.end, 9);
        let bytes = res.stream.map(|r| r.unwrap()).collect::<Vec<_>>().await.concat();
        assert_eq!(bytes, b"0123456789");
    }

    #[tokio::test]
    async fn open_range_streams_window() {
        let dir = tempdir().unwrap();
        let s = ls(dir.path());
        s.create("k3").await.unwrap();
        s.append("k3", b"0123456789").await.unwrap();
        let res = s.open_range("k3", Some((2, 5))).await.unwrap();
        assert_eq!(res.start, 2);
        assert_eq!(res.end, 5);
        let bytes = res.stream.map(|r| r.unwrap()).collect::<Vec<_>>().await.concat();
        assert_eq!(bytes, b"2345");
    }
}
