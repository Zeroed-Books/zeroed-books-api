CREATE TABLE "user" (
    id uuid PRIMARY KEY,
    created_at timestamptz NOT NULL DEFAULT now(),
    last_login timestamptz
);
