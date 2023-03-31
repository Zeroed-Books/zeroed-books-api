CREATE TABLE "user" (
    id uuid PRIMARY KEY,
    password TEXT NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    last_login timestamptz
);

CREATE TABLE "email" (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES "user" (id)
        ON DELETE CASCADE,
    provided_address TEXT NOT NULL,
    normalized_address TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    verified_at TIMESTAMPTZ
);

CREATE TABLE "email_verification" (
    token TEXT PRIMARY KEY,
    email_id uuid NOT NULL REFERENCES "email" (id)
        ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE "password_resets" (
    token TEXT PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES "user" ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE "account"
    ADD COLUMN legacy_user_id uuid REFERENCES "user" ON DELETE SET NULL;

ALTER TABLE "transaction"
    ADD COLUMN legacy_user_id uuid REFERENCES "user" ON DELETE SET NULL;
