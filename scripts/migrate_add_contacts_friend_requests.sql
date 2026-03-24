-- Migration: thêm bảng contacts và friend_requests
-- Chạy 1 lần nếu DB đã tồn tại (không có những bảng này)

CREATE TABLE IF NOT EXISTS contacts (
    user_id     UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    node_id     VARCHAR(64)  NOT NULL,
    nickname    VARCHAR(64),
    notes       TEXT,
    added_at    BIGINT       NOT NULL,
    PRIMARY KEY (user_id, node_id)
);

CREATE INDEX IF NOT EXISTS idx_contacts_user_id ON contacts(user_id);

CREATE TABLE IF NOT EXISTS friend_requests (
    id          BIGSERIAL    PRIMARY KEY,
    from_user_id UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    from_node_id VARCHAR(64) NOT NULL,
    from_username VARCHAR(32) NOT NULL,
    to_node_id  VARCHAR(64)  NOT NULL,
    status      VARCHAR(16)  NOT NULL DEFAULT 'pending'
                             CHECK (status IN ('pending', 'accepted', 'declined')),
    created_at  BIGINT       NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_friend_requests_to_node_id ON friend_requests(to_node_id);
CREATE INDEX IF NOT EXISTS idx_friend_requests_from_node_id ON friend_requests(from_node_id);
