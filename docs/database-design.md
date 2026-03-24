# K2 Network — Database Design

## Tổng quan

- **Database**: PostgreSQL 16
- **Port**: 5433 (Docker), tránh conflict với PostgreSQL local (5432)
- **Connection**: `postgresql://k2user:k2secret@localhost:5433/k2db`

---

## Kiến trúc 2 chế độ

| | Guest | Logged-in |
|---|---|---|
| Trade requests | Tối đa **2** | Không giới hạn |
| Lịch sử trade | Không lưu | Lưu vào `trade_history` |
| Đăng ký agent | ✗ | ✓ `user_agents` |
| Tìm kiếm | 1 topic | Nhiều topic cùng lúc |
| Node ID | Tạm thời, mất khi restart | Cố định, gắn với account |

---

## Schema

### `users`

Mỗi account gắn với **1 node duy nhất** — `secret_key` là iroh secret key (hex), từ đó derive ra `node_id` (public key). Khi login, server load lại đúng `secret_key` này → node_id luôn cố định, không bị đổi mỗi lần restart.

```sql
CREATE TABLE users (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username      VARCHAR(32)  NOT NULL UNIQUE,
    email         VARCHAR(255) NOT NULL UNIQUE,
    password_hash TEXT         NOT NULL,       -- bcrypt hash
    secret_key    TEXT         NOT NULL,       -- iroh SecretKey hex, 1 account = 1 node
    node_id       VARCHAR(64)  NOT NULL UNIQUE,-- iroh PublicKey hex (derived từ secret_key)
    created_at    TIMESTAMPTZ  NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ  NOT NULL DEFAULT now()
);
```

**Tại sao lưu `secret_key`?**
Hiện tại `K2Node::new()` sinh key ngẫu nhiên mỗi lần khởi động → node_id thay đổi sau mỗi restart. Khi có auth, server load `secret_key` từ DB → node_id cố định suốt vòng đời account.

---

### `refresh_tokens`

JWT flow: access token (15 phút) + refresh token (7 ngày). Refresh token được hash trước khi lưu, không lưu raw.

```sql
CREATE TABLE refresh_tokens (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash  TEXT        NOT NULL UNIQUE, -- SHA-256 hash của raw token
    expires_at  TIMESTAMPTZ NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked     BOOLEAN     NOT NULL DEFAULT false
);
```

**Flow:**
1. Login → tạo access token (JWT, 15min) + refresh token (random, 7 ngày)
2. Lưu `hash(refresh_token)` vào DB
3. Khi access token hết hạn → gửi refresh token → server verify hash → cấp access token mới
4. Logout → set `revoked = true`

---

### `trade_history`

Chỉ logged-in users mới được lưu. Guest không có `user_id` nên không lưu được.

```sql
CREATE TABLE trade_history (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    offer_id    VARCHAR(64) NOT NULL,         -- K2-{timestamp}-{random}
    topic       VARCHAR(64) NOT NULL,         -- "Digital Assets" | "Goods" | "Freelance Job"
    action      VARCHAR(16) NOT NULL          -- "buy" | "sell" | "exchange"
                CHECK (action IN ('buy', 'sell', 'exchange')),
    title       TEXT        NOT NULL,
    description TEXT,
    price_min   BIGINT      NOT NULL DEFAULT 0,
    price_max   BIGINT      NOT NULL DEFAULT 0,
    currency    VARCHAR(8)  NOT NULL DEFAULT 'VND',
    status      VARCHAR(16) NOT NULL DEFAULT 'pending'
                CHECK (status IN ('pending', 'matched', 'completed', 'cancelled')),
    form_data   JSONB,                        -- full payload gốc để tiện query
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

---

### `guest_requests`

Rate limiting cho guest: cho phép tối đa **2 requests** theo fingerprint (IP + user agent hash).

```sql
CREATE TABLE guest_requests (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    fingerprint   VARCHAR(128) NOT NULL UNIQUE, -- hash(IP + UserAgent)
    request_count INT          NOT NULL DEFAULT 1,
    first_request TIMESTAMPTZ  NOT NULL DEFAULT now(),
    last_request  TIMESTAMPTZ  NOT NULL DEFAULT now()
);
```

**Logic kiểm tra:**
```
fingerprint = hash(client_ip + user_agent)
SELECT request_count FROM guest_requests WHERE fingerprint = ?
  → count >= 2  →  trả lỗi 429, yêu cầu đăng nhập
  → count < 2   →  cho phép, tăng count
  → not found   →  INSERT mới với count = 1
```

---

### `user_agents`

Agents mà logged-in user đăng ký để mở rộng khả năng tìm kiếm / tự động hóa.

```sql
CREATE TABLE user_agents (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name        VARCHAR(64) NOT NULL,
    description TEXT,
    config      JSONB       NOT NULL DEFAULT '{}', -- cấu hình agent
    is_active   BOOLEAN     NOT NULL DEFAULT true,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

---

## Indexes

```sql
-- refresh_tokens
CREATE INDEX idx_refresh_tokens_user_id   ON refresh_tokens(user_id);
CREATE INDEX idx_refresh_tokens_token_hash ON refresh_tokens(token_hash);

-- trade_history
CREATE INDEX idx_trade_history_user_id ON trade_history(user_id);
CREATE INDEX idx_trade_history_topic   ON trade_history(topic);
CREATE INDEX idx_trade_history_status  ON trade_history(status);

-- guest_requests
CREATE UNIQUE INDEX idx_guest_requests_fingerprint ON guest_requests(fingerprint);

-- user_agents
CREATE INDEX idx_user_agents_user_id ON user_agents(user_id);
```

---

## Auth Flow (JWT)

```
ĐĂNG KÝ
  POST /auth/register { username, email, password }
    → sinh iroh SecretKey mới
    → derive node_id (PublicKey)
    → lưu users(username, email, bcrypt(password), secret_key, node_id)
    → trả { access_token, refresh_token, node_id }

ĐĂNG NHẬP
  POST /auth/login { email, password }
    → verify bcrypt(password)
    → load secret_key → khởi động K2Node với key cố định
    → tạo JWT access_token (15min) + refresh_token (7 ngày)
    → lưu hash(refresh_token) vào refresh_tokens
    → trả { access_token, refresh_token, node_id }

LÀM MỚI TOKEN
  POST /auth/refresh { refresh_token }
    → verify hash(refresh_token) trong DB, chưa revoked, chưa hết hạn
    → cấp access_token mới
    → trả { access_token }

ĐĂNG XUẤT
  POST /auth/logout { refresh_token }
    → set refresh_tokens.revoked = true
```

---

## Cấu trúc JWT Access Token

```json
{
  "sub": "<user_id UUID>",
  "node_id": "<node_id hex>",
  "username": "<username>",
  "exp": 1234567890
}
```

---

## Biến môi trường

```env
POSTGRES_DB=k2db
POSTGRES_USER=k2user
POSTGRES_PASSWORD=k2secret
DATABASE_URL=postgresql://k2user:k2secret@localhost:5433/k2db

JWT_SECRET=<chuỗi random dài, thay trước khi deploy>
JWT_ACCESS_EXPIRES_SECS=900     # 15 phút
JWT_REFRESH_EXPIRES_DAYS=7
```

---

## Sơ đồ quan hệ

```
users
  │
  ├──< refresh_tokens   (1 user → nhiều refresh tokens)
  ├──< trade_history    (1 user → nhiều trades)
  └──< user_agents      (1 user → nhiều agents)

guest_requests           (độc lập, không liên kết users)
```

---

## Các bước tiếp theo

1. **k2-web-server**: thêm `sqlx`, `bcrypt`, `jsonwebtoken` vào `Cargo.toml`
2. Viết `db.rs` — connection pool với `sqlx::PgPool`
3. Viết `routes/auth.rs` — `/auth/register`, `/auth/login`, `/auth/refresh`, `/auth/logout`
4. Middleware `auth_middleware` — extract JWT từ `Authorization: Bearer <token>`, inject `user_id` vào request
5. **k2-app-web**: thêm Auth context, màn hình Login/Register, guard cho các tính năng premium
