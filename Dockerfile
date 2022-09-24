FROM rust:1 AS builder

# Create appuser
ENV USER=zeroed-books
ENV UID=10001

RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    "${USER}"

WORKDIR /usr/src/zeroed-books-api

COPY . .
ENV SQLX_OFFLINE=true
RUN cargo build --locked --release


FROM debian:buster-slim

RUN apt-get update && apt-get install --no-install-recommends --yes \
    ca-certificates \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/* \
    && update-ca-certificates

# Import from builder.
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

WORKDIR /zeroed-books

COPY --from=builder /usr/src/zeroed-books-api/target/release/zeroed-books-api ./
COPY templates ./templates

USER zeroed-books:zeroed-books

ENTRYPOINT [ "/zeroed-books/zeroed-books-api" ]
