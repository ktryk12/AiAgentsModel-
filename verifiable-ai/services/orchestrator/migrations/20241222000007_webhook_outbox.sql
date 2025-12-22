CREATE TABLE IF NOT EXISTS webhook_outbox (
  id UUID PRIMARY KEY,
  job_id UUID NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
  event JSONB NOT NULL,

  status TEXT NOT NULL DEFAULT 'pending', -- pending, delivered, failed, retrying
  attempts INT NOT NULL DEFAULT 0,
  next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

  delivered_at TIMESTAMPTZ NULL,
  locked_by TEXT NULL,
  locked_until TIMESTAMPTZ NULL,

  last_error TEXT NULL
);

CREATE INDEX IF NOT EXISTS idx_webhook_outbox_due
  ON webhook_outbox (next_attempt_at)
  WHERE delivered_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_webhook_outbox_locked
  ON webhook_outbox (locked_until)
  WHERE delivered_at IS NULL;
