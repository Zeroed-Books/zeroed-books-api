CREATE TABLE "email" (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES "user" (id)
        ON DELETE CASCADE,
    provided_address TEXT NOT NULL,
    normalized_address TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    verified_at TIMESTAMPTZ
);
