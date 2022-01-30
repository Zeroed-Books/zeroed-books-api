# Zeroed Books API

A potential replacement for the Zeroed Books API written in Rust using Rocket.

## Run Locally

Using `docker compose`, you can stand up a local version of the application
with:

```console
docker compose -f ./docker-compose.base.yml -f ./docker-compose.standalone.yml up
```

## Configuration

For a full list of configuration options, see the `help` command of the
application binary.

```bash
zeroed-books-api help
```

### Environment Variables

**`DATABASE_URL`:** The connection string used to connect to the primary
Postgres database.

**`REDIS_URL`:** Connection string used to connect to Redis. Redis is
used as the backing store for rate limiting.

**`SECRET_KEY`:** A secret key used primarily to encrypt private cookies. This
can be generated with: `openssl rand -base64 32`.

**`SENDGRID_KEY`:** An API token for Sendgrid. If this is provided,
transactional emails will be sent using Sendgrid. If this is left empty, the
default development setting of logging emails to the console is used.

## Deployment

If the application is running behind a proxy, ensure that the proxy populates
the `X-Real-IP` header for the request. If this is not done, all requests will
originate from the same IP resulting in frequent rate limiting.

The database must have the `uuid-ossp` extension enabled:

```sql
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
```
