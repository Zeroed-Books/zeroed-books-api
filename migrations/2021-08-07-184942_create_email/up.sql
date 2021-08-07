CREATE TABLE "email" (
    provided_address TEXT NOT NULL,
    normalized_address TEXT PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES "user" (id)
        ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    verified_at TIMESTAMPTZ
);
