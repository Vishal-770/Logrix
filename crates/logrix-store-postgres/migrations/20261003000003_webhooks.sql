-- Logrix Webhooks: Endpoints and Delivery Auditing
CREATE TABLE IF NOT EXISTS logrix_webhook_endpoints (
    id UUID PRIMARY KEY,
    url TEXT NOT NULL,
    secret TEXT NOT NULL,
    events TEXT[] NOT NULL DEFAULT '{}',
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_logrix_webhook_endpoints_active ON logrix_webhook_endpoints (is_active);

CREATE TABLE IF NOT EXISTS logrix_webhook_deliveries (
    id UUID PRIMARY KEY,
    endpoint_id UUID REFERENCES logrix_webhook_endpoints(id) ON DELETE SET NULL,
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    status_code INT,
    success BOOLEAN NOT NULL,
    error_message TEXT,
    latency_ms BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_logrix_webhook_deliveries_created ON logrix_webhook_deliveries (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_logrix_webhook_deliveries_endpoint ON logrix_webhook_deliveries (endpoint_id);
