use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader, AsyncWriteExt};
use tokio::process::Command;
use uuid::Uuid;
use tokio::sync::RwLock;

use crate::types_jobs::{JobRecord, JobPhase, TrainingEvent, key_job, key_job_index, key_dataset_active};
use crate::vdb_exec::with_vdb_blocking;
use crate::types_training::DatasetRecord;
use vdb::VerifiableKV;

fn now() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

pub async fn spawn_training_job<S>(
    vdb: Arc<RwLock<VerifiableKV<S>>>,
    job_id: Uuid,
    logs_dir: PathBuf,
    adapters_dir: PathBuf,
) -> Result<(), String>
where
    S: vdb::Storage + Send + Sync + 'static,
{
    // 1. Load initial job record
    let job: JobRecord = with_vdb_blocking(vdb.clone(), move |db: &mut VerifiableKV<S>| {
        let key = key_job(job_id);
        let bytes = db.get(key.as_bytes()).map_err(|e| e.to_string())?.value.ok_or("job not found")?;
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())
    }).await.map_err(|e| e.to_string())??;

    // 2. Prepare log file
    tokio::fs::create_dir_all(&logs_dir).await.map_err(|e| e.to_string())?;
    let log_path = logs_dir.join(format!("{job_id}.ndjson"));

    // 3. Resolve absolute paths for worker
    let current_dir = std::env::current_dir().map_err(|e| e.to_string())?;
    let worker_script = current_dir.join("workers/lora_trainer.py");
    let out_dir = adapters_dir.join(job_id.to_string());
    
    // 4. Persistence helper
    let persist_job = |j: &JobRecord| {
        let vdb = vdb.clone();
        let j_clone = j.clone();
        async move {
            with_vdb_blocking(vdb, move |db: &mut VerifiableKV<S>| {
                let key = key_job(j_clone.job_id);
                let bytes = serde_json::to_vec(&j_clone).map_err(|e| e.to_string())?;
                db.set(key.as_bytes(), &bytes).map_err(|e| e.to_string())
            }).await.map_err(|e| e.to_string())??;
            Ok::<(), String>(())
        }
    };

    // 5. Spawn Worker
    let mut cmd = Command::new("python");
    cmd.arg(&worker_script)
       .arg("--job-id").arg(job_id.to_string())
       .arg("--dataset-id").arg(job.dataset_id.to_string())
       .arg("--base-repo").arg(&job.base_model_repo)
       .arg("--base-rev").arg(&job.base_model_revision)
       .arg("--out-dir").arg(out_dir.to_string_lossy().to_string())
       .arg("--lora-r").arg(job.lora.r.to_string())
       .arg("--lora-alpha").arg(job.lora.alpha.to_string())
       .arg("--epochs").arg(job.lora.epochs.to_string())
       .arg("--lr").arg(job.lora.lr.to_string());
       
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("failed to spawn worker: {}", e))?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let mut reader = BufReader::new(stdout).lines();

    // 6. Emit Start Event
    let mut current_job = job;
    let ts = now();
    current_job.apply_event(&TrainingEvent::Start { ts: Some(ts) }, ts);
    persist_job(&current_job).await.map_err(|e| format!("persist start failed: {}", e))?;

    // 7. Event Loop
    while let Ok(Some(line)) = reader.next_line().await {
        let line = line.trim();
        if line.is_empty() { continue; }

        {
            let mut f = tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
                .await
                .map_err(|e| e.to_string())?;
            f.write_all(line.as_bytes()).await.map_err(|e| e.to_string())?;
            f.write_all(b"\n").await.map_err(|e| e.to_string())?;
        }

        let ev: TrainingEvent = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => TrainingEvent::Error { message: format!("invalid_ndjson: {}", line) }
        };

        let ts = now();
        current_job.apply_event(&ev, ts);
        if let Err(e) = persist_job(&current_job).await {
            eprintln!("Failed to persist job update: {}", e);
        }

        if matches!(current_job.phase, JobPhase::Ready | JobPhase::Failed) {
            let ds_id = current_job.dataset_id;
            let _ = with_vdb_blocking(vdb.clone(), move |db: &mut VerifiableKV<S>| {
                let ds_key = crate::training_store::dataset_key(ds_id);
                if let Ok(res) = db.get(&ds_key) {
                    if let Some(ds_bytes) = res.value {
                        if let Ok(ds) = serde_json::from_slice::<DatasetRecord>(&ds_bytes) {
                            let hash_hex = hex::encode(ds.dataset_hash);
                            let lock_key = key_dataset_active(&hash_hex);
                            let _ = db.delete(lock_key.as_bytes());
                        }
                    }
                }
                Ok::<(), String>(())
            }).await;
            break;
        }
    }

    // 8. Wait for exit
    let status = child.wait().await.map_err(|e| e.to_string())?;
    if !status.success() {
        if !matches!(current_job.phase, JobPhase::Ready | JobPhase::Failed) {
             let ts = now();
             current_job.apply_event(&TrainingEvent::Error { 
                 message: format!("worker_exit_nonzero: {}", status) 
             }, ts);
             persist_job(&current_job).await.ok(); 
             
             let ds_id = current_job.dataset_id;
             let _ = with_vdb_blocking(vdb.clone(), move |db: &mut VerifiableKV<S>| {
                let ds_key = crate::training_store::dataset_key(ds_id);
                if let Ok(res) = db.get(&ds_key) {
                    if let Some(ds_bytes) = res.value {
                        if let Ok(ds) = serde_json::from_slice::<DatasetRecord>(&ds_bytes) {
                            let hash_hex = hex::encode(ds.dataset_hash);
                            let lock_key = key_dataset_active(&hash_hex);
                            let _ = db.delete(lock_key.as_bytes());
                        }
                    }
                }
                Ok::<(), String>(())
            }).await;
        }
    }

    Ok(())
}

pub async fn recover_jobs<S>(vdb: Arc<RwLock<VerifiableKV<S>>>) -> Result<(), String>
where 
    S: vdb::Storage + Send + Sync + 'static,
{
    with_vdb_blocking(vdb, |db: &mut VerifiableKV<S>| {
        let idx_key = key_job_index();
        let idx_bytes_opt = db.get(idx_key.as_bytes()).map_err(|e| e.to_string())?.value;
        let keys: Vec<String> = if let Some(b) = idx_bytes_opt {
            serde_json::from_slice(&b).unwrap_or_default()
        } else {
            Vec::new()
        };

        for key in keys {
            if let Ok(res) = db.get(key.as_bytes()) {
                if let Some(bytes) = res.value {
                    if let Ok(mut job) = serde_json::from_slice::<JobRecord>(&bytes) {
                        if !matches!(job.phase, JobPhase::Ready | JobPhase::Failed) {
                            job.phase = JobPhase::Failed;
                            job.error = Some("orchestrator_restart".into());
                            job.updated_at = now();
                            
                            let new_bytes = serde_json::to_vec(&job).map_err(|e| e.to_string())?;
                            db.set(key.as_bytes(), &new_bytes).map_err(|e| e.to_string())?;
                            
                            let ds_key = crate::training_store::dataset_key(job.dataset_id);
                            if let Ok(ds_res) = db.get(&ds_key) {
                                if let Some(ds_bytes) = ds_res.value {
                                     if let Ok(ds) = serde_json::from_slice::<DatasetRecord>(&ds_bytes) {
                                         let hash_hex = hex::encode(ds.dataset_hash);
                                         let lock_key = key_dataset_active(&hash_hex);
                                         let _ = db.delete(lock_key.as_bytes());
                                     }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok::<(), String>(())
    }).await.map_err(|e| e.to_string())??;
    
    Ok(())
}
