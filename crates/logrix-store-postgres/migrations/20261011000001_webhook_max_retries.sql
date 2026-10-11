-- Persist the retry policy configured through the webhook API.
ALTER TABLE logrix_webhook_endpoints
    ADD COLUMN IF NOT EXISTS max_retries BIGINT NOT NULL DEFAULT 0;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'logrix_webhook_endpoints_max_retries_check'
    ) THEN
        ALTER TABLE logrix_webhook_endpoints
            ADD CONSTRAINT logrix_webhook_endpoints_max_retries_check
            CHECK (max_retries BETWEEN 0 AND 4294967295);
    END IF;
END $$;
