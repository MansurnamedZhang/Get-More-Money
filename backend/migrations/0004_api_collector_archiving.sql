ALTER TABLE data_sources ADD COLUMN deleted_at TEXT;
ALTER TABLE sync_jobs ADD COLUMN deleted_at TEXT;

CREATE INDEX idx_data_sources_active ON data_sources(deleted_at, is_enabled, priority);
CREATE INDEX idx_sync_jobs_active ON sync_jobs(deleted_at, is_enabled, next_run_at);
