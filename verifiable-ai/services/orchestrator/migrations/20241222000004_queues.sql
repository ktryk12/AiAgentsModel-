ALTER TABLE jobs
  ADD COLUMN IF NOT EXISTS queue TEXT NOT NULL DEFAULT 'default';

CREATE INDEX IF NOT EXISTS idx_jobs_queue_status_priority_created
  ON jobs (queue, status, priority DESC, created_at);
