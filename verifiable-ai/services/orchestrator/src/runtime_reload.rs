use tokio::time::{sleep, Duration};

pub async fn reload_from_vdb(state: crate::state::SharedState) {
    loop {
        // 1) check if reload is needed
        let should = {
            let rt = state.runtime.lock().await;
            rt.pending_reload
        };

        if !should {
            sleep(Duration::from_millis(200)).await;
            continue;
        }

        // 2) mark loading
        {
            let mut rt = state.runtime.lock().await;
            rt.pending_reload = false;
            rt.status = crate::runtime::RuntimeStatus::Loading {
                started_at: crate::runtime::ModelRuntimeManager::now(),
            };
        }

        // 3) fetch active model from VDB (blocking)
        let active_res = tokio::task::spawn_blocking({
            let state = state.clone();
            move || {
                let mut guard = state.vdb.blocking_write();
                modelops::get_active_model(&mut *guard)
                    .map_err(|e| e.to_string())
            }
        }).await;

        let active = match active_res {
            Ok(Ok(Some(a))) => a,
            Ok(Ok(None)) => {
                let mut rt = state.runtime.lock().await;
                rt.status = crate::runtime::RuntimeStatus::Empty;
                continue;
            }
            Ok(Err(e)) => {
                let mut rt = state.runtime.lock().await;
                rt.status = crate::runtime::RuntimeStatus::Failed {
                    error: format!("active model read failed: {e}"),
                    failed_at: crate::runtime::ModelRuntimeManager::now(),
                };
                continue;
            }
            Err(e) => {
                let mut rt = state.runtime.lock().await;
                rt.status = crate::runtime::RuntimeStatus::Failed {
                    error: format!("vdb task join failed: {e}"),
                    failed_at: crate::runtime::ModelRuntimeManager::now(),
                };
                continue;
            }
        };

        // 4) provider.load
        let load_res = {
            let mut rt = state.runtime.lock().await;
            rt.provider.load(&active).await
        };

        match load_res {
            Ok(_) => {
                let mut rt = state.runtime.lock().await;
                rt.status = crate::runtime::RuntimeStatus::Ready {
                    active,
                    loaded_at: crate::runtime::ModelRuntimeManager::now(),
                };
            }
            Err(e) => {
                let mut rt = state.runtime.lock().await;
                rt.status = crate::runtime::RuntimeStatus::Failed {
                    error: format!("provider load failed: {e}"),
                    failed_at: crate::runtime::ModelRuntimeManager::now(),
                };
            }
        }
    }
}
