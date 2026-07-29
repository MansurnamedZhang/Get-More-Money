ALTER TABLE app_settings
ADD COLUMN transaction_hard_delete_minutes INTEGER NOT NULL DEFAULT 30
CHECK (transaction_hard_delete_minutes BETWEEN 0 AND 10080);
