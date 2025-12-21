use std::sync::Arc;
use tokio::sync::RwLock;

pub async fn with_vdb_blocking<S, R, F>(
    vdb: Arc<RwLock<S>>,
    f: F,
) -> Result<R, tokio::task::JoinError>
where
    S: Send + Sync + 'static,
    R: Send + 'static,
    F: FnOnce(&mut S) -> R + Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        let mut guard = vdb.blocking_write();
        f(&mut *guard)
    })
    .await
}
