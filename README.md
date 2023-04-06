# Zeroed Books API

[![GitHub workflow publish status](https://img.shields.io/github/actions/workflow/status/Zeroed-Books/zeroed-books-api/publish.yml?branch=main)](https://github.com/Zeroed-Books/zeroed-books-api/actions/workflows/publish.yml)
[![GitHub Container Registry package](https://img.shields.io/badge/GHCR-zeroed--books%2Fapi-blue)](https://github.com/orgs/Zeroed-Books/packages/container/package/api)

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

These attributes are commonly provided through environment variables rather than
as CLI flags:

**`DATABASE_URL`:** The connection string used to connect to the primary
Postgres database.

**`JWT_AUDIENCE`:** The identifier for the application that will be used to
verify that JWTs are intended for consumption by the application.

**`JWT_AUTHORITY`:** The accepted issuer for JWTs.

## Deployment

The database must have the `uuid-ossp` extension enabled:

```sql
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
```
