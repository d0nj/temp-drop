use crate::db::UploadRow;
use crate::state::AppState;
use crate::storage::Storage;
use std::time::Duration;

pub async fn sweep_once(state: &AppState, now: i64) -> Result<usize, String> {
    let pending_timeout = state.config.uploads.pending_timeout_hours * 3600;
    let rows = state
        .db
        .sweep(now, pending_timeout)
        .await
        .map_err(|e| e.to_string())?;
    let mut n = 0;
    for row in rows {
        if cleanup(state, &row).await {
            n += 1;
        }
    }
    Ok(n)
}

async fn cleanup(state: &AppState, row: &UploadRow) -> bool {
    let storage_ok = if row.status == "pending" {
        match &*state.storage {
            Storage::Local(local) => local.delete(&row.storage_key).await.is_ok(),
            Storage::S3(s3) => match row.s3_upload_id.as_deref() {
                Some(uid) => s3.abort(&row.storage_key, uid).await.is_ok(),
                None => s3.delete(&row.storage_key).await.is_ok(),
            },
        }
    } else {
        state.storage.delete(&row.storage_key).await.is_ok()
    };
    if !storage_ok {
        return false;
    } // leave the row; retry next sweep
    match state.db.delete_upload(&row.id).await {
        Ok(()) => true,
        Err(e) => {
            eprintln!("janitor: delete row {}: {e}", row.id);
            false
        }
    }
}

pub async fn run(state: AppState, interval: Duration) {
    loop {
        tokio::time::sleep(interval).await;
        let now = chrono::Utc::now().timestamp();
        if let Err(e) = sweep_once(&state, now).await {
            eprintln!("janitor: sweep failed: {e}");
        }
    }
}
