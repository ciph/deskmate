-- Bound expiry discovery by its existing predicate, without changing when
-- expiration is materialized or how RFC 3339 strings are compared.
CREATE INDEX idx_memories_active_expiry
    ON memories (expires_at)
    WHERE status = 'active' AND expires_at IS NOT NULL;
