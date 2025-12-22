ALTER TABLE jobs
  ADD COLUMN IF NOT EXISTS priority INT NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_jobs_priority_status_created
  ON jobs (status, priority DESC, created_at ASC);
