CREATE TABLE "password_resets" (
    token TEXT PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES "user" ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
