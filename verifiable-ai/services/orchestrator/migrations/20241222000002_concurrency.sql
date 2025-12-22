CREATE TABLE IF NOT EXISTS dataset_locks (
  dataset_id   TEXT PRIMARY KEY,
  job_id       UUID NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
  lease_until  TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_dataset_locks_lease_until
  ON dataset_locks (lease_until);

CREATE INDEX IF NOT EXISTS idx_jobs_status_attempts_created
  ON jobs (status, attempts, created_at);
