# Zeroed Books API

A potential replacement for the Zeroed Books API written in Rust using Rocket.

## Run Locally

Using `docker compose`, you can stand up a local version of the application
with:

```console
docker compose -f ./docker-compose.base.yml -f ./docker-compose.standalone.yml up
```

## Environment Variables

**`ROCKET_REDIS_URL`:** Connection string used to connect to Redis. Redis is
used as the backing store for rate limiting.

**`ROCKET_SENDGRID_KEY`:** An API token for Sendgrid. If this is provided,
transactional emails will be sent using Sendgrid. If this is left empty, the
default development setting of logging emails to the console is used.
