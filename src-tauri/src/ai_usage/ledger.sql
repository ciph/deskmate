CREATE TABLE usage_messages (
    message_id TEXT PRIMARY KEY NOT NULL,
    provider_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    -- Decimal text preserves the legacy JSON's full unsigned u64 range.
    tokens TEXT NOT NULL
) WITHOUT ROWID;
CREATE INDEX idx_usage_provider_created ON usage_messages(provider_id, created_at_ms);
CREATE TABLE usage_summary_windows (
    provider_id TEXT PRIMARY KEY NOT NULL,
    start_ms INTEGER NOT NULL,
    end_ms INTEGER NOT NULL CHECK (end_ms > start_ms)
) WITHOUT ROWID;
CREATE TABLE usage_summary_models (
    provider_id TEXT NOT NULL REFERENCES usage_summary_windows(provider_id) ON DELETE CASCADE,
    model_id TEXT NOT NULL,
    tokens TEXT NOT NULL,
    requests INTEGER NOT NULL CHECK (requests >= 0),
    PRIMARY KEY (provider_id, model_id)
) WITHOUT ROWID;
