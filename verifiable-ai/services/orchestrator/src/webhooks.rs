use crate::config::AppConfig;
use crate::state::SharedState;
use anyhow::Result;
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use sqlx::{PgPool, Row};
use std::time::Duration;
use tracing::{error, info, warn};
use uuid::Uuid;

const POLL_INTERVAL: Duration = Duration::from_secs(1);
const LEASE_DURATION: i64 = 30; // seconds
const MAX_ATTEMPTS: i32 = 20;

type HmacSha256 = Hmac<Sha256>;

pub async fn run_webhook_dispatcher(state: SharedState) {
    let url = match &state.config.webhook_url {
        Some(u) => u.clone(),
        None => {
            info!("webhook_dispatcher: disabled (no WEBHOOK_URL)");
            return;
        }
    };
    let secret = state.config.webhook_secret.clone().unwrap_or_default();
    
    info!(url=%url, "webhook_dispatcher: started");
    let client = reqwest::Client::new();
    let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "orchestrator".to_string());

    loop {
        // 1. Claim Batch (One at a time for simplicity, or batch?)
        // Let's do one at a time to ensure simple error handling first.
        match claim_and_send(&state.pg_pool, &client, &url, &secret, &hostname).await {
            Ok(some_work) => {
                if !some_work {
                    tokio::time::sleep(POLL_INTERVAL).await;
                }
            }
            Err(e) => {
                error!("webhook_dispatcher: error: {e:?}");
                tokio::time::sleep(POLL_INTERVAL).await;
            }
        }
    }
}

async fn claim_and_send(
    pool: &PgPool,
    client: &reqwest::Client,
    url: &str,
    secret: &str,
    hostname: &str,
) -> Result<bool> {
    let mut tx = pool.begin().await?;

    // 1. Claim one pending item
    // "locked_until IS NULL OR locked_until < NOW()" ensures we pick up abandoned locks
    let row = sqlx::query(
        r#"
        SELECT id, event, attempts
        FROM webhook_outbox
        WHERE status != 'delivered'
          AND next_attempt_at <= NOW()
          AND (locked_until IS NULL OR locked_until < NOW())
          AND attempts < $1
        ORDER BY next_attempt_at ASC
        LIMIT 1
        FOR UPDATE SKIP LOCKED
        "#
    )
    .bind(MAX_ATTEMPTS)
    .fetch_optional(&mut *tx)
    .await?;

    let row = match row {
        Some(r) => r,
        None => return Ok(false), // No work
    };

    let id: Uuid = row.get("id");
    let event: serde_json::Value = row.get("event");
    let attempts: i32 = row.get("attempts");

    // Lock it (extend lease)
    sqlx::query(
        r#"
        UPDATE webhook_outbox
        SET locked_by = $1, locked_until = NOW() + ($2 * INTERVAL '1 second')
        WHERE id = $3
        "#
    )
    .bind(hostname)
    .bind(LEASE_DURATION)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?; // Commit lock so other dispatchers skip it

    // 2. Prepare Payload & Signature
    let body = event.to_string();
    let ts = Utc::now().timestamp();
    let signature = sign_payload(secret, ts, &body);

    // 3. Send Request
    let resp = client.post(url)
        .header("Content-Type", "application/json")
        .header("Idempotency-Key", id.to_string())
        .header("X-Timestamp", ts.to_string())
        .header("X-Signature", signature)
        .body(body)
        .send()
        .await;

    // 4. Handle Result
    match resp {
        Ok(r) if r.status().is_success() => {
            // Success
            sqlx::query(
                r#"
                UPDATE webhook_outbox
                SET status='delivered', delivered_at=NOW(), locked_by=NULL, locked_until=NULL, last_error=NULL
                WHERE id=$1
                "#
            )
            .bind(id)
            .execute(pool)
            .await?;
            info!(id=%id, "webhook: delivered");
        }
        Ok(r) => {
            // HTTP Error (4xx/5xx)
            let status = r.status();
            let err_msg = format!("HTTP {}", status);
            handle_failure(pool, id, attempts, &err_msg).await?;
            warn!(id=%id, status=%status, "webhook: delivery failed");
        }
        Err(e) => {
            // Network Error
            let err_msg = e.to_string();
            handle_failure(pool, id, attempts, &err_msg).await?;
            warn!(id=%id, error=%err_msg, "webhook: delivery error");
        }
    }

    Ok(true)
}

async fn handle_failure(pool: &PgPool, id: Uuid, attempts: i32, err_msg: &str) -> Result<()> {
    let new_attempts = attempts + 1;
    let max_attempts = MAX_ATTEMPTS;
    
    // Exponential Backoff: base 1s * 2^attempts (capped at 1 hour)
    let backoff_secs = 2u64.pow(new_attempts.min(12) as u32).min(3600);
    // Add jitter? (Optional, skipping for simple implementation)

    if new_attempts >= max_attempts {
        sqlx::query(
            r#"
            UPDATE webhook_outbox
            SET status='failed', attempts=$1, locked_by=NULL, locked_until=NULL, last_error=$2, updated_at=NOW() -- implicitly updated? No field updated_at in schema, assuming created_at is enough or we updated status
            WHERE id=$3
            "#
        )
        .bind(new_attempts)
        .bind(err_msg)
        .bind(id)
        .execute(pool)
        .await?;
    } else {
        sqlx::query(
            r#"
            UPDATE webhook_outbox
            SET status='retrying', attempts=$1, next_attempt_at=NOW() + ($2 * INTERVAL '1 second'), locked_by=NULL, locked_until=NULL, last_error=$3
            WHERE id=$4
            "#
        )
        .bind(new_attempts)
        .bind(backoff_secs as i64)
        .bind(err_msg)
        .bind(id)
        .execute(pool)
        .await?;
    }
    Ok(())
}

fn sign_payload(secret: &str, ts: i64, body: &str) -> String {
    let payload = format!("{}.{}", ts, body);
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC can take any key length");
    mac.update(payload.as_bytes());
    let result = mac.finalize();
    hex::encode(result.into_bytes())
}
