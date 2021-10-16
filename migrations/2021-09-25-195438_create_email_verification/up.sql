CREATE TABLE "email_verification" (
    token TEXT PRIMARY KEY,
    email_id uuid NOT NULL REFERENCES "email" (id)
        ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
