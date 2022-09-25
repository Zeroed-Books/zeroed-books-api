INSERT INTO "currency" (code, minor_units)
VALUES ('USD', 2)
ON CONFLICT DO NOTHING;
