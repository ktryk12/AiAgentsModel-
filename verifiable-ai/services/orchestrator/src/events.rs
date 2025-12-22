use anyhow::Result;
use serde_json::Value as JsonValue;
use sqlx::{Postgres, PgPool, Transaction};
use uuid::Uuid;

pub async fn append_event(pool: &PgPool, job_id: Uuid, event: JsonValue) -> Result<()> {
    let mut tx = pool.begin().await?;
    append_event_tx(&mut tx, job_id, event).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn append_event_tx(
    tx: &mut Transaction<'_, Postgres>, 
    job_id: Uuid, 
    event: JsonValue
) -> Result<()> {
    // 1. Insert into job_events (Audit Log)
    sqlx::query(
        r#"INSERT INTO job_events (job_id, event) VALUES ($1, $2)"#
    )
    .bind(job_id)
    .bind(&event)
    .execute(&mut **tx)
    .await?;

    // 2. Insert into webhook_outbox (Push Notifications)
    let outbox_id = Uuid::new_v4();
    let envelope = serde_json::json!({
        "id": outbox_id, // Same as outbox PK for dedupe
        "job_id": job_id,
        "type": event.get("type").and_then(|v| v.as_str()).unwrap_or("unknown"),
        "ts": chrono::Utc::now(),
        "data": event
    });

    sqlx::query(
        r#"
        INSERT INTO webhook_outbox (id, job_id, event) 
        VALUES ($1, $2, $3)
        "#
    )
    .bind(outbox_id)
    .bind(job_id)
    .bind(envelope)
    .execute(&mut **tx)
    .await?;

    Ok(())
}
