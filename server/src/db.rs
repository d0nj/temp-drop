use sqlx::sqlite::SqlitePool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UploadRow {
    pub id: String,
    pub name: String,
    pub size: i64,
    pub backend: String,
    pub storage_key: String,
    pub status: String,
    pub received_bytes: i64,
    pub part_count: i64,
    pub s3_upload_id: Option<String>,
    pub ttl_seconds: Option<i64>,
    pub max_downloads: Option<i64>,
    pub download_count: i64,
    pub upload_token: String,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub chunk_size: i64,
}

#[derive(Clone)]
pub struct Db {
    pool: SqlitePool,
}

impl Db {
    pub async fn open(path: &std::path::Path) -> Result<Self, sqlx::Error> {
        let opts = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .pragma("secure_delete", "ON");
        let pool = SqlitePool::connect_with(opts).await?;
        let db = Self { pool };
        db.init_schema().await?;
        Ok(db)
    }

    pub async fn open_in_memory() -> Result<Self, sqlx::Error> {
        let pool = SqlitePool::connect("sqlite::memory:").await?;
        let db = Self { pool };
        db.init_schema().await?;
        Ok(db)
    }

    async fn init_schema(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS uploads (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                size INTEGER NOT NULL DEFAULT 0,
                backend TEXT NOT NULL,
                storage_key TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                received_bytes INTEGER NOT NULL DEFAULT 0,
                part_count INTEGER NOT NULL DEFAULT 0,
                s3_upload_id TEXT,
                ttl_seconds INTEGER,
                max_downloads INTEGER,
                download_count INTEGER NOT NULL DEFAULT 0,
                upload_token TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER,
                chunk_size INTEGER NOT NULL DEFAULT 33554432
            )",
        )
        .execute(&self.pool)
        .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_uploads_expires ON uploads(expires_at)")
            .execute(&self.pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_uploads_status ON uploads(status)")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn create_upload(&self, row: &UploadRow) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO uploads (id, name, size, backend, storage_key, status, received_bytes, part_count, s3_upload_id, ttl_seconds, max_downloads, download_count, upload_token, created_at, expires_at, chunk_size)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&row.id)
        .bind(&row.name)
        .bind(row.size)
        .bind(&row.backend)
        .bind(&row.storage_key)
        .bind(&row.status)
        .bind(row.received_bytes)
        .bind(row.part_count)
        .bind(&row.s3_upload_id)
        .bind(row.ttl_seconds)
        .bind(row.max_downloads)
        .bind(row.download_count)
        .bind(&row.upload_token)
        .bind(row.created_at)
        .bind(row.expires_at)
        .bind(row.chunk_size)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_upload(&self, id: &str) -> Result<Option<UploadRow>, sqlx::Error> {
        sqlx::query_as::<_, UploadRow>("SELECT * FROM uploads WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    /// CAS gate for receiving part `seq` (1-indexed). Returns false if out of order or complete.
    pub async fn take_part(
        &self,
        id: &str,
        seq: i64,
        chunk_bytes: i64,
    ) -> Result<bool, sqlx::Error> {
        let res = sqlx::query(
            "UPDATE uploads SET part_count = part_count + 1, received_bytes = received_bytes + ? WHERE id = ? AND status = 'pending' AND part_count = ?"
        )
        .bind(chunk_bytes)
        .bind(id)
        .bind(seq - 1)
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() == 1)
    }

    pub async fn bump_part_count(&self, id: &str) -> Result<bool, sqlx::Error> {
        let res = sqlx::query(
            "UPDATE uploads SET part_count = part_count + 1 WHERE id = ? AND status = 'pending'",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() == 1)
    }

    pub async fn set_s3_upload_id(&self, id: &str, upload_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE uploads SET s3_upload_id = ? WHERE id = ?")
            .bind(upload_id)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn complete_upload(&self, id: &str, final_size: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE uploads SET status = 'ready', size = ? WHERE id = ?")
            .bind(final_size)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_upload(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM uploads WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Best-effort WAL truncation so deleted-row content doesn't linger in WAL pages.
    pub async fn checkpoint(&self) -> Result<(), sqlx::Error> {
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Increments download count if upload is ready and not expired (by time or count limit).
    /// Returns true if download allowed; false if denied.
    pub async fn increment_download(&self, id: &str, now: i64) -> Result<bool, sqlx::Error> {
        let res = sqlx::query(
            "UPDATE uploads SET download_count = download_count + 1 WHERE id = ? AND status = 'ready' AND (expires_at IS NULL OR expires_at > ?) AND (max_downloads IS NULL OR download_count < max_downloads)"
        )
        .bind(id)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() == 1)
    }

    /// Sum of received_bytes for all pending local uploads.
    pub async fn pending_local_bytes(&self) -> Result<i64, sqlx::Error> {
        let row: (Option<i64>,) = sqlx::query_as(
            "SELECT SUM(received_bytes) FROM uploads WHERE status = 'pending' AND backend = 'local'"
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0.unwrap_or(0))
    }

    /// Rows to delete: expired (expires_at <= now), count-exhausted, or stale pending.
    pub async fn sweep(
        &self,
        now: i64,
        pending_timeout_secs: i64,
    ) -> Result<Vec<UploadRow>, sqlx::Error> {
        sqlx::query_as::<_, UploadRow>(
            "SELECT * FROM uploads WHERE (expires_at IS NOT NULL AND expires_at <= ?) OR (max_downloads IS NOT NULL AND download_count >= max_downloads) OR (status = 'pending' AND created_at <= ?)"
        )
        .bind(now)
        .bind(now - pending_timeout_secs)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn set_expires_at(&self, id: &str, ts: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE uploads SET expires_at = ? WHERE id = ?")
            .bind(ts)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_created_at(&self, id: &str, ts: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE uploads SET created_at = ? WHERE id = ?")
            .bind(ts)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_get_complete_lifecycle() {
        let db = Db::open_in_memory().await.unwrap();
        let row = UploadRow {
            id: "abc123456789".into(),
            name: "test.txt".into(),
            size: 0,
            backend: "local".into(),
            storage_key: "abc123456789".into(),
            status: "pending".into(),
            received_bytes: 0,
            part_count: 0,
            s3_upload_id: None,
            ttl_seconds: Some(3600),
            max_downloads: None,
            download_count: 0,
            upload_token: "tok".into(),
            created_at: 100,
            expires_at: Some(3700),
            chunk_size: 33554432,
        };
        db.create_upload(&row).await.unwrap();

        let got = db.get_upload("abc123456789").await.unwrap().unwrap();
        assert_eq!(got.name, "test.txt");
        assert_eq!(got.status, "pending");

        db.complete_upload("abc123456789", 1024).await.unwrap();
        let got2 = db.get_upload("abc123456789").await.unwrap().unwrap();
        assert_eq!(got2.status, "ready");
        assert_eq!(got2.size, 1024);
    }

    #[tokio::test]
    async fn take_part_is_atomic_sequence_gate() {
        let db = Db::open_in_memory().await.unwrap();
        let row = UploadRow {
            id: "id1".into(),
            name: "f.bin".into(),
            size: 0,
            backend: "local".into(),
            storage_key: "id1".into(),
            status: "pending".into(),
            received_bytes: 0,
            part_count: 0,
            s3_upload_id: None,
            ttl_seconds: Some(60),
            max_downloads: None,
            download_count: 0,
            upload_token: "t".into(),
            created_at: 10,
            expires_at: Some(70),
            chunk_size: 33554432,
        };
        db.create_upload(&row).await.unwrap();

        assert!(db.take_part("id1", 1, 100).await.unwrap());
        assert!(!db.take_part("id1", 1, 100).await.unwrap()); // duplicate part 1 fails
        assert!(db.take_part("id1", 2, 200).await.unwrap());
        let got = db.get_upload("id1").await.unwrap().unwrap();
        assert_eq!(got.part_count, 2);
        assert_eq!(got.received_bytes, 300);
    }

    #[tokio::test]
    async fn increment_download_gates_count_and_expiry_atomically() {
        let db = Db::open_in_memory().await.unwrap();
        let row = UploadRow {
            id: "id2".into(),
            name: "f.bin".into(),
            size: 10,
            backend: "local".into(),
            storage_key: "id2".into(),
            status: "ready".into(),
            received_bytes: 10,
            part_count: 1,
            s3_upload_id: None,
            ttl_seconds: None,
            max_downloads: Some(2),
            download_count: 0,
            upload_token: "t".into(),
            created_at: 100,
            expires_at: Some(200),
            chunk_size: 33554432,
        };
        db.create_upload(&row).await.unwrap();

        assert!(db.increment_download("id2", 150).await.unwrap()); // download 1 ok
        assert!(db.increment_download("id2", 150).await.unwrap()); // download 2 ok
        assert!(!db.increment_download("id2", 150).await.unwrap()); // download 3 denied (limit reached)
        assert!(!db.increment_download("id2", 250).await.unwrap()); // denied (expired time)
    }

    #[tokio::test]
    async fn pending_local_bytes_sums_pending_only() {
        let db = Db::open_in_memory().await.unwrap();
        let mut r1 = UploadRow {
            id: "p1".into(),
            name: "a".into(),
            size: 0,
            backend: "local".into(),
            storage_key: "p1".into(),
            status: "pending".into(),
            received_bytes: 50,
            part_count: 1,
            s3_upload_id: None,
            ttl_seconds: Some(60),
            max_downloads: None,
            download_count: 0,
            upload_token: "t".into(),
            created_at: 1,
            expires_at: Some(61),
            chunk_size: 33554432,
        };
        db.create_upload(&r1).await.unwrap();
        r1.id = "p2".into();
        r1.received_bytes = 70;
        db.create_upload(&r1).await.unwrap();

        assert_eq!(db.pending_local_bytes().await.unwrap(), 120);
    }

    #[tokio::test]
    async fn sweep_selects_only_dead_and_stale() {
        let db = Db::open_in_memory().await.unwrap();
        // row 1: expired
        let r1 = UploadRow {
            id: "e1".into(),
            name: "e1".into(),
            size: 10,
            backend: "local".into(),
            storage_key: "e1".into(),
            status: "ready".into(),
            received_bytes: 10,
            part_count: 1,
            s3_upload_id: None,
            ttl_seconds: Some(10),
            max_downloads: None,
            download_count: 0,
            upload_token: "t".into(),
            created_at: 0,
            expires_at: Some(10),
            chunk_size: 33554432,
        };
        // row 2: live
        let r2 = UploadRow {
            id: "l1".into(),
            name: "l1".into(),
            size: 10,
            backend: "local".into(),
            storage_key: "l1".into(),
            status: "ready".into(),
            received_bytes: 10,
            part_count: 1,
            s3_upload_id: None,
            ttl_seconds: Some(100),
            max_downloads: None,
            download_count: 0,
            upload_token: "t".into(),
            created_at: 0,
            expires_at: Some(100),
            chunk_size: 33554432,
        };
        // row 3: stale pending
        let r3 = UploadRow {
            id: "s1".into(),
            name: "s1".into(),
            size: 0,
            backend: "local".into(),
            storage_key: "s1".into(),
            status: "pending".into(),
            received_bytes: 0,
            part_count: 0,
            s3_upload_id: None,
            ttl_seconds: Some(100),
            max_downloads: None,
            download_count: 0,
            upload_token: "t".into(),
            created_at: 0,
            expires_at: Some(100),
            chunk_size: 33554432,
        };
        db.create_upload(&r1).await.unwrap();
        db.create_upload(&r2).await.unwrap();
        db.create_upload(&r3).await.unwrap();

        let dead = db.sweep(50, 30).await.unwrap(); // now = 50, pending_timeout = 30 (created_at 0 <= 50-30 = 20)
        let ids: Vec<&str> = dead.iter().map(|r| r.id.as_str()).collect();
        assert!(ids.contains(&"e1"));
        assert!(ids.contains(&"s1"));
        assert!(!ids.contains(&"l1"));
    }
}
