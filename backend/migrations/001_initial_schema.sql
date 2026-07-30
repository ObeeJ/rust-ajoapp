-- Migration: 001_initial_schema
-- All monetary values stored as INTEGER (kobo) — zero floating point

CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- ── Users ─────────────────────────────────────────────────────────────────────
CREATE TABLE users (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT        NOT NULL,
    phone       TEXT        NOT NULL UNIQUE,
    email       TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_users_phone ON users(phone);

-- ── Pins (hashed) ─────────────────────────────────────────────────────────────
CREATE TABLE pins (
    user_id     UUID        PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    hash        TEXT        NOT NULL
);

-- ── Wallets ───────────────────────────────────────────────────────────────────
CREATE TABLE wallets (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID        NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    available_kobo  BIGINT      NOT NULL DEFAULT 0 CHECK (available_kobo >= 0),
    ledger_kobo     BIGINT      NOT NULL DEFAULT 0,
    version         BIGINT      NOT NULL DEFAULT 0
);
CREATE INDEX idx_wallets_user_id ON wallets(user_id);

-- ── Double-Entry Ledger (append-only — no UPDATE/DELETE) ──────────────────────
CREATE TABLE ledger_entries (
    id                      UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_id               UUID        NOT NULL REFERENCES wallets(id),
    counterpart_wallet_id   UUID        REFERENCES wallets(id),
    kind                    TEXT        NOT NULL CHECK (kind IN ('credit','debit')),
    amount_kobo             BIGINT      NOT NULL CHECK (amount_kobo > 0),
    running_balance_kobo    BIGINT      NOT NULL,
    reference               TEXT        NOT NULL,
    description             TEXT        NOT NULL DEFAULT '',
    status                  TEXT        NOT NULL DEFAULT 'settled' CHECK (status IN ('pending','settled','failed')),
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_ledger_wallet_id  ON ledger_entries(wallet_id);
CREATE INDEX idx_ledger_reference  ON ledger_entries(reference);
CREATE UNIQUE INDEX idx_ledger_wallet_ref_kind ON ledger_entries(wallet_id, reference, kind);

-- ── Transaction summaries (public-facing, derived from ledger pairs) ──────────
CREATE TABLE transactions (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_id   UUID        NOT NULL REFERENCES wallets(id),
    kind        TEXT        NOT NULL CHECK (kind IN ('credit','debit')),
    amount_kobo BIGINT      NOT NULL CHECK (amount_kobo > 0),
    reference   TEXT        NOT NULL,
    description TEXT        NOT NULL DEFAULT '',
    status      TEXT        NOT NULL DEFAULT 'success',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_transactions_wallet_id ON transactions(wallet_id);
CREATE INDEX idx_transactions_created   ON transactions(created_at DESC);

-- ── Idempotency keys ──────────────────────────────────────────────────────────
CREATE TABLE idempotency_keys (
    key         TEXT        PRIMARY KEY,
    status      SMALLINT    NOT NULL,
    body        TEXT        NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ── Outbox (transactional outbox pattern) ─────────────────────────────────────
CREATE TABLE outbox (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    event_type  TEXT        NOT NULL,
    payload     JSONB       NOT NULL,
    status      TEXT        NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','delivered','failed')),
    attempts    INT         NOT NULL DEFAULT 0,
    next_retry  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_outbox_pending ON outbox(status, next_retry) WHERE status = 'pending';

-- ── Refresh tokens ────────────────────────────────────────────────────────────
CREATE TABLE refresh_tokens (
    token       TEXT        PRIMARY KEY,
    user_id     UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at  TIMESTAMPTZ NOT NULL,
    used        BOOLEAN     NOT NULL DEFAULT FALSE
);
CREATE INDEX idx_refresh_tokens_user ON refresh_tokens(user_id);

-- ── Ajo groups ────────────────────────────────────────────────────────────────
CREATE TABLE ajo_groups (
    id                  UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    name                TEXT        NOT NULL,
    admin_id            UUID        NOT NULL REFERENCES users(id),
    contribution_kobo   BIGINT      NOT NULL CHECK (contribution_kobo > 0),
    frequency           TEXT        NOT NULL CHECK (frequency IN ('daily','weekly','monthly')),
    member_count        INT         NOT NULL CHECK (member_count BETWEEN 2 AND 50),
    current_cycle       INT         NOT NULL DEFAULT 0,
    status              TEXT        NOT NULL DEFAULT 'active' CHECK (status IN ('active','completed','paused')),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE ajo_members (
    group_id        UUID    NOT NULL REFERENCES ajo_groups(id) ON DELETE CASCADE,
    user_id         UUID    NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    payout_position INT     NOT NULL,
    has_received    BOOLEAN NOT NULL DEFAULT FALSE,
    PRIMARY KEY (group_id, user_id)
);
CREATE INDEX idx_ajo_members_user ON ajo_members(user_id);

CREATE TABLE ajo_contributions (
    group_id    UUID    NOT NULL REFERENCES ajo_groups(id),
    user_id     UUID    NOT NULL REFERENCES users(id),
    cycle       INT     NOT NULL,
    PRIMARY KEY (group_id, user_id, cycle)
);

-- ── Bills ─────────────────────────────────────────────────────────────────────
CREATE TABLE bills (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    title       TEXT        NOT NULL,
    creator_id  UUID        NOT NULL REFERENCES users(id),
    total_kobo  BIGINT      NOT NULL CHECK (total_kobo > 0),
    status      TEXT        NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','partially_paid','settled')),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE bill_participants (
    bill_id     UUID    NOT NULL REFERENCES bills(id) ON DELETE CASCADE,
    user_id     UUID    NOT NULL REFERENCES users(id),
    share_kobo  BIGINT  NOT NULL CHECK (share_kobo > 0),
    paid        BOOLEAN NOT NULL DEFAULT FALSE,
    PRIMARY KEY (bill_id, user_id)
);
CREATE INDEX idx_bill_participants_user ON bill_participants(user_id);
