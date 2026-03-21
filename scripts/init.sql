-- K2 Network Database Schema
-- PostgreSQL 16

-- ============================================
-- USERS
-- ============================================
CREATE TABLE IF NOT EXISTS users (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username    VARCHAR(32)  NOT NULL UNIQUE,
    email       VARCHAR(255) NOT NULL UNIQUE,
    password_hash TEXT       NOT NULL,
    -- iroh secret key (hex-encoded) — fixed per account
    secret_key  TEXT         NOT NULL,
    -- derived node_id (public key hex) — for display/lookup
    node_id     VARCHAR(64)  NOT NULL UNIQUE,
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ  NOT NULL DEFAULT now()
);

-- ============================================
-- REFRESH TOKENS
-- ============================================
CREATE TABLE IF NOT EXISTS refresh_tokens (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash  TEXT         NOT NULL UNIQUE,
    expires_at  TIMESTAMPTZ  NOT NULL,
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT now(),
    revoked     BOOLEAN      NOT NULL DEFAULT false
);

CREATE INDEX IF NOT EXISTS idx_refresh_tokens_user_id ON refresh_tokens(user_id);
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_token_hash ON refresh_tokens(token_hash);

-- ============================================
-- TRADE HISTORY
-- ============================================
CREATE TABLE IF NOT EXISTS trade_history (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    offer_id    VARCHAR(64)  NOT NULL,
    topic       VARCHAR(64)  NOT NULL,
    action      VARCHAR(16)  NOT NULL CHECK (action IN ('buy', 'sell', 'exchange')),
    title       TEXT         NOT NULL,
    description TEXT,
    price_min   BIGINT       NOT NULL DEFAULT 0,
    price_max   BIGINT       NOT NULL DEFAULT 0,
    currency    VARCHAR(8)   NOT NULL DEFAULT 'VND',
    status      VARCHAR(16)  NOT NULL DEFAULT 'pending'
                             CHECK (status IN ('pending', 'matched', 'completed', 'cancelled')),
    form_data   JSONB,
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_trade_history_user_id ON trade_history(user_id);
CREATE INDEX IF NOT EXISTS idx_trade_history_topic ON trade_history(topic);
CREATE INDEX IF NOT EXISTS idx_trade_history_status ON trade_history(status);

-- ============================================
-- GUEST REQUEST TRACKING (rate limit)
-- ============================================
CREATE TABLE IF NOT EXISTS guest_requests (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- fingerprint: IP + user agent hash
    fingerprint     VARCHAR(128) NOT NULL,
    request_count   INT          NOT NULL DEFAULT 1,
    first_request   TIMESTAMPTZ  NOT NULL DEFAULT now(),
    last_request    TIMESTAMPTZ  NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_guest_requests_fingerprint ON guest_requests(fingerprint);

-- ============================================
-- AGENTS (đăng ký agent — chỉ logged-in users)
-- ============================================
CREATE TABLE IF NOT EXISTS user_agents (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name        VARCHAR(64)  NOT NULL,
    description TEXT,
    config      JSONB        NOT NULL DEFAULT '{}',
    is_active   BOOLEAN      NOT NULL DEFAULT true,
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_user_agents_user_id ON user_agents(user_id);

-- ============================================
-- Auto-update updated_at trigger
-- ============================================
CREATE OR REPLACE FUNCTION update_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_users_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();
